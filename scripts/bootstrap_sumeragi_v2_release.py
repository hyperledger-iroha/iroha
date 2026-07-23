#!/usr/bin/env python3
"""Authenticate a Sumeragi v2 release candidate before running its code.

This file is a trust root, not part of the candidate trust chain.  A release
operator MUST install it outside the candidate checkout and authenticate its
bytes (and the expected digest supplied below) before starting Python.  The
bootstrap's check of its own digest is useful evidence, but cannot make an
untrusted bootstrap trustworthy.  Invoke it with the protected interpreter as
``/absolute/python3 -I -S /absolute/bootstrap_sumeragi_v2_release.py ...``;
isolated, no-site startup is enforced before any candidate data is inspected.
The external launcher must also provide a loader-clean environment and
authenticate the release-host image and dynamic libraries: those events occur
before this Python code can enforce its closed child environments.

The release-host account and every owner of an ancestor of the trusted inputs,
candidate, and evidence directory are part of the trust boundary.  This tool
rejects symlinks and revalidates bytes, modes, and inodes, but it does not claim
to withstand a malicious same-UID process or a malicious trusted ancestor that
can swap pathnames between checks.
"""

from __future__ import annotations

import argparse
import base64
import binascii
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import selectors
import shutil
import signal
import stat
import subprocess
import sys
import time
from typing import Any, Iterable


_DIGEST_RE = re.compile(r"[0-9a-f]{64}")
_FINGERPRINT_RE = re.compile(r"SHA256:[A-Za-z0-9+/]{43}")
_OBJECT_ID_RE = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
_SAFE_PATH_RE = re.compile(r"/[A-Za-z0-9_./+:-]+")
_RUNNER_ENV_RE = re.compile(r"[A-Z][A-Z0-9_]*")
_RUNNER_TOOL_NAME_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+-]*")
_RUNNER_ENV_ALLOWLIST = {
    "CARGO_HOME",
    "CARGO_NET_GIT_FETCH_WITH_CLI",
    "CARGO_NET_OFFLINE",
    "NIX_SSL_CERT_FILE",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "SSL_CERT_FILE",
}
_IDENTITY_KEYS = {
    "schema_version",
    "head_commit",
    "head_tree",
    "index_tree",
    "workspace_source_manifest_sha256",
    "cargo_lock_sha256",
}
_EVIDENCE_KEYS = {
    "cargo_lock",
    "git",
    "raw_commit",
    "ssh_allowed_signers",
    "ssh_keygen",
    "ssh_revocation",
    "verify_transcript",
}
_ATTESTATION_KEYS = {
    "schema_version",
    "release_identity",
    "release_identity_sha256",
    "tools",
    "policies",
    "verification",
    "evidence",
}
_TERMINAL_EVIDENCE_KEYS = {
    "bootstrap",
    "release_signature_attestation",
    "release_signature_transcript",
    "release_signature_raw_commit",
    "release_signature_cargo_lock",
    "release_signature_allowed_signers",
    "release_signature_revocation",
    "release_signature_git",
    "release_signature_ssh_keygen",
    "corridor_completion",
    "corridor_summary",
    "corridor_production_inventory",
    "corridor_logs",
    "formal_completion",
    "formal_gate_log",
    "formal_proof_coverage",
    "formal_proof_evidence",
    "formal_verus_evidence",
    "formal_verus_log",
    "formal_cross_tool_evidence",
    "formal_harness_lock",
    "formal_toolchain",
    "seed_matrix_completion",
    "seed_matrix_summary",
    "seed_matrix_run_logs",
    "seed_matrix_localnet_manifest_index",
    "seed_matrix_localnet_manifests",
    "chaos_completion",
    "chaos_log",
    "taira_completion",
    "taira_evidence",
    "taira_run_log",
}
_TRANSCRIPT_KEYS = {
    "schema_version",
    "archive_names",
    "candidate_commit_oid",
    "environment",
    "policy_overrides",
    "policies",
    "replay",
    "tools",
    "commands",
    "tool_probes",
}
_MAX_TOOL_BYTES = 512 * 1024 * 1024
_MAX_HELPER_BYTES = 16 * 1024 * 1024
_MAX_POLICY_BYTES = 16 * 1024 * 1024
_MAX_IDENTITY_BYTES = 64 * 1024
_MAX_EVIDENCE_BYTES = 128 * 1024 * 1024
_MAX_TERMINAL_RECEIPT_BYTES = 64 * 1024 * 1024
_MAX_HELPER_OUTPUT_BYTES = 16 * 1024 * 1024
_MAX_RUNNER_TOOLS = 256
_DEFAULT_COMMAND_TIMEOUT_SECONDS = 600
_DIRECTORY_MODE = 0o700
_TOOL_MODE = 0o500
_DATA_MODE = 0o400


class BootstrapError(RuntimeError):
    """A closed bootstrap prerequisite or postcondition failed."""


class RunnerLaunchError(BootstrapError):
    """The authenticated runner never acquired a child process."""


@dataclass(frozen=True)
class FileSnapshot:
    """Stable bytes and metadata for one non-symlink regular file."""

    path: Path
    data: bytes
    device: int
    inode: int
    mode: int
    owner: int
    nlink: int
    size: int
    mtime_ns: int
    ctime_ns: int

    @property
    def sha256(self) -> str:
        """Return the SHA-256 digest of the captured bytes."""

        return hashlib.sha256(self.data).hexdigest()


@dataclass(frozen=True)
class DirectorySnapshot:
    """Stable identity and metadata for one private non-symlink directory."""

    path: Path
    device: int
    inode: int
    mode: int
    owner: int
    nlink: int
    mtime_ns: int
    ctime_ns: int


@dataclass(frozen=True)
class LargeFileSnapshot:
    """Stable metadata and streaming digest for a potentially large file."""

    path: Path
    sha256: str
    device: int
    inode: int
    mode: int
    owner: int
    nlink: int
    size: int
    mtime_ns: int
    ctime_ns: int


@dataclass(frozen=True)
class SymlinkSnapshot:
    """Stable identity and exact target for one private runner alias."""

    path: Path
    target: str
    device: int
    inode: int
    mode: int
    owner: int
    nlink: int
    mtime_ns: int
    ctime_ns: int


@dataclass(frozen=True)
class CommandResult:
    """Bounded command outcome."""

    returncode: int
    stdout: bytes
    stderr: bytes


def _canonical_json(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def _require_digest(value: str, label: str) -> str:
    if _DIGEST_RE.fullmatch(value) is None:
        raise BootstrapError(f"{label} must be one lowercase SHA-256 digest")
    return value


def _absolute_resolved_existing(path: Path, label: str) -> Path:
    if not path.is_absolute():
        raise BootstrapError(f"{label} must be an absolute resolved path")
    absolute = Path(os.path.abspath(path))
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise BootstrapError(f"{label} is unavailable") from error
    if path != absolute or path != resolved:
        raise BootstrapError(f"{label} must be an absolute resolved non-symlink path")
    return path


def _inside(path: Path, root: Path) -> bool:
    return path == root or root in path.parents


def _read_file(
    path: Path,
    label: str,
    *,
    maximum_bytes: int,
    executable: bool = False,
) -> FileSnapshot:
    path = _absolute_resolved_existing(path, label)
    try:
        before = path.lstat()
    except OSError as error:
        raise BootstrapError(f"{label} is unavailable") from error
    if not stat.S_ISREG(before.st_mode) or stat.S_ISLNK(before.st_mode):
        raise BootstrapError(f"{label} must be a regular non-symlink file")
    if executable and before.st_mode & 0o111 == 0:
        raise BootstrapError(f"{label} must be executable")
    if before.st_size > maximum_bytes:
        raise BootstrapError(f"{label} exceeds its closed size limit")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise BootstrapError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
            or stat.S_IMODE(opened.st_mode) != stat.S_IMODE(before.st_mode)
        ):
            raise BootstrapError(f"{label} changed while it was opened")
        chunks: list[bytes] = []
        total = 0
        while True:
            chunk = os.read(descriptor, min(1024 * 1024, maximum_bytes + 1 - total))
            if not chunk:
                break
            chunks.append(chunk)
            total += len(chunk)
            if total > maximum_bytes:
                raise BootstrapError(f"{label} exceeds its closed size limit")
        after = os.fstat(descriptor)
        if (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
            stat.S_IMODE(after.st_mode),
        ) != (
            opened.st_dev,
            opened.st_ino,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
            stat.S_IMODE(opened.st_mode),
        ):
            raise BootstrapError(f"{label} changed while it was read")
        return FileSnapshot(
            path,
            b"".join(chunks),
            opened.st_dev,
            opened.st_ino,
            stat.S_IMODE(opened.st_mode),
            opened.st_uid,
            opened.st_nlink,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
        )
    finally:
        os.close(descriptor)


def _require_unchanged(
    snapshot: FileSnapshot,
    label: str,
    *,
    maximum_bytes: int,
    executable: bool = False,
) -> None:
    current = _read_file(
        snapshot.path,
        label,
        maximum_bytes=maximum_bytes,
        executable=executable,
    )
    if current != snapshot:
        raise BootstrapError(f"{label} changed during the release bootstrap")


def _protected_snapshot(
    path: Path,
    expected_digest: str,
    label: str,
    *,
    candidate: Path,
    maximum_bytes: int,
    executable: bool = False,
) -> FileSnapshot:
    snapshot = _read_file(
        path, label, maximum_bytes=maximum_bytes, executable=executable
    )
    if _inside(snapshot.path, candidate):
        raise BootstrapError(f"{label} must be installed outside the candidate root")
    if snapshot.sha256 != _require_digest(expected_digest, f"expected {label} digest"):
        raise BootstrapError(f"{label} does not match its protected SHA-256")
    return snapshot


def _prepare_evidence_directory(path: Path, candidate: Path) -> tuple[Path, int]:
    if not path.is_absolute() or path != Path(os.path.abspath(path)):
        raise BootstrapError("evidence directory must be an absolute normalized path")
    if path.exists() or path.is_symlink():
        raise BootstrapError("evidence directory already exists; overwrite is forbidden")
    parent = _absolute_resolved_existing(path.parent, "evidence-directory parent")
    parent_stat = parent.lstat()
    if (
        not stat.S_ISDIR(parent_stat.st_mode)
        or parent_stat.st_uid != os.getuid()
        or stat.S_IMODE(parent_stat.st_mode) != _DIRECTORY_MODE
    ):
        raise BootstrapError(
            "evidence-directory parent must be owner-owned with exact mode 0700"
        )
    path = parent / path.name
    if _SAFE_PATH_RE.fullmatch(str(path)) is None or os.pathsep in str(path):
        raise BootstrapError("evidence directory must use the shell-safe release path alphabet")
    if _inside(path, candidate):
        raise BootstrapError("evidence directory must be outside the candidate root")
    created = False
    try:
        os.mkdir(path, _DIRECTORY_MODE)
        created = True
        os.chmod(path, _DIRECTORY_MODE, follow_symlinks=False)
        parent_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
        if hasattr(os, "O_NOFOLLOW"):
            parent_flags |= os.O_NOFOLLOW
        parent_fd = os.open(parent, parent_flags)
        try:
            os.fsync(parent_fd)
        finally:
            os.close(parent_fd)
        flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        descriptor = os.open(path, flags)
    except OSError as error:
        if created:
            try:
                os.rmdir(path)
            except OSError:
                pass
        raise BootstrapError("private evidence directory could not be created") from error
    opened = os.fstat(descriptor)
    if (
        not stat.S_ISDIR(opened.st_mode)
        or stat.S_IMODE(opened.st_mode) != _DIRECTORY_MODE
        or opened.st_uid != os.getuid()
    ):
        os.close(descriptor)
        try:
            os.rmdir(path)
        except OSError:
            pass
        raise BootstrapError("evidence directory must be owner-owned with exact mode 0700")
    return path, descriptor


def _write_artifact(
    directory: Path,
    directory_fd: int,
    name: str,
    data: bytes,
    mode: int,
) -> FileSnapshot:
    if not name or name in {".", ".."} or "/" in name or "\0" in name:
        raise BootstrapError("invalid bootstrap evidence name")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(name, flags, mode, dir_fd=directory_fd)
        try:
            os.fchmod(descriptor, mode)
            view = memoryview(data)
            while view:
                written = os.write(descriptor, view)
                if written <= 0:
                    raise BootstrapError(f"short write for bootstrap evidence {name}")
                view = view[written:]
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        os.fsync(directory_fd)
    except OSError as error:
        raise BootstrapError(f"could not publish bootstrap evidence {name}") from error
    return _read_file(
        directory / name,
        f"bootstrap evidence {name}",
        maximum_bytes=max(len(data), 1),
        executable=mode == _TOOL_MODE,
    )


def _publish_completion_marker(
    directory: Path,
    directory_fd: int,
    data: bytes,
    *,
    final_name: str = "BOOTSTRAP_COMPLETED.json",
) -> FileSnapshot:
    if (
        not final_name
        or final_name in {".", ".."}
        or "/" in final_name
        or "\0" in final_name
    ):
        raise BootstrapError("invalid bootstrap completion marker name")
    temporary_name = f".{final_name}.stage.{secrets.token_hex(16)}"
    staged: FileSnapshot | None = None
    completed = False

    def unlink_owned(name: str) -> None:
        if staged is None:
            return
        try:
            metadata = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        except OSError:
            return
        if (
            stat.S_ISREG(metadata.st_mode)
            and (metadata.st_dev, metadata.st_ino) == (staged.device, staged.inode)
        ):
            try:
                os.unlink(name, dir_fd=directory_fd)
            except OSError:
                pass

    try:
        staged = _write_artifact(
            directory,
            directory_fd,
            temporary_name,
            data,
            _DATA_MODE,
        )
        os.link(
            temporary_name,
            final_name,
            src_dir_fd=directory_fd,
            dst_dir_fd=directory_fd,
            follow_symlinks=False,
        )
        os.fsync(directory_fd)
        marker = _read_file(
            directory / final_name,
            "bootstrap completion marker",
            maximum_bytes=max(len(data), 1),
        )
        if (
            marker.device,
            marker.inode,
            marker.mode,
            marker.owner,
            marker.nlink,
            marker.data,
        ) != (
            staged.device,
            staged.inode,
            staged.mode,
            os.getuid(),
            2,
            staged.data,
        ):
            raise BootstrapError("bootstrap completion marker changed at publication")
        os.unlink(temporary_name, dir_fd=directory_fd)
        os.fsync(directory_fd)
        published = _read_file(
            directory / final_name,
            "bootstrap completion marker",
            maximum_bytes=max(len(data), 1),
        )
        if (
            published.device,
            published.inode,
            published.mode,
            published.owner,
            published.nlink,
            published.data,
        ) != (
            marker.device,
            marker.inode,
            marker.mode,
            os.getuid(),
            1,
            marker.data,
        ):
            raise BootstrapError("bootstrap completion marker changed after publication")
        completed = True
        return published
    except OSError as error:
        raise BootstrapError("bootstrap completion marker could not be published") from error
    finally:
        if staged is not None and not completed:
            unlink_owned(final_name)
            unlink_owned(temporary_name)
            try:
                os.fsync(directory_fd)
            except OSError:
                pass


def _abort(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except (OSError, ProcessLookupError):
        return
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline:
        process.poll()
        try:
            os.killpg(process.pid, 0)
        except (OSError, ProcessLookupError):
            return
        time.sleep(0.05)
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except (OSError, ProcessLookupError):
        pass
    try:
        process.wait(timeout=2)
    except subprocess.TimeoutExpired:
        pass


def _run_bounded(
    executable: Path,
    arguments: Iterable[str],
    *,
    cwd: Path,
    environment: dict[str, str],
    timeout_seconds: int,
    maximum_output_bytes: int,
) -> CommandResult:
    argv = [str(executable), *arguments]
    try:
        process = subprocess.Popen(
            argv,
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
    except OSError as error:
        raise BootstrapError(f"could not execute protected command {executable}") from error
    assert process.stdout is not None and process.stderr is not None
    os.set_blocking(process.stdout.fileno(), False)
    os.set_blocking(process.stderr.fileno(), False)
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    total = 0
    deadline = time.monotonic() + timeout_seconds
    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError
            events = selector.select(min(remaining, 1.0))
            for key, _ in events:
                try:
                    chunk = os.read(key.fileobj.fileno(), 64 * 1024)
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                total += len(chunk)
                if total > maximum_output_bytes:
                    raise BootstrapError("protected command exceeded its bounded output limit")
                buffers[key.data].extend(chunk)
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError
        returncode = process.wait(timeout=remaining)
    except TimeoutError as error:
        _abort(process)
        raise BootstrapError("protected command exceeded its bounded runtime") from error
    except BaseException:
        _abort(process)
        raise
    finally:
        selector.close()
        process.stdout.close()
        process.stderr.close()
    return CommandResult(returncode, bytes(buffers["stdout"]), bytes(buffers["stderr"]))


def _run_release_runner(
    executable: Path,
    arguments: Iterable[str],
    *,
    cwd: Path,
    environment: dict[str, str],
    stdout_descriptor: int,
    stderr_descriptor: int,
) -> CommandResult:
    """Run the release runner with private regular-file diagnostic sinks.

    The runner owns Cargo, rustc, validator, formal, chaos, and soak processes.
    Their in-scope operations have their own protocol and harness deadlines;
    this bootstrap must never turn a slow or stuck child into apparently valid
    evidence by signalling the process group. A runner which never terminates
    therefore remains visibly incomplete and cannot reach either completion
    marker. Direct regular-file descriptors avoid relay backpressure and ensure
    that bootstrap interruption cannot close a pipe reader underneath the
    still-active runner or any of its descendants.
    """

    argv = [str(executable), *arguments]
    try:
        process = subprocess.Popen(
            argv,
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=stdout_descriptor,
            stderr=stderr_descriptor,
            close_fds=True,
            start_new_session=True,
        )
    except OSError as error:
        raise RunnerLaunchError(
            f"could not execute protected command {executable}"
        ) from error
    # Intentionally do not signal the runner on bootstrap interruption. Its
    # absent external terminal marker is the fail-closed result, while the
    # inherited regular-file descriptors remain valid in the active process
    # tree for post-mortem evidence.
    returncode = process.wait()
    return CommandResult(returncode, b"", b"")


def _open_runner_log(directory_fd: int, name: str) -> int:
    """Create one owner-only, no-clobber regular file for runner output."""

    if name in {"", ".", ".."} or "/" in name or "\0" in name:
        raise BootstrapError("runner log name is invalid")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(name, flags, 0o600, dir_fd=directory_fd)
    except OSError as error:
        raise BootstrapError(f"could not create private runner log {name}") from error
    metadata = os.fstat(descriptor)
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != os.getuid()
        or metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) != 0o600
    ):
        os.close(descriptor)
        raise BootstrapError(f"private runner log {name} has unsafe metadata")
    os.fsync(directory_fd)
    return descriptor


def _capture_large_file(path: Path, label: str) -> LargeFileSnapshot:
    """Hash one stable regular file without retaining its contents in memory."""

    path = _absolute_resolved_existing(path, label)
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or stat.S_ISLNK(before.st_mode):
        raise BootstrapError(f"{label} must be a regular non-symlink file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise BootstrapError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
            or opened.st_mode != before.st_mode
            or opened.st_uid != before.st_uid
            or opened.st_nlink != before.st_nlink
        ):
            raise BootstrapError(f"{label} changed while it was opened")
        digest = hashlib.sha256()
        size = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            size += len(chunk)
            digest.update(chunk)
        after = os.fstat(descriptor)
        fields = (
            "st_dev",
            "st_ino",
            "st_mode",
            "st_uid",
            "st_nlink",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        if any(getattr(after, field) != getattr(opened, field) for field in fields):
            raise BootstrapError(f"{label} changed while it was hashed")
        if size != opened.st_size:
            raise BootstrapError(f"{label} has inconsistent size metadata")
        return LargeFileSnapshot(
            path=path,
            sha256=digest.hexdigest(),
            device=opened.st_dev,
            inode=opened.st_ino,
            mode=stat.S_IMODE(opened.st_mode),
            owner=opened.st_uid,
            nlink=opened.st_nlink,
            size=opened.st_size,
            mtime_ns=opened.st_mtime_ns,
            ctime_ns=opened.st_ctime_ns,
        )
    finally:
        os.close(descriptor)


def _require_large_file_unchanged(
    snapshot: LargeFileSnapshot, label: str
) -> None:
    if _capture_large_file(snapshot.path, label) != snapshot:
        raise BootstrapError(f"{label} changed after it was sealed")


def _seal_runner_log(
    descriptor: int, path: Path, label: str
) -> LargeFileSnapshot:
    """Flush, make immutable-by-mode, and snapshot a completed runner log."""

    os.fsync(descriptor)
    os.fchmod(descriptor, _DATA_MODE)
    os.fsync(descriptor)
    metadata = os.fstat(descriptor)
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != os.getuid()
        or metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) != _DATA_MODE
    ):
        raise BootstrapError(f"{label} has unsafe final metadata")
    return _capture_large_file(path, label)


def _closed_environment(
    evidence: Path,
    extra_path: list[Path],
    extra_values: dict[str, str] | None = None,
) -> dict[str, str]:
    path_entries: list[str] = [str(evidence)]
    for entry in extra_path:
        rendered = str(entry)
        if rendered not in path_entries:
            path_entries.append(rendered)
    environment = {
        "HOME": str(evidence / "home"),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": os.pathsep.join(path_entries),
        "TMPDIR": str(evidence / "tmp"),
        "TZ": "UTC",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_COUNT": "2",
        "GIT_CONFIG_KEY_0": "core.hooksPath",
        "GIT_CONFIG_VALUE_0": os.devnull,
        "GIT_CONFIG_KEY_1": "core.fsmonitor",
        "GIT_CONFIG_VALUE_1": "false",
        "GIT_TERMINAL_PROMPT": "0",
    }
    if extra_values:
        environment.update(extra_values)
    return environment


def _require_command_resolution(
    name: str,
    expected: Path,
    environment: dict[str, str],
    label: str,
) -> None:
    discovered = shutil.which(name, path=environment["PATH"])
    if discovered is None:
        raise BootstrapError(f"closed PATH does not expose protected {label}")
    try:
        resolved = Path(discovered).resolve(strict=True)
    except OSError as error:
        raise BootstrapError(f"closed PATH has an invalid {label} alias") from error
    if resolved != expected:
        raise BootstrapError(f"closed PATH resolves {name} to an unprotected executable")


def _load_identity(data: bytes) -> dict[str, Any]:
    if len(data) > _MAX_IDENTITY_BYTES:
        raise BootstrapError("candidate identity exceeds its closed size limit")
    try:
        value = json.loads(data)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise BootstrapError("trusted manifest helper returned invalid identity JSON") from error
    if not isinstance(value, dict) or set(value) != _IDENTITY_KEYS:
        raise BootstrapError("trusted manifest helper returned the wrong identity schema")
    if type(value["schema_version"]) is not int or value["schema_version"] != 1:
        raise BootstrapError("candidate identity must use first-release schema 1")
    for key in ("head_commit", "head_tree", "index_tree"):
        if not isinstance(value[key], str) or _OBJECT_ID_RE.fullmatch(value[key]) is None:
            raise BootstrapError(f"candidate identity has invalid {key}")
    for key in ("workspace_source_manifest_sha256", "cargo_lock_sha256"):
        if not isinstance(value[key], str) or _DIGEST_RE.fullmatch(value[key]) is None:
            raise BootstrapError(f"candidate identity has invalid {key}")
    canonical = _canonical_json(value)
    if data != canonical:
        raise BootstrapError("candidate identity is not canonical JSON")
    return value


def _compute_identity(
    python: Path,
    helper: Path,
    candidate: Path,
    environment: dict[str, str],
    timeout_seconds: int,
) -> tuple[bytes, dict[str, Any]]:
    result = _run_bounded(
        python,
        [
            "-I",
            "-S",
            str(helper),
            "--root",
            str(candidate),
            "--release-identity-json",
        ],
        cwd=candidate,
        environment=environment,
        timeout_seconds=timeout_seconds,
        maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES,
    )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", "replace").strip()
        raise BootstrapError(f"trusted manifest helper rejected candidate: {detail}")
    if result.stderr:
        raise BootstrapError("trusted manifest helper emitted unexpected stderr")
    return result.stdout, _load_identity(result.stdout)


def _parse_canonical_json(snapshot: FileSnapshot, label: str) -> dict[str, Any]:
    try:
        value = json.loads(snapshot.data)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise BootstrapError(f"{label} is not valid JSON") from error
    if not isinstance(value, dict) or snapshot.data != _canonical_json(value):
        raise BootstrapError(f"{label} must be one canonical JSON object")
    return value


def _validate_allowed_signers_policy(data: bytes) -> None:
    try:
        text_value = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise BootstrapError("SSH allowed-signers policy must be UTF-8 text") from error
    if "\r" in text_value or "\0" in text_value or not text_value.endswith("\n"):
        raise BootstrapError("SSH allowed-signers policy must be LF-only text")
    active = [
        line
        for line in text_value.splitlines()
        if line and not line.startswith("#")
    ]
    if len(active) != 1:
        raise BootstrapError(
            "SSH allowed-signers file must contain exactly one active key"
        )
    folded = active[0].casefold()
    if "cert-authority" in folded or "-cert-v01@openssh.com" in folded:
        raise BootstrapError(
            "SSH certificate-authority and certificate keys are not accepted in v1"
        )
    if "valid-after=" in folded or "valid-before=" in folded:
        raise BootstrapError(
            "time-bounded SSH allowed-signers policies are not accepted in v1"
        )


def _require_exact_json_fields(
    value: Any, expected: set[str], label: str
) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != expected:
        raise BootstrapError(f"{label} has the wrong schema")
    return value


def _private_directory_snapshot(path: Path, label: str) -> DirectorySnapshot:
    path = _absolute_resolved_existing(path, label)
    try:
        before = path.lstat()
    except OSError as error:
        raise BootstrapError(f"{label} is unavailable") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISDIR(before.st_mode)
        or stat.S_IMODE(before.st_mode) != _DIRECTORY_MODE
        or before.st_uid != os.getuid()
    ):
        raise BootstrapError(f"{label} must be owner-owned with exact mode 0700")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise BootstrapError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        fields = (
            "st_dev",
            "st_ino",
            "st_mode",
            "st_uid",
            "st_nlink",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        if not stat.S_ISDIR(opened.st_mode) or any(
            getattr(opened, field) != getattr(before, field) for field in fields
        ):
            raise BootstrapError(f"{label} changed while it was opened")
        return DirectorySnapshot(
            path=path,
            device=opened.st_dev,
            inode=opened.st_ino,
            mode=stat.S_IMODE(opened.st_mode),
            owner=opened.st_uid,
            nlink=opened.st_nlink,
            mtime_ns=opened.st_mtime_ns,
            ctime_ns=opened.st_ctime_ns,
        )
    finally:
        os.close(descriptor)


def _require_directory_unchanged(snapshot: DirectorySnapshot, label: str) -> None:
    if _private_directory_snapshot(snapshot.path, label) != snapshot:
        raise BootstrapError(f"{label} changed during terminal receipt validation")


def _sealed_directory_snapshot(path: Path, label: str) -> DirectorySnapshot:
    path = _absolute_resolved_existing(path, label)
    before = path.lstat()
    mode = stat.S_IMODE(before.st_mode)
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISDIR(before.st_mode)
        or before.st_uid != os.getuid()
        or mode & 0o222
    ):
        raise BootstrapError(
            f"{label} must be an owner-owned, non-writable sealed directory"
        )
    flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        fields = (
            "st_dev",
            "st_ino",
            "st_mode",
            "st_uid",
            "st_nlink",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        if not stat.S_ISDIR(opened.st_mode) or any(
            getattr(opened, field) != getattr(before, field) for field in fields
        ):
            raise BootstrapError(f"{label} changed while it was opened")
        return DirectorySnapshot(
            path=path,
            device=opened.st_dev,
            inode=opened.st_ino,
            mode=stat.S_IMODE(opened.st_mode),
            owner=opened.st_uid,
            nlink=opened.st_nlink,
            mtime_ns=opened.st_mtime_ns,
            ctime_ns=opened.st_ctime_ns,
        )
    finally:
        os.close(descriptor)


def _require_sealed_directory_unchanged(
    snapshot: DirectorySnapshot, label: str
) -> None:
    if _sealed_directory_snapshot(snapshot.path, label) != snapshot:
        raise BootstrapError(f"{label} changed after sealed-source validation")


def _fsync_sealed_tree(root: Path) -> None:
    """Synchronize retained sealed source files and directories bottom-up."""

    root = _absolute_resolved_existing(root, "retained sealed source")
    directories: list[Path] = []
    for current_text, names, files in os.walk(root, topdown=True, followlinks=False):
        current = Path(current_text)
        directories.append(current)
        for name in [*names, *files]:
            path = current / name
            metadata = path.lstat()
            if stat.S_ISLNK(metadata.st_mode):
                continue
            if stat.S_ISDIR(metadata.st_mode):
                continue
            if not stat.S_ISREG(metadata.st_mode):
                raise BootstrapError("retained sealed source contains a special file")
            flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
            if hasattr(os, "O_NOFOLLOW"):
                flags |= os.O_NOFOLLOW
            descriptor = os.open(path, flags)
            try:
                opened = os.fstat(descriptor)
                if (
                    not stat.S_ISREG(opened.st_mode)
                    or (opened.st_dev, opened.st_ino)
                    != (metadata.st_dev, metadata.st_ino)
                    or opened.st_mode != metadata.st_mode
                    or opened.st_uid != metadata.st_uid
                    or opened.st_size != metadata.st_size
                ):
                    raise BootstrapError(
                        "retained sealed source changed while opened for fsync"
                    )
                os.fsync(descriptor)
                after = os.fstat(descriptor)
                if after != opened:
                    raise BootstrapError(
                        "retained sealed source changed while it was synchronized"
                    )
            finally:
                os.close(descriptor)
    for directory in sorted(
        directories, key=lambda item: (-len(item.parts), str(item))
    ):
        flags = (
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
        )
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        descriptor = os.open(directory, flags)
        try:
            before = os.fstat(descriptor)
            os.fsync(descriptor)
            if os.fstat(descriptor) != before:
                raise BootstrapError(
                    "retained sealed directory changed while it was synchronized"
                )
        finally:
            os.close(descriptor)


def _validate_terminal_receipt(
    *,
    evidence: Path,
    candidate: Path,
    bootstrap_marker: FileSnapshot,
    bootstrap_sha256: str,
    identity_snapshot: FileSnapshot,
    identity: dict[str, Any],
    runner_snapshot: FileSnapshot,
    runner_record: dict[str, Any],
    protected: dict[str, FileSnapshot],
    identity_attestation: dict[str, Any],
    expected_signer_fingerprint: str,
) -> tuple[FileSnapshot, dict[str, Any], list[DirectorySnapshot]]:
    release_runner = evidence / "release-runner"
    output = release_runner / "output"
    release = output / "release"
    directories = [
        _private_directory_snapshot(release_runner, "release-runner directory"),
        _private_directory_snapshot(output, "release output directory"),
        _private_directory_snapshot(release, "terminal receipt directory"),
    ]
    receipt_path = release / "RELEASE_COMPLETED.json"
    receipt_snapshot = _read_file(
        receipt_path,
        "terminal release receipt",
        maximum_bytes=_MAX_TERMINAL_RECEIPT_BYTES,
    )
    if (
        receipt_snapshot.mode != _DATA_MODE
        or receipt_snapshot.owner != os.getuid()
        or receipt_snapshot.nlink != 1
    ):
        raise BootstrapError(
            "terminal release receipt must be owner-owned, single-link, and mode 0400"
        )
    receipt = _parse_canonical_json(receipt_snapshot, "terminal release receipt")
    _require_exact_json_fields(
        receipt,
        {"schema_version", "protocol", "result", "identity", "authentication", "evidence"},
        "terminal release receipt",
    )
    if (
        type(receipt["schema_version"]) is not int
        or receipt["schema_version"] != 1
        or receipt["protocol"] != "sumeragi-v2"
        or receipt["result"] != "release-complete"
    ):
        raise BootstrapError("terminal release receipt does not record release completion")
    receipt_evidence = _require_exact_json_fields(
        receipt["evidence"], _TERMINAL_EVIDENCE_KEYS, "terminal release evidence"
    )
    bootstrap_evidence = _require_exact_json_fields(
        receipt_evidence["bootstrap"],
        {
            "completion",
            "candidate_identity",
            "runner",
            "candidate_cargo_lock",
            "trusted_inputs",
            "identity_verification",
            "runner_tools",
        },
        "terminal bootstrap evidence",
    )
    if (
        not isinstance(bootstrap_evidence["trusted_inputs"], dict)
        or set(bootstrap_evidence["trusted_inputs"]) != set(protected)
        or not isinstance(bootstrap_evidence["identity_verification"], dict)
        or not isinstance(bootstrap_evidence["runner_tools"], dict)
        or set(bootstrap_evidence["runner_tools"])
        != set(runner_record["tools"])
    ):
        raise BootstrapError("terminal bootstrap evidence inventory is not exact")
    for label in (
        "corridor_completion",
        "formal_completion",
        "formal_verus_evidence",
        "formal_verus_log",
        "formal_cross_tool_evidence",
        "seed_matrix_completion",
        "chaos_completion",
        "taira_completion",
    ):
        record = receipt_evidence[label]
        if (
            not isinstance(record, dict)
            or not isinstance(record.get("path"), str)
            or not isinstance(record.get("sha256"), str)
            or _DIGEST_RE.fullmatch(record["sha256"]) is None
        ):
            raise BootstrapError(f"terminal release evidence {label} is malformed")

    receipt_identity = _require_exact_json_fields(
        receipt["identity"],
        {
            "head_commit",
            "head_tree",
            "index_tree",
            "cargo_lock_sha256",
            "candidate_source_manifest_sha256",
            "sealed_source_manifest_sha256",
        },
        "terminal release receipt identity",
    )
    expected_identity = {
        "head_commit": identity["head_commit"],
        "head_tree": identity["head_tree"],
        "index_tree": identity["index_tree"],
        "cargo_lock_sha256": identity["cargo_lock_sha256"],
        "candidate_source_manifest_sha256": identity[
            "workspace_source_manifest_sha256"
        ],
    }
    if any(receipt_identity.get(key) != value for key, value in expected_identity.items()):
        raise BootstrapError("terminal release receipt has the wrong candidate identity")
    if (
        not isinstance(receipt_identity["sealed_source_manifest_sha256"], str)
        or _DIGEST_RE.fullmatch(receipt_identity["sealed_source_manifest_sha256"])
        is None
    ):
        raise BootstrapError("terminal release receipt has an invalid sealed-source digest")

    authentication = _require_exact_json_fields(
        receipt["authentication"],
        {"schema_version", "bootstrap", "release_identity"},
        "terminal release authentication",
    )
    if type(authentication["schema_version"]) is not int or authentication["schema_version"] != 2:
        raise BootstrapError("terminal release authentication has the wrong schema version")
    bootstrap = _require_exact_json_fields(
        authentication["bootstrap"],
        {
            "schema_version",
            "completion_sha256",
            "frozen_bootstrap_sha256",
            "candidate_root",
            "candidate_identity_sha256",
            "candidate_commit_oid",
            "candidate_tree_oid",
            "runner",
            "signer_fingerprint",
            "allowed_signers_principal",
            "trusted_input_digests",
            "trusted_input_sources",
        },
        "terminal release bootstrap authentication",
    )
    if (
        type(bootstrap["schema_version"]) is not int
        or bootstrap["schema_version"] != 1
        or bootstrap["completion_sha256"] != bootstrap_marker.sha256
        or bootstrap["frozen_bootstrap_sha256"] != bootstrap_sha256
        or bootstrap["candidate_root"] != str(candidate)
        or bootstrap["candidate_identity_sha256"] != identity_snapshot.sha256
        or bootstrap["candidate_commit_oid"] != identity["head_commit"]
        or bootstrap["candidate_tree_oid"] != identity["head_tree"]
    ):
        raise BootstrapError("terminal release receipt has the wrong bootstrap binding")
    expected_trusted_digests = {
        label: snapshot.sha256 for label, snapshot in sorted(protected.items())
    }
    if bootstrap["trusted_input_digests"] != expected_trusted_digests:
        raise BootstrapError("terminal release receipt has wrong trusted-input digests")
    expected_trusted_sources = {
        label: {
            "path": str(snapshot.path),
            "sha256": snapshot.sha256,
            "size_bytes": snapshot.size,
            "mode": f"{snapshot.mode:04o}",
            "owner_uid": snapshot.owner,
            "nlink": snapshot.nlink,
        }
        for label, snapshot in sorted(protected.items())
    }
    if bootstrap["trusted_input_sources"] != expected_trusted_sources:
        raise BootstrapError("terminal release receipt has wrong trusted-input sources")
    if bootstrap["signer_fingerprint"] != expected_signer_fingerprint:
        raise BootstrapError("terminal release receipt has the wrong protected signer")
    if runner_snapshot.path != candidate / "scripts" / "run_sumeragi_v2_release_gates.sh":
        raise BootstrapError("terminal release receipt has the wrong runner root binding")
    receipt_runner = _require_exact_json_fields(
        bootstrap["runner"],
        {
            "path",
            "sha256",
            "mode",
            "argv",
            "closed_path_resolution",
            "output",
            "tool_directory",
            "tools",
            "self_digest_environment_variables",
        },
        "terminal release bootstrap runner",
    )
    expected_runner = {
        "path": str(runner_snapshot.path),
        "sha256": runner_snapshot.sha256,
        "mode": f"{runner_snapshot.mode:04o}",
        "argv": runner_record["argv"],
        "closed_path_resolution": runner_record["closed_path_resolution"],
        "output": runner_record["output"],
        "tool_directory": runner_record["tool_directory"],
        "tools": runner_record["tools"],
        "self_digest_environment_variables": runner_record[
            "self_digest_environment_variables"
        ],
    }
    if receipt_runner != expected_runner:
        raise BootstrapError("terminal release receipt has the wrong runner binding")
    release_identity = _require_exact_json_fields(
        authentication["release_identity"],
        {
            "schema_version",
            "signature_format",
            "verification_status",
            "candidate_commit_oid",
            "candidate_tree_oid",
            "signer_fingerprint",
            "primary_key_fingerprint",
            "allowed_signers_principal",
            "release_root",
            "archive_directory",
            "trust_policy",
            "attested_tools",
            "attested_policies",
            "replay",
        },
        "terminal release identity authentication",
    )
    expected_release_identity = {
        "schema_version": 1,
        "signature_format": "ssh",
        "verification_status": "G",
        "candidate_commit_oid": identity["head_commit"],
        "candidate_tree_oid": identity["head_tree"],
        "release_root": str(release_runner / "source"),
        "archive_directory": str(evidence),
        "signer_fingerprint": bootstrap["signer_fingerprint"],
        "allowed_signers_principal": bootstrap["allowed_signers_principal"],
    }
    for field, expected in expected_release_identity.items():
        if (
            field == "schema_version"
            and type(release_identity[field]) is not int
        ) or release_identity[field] != expected:
            raise BootstrapError(
                f"terminal release receipt has the wrong release identity {field}"
            )
    expected_trust_policy = {
        "git_sha256": protected["git"].sha256,
        "ssh_keygen_sha256": protected["ssh_keygen"].sha256,
        "allowed_signers_sha256": protected["allowed_signers"].sha256,
        "revocation_sha256": protected["revocation"].sha256,
        "signer_fingerprint": expected_signer_fingerprint,
    }
    if (
        release_identity["primary_key_fingerprint"] != ""
        or release_identity["trust_policy"] != expected_trust_policy
        or release_identity["attested_tools"] != identity_attestation["tools"]
        or release_identity["attested_policies"] != identity_attestation["policies"]
        or not isinstance(release_identity["replay"], dict)
        or release_identity["replay"].get("performed") is not True
    ):
        raise BootstrapError("terminal release identity trust evidence is not exact")
    return receipt_snapshot, receipt, directories


def _fsync_file_snapshot(snapshot: FileSnapshot, label: str) -> None:
    _require_unchanged(
        snapshot, label, maximum_bytes=max(snapshot.size, 1), executable=False
    )
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(snapshot.path, flags)
    try:
        opened = os.fstat(descriptor)
        if (
            (opened.st_dev, opened.st_ino) != (snapshot.device, snapshot.inode)
            or stat.S_IMODE(opened.st_mode) != snapshot.mode
            or opened.st_uid != snapshot.owner
            or opened.st_nlink != snapshot.nlink
            or opened.st_size != snapshot.size
        ):
            raise BootstrapError(f"{label} changed before fsync")
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    _require_unchanged(
        snapshot, label, maximum_bytes=max(snapshot.size, 1), executable=False
    )


def _validate_retained_source(
    *,
    evidence: Path,
    receipt: dict[str, Any],
    candidate_identity: dict[str, Any],
    python: Path,
    manifest_helper: Path,
    environment: dict[str, str],
    timeout_seconds: int,
) -> tuple[FileSnapshot, dict[str, Any], DirectorySnapshot]:
    sealed_root = evidence / "release-runner" / "source"
    sealed_directory = _sealed_directory_snapshot(
        sealed_root, "retained sealed source root"
    )
    sealed_identity_snapshot = _read_file(
        evidence / "release-runner" / "sealed-identity.json",
        "retained sealed identity",
        maximum_bytes=_MAX_IDENTITY_BYTES,
    )
    if (
        sealed_identity_snapshot.mode != _DATA_MODE
        or sealed_identity_snapshot.owner != os.getuid()
        or sealed_identity_snapshot.nlink != 1
    ):
        raise BootstrapError("retained sealed identity metadata is not exact")
    sealed_identity = _load_identity(sealed_identity_snapshot.data)
    for field in ("head_commit", "head_tree", "index_tree", "cargo_lock_sha256"):
        if sealed_identity[field] != candidate_identity[field]:
            raise BootstrapError(
                f"retained sealed identity disagrees with candidate {field}"
            )
    receipt_identity = receipt["identity"]
    if (
        receipt_identity["sealed_source_manifest_sha256"]
        != sealed_identity["workspace_source_manifest_sha256"]
        or receipt["authentication"]["release_identity"]["release_root"]
        != str(sealed_root)
    ):
        raise BootstrapError("terminal receipt does not bind the retained sealed root")
    recomputed_bytes, recomputed_identity = _compute_identity(
        python, manifest_helper, sealed_root, environment, timeout_seconds
    )
    if (
        recomputed_bytes != sealed_identity_snapshot.data
        or recomputed_identity != sealed_identity
    ):
        raise BootstrapError("retained sealed source does not reproduce its identity")
    _fsync_sealed_tree(sealed_root)
    _fsync_file_snapshot(sealed_identity_snapshot, "retained sealed identity")
    final_bytes, final_identity = _compute_identity(
        python, manifest_helper, sealed_root, environment, timeout_seconds
    )
    if final_bytes != recomputed_bytes or final_identity != recomputed_identity:
        raise BootstrapError("retained sealed source changed during durability closure")
    _require_sealed_directory_unchanged(
        sealed_directory, "retained sealed source root"
    )
    return sealed_identity_snapshot, sealed_identity, sealed_directory


def _receipt_artifact_path(
    receipt: dict[str, Any], label: str, evidence: Path
) -> Path:
    record = receipt["evidence"].get(label)
    if not isinstance(record, dict) or not {"path", "sha256"}.issubset(record):
        raise BootstrapError(f"terminal receipt omits {label}")
    rendered = record["path"]
    if not isinstance(rendered, str):
        raise BootstrapError(f"terminal receipt {label} path is not text")
    path = _absolute_resolved_existing(Path(rendered), f"terminal receipt {label}")
    if not _inside(path, evidence):
        raise BootstrapError(f"terminal receipt {label} escaped bootstrap evidence")
    return path


def _run_protected_receipt_validator(
    *,
    evidence: Path,
    candidate: Path,
    receipt: dict[str, Any],
    receipt_snapshot: FileSnapshot,
    sealed_identity_snapshot: FileSnapshot,
    sealed_root: Path,
    archives: dict[str, FileSnapshot],
    protected: dict[str, FileSnapshot],
    identity_snapshot: FileSnapshot,
    identity_outputs: dict[str, Path],
    bootstrap_marker: FileSnapshot,
    expected_signer_fingerprint: str,
    environment: dict[str, str],
    timeout_seconds: int,
) -> CommandResult:
    arguments = [
        "-I",
        "-S",
        str(archives["receipt_validator"].path),
        "--candidate-identity",
        str(identity_snapshot.path),
        "--sealed-identity",
        str(sealed_identity_snapshot.path),
        "--release-root",
        str(sealed_root),
        "--signature-attestation",
        str(identity_outputs["attestation"]),
        "--signature-transcript",
        str(identity_outputs["transcript"]),
        "--signature-raw-commit",
        str(identity_outputs["raw_commit"]),
        "--signature-cargo-lock",
        str(identity_outputs["cargo_lock"]),
        "--signature-allowed-signers",
        str(identity_outputs["allowed"]),
        "--signature-revocation",
        str(identity_outputs["revocation"]),
        "--signature-git",
        str(identity_outputs["git"]),
        "--signature-ssh-keygen",
        str(identity_outputs["ssh"]),
        "--expected-git-sha256",
        protected["git"].sha256,
        "--expected-ssh-keygen-sha256",
        protected["ssh_keygen"].sha256,
        "--expected-allowed-signers-sha256",
        protected["allowed_signers"].sha256,
        "--expected-revocation-sha256",
        protected["revocation"].sha256,
        "--expected-signer-fingerprint",
        expected_signer_fingerprint,
        "--bootstrap-completion",
        str(bootstrap_marker.path),
        "--bootstrap-evidence-dir",
        str(evidence),
        "--bootstrap-identity",
        str(identity_snapshot.path),
        "--bootstrap-attestation",
        str(identity_outputs["attestation"]),
        "--bootstrap-transcript",
        str(identity_outputs["transcript"]),
        "--expected-bootstrap-completion-sha256",
        bootstrap_marker.sha256,
        "--bootstrap-candidate-root",
        str(candidate),
        "--bootstrap-runner",
        str(candidate / "scripts" / "run_sumeragi_v2_release_gates.sh"),
        "--corridor-completion",
        str(_receipt_artifact_path(receipt, "corridor_completion", evidence)),
        "--formal-completion",
        str(_receipt_artifact_path(receipt, "formal_completion", evidence)),
        "--seed-completion",
        str(_receipt_artifact_path(receipt, "seed_matrix_completion", evidence)),
        "--chaos-completion",
        str(_receipt_artifact_path(receipt, "chaos_completion", evidence)),
        "--taira-completion",
        str(_receipt_artifact_path(receipt, "taira_completion", evidence)),
        "--repository-root",
        str(sealed_root),
        "--output",
        str(receipt_snapshot.path),
        "--verify-existing",
    ]
    result = _run_bounded(
        archives["python"].path,
        arguments,
        cwd=sealed_root,
        environment=environment,
        timeout_seconds=timeout_seconds,
        maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES,
    )
    expected_stdout = (
        f"Sumeragi v2 aggregate release receipt verified: "
        f"{receipt_snapshot.path}\n"
    ).encode()
    if result.returncode != 0 or result.stdout != expected_stdout or result.stderr:
        detail = result.stderr.decode("utf-8", "replace").strip()
        raise BootstrapError(
            f"protected receipt validator rejected terminal receipt: {detail}"
        )
    return result


def _validate_command_record(
    record: Any,
    label: str,
    *,
    require_success: bool,
) -> None:
    expected_keys = {
        "argv",
        "replay_argv",
        "exit_status",
        "stdout_base64",
        "stdout_sha256",
        "stdout_size_bytes",
        "stderr_base64",
        "stderr_sha256",
        "stderr_size_bytes",
    }
    if not isinstance(record, dict) or set(record) != expected_keys:
        raise BootstrapError(f"identity transcript has invalid {label} evidence")
    for key in ("argv", "replay_argv"):
        value = record[key]
        if not isinstance(value, list) or not value or not all(
            isinstance(argument, str) for argument in value
        ):
            raise BootstrapError(f"identity transcript has invalid {label} {key}")
    exit_status = record["exit_status"]
    if type(exit_status) is not int or exit_status < 0:
        raise BootstrapError(f"identity transcript has invalid {label} exit status")
    if require_success and exit_status != 0:
        raise BootstrapError(f"identity transcript records failed {label}")
    for stream in ("stdout", "stderr"):
        encoded = record[f"{stream}_base64"]
        digest = record[f"{stream}_sha256"]
        size = record[f"{stream}_size_bytes"]
        if not isinstance(encoded, str) or not isinstance(digest, str):
            raise BootstrapError(f"identity transcript has invalid {label} {stream}")
        if _DIGEST_RE.fullmatch(digest) is None or type(size) is not int or size < 0:
            raise BootstrapError(f"identity transcript has invalid {label} {stream}")
        try:
            decoded = base64.b64decode(encoded, validate=True)
        except (ValueError, binascii.Error) as error:
            raise BootstrapError(
                f"identity transcript has invalid {label} {stream} encoding"
            ) from error
        if len(decoded) != size or hashlib.sha256(decoded).hexdigest() != digest:
            raise BootstrapError(
                f"identity transcript has inconsistent {label} {stream} evidence"
            )


def _validate_identity_evidence(
    directory: Path,
    identity: dict[str, Any],
    identity_bytes: bytes,
    expected: dict[str, str],
) -> tuple[dict[str, FileSnapshot], dict[str, Any], dict[str, Any]]:
    attestation = _read_file(
        directory / "identity-attestation.json",
        "identity attestation",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    transcript = _read_file(
        directory / "identity-transcript.json",
        "identity transcript",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    attestation_json = _parse_canonical_json(attestation, "identity attestation")
    transcript_json = _parse_canonical_json(transcript, "identity transcript")
    if attestation.mode != _DATA_MODE or transcript.mode != _DATA_MODE:
        raise BootstrapError("identity attestation and transcript must have exact mode 0400")
    if set(attestation_json) != _ATTESTATION_KEYS:
        raise BootstrapError("identity attestation has the wrong schema")
    if set(transcript_json) != _TRANSCRIPT_KEYS:
        raise BootstrapError("identity transcript has the wrong schema")
    if (
        type(attestation_json.get("schema_version")) is not int
        or attestation_json["schema_version"] != 2
    ):
        raise BootstrapError("identity attestation must use schema version 2")
    if (
        type(transcript_json.get("schema_version")) is not int
        or transcript_json["schema_version"] != 2
    ):
        raise BootstrapError("identity transcript must use schema version 2")
    if attestation_json.get("release_identity") != identity:
        raise BootstrapError("identity attestation does not bind the candidate identity")
    if attestation_json.get("release_identity_sha256") != hashlib.sha256(
        identity_bytes
    ).hexdigest():
        raise BootstrapError("identity attestation has the wrong identity digest")
    verification = attestation_json.get("verification")
    if (
        not isinstance(verification, dict)
        or verification.get("signer_fingerprint") != expected["fingerprint"]
    ):
        raise BootstrapError("identity attestation has the wrong signer fingerprint")
    if verification.get("status") != "G":
        raise BootstrapError("identity attestation is not a good SSH signature")
    if verification.get("primary_key_fingerprint") != "":
        raise BootstrapError("identity attestation is not first-release SSH metadata")
    if not isinstance(verification.get("allowed_signers_principal"), str) or not verification.get(
        "allowed_signers_principal"
    ):
        raise BootstrapError("identity attestation omits its allowed-signers principal")
    tools = attestation_json.get("tools")
    if not isinstance(tools, dict):
        raise BootstrapError("identity attestation omits protected tools")
    for key, digest_key in (("git", "git"), ("ssh_keygen", "ssh")):
        item = tools.get(key)
        if not isinstance(item, dict):
            raise BootstrapError(f"identity attestation omits {key}")
        if (
            item.get("observed_sha256") != expected[digest_key]
            or item.get("protected_sha256") != expected[digest_key]
        ):
            raise BootstrapError(f"identity attestation has the wrong {key} digest")
        if item.get("mode") != "0500":
            raise BootstrapError(f"identity attestation has the wrong {key} mode")
        if type(item.get("size_bytes")) is not int or item["size_bytes"] < 0:
            raise BootstrapError(f"identity attestation has invalid {key} size")
    policies = attestation_json.get("policies")
    if not isinstance(policies, dict) or policies.get("signature_format") != "ssh":
        raise BootstrapError("identity attestation does not bind SSH policy")
    if policies.get("expected_signer_fingerprint") != expected["fingerprint"]:
        raise BootstrapError("identity attestation has the wrong protected fingerprint")
    for key, digest_key in (("ssh_allowed_signers", "allowed"), ("ssh_revocation", "revocation")):
        item = policies.get(key)
        if not isinstance(item, dict):
            raise BootstrapError(f"identity attestation omits {key}")
        if (
            item.get("observed_sha256") != expected[digest_key]
            or item.get("protected_sha256") != expected[digest_key]
        ):
            raise BootstrapError(f"identity attestation has the wrong {key} digest")
        if item.get("mode") != "0400":
            raise BootstrapError(f"identity attestation has the wrong {key} mode")
        if type(item.get("size_bytes")) is not int or item["size_bytes"] < 0:
            raise BootstrapError(f"identity attestation has invalid {key} size")
    evidence = attestation_json.get("evidence")
    if not isinstance(evidence, dict) or set(evidence) != _EVIDENCE_KEYS:
        raise BootstrapError("identity attestation has the wrong evidence inventory")
    snapshots: dict[str, FileSnapshot] = {
        "identity_attestation": attestation,
        "identity_transcript": transcript,
    }
    expected_archive_names = {
        "cargo_lock": "identity-Cargo.lock",
        "git": "identity-git",
        "raw_commit": "identity-raw-commit",
        "ssh_allowed_signers": "identity-allowed-signers",
        "ssh_keygen": "identity-ssh-keygen",
        "ssh_revocation": "identity-revocation",
        "verify_transcript": "identity-transcript.json",
    }
    seen_names: set[str] = set()
    for label, record in evidence.items():
        if not isinstance(record, dict):
            raise BootstrapError(f"identity evidence record {label} is invalid")
        name = record.get("archive_name")
        if (
            not isinstance(name, str)
            or not name
            or name in {".", ".."}
            or "/" in name
            or name in seen_names
        ):
            raise BootstrapError(f"identity evidence record {label} has an invalid archive name")
        seen_names.add(name)
        if name != expected_archive_names[label]:
            raise BootstrapError(f"identity evidence {label} has the wrong archive name")
        mode_text = record.get("mode")
        expected_mode = _TOOL_MODE if label in {"git", "ssh_keygen"} else _DATA_MODE
        if mode_text != f"{expected_mode:04o}":
            raise BootstrapError(f"identity evidence {label} has the wrong protected mode")
        digest = record.get("sha256")
        if digest is None:
            digest = record.get("observed_sha256")
        if not isinstance(digest, str) or _DIGEST_RE.fullmatch(digest) is None:
            raise BootstrapError(f"identity evidence {label} has an invalid digest")
        size = record.get("size_bytes")
        if type(size) is not int or size < 0 or size > _MAX_EVIDENCE_BYTES:
            raise BootstrapError(f"identity evidence {label} has an invalid size")
        snapshot = _read_file(
            directory / name,
            f"identity evidence {label}",
            maximum_bytes=_MAX_EVIDENCE_BYTES,
            executable=expected_mode == _TOOL_MODE,
        )
        if (
            snapshot.mode != expected_mode
            or len(snapshot.data) != size
            or snapshot.sha256 != digest
        ):
            raise BootstrapError(f"identity evidence {label} does not match its attestation")
        snapshots[label] = snapshot
    _validate_allowed_signers_policy(snapshots["ssh_allowed_signers"].data)
    transcript_record = evidence["verify_transcript"]
    transcript_digest = transcript_record.get("sha256")
    if transcript_digest is None:
        transcript_digest = transcript_record.get("observed_sha256")
    if (
        transcript_record.get("archive_name") != transcript.path.name
        or transcript_digest != transcript.sha256
    ):
        raise BootstrapError("identity transcript does not match its attested evidence record")
    if transcript_json.get("candidate_commit_oid") != identity["head_commit"]:
        raise BootstrapError("identity transcript has the wrong candidate commit")
    if transcript_json.get("archive_names") != expected_archive_names:
        raise BootstrapError("identity transcript has the wrong replay archive mapping")
    if transcript_json.get("tools") != tools or transcript_json.get("policies") != policies:
        raise BootstrapError("identity transcript disagrees with the attestation")
    commands = transcript_json.get("commands")
    if not isinstance(commands, dict) or set(commands) != {
        "show_signature_metadata",
        "verify_commit",
    }:
        raise BootstrapError("identity transcript has the wrong command inventory")
    _validate_command_record(
        commands["show_signature_metadata"],
        "show-signature command",
        require_success=True,
    )
    _validate_command_record(
        commands["verify_commit"],
        "verify-commit command",
        require_success=True,
    )
    probes = transcript_json.get("tool_probes")
    if not isinstance(probes, dict) or set(probes) != {"ssh_keygen_usage"}:
        raise BootstrapError("identity transcript has the wrong tool-probe inventory")
    _validate_command_record(
        probes["ssh_keygen_usage"],
        "ssh-keygen probe",
        require_success=False,
    )
    if tools["git"]["size_bytes"] != evidence["git"]["size_bytes"]:
        raise BootstrapError("identity Git size disagrees with its evidence")
    if tools["ssh_keygen"]["size_bytes"] != evidence["ssh_keygen"]["size_bytes"]:
        raise BootstrapError("identity ssh-keygen size disagrees with its evidence")
    if (
        policies["ssh_allowed_signers"]["size_bytes"]
        != evidence["ssh_allowed_signers"]["size_bytes"]
    ):
        raise BootstrapError("allowed-signers size disagrees with its evidence")
    if (
        policies["ssh_revocation"]["size_bytes"]
        != evidence["ssh_revocation"]["size_bytes"]
    ):
        raise BootstrapError("SSH revocation size disagrees with its evidence")
    return snapshots, attestation_json, transcript_json


def _artifact_record(source: FileSnapshot, archive: FileSnapshot) -> dict[str, Any]:
    return {
        "archive_name": archive.path.name,
        "archive_mode": f"{archive.mode:04o}",
        "observed_sha256": source.sha256,
        "protected_sha256": source.sha256,
        "size_bytes": len(source.data),
        "source_mode": f"{source.mode:04o}",
        "source_path": str(source.path),
    }


def _protected_size_limit(label: str, executable_labels: set[str]) -> int:
    if label in executable_labels:
        return _MAX_TOOL_BYTES
    if label in {"allowed_signers", "revocation"}:
        return _MAX_POLICY_BYTES
    return _MAX_HELPER_BYTES


def _parse_runner_environment(values: list[str]) -> dict[str, str]:
    result: dict[str, str] = {}
    for value in values:
        name, separator, assigned = value.partition("=")
        if (
            not separator
            or _RUNNER_ENV_RE.fullmatch(name) is None
            or name not in _RUNNER_ENV_ALLOWLIST
        ):
            raise BootstrapError(
                "runner environment entries must use an explicitly allowed NAME=VALUE"
            )
        if name in result or "\0" in assigned:
            raise BootstrapError("runner environment entries must be unique and NUL-free")
        result[name] = assigned
    return result


def _require_nonwritable_ancestors(path: Path, label: str) -> None:
    for ancestor in (path.parent, *path.parent.parents):
        metadata = ancestor.lstat()
        if (
            stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid not in {0, os.getuid()}
            or stat.S_IMODE(metadata.st_mode) & 0o022
        ):
            raise BootstrapError(
                f"{label} has a writable, symlinked, or untrusted ancestor"
            )


def _load_runner_tool_manifest(
    snapshot: FileSnapshot, candidate: Path
) -> dict[str, FileSnapshot]:
    manifest = _parse_canonical_json(snapshot, "runner tool manifest")
    _require_exact_json_fields(
        manifest, {"schema_version", "tools"}, "runner tool manifest"
    )
    tools = manifest["tools"]
    if (
        type(manifest["schema_version"]) is not int
        or manifest["schema_version"] != 1
        or not isinstance(tools, dict)
        or not tools
        or len(tools) > _MAX_RUNNER_TOOLS
    ):
        raise BootstrapError("runner tool manifest has an invalid first-release schema")
    reserved = {"bash", "git", "python3", "ssh-keygen"}
    snapshots: dict[str, FileSnapshot] = {}
    inodes: set[tuple[int, int]] = set()
    for name in sorted(tools):
        if (
            not isinstance(name, str)
            or _RUNNER_TOOL_NAME_RE.fullmatch(name) is None
            or name in reserved
            or os.pathsep in name
        ):
            raise BootstrapError("runner tool manifest has an unsafe alias")
        record = _require_exact_json_fields(
            tools[name], {"path", "sha256"}, f"runner tool {name}"
        )
        if not isinstance(record["path"], str):
            raise BootstrapError(f"runner tool {name} path is not text")
        source = _protected_snapshot(
            Path(record["path"]),
            _require_digest(record["sha256"], f"runner tool {name} digest"),
            f"runner tool {name}",
            candidate=candidate,
            maximum_bytes=_MAX_TOOL_BYTES,
            executable=True,
        )
        if (
            source.owner not in {0, os.getuid()}
            or source.mode & 0o022
            or os.pathsep in str(source.path)
        ):
            raise BootstrapError(f"runner tool {name} source is writable or untrusted")
        _require_nonwritable_ancestors(source.path, f"runner tool {name}")
        inode = (source.device, source.inode)
        if inode in inodes:
            raise BootstrapError("runner tool manifest contains an executable inode alias")
        inodes.add(inode)
        snapshots[name] = source
    return snapshots


def _runner_tool_record(
    name: str, source: FileSnapshot, alias: SymlinkSnapshot
) -> dict[str, Any]:
    return {
        "alias_name": name,
        "alias_path": str(alias.path),
        "sha256": source.sha256,
        "size_bytes": source.size,
        "source_mode": f"{source.mode:04o}",
        "source_path": str(source.path),
    }


def _runner_alias_snapshot(path: Path, target: Path, label: str) -> SymlinkSnapshot:
    metadata = path.lstat()
    if (
        not stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != os.getuid()
        or metadata.st_nlink != 1
        or os.readlink(path) != str(target)
        or path.resolve(strict=True) != target
    ):
        raise BootstrapError(f"{label} is not one exact protected symlink alias")
    return SymlinkSnapshot(
        path=path,
        target=str(target),
        device=metadata.st_dev,
        inode=metadata.st_ino,
        mode=stat.S_IMODE(metadata.st_mode),
        owner=metadata.st_uid,
        nlink=metadata.st_nlink,
        mtime_ns=metadata.st_mtime_ns,
        ctime_ns=metadata.st_ctime_ns,
    )


def _revalidate_runner_tools(
    sources: dict[str, FileSnapshot], aliases: dict[str, SymlinkSnapshot]
) -> None:
    if set(sources) != set(aliases):
        raise BootstrapError("runner tool alias inventory changed")
    for name in sorted(sources):
        _require_unchanged(
            sources[name],
            f"runner tool {name}",
            maximum_bytes=_MAX_TOOL_BYTES,
            executable=True,
        )
        current_alias = _runner_alias_snapshot(
            aliases[name].path, sources[name].path, f"runner tool alias {name}"
        )
        if current_alias != aliases[name]:
            raise BootstrapError(f"runner tool alias {name} changed")


def _cleanup(path: Path) -> None:
    stack = [path]
    while stack:
        target = stack.pop()
        try:
            metadata = target.lstat()
        except OSError:
            continue
        if (
            stat.S_ISDIR(metadata.st_mode)
            and not stat.S_ISLNK(metadata.st_mode)
            and metadata.st_uid == os.getuid()
        ):
            try:
                os.chmod(target, stat.S_IMODE(metadata.st_mode) | 0o700)
            except OSError:
                continue
            try:
                with os.scandir(target) as entries:
                    for entry in entries:
                        try:
                            if entry.is_dir(follow_symlinks=False):
                                stack.append(Path(entry.path))
                        except OSError:
                            continue
            except OSError:
                continue

    try:
        shutil.rmtree(path)
    except FileNotFoundError:
        return
    except OSError as error:
        print(f"warning: could not remove failed bootstrap evidence: {error}", file=sys.stderr)


def bootstrap(args: argparse.Namespace) -> int:
    if not sys.flags.isolated or not sys.flags.no_site:
        raise BootstrapError(
            "bootstrap must be started by protected Python with both -I and -S"
        )
    candidate = _absolute_resolved_existing(args.candidate_root, "candidate root")
    if not candidate.is_dir():
        raise BootstrapError("candidate root must be a directory")
    if _SAFE_PATH_RE.fullmatch(str(candidate)) is None:
        raise BootstrapError("candidate root must use the shell-safe release path alphabet")
    bootstrap_path = _absolute_resolved_existing(Path(__file__), "release bootstrap")
    if _inside(bootstrap_path, candidate):
        raise BootstrapError("release bootstrap must be installed outside the candidate root")

    protected_specs = (
        ("bootstrap", bootstrap_path, args.expected_bootstrap_sha256, _MAX_HELPER_BYTES, False),
        ("python", args.python_bin, args.expected_python_sha256, _MAX_TOOL_BYTES, True),
        ("git", args.git_bin, args.expected_git_sha256, _MAX_TOOL_BYTES, True),
        ("ssh_keygen", args.ssh_keygen_bin, args.expected_ssh_keygen_sha256, _MAX_TOOL_BYTES, True),
        ("bash", args.bash_bin, args.expected_bash_sha256, _MAX_TOOL_BYTES, True),
        (
            "manifest_helper",
            args.manifest_helper,
            args.expected_manifest_helper_sha256,
            _MAX_HELPER_BYTES,
            False,
        ),
        (
            "identity_verifier",
            args.identity_verifier,
            args.expected_identity_verifier_sha256,
            _MAX_HELPER_BYTES,
            False,
        ),
        (
            "receipt_validator",
            args.receipt_validator,
            args.expected_receipt_validator_sha256,
            _MAX_HELPER_BYTES,
            False,
        ),
        (
            "runner_tool_manifest",
            args.runner_tool_manifest,
            args.expected_runner_tool_manifest_sha256,
            _MAX_HELPER_BYTES,
            False,
        ),
        (
            "allowed_signers",
            args.ssh_allowed_signers,
            args.expected_ssh_allowed_signers_sha256,
            _MAX_POLICY_BYTES,
            False,
        ),
        (
            "revocation",
            args.ssh_revocation_file,
            args.expected_ssh_revocation_sha256,
            _MAX_POLICY_BYTES,
            False,
        ),
    )
    protected: dict[str, FileSnapshot] = {}
    executable_labels = {"python", "git", "ssh_keygen", "bash"}
    for label, path, digest, maximum, executable in protected_specs:
        protected[label] = _protected_snapshot(
            path,
            digest,
            label.replace("_", " "),
            candidate=candidate,
            maximum_bytes=maximum,
            executable=executable,
        )
    if protected["python"].path != Path(sys.executable).resolve(strict=True):
        raise BootstrapError("bootstrap must already be running under the protected Python")
    if not protected["allowed_signers"].data:
        raise BootstrapError("SSH allowed-signers policy must not be empty")
    if _FINGERPRINT_RE.fullmatch(args.expected_signer_fingerprint) is None:
        raise BootstrapError("expected signer fingerprint is invalid")

    runner_path = candidate / "scripts" / "run_sumeragi_v2_release_gates.sh"
    runner_snapshot = _read_file(
        runner_path,
        "signed candidate release runner",
        maximum_bytes=_MAX_HELPER_BYTES,
    )
    if not _inside(runner_snapshot.path, candidate):
        raise BootstrapError("candidate release runner escaped the candidate root")
    runner_tool_sources = _load_runner_tool_manifest(
        protected["runner_tool_manifest"], candidate
    )
    runner_extra_environment = _parse_runner_environment(args.runner_environment)

    evidence, evidence_fd = _prepare_evidence_directory(args.evidence_dir, candidate)
    evidence_directory_stat = os.fstat(evidence_fd)
    success = False
    runner_started = False
    runner_finished = False
    runner_stdout_descriptor: int | None = None
    runner_stderr_descriptor: int | None = None
    runner_logs: dict[str, LargeFileSnapshot] = {}
    try:
        for child in ("home", "tmp"):
            os.mkdir(child, _DIRECTORY_MODE, dir_fd=evidence_fd)
        os.mkdir("runner-bin", _DIRECTORY_MODE, dir_fd=evidence_fd)
        os.fsync(evidence_fd)
        runner_stdout_path = evidence / "runner-stdout.log"
        runner_stderr_path = evidence / "runner-stderr.log"
        runner_stdout_descriptor = _open_runner_log(
            evidence_fd, runner_stdout_path.name
        )
        runner_stderr_descriptor = _open_runner_log(
            evidence_fd, runner_stderr_path.name
        )
        archive_names = {
            "bootstrap": "trusted-bootstrap.py",
            "python": "python3",
            "git": "git",
            "ssh_keygen": "ssh-keygen",
            "bash": "bash",
            "manifest_helper": "compute-manifest.py",
            "identity_verifier": "verify-identity.py",
            "receipt_validator": "validate-receipt.py",
            "runner_tool_manifest": "runner-tool-manifest.json",
            "allowed_signers": "bootstrap-allowed-signers",
            "revocation": "bootstrap-revocation",
        }
        archives: dict[str, FileSnapshot] = {}
        for label, source in protected.items():
            mode = _TOOL_MODE if label in executable_labels else _DATA_MODE
            archives[label] = _write_artifact(
                evidence, evidence_fd, archive_names[label], source.data, mode
            )

        runner_bin = evidence / "runner-bin"
        runner_bin_flags = (
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
        )
        if hasattr(os, "O_NOFOLLOW"):
            runner_bin_flags |= os.O_NOFOLLOW
        runner_bin_fd = os.open(runner_bin, runner_bin_flags)
        try:
            runner_tool_aliases: dict[str, SymlinkSnapshot] = {}
            for name, source in sorted(runner_tool_sources.items()):
                os.symlink(str(source.path), name, dir_fd=runner_bin_fd)
                os.fsync(runner_bin_fd)
                runner_tool_aliases[name] = _runner_alias_snapshot(
                    runner_bin / name, source.path, f"runner tool alias {name}"
                )
        finally:
            os.close(runner_bin_fd)

        environment = _closed_environment(
            evidence,
            [runner_bin],
        )
        _require_command_resolution(
            "git", archives["git"].path, environment, "archived Git"
        )
        _require_command_resolution(
            "python3", archives["python"].path, environment, "archived Python"
        )
        _require_command_resolution(
            "bash", archives["bash"].path, environment, "archived Bash"
        )
        python_probe = _run_bounded(
            archives["python"].path,
            ["-I", "-S", "-c", "raise SystemExit(0)"],
            cwd=evidence,
            environment=environment,
            timeout_seconds=args.command_timeout_seconds,
            maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES,
        )
        if python_probe.returncode != 0 or python_probe.stdout or python_probe.stderr:
            raise BootstrapError("archived protected Python is not relocatable")
        bash_probe = _run_bounded(
            archives["bash"].path,
            ["-c", ":"],
            cwd=evidence,
            environment=environment,
            timeout_seconds=args.command_timeout_seconds,
            maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES,
        )
        if bash_probe.returncode != 0 or bash_probe.stdout or bash_probe.stderr:
            raise BootstrapError("archived protected Bash is not relocatable")
        identity_bytes, identity = _compute_identity(
            archives["python"].path,
            archives["manifest_helper"].path,
            candidate,
            environment,
            args.command_timeout_seconds,
        )
        identity_snapshot = _write_artifact(
            evidence, evidence_fd, "candidate-identity.json", identity_bytes, _DATA_MODE
        )

        identity_outputs = {
            "attestation": evidence / "identity-attestation.json",
            "transcript": evidence / "identity-transcript.json",
            "raw_commit": evidence / "identity-raw-commit",
            "cargo_lock": evidence / "identity-Cargo.lock",
            "allowed": evidence / "identity-allowed-signers",
            "revocation": evidence / "identity-revocation",
            "git": evidence / "identity-git",
            "ssh": evidence / "identity-ssh-keygen",
        }
        verifier_arguments = [
            "-I",
            "-S",
            str(archives["identity_verifier"].path),
            "--root", str(candidate),
            "--identity", str(identity_snapshot.path),
            "--git-bin", str(archives["git"].path),
            "--expected-git-sha256", protected["git"].sha256,
            "--ssh-keygen-bin", str(archives["ssh_keygen"].path),
            "--expected-ssh-keygen-sha256", protected["ssh_keygen"].sha256,
            "--expected-signer-fingerprint", args.expected_signer_fingerprint,
            "--ssh-allowed-signers", str(archives["allowed_signers"].path),
            "--expected-ssh-allowed-signers-sha256", protected["allowed_signers"].sha256,
            "--ssh-revocation-file", str(archives["revocation"].path),
            "--expected-ssh-revocation-sha256", protected["revocation"].sha256,
            "--attestation-output", str(identity_outputs["attestation"]),
            "--verify-transcript-output", str(identity_outputs["transcript"]),
            "--raw-commit-output", str(identity_outputs["raw_commit"]),
            "--cargo-lock-output", str(identity_outputs["cargo_lock"]),
            "--ssh-allowed-signers-output", str(identity_outputs["allowed"]),
            "--ssh-revocation-output", str(identity_outputs["revocation"]),
            "--git-archive-output", str(identity_outputs["git"]),
            "--ssh-keygen-archive-output", str(identity_outputs["ssh"]),
        ]
        verifier = _run_bounded(
            archives["python"].path,
            verifier_arguments,
            cwd=evidence,
            environment=environment,
            timeout_seconds=args.command_timeout_seconds,
            maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES,
        )
        if verifier.returncode != 0:
            detail = verifier.stderr.decode("utf-8", "replace").strip()
            raise BootstrapError(f"trusted identity verifier rejected candidate: {detail}")
        if verifier.stdout or verifier.stderr:
            raise BootstrapError("trusted identity verifier emitted unexpected output")

        for label, snapshot in protected.items():
            maximum = _protected_size_limit(label, executable_labels)
            _require_unchanged(
                snapshot,
                label.replace("_", " "),
                maximum_bytes=maximum,
                executable=label in executable_labels,
            )
        for label, snapshot in archives.items():
            maximum = _protected_size_limit(label, executable_labels)
            _require_unchanged(
                snapshot,
                f"archived {label.replace('_', ' ')}",
                maximum_bytes=maximum,
                executable=label in executable_labels,
            )
        _revalidate_runner_tools(runner_tool_sources, runner_tool_aliases)
        evidence_snapshots, identity_attestation, identity_transcript = (
            _validate_identity_evidence(
            evidence,
            identity,
            identity_bytes,
            {
                "git": protected["git"].sha256,
                "ssh": protected["ssh_keygen"].sha256,
                "allowed": protected["allowed_signers"].sha256,
                "revocation": protected["revocation"].sha256,
                "fingerprint": args.expected_signer_fingerprint,
            },
            )
        )
        recomputed_bytes, recomputed_identity = _compute_identity(
            archives["python"].path,
            archives["manifest_helper"].path,
            candidate,
            environment,
            args.command_timeout_seconds,
        )
        if recomputed_bytes != identity_bytes or recomputed_identity != identity:
            raise BootstrapError("candidate identity changed after authentication")
        _require_unchanged(
            runner_snapshot,
            "signed candidate release runner",
            maximum_bytes=_MAX_HELPER_BYTES,
        )

        completion_path = evidence / "BOOTSTRAP_COMPLETED.json"
        policy_environment_without_self_digest = {
            "SUMERAGI_V2_RELEASE_SSH_KEYGEN_BIN": str(archives["ssh_keygen"].path),
            "SUMERAGI_V2_RELEASE_EXPECTED_GIT_SHA256": protected["git"].sha256,
            "SUMERAGI_V2_RELEASE_EXPECTED_SSH_KEYGEN_SHA256": protected[
                "ssh_keygen"
            ].sha256,
            "SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT": (
                args.expected_signer_fingerprint
            ),
            "SUMERAGI_V2_RELEASE_SSH_ALLOWED_SIGNERS": str(
                archives["allowed_signers"].path
            ),
            "SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256": (
                protected["allowed_signers"].sha256
            ),
            "SUMERAGI_V2_RELEASE_SSH_REVOCATION_FILE": str(
                archives["revocation"].path
            ),
            "SUMERAGI_V2_RELEASE_EXPECTED_SSH_REVOCATION_SHA256": protected[
                "revocation"
            ].sha256,
            "SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION": str(completion_path),
            "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION": str(
                identity_outputs["attestation"]
            ),
            "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT": str(
                identity_outputs["transcript"]
            ),
            "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY": str(identity_snapshot.path),
            "SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR": str(evidence),
        }
        alias_environment_without_self_digest = {
            key.replace("SUMERAGI_V2_RELEASE_", "IROHA_RELEASE_", 1): value
            for key, value in policy_environment_without_self_digest.items()
            if key.startswith("SUMERAGI_V2_RELEASE_BOOTSTRAP_")
        }
        runner_environment_without_self_digest = _closed_environment(
            evidence,
            [runner_bin],
            {
                **runner_extra_environment,
                **policy_environment_without_self_digest,
                **alias_environment_without_self_digest,
            },
        )
        self_digest_variables = [
            "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
            "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
        ]

        marker_value = {
            "schema_version": 1,
            "trust_boundary": {
                "bootstrap_authentication": "external prerequisite",
                "release_image_and_dynamic_loader": "external prerequisite",
                "same_uid_and_trusted_ancestor_owners": True,
            },
            "candidate_root": str(candidate),
            "candidate_identity": identity,
            "candidate_identity_sha256": identity_snapshot.sha256,
            "trusted_inputs": {
                label: _artifact_record(protected[label], archives[label])
                for label in sorted(protected)
            },
            "identity_verification": {
                label: {
                    "archive_name": snapshot.path.name,
                    "mode": f"{snapshot.mode:04o}",
                    "sha256": snapshot.sha256,
                    "size_bytes": len(snapshot.data),
                }
                for label, snapshot in sorted(evidence_snapshots.items())
            },
            "runner": {
                "argv": [str(archives["bash"].path), str(runner_path), "--release"],
                "closed_path_resolution": {
                    "bash": str(archives["bash"].path),
                    "git": str(archives["git"].path),
                    "python3": str(archives["python"].path),
                },
                "environment_without_self_digest": (
                    runner_environment_without_self_digest
                ),
                "mode": f"{runner_snapshot.mode:04o}",
                "output": {
                    "stderr_path": str(runner_stderr_path),
                    "stdout_path": str(runner_stdout_path),
                    "active_mode": "0600",
                    "sealed_mode": "0400",
                },
                "path": str(runner_path),
                "tool_directory": str(runner_bin),
                "tools": {
                    name: _runner_tool_record(
                        name, runner_tool_sources[name], runner_tool_aliases[name]
                    )
                    for name in sorted(runner_tool_sources)
                },
                "self_digest_environment_variables": self_digest_variables,
                "sha256": runner_snapshot.sha256,
                "size_bytes": len(runner_snapshot.data),
            },
            "trusted_execution_probes": {
                "bash": {
                    "argv": [str(archives["bash"].path), "-c", ":"],
                    "exit_status": bash_probe.returncode,
                },
                "python": {
                    "argv": [
                        str(archives["python"].path),
                        "-I",
                        "-S",
                        "-c",
                        "raise SystemExit(0)",
                    ],
                    "exit_status": python_probe.returncode,
                },
            },
        }
        marker = _publish_completion_marker(
            evidence,
            evidence_fd,
            _canonical_json(marker_value),
        )
        runner_environment = {
            **runner_environment_without_self_digest,
            self_digest_variables[0]: marker.sha256,
            self_digest_variables[1]: marker.sha256,
        }
        runner_started = True
        try:
            assert runner_stdout_descriptor is not None
            assert runner_stderr_descriptor is not None
            runner = _run_release_runner(
                archives["bash"].path,
                [str(runner_path), "--release"],
                cwd=candidate,
                environment=runner_environment,
                stdout_descriptor=runner_stdout_descriptor,
                stderr_descriptor=runner_stderr_descriptor,
            )
        except RunnerLaunchError:
            runner_started = False
            raise
        runner_finished = True
        runner_status = runner.returncode if runner.returncode >= 0 else 128 - runner.returncode
        runner_logs = {
            "stdout": _seal_runner_log(
                runner_stdout_descriptor,
                runner_stdout_path,
                "release runner stdout log",
            ),
            "stderr": _seal_runner_log(
                runner_stderr_descriptor,
                runner_stderr_path,
                "release runner stderr log",
            ),
        }
        os.close(runner_stdout_descriptor)
        runner_stdout_descriptor = None
        os.close(runner_stderr_descriptor)
        runner_stderr_descriptor = None
        os.fsync(evidence_fd)

        post_error: BootstrapError | None = None
        try:
            for label, snapshot in protected.items():
                maximum = _protected_size_limit(label, executable_labels)
                _require_unchanged(
                    snapshot,
                    label.replace("_", " "),
                    maximum_bytes=maximum,
                    executable=label in executable_labels,
                )
            for label, snapshot in archives.items():
                maximum = _protected_size_limit(label, executable_labels)
                _require_unchanged(
                    snapshot,
                    f"archived {label.replace('_', ' ')}",
                    maximum_bytes=maximum,
                    executable=label in executable_labels,
                )
            _revalidate_runner_tools(runner_tool_sources, runner_tool_aliases)
            _require_unchanged(
                identity_snapshot,
                "candidate identity evidence",
                maximum_bytes=_MAX_IDENTITY_BYTES,
            )
            for label, snapshot in evidence_snapshots.items():
                _require_unchanged(
                    snapshot,
                    f"identity evidence {label}",
                    maximum_bytes=_MAX_EVIDENCE_BYTES,
                    executable=snapshot.mode == _TOOL_MODE,
                )
            _require_unchanged(
                marker,
                "bootstrap completion marker",
                maximum_bytes=_MAX_EVIDENCE_BYTES,
            )
            _require_unchanged(
                runner_snapshot,
                "signed candidate release runner",
                maximum_bytes=_MAX_HELPER_BYTES,
            )
            final_bytes, final_identity = _compute_identity(
                archives["python"].path,
                archives["manifest_helper"].path,
                candidate,
                environment,
                args.command_timeout_seconds,
            )
            if final_bytes != identity_bytes or final_identity != identity:
                raise BootstrapError("candidate identity changed while the signed runner executed")
            directory_stat = os.fstat(evidence_fd)
            pathname_stat = evidence.lstat()
            if (
                not stat.S_ISDIR(pathname_stat.st_mode)
                or (directory_stat.st_dev, directory_stat.st_ino)
                != (evidence_directory_stat.st_dev, evidence_directory_stat.st_ino)
                or (pathname_stat.st_dev, pathname_stat.st_ino)
                != (evidence_directory_stat.st_dev, evidence_directory_stat.st_ino)
                or stat.S_IMODE(directory_stat.st_mode) != _DIRECTORY_MODE
                or stat.S_IMODE(pathname_stat.st_mode) != _DIRECTORY_MODE
                or directory_stat.st_uid != os.getuid()
                or pathname_stat.st_uid != os.getuid()
            ):
                raise BootstrapError(
                    "bootstrap evidence directory changed while the runner executed"
                )
        except BootstrapError as error:
            post_error = error
        except OSError as error:
            post_error = BootstrapError(
                f"post-run bootstrap evidence became unavailable: {error}"
            )

        if runner_status != 0:
            if post_error is not None:
                print(f"post-run bootstrap validation also failed: {post_error}", file=sys.stderr)
            return runner_status
        if post_error is not None:
            raise post_error

        terminal_receipt, terminal_receipt_value, terminal_directories = (
            _validate_terminal_receipt(
            evidence=evidence,
            candidate=candidate,
            bootstrap_marker=marker,
            bootstrap_sha256=protected["bootstrap"].sha256,
            identity_snapshot=identity_snapshot,
            identity=identity,
            runner_snapshot=runner_snapshot,
            runner_record=marker_value["runner"],
            protected=protected,
            identity_attestation=identity_attestation,
            expected_signer_fingerprint=args.expected_signer_fingerprint,
            )
        )
        _require_unchanged(
            terminal_receipt,
            "terminal release receipt",
            maximum_bytes=_MAX_TERMINAL_RECEIPT_BYTES,
        )
        for index, directory in enumerate(terminal_directories):
            _require_directory_unchanged(
                directory, f"terminal release directory {index}"
            )
        sealed_identity_snapshot, sealed_identity, sealed_directory = (
            _validate_retained_source(
                evidence=evidence,
                receipt=terminal_receipt_value,
                candidate_identity=identity,
                python=archives["python"].path,
                manifest_helper=archives["manifest_helper"].path,
                environment=runner_environment,
                timeout_seconds=args.command_timeout_seconds,
            )
        )
        receipt_validation = _run_protected_receipt_validator(
            evidence=evidence,
            candidate=candidate,
            receipt=terminal_receipt_value,
            receipt_snapshot=terminal_receipt,
            sealed_identity_snapshot=sealed_identity_snapshot,
            sealed_root=sealed_directory.path,
            archives=archives,
            protected=protected,
            identity_snapshot=identity_snapshot,
            identity_outputs=identity_outputs,
            bootstrap_marker=marker,
            expected_signer_fingerprint=args.expected_signer_fingerprint,
            environment=runner_environment,
            timeout_seconds=args.command_timeout_seconds,
        )
        _require_unchanged(
            terminal_receipt,
            "protected-validator terminal release receipt",
            maximum_bytes=_MAX_TERMINAL_RECEIPT_BYTES,
        )
        _require_unchanged(
            sealed_identity_snapshot,
            "protected-validator sealed identity",
            maximum_bytes=_MAX_IDENTITY_BYTES,
        )
        _require_sealed_directory_unchanged(
            sealed_directory, "protected-validator retained sealed source"
        )
        release_completion_value = {
            "schema_version": 1,
            "result": "release-complete",
            "bootstrap_completion_sha256": marker.sha256,
            "candidate_root": str(candidate),
            "candidate_identity_sha256": identity_snapshot.sha256,
            "candidate_commit_oid": identity["head_commit"],
            "candidate_tree_oid": identity["head_tree"],
            "runner": {
                "path": str(runner_snapshot.path),
                "sha256": runner_snapshot.sha256,
                "mode": f"{runner_snapshot.mode:04o}",
                "logs": {
                    label: {
                        "path": str(snapshot.path),
                        "sha256": snapshot.sha256,
                        "size_bytes": snapshot.size,
                        "mode": f"{snapshot.mode:04o}",
                    }
                    for label, snapshot in sorted(runner_logs.items())
                },
            },
            "retained_source": {
                "path": str(sealed_directory.path),
                "identity_path": str(sealed_identity_snapshot.path),
                "identity_sha256": sealed_identity_snapshot.sha256,
                "source_manifest_sha256": sealed_identity[
                    "workspace_source_manifest_sha256"
                ],
                "mode": f"{sealed_directory.mode:04o}",
            },
            "receipt_validator": {
                "archive_path": str(archives["receipt_validator"].path),
                "sha256": protected["receipt_validator"].sha256,
                "exit_status": receipt_validation.returncode,
            },
            "terminal_receipt": {
                "path": str(terminal_receipt.path),
                "sha256": terminal_receipt.sha256,
                "size_bytes": terminal_receipt.size,
                "mode": f"{terminal_receipt.mode:04o}",
            },
        }
        release_completion = _publish_completion_marker(
            evidence,
            evidence_fd,
            _canonical_json(release_completion_value),
            final_name="BOOTSTRAP_RELEASE_COMPLETED.json",
        )
        if (
            release_completion.mode != _DATA_MODE
            or release_completion.owner != os.getuid()
            or release_completion.nlink != 1
        ):
            raise BootstrapError("external release completion marker metadata is not exact")

        # Close the publication window: success is returned only if both the
        # receipt and every trust input still match the snapshots that produced
        # the external no-clobber marker.
        _require_unchanged(
            terminal_receipt,
            "terminal release receipt",
            maximum_bytes=_MAX_TERMINAL_RECEIPT_BYTES,
        )
        for index, directory in enumerate(terminal_directories):
            _require_directory_unchanged(
                directory, f"terminal release directory {index}"
            )
        for label, snapshot in protected.items():
            maximum = _protected_size_limit(label, executable_labels)
            _require_unchanged(
                snapshot,
                label.replace("_", " "),
                maximum_bytes=maximum,
                executable=label in executable_labels,
            )
        for label, snapshot in archives.items():
            maximum = _protected_size_limit(label, executable_labels)
            _require_unchanged(
                snapshot,
                f"archived {label.replace('_', ' ')}",
                maximum_bytes=maximum,
                executable=label in executable_labels,
            )
        _revalidate_runner_tools(runner_tool_sources, runner_tool_aliases)
        _require_unchanged(
            identity_snapshot,
            "candidate identity evidence",
            maximum_bytes=_MAX_IDENTITY_BYTES,
        )
        for label, snapshot in evidence_snapshots.items():
            _require_unchanged(
                snapshot,
                f"identity evidence {label}",
                maximum_bytes=_MAX_EVIDENCE_BYTES,
                executable=snapshot.mode == _TOOL_MODE,
            )
        _require_unchanged(
            marker,
            "bootstrap completion marker",
            maximum_bytes=_MAX_EVIDENCE_BYTES,
        )
        _require_unchanged(
            runner_snapshot,
            "signed candidate release runner",
            maximum_bytes=_MAX_HELPER_BYTES,
        )
        for label, snapshot in runner_logs.items():
            _require_large_file_unchanged(
                snapshot, f"release runner {label} log"
            )
        _require_unchanged(
            sealed_identity_snapshot,
            "retained sealed identity",
            maximum_bytes=_MAX_IDENTITY_BYTES,
        )
        _require_sealed_directory_unchanged(
            sealed_directory, "retained sealed source root"
        )
        final_bytes, final_identity = _compute_identity(
            archives["python"].path,
            archives["manifest_helper"].path,
            candidate,
            environment,
            args.command_timeout_seconds,
        )
        if final_bytes != identity_bytes or final_identity != identity:
            raise BootstrapError(
                "candidate identity changed during external completion publication"
            )
        success = True
        try:
            print(
                "Sumeragi v2 external release completion: "
                f"{release_completion.path} sha256={release_completion.sha256}",
                file=sys.stderr,
            )
        except OSError:
            # The no-clobber marker is the authoritative result; a detached or
            # closed diagnostic stream must not turn durable success into an
            # ambiguous failed invocation.
            pass
        return 0
    finally:
        for descriptor in (runner_stdout_descriptor, runner_stderr_descriptor):
            if descriptor is not None:
                try:
                    os.close(descriptor)
                except OSError:
                    pass
        try:
            os.close(evidence_fd)
        except OSError:
            if success:
                raise BootstrapError("could not close successful bootstrap evidence")
        if not success and runner_started and not runner_finished:
            print(
                "warning: preserving bootstrap evidence because the release runner may still be active",
                file=sys.stderr,
            )
        elif not success:
            _cleanup(evidence)


def _positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be positive")
    return parsed


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-root", type=Path, required=True)
    parser.add_argument("--evidence-dir", type=Path, required=True)
    parser.add_argument("--expected-bootstrap-sha256", required=True)
    parser.add_argument("--python-bin", type=Path, required=True)
    parser.add_argument("--expected-python-sha256", required=True)
    parser.add_argument("--git-bin", type=Path, required=True)
    parser.add_argument("--expected-git-sha256", required=True)
    parser.add_argument("--ssh-keygen-bin", type=Path, required=True)
    parser.add_argument("--expected-ssh-keygen-sha256", required=True)
    parser.add_argument("--manifest-helper", type=Path, required=True)
    parser.add_argument("--expected-manifest-helper-sha256", required=True)
    parser.add_argument("--identity-verifier", type=Path, required=True)
    parser.add_argument("--expected-identity-verifier-sha256", required=True)
    parser.add_argument("--receipt-validator", type=Path, required=True)
    parser.add_argument("--expected-receipt-validator-sha256", required=True)
    parser.add_argument("--runner-tool-manifest", type=Path, required=True)
    parser.add_argument("--expected-runner-tool-manifest-sha256", required=True)
    parser.add_argument("--bash-bin", type=Path, required=True)
    parser.add_argument("--expected-bash-sha256", required=True)
    parser.add_argument("--expected-signer-fingerprint", required=True)
    parser.add_argument("--ssh-allowed-signers", type=Path, required=True)
    parser.add_argument("--expected-ssh-allowed-signers-sha256", required=True)
    parser.add_argument("--ssh-revocation-file", type=Path, required=True)
    parser.add_argument("--expected-ssh-revocation-sha256", required=True)
    parser.add_argument("--runner-environment", action="append", default=[])
    parser.add_argument(
        "--command-timeout-seconds",
        type=_positive_int,
        default=_DEFAULT_COMMAND_TIMEOUT_SECONDS,
    )
    return parser


def main() -> int:
    args = _parser().parse_args()
    try:
        return bootstrap(args)
    except BootstrapError as error:
        print(f"Sumeragi v2 release bootstrap failed: {error}", file=sys.stderr)
        return 2
    except OSError as error:
        print(f"Sumeragi v2 release bootstrap failed closed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
