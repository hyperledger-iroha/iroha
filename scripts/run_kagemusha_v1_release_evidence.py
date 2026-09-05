"""Collect, approve, and verify one KAGEMUSHA V1 release-evidence closure.

The runner deliberately separates candidate-selected production from locally
trusted verification.  ``collect`` executes only hash-pinned commands from a
hash-pinned canonical plan, captures immutable verification subjects, and
stops with unsigned signing requests.  ``finalize`` accepts detached public
approvals, revalidates every byte, and invokes the separately pinned release
verifier.  No private signing material is accepted by this program.

Invoke this file through an absolute interpreter path with ``-I -B -S``.  A
PATH-selected shebang cannot establish the runtime identity required by this
release gate, so direct script execution is intentionally unsupported.

The runner is not an OS sandbox.  Production use requires an otherwise idle,
dedicated operator account and an administrator-controlled Python/runtime
installation.  Candidate-selected commands are accepted only as root-owned
native executables under root-owned, non-publicly-writable directories; their
host dynamic loader and system libraries remain part of the sealed host trust
base rather than candidate evidence.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import os
import re
import resource
import shutil
import signal
import stat
import subprocess
import sys
import time
import types
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping, NoReturn, Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
# Local release code is intentionally not imported here.  ``main`` first
# authenticates the interpreter and both local source files using only the
# standard library, then loads those exact bytes.  These names are populated by
# ``_bootstrap_local_modules`` before any plan or policy data is processed.
artifact_contract: Any = None
release_verifier: Any = None
ReleaseArtifactError: Any = RuntimeError
StableFile: Any = object
canonical_json_bytes: Any = None
canonical_relative_path: Any = None
load_json_object: Any = None
stable_hash_path: Any = None
stable_hash_relative: Any = None
stable_read_path: Any = None


PLAN_SCHEMA = "iroha.kagemusha_v1.release_evidence_collection_plan"
COLLECTION_STATE_SCHEMA = "iroha.kagemusha_v1.release_evidence_collection_state"
SIGNING_REQUEST_SCHEMA = "iroha.kagemusha_v1.verification_signing_request"
DETACHED_APPROVAL_SCHEMA = "iroha.kagemusha_v1.detached_verification_approval"
RUN_RESULT_SCHEMA = "iroha.kagemusha_v1.release_evidence_runner_result"
SCHEMA_VERSION = 1

BUNDLED_RELEASE_VERIFIER = SCRIPT_DIR / "verify_kagemusha_v1_release_evidence.py"
BUNDLED_ARTIFACT_CONTRACT = SCRIPT_DIR / "release_artifact_contract.py"
RESOLVED_PYTHON = Path(sys.executable).resolve(strict=True)

MAX_PLAN_BYTES = 16 * 1024 * 1024
MAX_COLLECTION_STATE_BYTES = 32 * 1024 * 1024
MAX_APPROVAL_BYTES = 256 * 1024
MAX_STEP_TIMEOUT_MS = 60 * 60 * 1_000
MAX_PRODUCER_TRANSCRIPT_BYTES = 16 * 1024 * 1024
MAX_RUNTIME_EXECUTABLE_BYTES = 512 * 1024 * 1024
MAX_PROCESS_POLL_SECONDS = 0.02
TERMINATION_GRACE_SECONDS = 1.0
EXIT_PENDING_APPROVALS = 2

_HEX_64 = re.compile(r"[0-9a-f]{64}")
_SAFE_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+-]{0,127}")
_SAFE_PRODUCER_LITERAL = re.compile(r"[^\x00-\x1f\x7f]{1,1024}")
_NATIVE_MAGICS = (
    b"\x7fELF",
    b"\xcf\xfa\xed\xfe",
    b"\xfe\xed\xfa\xcf",
    b"\xca\xfe\xba\xbe",
    b"\xbe\xba\xfe\xca",
    b"\xca\xfe\xba\xbf",
    b"\xbf\xba\xfe\xca",
    b"MZ",
)

# The qualification commands receive no ambient credentials, build controls,
# proxy settings, or user-specific search paths.  Absolute executable paths are
# mandatory, so only /usr/bin/env-style script launchers need this fixed PATH.
MINIMAL_ENVIRONMENT = {
    "LANG": "C",
    "LC_ALL": "C",
    "PATH": "/usr/bin:/bin",
    "TZ": "UTC",
}

# ``-I -c`` prevents the child from importing repository code before this
# standard-library-only bootstrap has authenticated the exact interpreter and
# local source closure.  It compiles the authenticated bytes directly instead
# of asking an import loader to reopen either pathname.
PROJECTOR_BOOTSTRAP = r"""
import hashlib
import os
import stat
import sys
import types

def die(message):
    print("KAGEMUSHA projector bootstrap failed: " + message, file=sys.stderr)
    raise SystemExit(1)

if not (sys.flags.isolated and sys.flags.dont_write_bytecode and sys.flags.no_site):
    die("projector interpreter lacks -I -B -S isolation")

def pinned_bytes(path, expected, executable=False):
    if not os.path.isabs(path) or os.path.abspath(path) != path:
        die("closure path is not absolute and normalized")
    if os.path.realpath(path) != path:
        die("closure path traverses a symlink")
    before = os.stat(path, follow_symlinks=False)
    if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1
            or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)):
        die("closure member is not a single-link owner-controlled file")
    if before.st_uid not in (0, os.geteuid()):
        die("closure member has an untrusted owner")
    if executable and not os.access(path, os.X_OK):
        die("closure interpreter is not executable")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            die("closure member changed before open")
        chunks = []
        digest = hashlib.sha256()
        total = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            if total > 512 * 1024 * 1024:
                die("closure member exceeds size limit")
            chunks.append(chunk)
            digest.update(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    named = os.stat(path, follow_symlinks=False)
    fields = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns", "st_mode", "st_nlink")
    if any(getattr(before, field) != getattr(after, field)
           or getattr(before, field) != getattr(named, field) for field in fields):
        die("closure member changed while read")
    if digest.hexdigest() != expected:
        die("closure member hash mismatch")
    return b"".join(chunks)

root, interpreter, interpreter_sha, verifier, verifier_sha, contract, contract_sha = sys.argv[1:8]
projector_argv = sys.argv[8:]
if os.path.realpath(sys.executable) != interpreter:
    die("running interpreter differs from the pinned interpreter")
pinned_bytes(interpreter, interpreter_sha, executable=True)
contract_source = pinned_bytes(contract, contract_sha)
verifier_source = pinned_bytes(verifier, verifier_sha)
if os.path.dirname(contract) != root or os.path.dirname(verifier) != root:
    die("local projector sources are outside the pinned source root")

contract_module = types.ModuleType("release_artifact_contract")
contract_module.__file__ = contract
contract_module.__package__ = ""
sys.modules[contract_module.__name__] = contract_module
exec(compile(contract_source, contract, "exec", dont_inherit=True), contract_module.__dict__)

verifier_globals = {
    "__name__": "__main__",
    "__file__": verifier,
    "__package__": "",
    "__builtins__": __builtins__,
}
sys.argv = [verifier, *projector_argv]
exit_error = None
try:
    exec(compile(verifier_source, verifier, "exec", dont_inherit=True), verifier_globals)
except SystemExit as error:
    exit_error = error

# The verifier itself performs its evidence-tree closing scan.  Repeat the
# immutable tool closure checks after it returns as the tooling counterpart.
pinned_bytes(interpreter, interpreter_sha, executable=True)
pinned_bytes(contract, contract_sha)
pinned_bytes(verifier, verifier_sha)
if exit_error is not None:
    raise exit_error
"""

# The first-release Python projector closure is deliberately small and exact:
# the resolved interpreter plus every repository-local source imported by the
# projector.  Candidate-selected Python or shebang entrypoints are not allowed.


class KagemushaRunnerError(RuntimeError):
    """Raised when collection or finalization cannot remain fail closed."""


def _fail(message: str) -> NoReturn:
    raise KagemushaRunnerError(message)


_BOOTSTRAP_BINDING: dict[str, object] | None = None


def _stdlib_file_binding(
    path: Path,
    expected_sha256: str,
    *,
    label: str,
    executable: bool,
    return_payload: bool = False,
) -> tuple[dict[str, object], bytes | None]:
    """Authenticate one bootstrap file without executing repository-local code."""

    if _HEX_64.fullmatch(expected_sha256) is None or expected_sha256 == "0" * 64:
        _fail(f"{label} SHA-256 must be canonical nonzero lowercase hex")
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail(f"{label} path must be absolute and normalized")
    try:
        if path.resolve(strict=True) != path:
            _fail(f"{label} path must not be a symlink")
        before = path.stat(follow_symlinks=False)
    except OSError as error:
        raise KagemushaRunnerError(f"cannot inspect {label}: {error}") from error
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
    ):
        _fail(f"{label} must be a single-link, owner-controlled regular file")
    if before.st_uid not in {0, os.geteuid()}:
        _fail(f"{label} must be owned by the current operator or root")
    if executable and not os.access(path, os.X_OK):
        _fail(f"{label} must be executable")
    descriptor = -1
    digest = hashlib.sha256()
    payload = bytearray() if return_payload else None
    total = 0
    try:
        descriptor = os.open(
            path,
            os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
        )
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            _fail(f"{label} changed before it was opened")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            if total > MAX_RUNTIME_EXECUTABLE_BYTES:
                _fail(f"{label} exceeds the bootstrap size limit")
            digest.update(chunk)
            if payload is not None:
                payload.extend(chunk)
        after = os.fstat(descriptor)
        named = path.stat(follow_symlinks=False)
    finally:
        if descriptor >= 0:
            os.close(descriptor)
    stable_fields = (
        "st_dev",
        "st_ino",
        "st_size",
        "st_mtime_ns",
        "st_ctime_ns",
        "st_mode",
        "st_nlink",
    )
    if any(
        getattr(before, field) != getattr(after, field)
        or getattr(before, field) != getattr(named, field)
        for field in stable_fields
    ):
        _fail(f"{label} changed while it was authenticated")
    actual_sha256 = digest.hexdigest()
    if actual_sha256 != expected_sha256:
        _fail(f"{label} differs from its explicit immutable SHA-256")
    return (
        {"path": str(path), "sha256": actual_sha256, "byte_len": total},
        bytes(payload) if payload is not None else None,
    )


def _bootstrap_local_modules(args: argparse.Namespace) -> dict[str, object]:
    """Pin the runtime/local-source closure before importing either local module."""

    global _BOOTSTRAP_BINDING
    global ReleaseArtifactError, StableFile
    global artifact_contract, release_verifier
    global canonical_json_bytes, canonical_relative_path, load_json_object
    global stable_hash_path, stable_hash_relative, stable_read_path

    python_binding, _ = _stdlib_file_binding(
        RESOLVED_PYTHON,
        args.python_executable_sha256,
        label="Python executable",
        executable=True,
    )
    verifier_binding, verifier_payload = _stdlib_file_binding(
        BUNDLED_RELEASE_VERIFIER,
        args.release_verifier_sha256,
        label="bundled KAGEMUSHA release verifier",
        executable=False,
        return_payload=True,
    )
    contract_binding, contract_payload = _stdlib_file_binding(
        BUNDLED_ARTIFACT_CONTRACT,
        args.artifact_contract_sha256,
        label="bundled release artifact contract",
        executable=False,
        return_payload=True,
    )
    assert verifier_payload is not None and contract_payload is not None
    binding = {
        "python_executable": python_binding,
        "release_verifier": verifier_binding,
        "artifact_contract": contract_binding,
    }
    if _BOOTSTRAP_BINDING is not None:
        if binding != _BOOTSTRAP_BINDING:
            _fail("loaded verifier source/runtime closure differs from the requested pins")
        return binding

    # Compile the bytes read through the authenticated descriptors.  Import
    # loaders would reopen a mutable pathname and reintroduce a hash-to-exec
    # substitution window.
    loaded_artifact_contract = types.ModuleType("release_artifact_contract")
    loaded_artifact_contract.__file__ = str(BUNDLED_ARTIFACT_CONTRACT)
    loaded_artifact_contract.__package__ = ""
    sys.modules[loaded_artifact_contract.__name__] = loaded_artifact_contract
    exec(
        compile(
            contract_payload,
            str(BUNDLED_ARTIFACT_CONTRACT),
            "exec",
            dont_inherit=True,
        ),
        loaded_artifact_contract.__dict__,
    )

    loaded_verifier = types.ModuleType("verify_kagemusha_v1_release_evidence")
    loaded_verifier.__file__ = str(BUNDLED_RELEASE_VERIFIER)
    loaded_verifier.__package__ = ""
    sys.modules[loaded_verifier.__name__] = loaded_verifier
    exec(
        compile(
            verifier_payload,
            str(BUNDLED_RELEASE_VERIFIER),
            "exec",
            dont_inherit=True,
        ),
        loaded_verifier.__dict__,
    )

    # A concurrent mutation cannot affect the code executed above, but it must
    # still invalidate this collection before any caller-controlled data is
    # accepted.
    closing_python, _ = _stdlib_file_binding(
        RESOLVED_PYTHON,
        args.python_executable_sha256,
        label="Python executable",
        executable=True,
    )
    closing_verifier, _ = _stdlib_file_binding(
        BUNDLED_RELEASE_VERIFIER,
        args.release_verifier_sha256,
        label="bundled KAGEMUSHA release verifier",
        executable=False,
    )
    closing_contract, _ = _stdlib_file_binding(
        BUNDLED_ARTIFACT_CONTRACT,
        args.artifact_contract_sha256,
        label="bundled release artifact contract",
        executable=False,
    )
    if binding != {
        "python_executable": closing_python,
        "release_verifier": closing_verifier,
        "artifact_contract": closing_contract,
    }:
        _fail("verifier source/runtime closure changed while it was loaded")

    artifact_contract = loaded_artifact_contract
    release_verifier = loaded_verifier
    ReleaseArtifactError = loaded_artifact_contract.ReleaseArtifactError
    StableFile = loaded_artifact_contract.StableFile
    canonical_json_bytes = loaded_artifact_contract.canonical_json_bytes
    canonical_relative_path = loaded_artifact_contract.canonical_relative_path
    load_json_object = loaded_artifact_contract.load_json_object
    stable_hash_path = loaded_artifact_contract.stable_hash_path
    stable_hash_relative = loaded_artifact_contract.stable_hash_relative
    stable_read_path = loaded_artifact_contract.stable_read_path
    _BOOTSTRAP_BINDING = binding
    return binding


def _object(
    value: object,
    label: str,
    fields: set[str] | frozenset[str],
) -> Mapping[str, object]:
    if not isinstance(value, Mapping) or set(value) != set(fields):
        _fail(f"{label} fields must be exactly {', '.join(sorted(fields))}")
    return value


def _array(value: object, label: str) -> list[object]:
    if not isinstance(value, list):
        _fail(f"{label} must be an array")
    return value


def _string(value: object, label: str) -> str:
    if not isinstance(value, str) or not value:
        _fail(f"{label} must be a non-empty string")
    return value


def _safe_id(value: object, label: str) -> str:
    text = _string(value, label)
    if _SAFE_ID.fullmatch(text) is None:
        _fail(f"{label} must be a bounded safe identifier")
    return text


def _digest(value: object, label: str, *, nonzero: bool = True) -> str:
    if not isinstance(value, str) or _HEX_64.fullmatch(value) is None:
        _fail(f"{label} must be exactly 64 lowercase hexadecimal characters")
    if nonzero and value == "0" * 64:
        _fail(f"{label} must not be zero")
    return value


def _positive_int(value: object, label: str, *, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        _fail(f"{label} must be a positive integer")
    if value > maximum:
        _fail(f"{label} exceeds {maximum}")
    return value


def _absolute_path(value: object, label: str, *, must_exist: bool) -> Path:
    raw = _string(value, label)
    path = Path(raw)
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail(f"{label} must be absolute and normalized")
    parent = path.parent
    try:
        resolved_parent = parent.resolve(strict=True)
    except OSError as error:
        raise KagemushaRunnerError(f"{label} parent cannot be resolved: {error}") from error
    if resolved_parent != parent:
        _fail(f"{label} must not traverse symlinked directories")
    if must_exist:
        try:
            resolved = path.resolve(strict=True)
            info = path.lstat()
        except OSError as error:
            raise KagemushaRunnerError(f"{label} cannot be resolved: {error}") from error
        if resolved != path or stat.S_ISLNK(info.st_mode):
            _fail(f"{label} must not be a symlink or traverse symlinks")
    return path


def _relative_path(value: object, label: str, *, argv_safe: bool = False) -> str:
    try:
        path = canonical_relative_path(_string(value, label))
    except ReleaseArtifactError as error:
        raise KagemushaRunnerError(str(error)) from error
    if argv_safe and path.startswith("-"):
        _fail(f"{label} must not begin with '-' when passed as an argument")
    return path


def _load_pinned_json(
    path: Path,
    expected_sha256: str,
    *,
    label: str,
    max_size: int,
) -> tuple[StableFile, dict[str, object]]:
    normalized = _absolute_path(str(path), label, must_exist=True)
    expected = _digest(expected_sha256, f"{label} SHA-256")
    info, payload = stable_read_path(normalized, max_size=max_size)
    if info.sha256 != expected:
        _fail(f"{label} differs from its explicit immutable SHA-256")
    value = load_json_object(payload, label)
    try:
        canonical = canonical_json_bytes(value)
    except (TypeError, ValueError) as error:
        raise KagemushaRunnerError(f"{label} is not canonical JSON") from error
    if canonical != payload:
        _fail(f"{label} is not canonical JSON")
    return info, value


def _ensure_private_directory(path: Path, label: str) -> None:
    try:
        info = path.stat(follow_symlinks=False)
    except OSError as error:
        raise KagemushaRunnerError(f"cannot inspect {label}: {error}") from error
    if not stat.S_ISDIR(info.st_mode) or stat.S_ISLNK(info.st_mode):
        _fail(f"{label} must be a real directory")
    if info.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
        _fail(f"{label} must not be group- or world-writable")


def _prepare_new_output(path: Path, *, dry_run: bool) -> None:
    normalized = _absolute_path(str(path), "output directory", must_exist=False)
    _ensure_private_directory(normalized.parent, "output parent")
    if normalized.exists() or normalized.is_symlink():
        _fail("output directory must not already exist")
    if not dry_run:
        try:
            normalized.mkdir(mode=0o700)
        except FileExistsError as error:
            raise KagemushaRunnerError("output directory was created concurrently") from error


def _open_existing_output(path: Path) -> Path:
    normalized = _absolute_path(str(path), "output directory", must_exist=True)
    _ensure_private_directory(normalized, "output directory")
    return normalized


def _mkdir_private(path: Path) -> None:
    if path.exists():
        _ensure_private_directory(path, f"directory {path}")
        return
    parent = path.parent
    if parent != path:
        _mkdir_private(parent)
    try:
        path.mkdir(mode=0o700)
    except FileExistsError:
        _ensure_private_directory(path, f"directory {path}")


def _destination(root: Path, relative: str) -> Path:
    normalized = _relative_path(relative, "evidence destination")
    path = root.joinpath(*normalized.split("/"))
    _mkdir_private(path.parent)
    if path.parent.resolve(strict=True) != path.parent:
        _fail("evidence destination parent traverses a symlink")
    return path


def _atomic_write(path: Path, payload: bytes, *, mode: int = 0o600) -> None:
    _mkdir_private(path.parent)
    if path.exists() or path.is_symlink():
        _fail(f"refusing to overwrite {path}")
    descriptor = -1
    temporary = path.parent / f".{path.name}.tmp-{os.getpid()}-{time.time_ns()}"
    try:
        descriptor = os.open(
            temporary,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            mode,
        )
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                _fail(f"failed to write {path}")
            view = view[written:]
        os.fsync(descriptor)
        os.close(descriptor)
        descriptor = -1
        os.replace(temporary, path)
        directory_fd = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        try:
            temporary.unlink(missing_ok=True)
        except OSError:
            pass


def _stable_file_record(path: str, kind: str, info: StableFile) -> dict[str, object]:
    return {
        "path": path,
        "kind": kind,
        "sha256": info.sha256,
        "byte_len": info.size,
        "mode": info.mode,
        "device": info.device,
        "inode": info.inode,
        "mtime_ns": info.mtime_ns,
        "ctime_ns": info.ctime_ns,
        "link_count": info.link_count,
    }


def _record_identity(record: Mapping[str, object]) -> tuple[object, ...]:
    return tuple(
        record[field]
        for field in (
            "sha256",
            "byte_len",
            "mode",
            "device",
            "inode",
            "mtime_ns",
            "ctime_ns",
            "link_count",
        )
    )


def _capture_evidence_file(root: Path, path: str, kind: str) -> dict[str, object]:
    try:
        maximum = release_verifier._max_for_kind(kind)
        info = stable_hash_relative(root, path, max_size=maximum)
    except (KeyError, ReleaseArtifactError) as error:
        raise KagemushaRunnerError(f"invalid evidence file {path!r}: {error}") from error
    if info.size < 1:
        _fail(f"evidence file {path!r} must not be empty")
    return _stable_file_record(path, kind, info)


def _assert_record(root: Path, record: Mapping[str, object]) -> None:
    path = _relative_path(record["path"], "collected file path")
    kind = _string(record["kind"], f"collected file {path} kind")
    current = _capture_evidence_file(root, path, kind)
    if _record_identity(current) != _record_identity(record):
        _fail(f"collected evidence file {path!r} changed")


def _assert_record_matches_info(
    record: Mapping[str, object], info: StableFile, *, label: str
) -> None:
    expected = (
        info.sha256,
        info.size,
        info.mode,
        info.device,
        info.inode,
        info.mtime_ns,
        info.ctime_ns,
        info.link_count,
    )
    if _record_identity(record) != expected:
        _fail(f"{label} differs from the exact output descriptor")


def _scan_exact(root: Path, expected_paths: set[str]) -> None:
    try:
        actual = release_verifier._scan_evidence_tree(root)
    except (ReleaseArtifactError, release_verifier.KagemushaEvidenceError) as error:
        raise KagemushaRunnerError(str(error)) from error
    if actual != sorted(expected_paths):
        missing = sorted(expected_paths - set(actual))
        extra = sorted(set(actual) - expected_paths)
        _fail(f"evidence tree differs from its declaration; missing={missing!r}, extra={extra!r}")


def _copy_seed(source: Path, destination: Path, *, expected: StableFile, maximum: int) -> None:
    source_fd = destination_fd = -1
    source_before: os.stat_result | None = None
    digest = hashlib.sha256()
    total = 0
    try:
        source_fd = os.open(
            source,
            os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
        )
        source_before = os.fstat(source_fd)
        if not stat.S_ISREG(source_before.st_mode) or source_before.st_nlink != 1:
            _fail("seed source must be a single-link regular file")
        if source_before.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
            _fail("seed source must not be group- or world-writable")
        destination_fd = os.open(
            destination,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o600,
        )
        while True:
            chunk = os.read(source_fd, 1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            if total > maximum:
                _fail("seed source exceeds its declared evidence limit")
            digest.update(chunk)
            view = memoryview(chunk)
            while view:
                written = os.write(destination_fd, view)
                if written <= 0:
                    _fail("failed to copy seed source")
                view = view[written:]
        os.fsync(destination_fd)
        source_after = os.fstat(source_fd)
        named = source.stat(follow_symlinks=False)
        stable_fields = (
            "st_dev",
            "st_ino",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
            "st_mode",
            "st_nlink",
        )
        if any(
            getattr(source_before, field) != getattr(source_after, field)
            or getattr(source_before, field) != getattr(named, field)
            for field in stable_fields
        ):
            _fail("seed source changed while it was copied")
        if total != expected.size or digest.hexdigest() != expected.sha256:
            _fail("seed source bytes differ from their explicit immutable identity")
    finally:
        if source_fd >= 0:
            os.close(source_fd)
        if destination_fd >= 0:
            os.close(destination_fd)


@dataclass(frozen=True)
class Executable:
    """One exact command entrypoint."""

    path: Path
    sha256: str


@dataclass(frozen=True)
class SeedFile:
    """One immutable file copied into the evidence root."""

    source: Path
    evidence_path: str
    kind: str
    sha256: str
    byte_len: int


@dataclass(frozen=True)
class ProducedFile:
    """One exact output expected from a producer."""

    evidence_path: str
    kind: str
    max_bytes: int


@dataclass(frozen=True)
class ProducerStep:
    """One candidate-production step whose output receives no authority itself."""

    step_id: str
    executable: Executable
    arguments: tuple[tuple[str, str], ...]
    outputs: tuple[ProducedFile, ...]
    timeout_ms: int


@dataclass(frozen=True)
class VerificationStep:
    """One trusted report-verification command observed for approval."""

    step_id: str
    verifier_id: str
    executable: Executable
    report_schema: str
    report: str
    arguments: tuple[tuple[str, str], ...]
    stdout: str
    stderr: str
    observation: str
    timeout_ms: int


@dataclass(frozen=True)
class CollectionPlan:
    """Validated canonical runner plan."""

    manifest_template: Mapping[str, object]
    seed_files: tuple[SeedFile, ...]
    producer_steps: tuple[ProducerStep, ...]
    verification_steps: tuple[VerificationStep, ...]
    expected_reports: Mapping[str, str]


def _parse_executable(value: object, label: str) -> Executable:
    row = _object(value, label, {"path", "sha256"})
    path = _absolute_path(row["path"], f"{label} path", must_exist=True)
    digest = _digest(row["sha256"], f"{label} SHA-256")
    _validate_executable(Executable(path, digest), label)
    return Executable(path, digest)


def _validate_executable(executable: Executable, label: str) -> StableFile:
    path = _absolute_path(str(executable.path), f"{label} path", must_exist=True)
    try:
        info = stable_hash_path(path, max_size=MAX_RUNTIME_EXECUTABLE_BYTES)
    except ReleaseArtifactError as error:
        raise KagemushaRunnerError(str(error)) from error
    if info.sha256 != executable.sha256:
        _fail(f"{label} executable differs from its pinned SHA-256")
    path_info = path.stat(follow_symlinks=False)
    if (
        not stat.S_ISREG(path_info.st_mode)
        or path_info.st_nlink != 1
        or path_info.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        or not os.access(path, os.X_OK)
    ):
        _fail(
            f"{label} must be a single-link executable that is not group- or "
            "world-writable"
        )
    descriptor = -1
    try:
        descriptor = os.open(
            path,
            os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
        )
        magic = os.read(descriptor, 4)
    finally:
        if descriptor >= 0:
            os.close(descriptor)
    if magic.startswith(b"#!") or not any(magic.startswith(prefix) for prefix in _NATIVE_MAGICS):
        _fail(
            f"{label} must be a native executable; scripts and unpinned "
            "runtime/source closures are rejected"
        )
    if path_info.st_uid != 0:
        _fail(f"{label} native executable must be administrator-owned")
    if os.geteuid() == 0:
        _fail("release-evidence commands must run from an unprivileged operator account")
    for parent in path.parents:
        parent_info = parent.stat(follow_symlinks=False)
        if (
            not stat.S_ISDIR(parent_info.st_mode)
            or parent_info.st_uid != 0
            or parent_info.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        ):
            _fail(
                f"{label} path must be rooted exclusively in administrator-owned "
                "non-publicly-writable directories"
            )
    return info


def _manifest_report_matrix(template: object) -> dict[str, str]:
    manifest = _object(
        template,
        "manifest template",
        {
            "schema",
            "schema_version",
            "source",
            "artifacts",
            "protocols",
            "global_reports",
            "profiles",
            "reproducible_builds",
        },
    )
    if (
        manifest["schema"] != release_verifier.MANIFEST_SCHEMA
        or manifest["schema_version"] != SCHEMA_VERSION
    ):
        _fail("manifest template has the wrong schema or version")
    source = _object(manifest["source"], "manifest source", {"source_archive", "cargo_lock"})
    _relative_path(source["source_archive"], "source archive")
    _relative_path(source["cargo_lock"], "Cargo.lock")

    artifacts = _array(manifest["artifacts"], "manifest artifacts")
    if len(artifacts) != len(release_verifier.ARTIFACT_ROLES):
        _fail("manifest template does not contain the complete artifact-role matrix")
    for index, (raw, role) in enumerate(zip(artifacts, release_verifier.ARTIFACT_ROLES)):
        row = _object(raw, f"artifact {index}", {"role", "path"})
        if row["role"] != role:
            _fail("manifest artifact roles are not in canonical order")
        _relative_path(row["path"], f"artifact {role} path")
    if not isinstance(manifest["protocols"], Mapping):
        _fail("manifest protocols must be an object")

    reports: dict[str, str] = {}

    def add_report(raw_path: object, schema: str, label: str) -> None:
        path = _relative_path(raw_path, label, argv_safe=True)
        if path in reports:
            _fail(f"report path {path!r} is reused by multiple matrix cells")
        reports[path] = schema

    global_reports = _object(
        manifest["global_reports"],
        "global reports",
        {"circuit_shape", "security_review", "kat", "fuzz", "resource"},
    )
    global_schemas = {
        "circuit_shape": "iroha.kagemusha_v1.circuit_shape_report",
        "security_review": "iroha.kagemusha_v1.security_review_report",
        "kat": "iroha.kagemusha_v1.kat_report",
        "fuzz": "iroha.kagemusha_v1.fuzz_report",
        "resource": "iroha.kagemusha_v1.resource_report",
    }
    for name in sorted(global_schemas):
        add_report(global_reports[name], global_schemas[name], f"global {name} report")

    profiles = _array(manifest["profiles"], "profile qualifications")
    if not profiles or len(profiles) > 64:
        _fail("manifest template must contain 1 through 64 hardware profiles")
    profile_ids: list[str] = []
    for profile_index, raw_profile in enumerate(profiles):
        profile = _object(
            raw_profile,
            f"profile {profile_index}",
            {
                "hardware_profile",
                "suite_id",
                "qualification_report",
                "relations",
                "helpers",
                "receive_fold_occupancies",
                "recursive_depths",
                "aggregate_balance",
                "thermal",
                "envelope",
                "acceptance_cases",
            },
        )
        hardware = _object(
            profile["hardware_profile"],
            f"profile {profile_index} hardware profile",
            release_verifier._HARDWARE_PROFILE_FIELDS,
        )
        profile_id = _digest(
            hardware["hardware_profile_id"], f"profile {profile_index} hardware profile id"
        )
        _digest(profile["suite_id"], f"profile {profile_index} suite id")
        profile_ids.append(profile_id)
        add_report(
            profile["qualification_report"],
            "iroha.kagemusha_v1.hardware_profile_qualification_report",
            f"profile {profile_id} qualification report",
        )

        relations = _array(profile["relations"], f"profile {profile_id} relations")
        if len(relations) != len(release_verifier.RELATIONS):
            _fail("profile relation matrix is incomplete")
        for index, (raw, expected) in enumerate(zip(relations, release_verifier.RELATIONS)):
            row = _object(raw, f"relation {index}", {"relation", "report"})
            if row["relation"] != expected:
                _fail("profile relation matrix is not in canonical order")
            add_report(
                row["report"],
                "iroha.kagemusha_v1.relation_qualification_report",
                f"{expected} relation report",
            )

        helpers = _array(profile["helpers"], f"profile {profile_id} helpers")
        if len(helpers) != len(release_verifier.HELPERS):
            _fail("profile helper matrix is incomplete")
        for index, (raw, expected) in enumerate(zip(helpers, release_verifier.HELPERS)):
            row = _object(raw, f"helper {index}", {"helper", "report"})
            if row["helper"] != expected:
                _fail("profile helper matrix is not in canonical order")
            add_report(
                row["report"],
                "iroha.kagemusha_v1.helper_qualification_report",
                f"{expected} helper report",
            )

        # First-release qualification always exercises every fixed ReceiveFold
        # slot.  Keep this requirement local and explicit even if a concurrently
        # edited verifier source temporarily lacks its matching constant.
        occupancy_width = getattr(release_verifier, "RECEIVE_FOLD_BATCH_WIDTH", 16)
        if occupancy_width != 16:
            _fail("KAGEMUSHA V1 ReceiveFold occupancy width must remain exactly 16")
        occupancies = _array(
            profile["receive_fold_occupancies"], f"profile {profile_id} occupancies"
        )
        if len(occupancies) != occupancy_width:
            _fail("receive-fold occupancy matrix must contain exactly 1 through 16")
        for expected_occupancy, raw in enumerate(occupancies, start=1):
            occupancy = _object(
                raw,
                f"receive-fold occupancy {expected_occupancy}",
                {"occupancy", "report"},
            )
            if occupancy["occupancy"] != expected_occupancy:
                _fail("receive-fold occupancies must be exactly 1 through 16")
            add_report(
                occupancy["report"],
                "iroha.kagemusha_v1.receive_fold_occupancy_report",
                f"receive-fold occupancy {expected_occupancy} report",
            )

        depths = _array(profile["recursive_depths"], f"profile {profile_id} depths")
        if len(depths) != 4:
            _fail("recursive-depth matrix must contain exactly four rows")
        depth_values: list[int] = []
        for index, raw in enumerate(depths):
            row = _object(raw, f"recursive depth {index}", {"depth", "report"})
            depth = _positive_int(row["depth"], "recursive depth", maximum=(1 << 63) - 1)
            depth_values.append(depth)
            add_report(
                row["report"],
                "iroha.kagemusha_v1.recursive_depth_report",
                f"recursive depth {depth} report",
            )
        if depth_values[:3] != [8, 64, 1024] or depth_values[3] <= 1024:
            _fail("recursive depths must be exactly 8, 64, 1024, and one greater depth")

        for field, schema in (
            ("aggregate_balance", "iroha.kagemusha_v1.aggregate_balance_report"),
            ("thermal", "iroha.kagemusha_v1.thermal_report"),
            ("envelope", "iroha.kagemusha_v1.envelope_report"),
        ):
            add_report(profile[field], schema, f"profile {profile_id} {field} report")

        cases = _array(profile["acceptance_cases"], f"profile {profile_id} acceptance cases")
        if len(cases) != len(release_verifier.ACCEPTANCE_CASES):
            _fail("profile acceptance-case matrix is incomplete")
        for index, (raw, expected) in enumerate(zip(cases, release_verifier.ACCEPTANCE_CASES)):
            row = _object(raw, f"acceptance case {index}", {"case", "report"})
            if row["case"] != expected:
                _fail("profile acceptance cases are not in canonical order")
            add_report(
                row["report"],
                "iroha.kagemusha_v1.acceptance_case_report",
                f"{expected} acceptance report",
            )
    if profile_ids != sorted(set(profile_ids)):
        _fail("hardware profiles must be uniquely sorted by id")

    builds = _array(manifest["reproducible_builds"], "reproducible builds")
    if len(builds) < 2 or len(builds) > 8:
        _fail("manifest template must contain 2 through 8 reproducible builds")
    builder_ids: list[str] = []
    for index, raw in enumerate(builds):
        row = _object(raw, f"reproducible build {index}", {"builder_id", "report"})
        builder_ids.append(_digest(row["builder_id"], f"builder {index} id"))
        add_report(
            row["report"],
            "iroha.kagemusha_v1.reproducible_build_report",
            f"builder {index} report",
        )
    if builder_ids != sorted(set(builder_ids)):
        _fail("reproducible builds must be uniquely sorted by builder id")
    return reports


def _parse_plan(
    value: Mapping[str, object],
    policy: release_verifier.TrustedObserverPolicy,
) -> CollectionPlan:
    row = _object(
        value,
        "collection plan",
        {
            "schema",
            "schema_version",
            "manifest_template",
            "seed_files",
            "producer_steps",
            "verification_steps",
        },
    )
    if row["schema"] != PLAN_SCHEMA or row["schema_version"] != SCHEMA_VERSION:
        _fail("collection plan has the wrong schema or version")
    expected_reports = _manifest_report_matrix(row["manifest_template"])

    seeds: list[SeedFile] = []
    declared: dict[str, str] = {}
    for index, raw in enumerate(_array(row["seed_files"], "seed files")):
        seed = _object(
            raw,
            f"seed file {index}",
            {"source", "evidence_path", "kind", "sha256", "byte_len"},
        )
        source = _absolute_path(seed["source"], f"seed file {index} source", must_exist=True)
        evidence_path = _relative_path(seed["evidence_path"], f"seed file {index} path")
        kind = _string(seed["kind"], f"seed file {index} kind")
        if kind not in release_verifier.FILE_KINDS or kind in {"observation", "transcript"}:
            _fail(f"seed file {evidence_path!r} has a runner-reserved or unknown kind")
        maximum = release_verifier._max_for_kind(kind)
        byte_len = _positive_int(seed["byte_len"], f"seed file {index} length", maximum=maximum)
        sha256 = _digest(seed["sha256"], f"seed file {index} SHA-256")
        info = stable_hash_path(source, max_size=maximum)
        if info.sha256 != sha256 or info.size != byte_len:
            _fail(f"seed file {evidence_path!r} differs from its immutable identity")
        if evidence_path in declared:
            _fail(f"evidence path {evidence_path!r} is declared more than once")
        declared[evidence_path] = kind
        seeds.append(SeedFile(source, evidence_path, kind, sha256, byte_len))
    if [seed.evidence_path for seed in seeds] != sorted(seed.evidence_path for seed in seeds):
        _fail("seed files must be sorted by evidence path")

    producers: list[ProducerStep] = []
    producer_ids: list[str] = []
    available = set(declared)
    for index, raw in enumerate(_array(row["producer_steps"], "producer steps")):
        step = _object(
            raw,
            f"producer step {index}",
            {"id", "executable", "arguments", "outputs", "timeout_ms"},
        )
        step_id = _safe_id(step["id"], f"producer step {index} id")
        executable = _parse_executable(step["executable"], f"producer step {step_id}")
        outputs: list[ProducedFile] = []
        output_paths: set[str] = set()
        for output_index, raw_output in enumerate(
            _array(step["outputs"], f"producer {step_id} outputs")
        ):
            output = _object(
                raw_output,
                f"producer {step_id} output {output_index}",
                {"evidence_path", "kind", "max_bytes"},
            )
            path = _relative_path(output["evidence_path"], f"producer {step_id} output path")
            kind = _string(output["kind"], f"producer {step_id} output kind")
            if kind not in release_verifier.FILE_KINDS or kind in {"observation", "transcript"}:
                _fail(f"producer output {path!r} has a runner-reserved or unknown kind")
            maximum = _positive_int(
                output["max_bytes"],
                f"producer {step_id} output limit",
                maximum=release_verifier._max_for_kind(kind),
            )
            if path in declared or path in output_paths:
                _fail(f"evidence path {path!r} is declared more than once")
            output_paths.add(path)
            outputs.append(ProducedFile(path, kind, maximum))
        if not outputs:
            _fail(f"producer step {step_id!r} must declare at least one output")
        if [output.evidence_path for output in outputs] != sorted(
            output.evidence_path for output in outputs
        ):
            _fail(f"producer step {step_id!r} outputs must be sorted by path")

        arguments: list[tuple[str, str]] = []
        referenced_outputs: set[str] = set()
        raw_arguments = _array(step["arguments"], f"producer {step_id} arguments")
        if not raw_arguments or len(raw_arguments) > 256:
            _fail(f"producer step {step_id!r} must have 1 through 256 arguments")
        for argument_index, raw_argument in enumerate(raw_arguments):
            if not isinstance(raw_argument, Mapping) or len(raw_argument) != 1:
                _fail(f"producer {step_id} argument {argument_index} must have exactly one field")
            if "literal" in raw_argument:
                literal = _string(raw_argument["literal"], f"producer {step_id} literal")
                if _SAFE_PRODUCER_LITERAL.fullmatch(literal) is None:
                    _fail(f"producer {step_id} contains an unsafe literal")
                arguments.append(("literal", literal))
            elif "input" in raw_argument:
                path = _relative_path(raw_argument["input"], f"producer {step_id} input")
                if path not in available:
                    _fail(f"producer {step_id} references an unavailable input {path!r}")
                arguments.append(("input", path))
            elif "output" in raw_argument:
                path = _relative_path(raw_argument["output"], f"producer {step_id} output")
                if path not in output_paths:
                    _fail(f"producer {step_id} references an undeclared output {path!r}")
                referenced_outputs.add(path)
                arguments.append(("output", path))
            else:
                _fail(f"producer {step_id} argument must be literal, input, or output")
        if referenced_outputs != output_paths:
            _fail(f"producer {step_id} does not receive every declared output path")
        timeout_ms = _positive_int(
            step["timeout_ms"], f"producer {step_id} timeout", maximum=MAX_STEP_TIMEOUT_MS
        )
        producers.append(
            ProducerStep(step_id, executable, tuple(arguments), tuple(outputs), timeout_ms)
        )
        producer_ids.append(step_id)
        for output in outputs:
            declared[output.evidence_path] = output.kind
            available.add(output.evidence_path)
    if producer_ids != sorted(set(producer_ids)):
        _fail("producer steps must have uniquely sorted ids")

    verifications: list[VerificationStep] = []
    verification_ids: list[str] = []
    verification_reports: dict[str, str] = {}
    reserved_paths = set(declared)
    all_verification_files: set[str] = set()
    for index, raw in enumerate(_array(row["verification_steps"], "verification steps")):
        step = _object(
            raw,
            f"verification step {index}",
            {
                "id",
                "verifier_id",
                "executable",
                "report_schema",
                "report",
                "arguments",
                "stdout",
                "stderr",
                "observation",
                "timeout_ms",
            },
        )
        step_id = _safe_id(step["id"], f"verification step {index} id")
        verifier_id = _safe_id(step["verifier_id"], f"verification step {step_id} verifier id")
        trusted = policy.verifiers.get(verifier_id)
        if trusted is None:
            _fail(f"verification step {step_id!r} names an untrusted verifier")
        executable = _parse_executable(step["executable"], f"verification step {step_id}")
        if executable.sha256 != trusted.sha256:
            _fail(f"verification step {step_id!r} executable hash differs from policy")
        report_schema = _string(step["report_schema"], f"verification step {step_id} schema")
        if report_schema not in trusted.report_schemas:
            _fail(f"verification step {step_id!r} schema is not admitted by policy")
        report = _relative_path(
            step["report"], f"verification step {step_id} report", argv_safe=True
        )
        if report not in expected_reports or expected_reports[report] != report_schema:
            _fail(f"verification step {step_id!r} does not match the manifest report matrix")
        if report in verification_reports:
            _fail(f"manifest report {report!r} has more than one verification step")
        verification_reports[report] = step_id

        arguments: list[tuple[str, str]] = []
        file_arguments: set[str] = set()
        raw_arguments = _array(step["arguments"], f"verification {step_id} arguments")
        if not raw_arguments or len(raw_arguments) > 64:
            _fail(f"verification step {step_id!r} must have 1 through 64 arguments")
        for argument_index, raw_argument in enumerate(raw_arguments):
            if not isinstance(raw_argument, Mapping) or len(raw_argument) != 1:
                _fail(f"verification {step_id} argument {argument_index} must have one field")
            if "literal" in raw_argument:
                literal = _string(raw_argument["literal"], f"verification {step_id} literal")
                if release_verifier._SAFE_LITERAL.fullmatch(literal) is None:
                    _fail(f"verification {step_id} contains an unsafe literal")
                arguments.append(("literal", literal))
            elif "file" in raw_argument:
                path = _relative_path(
                    raw_argument["file"], f"verification {step_id} file", argv_safe=True
                )
                if path not in declared:
                    _fail(f"verification {step_id} references undeclared file {path!r}")
                if path in file_arguments:
                    _fail(f"verification {step_id} repeats file argument {path!r}")
                file_arguments.add(path)
                all_verification_files.add(path)
                arguments.append(("file", path))
            else:
                _fail(f"verification {step_id} argument must be literal or file")
        if report not in file_arguments:
            _fail(f"verification step {step_id!r} does not receive its report")

        stdout = _relative_path(step["stdout"], f"verification {step_id} stdout")
        stderr = _relative_path(step["stderr"], f"verification {step_id} stderr")
        observation = _relative_path(
            step["observation"], f"verification {step_id} observation"
        )
        for path in (stdout, stderr, observation):
            if path in reserved_paths:
                _fail(f"runner-owned path {path!r} is reused")
            reserved_paths.add(path)
        if len({stdout, stderr, observation}) != 3:
            _fail(f"verification step {step_id!r} stream paths must be distinct")
        timeout_ms = _positive_int(
            step["timeout_ms"], f"verification {step_id} timeout", maximum=MAX_STEP_TIMEOUT_MS
        )
        verifications.append(
            VerificationStep(
                step_id,
                verifier_id,
                executable,
                report_schema,
                report,
                tuple(arguments),
                stdout,
                stderr,
                observation,
                timeout_ms,
            )
        )
        verification_ids.append(step_id)
    if verification_ids != sorted(set(verification_ids)):
        _fail("verification steps must have uniquely sorted ids")
    if set(producer_ids) & set(verification_ids):
        _fail("producer and verification step ids must be disjoint")
    if set(verification_reports) != set(expected_reports):
        missing = sorted(set(expected_reports) - set(verification_reports))
        extra = sorted(set(verification_reports) - set(expected_reports))
        _fail(
            "verification matrix is not exhaustive; "
            f"missing={missing!r}, extra={extra!r}"
        )
    if set(declared) != all_verification_files:
        missing = sorted(set(declared) - all_verification_files)
        extra = sorted(all_verification_files - set(declared))
        _fail(
            "evidence files are not exactly consumed by verifiers; "
            f"missing={missing!r}, extra={extra!r}"
        )
    if len(verifications) > release_verifier.MAX_COMMANDS:
        _fail("verification plan exceeds the V1 command limit")

    return CollectionPlan(
        manifest_template=copy.deepcopy(dict(row["manifest_template"])),
        seed_files=tuple(seeds),
        producer_steps=tuple(producers),
        verification_steps=tuple(verifications),
        expected_reports=expected_reports,
    )


def _toolchain_binding(path: Path, expected_sha256: str, label: str) -> dict[str, object]:
    expected = _digest(expected_sha256, f"{label} SHA-256")
    normalized = _absolute_path(str(path), label, must_exist=True)
    info = stable_hash_path(normalized, max_size=MAX_RUNTIME_EXECUTABLE_BYTES)
    if info.sha256 != expected:
        _fail(f"{label} differs from its explicit immutable SHA-256")
    return {"path": str(normalized), "sha256": info.sha256, "byte_len": info.size}


def _toolchain_closure(args: argparse.Namespace) -> dict[str, object]:
    return {
        "python_executable": _toolchain_binding(
            RESOLVED_PYTHON, args.python_executable_sha256, "Python executable"
        ),
        "release_verifier": _toolchain_binding(
            BUNDLED_RELEASE_VERIFIER,
            args.release_verifier_sha256,
            "bundled KAGEMUSHA release verifier",
        ),
        "artifact_contract": _toolchain_binding(
            BUNDLED_ARTIFACT_CONTRACT,
            args.artifact_contract_sha256,
            "bundled release artifact contract",
        ),
    }


@dataclass(frozen=True)
class ProcessResult:
    """Exit and measured resource results for one child."""

    exit_code: int
    started_at_ms: int
    duration_ms: int
    cpu_millis: int
    peak_rss_bytes: int
    stdout: StableFile
    stderr: StableFile


def _terminate_process_group(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    deadline = time.monotonic() + TERMINATION_GRACE_SECONDS
    while time.monotonic() < deadline:
        waited, status, usage = os.wait4(process.pid, os.WNOHANG)
        if waited:
            process.returncode = os.waitstatus_to_exitcode(status)
            process._kagemusha_usage = usage  # type: ignore[attr-defined]
            return
        time.sleep(MAX_PROCESS_POLL_SECONDS)
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def _quiesce_process_group(process_group_id: int) -> None:
    """Terminate descendants left in a completed command's process group."""

    try:
        os.killpg(process_group_id, signal.SIGTERM)
    except ProcessLookupError:
        return
    deadline = time.monotonic() + TERMINATION_GRACE_SECONDS
    while time.monotonic() < deadline:
        try:
            os.killpg(process_group_id, 0)
        except ProcessLookupError:
            return
        time.sleep(MAX_PROCESS_POLL_SECONDS)
    try:
        os.killpg(process_group_id, signal.SIGKILL)
    except ProcessLookupError:
        return
    deadline = time.monotonic() + TERMINATION_GRACE_SECONDS
    while time.monotonic() < deadline:
        try:
            os.killpg(process_group_id, 0)
        except ProcessLookupError:
            return
        time.sleep(MAX_PROCESS_POLL_SECONDS)
    _fail("command left a surviving process-group descendant")


def _stable_transcript_from_fd(
    descriptor: int,
    path: Path,
    *,
    transcript_limit: int,
    require_nonempty: bool,
) -> StableFile:
    """Hash the exact parent-owned output inode and match it to its pathname."""

    before = os.fstat(descriptor)
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or stat.S_IMODE(before.st_mode) != 0o600
        or before.st_size > transcript_limit
        or (require_nonempty and before.st_size < 1)
    ):
        _fail("command transcript inode has an invalid identity or size")
    os.lseek(descriptor, 0, os.SEEK_SET)
    digest = hashlib.sha256()
    total = 0
    while True:
        chunk = os.read(descriptor, 1024 * 1024)
        if not chunk:
            break
        total += len(chunk)
        if total > transcript_limit:
            _fail("command transcript exceeded its byte limit")
        digest.update(chunk)
    after = os.fstat(descriptor)
    try:
        named = path.stat(follow_symlinks=False)
    except OSError as error:
        raise KagemushaRunnerError("command removed its transcript path") from error
    stable_fields = (
        "st_dev",
        "st_ino",
        "st_size",
        "st_mtime_ns",
        "st_ctime_ns",
        "st_mode",
        "st_nlink",
    )
    if any(
        getattr(before, field) != getattr(after, field)
        or getattr(before, field) != getattr(named, field)
        for field in stable_fields
    ):
        _fail("command replaced or mutated its transcript while it was captured")
    if total != before.st_size:
        _fail("command transcript size changed while it was captured")
    return StableFile(
        sha256=digest.hexdigest(),
        size=total,
        mode=stat.S_IMODE(before.st_mode),
        device=before.st_dev,
        inode=before.st_ino,
        mtime_ns=before.st_mtime_ns,
        ctime_ns=before.st_ctime_ns,
        link_count=before.st_nlink,
    )


def _run_process(
    executable: Path,
    arguments: Sequence[str],
    *,
    cwd: Path,
    stdout_path: Path,
    stderr_path: Path,
    timeout_ms: int,
    transcript_limit: int,
    require_nonempty_streams: bool,
) -> ProcessResult:
    _mkdir_private(stdout_path.parent)
    _mkdir_private(stderr_path.parent)
    if stdout_path.exists() or stderr_path.exists():
        _fail("command transcript path already exists")
    stdout_fd = stderr_fd = -1
    process: subprocess.Popen[bytes] | None = None
    started_at_ms = time.time_ns() // 1_000_000
    started = time.monotonic()
    usage: resource.struct_rusage | None = None
    exceeded = False
    timed_out = False
    try:
        stdout_fd = os.open(
            stdout_path,
            os.O_RDWR
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o600,
        )
        stderr_fd = os.open(
            stderr_path,
            os.O_RDWR
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o600,
        )
        process = subprocess.Popen(
            [str(executable), *arguments],
            cwd=cwd,
            env=MINIMAL_ENVIRONMENT,
            stdin=subprocess.DEVNULL,
            stdout=stdout_fd,
            stderr=stderr_fd,
            shell=False,
            close_fds=True,
            start_new_session=True,
        )
        status = 0
        while True:
            waited, status, candidate_usage = os.wait4(process.pid, os.WNOHANG)
            if waited:
                usage = candidate_usage
                break
            if (
                os.fstat(stdout_fd).st_size > transcript_limit
                or os.fstat(stderr_fd).st_size > transcript_limit
            ):
                exceeded = True
                _terminate_process_group(process)
                if hasattr(process, "_kagemusha_usage"):
                    usage = process._kagemusha_usage  # type: ignore[attr-defined]
                    status = 0
                    break
            if (time.monotonic() - started) * 1_000 >= timeout_ms:
                timed_out = True
                _terminate_process_group(process)
                if hasattr(process, "_kagemusha_usage"):
                    usage = process._kagemusha_usage  # type: ignore[attr-defined]
                    status = 0
                    break
            if exceeded or timed_out:
                waited, status, usage = os.wait4(process.pid, 0)
                assert waited == process.pid
                break
            time.sleep(MAX_PROCESS_POLL_SECONDS)
        if process.returncode is None:
            process.returncode = os.waitstatus_to_exitcode(status)
        _quiesce_process_group(process.pid)
        os.fsync(stdout_fd)
        os.fsync(stderr_fd)
        stdout_info = _stable_transcript_from_fd(
            stdout_fd,
            stdout_path,
            transcript_limit=transcript_limit,
            require_nonempty=require_nonempty_streams,
        )
        stderr_info = _stable_transcript_from_fd(
            stderr_fd,
            stderr_path,
            transcript_limit=transcript_limit,
            require_nonempty=require_nonempty_streams,
        )
    finally:
        if stdout_fd >= 0:
            os.close(stdout_fd)
        if stderr_fd >= 0:
            os.close(stderr_fd)
    if process is None or usage is None:
        _fail("command could not be observed")
    if exceeded:
        _fail("command transcript exceeded its byte limit")
    if timed_out:
        _fail("command exceeded its timeout")

    elapsed_ms = max(1, math.ceil((time.monotonic() - started) * 1_000))
    cpu_millis = max(1, math.ceil((usage.ru_utime + usage.ru_stime) * 1_000))
    peak_rss = int(usage.ru_maxrss)
    if sys.platform.startswith("linux"):
        peak_rss *= 1024
    if peak_rss < 1:
        _fail("command did not yield a positive peak-RSS observation")
    return ProcessResult(
        exit_code=process.returncode,
        started_at_ms=max(1, started_at_ms),
        duration_ms=elapsed_ms,
        cpu_millis=cpu_millis,
        peak_rss_bytes=peak_rss,
        stdout=stdout_info,
        stderr=stderr_info,
    )


def _binding_from_record(record: Mapping[str, object]) -> dict[str, object]:
    return {"sha256": record["sha256"], "byte_len": record["byte_len"]}


def _derive_candidate_context(
    template: Mapping[str, object],
    records: Mapping[str, Mapping[str, object]],
    policy_info: StableFile,
) -> tuple[dict[str, object], str]:
    source = template["source"]
    assert isinstance(source, Mapping)
    source_archive = _relative_path(source["source_archive"], "source archive")
    cargo_lock = _relative_path(source["cargo_lock"], "Cargo.lock")
    artifacts_raw = template["artifacts"]
    assert isinstance(artifacts_raw, list)
    artifacts = []
    for raw in artifacts_raw:
        assert isinstance(raw, Mapping)
        path = _relative_path(raw["path"], "artifact path")
        artifacts.append({"role": raw["role"], **_binding_from_record(records[path])})
    protocols = template["protocols"]
    assert isinstance(protocols, Mapping)
    profiles_raw = template["profiles"]
    assert isinstance(profiles_raw, list)
    profiles = []
    for raw in profiles_raw:
        assert isinstance(raw, Mapping)
        profiles.append(
            {"hardware_profile": dict(raw["hardware_profile"]), "suite_id": raw["suite_id"]}
        )
    artifact_set_digest = release_verifier.rust_artifact_set_digest(artifacts)
    vk_digest = release_verifier.rust_vk_set_digest(artifacts, protocols)
    return release_verifier.release_candidate_context(
        source_archive=_binding_from_record(records[source_archive]),
        cargo_lock=_binding_from_record(records[cargo_lock]),
        artifacts=artifacts,
        artifact_set_digest=artifact_set_digest,
        vk_digest=vk_digest,
        protocols=protocols,
        profile_inputs=profiles,
        observer_policy={"sha256": policy_info.sha256, "byte_len": policy_info.size},
    )


def _subject_digest(subject: Mapping[str, object]) -> str:
    return hashlib.sha256(canonical_json_bytes(dict(subject))).hexdigest()


def _collection_id(body: Mapping[str, object]) -> str:
    payload = canonical_json_bytes(dict(body))
    return hashlib.sha256(b"iroha:kagemusha:v1:release-collection\0" + payload).hexdigest()


def _validate_report_headers(
    root: Path,
    plan: CollectionPlan,
    records: Mapping[str, Mapping[str, object]],
) -> None:
    step_by_report = {step.report: step for step in plan.verification_steps}
    for path, schema in sorted(plan.expected_reports.items()):
        record = records[path]
        _, payload = artifact_contract.stable_read_relative(
            root, path, max_size=release_verifier.MAX_REPORT_BYTES, return_payload=True
        )
        assert payload is not None
        report = load_json_object(payload, f"typed report {path!r}")
        if canonical_json_bytes(report) != payload:
            _fail(f"typed report {path!r} is not canonical JSON")
        if (
            report.get("schema") != schema
            or report.get("schema_version") != SCHEMA_VERSION
            or report.get("verification_id") != step_by_report[path].step_id
        ):
            _fail(f"typed report {path!r} has the wrong schema, version, or verification id")
        if record["kind"] != "report":
            _fail(f"typed report {path!r} must have evidence kind 'report'")


def _load_inputs(args: argparse.Namespace) -> tuple[
    StableFile,
    Mapping[str, object],
    release_verifier.TrustedObserverPolicy,
    CollectionPlan,
    dict[str, object],
]:
    plan_info, raw_plan = _load_pinned_json(
        args.plan, args.plan_sha256, label="collection plan", max_size=MAX_PLAN_BYTES
    )
    policy_path = _absolute_path(
        str(args.observer_policy), "observer policy", must_exist=True
    )
    try:
        policy = release_verifier._load_observer_policy(
            policy_path, args.observer_policy_sha256
        )
    except (ReleaseArtifactError, release_verifier.KagemushaEvidenceError) as error:
        raise KagemushaRunnerError(str(error)) from error
    plan = _parse_plan(raw_plan, policy)
    toolchain = _toolchain_closure(args)
    return plan_info, raw_plan, policy, plan, toolchain


def _assert_control_inputs_unchanged(
    args: argparse.Namespace,
    *,
    plan_info: StableFile,
    policy: release_verifier.TrustedObserverPolicy,
    plan: CollectionPlan,
    toolchain: Mapping[str, object],
) -> None:
    """Re-pin every non-evidence input at a publication boundary."""

    try:
        current_plan = stable_hash_path(Path(args.plan), max_size=MAX_PLAN_BYTES)
        current_policy = stable_hash_path(
            policy.path, max_size=release_verifier.MAX_OBSERVER_POLICY_BYTES
        )
    except ReleaseArtifactError as error:
        raise KagemushaRunnerError(str(error)) from error
    if current_plan != plan_info:
        _fail("collection plan changed during the operation")
    if current_policy != policy.info:
        _fail("observer policy changed during the operation")
    if _toolchain_closure(args) != toolchain:
        _fail("verifier source/runtime closure changed during the operation")
    for step in (*plan.producer_steps, *plan.verification_steps):
        _validate_executable(step.executable, f"step {step.step_id}")


def _plan_projection(
    plan_info: StableFile,
    policy: release_verifier.TrustedObserverPolicy,
    plan: CollectionPlan,
    toolchain: Mapping[str, object],
) -> dict[str, object]:
    return {
        "schema": RUN_RESULT_SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "status": "dry_run",
        "plan": {"sha256": plan_info.sha256, "byte_len": plan_info.size},
        "observer_policy": {"sha256": policy.info.sha256, "byte_len": policy.info.size},
        "toolchain": dict(toolchain),
        "seed_file_count": len(plan.seed_files),
        "producer_step_count": len(plan.producer_steps),
        "verification_step_count": len(plan.verification_steps),
        "report_count": len(plan.expected_reports),
    }


def _collect(args: argparse.Namespace) -> int:
    plan_info, _, policy, plan, toolchain = _load_inputs(args)
    out_dir = Path(args.out_dir)
    _prepare_new_output(out_dir, dry_run=args.dry_run)
    if args.dry_run:
        sys.stdout.buffer.write(
            canonical_json_bytes(_plan_projection(plan_info, policy, plan, toolchain))
        )
        return 0

    stage = out_dir / ".collecting"
    evidence_root = stage / "evidence"
    control_root = stage / "control"
    try:
        stage.mkdir(mode=0o700)
        evidence_root.mkdir(mode=0o700)
        control_root.mkdir(mode=0o700)
        records: dict[str, dict[str, object]] = {}

        for seed in plan.seed_files:
            destination = _destination(evidence_root, seed.evidence_path)
            expected = stable_hash_path(
                seed.source, max_size=release_verifier._max_for_kind(seed.kind)
            )
            _copy_seed(
                seed.source,
                destination,
                expected=expected,
                maximum=release_verifier._max_for_kind(seed.kind),
            )
            record = _capture_evidence_file(evidence_root, seed.evidence_path, seed.kind)
            if record["sha256"] != seed.sha256 or record["byte_len"] != seed.byte_len:
                _fail(f"copied seed {seed.evidence_path!r} differs from its pin")
            records[seed.evidence_path] = record

        producer_transcripts = control_root / "producer-transcripts"
        for step in plan.producer_steps:
            _validate_executable(step.executable, f"producer step {step.step_id}")
            actual_arguments: list[str] = []
            for kind, value in step.arguments:
                if kind == "literal":
                    actual_arguments.append(value)
                elif kind == "input":
                    actual_arguments.append(str(evidence_root.joinpath(*value.split("/"))))
                else:
                    destination = _destination(evidence_root, value)
                    if destination.exists() or destination.is_symlink():
                        _fail(f"producer output {value!r} already exists")
                    actual_arguments.append(str(destination))
            result = _run_process(
                step.executable.path,
                actual_arguments,
                cwd=evidence_root,
                stdout_path=producer_transcripts / f"{step.step_id}.stdout",
                stderr_path=producer_transcripts / f"{step.step_id}.stderr",
                timeout_ms=step.timeout_ms,
                transcript_limit=MAX_PRODUCER_TRANSCRIPT_BYTES,
                require_nonempty_streams=False,
            )
            if result.exit_code != 0:
                _fail(f"producer step {step.step_id!r} exited with {result.exit_code}")
            _validate_executable(step.executable, f"producer step {step.step_id}")
            for previous in records.values():
                _assert_record(evidence_root, previous)
            for output in step.outputs:
                path = evidence_root.joinpath(*output.evidence_path.split("/"))
                if not path.exists():
                    _fail(f"producer step {step.step_id!r} omitted output {output.evidence_path!r}")
                record = _capture_evidence_file(evidence_root, output.evidence_path, output.kind)
                if int(record["byte_len"]) > output.max_bytes:
                    _fail(f"producer output {output.evidence_path!r} exceeds its step limit")
                records[output.evidence_path] = record
            _scan_exact(evidence_root, set(records))

        _scan_exact(evidence_root, set(records))
        _validate_report_headers(evidence_root, plan, records)
        candidate_context, candidate_context_digest = _derive_candidate_context(
            plan.manifest_template, records, policy.info
        )

        command_rows: list[dict[str, object]] = []
        for step in plan.verification_steps:
            _validate_executable(step.executable, f"verification step {step.step_id}")
            projected_arguments: list[dict[str, object]] = []
            actual_arguments: list[str] = []
            for kind, value in step.arguments:
                if kind == "literal":
                    projected_arguments.append({"literal": value})
                    actual_arguments.append(value)
                else:
                    record = records[value]
                    projected_arguments.append(
                        {"file": value, **_binding_from_record(record)}
                    )
                    # The observation authenticates this exact relative argv and
                    # the verifier runs with the evidence root as cwd.
                    actual_arguments.append(value)
            stdout_path = _destination(evidence_root, step.stdout)
            stderr_path = _destination(evidence_root, step.stderr)
            result = _run_process(
                step.executable.path,
                actual_arguments,
                cwd=evidence_root,
                stdout_path=stdout_path,
                stderr_path=stderr_path,
                timeout_ms=step.timeout_ms,
                transcript_limit=release_verifier.MAX_TRANSCRIPT_BYTES,
                require_nonempty_streams=True,
            )
            if result.exit_code != 0:
                _fail(f"verification step {step.step_id!r} exited with {result.exit_code}")
            _validate_executable(step.executable, f"verification step {step.step_id}")
            for previous in records.values():
                _assert_record(evidence_root, previous)
            stdout_record = _capture_evidence_file(evidence_root, step.stdout, "transcript")
            stderr_record = _capture_evidence_file(evidence_root, step.stderr, "transcript")
            _assert_record_matches_info(
                stdout_record, result.stdout, label=f"step {step.step_id} stdout"
            )
            _assert_record_matches_info(
                stderr_record, result.stderr, label=f"step {step.step_id} stderr"
            )
            records[step.stdout] = stdout_record
            records[step.stderr] = stderr_record
            _scan_exact(evidence_root, set(records))
            subject = {
                "command_id": step.step_id,
                "verifier_id": step.verifier_id,
                "verifier_sha256": step.executable.sha256,
                "candidate_context_digest": candidate_context_digest,
                "report_schema": step.report_schema,
                "arguments": projected_arguments,
                "exit_code": 0,
                "stdout": _binding_from_record(stdout_record),
                "stderr": _binding_from_record(stderr_record),
                "started_at_ms": result.started_at_ms,
                "duration_ms": result.duration_ms,
                "cpu_millis": result.cpu_millis,
                "peak_rss_bytes": result.peak_rss_bytes,
            }
            command_rows.append(
                {
                    "id": step.step_id,
                    "verifier_id": step.verifier_id,
                    "report_schema": step.report_schema,
                    "arguments": [
                        {kind: value} for kind, value in step.arguments
                    ],
                    "stdout": step.stdout,
                    "stderr": step.stderr,
                    "observation": step.observation,
                    "subject": subject,
                }
            )

        _assert_control_inputs_unchanged(
            args,
            plan_info=plan_info,
            policy=policy,
            plan=plan,
            toolchain=toolchain,
        )
        state_body: dict[str, object] = {
            "plan": {"sha256": plan_info.sha256, "byte_len": plan_info.size},
            "observer_policy": {
                "sha256": policy.info.sha256,
                "byte_len": policy.info.size,
            },
            "toolchain": dict(toolchain),
            "candidate_context": candidate_context,
            "candidate_context_digest": candidate_context_digest,
            "files": [records[path] for path in sorted(records)],
            "commands": command_rows,
        }
        collection_id = _collection_id(state_body)
        state = {
            "schema": COLLECTION_STATE_SCHEMA,
            "schema_version": SCHEMA_VERSION,
            "collection_id": collection_id,
            **state_body,
        }
        requests = control_root / "signing-requests"
        requests.mkdir(mode=0o700)
        for command in command_rows:
            subject = command["subject"]
            assert isinstance(subject, Mapping)
            request = {
                "schema": SIGNING_REQUEST_SCHEMA,
                "schema_version": SCHEMA_VERSION,
                "collection_id": collection_id,
                "command_id": command["id"],
                "subject_sha256": _subject_digest(subject),
                "subject": dict(subject),
            }
            _atomic_write(requests / f"{command['id']}.json", canonical_json_bytes(request))
        _atomic_write(control_root / "collection-state.json", canonical_json_bytes(state))
        _atomic_write(
            control_root / "candidate-context.json", canonical_json_bytes(candidate_context)
        )

        os.replace(evidence_root, out_dir / "evidence")
        os.replace(control_root, out_dir / "control")
        stage.rmdir()
        directory_fd = os.open(out_dir, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    except Exception:
        # The output path was atomically reserved by this invocation and did not
        # exist beforehand, so cleanup cannot remove caller-owned material.
        shutil.rmtree(out_dir, ignore_errors=True)
        raise

    result = {
        "schema": RUN_RESULT_SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "status": "awaiting_approvals",
        "collection_id": collection_id,
        "candidate_context_digest": candidate_context_digest,
        "signing_request_count": len(command_rows),
        "signing_requests": str(out_dir / "control" / "signing-requests"),
    }
    sys.stdout.buffer.write(canonical_json_bytes(result))
    return EXIT_PENDING_APPROVALS


def _load_collection_state(out_dir: Path) -> Mapping[str, object]:
    path = out_dir / "control" / "collection-state.json"
    _, payload = stable_read_path(path, max_size=MAX_COLLECTION_STATE_BYTES)
    state = load_json_object(payload, "collection state")
    required = {
        "schema",
        "schema_version",
        "collection_id",
        "plan",
        "observer_policy",
        "toolchain",
        "candidate_context",
        "candidate_context_digest",
        "files",
        "commands",
    }
    _object(state, "collection state", required)
    if (
        state["schema"] != COLLECTION_STATE_SCHEMA
        or state["schema_version"] != SCHEMA_VERSION
        or canonical_json_bytes(state) != payload
    ):
        _fail("collection state is not canonical supported V1 JSON")
    body = {
        field: state[field]
        for field in required - {"schema", "schema_version", "collection_id"}
    }
    if state["collection_id"] != _collection_id(body):
        _fail("collection state id does not authenticate its body")
    return state


def _validate_state_records(
    state: Mapping[str, object], evidence_root: Path
) -> tuple[dict[str, Mapping[str, object]], dict[str, Mapping[str, object]]]:
    records: dict[str, Mapping[str, object]] = {}
    expected_file_fields = {
        "path",
        "kind",
        "sha256",
        "byte_len",
        "mode",
        "device",
        "inode",
        "mtime_ns",
        "ctime_ns",
        "link_count",
    }
    paths: list[str] = []
    for index, raw in enumerate(_array(state["files"], "collected files")):
        record = _object(raw, f"collected file {index}", expected_file_fields)
        path = _relative_path(record["path"], f"collected file {index} path")
        if path in records:
            _fail("collection state repeats a file path")
        records[path] = record
        paths.append(path)
    if paths != sorted(paths):
        _fail("collection state files are not sorted")
    _scan_exact(evidence_root, set(records))
    for record in records.values():
        _assert_record(evidence_root, record)

    commands: dict[str, Mapping[str, object]] = {}
    command_ids: list[str] = []
    command_fields = {
        "id",
        "verifier_id",
        "report_schema",
        "arguments",
        "stdout",
        "stderr",
        "observation",
        "subject",
    }
    for index, raw in enumerate(_array(state["commands"], "collected commands")):
        command = _object(raw, f"collected command {index}", command_fields)
        command_id = _safe_id(command["id"], f"collected command {index} id")
        if command_id in commands:
            _fail("collection state repeats a command id")
        subject = _object(
            command["subject"],
            f"collected command {command_id} subject",
            {
                "command_id",
                "verifier_id",
                "verifier_sha256",
                "candidate_context_digest",
                "report_schema",
                "arguments",
                "exit_code",
                "stdout",
                "stderr",
                "started_at_ms",
                "duration_ms",
                "cpu_millis",
                "peak_rss_bytes",
            },
        )
        if subject["command_id"] != command_id:
            _fail("collection state command and subject ids differ")
        commands[command_id] = command
        command_ids.append(command_id)
    if command_ids != sorted(command_ids):
        _fail("collection state commands are not sorted")
    return records, commands


def _load_detached_approvals(
    approvals_dir: Path,
    *,
    out_dir: Path,
    state: Mapping[str, object],
    commands: Mapping[str, Mapping[str, object]],
    policy: release_verifier.TrustedObserverPolicy,
) -> dict[str, list[dict[str, object]]]:
    directory = _absolute_path(str(approvals_dir), "approvals directory", must_exist=True)
    _ensure_private_directory(directory, "approvals directory")
    try:
        directory.relative_to(out_dir)
    except ValueError:
        pass
    else:
        _fail("detached approvals directory must be outside the runner output")
    approvals: dict[str, dict[str, dict[str, object]]] = {
        command_id: {} for command_id in commands
    }
    maximum_files = len(commands) * len(policy.authorities)
    entries = sorted(os.scandir(directory), key=lambda entry: entry.name)
    if not entries or len(entries) > maximum_files:
        _fail("detached approvals directory has an invalid file count")
    for entry in entries:
        info = entry.stat(follow_symlinks=False)
        if entry.is_symlink() or not stat.S_ISREG(info.st_mode):
            _fail("detached approval entries must be regular files")
        path = Path(entry.path)
        _, payload = stable_read_path(path, max_size=MAX_APPROVAL_BYTES)
        approval = load_json_object(payload, f"detached approval {entry.name!r}")
        _object(
            approval,
            f"detached approval {entry.name!r}",
            {
                "schema",
                "schema_version",
                "collection_id",
                "command_id",
                "subject_sha256",
                "authority_id",
                "signature",
            },
        )
        if (
            approval["schema"] != DETACHED_APPROVAL_SCHEMA
            or approval["schema_version"] != SCHEMA_VERSION
            or canonical_json_bytes(approval) != payload
        ):
            _fail("detached approval is not canonical supported V1 JSON")
        if approval["collection_id"] != state["collection_id"]:
            _fail("detached approval names a different collection")
        command_id = _safe_id(approval["command_id"], "detached approval command id")
        command = commands.get(command_id)
        if command is None:
            _fail("detached approval names an unknown command")
        subject = command["subject"]
        assert isinstance(subject, Mapping)
        if approval["subject_sha256"] != _subject_digest(subject):
            _fail("detached approval substitutes its verification subject")
        authority_id = _digest(approval["authority_id"], "detached approval authority id")
        public_key = policy.authorities.get(authority_id)
        if public_key is None:
            _fail("detached approval names an unknown authority")
        signature_text = _string(approval["signature"], "detached approval signature")
        if len(signature_text) != 128 or re.fullmatch(r"[0-9a-f]{128}", signature_text) is None:
            _fail("detached approval signature must be 64 lowercase-hex bytes")
        signature = bytes.fromhex(signature_text)
        if not release_verifier._ed25519_verify(
            public_key, release_verifier._approval_message(subject), signature
        ):
            _fail("detached approval signature is invalid")
        if authority_id in approvals[command_id]:
            _fail("detached approval repeats an authority for one command")
        approvals[command_id][authority_id] = {
            "authority_id": authority_id,
            "signature": signature_text,
        }
    result: dict[str, list[dict[str, object]]] = {}
    for command_id in sorted(commands):
        command_approvals = approvals[command_id]
        if len(command_approvals) < policy.threshold:
            _fail(f"command {command_id!r} lacks the trusted approval threshold")
        result[command_id] = [command_approvals[key] for key in sorted(command_approvals)]
    return result


def _remove_finalization_outputs(
    observation_paths: Sequence[Path],
    manifest_path: Path,
    projection_path: Path,
    *,
    evidence_root: Path,
    transcript_paths: Sequence[Path],
) -> None:
    for path in (*observation_paths, manifest_path, projection_path, *transcript_paths):
        try:
            path.unlink(missing_ok=True)
        except OSError:
            pass
    for observation_path in observation_paths:
        parent = observation_path.parent
        while parent != evidence_root:
            try:
                parent.rmdir()
            except OSError:
                break
            parent = parent.parent


def _run_projection_verifier(
    *,
    out_dir: Path,
    manifest_path: Path,
    manifest_sha256: str,
    evidence_root: Path,
    observer_policy_path: Path,
    observer_policy_sha256: str,
    toolchain: Mapping[str, object],
) -> dict[str, object]:
    python_binding = _object(
        toolchain.get("python_executable"),
        "projector Python binding",
        {"path", "sha256", "byte_len"},
    )
    verifier_binding = _object(
        toolchain.get("release_verifier"),
        "projector verifier binding",
        {"path", "sha256", "byte_len"},
    )
    contract_binding = _object(
        toolchain.get("artifact_contract"),
        "projector contract binding",
        {"path", "sha256", "byte_len"},
    )
    if (
        python_binding["path"] != str(RESOLVED_PYTHON)
        or verifier_binding["path"] != str(BUNDLED_RELEASE_VERIFIER)
        or contract_binding["path"] != str(BUNDLED_ARTIFACT_CONTRACT)
    ):
        _fail("projector source/runtime closure names an unexpected path")
    stdout_path = out_dir / "control" / "final-verifier.stdout"
    stderr_path = out_dir / "control" / "final-verifier.stderr"
    result = _run_process(
        RESOLVED_PYTHON,
        [
            "-I",
            "-B",
            "-S",
            "-c",
            PROJECTOR_BOOTSTRAP,
            str(SCRIPT_DIR),
            str(python_binding["path"]),
            str(python_binding["sha256"]),
            str(verifier_binding["path"]),
            str(verifier_binding["sha256"]),
            str(contract_binding["path"]),
            str(contract_binding["sha256"]),
            "--manifest",
            str(manifest_path),
            "--manifest-sha256",
            manifest_sha256,
            "--evidence-root",
            str(evidence_root),
            "--observer-policy",
            str(observer_policy_path),
            "--observer-policy-sha256",
            observer_policy_sha256,
        ],
        cwd=out_dir,
        stdout_path=stdout_path,
        stderr_path=stderr_path,
        timeout_ms=MAX_STEP_TIMEOUT_MS,
        transcript_limit=release_verifier.MAX_TRANSCRIPT_BYTES,
        require_nonempty_streams=False,
    )
    if result.exit_code != 0:
        message = stderr_path.read_text(encoding="utf-8", errors="replace")[:4096].strip()
        _fail(f"bundled release verifier rejected the collection: {message}")
    stdout_info, payload = stable_read_path(
        stdout_path, max_size=release_verifier.MAX_TRANSCRIPT_BYTES
    )
    if stdout_info != result.stdout:
        _fail("release verifier projection differs from its exact output descriptor")
    projection = load_json_object(payload, "release verifier projection")
    if canonical_json_bytes(projection) != payload:
        _fail("release verifier output is not canonical JSON")
    if (
        projection.get("schema") != release_verifier.PROJECTION_SCHEMA
        or projection.get("schema_version") != SCHEMA_VERSION
        or projection.get("manifest_sha256") != manifest_sha256
    ):
        _fail("release verifier output has the wrong identity")
    return projection


def _finalize(args: argparse.Namespace) -> int:
    plan_info, _, policy, plan, toolchain = _load_inputs(args)
    out_dir = _open_existing_output(Path(args.out_dir))
    evidence_root = out_dir / "evidence"
    _ensure_private_directory(evidence_root, "evidence root")
    state = _load_collection_state(out_dir)
    if state["plan"] != {"sha256": plan_info.sha256, "byte_len": plan_info.size}:
        _fail("finalize plan differs from the collected plan")
    if state["observer_policy"] != {
        "sha256": policy.info.sha256,
        "byte_len": policy.info.size,
    }:
        _fail("finalize observer policy differs from the collected policy")
    if state["toolchain"] != toolchain:
        _fail("finalize verifier source/runtime closure differs from collection")
    records, commands = _validate_state_records(state, evidence_root)
    if set(commands) != {step.step_id for step in plan.verification_steps}:
        _fail("collected commands differ from the pinned plan")
    for step in plan.producer_steps:
        _validate_executable(step.executable, f"producer step {step.step_id}")
    for step in plan.verification_steps:
        _validate_executable(step.executable, f"verification step {step.step_id}")
        command = commands[step.step_id]
        if (
            command["verifier_id"] != step.verifier_id
            or command["report_schema"] != step.report_schema
            or command["arguments"] != [{kind: value} for kind, value in step.arguments]
            or command["stdout"] != step.stdout
            or command["stderr"] != step.stderr
            or command["observation"] != step.observation
        ):
            _fail(f"collected command {step.step_id!r} differs from the pinned plan")

    _, derived_context_digest = _derive_candidate_context(
        plan.manifest_template, records, policy.info
    )
    if (
        state["candidate_context_digest"] != derived_context_digest
        or not all(
            isinstance(command["subject"], Mapping)
            and command["subject"].get("candidate_context_digest") == derived_context_digest
            for command in commands.values()
        )
    ):
        _fail("candidate context changed after collection")

    approvals = _load_detached_approvals(
        Path(args.approvals_dir),
        out_dir=out_dir,
        state=state,
        commands=commands,
        policy=policy,
    )
    manifest_path = out_dir / "manifest.json"
    projection_path = out_dir / "projection.json"
    if manifest_path.exists() or projection_path.exists():
        _fail("finalization outputs already exist")

    observation_paths: list[Path] = []
    try:
        for command_id in sorted(commands):
            command = commands[command_id]
            relative = _relative_path(command["observation"], "observation path")
            path = _destination(evidence_root, relative)
            observation = {
                "schema": release_verifier.OBSERVATION_SCHEMA,
                "schema_version": SCHEMA_VERSION,
                "subject": dict(command["subject"]),
                "approvals": approvals[command_id],
            }
            _atomic_write(path, canonical_json_bytes(observation))
            observation_paths.append(path)
            records[relative] = _capture_evidence_file(evidence_root, relative, "observation")
        _scan_exact(evidence_root, set(records))
        for record in records.values():
            _assert_record(evidence_root, record)

        manifest = copy.deepcopy(dict(plan.manifest_template))
        manifest["files"] = [
            {
                "path": path,
                "kind": records[path]["kind"],
                "sha256": records[path]["sha256"],
                "byte_len": records[path]["byte_len"],
            }
            for path in sorted(records)
        ]
        manifest["commands"] = [
            {
                "id": command_id,
                "verifier_id": commands[command_id]["verifier_id"],
                "report_schema": commands[command_id]["report_schema"],
                "arguments": commands[command_id]["arguments"],
                "stdout": commands[command_id]["stdout"],
                "stderr": commands[command_id]["stderr"],
                "observation": commands[command_id]["observation"],
            }
            for command_id in sorted(commands)
        ]
        manifest_payload = canonical_json_bytes(manifest)
        manifest_sha256 = hashlib.sha256(manifest_payload).hexdigest()
        _atomic_write(manifest_path, manifest_payload)
        projection = _run_projection_verifier(
            out_dir=out_dir,
            manifest_path=manifest_path,
            manifest_sha256=manifest_sha256,
            evidence_root=evidence_root,
            observer_policy_path=policy.path,
            observer_policy_sha256=policy.info.sha256,
            toolchain=toolchain,
        )
        if (
            projection.get("receipt_projection", {})
            .get("evidence_closure", {})
            .get("candidate_context_digest")
            != derived_context_digest
        ):
            _fail("release verifier projection names a different candidate context")
        # Re-pin both tooling and evidence after the authoritative subprocess
        # returns and before publishing a verified projection.
        _assert_control_inputs_unchanged(
            args,
            plan_info=plan_info,
            policy=policy,
            plan=plan,
            toolchain=toolchain,
        )
        _scan_exact(evidence_root, set(records))
        for record in records.values():
            _assert_record(evidence_root, record)
        _atomic_write(projection_path, canonical_json_bytes(projection))
    except Exception:
        _remove_finalization_outputs(
            observation_paths,
            manifest_path,
            projection_path,
            evidence_root=evidence_root,
            transcript_paths=(
                out_dir / "control" / "final-verifier.stdout",
                out_dir / "control" / "final-verifier.stderr",
            ),
        )
        raise

    result = {
        "schema": RUN_RESULT_SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "status": "verified",
        "collection_id": state["collection_id"],
        "candidate_context_digest": derived_context_digest,
        "manifest": {"path": str(manifest_path), "sha256": manifest_sha256},
        "projection": {
            "path": str(projection_path),
            "sha256": hashlib.sha256(canonical_json_bytes(projection)).hexdigest(),
        },
    }
    sys.stdout.buffer.write(canonical_json_bytes(result))
    return 0


def _add_common_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--plan", required=True, type=Path)
    parser.add_argument("--plan-sha256", required=True)
    parser.add_argument("--observer-policy", required=True, type=Path)
    parser.add_argument("--observer-policy-sha256", required=True)
    parser.add_argument("--out-dir", required=True, type=Path)
    parser.add_argument("--python-executable-sha256", required=True)
    parser.add_argument("--release-verifier-sha256", required=True)
    parser.add_argument("--artifact-contract-sha256", required=True)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Collect and finalize KAGEMUSHA V1 release evidence without signing keys."
    )
    commands = parser.add_subparsers(dest="command", required=True)
    collect = commands.add_parser("collect", help="run the pinned qualification plan")
    _add_common_arguments(collect)
    collect.add_argument("--dry-run", action="store_true")
    finalize = commands.add_parser("finalize", help="assemble approved evidence and verify it")
    _add_common_arguments(finalize)
    finalize.add_argument("--approvals-dir", required=True, type=Path)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Run the collection or finalization phase."""

    args = _parser().parse_args(argv)
    try:
        _bootstrap_local_modules(args)
        if args.command == "collect":
            return _collect(args)
        return _finalize(args)
    except (
        KagemushaRunnerError,
        ReleaseArtifactError,
        OSError,
        ValueError,
        TypeError,
        json.JSONDecodeError,
    ) as error:
        print(f"KAGEMUSHA release-evidence runner failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    if not (
        sys.flags.isolated
        and sys.flags.dont_write_bytecode
        and sys.flags.no_site
    ):
        print(
            "KAGEMUSHA release-evidence runner requires an absolute Python "
            "interpreter invoked with -I -B -S",
            file=sys.stderr,
        )
        raise SystemExit(1)
    raise SystemExit(main())
