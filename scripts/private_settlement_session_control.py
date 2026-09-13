"""Bounded, durable control channels for one retained benchmark network.

Control messages authorize only dispatch and continuation. They never replace
the canonical request, measurement, process-lifetime or release validators.
Callers own the existing attempt deadline and the actual process lifecycle.
"""

from __future__ import annotations

from dataclasses import dataclass
from functools import wraps
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import struct
from typing import Any, BinaryIO, Callable

PROTOCOL = "AtomicPrivateSettlementV1"
MAX_FRAME_BYTES = 16 * 1024 * 1024
MAX_U64 = (1 << 64) - 1
IDENTITY_FIELDS = frozenset({
    "version", "protocol", "scope_sha256", "campaign_id", "plan_sha256",
    "session_id", "session_invocation_nonce", "session_request_sha256",
})
ATTEMPT_FIELDS = frozenset({
    "attempt_id", "request_id", "invocation_nonce", "session_attempt_index",
})
MESSAGE_FIELDS = IDENTITY_FIELDS | frozenset({
    "channel", "direction", "sequence", "previous_message_sha256", "kind",
    "payload", "forwarded_from",
})
CHANNELS = frozenset({"runner_adapter", "adapter_worker"})
DIRECTIONS = frozenset({"owner_to_child", "child_to_owner"})
CHANNEL_ENDPOINTS = {"runner_adapter": ("runner", "adapter"), "adapter_worker": ("adapter", "worker")}
KIND_DIRECTION = {
    "ready": "child_to_owner", "dispatch": "owner_to_child",
    "attempt_completed": "child_to_owner", "accept": "owner_to_child",
    "stop": "owner_to_child", "session_completed": "child_to_owner",
    "measurement_ready": "child_to_owner", "measurement_begin": "owner_to_child",
    "measurement_finished": "child_to_owner", "measurement_recorded": "owner_to_child",
}
LOCAL_MEASUREMENT_KINDS = frozenset({
    "measurement_ready", "measurement_begin", "measurement_finished", "measurement_recorded",
})
STOP_REASONS = frozenset({
    "attempt_failed", "attempt_timed_out", "attempt_incomplete",
    "validation_failed", "publication_failed", "setup_failed",
    "transport_interrupted", "cleanup_failed",
})


class SessionProtocolError(ValueError):
    """Malformed or contradictory evidence; it cannot be counted as success."""


class SessionInterrupted(EOFError):
    """The bounded channel ended without its required complete message."""


class ChildControlPipes:
    """Own separate control pipes while the caller owns spawn and deadlines.

    Pass only ``child_fds`` to Popen, then call ``child_spawn_finished`` in a
    finally block around spawn. This closes the parent's copies of the child
    ends on success and failure, so an exited worker cannot be hidden by a
    still-open parent write descriptor. Logs use separate descriptors.
    """

    def __init__(self):
        descriptors: list[int] = []
        try:
            child_read, parent_write = os.pipe()
            descriptors.extend((child_read, parent_write))
            parent_read, child_write = os.pipe()
            descriptors.extend((parent_read, child_write))
            require(all(fd > 2 for fd in descriptors), "standard descriptor is unavailable")
            for fd in descriptors:
                os.set_inheritable(fd, False)
            self.reader = os.fdopen(parent_read, "rb", buffering=0)
            descriptors.remove(parent_read)
            try:
                self.writer = os.fdopen(parent_write, "wb", buffering=0)
            except BaseException:
                self.reader.close()
                raise
            descriptors.remove(parent_write)
            self._child_fds = (child_read, child_write)
        except BaseException:
            for fd in descriptors:
                os.close(fd)
            raise

    @property
    def child_fds(self) -> tuple[int, int]:
        """Return exactly read/write child ends before spawn completes."""
        require(self._child_fds is not None, "child descriptors have already been released")
        return self._child_fds

    def child_spawn_finished(self) -> None:
        """Release only this owner's copied child ends, never another process."""
        if self._child_fds is not None:
            for fd in self._child_fds:
                os.close(fd)
            self._child_fds = None

    def close(self) -> None:
        """Close every owned pipe end without killing or waiting for a process."""
        self.child_spawn_finished()
        self.reader.close()
        self.writer.close()

    def __enter__(self) -> ChildControlPipes:
        return self

    def __exit__(self, *_: object) -> None:
        self.close()


def require(condition: bool, reason: str) -> None:
    """Reject one failed protocol predicate with a nonsecret fixed reason."""
    if not condition:
        raise SessionProtocolError(reason)


def exact(value: Any, fields: frozenset[str], label: str) -> dict[str, Any]:
    """Admit precisely one declared JSON object shape."""
    require(type(value) is dict and set(value) == fields, f"invalid {label} fields")
    return value


def digest(value: Any) -> str:
    """Admit a nonzero SHA-256 digest or random nonce in canonical text."""
    require(type(value) is str and re.fullmatch(r"[0-9a-f]{64}", value) is not None
            and value != "0" * 64, "invalid digest or nonce")
    return value


def unsigned(value: Any) -> int:
    """Reject booleans, negatives and integers outside the wire u64 range."""
    require(type(value) is int and 0 <= value <= MAX_U64, "invalid unsigned integer")
    return value


def canonical(value: Any) -> bytes:
    """Encode protocol JSON; reject floats and unbounded/deep object trees."""
    def check(item: Any, depth: int) -> None:
        require(depth <= 32, "control JSON nesting exceeds its bound")
        if item is None or type(item) in (bool, str):
            return
        if type(item) is int:
            unsigned(item)
        elif type(item) is dict:
            require(all(type(key) is str for key in item), "nontext JSON object key")
            for child in item.values():
                check(child, depth + 1)
        elif type(item) is list:
            for child in item:
                check(child, depth + 1)
        else:
            raise SessionProtocolError("unsupported control JSON value")
    check(value, 0)
    try:
        raw = json.dumps(value, ensure_ascii=False, sort_keys=True,
                         separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (UnicodeError, ValueError, RecursionError) as error:
        raise SessionProtocolError("invalid control JSON encoding") from error
    require(0 < len(raw) <= MAX_FRAME_BYTES, "control JSON exceeds its bound")
    return raw


def decode(raw: bytes) -> dict[str, Any]:
    """Reject duplicate keys, noncanonical bytes and unknown JSON types."""
    require(type(raw) is bytes and 0 < len(raw) <= MAX_FRAME_BYTES,
            "control JSON exceeds its bound")
    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in items:
            require(key not in result, "duplicate control JSON key")
            result[key] = value
        return result
    def bad_constant(_: str) -> Any:
        raise SessionProtocolError("nonfinite control JSON value")
    try:
        result = json.loads(raw.decode("utf-8"), object_pairs_hook=pairs,
                            parse_constant=bad_constant)
    except (UnicodeError, ValueError, RecursionError) as error:
        raise SessionProtocolError("invalid control JSON") from error
    require(type(result) is dict and canonical(result) == raw,
            "control JSON is not a canonical object")
    return result


def reference(value: Any) -> dict[str, Any]:
    """Validate a located immutable record reference under its owner's root."""
    row = exact(value, frozenset({"path", "sha256", "bytes"}), "record reference")
    path = row["path"]
    require(type(path) is str and 0 < len(path.encode('utf-8')) <= 4096
            and not any(ord(char) < 32 or 127 <= ord(char) <= 159 for char in path) and "\\" not in path,
            "invalid record path")
    parsed = PurePosixPath(path)
    require(bool(parsed.parts) and not parsed.is_absolute() and str(parsed) == path
            and all(part not in ("", ".", "..") for part in parsed.parts),
            "noncanonical record path")
    digest(row["sha256"])
    require(0 < unsigned(row["bytes"]) <= MAX_FRAME_BYTES + 4, "invalid record size")
    return row


def identity(value: Any) -> dict[str, Any]:
    """Validate all scope, campaign, session and invocation bindings."""
    row = exact(value, IDENTITY_FIELDS, "session identity")
    require(type(row["version"]) is int and row["version"] == 1
            and row["protocol"] == PROTOCOL, "unsupported session protocol")
    require(type(row["campaign_id"]) is str
            and re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,63}", row["campaign_id"]) is not None,
            "campaign_id must be a canonical registered slug")
    for field in IDENTITY_FIELDS - {"version", "protocol", "campaign_id"}:
        digest(row[field])
    return row


def attempt(value: Any) -> dict[str, Any]:
    """Validate an exact attempt identity independent of process exit."""
    row = exact(value, ATTEMPT_FIELDS, "attempt identity")
    for field in ATTEMPT_FIELDS - {"session_attempt_index"}:
        digest(row[field])
    unsigned(row["session_attempt_index"])
    return row


def _payload(message: dict[str, Any]) -> None:
    channel, kind = message["channel"], message["kind"]
    fields: frozenset[str]
    if kind in LOCAL_MEASUREMENT_KINDS:
        require(channel == "adapter_worker", "measurement control belongs to the native adapter channel")
        fields = ATTEMPT_FIELDS | {"marker"}
        if kind == "measurement_begin":
            fields |= {"process_observation"}
        elif kind == "measurement_recorded":
            fields |= {"measurement_window"}
    elif kind == "dispatch":
        fields = ATTEMPT_FIELDS | {"attempt_started", "request"}
    elif kind == "accept":
        fields = ATTEMPT_FIELDS | {
            "rust_terminal", "adapter_outcome", "response", "validation", "sample",
        }
    elif kind == "attempt_completed":
        fields = ATTEMPT_FIELDS | {"rust_terminal"}
        if channel == "runner_adapter":
            fields |= {"adapter_outcome", "response"}
    elif kind == "ready":
        fields = frozenset({"ready"})
        if channel == "runner_adapter":
            fields |= {"process_observation"}
    elif kind == "session_completed":
        fields = frozenset({"worker_terminal"})
        if channel == "runner_adapter":
            fields |= {"adapter_lifecycle"}
    else:
        fields = frozenset({"active_attempt_id", "reason", "validation"})
    payload = exact(message["payload"], fields, "control payload")
    if ATTEMPT_FIELDS <= fields:
        attempt({key: payload[key] for key in ATTEMPT_FIELDS})
    for field in fields - ATTEMPT_FIELDS:
        if kind == "stop":
            if field == "active_attempt_id":
                if payload[field] is not None:
                    digest(payload[field])
            elif field == "reason":
                require(type(payload[field]) is str and payload[field] in STOP_REASONS,
                        "undeclared session stop reason")
            elif payload[field] is not None:
                reference(payload[field])
        elif kind == "attempt_completed" and field in {"adapter_outcome", "response"}:
            # Failed/incomplete outcomes have no successful response. Their typed
            # semantics are checked by the attempt reducer, never inferred here.
            if payload[field] is not None:
                reference(payload[field])
        else:
            reference(payload[field])


def validate_message(value: Any) -> dict[str, Any]:
    """Check the mandatory first-release channel-specific wire shape."""
    message = exact(value, MESSAGE_FIELDS, "control message")
    identity({key: message[key] for key in IDENTITY_FIELDS})
    require(type(message["channel"]) is str and message["channel"] in CHANNELS
            and type(message["direction"]) is str and message["direction"] in DIRECTIONS,
            "unknown control channel or direction")
    kind = message["kind"]
    require(type(kind) is str and kind in KIND_DIRECTION
            and KIND_DIRECTION[kind] == message["direction"], "invalid control kind/direction")
    unsigned(message["sequence"])
    digest(message["previous_message_sha256"])
    forwarded = (message["channel"], message["direction"]) in {
        ("adapter_worker", "owner_to_child"), ("runner_adapter", "child_to_owner"),
    }
    if kind in LOCAL_MEASUREMENT_KINDS:
        require(message["forwarded_from"] is None, "measurement control is local to its process owner")
    elif forwarded:
        reference(message["forwarded_from"])
    else:
        require(message["forwarded_from"] is None, "origin message cannot claim forwarding")
    _payload(message)
    return message


def _metadata(info: os.stat_result) -> tuple[int, ...]:
    return tuple(getattr(info, key) for key in (
        "st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns",
    ))


def _directory_identity(info: os.stat_result) -> tuple[int, ...]:
    """Bind a held directory without treating lawful sibling writes as replacement."""
    return tuple(getattr(info, key) for key in ("st_dev", "st_ino", "st_mode", "st_uid", "st_gid"))


class RecordDirectory:
    """Anchor controlled records to an owner-only directory descriptor.

    The caller retains this owner for the session. An interrupted new file stays
    present as incomplete evidence; neither retries nor publication overwrite it.
    """

    def __init__(self, path: Path):
        self.path = path
        self.fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW)
        try:
            info = os.fstat(self.fd)
            require(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) & 0o077 == 0,
                    "record directory is not owner-only")
            self.inode = info.st_dev, info.st_ino
            self.validate()
        except BaseException:
            os.close(self.fd)
            self.fd = -1
            raise

    def validate(self) -> None:
        """Reject a replaced named root without following a substituted link."""
        require(self.fd >= 0, "record directory is closed")
        info, named = os.fstat(self.fd), os.stat(self.path, follow_symlinks=False)
        require(stat.S_ISDIR(named.st_mode)
                and (info.st_dev, info.st_ino) == (named.st_dev, named.st_ino) == self.inode
                and info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) & 0o077 == 0,
                "record directory identity or permissions changed")

    def publish(self, name: str, raw: bytes) -> dict[str, Any]:
        """Fsync exact new bytes and their directory before returning a reference."""
        self.validate()
        reference({"path": name, "sha256": "1" * 64, "bytes": 1})
        parts = PurePosixPath(name).parts
        require(all(re.fullmatch(r"[a-z0-9][a-z0-9_.-]{0,159}", part) is not None for part in parts),
                "invalid record path component")
        require(type(raw) is bytes and 0 < len(raw) <= MAX_FRAME_BYTES + 4,
                "invalid retained record size")
        parents = [os.dup(self.fd)]
        entries: list[tuple[int, str, tuple[int, int]]] = []
        fd = -1
        try:
            for part in parts[:-1]:
                child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW,
                                dir_fd=parents[-1])
                info = os.fstat(child)
                entries.append((parents[-1], part, (info.st_dev, info.st_ino)))
                parents.append(child)
                require(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) & 0o077 == 0,
                        "record parent is not owner-only")
            fd = os.open(parts[-1], os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW,
                         0o600, dir_fd=parents[-1])
            view = memoryview(raw)
            while view:
                written = os.write(fd, view)
                require(written > 0, "record write made no progress")
                view = view[written:]
            os.fsync(fd)
            os.fsync(parents[-1])
            before = os.fstat(fd)
            require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
                    and before.st_uid == os.geteuid() and stat.S_IMODE(before.st_mode) & 0o077 == 0
                    and before.st_size == len(raw), "published record file is unsafe or changed size")
            offset, published_sha = 0, hashlib.sha256()
            while offset < len(raw):
                chunk = os.pread(fd, min(65536, len(raw) - offset), offset)
                require(bool(chunk), "published record was truncated")
                published_sha.update(chunk)
                offset += len(chunk)
            require(published_sha.digest() == hashlib.sha256(raw).digest()
                    and _metadata(before) == _metadata(os.fstat(fd))
                    == _metadata(os.stat(parts[-1], dir_fd=parents[-1], follow_symlinks=False)),
                    "published record bytes or named identity changed")
            for parent, part, prior in reversed(entries):
                info = os.stat(part, dir_fd=parent, follow_symlinks=False)
                require(stat.S_ISDIR(info.st_mode) and (info.st_dev, info.st_ino) == prior
                        and info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) & 0o077 == 0,
                        "record parent changed during publication")
            self.validate()
        finally:
            if fd >= 0:
                os.close(fd)
            for parent in reversed(parents):
                os.close(parent)
        return {"path": name, "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}

    def read(self, binding: Any) -> bytes:
        """Read exact bound bytes through non-symlink parent descriptors."""
        row = reference(binding)
        return self._read_record(row["path"], row)

    def locate(self, name: str) -> dict[str, Any]:
        """Bind newly published native bytes through the same strict reader.

        A located file establishes its initial bytes; it does not authenticate
        producer execution. Callers must join the actual native process receipt.
        """
        reference({"path": name, "sha256": "1" * 64, "bytes": 1})
        raw = self._read_record(name, None)
        return {"path": name, "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}

    def _read_record(self, name: str, row: dict[str, Any] | None) -> bytes:
        """Share complete held/named identity checks for bound and initial reads."""
        self.validate()
        parts = PurePosixPath(name).parts
        parents = [os.dup(self.fd)]
        entries: list[tuple[int, str, int, tuple[int, ...]]] = []
        fd = -1
        try:
            for name in parts[:-1]:
                child = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW,
                                dir_fd=parents[-1])
                info = os.fstat(child)
                entries.append((parents[-1], name, child, _directory_identity(info)))
                parents.append(child)
                require(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) & 0o077 == 0,
                        "record parent is not owner-only")
            fd = os.open(parts[-1], os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC | os.O_NOFOLLOW,
                         dir_fd=parents[-1])
            before = os.fstat(fd)
            require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
                    and before.st_uid == os.geteuid() and stat.S_IMODE(before.st_mode) & 0o077 == 0
                    and 0 < before.st_size <= MAX_FRAME_BYTES + 4
                    and (row is None or before.st_size == row["bytes"]),
                    "record file is unsafe or changed size")
            chunks, remaining = [], before.st_size
            while remaining:
                chunk = os.read(fd, min(remaining, 65536))
                require(bool(chunk), "record file was truncated")
                chunks.append(chunk)
                remaining -= len(chunk)
            raw = b"".join(chunks)
            require(_metadata(before) == _metadata(os.fstat(fd))
                    == _metadata(os.stat(parts[-1], dir_fd=parents[-1], follow_symlinks=False))
                    and (row is None or hashlib.sha256(raw).hexdigest() == row["sha256"]),
                    "record bytes or identity changed")
            for parent, name, held, prior in reversed(entries):
                require(_directory_identity(os.fstat(held)) == prior
                        == _directory_identity(os.stat(name, dir_fd=parent, follow_symlinks=False)),
                        "record parent identity or permissions changed during read")
            self.validate()
            return raw
        finally:
            if fd >= 0:
                os.close(fd)
            for parent in reversed(parents):
                os.close(parent)

    def close(self) -> None:
        """Close the root once; never terminate any process."""
        if self.fd >= 0:
            os.close(self.fd)
            self.fd = -1

    def __enter__(self) -> RecordDirectory:
        return self

    def __exit__(self, *_: object) -> None:
        self.close()


@dataclass(frozen=True)
class RetainedMessage:
    """Validated wire bytes plus their durable local journal reference."""
    raw: bytes
    binding: dict[str, Any]

    def __post_init__(self) -> None:
        """Reject a frame whose reference or length does not describe its bytes."""
        reference(self.binding)
        require(type(self.raw) is bytes and 4 < len(self.raw) <= MAX_FRAME_BYTES + 4
                and struct.unpack(">I", self.raw[:4])[0] == len(self.raw) - 4
                and self.binding["bytes"] == len(self.raw)
                and self.binding["sha256"] == hashlib.sha256(self.raw).hexdigest(),
                "retained control frame differs from its binding")

    def decoded(self) -> dict[str, Any]:
        """Return a fresh value so callers cannot mutate the validated wire bytes."""
        return decode(self.raw[4:])


class ControlChain:
    """Own one direction; a malformed/partial send or receive poisons it forever."""

    def __init__(self, session: dict[str, Any], started_sha256: str, channel: str,
                 direction: str, journal: RecordDirectory, *, observer: str,
                 journal_prefix: str):
        self.session = decode(canonical(identity(session)))
        require(channel in CHANNELS and direction in DIRECTIONS, "unknown control chain")
        require(observer in CHANNEL_ENDPOINTS[channel], "observer is not a channel endpoint")
        reference({"path": journal_prefix, "sha256": "1" * 64, "bytes": 1})
        self.channel, self.direction, self.journal = channel, direction, journal
        self.observer, self.journal_prefix = observer, journal_prefix
        self.sequence = 0
        self.previous = hashlib.sha256(canonical({
            "domain": "iroha:private-settlement:session-control-chain:v1",
            "session_started_sha256": digest(started_sha256),
            "channel": channel, "direction": direction,
        })).hexdigest()
        self.poisoned = False

    def _check(self, message: dict[str, Any]) -> None:
        require(not self.poisoned and self.sequence <= MAX_U64, "control chain is closed")
        validate_message(message)
        require(all(message[key] == value for key, value in self.session.items())
                and message["channel"] == self.channel and message["direction"] == self.direction
                and message["sequence"] == self.sequence
                and message["previous_message_sha256"] == self.previous,
                "control identity, order or predecessor differs")

    def _name(self) -> str:
        return (f"{self.journal_prefix}/{self.observer}.{self.channel}."
                f"{self.direction}.{self.sequence:020d}.frame")

    def _require_endpoint(self, *, sending: bool) -> None:
        owner, child = CHANNEL_ENDPOINTS[self.channel]
        sender, receiver = (owner, child) if self.direction == "owner_to_child" else (child, owner)
        require(self.observer == (sender if sending else receiver), "control operation uses wrong endpoint")

    def _advance(self, raw: bytes) -> None:
        self.previous = hashlib.sha256(raw).hexdigest()
        self.sequence += 1

    def send(self, stream: BinaryIO, kind: str, payload: dict[str, Any], *,
             forwarded_from: dict[str, Any] | None = None) -> RetainedMessage:
        """Publish before writing any framed bytes; never retry a partial frame."""
        try:
            self._require_endpoint(sending=True)
            message = {**self.session, "channel": self.channel, "direction": self.direction,
                       "sequence": self.sequence, "previous_message_sha256": self.previous,
                       "kind": kind, "payload": payload, "forwarded_from": forwarded_from}
            self._check(message)
            body = canonical(message)
            raw = struct.pack(">I", len(body)) + body
            binding = self.journal.publish(self._name(), raw)
            view = memoryview(raw)
            while view:
                count = stream.write(view)
                require(type(count) is int and 0 < count <= len(view), "control write failed")
                view = view[count:]
            stream.flush()
            self._advance(raw)
            return RetainedMessage(raw, binding)
        except BaseException:
            self.poisoned = True
            raise

    def receive(self, stream: BinaryIO) -> RetainedMessage:
        """Retain even malformed complete frames before decoding or acting."""
        retained = bytearray()
        def read_exact(length: int) -> bytes:
            chunks, remaining = [], length
            while remaining:
                chunk = stream.read(remaining)
                if not chunk:
                    raise SessionInterrupted("control channel ended during required frame")
                require(type(chunk) is bytes and len(chunk) <= remaining, "invalid control read")
                retained.extend(chunk)
                chunks.append(chunk)
                remaining -= len(chunk)
            return b"".join(chunks)
        published = False
        try:
            self._require_endpoint(sending=False)
            require(not self.poisoned and self.sequence <= MAX_U64, "control chain is closed")
            length = struct.unpack(">I", read_exact(4))[0]
            require(0 < length <= MAX_FRAME_BYTES, "control frame length exceeds its bound")
            body = read_exact(length)
            raw = bytes(retained)
            binding = self.journal.publish(self._name(), raw)
            published = True
            self._check(decode(body))
            self._advance(raw)
            return RetainedMessage(raw, binding)
        except BaseException:
            self.poisoned = True
            if retained and not published:
                self.journal.publish(self._name() + ".incomplete", bytes(retained))
            raise


def verify_forwarded(message: RetainedMessage, upstream: RetainedMessage) -> None:
    """Join the adapter's new channel to already validated upstream bytes."""
    child, parent = message.decoded(), upstream.decoded()
    validate_message(child)
    validate_message(parent)
    require(child["forwarded_from"] == upstream.binding
            and child["direction"] == parent["direction"]
            and child["channel"] != parent["channel"]
            and child["kind"] == parent["kind"]
            and all(child[key] == parent[key] for key in IDENTITY_FIELDS),
            "adapter forwarding does not bind its upstream message")
    forwarded_payload = dict(child["payload"])
    if child["kind"] == "attempt_completed":
        forwarded_payload.pop("adapter_outcome")
        forwarded_payload.pop("response")
    elif child["kind"] == "ready":
        forwarded_payload.pop("process_observation")
    elif child["kind"] == "session_completed":
        forwarded_payload.pop("adapter_lifecycle")
    require(forwarded_payload == parent["payload"], "adapter changed forwarded control payload")


def _stop_on_error(operation: Callable[..., Any]) -> Callable[..., Any]:
    """A protocol rejection ends continuation; corrected retries are not accepted."""
    @wraps(operation)
    def guarded(self: AttemptGate, *args: Any, **kwargs: Any) -> Any:
        try:
            return operation(self, *args, **kwargs)
        except BaseException:
            self.phase = "stopped"
            raise
    return guarded


class AttemptGate:
    """Require actual successful validation before dispatching any successor.

    The owner supplies each exact planned request and durable start. Completion
    alone leaves the gate closed. No method claims process exit or quiescence.
    """

    def __init__(self, session: dict[str, Any], attempt_count: int):
        self.session = decode(canonical(identity(session)))
        require(0 < unsigned(attempt_count) <= 1_000_000, "invalid session attempt count")
        self.count, self.index = attempt_count, 0
        self.phase = "initializing"
        self.active: dict[str, Any] | None = None
        self.completed: dict[str, Any] | None = None
        self.accepted: list[str] = []

    @_stop_on_error
    def ready(self) -> None:
        """Called only after the actual complete live inventory is validated."""
        require(self.phase == "initializing", "duplicate or late session readiness")
        self.phase = "awaiting_dispatch"

    @_stop_on_error
    def dispatch(self, message: RetainedMessage, *, expected_request: bytes,
                 expected_start: bytes, records: RecordDirectory) -> None:
        """Authenticate request and durable-start bytes before allowing execution."""
        require(self.phase == "awaiting_dispatch", "predecessor is not accepted")
        value = message.decoded()
        validate_message(value)
        payload = value["payload"]
        require(value["kind"] == "dispatch"
                and all(value[key] == item for key, item in self.session.items())
                and payload["session_attempt_index"] == self.index,
                "dispatch differs from the expected session attempt")
        request = decode(expected_request)
        started = decode(expected_start)
        wanted = {key: payload[key] for key in ATTEMPT_FIELDS}
        require(records.read(payload["request"]) == expected_request
                and records.read(payload["attempt_started"]) == expected_start
                and all(started.get(key) == item for key, item in wanted.items())
                and all(request.get(key) == item for key, item in wanted.items() if key != "attempt_id")
                and all(started.get(key) == item for key, item in self.session.items())
                and request.get("session_id") == self.session["session_id"]
                and request.get("session_invocation_nonce") == self.session["session_invocation_nonce"]
                and started.get("request") == payload["request"],
                "dispatch request/start binding differs from registered evidence")
        self.active = wanted
        self.phase = "executing"

    @_stop_on_error
    def complete(self, message: RetainedMessage) -> None:
        """Retain completion references while leaving the continuation gate closed."""
        require(self.phase == "executing", "completion has no active attempt")
        value = message.decoded()
        validate_message(value)
        require(value["kind"] == "attempt_completed"
                and all(value[key] == item for key, item in self.session.items())
                and all(value["payload"][key] == item for key, item in self.active.items()),
                "completion belongs to another invocation")
        self.completed = value["payload"]
        self.phase = "awaiting_validation"

    @_stop_on_error
    def accept(self, message: RetainedMessage, *, records: RecordDirectory,
               validate: Callable[[dict[str, bytes]], None]) -> None:
        """Recheck bound records and invoke the canonical semantic validator."""
        require(self.phase == "awaiting_validation", "acceptance has no completed attempt")
        value = message.decoded()
        validate_message(value)
        payload = value["payload"]
        require(value["kind"] == "accept"
                and all(value[key] == item for key, item in self.session.items())
                and all(payload[key] == item for key, item in self.active.items())
                and all(payload[key] == item for key, item in self.completed.items()),
                "acceptance substitutes a completed record or invocation")
        bound = {key: records.read(payload[key]) for key in payload.keys() - ATTEMPT_FIELDS}
        try:
            validate(bound)
            # A validator may perform lengthy work. Re-read every referenced file
            # before the acknowledgement is allowed to advance this owner.
            require(all(records.read(payload[key]) == raw for key, raw in bound.items()),
                    "acceptance evidence changed during semantic validation")
        except BaseException:
            self.phase = "stopped"
            raise
        self.accepted.append(self.active["request_id"])
        self.index += 1
        self.active = self.completed = None
        self.phase = "completed" if self.index == self.count else "awaiting_dispatch"

    def stop(self) -> None:
        """End continuation without rewriting previously accepted attempts."""
        require(self.phase != "completed", "completed session cannot be stopped retroactively")
        self.phase = "stopped"
