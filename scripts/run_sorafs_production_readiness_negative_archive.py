#!/usr/bin/env python3
"""Archive the fixed SoraFS negative-promotion matrix without evidence payloads."""

from __future__ import annotations

import argparse
import ctypes
import errno
import hashlib
import json
import os
import platform
import selectors
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import run_sorafs_production_readiness as promotion_runner  # noqa: E402
from check_sorafs_production_readiness import (  # noqa: E402
    DEFAULT_REQUIRED_GATES,
    MAX_SUMMARY_BYTES,
    SUMMARY_SCHEMA,
    canonical_lower_hex,
    validate_aggregate_summary_output,
)
from sorafs_checker_preflight import (  # noqa: E402
    render_checker_summary,
    validate_checker_output_parent,
)
from sorafs_evidence_json import decode_evidence_json, read_evidence_bytes  # noqa: E402
from sorafs_path_identity import resolve_path_identity  # noqa: E402
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
)
from sorafs_runner_preflight import (  # noqa: E402
    emit_runner_error_lines,
    emit_runner_notice,
    plan_rendered_path_is_safe,
)
from sorafs_software_signer_evidence import (  # noqa: E402
    MAX_EXTERNAL_SIGNER_VERIFIER_BYTES,
)


ARCHIVE_SCHEMA = "sorafs.production_readiness.negative_archive.v1"
RECEIPT_SCHEMA = "sorafs.production_readiness.negative_receipt.v1"
ARCHIVE_MANIFEST_FILENAME = "negative-promotion-archive.json"
BUNDLED_RUNNER = SCRIPT_DIR / "run_sorafs_production_readiness.py"
BUNDLED_CHECKER = SCRIPT_DIR / "check_sorafs_production_readiness.py"
MAX_TOOL_BYTES = 16 * 1024 * 1024
MAX_TOOLCHAIN_BYTES = 32 * 1024 * 1024
MAX_TOOLCHAIN_FILES = 512
MAX_PROCESS_OUTPUT_BYTES = 4 * 1024 * 1024
MAX_PROCESS_SECONDS = 300
TOOLCHAIN_DIGEST_DOMAIN = (
    b"iroha:sorafs:production-readiness:negative-archive-toolchain:v1\x00"
)
EXPECTED_CHECKER_EXIT_CODE = 1
EXPECTED_AGGREGATE_STATUS = "blocked"
ARCHIVE_STATUS = "locally-qualified"
ARCHIVE_ATTESTATION_SCOPE = "local-execution-receipt"
BASELINE_INPUT_COUNT = 5 + len(DEFAULT_REQUIRED_GATES)

EXPECTED_REJECTION_FIELDS = frozenset(
    {"checker_exit_code", "aggregate_status", "diagnostic_class"}
)
OUTPUT_HASH_FIELDS = frozenset(
    {
        "aggregate_summary_sha256",
        "aggregate_semantic_sha256",
        "stdout_sha256",
        "stderr_sha256",
    }
)
RECEIPT_FIELDS = frozenset(
    {
        "schema",
        "mutation_id",
        "baseline_input_set_sha256",
        "aggregate_checker_sha256",
        "aggregate_toolchain_sha256",
        "expected_rejection",
        "observed_diagnostic_class",
        "output_sha256",
        "errors",
    }
)
BASELINE_OUTPUT_HASH_FIELDS = frozenset(
    {
        "aggregate_summary_sha256",
        "replay_summary_sha256",
        "replay_manifest_sha256",
        "stdout_sha256",
        "stderr_sha256",
    }
)
ARCHIVE_RECEIPT_ROW_FIELDS = frozenset(
    {"mutation_id", "receipt_file", "sha256"}
)
PYTHON_RUNTIME_FIELDS = frozenset(
    {"implementation", "version", "executable_sha256"}
)
ARCHIVE_FIELDS = frozenset(
    {
        "schema",
        "status",
        "attestation_scope",
        "externally_authenticated",
        "promotion_eligible",
        "baseline_input_count",
        "baseline_input_set_sha256",
        "aggregate_runner_sha256",
        "aggregate_checker_sha256",
        "aggregate_toolchain_sha256",
        "python_runtime",
        "baseline_output_sha256",
        "mutation_count",
        "mutation_ids",
        "receipts",
        "errors",
    }
)


class NegativeArchiveError(RuntimeError):
    """Raised when the negative-promotion archive cannot be qualified safely."""


@dataclass(frozen=True)
class MutationCase:
    """One closed negative-promotion mutation and its expected diagnostic."""

    mutation_id: str
    diagnostic_class: str
    diagnostic_fragment: str
    expected_aggregate_contract_errors: tuple[str, ...] = ()


MUTATION_CASES = (
    MutationCase(
        "tampered-lane-summary-bytes",
        "lane_summary_binding_mismatch",
        "foundational prerequisite lane summary binding for ai_prescreen "
        "does not match the supplied readiness summary",
        (
            "ai_prescreen aggregate foundational lane digest must match "
            "required row sha256",
        ),
    ),
    MutationCase(
        "stale-explicit-clock",
        "summary_artifact_stale",
        "exceeds max summary artifact age",
    ),
    MutationCase(
        "missing-lane-summary",
        "required_lane_missing",
        "missing required ai_prescreen production readiness summary",
    ),
    MutationCase(
        "duplicate-lane-summary",
        "required_lane_duplicate",
        "duplicate ai_prescreen production readiness summary",
    ),
    MutationCase(
        "predecessor-expectation-mismatch",
        "foundational_predecessor_mismatch",
        "foundational prerequisite previous_envelope_sha256 must match the "
        "operator-reviewed expected digest",
    ),
    MutationCase(
        "foundational-signature-forgery",
        "foundational_signature_invalid",
        "foundational prerequisite signature verification failed",
    ),
)

MUTATION_BY_ID = {case.mutation_id: case for case in MUTATION_CASES}
if len(MUTATION_BY_ID) != len(MUTATION_CASES):  # pragma: no cover - static guard
    raise RuntimeError("negative-promotion mutation ids must be unique")


@dataclass(frozen=True)
class ProcessResult:
    """One bounded local checker or runner process result."""

    exit_code: int
    stdout: bytes
    stderr: bytes


@dataclass(frozen=True)
class BaselineSnapshot:
    """Stable exact baseline input bytes and their ordered digest snapshot."""

    rows: tuple[tuple[str, Path, bytes], ...]
    digest_rows: promotion_runner.InputDigestSnapshot
    input_set_sha256: str


@dataclass(frozen=True)
class ToolchainSnapshot:
    """Exact bounded Python source inventory executed by every promotion run."""

    rows: tuple[tuple[str, Path, bytes, str], ...]
    aggregate_sha256: str
    runner_sha256: str
    checker_sha256: str


@dataclass(frozen=True)
class VerifierSnapshot:
    """Exact reviewed foundational receipt verifier staged for isolated runs."""

    source: Path
    raw: bytes
    sha256: str


@dataclass(frozen=True)
class PythonRuntime:
    """Public provenance for the external Python runtime dependency."""

    executable: Path
    implementation: str
    version: str
    executable_sha256: str

    def receipt_value(self) -> dict[str, str]:
        """Return the path-free runtime provenance stored in the archive."""

        return {
            "implementation": self.implementation,
            "version": self.version,
            "executable_sha256": self.executable_sha256,
        }


@dataclass(frozen=True)
class ArchiveParentIdentity:
    """Stable identity of the owner-controlled publication directory."""

    device: int
    inode: int


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _canonical_json_bytes(payload: Mapping[str, Any]) -> bytes:
    return (
        json.dumps(
            payload,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
        + "\n"
    ).encode("utf-8")


def _semantically_equivalent_json_mutation(raw: bytes) -> bytes:
    """Return different bounded JSON bytes that decode to the same object."""

    try:
        payload = decode_evidence_json(raw)
    except (
        UnicodeDecodeError,
        json.JSONDecodeError,
        TypeError,
        ValueError,
    ) as error:
        raise NegativeArchiveError(
            "isolated lane summary is not strict JSON"
        ) from error
    candidates = [
        json.dumps(
            payload,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8"),
        _canonical_json_bytes(payload),
        json.dumps(
            dict(reversed(tuple(payload.items()))),
            sort_keys=False,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8"),
    ]
    for candidate in candidates:
        if (
            candidate != raw
            and len(candidate) <= MAX_SUMMARY_BYTES
            and decode_evidence_json(candidate) == payload
        ):
            return candidate
    raise NegativeArchiveError(
        "isolated lane summary has no bounded semantic-preserving mutation"
    )


def _flip_lower_hex(value: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or canonical_lower_hex(value, len(value)) is None
    ):
        raise NegativeArchiveError("reviewed hexadecimal value is not canonical")
    return ("1" if value[0] == "0" else "0") + value[1:]


def _write_new_file(path: Path, payload: bytes) -> None:
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_BINARY", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = -1
    try:
        descriptor = os.open(path, flags, 0o600)
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode) or opened.st_nlink != 1:
            raise NegativeArchiveError(
                "staged archive file must be a singly-linked regular file"
            )
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise NegativeArchiveError("short write while staging archive data")
            view = view[written:]
        os.fsync(descriptor)
    except FileExistsError as error:
        raise NegativeArchiveError(
            "staged archive file must not already exist"
        ) from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)


def _replace_file_bytes(path: Path, payload: bytes) -> None:
    try:
        before = os.stat(path, follow_symlinks=False)
    except (OSError, RuntimeError) as error:
        raise NegativeArchiveError(
            "isolated mutation input cannot be inspected"
        ) from error
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise NegativeArchiveError(
            "isolated mutation input must be a singly-linked regular file"
        )
    temporary = path.with_name(f".{path.name}.mutation")
    _write_new_file(temporary, payload)
    try:
        os.replace(temporary, path)
    except (OSError, RuntimeError) as error:
        raise NegativeArchiveError(
            "isolated mutation could not be installed"
        ) from error


def _fsync_directory(path: Path) -> None:
    descriptor = -1
    try:
        descriptor = os.open(
            path,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        os.fsync(descriptor)
    except (OSError, RuntimeError) as error:
        raise NegativeArchiveError(
            "archive directory could not be synchronized"
        ) from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)


def _publish_directory_noreplace(
    staging: Path,
    destination: Path,
    parent_fd: int,
) -> None:
    """Atomically publish one directory without replacing a raced destination."""

    if (
        staging.parent != destination.parent
        or not staging.name
        or not destination.name
        or Path(staging.name).name != staging.name
        or Path(destination.name).name != destination.name
    ):
        raise NegativeArchiveError(
            "archive publication paths must share one controlled parent"
        )
    libc = ctypes.CDLL(None, use_errno=True)
    source = os.fsencode(staging.name)
    target = os.fsencode(destination.name)
    if sys.platform == "darwin" and hasattr(libc, "renameatx_np"):
        renameatx_np = libc.renameatx_np
        renameatx_np.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        renameatx_np.restype = ctypes.c_int
        result = renameatx_np(
            parent_fd,
            source,
            parent_fd,
            target,
            0x00000004,  # RENAME_EXCL
        )
    elif hasattr(libc, "renameat2"):
        renameat2 = libc.renameat2
        renameat2.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        renameat2.restype = ctypes.c_int
        result = renameat2(
            parent_fd,
            source,
            parent_fd,
            target,
            0x00000001,  # RENAME_NOREPLACE
        )
    else:
        raise NegativeArchiveError(
            "exclusive atomic archive publication is unsupported on this platform"
        )
    if result == 0:
        return
    error_number = ctypes.get_errno()
    if error_number in {errno.EEXIST, errno.ENOTEMPTY}:
        raise NegativeArchiveError("negative-promotion archive must not already exist")
    raise NegativeArchiveError("negative-promotion archive publication failed")


def _posix_process_group_exists(process_group_id: int) -> bool:
    """Return whether the private POSIX process group still has members."""

    try:
        os.killpg(process_group_id, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    except OSError:
        return True
    return True


def _wait_for_process_group_exit(
    process_group_id: int,
    process: subprocess.Popen[bytes],
    timeout_seconds: float,
) -> bool:
    """Reap the leader while waiting a bounded interval for its group to exit."""

    deadline = time.monotonic() + timeout_seconds
    while True:
        process.poll()
        if not _posix_process_group_exists(process_group_id):
            return True
        remaining_seconds = deadline - time.monotonic()
        if remaining_seconds <= 0:
            return False
        time.sleep(min(remaining_seconds, 0.01))


def _reap_direct_child(process: subprocess.Popen[bytes]) -> None:
    """Reap the direct child, force-killing it if group signaling failed."""

    try:
        process.wait(timeout=1)
        return
    except subprocess.TimeoutExpired:
        pass
    try:
        process.kill()
    except OSError:
        pass
    try:
        process.wait(timeout=1)
    except subprocess.TimeoutExpired:
        pass


def _stop_child_process(process: subprocess.Popen[bytes]) -> None:
    """Stop only the private process group created for one bounded invocation."""

    if os.name == "posix":
        process_group_id = process.pid
        try:
            os.killpg(process_group_id, signal.SIGTERM)
        except OSError:
            pass
        if not _wait_for_process_group_exit(
            process_group_id,
            process,
            1,
        ):
            try:
                os.killpg(process_group_id, signal.SIGKILL)
            except OSError:
                pass
            _wait_for_process_group_exit(
                process_group_id,
                process,
                1,
            )
        _reap_direct_child(process)
        return
    if process.poll() is None:
        try:
            process.terminate()
        except OSError:
            pass
    try:
        process.wait(timeout=1)
        return
    except subprocess.TimeoutExpired:
        pass
    _reap_direct_child(process)


def _run_bounded(command: Sequence[str], cwd: Path) -> ProcessResult:
    environment = os.environ.copy()
    environment.pop("PYTHONHOME", None)
    environment.pop("PYTHONPATH", None)
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    environment["PYTHONHASHSEED"] = "0"
    environment["PYTHONNOUSERSITE"] = "1"
    environment["PYTHONSAFEPATH"] = "1"
    process: subprocess.Popen[bytes] | None = None
    stream_selector = selectors.DefaultSelector()
    buffers = {
        "stdout": bytearray(),
        "stderr": bytearray(),
    }
    try:
        process = subprocess.Popen(
            list(command),
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            bufsize=0,
            start_new_session=os.name == "posix",
        )
        if process.stdout is None or process.stderr is None:
            raise NegativeArchiveError(
                "pinned promotion process output pipes were unavailable"
            )
        for label, stream in (
            ("stdout", process.stdout),
            ("stderr", process.stderr),
        ):
            os.set_blocking(stream.fileno(), False)
            stream_selector.register(
                stream,
                selectors.EVENT_READ,
                data=label,
            )
        deadline = time.monotonic() + MAX_PROCESS_SECONDS
        while stream_selector.get_map() or process.poll() is None:
            remaining_seconds = deadline - time.monotonic()
            if remaining_seconds <= 0:
                raise NegativeArchiveError(
                    "pinned promotion process exceeded its time bound"
                )
            events = stream_selector.select(
                timeout=min(remaining_seconds, 0.1)
            )
            for key, _event_mask in events:
                label = key.data
                buffer = buffers[label]
                read_bound = min(
                    64 * 1024,
                    MAX_PROCESS_OUTPUT_BYTES - len(buffer) + 1,
                )
                try:
                    chunk = os.read(key.fileobj.fileno(), read_bound)
                except BlockingIOError:
                    continue
                if not chunk:
                    stream_selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                buffer.extend(chunk)
                if len(buffer) > MAX_PROCESS_OUTPUT_BYTES:
                    raise NegativeArchiveError(
                        "pinned promotion process output exceeded its bound"
                    )
        if (
            os.name == "posix"
            and _posix_process_group_exists(process.pid)
        ):
            raise NegativeArchiveError(
                "pinned promotion process left live descendants"
            )
        return ProcessResult(
            process.returncode,
            bytes(buffers["stdout"]),
            bytes(buffers["stderr"]),
        )
    except NegativeArchiveError:
        if process is not None:
            _stop_child_process(process)
        raise
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        if process is not None:
            _stop_child_process(process)
        raise NegativeArchiveError(
            "pinned promotion process could not run"
        ) from error
    except BaseException:
        if process is not None:
            _stop_child_process(process)
        raise
    finally:
        stream_selector.close()
        if process is not None:
            for stream in (process.stdout, process.stderr):
                if stream is not None and not stream.closed:
                    stream.close()


def _snapshot_toolchain() -> ToolchainSnapshot:
    """Read and bind the complete bounded top-level Python tool inventory."""

    try:
        paths = sorted(SCRIPT_DIR.glob("*.py"), key=lambda path: path.name)
    except (OSError, RuntimeError) as error:
        raise NegativeArchiveError(
            "bundled promotion toolchain could not be enumerated"
        ) from error
    if (
        not paths
        or len(paths) > MAX_TOOLCHAIN_FILES
        or len({path.name for path in paths}) != len(paths)
    ):
        raise NegativeArchiveError(
            "bundled promotion toolchain inventory is outside its bound"
        )
    digest = hashlib.sha256(TOOLCHAIN_DIGEST_DOMAIN)
    digest.update(len(paths).to_bytes(4, "big"))
    rows: list[tuple[str, Path, bytes, str]] = []
    total_bytes = 0
    for path in paths:
        try:
            if path.is_symlink():
                raise NegativeArchiveError(
                    "bundled promotion toolchain must not contain symlinks"
                )
            raw = read_evidence_bytes(path, MAX_TOOL_BYTES)
        except NegativeArchiveError:
            raise
        except (OSError, RuntimeError, ValueError) as error:
            raise NegativeArchiveError(
                "bundled promotion toolchain could not be read"
            ) from error
        total_bytes += len(raw)
        if total_bytes > MAX_TOOLCHAIN_BYTES:
            raise NegativeArchiveError(
                "bundled promotion toolchain exceeds its aggregate byte bound"
            )
        name_bytes = path.name.encode("utf-8")
        file_sha256 = _sha256(raw)
        digest.update(len(name_bytes).to_bytes(2, "big"))
        digest.update(name_bytes)
        digest.update(len(raw).to_bytes(8, "big"))
        digest.update(bytes.fromhex(file_sha256))
        rows.append((path.name, path, raw, file_sha256))
    digest_by_name = {name: file_sha256 for name, _path, _raw, file_sha256 in rows}
    try:
        runner_sha256 = digest_by_name[BUNDLED_RUNNER.name]
        checker_sha256 = digest_by_name[BUNDLED_CHECKER.name]
    except KeyError as error:
        raise NegativeArchiveError(
            "bundled promotion toolchain is missing its runner or checker"
        ) from error
    return ToolchainSnapshot(
        rows=tuple(rows),
        aggregate_sha256=digest.hexdigest(),
        runner_sha256=runner_sha256,
        checker_sha256=checker_sha256,
    )


def _install_toolchain(snapshot: ToolchainSnapshot, destination: Path) -> None:
    """Install the exact source snapshot used by every child process."""

    destination.mkdir(mode=0o700)
    for name, _source, raw, expected_sha256 in snapshot.rows:
        target = destination / name
        _write_new_file(target, raw)
        if _sha256(read_evidence_bytes(target, MAX_TOOL_BYTES)) != expected_sha256:
            raise NegativeArchiveError(
                "installed promotion toolchain failed exact readback"
            )
        target.chmod(0o400)
    _fsync_directory(destination)
    destination.chmod(0o500)


def _require_toolchain_unchanged(snapshot: ToolchainSnapshot) -> None:
    observed = _snapshot_toolchain()
    expected_rows = tuple(
        (name, file_sha256)
        for name, _path, _raw, file_sha256 in snapshot.rows
    )
    observed_rows = tuple(
        (name, file_sha256)
        for name, _path, _raw, file_sha256 in observed.rows
    )
    if (
        observed.aggregate_sha256 != snapshot.aggregate_sha256
        or observed_rows != expected_rows
    ):
        raise NegativeArchiveError(
            "bundled promotion toolchain changed during negative qualification"
        )


def _snapshot_foundational_verifier(args: argparse.Namespace) -> VerifierSnapshot:
    """Read the bounded verifier and bind it to its reviewed SHA-256."""

    source = getattr(args, "foundational_signer_verifier", None)
    expected_sha256 = getattr(
        args,
        "foundational_signer_verifier_sha256",
        None,
    )
    if not isinstance(source, Path) or (
        canonical_lower_hex(expected_sha256, 64) is None
        or not any(bytes.fromhex(expected_sha256))
    ):
        raise NegativeArchiveError(
            "reviewed foundational signer verifier inputs are incomplete"
        )
    try:
        raw = read_evidence_bytes(
            source,
            MAX_EXTERNAL_SIGNER_VERIFIER_BYTES,
        )
    except (OSError, RuntimeError, ValueError) as error:
        raise NegativeArchiveError(
            "foundational signer verifier could not be snapshotted safely"
        ) from error
    observed_sha256 = _sha256(raw)
    if not raw or observed_sha256 != expected_sha256:
        raise NegativeArchiveError(
            "foundational signer verifier does not match its reviewed SHA-256"
        )
    return VerifierSnapshot(
        source=source,
        raw=raw,
        sha256=observed_sha256,
    )


def _install_foundational_verifier(
    snapshot: VerifierSnapshot,
    destination: Path,
) -> Path:
    """Install one exact executable verifier below the private work root."""

    destination.mkdir(mode=0o700)
    target = destination / "foundational-signer-verifier"
    _write_new_file(target, snapshot.raw)
    target.chmod(0o500)
    try:
        installed = read_evidence_bytes(
            target,
            MAX_EXTERNAL_SIGNER_VERIFIER_BYTES,
        )
    except (OSError, RuntimeError, ValueError) as error:
        raise NegativeArchiveError(
            "staged foundational signer verifier could not be read back"
        ) from error
    if installed != snapshot.raw or _sha256(installed) != snapshot.sha256:
        raise NegativeArchiveError(
            "staged foundational signer verifier failed exact readback"
        )
    _fsync_directory(destination)
    destination.chmod(0o500)
    return target


def _require_foundational_verifier_unchanged(
    snapshot: VerifierSnapshot,
) -> None:
    """Require the reviewed verifier source to retain its exact bytes."""

    try:
        observed = read_evidence_bytes(
            snapshot.source,
            MAX_EXTERNAL_SIGNER_VERIFIER_BYTES,
        )
    except (OSError, RuntimeError, ValueError) as error:
        raise NegativeArchiveError(
            "foundational signer verifier could not be rehashed safely"
        ) from error
    if observed != snapshot.raw or _sha256(observed) != snapshot.sha256:
        raise NegativeArchiveError(
            "foundational signer verifier changed during negative qualification"
        )


def _python_runtime() -> PythonRuntime:
    """Bind the external interpreter while keeping its filesystem path private."""

    identity_errors: list[str] = []
    executable = resolve_path_identity(
        Path(sys.executable),
        identity_errors,
        label="Python runtime",
    )
    if executable is None or identity_errors:
        raise NegativeArchiveError("Python runtime could not be bound")
    try:
        raw = read_evidence_bytes(executable, MAX_TOOL_BYTES)
    except (OSError, RuntimeError, ValueError) as error:
        raise NegativeArchiveError("Python runtime could not be bound") from error
    implementation = platform.python_implementation().lower()
    version = ".".join(
        str(component)
        for component in (
            sys.version_info.major,
            sys.version_info.minor,
            sys.version_info.micro,
        )
    )
    if not implementation or not version:
        raise NegativeArchiveError("Python runtime provenance is incomplete")
    return PythonRuntime(
        executable=executable,
        implementation=implementation,
        version=version,
        executable_sha256=_sha256(raw),
    )


def _require_python_runtime_unchanged(runtime: PythonRuntime) -> None:
    try:
        observed = read_evidence_bytes(runtime.executable, MAX_TOOL_BYTES)
    except (OSError, RuntimeError, ValueError) as error:
        raise NegativeArchiveError("Python runtime could not be rehashed") from error
    if _sha256(observed) != runtime.executable_sha256:
        raise NegativeArchiveError(
            "Python runtime changed during negative qualification"
        )


def _load_promotion_args(path: Path) -> argparse.Namespace:
    try:
        args = promotion_runner.parse_args([f"@{path}"])
    except SystemExit as error:
        raise NegativeArchiveError(
            "reviewed promotion argument file could not be parsed"
        ) from error
    if args.dry_run:
        raise NegativeArchiveError(
            "reviewed promotion argument file must not request --dry-run"
        )
    errors = promotion_runner.validate_inputs(args)
    if errors:
        raise NegativeArchiveError(
            "reviewed promotion arguments failed the pinned runner preflight"
        )
    return args


def _snapshot_baseline(args: argparse.Namespace) -> BaselineSnapshot:
    rows: list[tuple[str, Path, bytes]] = []
    digests: list[tuple[str, str]] = []
    try:
        input_paths = promotion_runner.production_input_paths(args)
    except ValueError as error:
        raise NegativeArchiveError(
            "promotion baseline does not contain the exact input inventory"
        ) from error
    if (
        tuple(slot for slot, _path in input_paths)
        != promotion_runner.REPLAY_INPUT_SLOTS
    ):
        raise NegativeArchiveError(
            "promotion baseline input slots are not in canonical order"
        )
    for slot, path in input_paths:
        try:
            raw = read_evidence_bytes(path, MAX_SUMMARY_BYTES)
        except (OSError, RuntimeError, ValueError) as error:
            raise NegativeArchiveError(
                "promotion baseline input could not be read safely"
            ) from error
        digest = _sha256(raw)
        rows.append((slot, path, raw))
        digests.append((slot, digest))
    digest_rows = tuple(digests)
    return BaselineSnapshot(
        rows=tuple(rows),
        digest_rows=digest_rows,
        input_set_sha256=promotion_runner.input_set_sha256(digest_rows),
    )


def _require_baseline_unchanged(snapshot: BaselineSnapshot) -> None:
    observed: list[tuple[str, str]] = []
    for slot, path, _raw in snapshot.rows:
        try:
            current = read_evidence_bytes(path, MAX_SUMMARY_BYTES)
        except (OSError, RuntimeError, ValueError) as error:
            raise NegativeArchiveError(
                "promotion baseline input could not be rehashed safely"
            ) from error
        observed.append((slot, _sha256(current)))
    if tuple(observed) != snapshot.digest_rows:
        raise NegativeArchiveError(
            "promotion baseline input set changed during negative qualification"
        )


def _input_filename(slot: str) -> str:
    if slot == "topology_qualification":
        return "topology-qualification.json"
    if slot == "topology_qualification_envelope":
        return "topology-qualification-envelope.json"
    if slot == "resilience_qualification":
        return "resilience-qualification.json"
    if slot == "l1_lane_evidence_inventory":
        return "l1-lane-evidence.inventory"
    if slot == "foundational_prerequisite":
        return "foundational-prerequisite.json"
    if slot not in DEFAULT_REQUIRED_GATES:
        raise NegativeArchiveError("unknown promotion input slot")
    return f"{slot.replace('_', '-')}-summary.json"


def _copy_baseline(snapshot: BaselineSnapshot, destination: Path) -> None:
    destination.mkdir(mode=0o700)
    for slot, _source, raw in snapshot.rows:
        _write_new_file(destination / _input_filename(slot), raw)
    _fsync_directory(destination)


def _promotion_runner_command(
    args: argparse.Namespace,
    toolchain_root: Path,
    runtime: PythonRuntime,
    foundational_verifier: Path,
) -> list[str]:
    command = [
        str(runtime.executable),
        "-I",
        "-B",
        str(toolchain_root / BUNDLED_RUNNER.name),
        "--verifier",
        str(toolchain_root / BUNDLED_CHECKER.name),
        "--out-dir",
        "baseline-output",
        "--summary-out",
        "baseline-output/aggregate-summary.json",
        "--topology-qualification-summary",
        _input_filename("topology_qualification"),
        "--topology-qualification-envelope",
        _input_filename("topology_qualification_envelope"),
        "--topology-qualification-verification-public-key-hex",
        args.topology_qualification_verification_public_key_hex,
        "--topology-qualification-signer-service-id",
        args.topology_qualification_signer_service_id,
        "--topology-qualification-signer-administrator-id",
        args.topology_qualification_signer_administrator_id,
        "--topology-qualification-signer-key-revision",
        str(args.topology_qualification_signer_key_revision),
        "--topology-qualification-signer-policy-revision",
        str(args.topology_qualification_signer_policy_revision),
        "--topology-qualification-signer-policy-digest-hex",
        args.topology_qualification_signer_policy_digest_hex,
        "--max-topology-qualification-review-age-secs",
        str(args.max_topology_qualification_review_age_secs),
        "--resilience-qualification-summary",
        _input_filename("resilience_qualification"),
        "--resilience-qualification-signer-public-key-hex",
        args.resilience_qualification_signer_public_key_hex,
        "--l1-lane-evidence-inventory",
        _input_filename("l1_lane_evidence_inventory"),
        "--l1-lane-evidence-inventory-verification-public-key-hex",
        args.l1_lane_evidence_inventory_verification_public_key_hex,
        "--l1-lane-evidence-inventory-signer-service-id",
        args.l1_lane_evidence_inventory_signer_service_id,
        "--l1-lane-evidence-inventory-signer-administrator-id",
        args.l1_lane_evidence_inventory_signer_administrator_id,
        "--l1-lane-evidence-inventory-signer-key-revision",
        str(args.l1_lane_evidence_inventory_signer_key_revision),
        "--l1-lane-evidence-inventory-signer-policy-revision",
        str(args.l1_lane_evidence_inventory_signer_policy_revision),
        "--l1-lane-evidence-inventory-signer-policy-digest-sha256",
        args.l1_lane_evidence_inventory_signer_policy_digest_sha256,
        "--foundational-prerequisite-summary",
        _input_filename("foundational_prerequisite"),
        "--require-gate",
        ",".join(DEFAULT_REQUIRED_GATES),
        "--now-unix",
        str(args.now_unix),
        "--max-summary-artifact-age-secs",
        str(args.max_summary_artifact_age_secs),
        "--deployment-id",
        args.deployment_id,
        "--environment",
        args.environment,
        "--foundational-prerequisite-signer-public-key-hex",
        args.foundational_signer_public_key_hex,
        "--foundational-prerequisite-signer-verifier",
        str(foundational_verifier),
        "--foundational-prerequisite-signer-verifier-sha256",
        args.foundational_signer_verifier_sha256,
        "--foundational-prerequisite-release-sequence",
        str(args.foundational_release_sequence),
        "--foundational-prerequisite-previous-envelope-sha256",
        args.foundational_previous_envelope_sha256,
    ]
    for gate in DEFAULT_REQUIRED_GATES:
        command.extend(
            [
                promotion_runner.SUMMARY_FLAGS_BY_GATE[gate],
                _input_filename(gate),
            ]
        )
    return command


def _baseline_output_hashes(
    args: argparse.Namespace,
    snapshot: BaselineSnapshot,
    root: Path,
    toolchain_root: Path,
    runtime: PythonRuntime,
    foundational_verifier: Path,
) -> dict[str, str]:
    result = _run_bounded(
        _promotion_runner_command(
            args,
            toolchain_root,
            runtime,
            foundational_verifier,
        ),
        root,
    )
    if result.exit_code != 0:
        raise NegativeArchiveError(
            "pinned promotion runner rejected the reviewed baseline"
        )
    output_root = root / "baseline-output"
    first_path = output_root / "aggregate-summary.json"
    second_path = output_root / promotion_runner.REPLAY_SUMMARY_FILENAME
    manifest_path = output_root / promotion_runner.REPLAY_MANIFEST_FILENAME
    replay, replay_errors = promotion_runner.load_and_validate_replayed_aggregates(
        first_path,
        second_path,
    )
    if replay is None or replay_errors:
        raise NegativeArchiveError(
            "pinned promotion runner baseline replay was not deterministic and ready: "
            + "; ".join(replay_errors)
        )
    try:
        manifest_raw = read_evidence_bytes(manifest_path, MAX_SUMMARY_BYTES)
        manifest = decode_evidence_json(manifest_raw)
    except (
        OSError,
        RuntimeError,
        UnicodeDecodeError,
        json.JSONDecodeError,
        ValueError,
    ) as error:
        raise NegativeArchiveError(
            "pinned promotion runner replay manifest could not be read"
        ) from error
    if promotion_runner.validate_replay_manifest(
        manifest,
        snapshot.digest_rows,
        replay,
    ):
        raise NegativeArchiveError(
            "pinned promotion runner replay manifest did not bind the baseline"
        )
    first_raw = read_evidence_bytes(first_path, MAX_SUMMARY_BYTES)
    second_raw = read_evidence_bytes(second_path, MAX_SUMMARY_BYTES)
    return {
        "aggregate_summary_sha256": _sha256(first_raw),
        "replay_summary_sha256": _sha256(second_raw),
        "replay_manifest_sha256": _sha256(manifest_raw),
        "stdout_sha256": _sha256(result.stdout),
        "stderr_sha256": _sha256(result.stderr),
    }


def _checker_command(
    args: argparse.Namespace,
    toolchain_root: Path,
    runtime: PythonRuntime,
    foundational_verifier: Path,
    *,
    now_unix: int,
    predecessor_sha256: str,
    evidence_files: Sequence[str],
) -> list[str]:
    command = [
        str(runtime.executable),
        "-I",
        "-B",
        str(toolchain_root / BUNDLED_CHECKER.name),
        "--topology-qualification-summary",
        _input_filename("topology_qualification"),
        "--topology-qualification-envelope",
        _input_filename("topology_qualification_envelope"),
        "--topology-qualification-verification-public-key-hex",
        args.topology_qualification_verification_public_key_hex,
        "--topology-qualification-signer-service-id",
        args.topology_qualification_signer_service_id,
        "--topology-qualification-signer-administrator-id",
        args.topology_qualification_signer_administrator_id,
        "--topology-qualification-signer-key-revision",
        str(args.topology_qualification_signer_key_revision),
        "--topology-qualification-signer-policy-revision",
        str(args.topology_qualification_signer_policy_revision),
        "--topology-qualification-signer-policy-digest-hex",
        args.topology_qualification_signer_policy_digest_hex,
        "--max-topology-qualification-review-age-secs",
        str(args.max_topology_qualification_review_age_secs),
        "--resilience-qualification-summary",
        _input_filename("resilience_qualification"),
        "--resilience-qualification-signer-public-key-hex",
        args.resilience_qualification_signer_public_key_hex,
        "--l1-lane-evidence-inventory",
        _input_filename("l1_lane_evidence_inventory"),
        "--l1-lane-evidence-inventory-verification-public-key-hex",
        args.l1_lane_evidence_inventory_verification_public_key_hex,
        "--l1-lane-evidence-inventory-signer-service-id",
        args.l1_lane_evidence_inventory_signer_service_id,
        "--l1-lane-evidence-inventory-signer-administrator-id",
        args.l1_lane_evidence_inventory_signer_administrator_id,
        "--l1-lane-evidence-inventory-signer-key-revision",
        str(args.l1_lane_evidence_inventory_signer_key_revision),
        "--l1-lane-evidence-inventory-signer-policy-revision",
        str(args.l1_lane_evidence_inventory_signer_policy_revision),
        "--l1-lane-evidence-inventory-signer-policy-digest-sha256",
        args.l1_lane_evidence_inventory_signer_policy_digest_sha256,
    ]
    for evidence_file in evidence_files:
        command.extend(["--evidence", evidence_file])
    for gate in DEFAULT_REQUIRED_GATES:
        command.extend(
            ["--l1-lane-summary", f"{gate}={_input_filename(gate)}"]
        )
    command.extend(
        [
            "--require-gate",
            ",".join(DEFAULT_REQUIRED_GATES),
            "--summary-out",
            "negative-aggregate.json",
            "--now-unix",
            str(now_unix),
            "--max-summary-artifact-age-secs",
            str(args.max_summary_artifact_age_secs),
            "--deployment-id",
            args.deployment_id,
            "--environment",
            args.environment,
            "--foundational-prerequisite-signer-public-key-hex",
            args.foundational_signer_public_key_hex,
            "--foundational-prerequisite-signer-verifier",
            str(foundational_verifier),
            "--foundational-prerequisite-signer-verifier-sha256",
            args.foundational_signer_verifier_sha256,
            "--foundational-prerequisite-release-sequence",
            str(args.foundational_release_sequence),
            "--foundational-prerequisite-previous-envelope-sha256",
            predecessor_sha256,
        ]
    )
    return command


def _mutate_signature(path: Path) -> None:
    try:
        payload = decode_evidence_json(read_evidence_bytes(path, MAX_SUMMARY_BYTES))
        signature = payload["signature"]
        signature_hex = signature["signature_hex"]
    except (
        KeyError,
        OSError,
        RuntimeError,
        UnicodeDecodeError,
        json.JSONDecodeError,
        TypeError,
        ValueError,
    ) as error:
        raise NegativeArchiveError(
            "foundational signature mutation input is malformed"
        ) from error
    if canonical_lower_hex(signature_hex, 128) is None:
        raise NegativeArchiveError(
            "foundational signature mutation input is not canonical"
        )
    signature["signature_hex"] = _flip_lower_hex(signature_hex)
    _replace_file_bytes(path, _canonical_json_bytes(payload))


def _apply_mutation(
    case: MutationCase,
    args: argparse.Namespace,
    root: Path,
) -> tuple[list[str], int, str]:
    evidence_files = [
        _input_filename("foundational_prerequisite"),
        *(_input_filename(gate) for gate in DEFAULT_REQUIRED_GATES),
    ]
    now_unix = args.now_unix
    predecessor = args.foundational_previous_envelope_sha256
    ai_summary = root / _input_filename("ai_prescreen")
    if case.mutation_id == "tampered-lane-summary-bytes":
        try:
            raw = read_evidence_bytes(ai_summary, MAX_SUMMARY_BYTES)
        except (OSError, RuntimeError, ValueError) as error:
            raise NegativeArchiveError(
                "isolated lane summary could not be tampered"
            ) from error
        _replace_file_bytes(
            ai_summary,
            _semantically_equivalent_json_mutation(raw),
        )
    elif case.mutation_id == "stale-explicit-clock":
        now_unix += args.max_summary_artifact_age_secs + 1
    elif case.mutation_id == "missing-lane-summary":
        try:
            ai_summary.unlink()
        except (OSError, RuntimeError) as error:
            raise NegativeArchiveError(
                "isolated lane summary could not be omitted"
            ) from error
        evidence_files.remove(_input_filename("ai_prescreen"))
    elif case.mutation_id == "duplicate-lane-summary":
        duplicate_name = "ai-prescreen-summary-duplicate.json"
        try:
            duplicate_raw = read_evidence_bytes(ai_summary, MAX_SUMMARY_BYTES)
        except (OSError, RuntimeError, ValueError) as error:
            raise NegativeArchiveError(
                "isolated lane summary could not be duplicated"
            ) from error
        _write_new_file(root / duplicate_name, duplicate_raw)
        evidence_files.append(duplicate_name)
    elif case.mutation_id == "predecessor-expectation-mismatch":
        predecessor = _flip_lower_hex(predecessor)
    elif case.mutation_id == "foundational-signature-forgery":
        _mutate_signature(root / _input_filename("foundational_prerequisite"))
    else:  # pragma: no cover - guarded by the closed constant
        raise NegativeArchiveError("unknown negative-promotion mutation")
    _fsync_directory(root)
    return evidence_files, now_unix, predecessor


def _known_diagnostic_classes(errors: Sequence[Any]) -> tuple[str, ...]:
    strings = tuple(error for error in errors if isinstance(error, str))
    return tuple(
        case.diagnostic_class
        for case in MUTATION_CASES
        if any(case.diagnostic_fragment in error for error in strings)
    )


def _aggregate_contract_matches_case(
    case: MutationCase,
    summary_errors: object,
    observed_errors: Sequence[str],
) -> bool:
    """Return whether derived aggregate errors exactly match one matrix case."""

    expected = case.expected_aggregate_contract_errors
    return (
        tuple(observed_errors) == expected
        and isinstance(summary_errors, list)
        and all(summary_errors.count(error) == 1 for error in expected)
    )


def validate_receipt(
    receipt: object,
    *,
    case: MutationCase,
    baseline_input_set_sha256: str,
    checker_sha256: str,
    toolchain_sha256: str,
) -> list[str]:
    """Validate one schema-closed payload-free negative receipt."""

    if not isinstance(receipt, Mapping):
        return ["negative-promotion receipt must be an object"]
    errors: list[str] = []
    if set(receipt) != RECEIPT_FIELDS:
        errors.append(
            "negative-promotion receipt fields must match the schema-closed contract"
        )
    if receipt.get("schema") != RECEIPT_SCHEMA:
        errors.append("negative-promotion receipt schema must match the contract")
    if receipt.get("mutation_id") != case.mutation_id:
        errors.append("negative-promotion receipt mutation id must match the matrix")
    if receipt.get("baseline_input_set_sha256") != baseline_input_set_sha256:
        errors.append(
            "negative-promotion receipt must bind the baseline input-set digest"
        )
    if receipt.get("aggregate_checker_sha256") != checker_sha256:
        errors.append("negative-promotion receipt must bind the bundled checker")
    if receipt.get("aggregate_toolchain_sha256") != toolchain_sha256:
        errors.append(
            "negative-promotion receipt must bind the executed toolchain"
        )
    expected = receipt.get("expected_rejection")
    if (
        not isinstance(expected, Mapping)
        or set(expected) != EXPECTED_REJECTION_FIELDS
        or expected.get("checker_exit_code") != EXPECTED_CHECKER_EXIT_CODE
        or expected.get("aggregate_status") != EXPECTED_AGGREGATE_STATUS
        or expected.get("diagnostic_class") != case.diagnostic_class
    ):
        errors.append(
            "negative-promotion receipt expected rejection must match the matrix"
        )
    if receipt.get("observed_diagnostic_class") != case.diagnostic_class:
        errors.append(
            "negative-promotion receipt observed diagnostic class must match "
            "expectation"
        )
    output_hashes = receipt.get("output_sha256")
    if (
        not isinstance(output_hashes, Mapping)
        or set(output_hashes) != OUTPUT_HASH_FIELDS
        or any(
            canonical_lower_hex(output_hashes.get(field), 64) is None
            for field in OUTPUT_HASH_FIELDS
        )
    ):
        errors.append(
            "negative-promotion receipt output hashes must be canonical SHA-256"
        )
    if receipt.get("errors") != []:
        errors.append("negative-promotion receipt errors must be empty")
    return errors


def _execute_mutation(
    case: MutationCase,
    args: argparse.Namespace,
    snapshot: BaselineSnapshot,
    checker_sha256: str,
    toolchain_sha256: str,
    toolchain_root: Path,
    runtime: PythonRuntime,
    foundational_verifier: Path,
    root: Path,
) -> dict[str, Any]:
    _copy_baseline(snapshot, root)
    evidence_files, now_unix, predecessor = _apply_mutation(case, args, root)
    result = _run_bounded(
        _checker_command(
            args,
            toolchain_root,
            runtime,
            foundational_verifier,
            now_unix=now_unix,
            predecessor_sha256=predecessor,
            evidence_files=evidence_files,
        ),
        root,
    )
    if result.exit_code != EXPECTED_CHECKER_EXIT_CODE:
        raise NegativeArchiveError(
            "negative-promotion checker did not return the expected rejection"
        )
    summary_path = root / "negative-aggregate.json"
    try:
        summary_raw = read_evidence_bytes(summary_path, MAX_SUMMARY_BYTES)
        summary = decode_evidence_json(summary_raw)
    except (
        OSError,
        RuntimeError,
        UnicodeDecodeError,
        json.JSONDecodeError,
        ValueError,
    ) as error:
        raise NegativeArchiveError(
            "negative-promotion checker output could not be read"
        ) from error
    if not isinstance(summary, dict) or (
        summary.get("schema") != SUMMARY_SCHEMA
        or summary.get("status") != EXPECTED_AGGREGATE_STATUS
    ):
        raise NegativeArchiveError(
            "negative-promotion checker did not emit a blocked aggregate"
        )
    aggregate_contract_errors: list[str] = []
    validate_aggregate_summary_output(
        summary,
        DEFAULT_REQUIRED_GATES,
        aggregate_contract_errors,
    )
    if not _aggregate_contract_matches_case(
        case,
        summary.get("errors"),
        aggregate_contract_errors,
    ):
        raise NegativeArchiveError(
            "negative-promotion checker output failed its schema contract"
        )
    observed_classes = _known_diagnostic_classes(summary.get("errors", []))
    if observed_classes != (case.diagnostic_class,):
        raise NegativeArchiveError(
            "negative-promotion diagnostic did not match the closed matrix"
        )
    semantic_bytes = json.dumps(
        summary,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    receipt: dict[str, Any] = {
        "schema": RECEIPT_SCHEMA,
        "mutation_id": case.mutation_id,
        "baseline_input_set_sha256": snapshot.input_set_sha256,
        "aggregate_checker_sha256": checker_sha256,
        "aggregate_toolchain_sha256": toolchain_sha256,
        "expected_rejection": {
            "checker_exit_code": EXPECTED_CHECKER_EXIT_CODE,
            "aggregate_status": EXPECTED_AGGREGATE_STATUS,
            "diagnostic_class": case.diagnostic_class,
        },
        "observed_diagnostic_class": case.diagnostic_class,
        "output_sha256": {
            "aggregate_summary_sha256": _sha256(summary_raw),
            "aggregate_semantic_sha256": _sha256(semantic_bytes),
            "stdout_sha256": _sha256(result.stdout),
            "stderr_sha256": _sha256(result.stderr),
        },
        "errors": [],
    }
    receipt_errors = validate_receipt(
        receipt,
        case=case,
        baseline_input_set_sha256=snapshot.input_set_sha256,
        checker_sha256=checker_sha256,
        toolchain_sha256=toolchain_sha256,
    )
    if receipt_errors:
        raise NegativeArchiveError(
            "negative-promotion receipt failed its schema contract"
        )
    return receipt


def validate_archive_manifest(
    manifest: object,
    *,
    baseline_input_set_sha256: str,
    runner_sha256: str,
    checker_sha256: str,
    toolchain_sha256: str,
    python_runtime: PythonRuntime,
) -> list[str]:
    """Validate the exact payload-free negative-promotion archive manifest."""

    if not isinstance(manifest, Mapping):
        return ["negative-promotion archive manifest must be an object"]
    errors: list[str] = []
    if set(manifest) != ARCHIVE_FIELDS:
        errors.append(
            "negative-promotion archive fields must match the schema-closed contract"
        )
    if manifest.get("schema") != ARCHIVE_SCHEMA:
        errors.append("negative-promotion archive schema must match the contract")
    if manifest.get("status") != ARCHIVE_STATUS:
        errors.append(
            "negative-promotion archive status must be locally-qualified"
        )
    if manifest.get("attestation_scope") != ARCHIVE_ATTESTATION_SCOPE:
        errors.append(
            "negative-promotion archive attestation scope must be local execution"
        )
    if manifest.get("externally_authenticated") is not False:
        errors.append(
            "negative-promotion archive must not claim external authentication"
        )
    if manifest.get("promotion_eligible") is not False:
        errors.append(
            "negative-promotion archive must remain locally non-promotable"
        )
    if manifest.get("baseline_input_count") != BASELINE_INPUT_COUNT:
        errors.append(
            "negative-promotion archive baseline input count must match the "
            "topology summary/envelope, resilience, signed inventory, "
            "foundation, and 17-lane inventory"
        )
    if manifest.get("baseline_input_set_sha256") != baseline_input_set_sha256:
        errors.append("negative-promotion archive must bind the baseline input set")
    if manifest.get("aggregate_runner_sha256") != runner_sha256:
        errors.append("negative-promotion archive must bind the bundled runner")
    if manifest.get("aggregate_checker_sha256") != checker_sha256:
        errors.append("negative-promotion archive must bind the bundled checker")
    if manifest.get("aggregate_toolchain_sha256") != toolchain_sha256:
        errors.append(
            "negative-promotion archive must bind the executed toolchain"
        )
    runtime_value = manifest.get("python_runtime")
    if (
        not isinstance(runtime_value, Mapping)
        or set(runtime_value) != PYTHON_RUNTIME_FIELDS
        or runtime_value != python_runtime.receipt_value()
        or canonical_lower_hex(runtime_value.get("executable_sha256"), 64)
        is None
    ):
        errors.append(
            "negative-promotion archive must bind the external Python runtime"
        )
    baseline_hashes = manifest.get("baseline_output_sha256")
    if (
        not isinstance(baseline_hashes, Mapping)
        or set(baseline_hashes) != BASELINE_OUTPUT_HASH_FIELDS
        or any(
            canonical_lower_hex(baseline_hashes.get(field), 64) is None
            for field in BASELINE_OUTPUT_HASH_FIELDS
        )
    ):
        errors.append(
            "negative-promotion archive baseline output hashes must be canonical"
        )
    expected_ids = [case.mutation_id for case in MUTATION_CASES]
    if manifest.get("mutation_count") != len(MUTATION_CASES):
        errors.append("negative-promotion archive mutation count must be six")
    if manifest.get("mutation_ids") != expected_ids:
        errors.append(
            "negative-promotion archive mutation ids must match the closed matrix"
        )
    receipt_rows = manifest.get("receipts")
    if not isinstance(receipt_rows, list) or len(receipt_rows) != len(MUTATION_CASES):
        errors.append("negative-promotion archive must contain six receipt rows")
    else:
        observed_ids: list[Any] = []
        observed_files: list[Any] = []
        for row in receipt_rows:
            if not isinstance(row, Mapping) or set(row) != ARCHIVE_RECEIPT_ROW_FIELDS:
                errors.append(
                    "negative-promotion archive receipt rows must match the contract"
                )
                continue
            observed_ids.append(row.get("mutation_id"))
            observed_files.append(row.get("receipt_file"))
            if canonical_lower_hex(row.get("sha256"), 64) is None:
                errors.append(
                    "negative-promotion archive receipt digest must be canonical"
                )
        if observed_ids != expected_ids:
            errors.append(
                "negative-promotion archive receipt rows must use matrix order"
            )
        expected_files = [
            f"{index:02d}-{case.mutation_id}.json"
            for index, case in enumerate(MUTATION_CASES, start=1)
        ]
        if observed_files != expected_files:
            errors.append(
                "negative-promotion archive receipt filenames must match the matrix"
            )
    if manifest.get("errors") != []:
        errors.append("negative-promotion archive errors must be empty")
    return errors


def _publish_archive(
    archive_out_dir: Path,
    *,
    parent_identity: ArchiveParentIdentity,
    snapshot: BaselineSnapshot,
    runner_sha256: str,
    checker_sha256: str,
    toolchain_sha256: str,
    python_runtime: PythonRuntime,
    baseline_output_hashes: dict[str, str],
    receipts: Sequence[dict[str, Any]],
) -> None:
    if len(receipts) != len(MUTATION_CASES):
        raise NegativeArchiveError(
            "negative-promotion archive requires exactly six receipts"
        )
    parent = archive_out_dir.parent
    parent_fd = -1
    staging: Path | None = None
    published = False
    try:
        parent_fd = os.open(
            parent,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        opened_parent = os.fstat(parent_fd)
        parent_problem = _archive_parent_problem(opened_parent)
        if (
            parent_problem is not None
            or opened_parent.st_dev != parent_identity.device
            or opened_parent.st_ino != parent_identity.inode
        ):
            raise NegativeArchiveError(
                "archive publication parent changed or became untrusted"
            )
        staging = Path(
            tempfile.mkdtemp(
                prefix=".sorafs-negative-promotion-staging-",
                dir=parent,
            )
        )
        staging.chmod(0o700)
        receipt_rows: list[dict[str, str]] = []
        for index, (case, receipt) in enumerate(
            zip(MUTATION_CASES, receipts),
            start=1,
        ):
            filename = f"{index:02d}-{case.mutation_id}.json"
            raw = render_checker_summary(receipt).encode("utf-8")
            _write_new_file(staging / filename, raw)
            receipt_rows.append(
                {
                    "mutation_id": case.mutation_id,
                    "receipt_file": filename,
                    "sha256": _sha256(raw),
                }
            )
        manifest: dict[str, Any] = {
            "schema": ARCHIVE_SCHEMA,
            "status": ARCHIVE_STATUS,
            "attestation_scope": ARCHIVE_ATTESTATION_SCOPE,
            "externally_authenticated": False,
            "promotion_eligible": False,
            "baseline_input_count": BASELINE_INPUT_COUNT,
            "baseline_input_set_sha256": snapshot.input_set_sha256,
            "aggregate_runner_sha256": runner_sha256,
            "aggregate_checker_sha256": checker_sha256,
            "aggregate_toolchain_sha256": toolchain_sha256,
            "python_runtime": python_runtime.receipt_value(),
            "baseline_output_sha256": baseline_output_hashes,
            "mutation_count": len(MUTATION_CASES),
            "mutation_ids": [case.mutation_id for case in MUTATION_CASES],
            "receipts": receipt_rows,
            "errors": [],
        }
        manifest_errors = validate_archive_manifest(
            manifest,
            baseline_input_set_sha256=snapshot.input_set_sha256,
            runner_sha256=runner_sha256,
            checker_sha256=checker_sha256,
            toolchain_sha256=toolchain_sha256,
            python_runtime=python_runtime,
        )
        if manifest_errors:
            raise NegativeArchiveError(
                "negative-promotion archive failed its schema contract"
            )
        _write_new_file(
            staging / ARCHIVE_MANIFEST_FILENAME,
            render_checker_summary(manifest).encode("utf-8"),
        )
        for case, row, expected_receipt in zip(
            MUTATION_CASES,
            receipt_rows,
            receipts,
        ):
            receipt_path = staging / row["receipt_file"]
            receipt_raw = read_evidence_bytes(receipt_path, MAX_SUMMARY_BYTES)
            if _sha256(receipt_raw) != row["sha256"]:
                raise NegativeArchiveError(
                    "staged negative-promotion receipt digest does not match"
                )
            receipt = decode_evidence_json(receipt_raw)
            if receipt != expected_receipt or validate_receipt(
                receipt,
                case=case,
                baseline_input_set_sha256=snapshot.input_set_sha256,
                checker_sha256=checker_sha256,
                toolchain_sha256=toolchain_sha256,
            ):
                raise NegativeArchiveError(
                    "staged negative-promotion receipt failed readback"
                )
        manifest_raw = read_evidence_bytes(
            staging / ARCHIVE_MANIFEST_FILENAME,
            MAX_SUMMARY_BYTES,
        )
        if decode_evidence_json(manifest_raw) != manifest:
            raise NegativeArchiveError(
                "staged negative-promotion archive failed readback"
            )
        _fsync_directory(staging)
        _publish_directory_noreplace(staging, archive_out_dir, parent_fd)
        published = True
        os.fsync(parent_fd)
    finally:
        if not published and staging is not None:
            shutil.rmtree(staging, ignore_errors=True)
        if parent_fd >= 0:
            os.close(parent_fd)


def _archive_parent_problem(parent_stat: os.stat_result) -> str | None:
    if not stat.S_ISDIR(parent_stat.st_mode):
        return "--archive-out-dir parent must be a directory"
    get_effective_user = getattr(os, "geteuid", None)
    if get_effective_user is not None:
        if parent_stat.st_uid != get_effective_user():
            return "--archive-out-dir parent must be owned by the current user"
        if stat.S_IMODE(parent_stat.st_mode) & 0o022:
            return (
                "--archive-out-dir parent must not be group- or "
                "world-writable"
            )
    return None


def _capture_archive_parent(path: Path) -> ArchiveParentIdentity:
    try:
        parent_stat = os.stat(path.parent, follow_symlinks=False)
    except (OSError, RuntimeError) as error:
        raise NegativeArchiveError(
            "archive publication parent could not be bound"
        ) from error
    problem = _archive_parent_problem(parent_stat)
    if problem is not None:
        raise NegativeArchiveError(problem)
    return ArchiveParentIdentity(
        device=parent_stat.st_dev,
        inode=parent_stat.st_ino,
    )


def _validate_archive_output(path: Path) -> list[str]:
    errors: list[str] = []
    if not isinstance(path, Path) or not plan_rendered_path_is_safe(path):
        return ["--archive-out-dir must be a canonical safe artifact path"]
    validate_checker_output_parent(path, errors, label="--archive-out-dir")
    try:
        if path.is_symlink():
            errors.append("--archive-out-dir must not be a symlink")
        elif path.exists():
            errors.append("--archive-out-dir must not already exist")
        parent_stat = os.stat(path.parent, follow_symlinks=False)
        parent_problem = _archive_parent_problem(parent_stat)
        if parent_problem is not None:
            errors.append(parent_problem)
    except FileNotFoundError:
        errors.append("--archive-out-dir parent must already exist")
    except (OSError, RuntimeError):
        errors.append("--archive-out-dir cannot be inspected")
    return errors


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse the negative-promotion archive runner arguments."""

    parser = EvidenceArgumentParser(
        description=(
            "Run and archive the fixed SoraFS negative-promotion matrix over "
            "isolated copies of one reviewed ready input set."
        )
    )
    parser.add_argument(
        "--promotion-args-file",
        type=Path,
        required=True,
        help=(
            "Reviewed @ARGFILE accepted by run_sorafs_production_readiness.py. "
            "Its configured outputs are ignored in favor of isolated outputs."
        ),
    )
    parser.add_argument(
        "--archive-out-dir",
        type=Path,
        required=True,
        help=(
            "New directory for six payload-free receipts and their archive "
            "manifest. The parent must already exist, be owned by the current "
            "user, and not be group- or world-writable."
        ),
    )
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_runner_error_lines((str(error),))
        raise SystemExit(2) from error
    return parser.parse_args(expanded)


def main(argv: list[str] | None = None) -> int:
    """Run the baseline, fixed negative matrix, and atomic receipt publication."""

    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1
    destination = (
        args.archive_out_dir
        if args.archive_out_dir.is_absolute()
        else Path.cwd() / args.archive_out_dir
    )
    output_errors = _validate_archive_output(destination)
    if output_errors:
        emit_runner_error_lines(output_errors)
        return 2
    try:
        parent_identity = _capture_archive_parent(destination)
        toolchain = _snapshot_toolchain()
        runtime = _python_runtime()
        promotion_args = _load_promotion_args(args.promotion_args_file)
        verifier = _snapshot_foundational_verifier(promotion_args)
        snapshot = _snapshot_baseline(promotion_args)
        with tempfile.TemporaryDirectory(
            prefix="sorafs-negative-promotion-work-"
        ) as temporary:
            work_root_errors: list[str] = []
            work_root = resolve_path_identity(
                Path(temporary),
                work_root_errors,
                label="negative-promotion work root",
            )
            if work_root is None or work_root_errors:
                raise NegativeArchiveError(
                    "negative-promotion work root could not be bound"
                )
            toolchain_root = work_root / "toolchain"
            _install_toolchain(toolchain, toolchain_root)
            foundational_verifier = _install_foundational_verifier(
                verifier,
                work_root / "runtime-trust",
            )
            baseline_root = work_root / "baseline"
            _copy_baseline(snapshot, baseline_root)
            baseline_hashes = _baseline_output_hashes(
                promotion_args,
                snapshot,
                baseline_root,
                toolchain_root,
                runtime,
                foundational_verifier,
            )
            receipts = []
            for index, case in enumerate(MUTATION_CASES, start=1):
                receipts.append(
                    _execute_mutation(
                        case,
                        promotion_args,
                        snapshot,
                        toolchain.checker_sha256,
                        toolchain.aggregate_sha256,
                        toolchain_root,
                        runtime,
                        foundational_verifier,
                        work_root / f"negative-{index:02d}",
                    )
                )
        _require_baseline_unchanged(snapshot)
        _require_foundational_verifier_unchanged(verifier)
        _require_toolchain_unchanged(toolchain)
        _require_python_runtime_unchanged(runtime)
        _publish_archive(
            destination,
            parent_identity=parent_identity,
            snapshot=snapshot,
            runner_sha256=toolchain.runner_sha256,
            checker_sha256=toolchain.checker_sha256,
            toolchain_sha256=toolchain.aggregate_sha256,
            python_runtime=runtime,
            baseline_output_hashes=baseline_hashes,
            receipts=receipts,
        )
    except NegativeArchiveError as error:
        emit_runner_error_lines((str(error),))
        return 1
    except (
        OSError,
        RuntimeError,
        TypeError,
        UnicodeDecodeError,
        json.JSONDecodeError,
        ValueError,
    ):
        emit_runner_error_lines(
            ("negative-promotion archive failed during bounded local processing",)
        )
        return 1
    emit_runner_notice(
        "SoraFS negative-promotion archive locally qualified all six fixed "
        "rejection cases; external provenance is still required."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
