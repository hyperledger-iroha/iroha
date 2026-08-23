#!/usr/bin/env python3
"""Verify detached OpenSSH SSHSIGs over canonical Sumeragi replay receipts.

This module never signs and never accepts private-key material.  Release callers
must supply a detached SSHSIG, a checksum-pinned ssh-keygen executable, one
strict allowed-signers entry, and a checksum-pinned revocation file.  The
verifier runs in a new process group with closed descriptors/environment,
bounded diagnostics and time, and mandatory descendant cleanup.
"""

from __future__ import annotations

import base64
import binascii
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import re
import selectors
import signal
import stat
import subprocess
import tempfile
import time
from typing import Any


SSHSIG_NAMESPACE = "iroha-sumeragi-v2-replay-receipt-v1"
SIGNATURE_FORMAT = "detached-ssh"
SIGNING_CONTRACT = {
    "scheme": SIGNATURE_FORMAT,
    "provider": "openssh-sshsig",
    "namespace": SSHSIG_NAMESPACE,
    "payload": "receipt.json",
    "artifact": "receipt.json.sig",
    "policy": {
        "allowed_signers": "allowed_signers",
        "revocation": "revocation.krl",
        "active_signers": 1,
    },
}

SHA256_RE = re.compile(r"[0-9a-f]{64}", re.ASCII)
FINGERPRINT_RE = re.compile(r"SHA256:[A-Za-z0-9+/]{43}", re.ASCII)
PRINCIPAL_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.@+-]{0,127}", re.ASCII)
BASE64_RE = re.compile(r"[A-Za-z0-9+/]+={0,2}", re.ASCII)
MAX_RECEIPT_BYTES = 256 * 1024
MAX_SIGNATURE_BYTES = 64 * 1024
MAX_POLICY_BYTES = 256 * 1024
MAX_TOOL_BYTES = 64 * 1024 * 1024
MAX_DIAGNOSTIC_BYTES = 64 * 1024
VERIFIER_TIMEOUT_SECONDS = 30.0
VERIFIER_CLEANUP_SECONDS = 2.0


class SigningError(RuntimeError):
    """The detached release signature or one of its inputs is invalid."""


@dataclass(frozen=True)
class FileSnapshot:
    """Stable identity and exact bytes of one canonical regular file."""

    path: Path
    data: bytes
    sha256: str
    size_bytes: int
    mode: int
    nlink: int
    device: int
    inode: int
    mtime_ns: int
    ctime_ns: int
    parent_device: int
    parent_inode: int
    parent_mtime_ns: int
    parent_ctime_ns: int
    parent_mode: int
    parent_uid: int

    def record(self, logical_path: str) -> dict[str, Any]:
        return {
            "path": logical_path,
            "sha256": self.sha256,
            "size_bytes": self.size_bytes,
            "mode": self.mode,
            "nlink": self.nlink,
        }


@dataclass(frozen=True)
class SignatureInputs:
    """Closed public inputs required to verify one release SSHSIG."""

    signature: Path
    expected_signature_sha256: str
    ssh_keygen: Path
    expected_ssh_keygen_sha256: str
    allowed_signers: Path
    expected_allowed_signers_sha256: str
    revocation_file: Path
    expected_revocation_sha256: str
    principal: str
    expected_signer_fingerprint: str


@dataclass(frozen=True)
class VerificationResult:
    """Public, bounded result of one successful ssh-keygen invocation."""

    signer_fingerprint: str
    stdout_sha256: str
    duration_monotonic_ns: int
    signature: FileSnapshot
    ssh_keygen: FileSnapshot
    allowed_signers: FileSnapshot
    revocation_file: FileSnapshot


@dataclass(frozen=True)
class ExactByteVerificationResult:
    """Result from verifying one already-sealed exact-byte input set."""

    signer_fingerprint: str
    stdout_sha256: str
    duration_monotonic_ns: int


@dataclass
class StagingRoot:
    """Held identities for the private verifier staging hierarchy."""

    outer_path: Path
    outer_descriptor: int
    outer_identity: tuple[int, int]
    parent_path: Path
    parent_name: str
    parent_descriptor: int
    parent_identity: tuple[int, int]
    path: Path
    name: str
    descriptor: int
    identity: tuple[int, int]
    created: dict[str, tuple[int, int]]
    parent_mtime_ns: int
    parent_ctime_ns: int
    root_mtime_ns: int
    root_ctime_ns: int


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def require_sha256(value: str, label: str) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        raise SigningError(f"{label} must be one lowercase SHA-256 digest")
    return value


def _absolute(path: Path) -> Path:
    return Path(os.path.abspath(path))


def read_snapshot(
    path: Path,
    label: str,
    *,
    maximum_bytes: int,
    executable: bool = False,
) -> FileSnapshot:
    """Read one canonical single-link file without following a final symlink."""

    path = _absolute(path)
    try:
        resolved = path.resolve(strict=True)
        before = path.lstat()
        parent_before = path.parent.lstat()
    except OSError as error:
        raise SigningError(f"{label} is unavailable") from error
    if resolved != path or stat.S_ISLNK(before.st_mode):
        raise SigningError(f"{label} must have a canonical non-symlink path")
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise SigningError(f"{label} must be one regular non-hard-linked file")
    if not stat.S_ISDIR(parent_before.st_mode):
        raise SigningError(f"{label} parent must be one canonical directory")
    if executable and before.st_mode & 0o111 == 0:
        raise SigningError(f"{label} is not executable")
    if before.st_size > maximum_bytes:
        raise SigningError(f"{label} exceeds its closed size limit")

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise SigningError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_nlink != 1
            or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
        ):
            raise SigningError(f"{label} changed while opening")
        chunks: list[bytes] = []
        total = 0
        digest = hashlib.sha256()
        while True:
            chunk = os.read(descriptor, min(1024 * 1024, maximum_bytes + 1 - total))
            if not chunk:
                break
            chunks.append(chunk)
            digest.update(chunk)
            total += len(chunk)
            if total > maximum_bytes:
                raise SigningError(f"{label} exceeds its closed size limit")
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    stable_opened = (
        opened.st_dev,
        opened.st_ino,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
        stat.S_IMODE(opened.st_mode),
        opened.st_nlink,
    )
    stable_after = (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
        stat.S_IMODE(after.st_mode),
        after.st_nlink,
    )
    if stable_after != stable_opened or total != opened.st_size:
        raise SigningError(f"{label} changed while reading")
    try:
        linked = path.lstat()
        parent_after = path.parent.lstat()
    except OSError as error:
        raise SigningError(f"{label} changed after reading") from error
    if (
        stat.S_ISLNK(linked.st_mode)
        or linked.st_nlink != 1
        or (linked.st_dev, linked.st_ino) != (opened.st_dev, opened.st_ino)
    ):
        raise SigningError(f"{label} pathname changed while reading")
    if any(
        getattr(parent_before, field) != getattr(parent_after, field)
        for field in ("st_dev", "st_ino", "st_mtime_ns", "st_ctime_ns", "st_mode")
    ):
        raise SigningError(f"{label} parent changed while reading")
    return FileSnapshot(
        path=path,
        data=b"".join(chunks),
        sha256=digest.hexdigest(),
        size_bytes=total,
        mode=stat.S_IMODE(opened.st_mode),
        nlink=opened.st_nlink,
        device=opened.st_dev,
        inode=opened.st_ino,
        mtime_ns=opened.st_mtime_ns,
        ctime_ns=opened.st_ctime_ns,
        parent_device=parent_after.st_dev,
        parent_inode=parent_after.st_ino,
        parent_mtime_ns=parent_after.st_mtime_ns,
        parent_ctime_ns=parent_after.st_ctime_ns,
        parent_mode=stat.S_IMODE(parent_after.st_mode),
        parent_uid=parent_after.st_uid,
    )


def require_unchanged(
    snapshot: FileSnapshot,
    label: str,
    *,
    maximum_bytes: int,
    executable: bool = False,
) -> None:
    current = read_snapshot(
        snapshot.path,
        label,
        maximum_bytes=maximum_bytes,
        executable=executable,
    )
    if current != snapshot:
        raise SigningError(f"{label} changed during verification")


def _require_digest(snapshot: FileSnapshot, expected: str, label: str) -> None:
    if snapshot.sha256 != require_sha256(expected, f"expected {label} digest"):
        raise SigningError(f"{label} does not match its protected SHA-256")


def validate_signing_contract(value: Any) -> None:
    if value != SIGNING_CONTRACT:
        raise SigningError("receipt detached-SSH contract differs from V1")


def _validate_signature(data: bytes) -> None:
    try:
        text = data.decode("ascii")
    except UnicodeDecodeError as error:
        raise SigningError("detached SSHSIG is not ASCII armor") from error
    if "\r" in text or "\0" in text or not text.endswith("\n"):
        raise SigningError("detached SSHSIG must be canonical LF-only armor")
    lines = text.splitlines()
    if (
        len(lines) < 3
        or lines[0] != "-----BEGIN SSH SIGNATURE-----"
        or lines[-1] != "-----END SSH SIGNATURE-----"
        or any(not line or len(line) > 76 for line in lines[1:-1])
    ):
        raise SigningError("detached SSHSIG armor is malformed")
    encoded = "".join(lines[1:-1])
    if BASE64_RE.fullmatch(encoded) is None:
        raise SigningError("detached SSHSIG armor is malformed")
    try:
        decoded = base64.b64decode(encoded, validate=True)
    except (ValueError, binascii.Error) as error:
        raise SigningError("detached SSHSIG armor is malformed") from error
    if not decoded.startswith(b"SSHSIG"):
        raise SigningError("detached signature is not OpenSSH SSHSIG data")


def _validate_allowed_signers(data: bytes, principal: str) -> None:
    if PRINCIPAL_RE.fullmatch(principal) is None:
        raise SigningError("release signer principal is invalid")
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise SigningError("SSH allowed-signers policy must be UTF-8 text") from error
    if "\r" in text or "\0" in text or not text.endswith("\n"):
        raise SigningError("SSH allowed-signers policy must be canonical LF-only text")
    lines = text.splitlines()
    if len(lines) != 1:
        raise SigningError("SSH allowed-signers policy must contain exactly one active key")
    fields = lines[0].split(" ")
    if len(fields) != 3 or fields[0] != principal or fields[1] != "ssh-ed25519":
        raise SigningError(
            "SSH allowed-signers entry must be exactly one principal and one Ed25519 key"
        )
    if PRINCIPAL_RE.fullmatch(fields[0]) is None or BASE64_RE.fullmatch(fields[2]) is None:
        raise SigningError("SSH allowed-signers entry is malformed")
    try:
        key = base64.b64decode(fields[2], validate=True)
    except (ValueError, binascii.Error) as error:
        raise SigningError("SSH allowed-signers key is malformed") from error
    if base64.b64encode(key).decode("ascii") != fields[2]:
        raise SigningError("SSH allowed-signers key is not canonical base64")

    def wire_string(blob: bytes, offset: int) -> tuple[bytes, int]:
        if offset + 4 > len(blob):
            raise SigningError("SSH allowed-signers Ed25519 wire key is truncated")
        size = int.from_bytes(blob[offset : offset + 4], "big")
        offset += 4
        if size == 0 or offset + size > len(blob):
            raise SigningError("SSH allowed-signers Ed25519 wire key is malformed")
        return blob[offset : offset + size], offset + size

    wire_type, offset = wire_string(key, 0)
    wire_key, offset = wire_string(key, offset)
    if wire_type != b"ssh-ed25519" or len(wire_key) != 32 or offset != len(key):
        raise SigningError("SSH allowed-signers key is malformed")
    canonical = f"{principal} ssh-ed25519 {fields[2]}\n"
    if text != canonical:
        raise SigningError("SSH allowed-signers entry is not canonical")


def _write_staged(
    directory: int, name: str, data: bytes, mode: int
) -> tuple[int, int]:
    if "/" in name or name in {"", ".", ".."}:
        raise SigningError("private verifier input name is invalid")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(name, flags, mode, dir_fd=directory)
    try:
        offset = 0
        while offset < len(data):
            written = os.write(descriptor, data[offset:])
            if written <= 0:
                raise SigningError("private verifier input could not be staged")
            offset += written
        os.fchmod(descriptor, mode)
        os.fsync(descriptor)
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or stat.S_IMODE(metadata.st_mode) != mode
            or metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
            or metadata.st_size != len(data)
        ):
            raise SigningError("private verifier input metadata differs")
        identity = (metadata.st_dev, metadata.st_ino)
    finally:
        os.close(descriptor)
    return identity


def _directory_flags() -> int:
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
    )
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _private_directory(
    metadata: os.stat_result, label: str
) -> tuple[int, int]:
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != 0o700
        or metadata.st_uid != os.geteuid()
    ):
        raise SigningError(f"{label} is not an owner-owned 0700 directory")
    return (metadata.st_dev, metadata.st_ino)


def _create_staging_root() -> StagingRoot:
    """Create a private two-level root and hold every directory identity."""

    try:
        outer_path = Path(tempfile.gettempdir()).resolve(strict=True)
        outer_link = outer_path.lstat()
        outer_descriptor = os.open(outer_path, _directory_flags())
    except OSError as error:
        raise SigningError("verifier staging parent is unavailable") from error
    parent_descriptor = -1
    root_descriptor = -1
    parent_path: Path | None = None
    parent_name: str | None = None
    parent_identity: tuple[int, int] | None = None
    root_identity: tuple[int, int] | None = None
    root_name = "evidence"
    try:
        outer_opened = os.fstat(outer_descriptor)
        outer_identity = (outer_opened.st_dev, outer_opened.st_ino)
        if (
            not stat.S_ISDIR(outer_opened.st_mode)
            or (outer_link.st_dev, outer_link.st_ino) != outer_identity
        ):
            raise SigningError("verifier staging parent identity differs")
        raw_parent = tempfile.mkdtemp(
            prefix="sumeragi-v2-replay-sshsig.", dir=outer_path
        )
        parent_path = Path(raw_parent)
        parent_name = parent_path.name
        parent_link = os.stat(
            parent_name,
            dir_fd=outer_descriptor,
            follow_symlinks=False,
        )
        parent_identity = _private_directory(
            parent_link, "private verifier parent"
        )
        if (
            parent_path.parent != outer_path
            or parent_path.resolve(strict=True) != parent_path
        ):
            raise SigningError("private verifier parent is not canonical")
        parent_descriptor = os.open(
            parent_name, _directory_flags(), dir_fd=outer_descriptor
        )
        if (
            _private_directory(
                os.fstat(parent_descriptor), "private verifier parent"
            )
            != parent_identity
        ):
            raise SigningError("private verifier parent identity differs")
        os.mkdir(root_name, 0o700, dir_fd=parent_descriptor)
        root_link = os.stat(
            root_name,
            dir_fd=parent_descriptor,
            follow_symlinks=False,
        )
        root_identity = _private_directory(root_link, "private verifier root")
        root_descriptor = os.open(
            root_name, _directory_flags(), dir_fd=parent_descriptor
        )
        if (
            _private_directory(
                os.fstat(root_descriptor), "private verifier root"
            )
            != root_identity
        ):
            raise SigningError("private verifier root identity differs")
        return StagingRoot(
            outer_path=outer_path,
            outer_descriptor=outer_descriptor,
            outer_identity=outer_identity,
            parent_path=parent_path,
            parent_name=parent_name,
            parent_descriptor=parent_descriptor,
            parent_identity=parent_identity,
            path=parent_path / root_name,
            name=root_name,
            descriptor=root_descriptor,
            identity=root_identity,
            created={},
            parent_mtime_ns=0,
            parent_ctime_ns=0,
            root_mtime_ns=0,
            root_ctime_ns=0,
        )
    except Exception:
        if root_descriptor >= 0:
            os.close(root_descriptor)
        if (
            parent_descriptor >= 0
            and root_identity is not None
        ):
            try:
                linked = os.stat(
                    root_name,
                    dir_fd=parent_descriptor,
                    follow_symlinks=False,
                )
                if (
                    (linked.st_dev, linked.st_ino) == root_identity
                    and os.listdir(parent_descriptor) == [root_name]
                ):
                    os.rmdir(root_name, dir_fd=parent_descriptor)
            except OSError:
                pass
        if parent_descriptor >= 0:
            os.close(parent_descriptor)
        if parent_name is not None and parent_identity is not None:
            try:
                linked = os.stat(
                    parent_name,
                    dir_fd=outer_descriptor,
                    follow_symlinks=False,
                )
                if (linked.st_dev, linked.st_ino) == parent_identity:
                    os.rmdir(parent_name, dir_fd=outer_descriptor)
            except OSError:
                pass
        os.close(outer_descriptor)
        raise


def _require_staging_root(stage: StagingRoot) -> None:
    try:
        outer_link = stage.outer_path.lstat()
        parent_link = os.stat(
            stage.parent_name,
            dir_fd=stage.outer_descriptor,
            follow_symlinks=False,
        )
        parent_lexical = stage.parent_path.lstat()
        root_link = os.stat(
            stage.name,
            dir_fd=stage.parent_descriptor,
            follow_symlinks=False,
        )
        root_lexical = stage.path.lstat()
    except OSError as error:
        raise SigningError("private verifier staging ownership changed") from error
    parent_opened = os.fstat(stage.parent_descriptor)
    root_opened = os.fstat(stage.descriptor)
    if (
        (outer_link.st_dev, outer_link.st_ino) != stage.outer_identity
        or (os.fstat(stage.outer_descriptor).st_dev, os.fstat(stage.outer_descriptor).st_ino)
        != stage.outer_identity
        or _private_directory(parent_link, "private verifier parent")
        != stage.parent_identity
        or _private_directory(parent_lexical, "private verifier parent")
        != stage.parent_identity
        or _private_directory(parent_opened, "private verifier parent")
        != stage.parent_identity
        or _private_directory(root_link, "private verifier root")
        != stage.identity
        or _private_directory(root_lexical, "private verifier root")
        != stage.identity
        or _private_directory(root_opened, "private verifier root")
        != stage.identity
        or (
            stage.parent_mtime_ns != 0
            and (
                parent_opened.st_mtime_ns != stage.parent_mtime_ns
                or parent_opened.st_ctime_ns != stage.parent_ctime_ns
            )
        )
        or (
            stage.root_mtime_ns != 0
            and (
                root_opened.st_mtime_ns != stage.root_mtime_ns
                or root_opened.st_ctime_ns != stage.root_ctime_ns
            )
        )
    ):
        raise SigningError("private verifier staging ownership changed")


def _seal_staging_root(stage: StagingRoot) -> None:
    parent = os.fstat(stage.parent_descriptor)
    root = os.fstat(stage.descriptor)
    stage.parent_mtime_ns = parent.st_mtime_ns
    stage.parent_ctime_ns = parent.st_ctime_ns
    stage.root_mtime_ns = root.st_mtime_ns
    stage.root_ctime_ns = root.st_ctime_ns
    _require_staging_root(stage)


def _cleanup_staging_root(stage: StagingRoot) -> None:
    """Remove only held fixed-name inodes; never recurse through pathnames."""

    failures: list[str] = []
    try:
        for name in reversed(tuple(stage.created)):
            try:
                metadata = os.stat(
                    name,
                    dir_fd=stage.descriptor,
                    follow_symlinks=False,
                )
                if (
                    not stat.S_ISREG(metadata.st_mode)
                    or metadata.st_nlink != 1
                    or (metadata.st_dev, metadata.st_ino)
                    != stage.created[name]
                ):
                    failures.append(name)
                    continue
                os.unlink(name, dir_fd=stage.descriptor)
            except OSError:
                failures.append(name)
        try:
            remaining_root = os.listdir(stage.descriptor)
        except OSError:
            remaining_root = ["<unreadable-root>"]
        failures.extend(str(name) for name in remaining_root)
        root_removed = False
        try:
            root_link = os.stat(
                stage.name,
                dir_fd=stage.parent_descriptor,
                follow_symlinks=False,
            )
            if (root_link.st_dev, root_link.st_ino) != stage.identity:
                failures.append("<root-link>")
            elif not remaining_root:
                os.fsync(stage.descriptor)
                os.rmdir(stage.name, dir_fd=stage.parent_descriptor)
                os.fsync(stage.parent_descriptor)
                root_removed = True
        except OSError:
            failures.append("<root-link>")

        if root_removed:
            try:
                remaining_parent = os.listdir(stage.parent_descriptor)
            except OSError:
                remaining_parent = ["<unreadable-parent>"]
            failures.extend(str(name) for name in remaining_parent)
            try:
                parent_link = os.stat(
                    stage.parent_name,
                    dir_fd=stage.outer_descriptor,
                    follow_symlinks=False,
                )
                outer_link = stage.outer_path.lstat()
                if (
                    (parent_link.st_dev, parent_link.st_ino)
                    != stage.parent_identity
                    or (outer_link.st_dev, outer_link.st_ino)
                    != stage.outer_identity
                ):
                    failures.append("<parent-link>")
                elif not remaining_parent:
                    os.rmdir(
                        stage.parent_name, dir_fd=stage.outer_descriptor
                    )
                    os.fsync(stage.outer_descriptor)
            except OSError:
                failures.append("<parent-link>")
    finally:
        os.close(stage.descriptor)
        os.close(stage.parent_descriptor)
        os.close(stage.outer_descriptor)
    if failures:
        raise SigningError(
            "private verifier staging cleanup refused changed ownership: "
            + ", ".join(sorted(set(failures)))
        )


def _process_group_exists(process: subprocess.Popen[bytes]) -> bool:
    try:
        os.killpg(process.pid, 0)
    except ProcessLookupError:
        return False
    except (PermissionError, OSError):
        return True
    return True


def _kill_group(process: subprocess.Popen[bytes], sig: signal.Signals) -> bool:
    try:
        os.killpg(process.pid, sig)
    except ProcessLookupError:
        return True
    except OSError:
        return False
    return True


def _kill_and_reap(process: subprocess.Popen[bytes]) -> bool:
    deadline = time.monotonic() + VERIFIER_CLEANUP_SECONDS
    ok = _kill_group(process, signal.SIGTERM)
    grace = min(deadline, time.monotonic() + min(0.25, VERIFIER_CLEANUP_SECONDS))
    while process.poll() is None and time.monotonic() < grace:
        try:
            process.wait(timeout=min(0.05, max(0.001, grace - time.monotonic())))
        except subprocess.TimeoutExpired:
            pass
    if process.poll() is None or _process_group_exists(process):
        ok = _kill_group(process, signal.SIGKILL) and ok
    while process.poll() is None:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return False
        try:
            process.wait(timeout=min(0.05, remaining))
        except subprocess.TimeoutExpired:
            _kill_group(process, signal.SIGKILL)
    while _process_group_exists(process):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return False
        ok = _kill_group(process, signal.SIGKILL) and ok
        time.sleep(min(0.01, remaining))
    return ok


def _run_verifier(
    executable: Path,
    arguments: list[str],
    *,
    stdin_snapshot: FileSnapshot,
    cwd: Path,
) -> tuple[bytes, int]:
    """Run ssh-keygen with accepted bounded process-group supervision."""

    if os.name != "posix":  # pragma: no cover - release corridor is POSIX
        raise SigningError("release SSHSIG verification requires POSIX process groups")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        stdin_descriptor = os.open(stdin_snapshot.path, flags)
    except OSError as error:
        raise SigningError("canonical receipt could not be opened as verifier stdin") from error
    opened_stdin = os.fstat(stdin_descriptor)
    if (
        not stat.S_ISREG(opened_stdin.st_mode)
        or opened_stdin.st_nlink != 1
        or (opened_stdin.st_dev, opened_stdin.st_ino)
        != (stdin_snapshot.device, stdin_snapshot.inode)
        or opened_stdin.st_size != stdin_snapshot.size_bytes
        or stat.S_IMODE(opened_stdin.st_mode) != stdin_snapshot.mode
        or opened_stdin.st_mtime_ns != stdin_snapshot.mtime_ns
        or opened_stdin.st_ctime_ns != stdin_snapshot.ctime_ns
    ):
        os.close(stdin_descriptor)
        raise SigningError("canonical receipt stdin inode differs from its snapshot")
    process: subprocess.Popen[bytes] | None = None
    selector = selectors.DefaultSelector()
    cleanup_required = True
    started = time.monotonic_ns()
    try:
        process = subprocess.Popen(
            [str(executable), *arguments],
            stdin=stdin_descriptor,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=cwd,
            env={"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin", "TZ": "UTC"},
            close_fds=True,
            start_new_session=True,
            umask=0o077,
            bufsize=0,
        )
        assert process.stdout is not None
        assert process.stderr is not None
        for pipe, label in ((process.stdout, "stdout"), (process.stderr, "stderr")):
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ, label)
        buffers = {"stdout": bytearray(), "stderr": bytearray()}
        total = 0
        deadline = time.monotonic() + VERIFIER_TIMEOUT_SECONDS
        while selector.get_map() or process.poll() is None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise SigningError("pinned ssh-keygen verification timed out")
            for key, _ in selector.select(min(remaining, 0.05)):
                try:
                    chunk = os.read(key.fd, min(4096, MAX_DIAGNOSTIC_BYTES + 1 - total))
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                total += len(chunk)
                if total > MAX_DIAGNOSTIC_BYTES:
                    raise SigningError(
                        "pinned ssh-keygen output exceeded its bound"
                    )
                buffers[key.data].extend(chunk)
        status = process.wait()
        if _process_group_exists(process):
            raise SigningError(
                "pinned ssh-keygen left a live process-group member"
            )
        cleanup_required = False
        if status != 0:
            raise SigningError("detached SSHSIG verification failed")
        if buffers["stderr"]:
            raise SigningError("pinned ssh-keygen emitted separate stderr")
        return bytes(buffers["stdout"]), time.monotonic_ns() - started
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        raise SigningError(
            "pinned ssh-keygen could not be executed safely"
        ) from error
    finally:
        os.close(stdin_descriptor)
        cleanup_ok = True
        if process is not None and cleanup_required:
            cleanup_ok = _kill_and_reap(process)
        for pipe in (
            None if process is None else process.stdout,
            None if process is None else process.stderr,
        ):
            if pipe is not None and not pipe.closed:
                try:
                    selector.unregister(pipe)
                except (KeyError, OSError, ValueError):
                    pass
                try:
                    pipe.close()
                except OSError:
                    cleanup_ok = False
        selector.close()
        if not cleanup_ok:
            raise SigningError(
                "pinned ssh-keygen process group could not be cleaned"
            )


def _require_exact_bytes(data: bytes, label: str, maximum_bytes: int) -> None:
    if not isinstance(data, bytes):
        raise SigningError(f"{label} must be exact bytes")
    if len(data) > maximum_bytes:
        raise SigningError(f"{label} exceeds its closed size limit")


def verify_exact_signature_bytes(
    *,
    receipt: bytes,
    signature: bytes,
    ssh_keygen: bytes,
    allowed_signers: bytes,
    revocation_file: bytes,
    expected_signature_sha256: str,
    expected_ssh_keygen_sha256: str,
    expected_allowed_signers_sha256: str,
    expected_revocation_sha256: str,
    principal: str,
    expected_signer_fingerprint: str,
) -> ExactByteVerificationResult:
    """Verify one dirfd-sealed exact-byte input set without reopening its paths."""

    for data, label, maximum in (
        (receipt, "canonical receipt", MAX_RECEIPT_BYTES),
        (signature, "detached SSHSIG", MAX_SIGNATURE_BYTES),
        (ssh_keygen, "pinned ssh-keygen executable", MAX_TOOL_BYTES),
        (allowed_signers, "SSH allowed-signers policy", MAX_POLICY_BYTES),
        (revocation_file, "SSH revocation policy", MAX_POLICY_BYTES),
    ):
        _require_exact_bytes(data, label, maximum)
    if not ssh_keygen:
        raise SigningError("pinned ssh-keygen executable is empty")
    for data, expected, label in (
        (signature, expected_signature_sha256, "detached SSHSIG"),
        (
            ssh_keygen,
            expected_ssh_keygen_sha256,
            "pinned ssh-keygen executable",
        ),
        (
            allowed_signers,
            expected_allowed_signers_sha256,
            "SSH allowed-signers policy",
        ),
        (
            revocation_file,
            expected_revocation_sha256,
            "SSH revocation policy",
        ),
    ):
        if sha256_bytes(data) != require_sha256(
            expected, f"expected {label} digest"
        ):
            raise SigningError(f"{label} does not match its protected SHA-256")
    if FINGERPRINT_RE.fullmatch(expected_signer_fingerprint) is None:
        raise SigningError("expected signer fingerprint is invalid")
    _validate_signature(signature)
    _validate_allowed_signers(allowed_signers, principal)

    stage = _create_staging_root()
    try:
        staged_values = (
            ("receipt.json", receipt, 0o400, MAX_RECEIPT_BYTES, False),
            (
                "receipt.json.sig",
                signature,
                0o400,
                MAX_SIGNATURE_BYTES,
                False,
            ),
            ("ssh-keygen", ssh_keygen, 0o500, MAX_TOOL_BYTES, True),
            (
                "allowed_signers",
                allowed_signers,
                0o400,
                MAX_POLICY_BYTES,
                False,
            ),
            (
                "revocation.krl",
                revocation_file,
                0o400,
                MAX_POLICY_BYTES,
                False,
            ),
        )
        for name, data, mode, _maximum, _executable in staged_values:
            stage.created[name] = _write_staged(
                stage.descriptor, name, data, mode
            )
        os.fsync(stage.descriptor)

        staged: dict[str, FileSnapshot] = {}
        for name, data, mode, maximum, executable in staged_values:
            snapshot = read_snapshot(
                stage.path / name,
                f"private staged {name}",
                maximum_bytes=maximum,
                executable=executable,
            )
            if (
                snapshot.data != data
                or snapshot.mode != mode
                or snapshot.nlink != 1
                or (snapshot.device, snapshot.inode) != stage.created[name]
            ):
                raise SigningError(f"private staged {name} differs")
            staged[name] = snapshot
        _seal_staging_root(stage)

        stdout, duration = _run_verifier(
            staged["ssh-keygen"].path,
            [
                "-Y",
                "verify",
                "-f",
                str(staged["allowed_signers"].path),
                "-I",
                principal,
                "-n",
                SSHSIG_NAMESPACE,
                "-s",
                str(staged["receipt.json.sig"].path),
                "-r",
                str(staged["revocation.krl"].path),
            ],
            stdin_snapshot=staged["receipt.json"],
            cwd=stage.path,
        )
        for name, data, _mode, maximum, executable in staged_values:
            current = read_snapshot(
                stage.path / name,
                f"private staged {name}",
                maximum_bytes=maximum,
                executable=executable,
            )
            if current != staged[name] or current.data != data:
                raise SigningError(
                    f"private staged {name} changed during verification"
                )
        if set(os.listdir(stage.descriptor)) != set(stage.created):
            raise SigningError("private verifier staging file set changed")
        _require_staging_root(stage)
    finally:
        _cleanup_staging_root(stage)

    try:
        observed_output = stdout.decode("utf-8")
    except UnicodeDecodeError as error:
        raise SigningError(
            "pinned ssh-keygen emitted non-UTF-8 output"
        ) from error
    prefix = (
        f'Good "{SSHSIG_NAMESPACE}" signature for {principal} '
        "with ED25519 key "
    )
    expected_output = f"{prefix}{expected_signer_fingerprint}\n"
    if observed_output != expected_output:
        raise SigningError(
            "pinned ssh-keygen emitted an unexpected success record"
        )
    return ExactByteVerificationResult(
        signer_fingerprint=expected_signer_fingerprint,
        stdout_sha256=sha256_bytes(stdout),
        duration_monotonic_ns=duration,
    )


def verify_external_signature(
    receipt: FileSnapshot, inputs: SignatureInputs
) -> VerificationResult:
    """Verify path inputs after snapshotting and reauthenticating every source."""

    if receipt.size_bytes != len(receipt.data):
        raise SigningError("canonical receipt snapshot size differs")
    signature = read_snapshot(
        inputs.signature,
        "detached SSHSIG",
        maximum_bytes=MAX_SIGNATURE_BYTES,
    )
    ssh_keygen = read_snapshot(
        inputs.ssh_keygen,
        "pinned ssh-keygen executable",
        maximum_bytes=MAX_TOOL_BYTES,
        executable=True,
    )
    allowed = read_snapshot(
        inputs.allowed_signers,
        "SSH allowed-signers policy",
        maximum_bytes=MAX_POLICY_BYTES,
    )
    revocation = read_snapshot(
        inputs.revocation_file,
        "SSH revocation policy",
        maximum_bytes=MAX_POLICY_BYTES,
    )
    exact = verify_exact_signature_bytes(
        receipt=receipt.data,
        signature=signature.data,
        ssh_keygen=ssh_keygen.data,
        allowed_signers=allowed.data,
        revocation_file=revocation.data,
        expected_signature_sha256=inputs.expected_signature_sha256,
        expected_ssh_keygen_sha256=inputs.expected_ssh_keygen_sha256,
        expected_allowed_signers_sha256=(
            inputs.expected_allowed_signers_sha256
        ),
        expected_revocation_sha256=inputs.expected_revocation_sha256,
        principal=inputs.principal,
        expected_signer_fingerprint=inputs.expected_signer_fingerprint,
    )

    require_unchanged(
        receipt,
        "canonical receipt",
        maximum_bytes=MAX_RECEIPT_BYTES,
    )
    for snapshot, label, bound, executable in (
        (signature, "detached SSHSIG", MAX_SIGNATURE_BYTES, False),
        (
            ssh_keygen,
            "pinned ssh-keygen executable",
            MAX_TOOL_BYTES,
            True,
        ),
        (allowed, "SSH allowed-signers policy", MAX_POLICY_BYTES, False),
        (revocation, "SSH revocation policy", MAX_POLICY_BYTES, False),
    ):
        require_unchanged(
            snapshot,
            label,
            maximum_bytes=bound,
            executable=executable,
        )
    return VerificationResult(
        signer_fingerprint=exact.signer_fingerprint,
        stdout_sha256=exact.stdout_sha256,
        duration_monotonic_ns=exact.duration_monotonic_ns,
        signature=signature,
        ssh_keygen=ssh_keygen,
        allowed_signers=allowed,
        revocation_file=revocation,
    )
