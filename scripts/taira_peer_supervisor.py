#!/usr/bin/env python3
"""Supervise exactly one Taira validator process.

This helper is installed by ``migrate_taira_peer_supervision.py`` and is not
intended to be started by hand.  It preserves the validated binary, config,
and storage-directory identities from the migration plan, forwards shutdown
signals to the validator, and applies bounded exponential restart backoff. An
explicit peer identity can also enable a durable local lifecycle journal for a
separately protected four-peer evidence collector.
"""

from __future__ import annotations

import argparse
import fcntl
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
from typing import Any, Callable


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
LIFECYCLE_STATE_SCHEMA = "iroha.taira.peer-supervisor-lifecycle-state.v1"
LIFECYCLE_RAW_WINDOW_SCHEMA = "iroha.taira.peer-supervisor-raw-window.v1"
LIFECYCLE_CHAIN_DOMAIN = b"iroha.taira.peer-supervisor-lifecycle-chain.v1\0"
LIFECYCLE_BINDING_DOMAIN = b"iroha.taira.peer-supervisor-lifecycle-binding.v1\0"
LIFECYCLE_STATE_MAX_BYTES = 16 * 1024
LIFECYCLE_JOURNAL_MAX_BYTES = 16 * 1024 * 1024
LIFECYCLE_RECORD_MAX_BYTES = 4 * 1024
DEFAULT_LIFECYCLE_HEALTHY_INTERVAL_SECONDS = 60.0
LIFECYCLE_STATE_FIELDS = {
    "schema",
    "schema_version",
    "binding_sha256",
    "validator_id",
    "node_id",
    "restart_generation",
    "supervisor_generation",
    "process_generation",
    "restart_count",
    "unexpected_exit_total",
    "journal_sequence",
    "journal_chain_sha256",
    "journal_record_count",
    "journal_size_bytes",
    "journal_sha256",
    "pending_record",
}
LIFECYCLE_RECORD_FIELDS = {
    "index",
    "journal_sequence",
    "observed_at_unix_ms",
    "validator_id",
    "node_id",
    "event",
    "restart_count",
    "supervisor_generation",
    "process_generation",
    "unexpected_exit_total",
}
LIFECYCLE_CHECKPOINT_FIELDS = {
    "captured_at_unix_ms",
    "journal_sequence",
    "journal_chain_sha256",
    "validators",
}
LIFECYCLE_RAW_WINDOW_FIELDS = {
    "schema",
    "schema_version",
    "binding_sha256",
    "validator_id",
    "node_id",
    "baseline",
    "terminal",
    "record_count",
    "records_sha256",
}
LIFECYCLE_VALIDATOR_FIELDS = {
    "validator_id",
    "node_id",
    "restart_count",
    "supervisor_generation",
    "process_generation",
    "unexpected_exit_total",
}
LIFECYCLE_EVENTS = frozenset({"healthy", "restart", "unexpected_exit"})
LIFECYCLE_VALIDATOR_RE = re.compile(r"taira-validator-[1-4]")
LIFECYCLE_NODE_ID_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:@+-]{7,255}")
LOWER_SHA256_RE = re.compile(r"[0-9a-f]{64}")


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


def canonical_json_line(value: object) -> bytes:
    """Encode one bounded, ASCII, canonical JSON line."""

    return (
        json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("ascii")


def lifecycle_binding_sha256(
    args: argparse.Namespace, validator_id: str, node_id: str
) -> str:
    """Bind one local journal to the exact deployed peer identity."""

    payload = {
        "node_id": node_id,
        "restart_generation": args.restart_generation,
        "runtime_binding_sha256": terminal_binding_sha256(args),
        "schema": LIFECYCLE_STATE_SCHEMA,
        "validator_id": validator_id,
    }
    return hashlib.sha256(
        LIFECYCLE_BINDING_DOMAIN + canonical_json_line(payload)
    ).hexdigest()


def _lifecycle_initial_chain(binding_sha256: str) -> str:
    """Return the domain-separated chain root for one peer binding."""

    return hashlib.sha256(
        LIFECYCLE_CHAIN_DOMAIN + bytes.fromhex(binding_sha256)
    ).hexdigest()


def _lifecycle_next_chain(prior: str, record: dict[str, object]) -> str:
    """Extend the local journal chain with one canonical verifier-shaped row."""

    return hashlib.sha256(
        LIFECYCLE_CHAIN_DOMAIN
        + bytes.fromhex(prior)
        + canonical_json_line(record)
    ).hexdigest()


def _require_nonnegative_integer(value: object, label: str) -> int:
    """Return one exact nonnegative integer, rejecting booleans."""

    if type(value) is not int or value < 0:
        raise IdentityError(f"{label} is not an exact nonnegative integer")
    return value


def _require_positive_integer(value: object, label: str) -> int:
    """Return one exact positive integer, rejecting booleans."""

    result = _require_nonnegative_integer(value, label)
    if result == 0:
        raise IdentityError(f"{label} is not positive")
    return result


def _require_lifecycle_regular_file(
    path: Path, label: str, maximum_bytes: int
) -> tuple[bytes, os.stat_result]:
    """Read one stable owner-private lifecycle file without following links."""

    before = require_acl_free_path(path, label)
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_uid != os.geteuid()
        or stat.S_IMODE(before.st_mode) != 0o600
        or before.st_nlink != 1
        or before.st_size > maximum_bytes
    ):
        raise IdentityError(f"{label} has unsafe metadata")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        opened = os.fstat(descriptor)
        if exact_file_identity(opened) != exact_file_identity(before):
            raise IdentityError(f"{label} changed while opening")
        body = bytearray()
        while len(body) <= maximum_bytes:
            chunk = os.read(descriptor, min(64 * 1024, maximum_bytes + 1 - len(body)))
            if not chunk:
                break
            body.extend(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    named = path.lstat()
    if (
        len(body) > maximum_bytes
        or exact_file_identity(after) != exact_file_identity(opened)
        or exact_file_identity(named) != exact_file_identity(after)
    ):
        raise IdentityError(f"{label} changed while reading")
    return bytes(body), after


def _publish_lifecycle_file(
    path: Path,
    body: bytes,
    label: str,
    maximum_bytes: int,
    *,
    allow_empty: bool = False,
) -> str:
    """Atomically publish and durably verify one owner-private lifecycle file."""

    if (not body and not allow_empty) or len(body) > maximum_bytes:
        raise IdentityError(f"{label} has an invalid publication size")
    parent_info = require_private_directory(path.parent, f"{label} directory")
    try:
        _require_lifecycle_regular_file(path, label, maximum_bytes)
    except FileNotFoundError:
        pass
    temporary = path.with_name(f".{path.name}.{os.getpid()}.{time.time_ns()}.tmp")
    descriptor = os.open(
        temporary,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
        0o600,
    )
    published = False
    try:
        os.fchmod(descriptor, 0o600)
        temporary_info = os.fstat(descriptor)
        clear_inherited_acl(temporary, temporary_info, f"{label} staging file")
        offset = 0
        while offset < len(body):
            written = os.write(descriptor, body[offset:])
            if written <= 0:
                raise OSError(f"short {label} write")
            offset += written
        os.fsync(descriptor)
        staged = os.fstat(descriptor)
        if (
            staged.st_uid != os.geteuid()
            or stat.S_IMODE(staged.st_mode) != 0o600
            or staged.st_nlink != 1
            or staged.st_size != len(body)
        ):
            raise IdentityError(f"{label} staging file has unsafe metadata")
        current_parent = require_private_directory(path.parent, f"{label} directory")
        if (current_parent.st_dev, current_parent.st_ino) != (
            parent_info.st_dev,
            parent_info.st_ino,
        ):
            raise IdentityError(f"{label} directory changed during publication")
        os.replace(temporary, path)
        published = True
        fsync_directory(path.parent)
    finally:
        os.close(descriptor)
        if not published:
            try:
                staged = temporary.lstat()
            except FileNotFoundError:
                pass
            else:
                if (staged.st_dev, staged.st_ino) == (
                    temporary_info.st_dev,
                    temporary_info.st_ino,
                ):
                    temporary.unlink()
                    fsync_directory(path.parent)
    actual, _ = _require_lifecycle_regular_file(path, label, maximum_bytes)
    if actual != body:
        raise IdentityError(f"{label} failed publication verification")
    return hashlib.sha256(body).hexdigest()


def _open_lifecycle_lock(
    root: Path, filename: str, label: str, *, nonblocking: bool
) -> tuple[int, os.stat_result]:
    """Open and acquire one stable owner-private lifecycle lock inode."""

    path = root / filename
    existed = True
    try:
        before = require_acl_free_path(path, label)
    except FileNotFoundError:
        existed = False
        before = None
    if before is not None and (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_uid != os.geteuid()
        or stat.S_IMODE(before.st_mode) != 0o600
        or before.st_nlink != 1
        or before.st_size != 0
    ):
        raise IdentityError(f"{label} has unsafe metadata")
    descriptor = os.open(
        path,
        os.O_RDWR
        | os.O_CREAT
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
        0o600,
    )
    try:
        opened = os.fstat(descriptor)
        if not existed:
            os.fchmod(descriptor, 0o600)
            os.fsync(descriptor)
            clear_inherited_acl(path, opened, label)
            fsync_directory(root)
            opened = os.fstat(descriptor)
        named = require_acl_free_path(path, label)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_uid != os.geteuid()
            or stat.S_IMODE(opened.st_mode) != 0o600
            or opened.st_nlink != 1
            or opened.st_size != 0
            or (opened.st_dev, opened.st_ino) != (named.st_dev, named.st_ino)
        ):
            raise IdentityError(f"{label} changed while opening")
        flags = fcntl.LOCK_EX | (fcntl.LOCK_NB if nonblocking else 0)
        try:
            fcntl.flock(descriptor, flags)
        except BlockingIOError as error:
            raise IdentityError(f"{label} is already held") from error
        named = path.lstat()
        current = os.fstat(descriptor)
        if (current.st_dev, current.st_ino) != (named.st_dev, named.st_ino):
            raise IdentityError(f"{label} path changed while locking")
        return descriptor, current
    except BaseException:
        os.close(descriptor)
        raise


def _confirm_lifecycle_path(
    path: Path, descriptor: int, expected: os.stat_result, label: str
) -> None:
    """Require a held lock descriptor to remain the exact named inode."""

    opened = os.fstat(descriptor)
    named = path.lstat()
    if (
        (opened.st_dev, opened.st_ino) != (expected.st_dev, expected.st_ino)
        or (named.st_dev, named.st_ino) != (expected.st_dev, expected.st_ino)
        or opened.st_nlink != 1
        or named.st_nlink != 1
    ):
        raise IdentityError(f"{label} path changed while active")


def _decode_lifecycle_state(body: bytes) -> dict[str, object]:
    """Decode one exact canonical local lifecycle state snapshot."""

    if not body or len(body) > LIFECYCLE_STATE_MAX_BYTES:
        raise IdentityError("lifecycle state has an invalid size")
    try:
        value = json.loads(body)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise IdentityError("lifecycle state is not canonical JSON") from error
    if (
        not isinstance(value, dict)
        or set(value) != LIFECYCLE_STATE_FIELDS
        or canonical_json_line(value) != body
        or value.get("schema") != LIFECYCLE_STATE_SCHEMA
        or type(value.get("schema_version")) is not int
        or value.get("schema_version") != 1
    ):
        raise IdentityError("lifecycle state is not canonical")
    for field in ("binding_sha256", "restart_generation", "journal_chain_sha256", "journal_sha256"):
        if not isinstance(value[field], str) or LOWER_SHA256_RE.fullmatch(value[field]) is None:
            raise IdentityError(f"lifecycle state {field} is invalid")
    if (
        not isinstance(value["validator_id"], str)
        or LIFECYCLE_VALIDATOR_RE.fullmatch(value["validator_id"]) is None
        or not isinstance(value["node_id"], str)
        or LIFECYCLE_NODE_ID_RE.fullmatch(value["node_id"]) is None
    ):
        raise IdentityError("lifecycle state peer identity is invalid")
    for field in (
        "supervisor_generation",
        "process_generation",
        "restart_count",
        "unexpected_exit_total",
        "journal_sequence",
        "journal_record_count",
        "journal_size_bytes",
    ):
        _require_nonnegative_integer(value[field], f"lifecycle state {field}")
    if value["supervisor_generation"] == 0:
        raise IdentityError("lifecycle supervisor generation is not positive")
    pending = value["pending_record"]
    if pending is not None and (
        not isinstance(pending, dict)
        or set(pending) != {"record", "journal_chain_sha256"}
        or not isinstance(pending["record"], dict)
        or set(pending["record"]) != LIFECYCLE_RECORD_FIELDS
        or not isinstance(pending["journal_chain_sha256"], str)
        or LOWER_SHA256_RE.fullmatch(pending["journal_chain_sha256"]) is None
    ):
        raise IdentityError("lifecycle pending record is invalid")
    return value


def _decode_lifecycle_records(body: bytes) -> list[dict[str, object]]:
    """Decode an exact canonical local record stream."""

    if len(body) > LIFECYCLE_JOURNAL_MAX_BYTES:
        raise IdentityError("lifecycle journal exceeds its bound")
    if body and not body.endswith(b"\n"):
        raise IdentityError("lifecycle journal has a partial record")
    records: list[dict[str, object]] = []
    for index, line in enumerate(body.splitlines(keepends=True)):
        if len(line) > LIFECYCLE_RECORD_MAX_BYTES:
            raise IdentityError("lifecycle journal record exceeds its bound")
        try:
            value = json.loads(line)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise IdentityError("lifecycle journal record is not JSON") from error
        if (
            not isinstance(value, dict)
            or set(value) != LIFECYCLE_RECORD_FIELDS
            or canonical_json_line(value) != line
        ):
            raise IdentityError("lifecycle journal record is not canonical")
        if _require_nonnegative_integer(value["index"], "lifecycle record index") != index:
            raise IdentityError("lifecycle journal indexes are not contiguous")
        if _require_positive_integer(
            value["journal_sequence"], "lifecycle record sequence"
        ) != index + 1:
            raise IdentityError("lifecycle journal sequences are not contiguous")
        _require_positive_integer(
            value["observed_at_unix_ms"], "lifecycle record observation"
        )
        if (
            not isinstance(value["validator_id"], str)
            or LIFECYCLE_VALIDATOR_RE.fullmatch(value["validator_id"]) is None
            or not isinstance(value["node_id"], str)
            or LIFECYCLE_NODE_ID_RE.fullmatch(value["node_id"]) is None
            or value["event"] not in LIFECYCLE_EVENTS
        ):
            raise IdentityError("lifecycle journal record identity is invalid")
        for field in (
            "restart_count",
            "supervisor_generation",
            "process_generation",
            "unexpected_exit_total",
        ):
            _require_nonnegative_integer(value[field], f"lifecycle record {field}")
        if value["supervisor_generation"] == 0 or value["process_generation"] == 0:
            raise IdentityError("lifecycle record generations are not positive")
        records.append(value)
    return records


def _validate_lifecycle_snapshot(
    binding_sha256: str,
    validator_id: str,
    node_id: str,
    state: dict[str, object],
    body: bytes,
    records: list[dict[str, object]],
) -> None:
    """Require state, chain, records, generations, and counters to cohere."""

    digest = hashlib.sha256(body).hexdigest()
    if (
        state["journal_record_count"] != len(records)
        or state["journal_sequence"] != len(records)
        or state["journal_size_bytes"] != len(body)
        or state["journal_sha256"] != digest
    ):
        raise IdentityError("lifecycle state does not bind the journal bytes")
    chain = _lifecycle_initial_chain(binding_sha256)
    prior: dict[str, object] | None = None
    for record in records:
        if record["validator_id"] != validator_id or record["node_id"] != node_id:
            raise IdentityError("lifecycle journal peer identity changed")
        if prior is not None:
            if record["observed_at_unix_ms"] < prior["observed_at_unix_ms"]:
                raise IdentityError("lifecycle journal wall clock regressed")
            if record["supervisor_generation"] < prior["supervisor_generation"]:
                raise IdentityError("lifecycle supervisor generation regressed")
            expected_restart = prior["restart_count"]
            expected_process = prior["process_generation"]
            expected_exits = prior["unexpected_exit_total"]
            if record["event"] == "restart":
                expected_restart += 1
                expected_process += 1
            elif record["event"] == "unexpected_exit":
                expected_exits += 1
            if (
                record["restart_count"] != expected_restart
                or record["process_generation"] != expected_process
                or record["unexpected_exit_total"] != expected_exits
            ):
                raise IdentityError("lifecycle journal counters are incoherent")
        elif not (
            record["restart_count"] == 0
            and record["process_generation"] == 1
            and (
                (
                    record["event"] == "healthy"
                    and record["unexpected_exit_total"] == 0
                )
                or (
                    record["event"] == "unexpected_exit"
                    and record["unexpected_exit_total"] == 1
                )
            )
        ):
            raise IdentityError("lifecycle journal first process transition is invalid")
        chain = _lifecycle_next_chain(chain, record)
        prior = record
    if state["journal_chain_sha256"] != chain:
        raise IdentityError("lifecycle journal chain digest is wrong")
    if prior is None:
        if (
            state["process_generation"] != 0
            or state["restart_count"] != 0
            or state["unexpected_exit_total"] != 0
        ):
            raise IdentityError("empty lifecycle journal has nonzero counters")
    elif (
        state["process_generation"] != prior["process_generation"]
        or state["restart_count"] != prior["restart_count"]
        or state["unexpected_exit_total"] != prior["unexpected_exit_total"]
        or state["supervisor_generation"] < prior["supervisor_generation"]
    ):
        raise IdentityError("lifecycle state counters differ from its journal tip")


class LifecycleJournal:
    """Process-held, durable local journal for one supervised Taira peer.

    This journal is a collection input, not an observation authority. Its
    records deliberately match the public-soak verifier row schema, but a
    separately protected collector must capture four peers, construct the
    evidence window, and obtain the independent native-verifier receipt.
    """

    # TODO: Install the four-peer collector behind the protected public-soak
    # controller and provision the independent native verifier that signs off
    # on its globally resequenced journal and exact deployed-runtime bindings.

    OWNER_LOCK = "owner.lock"
    STATE_LOCK = "state.lock"
    STATE_FILE = "state.json"
    JOURNAL_FILE = "journal.jsonl"
    ENTRIES = frozenset({OWNER_LOCK, STATE_LOCK, STATE_FILE, JOURNAL_FILE})

    def __init__(
        self, root: Path, binding_sha256: str, validator_id: str, node_id: str,
        restart_generation: str,
    ) -> None:
        if not root.is_absolute() or ".." in root.parts:
            raise IdentityError("lifecycle journal root is not canonical and absolute")
        if LOWER_SHA256_RE.fullmatch(binding_sha256) is None:
            raise IdentityError("lifecycle binding is not a lowercase SHA-256 digest")
        if LIFECYCLE_VALIDATOR_RE.fullmatch(validator_id) is None:
            raise IdentityError("lifecycle validator ID is not canonical")
        if LIFECYCLE_NODE_ID_RE.fullmatch(node_id) is None:
            raise IdentityError("lifecycle node ID is not canonical")
        if LOWER_SHA256_RE.fullmatch(restart_generation) is None:
            raise IdentityError("lifecycle restart generation is invalid")
        try:
            root.mkdir(mode=0o700)
            fsync_directory(root.parent)
        except FileExistsError:
            pass
        root_info = require_private_directory(root, "lifecycle journal root")
        unexpected = {entry.name for entry in root.iterdir()} - self.ENTRIES
        if unexpected:
            raise IdentityError("lifecycle journal root contains unexpected entries")
        self.root = root
        self.root_info = root_info
        self.binding_sha256 = binding_sha256
        self.validator_id = validator_id
        self.node_id = node_id
        self.restart_generation = restart_generation
        self.owner_fd, self.owner_info = _open_lifecycle_lock(
            root, self.OWNER_LOCK, "lifecycle owner lock", nonblocking=True
        )
        try:
            self.state_fd, self.state_lock_info = _open_lifecycle_lock(
                root, self.STATE_LOCK, "lifecycle state lock", nonblocking=False
            )
        except BaseException:
            os.close(self.owner_fd)
            raise
        self.closed = False
        self._state_sha256 = ""
        try:
            self._lock_state()
            try:
                self._initialize_or_resume()
            finally:
                self._unlock_state()
        except BaseException:
            self.close()
            raise

    @property
    def state_path(self) -> Path:
        """Return the fixed local state path."""

        return self.root / self.STATE_FILE

    @property
    def journal_path(self) -> Path:
        """Return the fixed local record-stream path."""

        return self.root / self.JOURNAL_FILE

    def _confirm_lock_paths(self) -> None:
        """Require both held lock descriptors to remain the exact named inodes."""

        _confirm_lifecycle_path(
            self.root / self.OWNER_LOCK,
            self.owner_fd,
            self.owner_info,
            "lifecycle owner lock",
        )
        _confirm_lifecycle_path(
            self.root / self.STATE_LOCK,
            self.state_fd,
            self.state_lock_info,
            "lifecycle state lock",
        )

    def _confirm_root(self) -> None:
        current = require_private_directory(self.root, "lifecycle journal root")
        if (current.st_dev, current.st_ino) != (
            self.root_info.st_dev,
            self.root_info.st_ino,
        ):
            raise IdentityError("lifecycle journal root changed while active")
        if {entry.name for entry in self.root.iterdir()} != self.ENTRIES:
            raise IdentityError("lifecycle journal root entries changed while active")
        self._confirm_lock_paths()

    def _lock_state(self) -> None:
        if self.closed:
            raise IdentityError("lifecycle journal is closed")
        self._confirm_lock_paths()
        fcntl.flock(self.state_fd, fcntl.LOCK_EX)
        self._confirm_lock_paths()

    def _unlock_state(self) -> None:
        fcntl.flock(self.state_fd, fcntl.LOCK_UN)

    def _read_state(self) -> tuple[dict[str, object], bytes]:
        body, _ = _require_lifecycle_regular_file(
            self.state_path, "lifecycle state", LIFECYCLE_STATE_MAX_BYTES
        )
        if self._state_sha256 and hashlib.sha256(body).hexdigest() != self._state_sha256:
            raise IdentityError("lifecycle state changed outside the active writer")
        state = _decode_lifecycle_state(body)
        if (
            state["binding_sha256"] != self.binding_sha256
            or state["validator_id"] != self.validator_id
            or state["node_id"] != self.node_id
            or state["restart_generation"] != self.restart_generation
        ):
            raise IdentityError("lifecycle state binding changed")
        return state, body

    def _read_journal(self) -> tuple[bytes, list[dict[str, object]]]:
        body, _ = _require_lifecycle_regular_file(
            self.journal_path, "lifecycle journal", LIFECYCLE_JOURNAL_MAX_BYTES
        )
        return body, _decode_lifecycle_records(body)

    def _validate_committed(
        self, state: dict[str, object], body: bytes, records: list[dict[str, object]]
    ) -> None:
        _validate_lifecycle_snapshot(
            self.binding_sha256,
            self.validator_id,
            self.node_id,
            state,
            body,
            records,
        )

    def _publish_state(
        self, state: dict[str, object], *, initializing: bool = False
    ) -> None:
        body = canonical_json_line(state)
        if initializing:
            self._confirm_root_after_lock_creation()
        else:
            self._confirm_root()
        self._state_sha256 = _publish_lifecycle_file(
            self.state_path, body, "lifecycle state", LIFECYCLE_STATE_MAX_BYTES
        )
        self._confirm_root()

    def _initialize_or_resume(self) -> None:
        self._confirm_root_after_lock_creation()
        try:
            state_body, _ = _require_lifecycle_regular_file(
                self.state_path, "lifecycle state", LIFECYCLE_STATE_MAX_BYTES
            )
            journal_body, _ = _require_lifecycle_regular_file(
                self.journal_path, "lifecycle journal", LIFECYCLE_JOURNAL_MAX_BYTES
            )
        except FileNotFoundError:
            if self.state_path.exists() or self.journal_path.exists():
                raise IdentityError("lifecycle journal is only partially initialized")
            journal_body = b""
            _publish_lifecycle_file(
                self.journal_path,
                journal_body,
                "lifecycle journal",
                LIFECYCLE_JOURNAL_MAX_BYTES,
                allow_empty=True,
            )
            self._confirm_root_after_lock_creation()
            journal_sha256 = hashlib.sha256(b"").hexdigest()
            state = {
                "schema": LIFECYCLE_STATE_SCHEMA,
                "schema_version": 1,
                "binding_sha256": self.binding_sha256,
                "validator_id": self.validator_id,
                "node_id": self.node_id,
                "restart_generation": self.restart_generation,
                "supervisor_generation": 1,
                "process_generation": 0,
                "restart_count": 0,
                "unexpected_exit_total": 0,
                "journal_sequence": 0,
                "journal_chain_sha256": _lifecycle_initial_chain(
                    self.binding_sha256
                ),
                "journal_record_count": 0,
                "journal_size_bytes": 0,
                "journal_sha256": journal_sha256,
                "pending_record": None,
            }
            self._publish_state(state, initializing=True)
            return
        self._state_sha256 = hashlib.sha256(state_body).hexdigest()
        state = _decode_lifecycle_state(state_body)
        if (
            state["binding_sha256"] != self.binding_sha256
            or state["validator_id"] != self.validator_id
            or state["node_id"] != self.node_id
            or state["restart_generation"] != self.restart_generation
        ):
            raise IdentityError("lifecycle state belongs to another peer binding")
        records = _decode_lifecycle_records(journal_body)
        self._confirm_root()
        self._recover_pending(state, journal_body, records)
        state, _ = self._read_state()
        journal_body, records = self._read_journal()
        self._validate_committed(state, journal_body, records)
        state["supervisor_generation"] += 1
        self._publish_state(state)
        self._confirm_root()

    def _confirm_root_after_lock_creation(self) -> None:
        current = require_private_directory(self.root, "lifecycle journal root")
        if (current.st_dev, current.st_ino) != (
            self.root_info.st_dev,
            self.root_info.st_ino,
        ):
            raise IdentityError("lifecycle journal root changed during lock creation")
        entries = {entry.name for entry in self.root.iterdir()}
        if not entries.issubset(self.ENTRIES) or not {
            self.OWNER_LOCK,
            self.STATE_LOCK,
        }.issubset(entries):
            raise IdentityError("lifecycle journal root entries are unsafe")
        self._confirm_lock_paths()

    def _recover_pending(
        self,
        state: dict[str, object],
        journal_body: bytes,
        records: list[dict[str, object]],
    ) -> None:
        pending = state["pending_record"]
        if pending is None:
            self._validate_committed(state, journal_body, records)
            return
        assert isinstance(pending, dict)
        record = pending["record"]
        assert isinstance(record, dict)
        committed_size = state["journal_size_bytes"]
        committed_sha = state["journal_sha256"]
        assert isinstance(committed_size, int) and isinstance(committed_sha, str)
        record_line = canonical_json_line(record)
        if (
            len(journal_body) == committed_size
            and hashlib.sha256(journal_body).hexdigest() == committed_sha
        ):
            committed_records = records
            self._validate_committed(state, journal_body, committed_records)
            self._validate_pending_record(state, record, committed_records)
            next_body = journal_body + record_line
            _publish_lifecycle_file(
                self.journal_path,
                next_body,
                "lifecycle journal",
                LIFECYCLE_JOURNAL_MAX_BYTES,
            )
            self._confirm_root()
            journal_body = next_body
        elif (
            len(journal_body) == committed_size + len(record_line)
            and journal_body.endswith(record_line)
            and hashlib.sha256(journal_body[:-len(record_line)]).hexdigest()
            == committed_sha
        ):
            if not records or records[-1] != record:
                raise IdentityError("lifecycle pending record differs from journal tip")
            committed_records = records[:-1]
            self._validate_committed(
                state, journal_body[:-len(record_line)], committed_records
            )
            self._validate_pending_record(state, record, committed_records)
        else:
            raise IdentityError("lifecycle pending transition is irreconcilable")
        final_records = _decode_lifecycle_records(journal_body)
        if len(final_records) != len(committed_records) + 1 or final_records[-1] != record:
            raise IdentityError("lifecycle pending transition did not append exactly once")
        final = dict(state)
        final["supervisor_generation"] = record["supervisor_generation"]
        final["process_generation"] = record["process_generation"]
        final["restart_count"] = record["restart_count"]
        final["unexpected_exit_total"] = record["unexpected_exit_total"]
        final["journal_sequence"] = record["journal_sequence"]
        final["journal_chain_sha256"] = pending["journal_chain_sha256"]
        final["journal_record_count"] = len(final_records)
        final["journal_size_bytes"] = len(journal_body)
        final["journal_sha256"] = hashlib.sha256(journal_body).hexdigest()
        final["pending_record"] = None
        self._publish_state(final)

    def _validate_pending_record(
        self,
        state: dict[str, object],
        record: dict[str, object],
        records: list[dict[str, object]],
    ) -> None:
        """Require a prepared record to be the sole legal next transition."""

        if set(record) != LIFECYCLE_RECORD_FIELDS:
            raise IdentityError("lifecycle pending record shape is wrong")
        if (
            record["index"] != len(records)
            or record["journal_sequence"] != state["journal_sequence"] + 1
            or record["validator_id"] != self.validator_id
            or record["node_id"] != self.node_id
            or record["event"] not in LIFECYCLE_EVENTS
            or record["supervisor_generation"] != state["supervisor_generation"]
        ):
            raise IdentityError("lifecycle pending record identity is wrong")
        _require_positive_integer(
            record["observed_at_unix_ms"], "lifecycle pending observation"
        )
        if records and record["observed_at_unix_ms"] < records[-1]["observed_at_unix_ms"]:
            raise IdentityError("lifecycle pending observation regressed")
        process_generation = state["process_generation"]
        restart_count = state["restart_count"]
        unexpected_exit_total = state["unexpected_exit_total"]
        if record["event"] == "healthy" and process_generation == 0:
            process_generation = 1
        elif record["event"] == "restart":
            if process_generation == 0:
                raise IdentityError("initial lifecycle process cannot restart")
            process_generation += 1
            restart_count += 1
        elif record["event"] == "unexpected_exit":
            if process_generation == 0:
                process_generation = 1
            unexpected_exit_total += 1
        if (
            record["process_generation"] != process_generation
            or record["restart_count"] != restart_count
            or record["unexpected_exit_total"] != unexpected_exit_total
        ):
            raise IdentityError("lifecycle pending counters are incoherent")
        pending = state["pending_record"]
        assert isinstance(pending, dict)
        if pending["journal_chain_sha256"] != _lifecycle_next_chain(
            str(state["journal_chain_sha256"]), record
        ):
            raise IdentityError("lifecycle pending chain digest is wrong")

    def record(
        self, event: str, *, observed_at_unix_ms: int | None = None
    ) -> dict[str, object]:
        """Durably append one exact healthy/restart/unexpected-exit record."""

        if event not in LIFECYCLE_EVENTS:
            raise IdentityError("lifecycle event is not exact")
        observed = (
            time.time_ns() // 1_000_000
            if observed_at_unix_ms is None
            else _require_positive_integer(
                observed_at_unix_ms, "lifecycle observation time"
            )
        )
        self._lock_state()
        try:
            self._confirm_root()
            state, _ = self._read_state()
            journal_body, records = self._read_journal()
            if state["pending_record"] is not None:
                raise IdentityError("lifecycle state has an active pending record")
            self._validate_committed(state, journal_body, records)
            if records and observed < records[-1]["observed_at_unix_ms"]:
                raise IdentityError("lifecycle observation wall clock regressed")
            process_generation = state["process_generation"]
            restart_count = state["restart_count"]
            unexpected_exit_total = state["unexpected_exit_total"]
            if event == "healthy" and process_generation == 0:
                process_generation = 1
            elif event == "restart":
                if process_generation == 0:
                    raise IdentityError("initial process start is not a restart")
                process_generation += 1
                restart_count += 1
            elif event == "unexpected_exit":
                if process_generation == 0:
                    process_generation = 1
                unexpected_exit_total += 1
            record: dict[str, object] = {
                "index": len(records),
                "journal_sequence": state["journal_sequence"] + 1,
                "observed_at_unix_ms": observed,
                "validator_id": self.validator_id,
                "node_id": self.node_id,
                "event": event,
                "restart_count": restart_count,
                "supervisor_generation": state["supervisor_generation"],
                "process_generation": process_generation,
                "unexpected_exit_total": unexpected_exit_total,
            }
            if len(canonical_json_line(record)) > LIFECYCLE_RECORD_MAX_BYTES:
                raise IdentityError("lifecycle record exceeds its bound")
            next_chain = _lifecycle_next_chain(
                str(state["journal_chain_sha256"]), record
            )
            prepared = dict(state)
            prepared["pending_record"] = {
                "record": record,
                "journal_chain_sha256": next_chain,
            }
            self._publish_state(prepared)
            next_body = journal_body + canonical_json_line(record)
            _publish_lifecycle_file(
                self.journal_path,
                next_body,
                "lifecycle journal",
                LIFECYCLE_JOURNAL_MAX_BYTES,
            )
            self._confirm_root()
            final = dict(prepared)
            final["supervisor_generation"] = record["supervisor_generation"]
            final["process_generation"] = process_generation
            final["restart_count"] = restart_count
            final["unexpected_exit_total"] = unexpected_exit_total
            final["journal_sequence"] = record["journal_sequence"]
            final["journal_chain_sha256"] = next_chain
            final["journal_record_count"] = len(records) + 1
            final["journal_size_bytes"] = len(next_body)
            final["journal_sha256"] = hashlib.sha256(next_body).hexdigest()
            final["pending_record"] = None
            self._publish_state(final)
            self._confirm_root()
            return dict(record)
        finally:
            self._unlock_state()

    def process_has_started(self) -> bool:
        """Return whether this binding already recorded an initial child start."""

        self._lock_state()
        try:
            self._confirm_root()
            state, _ = self._read_state()
            journal_body, records = self._read_journal()
            self._validate_committed(state, journal_body, records)
            return bool(state["process_generation"])
        finally:
            self._unlock_state()

    def checkpoint(
        self, *, captured_at_unix_ms: int | None = None
    ) -> dict[str, object]:
        """Capture one stable per-peer cursor shaped for lifecycle aggregation."""

        captured = (
            time.time_ns() // 1_000_000
            if captured_at_unix_ms is None
            else _require_positive_integer(
                captured_at_unix_ms, "lifecycle checkpoint time"
            )
        )
        self._lock_state()
        try:
            self._confirm_root()
            state, _ = self._read_state()
            journal_body, records = self._read_journal()
            if state["pending_record"] is not None:
                raise IdentityError("cannot capture a pending lifecycle transition")
            self._validate_committed(state, journal_body, records)
            if state["process_generation"] == 0:
                raise IdentityError("cannot capture lifecycle before initial health")
            if records and state["supervisor_generation"] != records[-1]["supervisor_generation"]:
                raise IdentityError(
                    "cannot capture lifecycle before this supervisor records its child"
                )
            if records and captured < records[-1]["observed_at_unix_ms"]:
                raise IdentityError("lifecycle checkpoint predates its journal tip")
            return {
                "captured_at_unix_ms": captured,
                "journal_sequence": state["journal_sequence"],
                "journal_chain_sha256": state["journal_chain_sha256"],
                "validators": [
                    {
                        "validator_id": self.validator_id,
                        "node_id": self.node_id,
                        "restart_count": state["restart_count"],
                        "supervisor_generation": state["supervisor_generation"],
                        "process_generation": state["process_generation"],
                        "unexpected_exit_total": state["unexpected_exit_total"],
                    }
                ],
            }
        finally:
            self._unlock_state()

    def export_window(
        self,
        baseline: dict[str, object],
        terminal: dict[str, object],
        target: Path,
    ) -> dict[str, object]:
        """Publish canonical raw input for a protected four-peer aggregator.

        The rows use the public verifier's exact field set. The file is not a
        public lifecycle inventory: independent peers have colliding local
        sequences, so a protected collector must globally resequence and bind
        all four checkpoints before native verification.
        """

        self._lock_state()
        try:
            self._confirm_root()
            state, _ = self._read_state()
            journal_body, records = self._read_journal()
            self._validate_committed(state, journal_body, records)
            _validate_lifecycle_checkpoint(baseline, self.validator_id, self.node_id)
            _validate_lifecycle_checkpoint(terminal, self.validator_id, self.node_id)
            _validate_checkpoint_cursor(baseline, records)
            _validate_checkpoint_cursor(terminal, records)
            baseline_sequence = baseline["journal_sequence"]
            terminal_sequence = terminal["journal_sequence"]
            assert isinstance(baseline_sequence, int)
            assert isinstance(terminal_sequence, int)
            if not (0 <= baseline_sequence < terminal_sequence <= len(records)):
                raise IdentityError("lifecycle export window is not exact")
            selected_records = records[baseline_sequence:terminal_sequence]
            if (
                baseline["captured_at_unix_ms"]
                > selected_records[0]["observed_at_unix_ms"]
                or selected_records[-1]["observed_at_unix_ms"]
                > terminal["captured_at_unix_ms"]
            ):
                raise IdentityError("lifecycle export observations escape the window")
            chains = [_lifecycle_initial_chain(self.binding_sha256)]
            for record in records:
                chains.append(_lifecycle_next_chain(chains[-1], record))
            if (
                baseline["journal_chain_sha256"] != chains[baseline_sequence]
                or terminal["journal_chain_sha256"] != chains[terminal_sequence]
            ):
                raise IdentityError("lifecycle export checkpoint chain is wrong")
            selected = []
            for index, original in enumerate(
                selected_records
            ):
                row = dict(original)
                row["index"] = index
                selected.append(row)
            record_bytes = b"".join(canonical_json_line(row) for row in selected)
            records_sha256 = hashlib.sha256(
                b"iroha.taira.peer-supervisor-raw-window-records.v1\0"
                + record_bytes
            ).hexdigest()
            header = {
                "baseline": baseline,
                "binding_sha256": self.binding_sha256,
                "node_id": self.node_id,
                "record_count": len(selected),
                "records_sha256": records_sha256,
                "schema": LIFECYCLE_RAW_WINDOW_SCHEMA,
                "schema_version": 1,
                "terminal": terminal,
                "validator_id": self.validator_id,
            }
            if set(header) != LIFECYCLE_RAW_WINDOW_FIELDS:
                raise IdentityError("lifecycle raw export header shape is not exact")
            body = canonical_json_line(header) + record_bytes
            if not target.is_absolute() or ".." in target.parts:
                raise IdentityError("lifecycle export path is not canonical and absolute")
            digest = _publish_lifecycle_file(
                target,
                body,
                "lifecycle export",
                LIFECYCLE_JOURNAL_MAX_BYTES,
            )
            return {
                "record_count": len(selected),
                "raw_records_sha256": records_sha256,
                "schema": LIFECYCLE_RAW_WINDOW_SCHEMA,
                "sha256": digest,
                "size_bytes": len(body),
            }
        finally:
            self._unlock_state()

    def close(self) -> None:
        """Release this process's exact peer-writer lease."""

        if getattr(self, "closed", True):
            return
        self.closed = True
        try:
            fcntl.flock(self.state_fd, fcntl.LOCK_UN)
        finally:
            os.close(self.state_fd)
        try:
            fcntl.flock(self.owner_fd, fcntl.LOCK_UN)
        finally:
            os.close(self.owner_fd)


def _validate_lifecycle_checkpoint(
    checkpoint: dict[str, object], validator_id: str, node_id: str
) -> None:
    """Validate one exact single-peer checkpoint before window export."""

    if not isinstance(checkpoint, dict) or set(checkpoint) != LIFECYCLE_CHECKPOINT_FIELDS:
        raise IdentityError("lifecycle checkpoint shape is not exact")
    _require_positive_integer(
        checkpoint["captured_at_unix_ms"], "lifecycle checkpoint capture time"
    )
    _require_nonnegative_integer(
        checkpoint["journal_sequence"], "lifecycle checkpoint sequence"
    )
    chain = checkpoint["journal_chain_sha256"]
    if not isinstance(chain, str) or LOWER_SHA256_RE.fullmatch(chain) is None:
        raise IdentityError("lifecycle checkpoint chain digest is invalid")
    validators = checkpoint["validators"]
    if not isinstance(validators, list) or len(validators) != 1:
        raise IdentityError("per-peer lifecycle checkpoint must have one validator")
    row = validators[0]
    if (
        not isinstance(row, dict)
        or set(row) != LIFECYCLE_VALIDATOR_FIELDS
        or row["validator_id"] != validator_id
        or row["node_id"] != node_id
    ):
        raise IdentityError("lifecycle checkpoint peer identity is wrong")
    for field in ("restart_count", "unexpected_exit_total"):
        _require_nonnegative_integer(row[field], f"lifecycle checkpoint {field}")
    for field in ("supervisor_generation", "process_generation"):
        _require_positive_integer(row[field], f"lifecycle checkpoint {field}")


def _validate_checkpoint_cursor(
    checkpoint: dict[str, object], records: list[dict[str, object]]
) -> None:
    """Bind a checkpoint row and capture time to its exact local chain cursor."""

    sequence = checkpoint["journal_sequence"]
    assert isinstance(sequence, int)
    if sequence <= 0 or sequence > len(records):
        raise IdentityError("lifecycle checkpoint sequence is outside the journal")
    record = records[sequence - 1]
    if checkpoint["captured_at_unix_ms"] < record["observed_at_unix_ms"]:
        raise IdentityError("lifecycle checkpoint predates its cursor record")
    validators = checkpoint["validators"]
    assert isinstance(validators, list) and isinstance(validators[0], dict)
    row = validators[0]
    for field in (
        "restart_count",
        "supervisor_generation",
        "process_generation",
        "unexpected_exit_total",
    ):
        if row[field] != record[field]:
            raise IdentityError("lifecycle checkpoint counters differ from its cursor")


def _checkpoint_from_lifecycle_state(
    state: dict[str, object], captured_at_unix_ms: int
) -> dict[str, object]:
    """Project one validated local state into the aggregator checkpoint shape."""

    return {
        "captured_at_unix_ms": captured_at_unix_ms,
        "journal_sequence": state["journal_sequence"],
        "journal_chain_sha256": state["journal_chain_sha256"],
        "validators": [
            {
                "validator_id": state["validator_id"],
                "node_id": state["node_id"],
                "restart_count": state["restart_count"],
                "supervisor_generation": state["supervisor_generation"],
                "process_generation": state["process_generation"],
                "unexpected_exit_total": state["unexpected_exit_total"],
            }
        ],
    }


def _inspect_lifecycle_snapshot(
    root: Path, binding_sha256: str, validator_id: str, node_id: str
) -> tuple[dict[str, object], bytes, list[dict[str, object]]]:
    """Read one quiescent peer snapshot while holding its exact state lock."""

    if not root.is_absolute() or ".." in root.parts:
        raise IdentityError("lifecycle journal root is not canonical and absolute")
    if (
        LOWER_SHA256_RE.fullmatch(binding_sha256) is None
        or LIFECYCLE_VALIDATOR_RE.fullmatch(validator_id) is None
        or LIFECYCLE_NODE_ID_RE.fullmatch(node_id) is None
    ):
        raise IdentityError("lifecycle inspection identity is not canonical")
    root_info = require_private_directory(root, "lifecycle journal root")
    if {entry.name for entry in root.iterdir()} != LifecycleJournal.ENTRIES:
        raise IdentityError("lifecycle journal root entries are not exact")
    descriptor, lock_info = _open_lifecycle_lock(
        root,
        LifecycleJournal.STATE_LOCK,
        "lifecycle state lock",
        nonblocking=True,
    )
    try:
        _confirm_lifecycle_path(
            root / LifecycleJournal.STATE_LOCK,
            descriptor,
            lock_info,
            "lifecycle state lock",
        )
        state_body, _ = _require_lifecycle_regular_file(
            root / LifecycleJournal.STATE_FILE,
            "lifecycle state",
            LIFECYCLE_STATE_MAX_BYTES,
        )
        journal_body, _ = _require_lifecycle_regular_file(
            root / LifecycleJournal.JOURNAL_FILE,
            "lifecycle journal",
            LIFECYCLE_JOURNAL_MAX_BYTES,
        )
        state = _decode_lifecycle_state(state_body)
        records = _decode_lifecycle_records(journal_body)
        if (
            state["pending_record"] is not None
            or state["binding_sha256"] != binding_sha256
            or state["validator_id"] != validator_id
            or state["node_id"] != node_id
        ):
            raise IdentityError("lifecycle snapshot binding or phase is wrong")
        _validate_lifecycle_snapshot(
            binding_sha256,
            validator_id,
            node_id,
            state,
            journal_body,
            records,
        )
        current_root = require_private_directory(root, "lifecycle journal root")
        _confirm_lifecycle_path(
            root / LifecycleJournal.STATE_LOCK,
            descriptor,
            lock_info,
            "lifecycle state lock",
        )
        if (
            (current_root.st_dev, current_root.st_ino)
            != (root_info.st_dev, root_info.st_ino)
            or {entry.name for entry in root.iterdir()} != LifecycleJournal.ENTRIES
        ):
            raise IdentityError("lifecycle journal root changed during inspection")
        return state, journal_body, records
    finally:
        fcntl.flock(descriptor, fcntl.LOCK_UN)
        os.close(descriptor)


def capture_lifecycle_checkpoint(
    root: Path,
    binding_sha256: str,
    validator_id: str,
    node_id: str,
    *,
    captured_at_unix_ms: int | None = None,
) -> dict[str, object]:
    """Capture a stable local cursor without taking the supervisor owner lease."""

    captured = (
        time.time_ns() // 1_000_000
        if captured_at_unix_ms is None
        else _require_positive_integer(
            captured_at_unix_ms, "lifecycle checkpoint time"
        )
    )
    state, _journal_body, records = _inspect_lifecycle_snapshot(
        root, binding_sha256, validator_id, node_id
    )
    if state["process_generation"] == 0:
        raise IdentityError("cannot capture lifecycle before initial health")
    if records and state["supervisor_generation"] != records[-1]["supervisor_generation"]:
        raise IdentityError(
            "cannot capture lifecycle before this supervisor records its child"
        )
    if records and captured < records[-1]["observed_at_unix_ms"]:
        raise IdentityError("lifecycle checkpoint predates its journal tip")
    return _checkpoint_from_lifecycle_state(state, captured)


def export_lifecycle_raw_window(
    root: Path,
    binding_sha256: str,
    validator_id: str,
    node_id: str,
    baseline: dict[str, object],
    terminal: dict[str, object],
    target: Path,
) -> dict[str, object]:
    """Export peer-local raw rows for the future protected four-peer collector."""

    _state, _journal_body, records = _inspect_lifecycle_snapshot(
        root, binding_sha256, validator_id, node_id
    )
    _validate_lifecycle_checkpoint(baseline, validator_id, node_id)
    _validate_lifecycle_checkpoint(terminal, validator_id, node_id)
    _validate_checkpoint_cursor(baseline, records)
    _validate_checkpoint_cursor(terminal, records)
    baseline_sequence = baseline["journal_sequence"]
    terminal_sequence = terminal["journal_sequence"]
    assert isinstance(baseline_sequence, int)
    assert isinstance(terminal_sequence, int)
    if not (0 <= baseline_sequence < terminal_sequence <= len(records)):
        raise IdentityError("lifecycle raw export window is not exact")
    selected_records = records[baseline_sequence:terminal_sequence]
    if (
        baseline["captured_at_unix_ms"]
        > selected_records[0]["observed_at_unix_ms"]
        or selected_records[-1]["observed_at_unix_ms"]
        > terminal["captured_at_unix_ms"]
    ):
        raise IdentityError("lifecycle raw export observations escape the window")
    chains = [_lifecycle_initial_chain(binding_sha256)]
    for record in records:
        chains.append(_lifecycle_next_chain(chains[-1], record))
    if (
        baseline["journal_chain_sha256"] != chains[baseline_sequence]
        or terminal["journal_chain_sha256"] != chains[terminal_sequence]
    ):
        raise IdentityError("lifecycle raw export checkpoint chain is wrong")
    selected: list[dict[str, object]] = []
    for index, original in enumerate(selected_records):
        row = dict(original)
        row["index"] = index
        selected.append(row)
    record_bytes = b"".join(canonical_json_line(row) for row in selected)
    records_sha256 = hashlib.sha256(
        b"iroha.taira.peer-supervisor-raw-window-records.v1\0" + record_bytes
    ).hexdigest()
    header = {
        "baseline": baseline,
        "binding_sha256": binding_sha256,
        "node_id": node_id,
        "record_count": len(selected),
        "records_sha256": records_sha256,
        "schema": LIFECYCLE_RAW_WINDOW_SCHEMA,
        "schema_version": 1,
        "terminal": terminal,
        "validator_id": validator_id,
    }
    if set(header) != LIFECYCLE_RAW_WINDOW_FIELDS:
        raise IdentityError("lifecycle raw export header shape is not exact")
    body = canonical_json_line(header) + record_bytes
    if not target.is_absolute() or ".." in target.parts:
        raise IdentityError("lifecycle raw export path is not canonical and absolute")
    digest = _publish_lifecycle_file(
        target,
        body,
        "lifecycle raw export",
        LIFECYCLE_JOURNAL_MAX_BYTES,
    )
    return {
        "record_count": len(selected),
        "raw_records_sha256": records_sha256,
        "schema": LIFECYCLE_RAW_WINDOW_SCHEMA,
        "sha256": digest,
        "size_bytes": len(body),
    }


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

    def wait(
        self,
        child: subprocess.Popen[bytes],
        periodic: Callable[[], None] | None = None,
    ) -> int:
        """Wait while draining stderr and running one bounded periodic hook."""

        while True:
            self._drain()
            return_code = child.poll()
            if return_code is not None:
                self._drain()
                return return_code
            if periodic is not None:
                periodic()
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
    parser.add_argument("--lifecycle-journal-root")
    parser.add_argument("--validator-id")
    parser.add_argument("--node-id")
    parser.add_argument(
        "--lifecycle-healthy-interval-seconds",
        type=float,
        default=DEFAULT_LIFECYCLE_HEALTHY_INTERVAL_SECONDS,
    )
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
    lifecycle_values = (
        args.lifecycle_journal_root,
        args.validator_id,
        args.node_id,
    )
    if any(value is not None for value in lifecycle_values) and not all(
        value is not None for value in lifecycle_values
    ):
        parser.error(
            "--lifecycle-journal-root, --validator-id, and --node-id "
            "must be provided together"
        )
    if (
        not math.isfinite(args.lifecycle_healthy_interval_seconds)
        or args.lifecycle_healthy_interval_seconds <= 0
    ):
        parser.error("--lifecycle-healthy-interval-seconds must be positive")
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
    if args.lifecycle_journal_root is not None:
        lifecycle_root = Path(args.lifecycle_journal_root)
        if (
            not lifecycle_root.is_absolute()
            or ".." in lifecycle_root.parts
            or lifecycle_root in {terminal_file, pid_file}
            or LIFECYCLE_VALIDATOR_RE.fullmatch(args.validator_id) is None
            or LIFECYCLE_NODE_ID_RE.fullmatch(args.node_id) is None
        ):
            parser.error("lifecycle journal identity or path is not canonical")
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

    lifecycle: LifecycleJournal | None = None
    if args.lifecycle_journal_root is not None:
        try:
            lifecycle = LifecycleJournal(
                Path(args.lifecycle_journal_root),
                lifecycle_binding_sha256(args, args.validator_id, args.node_id),
                args.validator_id,
                args.node_id,
                args.restart_generation,
            )
        except (IdentityError, OSError) as exc:
            print(
                f"taira supervisor lifecycle refusal: {exc}",
                file=sys.stderr,
                flush=True,
            )
            return 78

    def finish(result: int) -> int:
        if lifecycle is not None:
            lifecycle.close()
        return result

    backoff = args.initial_backoff_seconds
    fatal_tracker = RapidFatalExitTracker()
    while stopping_signal is None:
        try:
            require_runtime_identity(args)
        except (IdentityError, OSError) as exc:
            print(
                f"taira supervisor identity refusal: {exc}", file=sys.stderr, flush=True
            )
            return finish(78)

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
        if lifecycle is not None:
            try:
                already_started = lifecycle.process_has_started()
                if already_started:
                    lifecycle.record("restart")
                elif child.poll() is None:
                    lifecycle.record("healthy")
            except (IdentityError, OSError) as exc:
                try:
                    child.terminate()
                except ProcessLookupError:
                    pass
                stderr_capture.wait(child)
                stderr_capture.finish()
                remove_owned_pid(pid_file, child.pid)
                child = None
                print(
                    f"taira supervisor lifecycle refusal: {exc}",
                    file=sys.stderr,
                    flush=True,
                )
                return finish(78)
        print(f"taira validator started pid={child.pid}", flush=True)
        next_healthy_at = (
            time.monotonic() + args.lifecycle_healthy_interval_seconds
        )

        def record_periodic_health() -> None:
            nonlocal next_healthy_at
            if lifecycle is None:
                return
            now = time.monotonic()
            if now >= next_healthy_at:
                lifecycle.record("healthy")
                next_healthy_at = now + args.lifecycle_healthy_interval_seconds

        try:
            return_code = stderr_capture.wait(child, record_periodic_health)
        except (IdentityError, OSError) as exc:
            try:
                child.terminate()
            except ProcessLookupError:
                pass
            return_code = stderr_capture.wait(child)
            stderr_capture.finish()
            remove_owned_pid(pid_file, child.pid)
            child = None
            print(
                f"taira supervisor lifecycle refusal: {exc}",
                file=sys.stderr,
                flush=True,
            )
            return finish(78)
        stderr_tail = stderr_capture.finish()
        uptime = time.monotonic() - started
        remove_owned_pid(pid_file, child.pid)
        child = None

        if stopping_signal is not None:
            print(
                f"taira validator stopped signal={stopping_signal} rc={return_code}",
                flush=True,
            )
            return finish(0)

        if restart_requested:
            restart_requested = False
            fatal_tracker.observe(None)
            backoff = args.initial_backoff_seconds
            print(
                "taira validator restart requested by capture authority",
                flush=True,
            )
            continue

        if lifecycle is not None:
            try:
                lifecycle.record("unexpected_exit")
            except (IdentityError, OSError) as exc:
                print(
                    f"taira supervisor lifecycle refusal: {exc}",
                    file=sys.stderr,
                    flush=True,
                )
                return finish(78)

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
                return finish(hold_terminal_unhealthy(None))
            return finish(hold_terminal_unhealthy(fatal_fingerprint))
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
    return finish(0)


if __name__ == "__main__":
    raise SystemExit(run())
