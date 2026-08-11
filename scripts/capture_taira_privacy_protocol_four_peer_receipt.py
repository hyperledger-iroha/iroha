#!/usr/bin/env python3
"""Capture bounded Exact12 evidence on the secret-free qualification host.

This controller never invokes Cargo and never accepts an existing protocol
receipt.  It executes only the two prebuilt native test drivers from the
root-frozen macOS build handoff, records their complete bounded output, and
can preserve those bytes for diagnosis.  Whole-test execution is not release
authority: v2 issuance remains deliberately closed until this controller uses
the narrow action-driver IPC to own submission, direct peer queries, restarts,
and outcome validation for every retained protocol plus the independent
full-governance case.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import os
import selectors
import signal
import stat
import subprocess
import sys
import time
from pathlib import Path
from typing import NoReturn, Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

try:
    from . import taira_privacy_protocol_receipt as evidence
    from . import taira_privacy_sealed_controller as sealed_controller
    from . import taira_rollout_admission as admission
    from .release_artifact_contract import (
        ReleaseArtifactError,
        canonical_json_bytes,
        create_fresh_directory,
        exclusive_output_fd,
        exclusive_write_bytes,
        stable_hash_path,
        stable_open_relative,
        stable_read_path,
    )
except ImportError:
    import taira_privacy_protocol_receipt as evidence
    import taira_privacy_sealed_controller as sealed_controller
    import taira_rollout_admission as admission
    from release_artifact_contract import (
        ReleaseArtifactError,
        canonical_json_bytes,
        create_fresh_directory,
        exclusive_output_fd,
        exclusive_write_bytes,
        stable_hash_path,
        stable_open_relative,
        stable_read_path,
    )


DAEMON_FEATURES = "embedded-soracloud-runtime,zk-stark"
DEFAULT_CASE_TIMEOUT_SECONDS = 90 * 60


class PrivacyProtocolReceiptError(RuntimeError):
    """The candidate did not satisfy the Exact12 production-network gate."""


def _fail(message: str) -> NoReturn:
    raise PrivacyProtocolReceiptError(message)


def _require_independent_native_evidence_authority() -> None:
    """Translate the Linux native-evidence provisioning barrier."""

    try:
        admission._require_independent_native_evidence_authority()
    except admission.TairaRolloutAdmissionError as exc:
        raise PrivacyProtocolReceiptError(str(exc)) from exc


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


def _copy_frozen_file(
    source: Path,
    destination: Path,
    expected_sha256: str,
    *,
    mode: int,
    label: str,
    executable: bool,
) -> Path:
    """Copy one root-frozen handoff file without a pathname or read gap."""

    source_info = stable_hash_path(source)
    if source_info.sha256 != expected_sha256:
        _fail(f"{label} changed before installation")
    creation_mode = 0o755 if executable else 0o600
    try:
        with (
            stable_open_relative(
                source.parent, source.name, expected=source_info
            ) as source_descriptor,
            exclusive_output_fd(destination, mode=creation_mode) as target_descriptor,
        ):
            while chunk := os.read(source_descriptor, 1024 * 1024):
                view = memoryview(chunk)
                while view:
                    written = os.write(target_descriptor, view)
                    if written <= 0:
                        _fail(f"short write installing {label}")
                    view = view[written:]
    except ReleaseArtifactError as exc:
        raise PrivacyProtocolReceiptError(str(exc)) from exc
    destination.chmod(mode)
    installed = _canonical_file(destination, f"installed {label}", executable=executable)
    if stable_hash_path(installed).sha256 != expected_sha256:
        _fail(f"installed {label} digest differs")
    return installed


def _install_runtime_executable(
    source: Path, destination: Path, expected_sha256: str
) -> Path:
    """Copy one frozen handoff file into the runtime-only executable directory."""

    return _copy_frozen_file(
        source,
        destination,
        expected_sha256,
        mode=0o500,
        label="native qualification executable",
        executable=True,
    )


def _child_environment(validator_binary: Path, work_directory: Path) -> dict[str, str]:
    environment = {
        name: os.environ[name]
        for name in ("HOME", "LANG", "LC_ALL", "PATH", "TMPDIR")
        if name in os.environ
    }
    environment.update(
        {
            "IROHA_TEST_ALLOW_REENTRANT_BUILD": "0",
            "IROHA_TEST_BUILD_PROFILE": "release",
            "IROHA_TEST_REQUIRE_NETWORK": "1",
            "IROHA_TEST_SERIALIZE_NETWORKS": "1",
            "IROHA_TEST_SKIP_BUILD": "1",
            "PROFILE": "release",
            "TEST_NETWORK_BIN_IROHAD": str(validator_binary),
            "TEST_NETWORK_IROHAD_FEATURES": DAEMON_FEATURES,
            "TMPDIR": str(work_directory),
        }
    )
    return environment


def _run_bounded(
    executable: Path,
    arguments: tuple[str, ...],
    *,
    environment: dict[str, str],
    work_directory: Path,
    timeout_seconds: int,
) -> tuple[bytes, int]:
    process = subprocess.Popen(
        [str(executable), *arguments],
        cwd=work_directory,
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        close_fds=True,
        start_new_session=True,
    )
    assert process.stdout is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    deadline = time.monotonic() + timeout_seconds
    output = bytearray()
    reached_eof = False

    def kill_group() -> None:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except (PermissionError, ProcessLookupError):
            if process.poll() is None:
                process.kill()
        process.wait()

    try:
        while not reached_eof:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                kill_group()
                _fail(f"native privacy driver exceeded {timeout_seconds} seconds")
            events = selector.select(min(remaining, 1.0))
            if not events:
                if process.poll() is not None:
                    # A closed child can still leave readable bytes in the pipe.
                    events = selector.select(0)
                    if not events:
                        reached_eof = True
                continue
            for key, _mask in events:
                chunk = os.read(key.fileobj.fileno(), 64 * 1024)
                if not chunk:
                    reached_eof = True
                    break
                output.extend(chunk)
                if len(output) > evidence.MAX_COMMAND_OUTPUT_BYTES:
                    kill_group()
                    _fail(
                        "native privacy driver output exceeds the canonical transcript bound"
                    )
        status = process.wait(timeout=max(1.0, deadline - time.monotonic()))
    except BaseException:
        if process.poll() is None:
            kill_group()
        raise
    finally:
        selector.close()
        process.stdout.close()
    if not output:
        _fail("native privacy driver produced an empty transcript")
    sys.stdout.buffer.write(output)
    sys.stdout.buffer.flush()
    return bytes(output), status


def _write_case_evidence(
    output: Path,
    *,
    index: int,
    candidate_binding_sha256: str,
    drivers: dict[str, Path],
    driver_digests: dict[str, str],
    environment: dict[str, str],
    work_root: Path,
    timeout_seconds: int,
) -> dict[str, object]:
    case, kind = evidence.CASE_DEFINITIONS[index]
    command_rows: list[dict[str, object]] = []
    for command_index, (driver_name, arguments) in enumerate(
        evidence.command_plan(index)
    ):
        command_work = work_root / f"case-{index:02d}-command-{command_index:02d}"
        command_work.mkdir(mode=0o700)
        command_environment = dict(environment)
        command_environment["TMPDIR"] = str(command_work)
        output_bytes, status = _run_bounded(
            drivers[driver_name],
            arguments,
            environment=command_environment,
            work_directory=command_work,
            timeout_seconds=timeout_seconds,
        )
        if status != 0:
            _fail(f"privacy case {case!r} exited with status {status}")
        command_rows.append(
            {
                "args": list(arguments),
                "driver": driver_name,
                "driver_sha256": driver_digests[driver_name],
                "exit_code": status,
                "index": command_index,
                "output_base64": base64.b64encode(output_bytes).decode("ascii"),
                "output_sha256": hashlib.sha256(output_bytes).hexdigest(),
                "output_size": len(output_bytes),
            }
        )

    transcript_body: dict[str, object] = {
        "candidate_binding_sha256": candidate_binding_sha256,
        "case": case,
        "commands": command_rows,
        "index": index,
        "kind": kind,
        "schema": evidence.TRANSCRIPT_SCHEMA,
        "schema_version": evidence.TRANSCRIPT_SCHEMA_VERSION,
    }
    transcript_id = evidence.compute_transcript_id(transcript_body)
    transcript_payload = canonical_json_bytes(
        {**transcript_body, "transcript_id": transcript_id}
    )
    transcript_path = output / evidence.transcript_name(index)
    exclusive_write_bytes(transcript_path, transcript_payload, mode=0o600)
    transcript_sha256 = hashlib.sha256(transcript_payload).hexdigest()

    result_body: dict[str, object] = {
        "candidate_binding_sha256": candidate_binding_sha256,
        "case": case,
        "index": index,
        "kind": kind,
        "schema": evidence.RESULT_SCHEMA,
        "schema_version": evidence.RESULT_SCHEMA_VERSION,
        "status": "passed",
        "transcript_id": transcript_id,
        "transcript_path": evidence.transcript_name(index),
        "transcript_sha256": transcript_sha256,
        "transcript_size": len(transcript_payload),
    }
    result_id = evidence.compute_result_id(result_body)
    result_payload = canonical_json_bytes({**result_body, "result_id": result_id})
    result_path = output / evidence.result_name(index)
    exclusive_write_bytes(result_path, result_payload, mode=0o600)
    return {
        "case": case,
        "index": index,
        "kind": kind,
        "result_id": result_id,
        "result_path": evidence.result_name(index),
        "result_sha256": hashlib.sha256(result_payload).hexdigest(),
        "result_size": len(result_payload),
        "transcript_id": transcript_id,
        "transcript_path": evidence.transcript_name(index),
        "transcript_sha256": transcript_sha256,
        "transcript_size": len(transcript_payload),
    }


def capture(args: argparse.Namespace) -> dict[str, object]:
    # The Linux archive is part of the receipt candidate; refuse before reading
    # it or any native driver and before creating diagnostic/output state.
    _require_independent_native_evidence_authority()

    validator = _canonical_file(
        args.validator_binary, "candidate validator binary"
    )
    network_driver = _canonical_file(
        args.network_driver, "network functional driver"
    )
    jindo_driver = _canonical_file(
        args.jindo_driver, "Jindo security driver"
    )
    action_driver = _canonical_file(
        args.action_driver, "privacy action-construction driver"
    )
    linux_archive = _canonical_file(args.linux_archive, "Linux release archive")
    exact12 = _canonical_file(args.exact12_matrix, "Exact12 matrix")
    source_identity_path = _canonical_file(args.source_identity, "source identity")
    source = _source_identity(source_identity_path)
    # The separate complete controller-case registry, not the native
    # constructor table, a caller flag, or a libtest marker, is the issuance
    # barrier.  It remains empty, so the legacy whole-test capture below is
    # unreachable and cannot attest semantic success.
    try:
        sealed_controller.require_complete_release_operation_surface()
    except sealed_controller.SealedPrivacyControllerError as exc:
        raise PrivacyProtocolReceiptError(
            "sealed controller does not yet own every retained protocol case: "
            f"{exc}"
        ) from exc
    _fail(
        "privacy protocol v2 issuance is closed: no receipt emitter is connected "
        "to the complete sealed-controller case records"
    )
    output_parent = _canonical_directory(args.output_directory.parent, "output parent")
    work_parent = _canonical_directory(args.work_directory.parent, "work parent")
    if args.output_directory.parent != output_parent:
        _fail("privacy evidence output escaped its canonical parent")
    if args.work_directory.parent != work_parent:
        _fail("privacy evidence work directory escaped its canonical parent")
    if args.output_directory == args.work_directory:
        _fail("privacy evidence output and work directories must be distinct")
    try:
        output = create_fresh_directory(args.output_directory, mode=0o700)
        work_root = create_fresh_directory(args.work_directory, mode=0o700)
    except ReleaseArtifactError as exc:
        raise PrivacyProtocolReceiptError(str(exc)) from exc

    paths = {
        "action_driver": action_driver,
        "exact12": exact12,
        "jindo_driver": jindo_driver,
        "linux_archive": linux_archive,
        "network_driver": network_driver,
        "source_identity": source_identity_path,
        "validator": validator,
    }
    before = {
        name: stable_hash_path(
            path,
            max_size=(
                evidence.MAX_DRIVER_BYTES if name.endswith("_driver") else None
            ),
        )
        for name, path in paths.items()
    }
    driver_digests = {
        evidence.ACTION_DRIVER: before["action_driver"].sha256,
        evidence.JINDO_DRIVER: before["jindo_driver"].sha256,
        evidence.NETWORK_DRIVER: before["network_driver"].sha256,
    }
    driver_sources = {
        evidence.ACTION_DRIVER: action_driver,
        evidence.JINDO_DRIVER: jindo_driver,
        evidence.NETWORK_DRIVER: network_driver,
    }
    for driver_name, evidence_name in sorted(
        evidence.DRIVER_EVIDENCE_NAMES.items()
    ):
        _copy_frozen_file(
            driver_sources[driver_name],
            output / evidence_name,
            driver_digests[driver_name],
            mode=0o600,
            label=f"preserved {driver_name} bytes",
            executable=False,
        )
    executable_root = work_root / "executables"
    executable_root.mkdir(mode=0o700)
    installed_validator = _install_runtime_executable(
        validator,
        executable_root / "iroha3d",
        before["validator"].sha256,
    )
    driver_paths = {
        evidence.JINDO_DRIVER: _install_runtime_executable(
            jindo_driver,
            executable_root / evidence.JINDO_DRIVER,
            driver_digests[evidence.JINDO_DRIVER],
        ),
        evidence.NETWORK_DRIVER: _install_runtime_executable(
            network_driver,
            executable_root / evidence.NETWORK_DRIVER,
            driver_digests[evidence.NETWORK_DRIVER],
        ),
    }
    candidate: dict[str, object] = {
        "artifact_handoff_sha256": args.artifact_handoff_sha256,
        "drivers": driver_digests,
        "exact12_matrix_sha256": before["exact12"].sha256,
        "linux_release_archive_sha256": before["linux_archive"].sha256,
        "source": source.as_dict(),
        "validator_binary_sha256": before["validator"].sha256,
    }
    candidate_binding_sha256 = evidence.compute_candidate_binding_sha256(candidate)
    environment = _child_environment(installed_validator, work_root)
    case_rows = [
        _write_case_evidence(
            output,
            index=index,
            candidate_binding_sha256=candidate_binding_sha256,
            drivers=driver_paths,
            driver_digests=driver_digests,
            environment=environment,
            work_root=work_root,
            timeout_seconds=args.case_timeout_seconds,
        )
        for index in range(len(evidence.CASE_DEFINITIONS))
    ]

    for name, path in paths.items():
        if stable_hash_path(path) != before[name]:
            _fail(f"{name} changed while the privacy qualification ran")

    issued_at = int(time.time())
    receipt_body: dict[str, object] = {
        "candidate": candidate,
        "cases": case_rows,
        "expires_at_unix": issued_at + evidence.MAX_RECEIPT_LIFETIME_SECONDS,
        "issued_at_unix": issued_at,
        "outcomes": [
            {
                "case_index": case_index,
                "closed_reason": closed_reason,
                "index": index,
                "production_outcome": production_outcome,
                "profile": profile,
                "protocol": protocol,
                "security_boundary": security_boundary,
            }
            for index, (
                protocol,
                case_index,
                profile,
                production_outcome,
                closed_reason,
                security_boundary,
            ) in enumerate(evidence.OUTCOMES)
        ],
        "platform": {"arch": "arm64", "os": "macos", "peer_count": 4},
        "schema": evidence.RECEIPT_SCHEMA,
        "schema_version": evidence.RECEIPT_SCHEMA_VERSION,
    }
    receipt_id = evidence.compute_receipt_id(receipt_body)
    exclusive_write_bytes(
        output / evidence.RECEIPT_NAME,
        canonical_json_bytes({**receipt_body, "receipt_id": receipt_id}),
        mode=0o600,
    )
    evidence.validate_evidence_directory(
        output,
        expected_source=source.as_dict(),
        expected_validator_binary_sha256=before["validator"].sha256,
        expected_linux_release_archive_sha256=before["linux_archive"].sha256,
        expected_exact12_matrix_sha256=before["exact12"].sha256,
        expected_artifact_handoff_sha256=args.artifact_handoff_sha256,
        expected_receipt_id=receipt_id,
        now_unix=issued_at,
    )
    return {
        "case_count": len(case_rows),
        "evidence_directory": str(output),
        "outcome_count": len(evidence.OUTCOMES),
        "receipt_id": receipt_id,
        "validator_binary_sha256": before["validator"].sha256,
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--validator-binary", type=Path, required=True)
    parser.add_argument("--action-driver", type=Path, required=True)
    parser.add_argument("--network-driver", type=Path, required=True)
    parser.add_argument("--jindo-driver", type=Path, required=True)
    parser.add_argument("--linux-archive", type=Path, required=True)
    parser.add_argument("--exact12-matrix", type=Path, required=True)
    parser.add_argument("--source-identity", type=Path, required=True)
    parser.add_argument("--artifact-handoff-sha256", required=True)
    parser.add_argument("--output-directory", type=Path, required=True)
    parser.add_argument("--work-directory", type=Path, required=True)
    parser.add_argument(
        "--case-timeout-seconds",
        type=int,
        default=DEFAULT_CASE_TIMEOUT_SECONDS,
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.case_timeout_seconds <= 0:
            _fail("case timeout must be positive")
        result = capture(args)
    except (
        OSError,
        ReleaseArtifactError,
        PrivacyProtocolReceiptError,
        evidence.PrivacyProtocolEvidenceError,
        sealed_controller.SealedPrivacyControllerError,
        admission.TairaRolloutAdmissionError,
    ) as exc:
        print(f"Taira privacy protocol evidence refused: {exc}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write(canonical_json_bytes(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
