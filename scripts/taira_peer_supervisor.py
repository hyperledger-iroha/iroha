#!/usr/bin/env python3
"""Supervise exactly one Taira validator process.

This helper is installed by ``migrate_taira_peer_supervision.py`` and is not
intended to be started by hand.  It preserves the validated binary, config,
and storage-directory identities from the migration plan, forwards shutdown
signals to the validator, and applies bounded exponential restart backoff.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import signal
import stat
import subprocess
import sys
import time
from pathlib import Path
from types import FrameType
from typing import Any


class IdentityError(RuntimeError):
    """Raised when a planned runtime path has changed identity."""


BINARY_STAT_SEAL_FIELDS = (
    "binary_device",
    "binary_inode",
    "binary_size",
    "binary_mtime_ns",
    "binary_ctime_ns",
)
MACOS_ACL_INSPECTOR = Path("/bin/ls")
MACOS_ACL_CLEARER = Path("/bin/chmod")
MACOS_ACL_COMMAND_TIMEOUT_SECONDS = 5
MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES = 64 * 1024
TERMINAL_UNHEALTHY_SCHEMA = "taira-terminal-unhealthy-v1"
TERMINAL_UNHEALTHY_MAX_BYTES = 1024
FATAL_STDERR_TAIL_MAX_BYTES = 64 * 1024
FATAL_SIGNATURE_MAX_BYTES = 4096
RAPID_FATAL_EXIT_LIMIT = 3
DEFAULT_RAPID_FATAL_UPTIME_SECONDS = 30.0
FATAL_LINE_RE = re.compile(
    r"(?i)(?:\bfatal\b|\bpanic(?:ked)?\b|\bunrecoverable\b|\berror\b)"
)
ANSI_ESCAPE_RE = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
TRACING_TIMESTAMP_RE = re.compile(
    r"(?i)\b[0-9]{4}-[0-9]{2}-[0-9]{2}"
    r"[T ][0-9]{2}:[0-9]{2}:[0-9]{2}"
    r"(?:\.[0-9]+)?(?:Z|[+-][0-9]{2}:[0-9]{2})\b"
)
ABSOLUTE_PATH_RE = re.compile(r"(?<![A-Za-z0-9_])(?:/[^\s\"'<>:]+)+")
UUID_RE = re.compile(
    r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b"
)
LONG_HEX_RE = re.compile(r"(?i)\b(?:0x)?[0-9a-f]{8,}\b")
HIGH_ENTROPY_TOKEN_RE = re.compile(
    r"(?<![A-Za-z0-9+/_-])[A-Za-z0-9+/_-]{40,}={0,2}" r"(?![A-Za-z0-9+/_-])"
)
DECIMAL_RE = re.compile(r"\b[0-9]+\b")


def metadata_identity(info: os.stat_result) -> tuple[int, ...]:
    """Return the path fields that must remain stable around an ACL query."""

    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_uid,
        info.st_gid,
        info.st_nlink,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def require_acl_free_path(path: Path, label: str) -> os.stat_result:
    """Require a stable path, and on macOS prove it has no extended ACL."""

    before = path.lstat()
    if sys.platform == "darwin":
        try:
            result = subprocess.run(
                [str(MACOS_ACL_INSPECTOR), "-ldeq", str(path)],
                check=False,
                stdin=subprocess.DEVNULL,
                capture_output=True,
                timeout=MACOS_ACL_COMMAND_TIMEOUT_SECONDS,
                env={"LC_ALL": "C", "PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise IdentityError(
                f"bounded macOS ACL command failed for {label}: {path}"
            ) from error
        if (
            len(result.stdout) > MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES
            or len(result.stderr) > MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES
        ):
            raise IdentityError(
                f"macOS ACL command output exceeded its bound for {label}: {path}"
            )
        if (
            result.returncode != 0
            or result.stderr
            or not result.stdout.endswith(b"\n")
            or result.stdout.count(b"\n") != 1
        ):
            raise IdentityError(f"{label} must not have an extended ACL: {path}")
    after = path.lstat()
    if metadata_identity(after) != metadata_identity(before):
        raise IdentityError(f"{label} changed during ACL validation: {path}")
    return after


def fsync_directory(path: Path) -> None:
    """Durably order one publication or exact removal in ``path``."""

    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def require_private_directory(path: Path, label: str) -> os.stat_result:
    """Require one stable owner-only, non-symlink directory."""

    info = require_acl_free_path(path, label)
    if (
        stat.S_ISLNK(info.st_mode)
        or not stat.S_ISDIR(info.st_mode)
        or info.st_uid != os.geteuid()
        or stat.S_IMODE(info.st_mode) != 0o700
    ):
        raise IdentityError(f"{label} is not an owner-private directory")
    return info


def clear_inherited_acl(path: Path, expected: os.stat_result, label: str) -> None:
    """Clear a macOS inherited ACL without accepting a pathname replacement."""

    if sys.platform != "darwin":
        return
    try:
        result = subprocess.run(
            [str(MACOS_ACL_CLEARER), "-N", str(path)],
            check=False,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            timeout=MACOS_ACL_COMMAND_TIMEOUT_SECONDS,
            env={"LC_ALL": "C", "PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise IdentityError(f"bounded macOS ACL clear failed for {label}") from error
    if (
        result.returncode != 0
        or result.stdout
        or result.stderr
        or len(result.stdout) > MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES
        or len(result.stderr) > MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES
    ):
        raise IdentityError(f"macOS ACL clear failed for {label}")
    current = require_acl_free_path(path, label)
    if (current.st_dev, current.st_ino) != (expected.st_dev, expected.st_ino):
        raise IdentityError(f"{label} changed during ACL clearing")


def exact_file_identity(info: os.stat_result) -> tuple[int, ...]:
    """Return identity fields used for exact-inode removal and publication."""

    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_uid,
        info.st_gid,
        info.st_nlink,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def terminal_binding_sha256(args: argparse.Namespace) -> str:
    """Bind a persisted terminal latch to generation, binary, and config identity."""

    stat_values = tuple(getattr(args, field, None) for field in BINARY_STAT_SEAL_FIELDS)
    payload = {
        "binary_sha256": args.binary_sha256,
        "binary_stat_seal": stat_values,
        "config_sha256": args.config_sha256,
        "restart_generation": args.restart_generation,
        "schema": TERMINAL_UNHEALTHY_SCHEMA,
    }
    encoded = json.dumps(
        payload,
        ensure_ascii=True,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("ascii")
    return hashlib.sha256(encoded).hexdigest()


def terminal_payload(binding: str, fatal_fingerprint: str) -> bytes:
    """Build the canonical, payload-free terminal-unhealthy marker."""

    payload = {
        "binding_sha256": binding,
        "fatal_fingerprint_sha256": fatal_fingerprint,
        "hit_count": RAPID_FATAL_EXIT_LIMIT,
        "schema": TERMINAL_UNHEALTHY_SCHEMA,
    }
    body = (
        json.dumps(
            payload,
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("ascii")
    if len(body) > TERMINAL_UNHEALTHY_MAX_BYTES:
        raise IdentityError("terminal-unhealthy fingerprint exceeded its bound")
    return body


def decode_terminal_payload(body: bytes) -> dict[str, Any]:
    """Decode only the canonical bounded terminal marker schema."""

    if not body or len(body) > TERMINAL_UNHEALTHY_MAX_BYTES:
        raise IdentityError("terminal-unhealthy fingerprint has an invalid size")
    try:
        payload = json.loads(body)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise IdentityError(
            "terminal-unhealthy fingerprint is not canonical"
        ) from error
    if (
        not isinstance(payload, dict)
        or set(payload)
        != {
            "binding_sha256",
            "fatal_fingerprint_sha256",
            "hit_count",
            "schema",
        }
        or payload.get("schema") != TERMINAL_UNHEALTHY_SCHEMA
        or payload.get("hit_count") != RAPID_FATAL_EXIT_LIMIT
        or not isinstance(payload.get("binding_sha256"), str)
        or re.fullmatch(r"[0-9a-f]{64}", payload["binding_sha256"]) is None
        or not isinstance(payload.get("fatal_fingerprint_sha256"), str)
        or re.fullmatch(r"[0-9a-f]{64}", payload["fatal_fingerprint_sha256"]) is None
        or terminal_payload(
            payload["binding_sha256"], payload["fatal_fingerprint_sha256"]
        )
        != body
    ):
        raise IdentityError("terminal-unhealthy fingerprint is not canonical")
    return payload


def read_terminal_payload(
    path: Path,
) -> tuple[dict[str, Any], os.stat_result] | None:
    """Read one owner-private marker through a stable no-follow descriptor."""

    try:
        before = require_acl_free_path(path, "terminal-unhealthy fingerprint")
    except FileNotFoundError:
        return None
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_uid != os.geteuid()
        or before.st_nlink != 1
        or stat.S_IMODE(before.st_mode) != 0o600
        or before.st_size > TERMINAL_UNHEALTHY_MAX_BYTES
    ):
        raise IdentityError("terminal-unhealthy fingerprint has unsafe metadata")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        body = bytearray()
        while len(body) <= TERMINAL_UNHEALTHY_MAX_BYTES:
            chunk = os.read(
                descriptor,
                min(
                    256,
                    TERMINAL_UNHEALTHY_MAX_BYTES + 1 - len(body),
                ),
            )
            if not chunk:
                break
            body.extend(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if (
        exact_file_identity(before) != exact_file_identity(after)
        or len(body) > TERMINAL_UNHEALTHY_MAX_BYTES
    ):
        raise IdentityError("terminal-unhealthy fingerprint changed while reading")
    return decode_terminal_payload(bytes(body)), after


def unlink_exact(path: Path, expected: os.stat_result, label: str) -> None:
    """Unlink only the exact non-symlink inode previously authenticated."""

    try:
        current = path.lstat()
    except FileNotFoundError as error:
        raise IdentityError(f"{label} disappeared before exact removal") from error
    if exact_file_identity(current) != exact_file_identity(expected):
        raise IdentityError(f"{label} changed before exact removal")
    path.unlink()


def clear_stale_terminal_payload(path: Path, expected: os.stat_result) -> None:
    """Durably remove only one authenticated stale-generation marker."""

    unlink_exact(path, expected, "terminal-unhealthy fingerprint")
    fsync_directory(path.parent)
    try:
        path.lstat()
    except FileNotFoundError:
        return
    raise IdentityError("terminal-unhealthy fingerprint reappeared during reset")


def existing_terminal_latch(path: Path, binding: str) -> bool:
    """Return a matching latch, or durably clear an old identity/generation."""

    require_private_directory(path.parent, "terminal-unhealthy directory")
    existing = read_terminal_payload(path)
    if existing is None:
        return False
    payload, info = existing
    if payload["binding_sha256"] == binding:
        return True
    clear_stale_terminal_payload(path, info)
    return False


def publish_terminal_payload(
    path: Path, binding: str, fatal_fingerprint: str
) -> os.stat_result:
    """Atomically and durably publish one owner-private terminal fingerprint."""

    require_private_directory(path.parent, "terminal-unhealthy directory")
    body = terminal_payload(binding, fatal_fingerprint)
    existing = read_terminal_payload(path)
    if existing is not None:
        payload, info = existing
        if (
            terminal_payload(
                payload["binding_sha256"], payload["fatal_fingerprint_sha256"]
            )
            == body
        ):
            return info
        raise IdentityError("terminal-unhealthy fingerprint already exists")

    temporary = path.with_name(f".{path.name}.{os.getpid()}.{time.monotonic_ns()}.tmp")
    descriptor = -1
    temporary_info: os.stat_result | None = None
    staged_inode: tuple[int, int, int] | None = None
    published_created = False
    publication_complete = False
    try:
        descriptor = os.open(
            temporary,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_NOFOLLOW", 0)
            | getattr(os, "O_CLOEXEC", 0),
            0o600,
        )
        os.fchmod(descriptor, 0o600)
        clear_inherited_acl(
            temporary,
            os.fstat(descriptor),
            "terminal-unhealthy staging file",
        )
        offset = 0
        while offset < len(body):
            written = os.write(descriptor, body[offset:])
            if written <= 0:
                raise OSError("short terminal-unhealthy fingerprint write")
            offset += written
        temporary_info = os.fstat(descriptor)
        if (
            not stat.S_ISREG(temporary_info.st_mode)
            or temporary_info.st_uid != os.geteuid()
            or temporary_info.st_nlink != 1
            or stat.S_IMODE(temporary_info.st_mode) != 0o600
            or temporary_info.st_size != len(body)
        ):
            raise IdentityError("terminal-unhealthy staging file has unsafe metadata")
        os.fsync(descriptor)
        staged_inode = (
            temporary_info.st_dev,
            temporary_info.st_ino,
            temporary_info.st_size,
        )
        os.close(descriptor)
        descriptor = -1
        try:
            os.link(temporary, path, follow_symlinks=False)
        except FileExistsError:
            concurrent = read_terminal_payload(path)
            if concurrent is None:
                raise IdentityError(
                    "terminal-unhealthy publication raced with replacement"
                )
            payload, info = concurrent
            if (
                terminal_payload(
                    payload["binding_sha256"],
                    payload["fatal_fingerprint_sha256"],
                )
                != body
            ):
                raise IdentityError(
                    "terminal-unhealthy publication raced with replacement"
                )
            return info
        published_created = True
        fsync_directory(path.parent)
        assert temporary_info is not None
        linked_temporary = temporary.lstat()
        if (
            linked_temporary.st_dev,
            linked_temporary.st_ino,
            linked_temporary.st_size,
            linked_temporary.st_nlink,
        ) != (
            temporary_info.st_dev,
            temporary_info.st_ino,
            temporary_info.st_size,
            2,
        ):
            raise IdentityError(
                "terminal-unhealthy staging identity changed during publication"
            )
        temporary_info = linked_temporary
        unlink_exact(
            temporary,
            linked_temporary,
            "terminal-unhealthy staging file",
        )
        temporary_info = None
        fsync_directory(path.parent)
        published = path.lstat()
        assert staged_inode is not None
        if (
            published.st_dev,
            published.st_ino,
            published.st_size,
        ) != staged_inode:
            raise IdentityError(
                "terminal-unhealthy fingerprint changed after publication"
            )
        if (
            not stat.S_ISREG(published.st_mode)
            or published.st_uid != os.geteuid()
            or published.st_nlink != 1
            or stat.S_IMODE(published.st_mode) != 0o600
        ):
            raise IdentityError(
                "terminal-unhealthy fingerprint has unsafe published metadata"
            )
        decoded = read_terminal_payload(path)
        if decoded is None or decoded[0] != decode_terminal_payload(body):
            raise IdentityError(
                "terminal-unhealthy fingerprint failed publication verification"
            )
        publication_complete = True
        return decoded[1]
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        if staged_inode is not None:
            try:
                current_temporary = temporary.lstat()
            except FileNotFoundError:
                pass
            else:
                if (
                    current_temporary.st_dev,
                    current_temporary.st_ino,
                    current_temporary.st_size,
                ) != staged_inode:
                    raise IdentityError(
                        "terminal-unhealthy staging file changed before cleanup"
                    )
                unlink_exact(
                    temporary,
                    current_temporary,
                    "terminal-unhealthy staging file",
                )
                fsync_directory(path.parent)
        if published_created and not publication_complete:
            try:
                current_published = path.lstat()
            except FileNotFoundError:
                pass
            else:
                assert staged_inode is not None
                if (
                    current_published.st_dev,
                    current_published.st_ino,
                    current_published.st_size,
                ) != staged_inode:
                    raise IdentityError(
                        "terminal-unhealthy fingerprint changed before rollback"
                    )
                unlink_exact(
                    path,
                    current_published,
                    "terminal-unhealthy fingerprint",
                )
                fsync_directory(path.parent)


def normalize_fatal_exit(
    return_code: int, uptime_seconds: float, rapid_limit_seconds: float, stderr: bytes
) -> str | None:
    """Return a redaction-safe digest for one rapid, explicit fatal exit."""

    if return_code <= 0 or uptime_seconds > rapid_limit_seconds or not stderr:
        return None
    text = stderr[-FATAL_STDERR_TAIL_MAX_BYTES:].decode("utf-8", errors="replace")
    fatal_lines: list[str] = []
    for raw_line in text.splitlines():
        line = ANSI_ESCAPE_RE.sub("", raw_line)
        line = "".join(
            character if character.isprintable() else " " for character in line
        )
        if FATAL_LINE_RE.search(line) is None:
            continue
        line = TRACING_TIMESTAMP_RE.sub("<timestamp>", line)
        line = ABSOLUTE_PATH_RE.sub("<path>", line)
        line = UUID_RE.sub("<uuid>", line)
        line = LONG_HEX_RE.sub("<hex>", line)
        line = HIGH_ENTROPY_TOKEN_RE.sub("<token>", line)
        line = DECIMAL_RE.sub("<n>", line)
        line = " ".join(line.lower().split())
        if line:
            fatal_lines.append(line[:512])
    if not fatal_lines:
        return None
    signature = (f"rc={return_code}\n" + "\n".join(fatal_lines[-8:])).encode("utf-8")[
        :FATAL_SIGNATURE_MAX_BYTES
    ]
    return hashlib.sha256(signature).hexdigest()


class RapidFatalExitTracker:
    """Count only consecutive identical normalized rapid fatal exits."""

    def __init__(self) -> None:
        self.fingerprint: str | None = None
        self.hits = 0

    def observe(self, fingerprint: str | None) -> bool:
        """Record one exit and report whether the three-hit latch must close."""

        if fingerprint is None:
            self.fingerprint = None
            self.hits = 0
            return False
        if fingerprint == self.fingerprint:
            self.hits += 1
        else:
            self.fingerprint = fingerprint
            self.hits = 1
        return self.hits >= RAPID_FATAL_EXIT_LIMIT


class BoundedStderrCapture:
    """Drain child stderr without unbounded memory while preserving normal logs."""

    def __init__(self, stream: Any) -> None:
        self.stream = stream
        self.descriptor = stream.fileno()
        os.set_blocking(self.descriptor, False)
        self.buffer = bytearray()

    def start(self) -> None:
        """Retain the compatibility hook used immediately after ``Popen``."""

    def _drain(self) -> None:
        while True:
            try:
                chunk = os.read(self.descriptor, 4096)
            except BlockingIOError:
                return
            except OSError:
                return
            if not chunk:
                return
            try:
                offset = 0
                while offset < len(chunk):
                    written = os.write(2, chunk[offset:])
                    if written <= 0:
                        raise OSError("short stderr forwarding write")
                    offset += written
            except OSError:
                pass
            self.buffer.extend(chunk)
            excess = len(self.buffer) - FATAL_STDERR_TAIL_MAX_BYTES
            if excess > 0:
                del self.buffer[:excess]

    def wait(self, child: subprocess.Popen[bytes]) -> int:
        """Wait while continuously draining the nonblocking child pipe."""

        while True:
            self._drain()
            return_code = child.poll()
            if return_code is not None:
                self._drain()
                return return_code
            time.sleep(0.01)

    def finish(self) -> bytes:
        """Close after child exit and return only the bounded stderr tail."""

        self._drain()
        self.stream.close()
        return bytes(self.buffer)


def forward_restart_to_child(child: subprocess.Popen[bytes] | None) -> None:
    """Forward a capture-authority restart request only to our live child."""

    if child is None or child.poll() is not None:
        return
    try:
        child.send_signal(signal.SIGTERM)
    except ProcessLookupError:
        pass


def sha256_file(path: Path) -> str:
    """Return the SHA-256 digest of a regular file without following symlinks."""

    before = path.lstat()
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise IdentityError(f"expected a non-symlink regular file: {path}")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        digest = hashlib.sha256()
        while chunk := os.read(descriptor, 1024 * 1024):
            digest.update(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if (before.st_dev, before.st_ino, before.st_size) != (
        after.st_dev,
        after.st_ino,
        after.st_size,
    ):
        raise IdentityError(f"file changed while hashing: {path}")
    return digest.hexdigest()


def binary_stat_seal(
    args: argparse.Namespace,
) -> tuple[int, int, int, int, int] | None:
    """Return the optional all-or-none binary stat seal."""

    values = tuple(getattr(args, field, None) for field in BINARY_STAT_SEAL_FIELDS)
    present = tuple(value is not None for value in values)
    if not any(present):
        return None
    if not all(present):
        raise IdentityError("binary stat seal fields must be provided together")
    device, inode, size, mtime_ns, ctime_ns = values
    if (
        not isinstance(device, int)
        or device < 0
        or not isinstance(inode, int)
        or inode <= 0
        or not isinstance(size, int)
        or size < 0
        or not isinstance(mtime_ns, int)
        or mtime_ns < 0
        or not isinstance(ctime_ns, int)
        or ctime_ns < 0
    ):
        raise IdentityError("binary stat seal metadata is invalid")
    return device, inode, size, mtime_ns, ctime_ns


def require_trusted_binary_path(path: Path) -> None:
    """Require a root-owned path that the runtime user cannot rename or rewrite."""

    if not path.is_absolute() or ".." in path.parts:
        raise IdentityError(
            f"stat-sealed validator binary path is not canonical and absolute: {path}"
        )
    components = [*reversed(path.parents), path]
    for index, component in enumerate(components):
        info = component.lstat()
        if stat.S_ISLNK(info.st_mode):
            raise IdentityError(
                f"stat-sealed validator binary path contains a symlink: {component}"
            )
        if index + 1 == len(components):
            if not stat.S_ISREG(info.st_mode):
                raise IdentityError(
                    f"stat-sealed validator binary is not a regular file: {component}"
                )
        elif not stat.S_ISDIR(info.st_mode):
            raise IdentityError(
                f"stat-sealed validator binary ancestor is not a directory: {component}"
            )
        if info.st_uid != 0:
            raise IdentityError(
                f"stat-sealed validator binary path is not root-owned: {component}"
            )
        if stat.S_IMODE(info.st_mode) & 0o022:
            raise IdentityError(
                "stat-sealed validator binary path is group/world writable: "
                f"{component}"
            )
        require_acl_free_path(component, "stat-sealed validator binary path")


def require_binary_stat_identity(
    path: Path, expected: tuple[int, int, int, int, int]
) -> None:
    """Validate an executable binary against an O(1) descriptor stat seal."""

    require_trusted_binary_path(path)
    before = path.lstat()
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise IdentityError(f"expected a non-symlink regular file: {path}")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    before_identity = (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    )
    after_identity = (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    )
    if not stat.S_ISREG(after.st_mode) or before_identity != after_identity:
        raise IdentityError(f"validator binary changed during stat validation: {path}")
    if after_identity != expected:
        raise IdentityError(f"validator binary stat identity changed: {path}")
    if not after.st_mode & 0o111:
        raise IdentityError(f"validator binary is not executable: {path}")


def require_runtime_identity(args: argparse.Namespace) -> None:
    """Refuse when binary, config, working-directory, or storage identity drifted."""

    binary = Path(args.binary)
    config = Path(args.config)
    workdir = Path(args.workdir)
    storage_dir = Path(args.storage_dir)
    stat_seal = binary_stat_seal(args)
    if stat_seal is None:
        if sha256_file(binary) != args.binary_sha256:
            raise IdentityError(f"validator binary digest changed: {binary}")
        if not os.access(binary, os.X_OK):
            raise IdentityError(f"validator binary is not executable: {binary}")
    else:
        require_binary_stat_identity(binary, stat_seal)
    if sha256_file(config) != args.config_sha256:
        raise IdentityError(f"validator config digest changed: {config}")
    workdir_stat = workdir.lstat()
    if stat.S_ISLNK(workdir_stat.st_mode) or not stat.S_ISDIR(workdir_stat.st_mode):
        raise IdentityError(f"storage path is not a non-symlink directory: {workdir}")
    if (
        workdir_stat.st_dev != args.workdir_device
        or workdir_stat.st_ino != args.workdir_inode
    ):
        raise IdentityError(f"working directory identity changed: {workdir}")
    storage_stat = storage_dir.lstat()
    if stat.S_ISLNK(storage_stat.st_mode) or not stat.S_ISDIR(storage_stat.st_mode):
        raise IdentityError(
            f"storage path is not a non-symlink directory: {storage_dir}"
        )
    if (
        storage_stat.st_dev != args.storage_device
        or storage_stat.st_ino != args.storage_inode
    ):
        raise IdentityError(f"storage directory identity changed: {storage_dir}")


def atomic_write_pid(path: Path, pid: int) -> None:
    """Atomically publish the currently supervised validator PID."""

    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    descriptor = os.open(
        temporary,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
        0o600,
    )
    try:
        body = f"{pid}\n".encode("ascii")
        offset = 0
        while offset < len(body):
            offset += os.write(descriptor, body[offset:])
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    os.replace(temporary, path)


def remove_owned_pid(path: Path, pid: int) -> None:
    """Remove a PID file only when it still names this supervisor's child."""

    try:
        current = path.read_text(encoding="ascii").strip()
    except FileNotFoundError:
        return
    if current == str(pid):
        path.unlink(missing_ok=True)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse the launchd-owned single-peer supervisor arguments."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--binary-device", type=int)
    parser.add_argument("--binary-inode", type=int)
    parser.add_argument("--binary-size", type=int)
    parser.add_argument("--binary-mtime-ns", type=int)
    parser.add_argument("--binary-ctime-ns", type=int)
    parser.add_argument("--config", required=True)
    parser.add_argument("--config-sha256", required=True)
    parser.add_argument("--workdir", required=True)
    parser.add_argument("--workdir-device", required=True, type=int)
    parser.add_argument("--workdir-inode", required=True, type=int)
    parser.add_argument("--storage-dir", required=True)
    parser.add_argument("--storage-device", required=True, type=int)
    parser.add_argument("--storage-inode", required=True, type=int)
    parser.add_argument("--pid-file", required=True)
    parser.add_argument("--terminal-unhealthy-file", required=True)
    parser.add_argument("--restart-generation", required=True)
    parser.add_argument("--initial-backoff-seconds", type=float, default=1.0)
    parser.add_argument("--maximum-backoff-seconds", type=float, default=30.0)
    parser.add_argument("--stable-uptime-seconds", type=float, default=120.0)
    parser.add_argument(
        "--rapid-fatal-uptime-seconds",
        type=float,
        default=DEFAULT_RAPID_FATAL_UPTIME_SECONDS,
    )
    args = parser.parse_args(argv)
    try:
        binary_stat_seal(args)
    except IdentityError as exc:
        parser.error(str(exc))
    if re.fullmatch(r"[0-9a-f]{64}", args.binary_sha256) is None:
        parser.error("--binary-sha256 must be one lowercase SHA-256 digest")
    if re.fullmatch(r"[0-9a-f]{64}", args.config_sha256) is None:
        parser.error("--config-sha256 must be one lowercase SHA-256 digest")
    if (
        not math.isfinite(args.initial_backoff_seconds)
        or args.initial_backoff_seconds <= 0
    ):
        parser.error("--initial-backoff-seconds must be positive")
    if (
        not math.isfinite(args.maximum_backoff_seconds)
        or args.maximum_backoff_seconds < args.initial_backoff_seconds
    ):
        parser.error("--maximum-backoff-seconds must be at least the initial backoff")
    if not math.isfinite(args.stable_uptime_seconds) or args.stable_uptime_seconds <= 0:
        parser.error("--stable-uptime-seconds must be positive")
    if (
        not math.isfinite(args.rapid_fatal_uptime_seconds)
        or args.rapid_fatal_uptime_seconds <= 0
    ):
        parser.error("--rapid-fatal-uptime-seconds must be positive")
    if re.fullmatch(r"[0-9a-f]{64}", args.restart_generation) is None:
        parser.error("--restart-generation must be one lowercase SHA-256 digest")
    terminal_file = Path(args.terminal_unhealthy_file)
    pid_file = Path(args.pid_file)
    if (
        not terminal_file.is_absolute()
        or ".." in terminal_file.parts
        or terminal_file == pid_file
    ):
        parser.error(
            "--terminal-unhealthy-file must be a distinct canonical absolute path"
        )
    return args


def run(argv: list[str] | None = None) -> int:
    """Run the per-peer restart loop until launchd asks it to stop."""

    args = parse_args(argv)
    pid_file = Path(args.pid_file)
    pid_file.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    terminal_file = Path(args.terminal_unhealthy_file)
    terminal_file.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    stopping_signal: int | None = None
    restart_requested = False
    child: subprocess.Popen[bytes] | None = None

    def request_stop(signum: int, _frame: FrameType | None) -> None:
        nonlocal stopping_signal
        stopping_signal = signum
        if child is not None and child.poll() is None:
            try:
                child.send_signal(signum)
            except ProcessLookupError:
                pass

    def request_restart(_signum: int, _frame: FrameType | None) -> None:
        nonlocal restart_requested
        if stopping_signal is not None:
            return
        restart_requested = True
        forward_restart_to_child(child)

    signal.signal(signal.SIGTERM, request_stop)
    signal.signal(signal.SIGINT, request_stop)
    signal.signal(signal.SIGHUP, request_stop)
    signal.signal(signal.SIGUSR1, request_restart)

    def hold_terminal_unhealthy(fingerprint: str | None) -> int:
        if fingerprint is None:
            message = "taira supervisor terminal-unhealthy publication refusal"
        else:
            message = (
                "taira supervisor terminal-unhealthy "
                f"fatal_fingerprint_sha256={fingerprint}"
            )
        print(message, file=sys.stderr, flush=True)
        while stopping_signal is None:
            time.sleep(0.25)
        return 0

    try:
        require_runtime_identity(args)
    except (IdentityError, OSError) as exc:
        print(f"taira supervisor identity refusal: {exc}", file=sys.stderr, flush=True)
        return 78
    binding = terminal_binding_sha256(args)
    try:
        if existing_terminal_latch(terminal_file, binding):
            persisted = read_terminal_payload(terminal_file)
            assert persisted is not None
            return hold_terminal_unhealthy(
                str(persisted[0]["fatal_fingerprint_sha256"])
            )
    except (IdentityError, OSError):
        return hold_terminal_unhealthy(None)

    backoff = args.initial_backoff_seconds
    fatal_tracker = RapidFatalExitTracker()
    while stopping_signal is None:
        try:
            require_runtime_identity(args)
        except (IdentityError, OSError) as exc:
            print(
                f"taira supervisor identity refusal: {exc}", file=sys.stderr, flush=True
            )
            return 78

        if stopping_signal is not None:
            break
        started = time.monotonic()
        try:
            child = subprocess.Popen(
                [args.binary, "--sora", "--config", args.config],
                cwd=args.workdir,
                stderr=subprocess.PIPE,
                bufsize=0,
            )
        except OSError as exc:
            print(
                "taira validator spawn failed "
                f"error={exc!s} restart_in_seconds={backoff:.3f}",
                file=sys.stderr,
                flush=True,
            )
            fatal_tracker.observe(None)
            deadline = time.monotonic() + backoff
            while stopping_signal is None:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    break
                time.sleep(min(remaining, 0.25))
            backoff = min(args.maximum_backoff_seconds, backoff * 2)
            continue
        assert child.stderr is not None
        stderr_capture = BoundedStderrCapture(child.stderr)
        stderr_capture.start()
        if stopping_signal is not None:
            try:
                child.send_signal(stopping_signal)
            except ProcessLookupError:
                pass
        elif restart_requested:
            forward_restart_to_child(child)
        try:
            atomic_write_pid(pid_file, child.pid)
        except OSError:
            try:
                child.terminate()
            except ProcessLookupError:
                pass
            stderr_capture.wait(child)
            stderr_capture.finish()
            raise
        print(f"taira validator started pid={child.pid}", flush=True)
        return_code = stderr_capture.wait(child)
        stderr_tail = stderr_capture.finish()
        uptime = time.monotonic() - started
        remove_owned_pid(pid_file, child.pid)
        child = None

        if stopping_signal is not None:
            print(
                f"taira validator stopped signal={stopping_signal} rc={return_code}",
                flush=True,
            )
            return 0

        if restart_requested:
            restart_requested = False
            fatal_tracker.observe(None)
            backoff = args.initial_backoff_seconds
            print(
                "taira validator restart requested by capture authority",
                flush=True,
            )
            continue

        if uptime >= args.stable_uptime_seconds:
            backoff = args.initial_backoff_seconds
        fatal_fingerprint = normalize_fatal_exit(
            return_code,
            uptime,
            args.rapid_fatal_uptime_seconds,
            stderr_tail,
        )
        if fatal_tracker.observe(fatal_fingerprint):
            assert fatal_fingerprint is not None
            try:
                publish_terminal_payload(
                    terminal_file,
                    binding,
                    fatal_fingerprint,
                )
            except (IdentityError, OSError):
                return hold_terminal_unhealthy(None)
            return hold_terminal_unhealthy(fatal_fingerprint)
        print(
            "taira validator exited "
            f"rc={return_code} uptime_seconds={uptime:.3f} "
            f"restart_in_seconds={backoff:.3f}",
            file=sys.stderr,
            flush=True,
        )
        deadline = time.monotonic() + backoff
        while stopping_signal is None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                break
            time.sleep(min(remaining, 0.25))
        backoff = min(args.maximum_backoff_seconds, backoff * 2)
    return 0


if __name__ == "__main__":
    raise SystemExit(run())
