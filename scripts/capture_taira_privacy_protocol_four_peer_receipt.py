#!/usr/bin/env python3
"""Run the canonical Exact12 production-network gates and emit one receipt.

The receipt is deliberately separate from native proof-fixture evidence and
from the generic validator restart receipt.  It is created only after the
fixed four-peer integration targets have run against the exact prebuilt
candidate ``irohad``.  The resulting body binds that binary, the signed Linux
release archive, the canonical Exact12 matrix, and the complete source
identity.  A later signed candidate archive makes the domain-separated receipt
ID cryptographically authoritative.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import stat
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import NoReturn, Sequence

try:
    from .release_artifact_contract import (
        ReleaseArtifactError,
        canonical_json_bytes,
        exclusive_write_bytes,
        stable_hash_path,
        stable_read_path,
    )
    from . import taira_rollout_admission as admission
except ImportError:
    from release_artifact_contract import (
        ReleaseArtifactError,
        canonical_json_bytes,
        exclusive_write_bytes,
        stable_hash_path,
        stable_read_path,
    )
    import taira_rollout_admission as admission


NETWORK_FEATURES = "zk-stark privacy-release-evidence"
DAEMON_FEATURES = "embedded-soracloud-runtime,zk-stark"
JINDO_SECURITY_FILTER = "privacy_engines::jindo::security::tests"


class PrivacyProtocolReceiptError(RuntimeError):
    """The candidate did not satisfy the Exact12 production-network gate."""


def _fail(message: str) -> NoReturn:
    raise PrivacyProtocolReceiptError(message)


def _canonical_file(path: Path, label: str, *, executable: bool = False) -> Path:
    if not path.is_absolute():
        _fail(f"{label} must be an absolute path")
    try:
        resolved = path.resolve(strict=True)
        info = path.lstat()
    except OSError as exc:
        raise PrivacyProtocolReceiptError(f"cannot inspect {label}: {exc}") from exc
    if resolved != path or stat.S_ISLNK(info.st_mode):
        _fail(f"{label} must be canonical and must not be a symlink")
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_size <= 0:
        _fail(f"{label} must be one non-empty singly linked regular file")
    if executable and info.st_mode & 0o111 == 0:
        _fail(f"{label} must be executable")
    return path


def _canonical_directory(path: Path, label: str) -> Path:
    if not path.is_absolute():
        _fail(f"{label} must be an absolute path")
    try:
        resolved = path.resolve(strict=True)
        info = path.lstat()
    except OSError as exc:
        raise PrivacyProtocolReceiptError(f"cannot inspect {label}: {exc}") from exc
    if resolved != path or stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
        _fail(f"{label} must be one canonical non-symlink directory")
    return path


def _source_identity(path: Path) -> admission.SourceIdentity:
    _, payload = stable_read_path(path, max_size=1024 * 1024)
    value = admission._canonical_object(payload, "Taira source identity")
    admission._exact_fields(
        value, {"source", "source_date_epoch"}, "Taira source identity"
    )
    return admission._source_identity(value["source"], "Taira source identity")


def _case_commands(case: str) -> tuple[tuple[str, ...], ...]:
    network = (
        "cargo",
        "test",
        "--locked",
        "--release",
        "-p",
        "integration_tests",
        "--test",
        "network_functional",
        "--features",
        NETWORK_FEATURES,
        case,
        "--",
        "--exact",
        "--nocapture",
        "--test-threads=1",
    )
    if case != PRIVACY_CASES["iroha-jindo-polynomial-commitment-v0"]:
        return (network,)
    jindo_certificate = (
        "cargo",
        "test",
        "--locked",
        "--release",
        "-p",
        "iroha_core",
        "--lib",
        JINDO_SECURITY_FILTER,
        "--",
        "--nocapture",
        "--test-threads=1",
    )
    return (network, jindo_certificate)


PRIVACY_CASES = {
    row[0]: row[1]
    for row in admission.PRIVACY_PROTOCOL_FOUR_PEER_OUTCOMES_V1
}


def _run_case(
    case: str,
    *,
    repository: Path,
    target_dir: Path,
    validator_binary: Path,
    log_path: Path,
) -> str:
    environment = os.environ.copy()
    environment.update(
        {
            "CARGO_BUILD_JOBS": "1",
            "CARGO_TARGET_DIR": str(target_dir),
            "IROHA_TEST_ALLOW_REENTRANT_BUILD": "0",
            "IROHA_TEST_BUILD_PROFILE": "release",
            "IROHA_TEST_REQUIRE_NETWORK": "1",
            "IROHA_TEST_SERIALIZE_NETWORKS": "1",
            "IROHA_TEST_SKIP_BUILD": "1",
            "PROFILE": "release",
            "TEST_NETWORK_BIN_IROHAD": str(validator_binary),
            "TEST_NETWORK_IROHAD_FEATURES": DAEMON_FEATURES,
        }
    )
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC
    descriptor = os.open(log_path, flags, 0o600)
    validator_sha256 = stable_hash_path(validator_binary).sha256
    try:
        for command in _case_commands(case):
            header = (
                f"TEST_NETWORK_BIN_IROHAD_SHA256={validator_sha256}\n"
                + "$ "
                + " ".join(command)
                + "\n"
            ).encode("utf-8")
            os.write(descriptor, header)
            sys.stdout.buffer.write(header)
            sys.stdout.buffer.flush()
            process = subprocess.Popen(
                command,
                cwd=repository,
                env=environment,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
            )
            assert process.stdout is not None
            output_tail = b""
            while chunk := process.stdout.read(64 * 1024):
                os.write(descriptor, chunk)
                sys.stdout.buffer.write(chunk)
                sys.stdout.buffer.flush()
                output_tail = (output_tail + chunk)[-(4 * 1024 * 1024) :]
            status = process.wait()
            if status != 0:
                _fail(f"privacy four-peer case {case!r} exited with status {status}")
            lowered = output_tail.lower()
            if b"running 0 tests" in lowered:
                _fail(f"privacy four-peer case {case!r} executed zero tests")
            if b"fixture-only" in lowered or b"fixture_only" in lowered:
                _fail(f"privacy four-peer case {case!r} reported fixture-only evidence")
            passed = re.findall(
                rb"test result: ok\. ([1-9][0-9]*) passed; 0 failed; 0 ignored;",
                output_tail,
            )
            if not passed:
                _fail(
                    f"privacy four-peer case {case!r} lacks an unskipped passing test result"
                )
            is_network_case = "integration_tests" in command
            if is_network_case:
                marker = (
                    "TAIRA_PRIVACY_PROTOCOL_FOUR_PEER_CASE_V1:"
                    f"{case}:passed"
                ).encode("ascii")
                if marker not in output_tail:
                    _fail(
                        f"privacy four-peer case {case!r} lacks its post-query/restart marker"
                    )
                if b"running 1 test" not in lowered or passed[-1] != b"1":
                    _fail(
                        f"privacy four-peer case {case!r} did not execute exactly one named test"
                    )
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    return stable_hash_path(log_path).sha256


def capture(args: argparse.Namespace) -> dict[str, object]:
    repository = _canonical_directory(args.repository, "repository")
    validator = _canonical_file(
        args.validator_binary, "candidate validator binary", executable=True
    )
    linux_archive = _canonical_file(args.linux_archive, "Linux release archive")
    exact12 = _canonical_file(args.exact12_matrix, "Exact12 matrix")
    source_identity_path = _canonical_file(args.source_identity, "source identity")
    source = _source_identity(source_identity_path)
    target_dir = _canonical_directory(args.cargo_target_dir, "Cargo target directory")
    if args.output.exists() or args.output.is_symlink() or not args.output.is_absolute():
        _fail("receipt output must be one new absolute path")
    if args.output.parent.resolve(strict=True) != args.output.parent:
        _fail("receipt output parent must be canonical")

    head = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repository,
        check=False,
        capture_output=True,
        text=True,
    )
    if head.returncode != 0 or head.stdout.strip() != source.commit:
        _fail("receipt repository HEAD differs from the candidate source identity")
    cargo_lock = _canonical_file(repository / "Cargo.lock", "Cargo.lock")
    if stable_hash_path(cargo_lock).sha256 != source.cargo_lock_sha256:
        _fail("receipt Cargo.lock differs from the candidate source identity")

    candidate_files = {
        "validator": stable_hash_path(validator),
        "linux_archive": stable_hash_path(linux_archive),
        "exact12": stable_hash_path(exact12),
        "source_identity": stable_hash_path(source_identity_path),
    }
    unique_cases = tuple(dict.fromkeys(PRIVACY_CASES.values()))
    with tempfile.TemporaryDirectory(
        prefix="taira-privacy-protocol-four-peer-"
    ) as raw_logs:
        log_root = Path(raw_logs).resolve(strict=True)
        case_digests: dict[str, str] = {}
        for ordinal, case in enumerate(unique_cases, start=1):
            case_digests[case] = _run_case(
                case,
                repository=repository,
                target_dir=target_dir,
                validator_binary=validator,
                log_path=log_root / f"case-{ordinal:02d}.log",
            )

    for label, before in candidate_files.items():
        path = {
            "validator": validator,
            "linux_archive": linux_archive,
            "exact12": exact12,
            "source_identity": source_identity_path,
        }[label]
        if stable_hash_path(path) != before:
            _fail(f"{label} changed while the privacy four-peer cases ran")

    issued_at = int(time.time())
    body: dict[str, object] = {
        "candidate": {
            "exact12_matrix_sha256": candidate_files["exact12"].sha256,
            "linux_release_archive_sha256": candidate_files[
                "linux_archive"
            ].sha256,
            "source": source.as_dict(),
            "validator_binary_sha256": candidate_files["validator"].sha256,
        },
        "expires_at_unix": (
            issued_at + admission.MAX_PRIVACY_PROTOCOL_RECEIPT_LIFETIME_SECONDS
        ),
        "issued_at_unix": issued_at,
        "outcomes": [
            {
                "case": case,
                "case_output_sha256": case_digests[case],
                "closed_reason": closed_reason,
                "index": index,
                "production_outcome": production_outcome,
                "profile": profile,
                "protocol": protocol,
                "security_boundary": security_boundary,
                "validator_binary_sha256": candidate_files["validator"].sha256,
            }
            for index, (
                protocol,
                case,
                profile,
                production_outcome,
                closed_reason,
                security_boundary,
            ) in enumerate(admission.PRIVACY_PROTOCOL_FOUR_PEER_OUTCOMES_V1)
        ],
        "platform": {"arch": "arm64", "os": "macos", "peer_count": 4},
        "schema": admission.PRIVACY_PROTOCOL_RECEIPT_SCHEMA,
        "schema_version": admission.PRIVACY_PROTOCOL_RECEIPT_SCHEMA_VERSION,
    }
    receipt_id = admission.compute_privacy_protocol_receipt_id(body)
    receipt = {**body, "receipt_id": receipt_id}
    payload = canonical_json_bytes(receipt)
    admission._validate_privacy_protocol_receipt(
        payload,
        expected_source=source,
        expected_validator_binary_sha256=candidate_files["validator"].sha256,
        expected_linux_release_archive_sha256=candidate_files[
            "linux_archive"
        ].sha256,
        expected_exact12_matrix_sha256=candidate_files["exact12"].sha256,
        expected_receipt_id=receipt_id,
        now_unix=issued_at,
    )
    exclusive_write_bytes(args.output, payload, mode=0o600)
    return {
        "linux_release_archive_sha256": candidate_files["linux_archive"].sha256,
        "outcome_count": len(admission.PRIVACY_PROTOCOL_FOUR_PEER_OUTCOMES_V1),
        "output": str(args.output),
        "receipt_id": receipt_id,
        "validator_binary_sha256": candidate_files["validator"].sha256,
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--validator-binary", type=Path, required=True)
    parser.add_argument("--linux-archive", type=Path, required=True)
    parser.add_argument("--exact12-matrix", type=Path, required=True)
    parser.add_argument("--source-identity", type=Path, required=True)
    parser.add_argument("--cargo-target-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        result = capture(args)
    except (
        OSError,
        ReleaseArtifactError,
        PrivacyProtocolReceiptError,
        admission.TairaRolloutAdmissionError,
    ) as exc:
        print(f"Taira privacy protocol receipt refused: {exc}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write(canonical_json_bytes(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
