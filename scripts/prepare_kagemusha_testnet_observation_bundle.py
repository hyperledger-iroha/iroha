"""Package an already authenticated KAGEMUSHA release for testnet proof observation.

This command does not create proof keys, validation evidence, or release signatures.
Its input must already satisfy the complete first-release Kagami verifier. The
separate operator-pin file is a review candidate; apps must obtain their trust
anchors through an independent operator-controlled channel and must not load
this output automatically. No monetary or hardware qualification is produced.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


HEX32 = re.compile(r"[0-9a-f]{64}\Z")
MAX_CONTROL_BYTES = 128 * 1024 * 1024
MAX_EXECUTABLE_BYTES = 512 * 1024 * 1024
MAX_ARTIFACT_BYTES = 512 * 1024 * 1024
REPORT_SCHEMA = "iroha.kagemusha.v1.authenticated-release-report"
PIN_SCHEMA = "iroha.kagemusha.v1.testnet-observation-operator-pins-candidate"


class BundleError(ValueError):
    """An input or copied byte failed the closed testnet bundle contract."""


def digest_arg(value: object, label: str) -> str:
    """Accept exactly one nonzero lowercase SHA-256-width identity."""

    if not isinstance(value, str) or HEX32.fullmatch(value) is None or value == "0" * 64:
        raise BundleError(f"{label} must be 64 nonzero lowercase hex characters")
    return value


def stable_identity(left: os.stat_result, right: os.stat_result) -> bool:
    """Compare the file fields that a read must preserve, excluding access time."""

    return all(
        getattr(left, name) == getattr(right, name)
        for name in ("st_dev", "st_ino", "st_mode", "st_uid", "st_gid", "st_size", "st_mtime_ns", "st_ctime_ns", "st_nlink")
    )


def read_control(path: Path, label: str, expected_sha256: str | None = None) -> bytes:
    """Snapshot one bounded, non-symlink, single-link input using one file handle."""

    if not path.is_absolute() or path.resolve(strict=True) != path:
        raise BundleError(f"{label} path must be canonical and absolute")
    before = path.stat(follow_symlinks=False)
    limit = MAX_EXECUTABLE_BYTES if label == "Kagami executable" else MAX_CONTROL_BYTES
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        or before.st_size == 0
        or before.st_size > limit
    ):
        raise BundleError(f"{label} must be one bounded owner-controlled regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if not stable_identity(before, opened):
            raise BundleError(f"{label} changed before opening")
        with os.fdopen(descriptor, "rb", closefd=False) as source:
            payload = source.read(limit + 1)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if len(payload) != before.st_size or not stable_identity(before, after):
        raise BundleError(f"{label} changed while reading")
    if expected_sha256 is not None and hashlib.sha256(payload).hexdigest() != expected_sha256:
        raise BundleError(f"{label} differs from the independent SHA-256 pin")
    return payload


def verify_report(report: Any, args: argparse.Namespace) -> list[tuple[str, int]]:
    """Require Kagami's exact authenticated release, native loader, and artifact inventory."""

    if not isinstance(report, dict) or any(
        report.get(field) != value
        for field, value in (
            ("schema", REPORT_SCHEMA),
            ("schema_version", 1),
            ("status", "authenticated"),
            ("runtime_loaded", True),
            ("native_artifact_manifest_authenticated", True),
            ("native_artifact_hash_verified", True),
            ("authority_review_projection_sha256", args.authority_review_projection_sha256),
            ("native_artifact_manifest_sha256", args.native_artifact_manifest_sha256),
            ("release_id", args.release_id),
            ("attestation_digest", args.attestation_digest),
        )
    ):
        raise BundleError("Kagami did not authenticate the independently pinned release")
    threshold = report.get("authority_threshold")
    signers = report.get("approved_signers")
    if (
        not isinstance(threshold, int)
        or isinstance(threshold, bool)
        or threshold < 2
        or not isinstance(signers, list)
        or len(signers) < threshold
    ):
        raise BundleError("release lacks a threshold of independent approvals")
    rows = report.get("artifacts")
    if not isinstance(rows, list) or len(rows) != 50:
        raise BundleError("Kagami report lacks the exact 50 release artifacts")
    result: list[tuple[str, int]] = []
    for row in rows:
        if not isinstance(row, dict):
            raise BundleError("Kagami artifact row is malformed")
        digest = digest_arg(row.get("sha256"), "artifact SHA-256")
        size = row.get("byte_len")
        if not isinstance(size, int) or isinstance(size, bool) or not 0 < size <= MAX_ARTIFACT_BYTES:
            raise BundleError("Kagami artifact byte length is invalid")
        if any(prior_digest == digest for prior_digest, _ in result):
            raise BundleError("Kagami artifact inventory repeats a content address")
        result.append((digest, size))
    return result


def run_kagami(args: argparse.Namespace, bundle: Path | None = None) -> dict[str, Any]:
    """Execute a private copy of the separately hash-pinned Kagami binary."""

    executable = read_control(args.kagami, "Kagami executable", args.kagami_sha256)
    root = bundle / "proof" if bundle is not None else None
    argv = [
        "", "kagemusha", "authenticate-release-v1",
        "--manifest", str(root / "manifest.norito" if root else args.manifest),
        "--validation-receipt", str(root / "validation-receipt.norito" if root else args.receipt),
        "--authority-policy", str(bundle / "operator" / "authority-policy.norito" if bundle else args.authority_policy),
        "--attestation", str(root / "release-attestation.norito" if root else args.attestation),
        "--recursive-profile", str(root / "recursive-profile.json" if root else args.recursive_profile),
        "--artifact-root", str(root / "artifacts" if root else args.artifact_root),
        "--authority-review-projection", str(args.authority_review_projection),
        "--authority-review-projection-sha256", args.authority_review_projection_sha256,
        "--native-artifact-manifest", str(args.native_artifact_manifest),
        "--native-artifact-manifest-sha256", args.native_artifact_manifest_sha256,
        "--native-artifact", str(args.native_artifact),
    ]
    with tempfile.TemporaryDirectory(prefix="kagemusha-pinned-kagami-") as private_directory:
        private_executable = Path(private_directory).resolve() / "kagami"
        descriptor = os.open(private_executable, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o500)
        try:
            with os.fdopen(descriptor, "wb", closefd=False) as output:
                output.write(executable)
                output.flush()
            os.fsync(descriptor)
            os.fchmod(descriptor, 0o500)
        finally:
            os.close(descriptor)
        read_control(private_executable, "Kagami executable", args.kagami_sha256)
        argv[0] = str(private_executable)
        process = subprocess.run(
            argv, check=False, capture_output=True, text=True, timeout=1200,
            env={"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin", "TZ": "UTC"},
        )
    read_control(args.kagami, "Kagami executable", args.kagami_sha256)
    if process.returncode != 0 or len(process.stdout) > 4 * 1024 * 1024:
        raise BundleError(f"Kagami release authentication failed: {process.stderr[-2000:]}")
    try:
        report = json.loads(process.stdout)
    except json.JSONDecodeError as error:
        raise BundleError("Kagami did not return one JSON release report") from error
    verify_report(report, args)
    return report


def copy_artifact(source: Path, target: Path, digest: str, byte_len: int) -> None:
    """Copy one immutable content-addressed artifact and rehash exact copied bytes."""

    if source.is_symlink():
        raise BundleError(f"artifact {digest} is a symlink")
    source_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    source_fd = os.open(source, source_flags)
    try:
        before = os.fstat(source_fd)
        if not stat.S_ISREG(before.st_mode) or before.st_size != byte_len:
            raise BundleError(f"artifact {digest} has the wrong file type or length")
        target_fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        try:
            hasher = hashlib.sha256()
            copied = 0
            while True:
                chunk = os.read(source_fd, 1024 * 1024)
                if not chunk:
                    break
                copied += len(chunk)
                if copied > byte_len:
                    raise BundleError(f"artifact {digest} exceeds its signed length")
                hasher.update(chunk)
                view = memoryview(chunk)
                while view:
                    view = view[os.write(target_fd, view):]
            os.fsync(target_fd)
        finally:
            os.close(target_fd)
        if copied != byte_len or hasher.hexdigest() != digest or not stable_identity(before, os.fstat(source_fd)):
            raise BundleError(f"artifact {digest} changed or differs from its signed binding")
    finally:
        os.close(source_fd)


def write_json(path: Path, value: dict[str, Any]) -> None:
    """Write one canonical candidate document using an exclusive file creation."""

    payload = (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as output:
            output.write(payload)
            output.flush()
            os.fsync(descriptor)
    finally:
        os.close(descriptor)


def prepare(args: argparse.Namespace) -> None:
    """Create a complete copy only after both source and copied release authenticate."""

    for name in ("network_id", "release_id", "attestation_digest", "authority_policy_sha256",
                 "kagami_sha256", "authority_review_projection_sha256", "native_artifact_manifest_sha256"):
        digest_arg(getattr(args, name), name)
    if len({args.network_id, args.release_id, args.attestation_digest}) != 3:
        raise BundleError("network, release, and attestation pins must be distinct")
    if not args.output.is_absolute() or args.output.exists():
        raise BundleError("output must be a new absolute directory")
    if args.output.parent.resolve(strict=True) != args.output.parent:
        raise BundleError("output parent must be canonical and absolute")
    if not args.artifact_root.is_absolute() or args.artifact_root.resolve(strict=True) != args.artifact_root:
        raise BundleError("artifact root must be a canonical absolute directory")
    policy = read_control(args.authority_policy, "operator authority policy", args.authority_policy_sha256)
    controls = {
        "manifest.norito": read_control(args.manifest, "release manifest"),
        "validation-receipt.norito": read_control(args.receipt, "validation receipt"),
        "release-attestation.norito": read_control(args.attestation, "release attestation"),
        "recursive-profile.json": read_control(args.recursive_profile, "recursive profile"),
    }
    source_report = run_kagami(args)
    artifacts = verify_report(source_report, args)
    args.output.mkdir(mode=0o700)
    try:
        proof = args.output / "proof"
        operator = args.output / "operator"
        artifact_output = proof / "artifacts"
        proof.mkdir(mode=0o700)
        operator.mkdir(mode=0o700)
        artifact_output.mkdir(mode=0o700)
        for name, payload in controls.items():
            destination = proof / name
            descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            try:
                with os.fdopen(descriptor, "wb", closefd=False) as output:
                    output.write(payload)
                    output.flush()
                    os.fsync(descriptor)
            finally:
                os.close(descriptor)
        authority = operator / "authority-policy.norito"
        descriptor = os.open(authority, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        try:
            with os.fdopen(descriptor, "wb", closefd=False) as output:
                output.write(policy)
                output.flush()
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        for digest, size in artifacts:
            copy_artifact(args.artifact_root / digest, artifact_output / digest, digest, size)
        copied_report = run_kagami(args, args.output)
        if copied_report != source_report:
            raise BundleError("copied release differs from authenticated source report")
        # The typed policy digest in the report is domain-separated; the independent
        # operator pin above is SHA-256 of the exact canonical policy archive bytes.
        pins = {
            "schema": PIN_SCHEMA,
            "schema_version": 1,
            "review_status": "candidate_only",
            "network_id": args.network_id,
            "release_id": args.release_id,
            "attestation_digest": args.attestation_digest,
            "authority_policy_sha256": args.authority_policy_sha256,
            "authority_policy_digest": copied_report["authority_policy_digest"],
            "manifest_sha256": hashlib.sha256(controls["manifest.norito"]).hexdigest(),
            "receipt_sha256": hashlib.sha256(controls["validation-receipt.norito"]).hexdigest(),
            "release_attestation_sha256": hashlib.sha256(controls["release-attestation.norito"]).hexdigest(),
            "recursive_profile_sha256": hashlib.sha256(controls["recursive-profile.json"]).hexdigest(),
            "artifact_set_digest": copied_report["artifact_set_digest"],
            "artifact_count": len(artifacts),
            "hardware_qualified": False,
            "monetary_admission": False,
        }
        write_json(operator / "pins.candidate.json", pins)
        write_json(args.output / "authenticated-release-report.json", copied_report)
    except BaseException:
        shutil.rmtree(args.output)
        raise


def main(argv: list[str] | None = None) -> int:
    """Parse explicit operator roots and package a ready testnet observation candidate."""

    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("kagami", "manifest", "receipt", "authority_policy", "attestation",
                 "recursive_profile", "artifact_root", "authority_review_projection",
                 "native_artifact_manifest", "native_artifact", "output"):
        parser.add_argument("--" + name.replace("_", "-"), required=True, type=Path)
    for name in ("kagami_sha256", "network_id", "release_id", "attestation_digest",
                 "authority_policy_sha256", "authority_review_projection_sha256",
                 "native_artifact_manifest_sha256"):
        parser.add_argument("--" + name.replace("_", "-"), required=True)
    args = parser.parse_args(argv)
    try:
        prepare(args)
    except (BundleError, OSError, subprocess.TimeoutExpired) as error:
        print(f"KAGEMUSHA testnet bundle rejected: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
