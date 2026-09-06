"""Utilities for interacting with the SoraNet privacy telemetry admin surface.

Roadmap item PY6-P4/P5 calls for telemetry/admin helpers so SDKs can ingest the
`/privacy/events` NDJSON stream emitted by relay runtimes.  This module exposes
typed dataclasses plus a convenience fetcher that drains the admin endpoint and
parses each observation into deterministic Python structures.
"""

from __future__ import annotations

import json
import math
from contextlib import closing
from dataclasses import dataclass
from enum import Enum
from typing import Any, Iterator, List, Mapping, Optional, Union
from urllib.parse import urlsplit, urlunsplit

import requests
from requests import Response, Session

_DEFAULT_NDJSON_MAX_RESPONSE_BYTES = 16 * 1024 * 1024
_DEFAULT_NDJSON_MAX_LINE_BYTES = 1024 * 1024
_DEFAULT_NDJSON_MAX_EVENTS = 100_000

__all__ = [
    "PrivacyMode",
    "PrivacyHandshakeFailureReason",
    "PrivacyThrottleScope",
    "PrivacyEventKind",
    "PrivacyEventHandshakeSuccess",
    "PrivacyEventHandshakeFailure",
    "PrivacyEventThrottle",
    "PrivacyEventActiveSample",
    "PrivacyEventVerifiedBytes",
    "PrivacyEventGarAbuseCategory",
    "PrivacyEvent",
    "parse_privacy_event",
    "parse_privacy_event_line",
    "load_privacy_events_from_ndjson",
    "fetch_privacy_events",
    "stream_privacy_events",
]


class PrivacyMode(str, Enum):
    """Relay mode that generated the telemetry sample."""

    ENTRY = "entry"
    MIDDLE = "middle"
    EXIT = "exit"

    @classmethod
    def from_value(cls, value: str) -> "PrivacyMode":
        try:
            return cls(value)
        except ValueError as exc:  # pragma: no cover - defensive
            raise TypeError("privacy mode must be one of entry/middle/exit") from exc


class PrivacyHandshakeFailureReason(str, Enum):
    """Classification surfaced for handshake failures."""

    POW = "Pow"
    TIMEOUT = "Timeout"
    DOWNGRADE = "Downgrade"
    OTHER = "Other"

    @classmethod
    def from_value(cls, value: str) -> "PrivacyHandshakeFailureReason":
        try:
            return cls(value)
        except ValueError as exc:
            raise TypeError(
                "handshake failure reason must be Pow, Timeout, Downgrade, or Other"
            ) from exc


class PrivacyThrottleScope(str, Enum):
    """Throttle scopes emitted by the relay runtime."""

    CONGESTION = "Congestion"
    COOLDOWN = "Cooldown"
    EMERGENCY = "Emergency"
    REMOTE_QUOTA = "RemoteQuota"

    @classmethod
    def from_value(cls, value: str) -> "PrivacyThrottleScope":
        try:
            return cls(value)
        except ValueError as exc:
            raise TypeError(
                "throttle scope must be one of Congestion, Cooldown, Emergency, or RemoteQuota"
            ) from exc


class PrivacyEventKind(str, Enum):
    """Event kinds surfaced by the privacy admin feed."""

    HANDSHAKE_SUCCESS = "HandshakeSuccess"
    HANDSHAKE_FAILURE = "HandshakeFailure"
    THROTTLE = "Throttle"
    ACTIVE_SAMPLE = "ActiveSample"
    VERIFIED_BYTES = "VerifiedBytes"
    GAR_ABUSE_CATEGORY = "GarAbuseCategory"


@dataclass(frozen=True, slots=True)
class PrivacyEventHandshakeSuccess:
    rtt_ms: Optional[int]
    active_circuits_after: Optional[int]


@dataclass(frozen=True, slots=True)
class PrivacyEventHandshakeFailure:
    reason: PrivacyHandshakeFailureReason
    rtt_ms: Optional[int]


@dataclass(frozen=True, slots=True)
class PrivacyEventThrottle:
    scope: PrivacyThrottleScope


@dataclass(frozen=True, slots=True)
class PrivacyEventActiveSample:
    active_circuits: int


@dataclass(frozen=True, slots=True)
class PrivacyEventVerifiedBytes:
    bytes: int


@dataclass(frozen=True, slots=True)
class PrivacyEventGarAbuseCategory:
    """GAR category represented only by its fixed eight-byte digest."""

    category_hash: bytes


PrivacyEventPayload = Optional[
    Union[
        PrivacyEventHandshakeSuccess,
        PrivacyEventHandshakeFailure,
        PrivacyEventThrottle,
        PrivacyEventActiveSample,
        PrivacyEventVerifiedBytes,
        PrivacyEventGarAbuseCategory,
    ]
]


@dataclass(frozen=True, slots=True)
class PrivacyEvent:
    """Typed representation of a single NDJSON line."""

    timestamp_unix: int
    mode: PrivacyMode
    kind: PrivacyEventKind
    payload: PrivacyEventPayload


def _require_int(obj: Mapping[str, Any], field: str) -> int:
    value = obj.get(field)
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{field} must be a non-negative integer")
    if value < 0:
        raise ValueError(f"{field} must be a non-negative integer")
    return value


def _require_optional_int(obj: Mapping[str, Any], field: str) -> Optional[int]:
    value = obj.get(field)
    if value is None:
        return None
    return _require_int(obj, field)


def _require_fixed_bytes(
    obj: Mapping[str, object], field: str, expected_len: int
) -> bytes:
    value = obj.get(field)
    if not isinstance(value, (list, tuple)) or len(value) != expected_len:
        raise TypeError(f"{field} must be an array of exactly {expected_len} bytes")
    output = bytearray()
    for item in value:
        if isinstance(item, bool) or not isinstance(item, int) or not 0 <= item <= 255:
            raise TypeError(f"{field} must contain only integer bytes")
        output.append(item)
    return bytes(output)


def _parse_payload(kind: PrivacyEventKind, payload: Optional[Mapping[str, object]]) -> PrivacyEventPayload:
    if kind is PrivacyEventKind.HANDSHAKE_SUCCESS:
        payload = payload or {}
        return PrivacyEventHandshakeSuccess(
            rtt_ms=_require_optional_int(payload, "rtt_ms"),
            active_circuits_after=_require_optional_int(payload, "active_circuits_after"),
        )
    if kind is PrivacyEventKind.HANDSHAKE_FAILURE:
        if payload is None:
            raise TypeError("handshake failure payload missing")
        reason = payload.get("reason")
        if not isinstance(reason, str):
            raise TypeError("handshake failure payload missing string `reason`")
        return PrivacyEventHandshakeFailure(
            reason=PrivacyHandshakeFailureReason.from_value(reason),
            rtt_ms=_require_optional_int(payload, "rtt_ms"),
        )
    if kind is PrivacyEventKind.THROTTLE:
        if payload is None:
            raise TypeError("throttle payload missing")
        scope = payload.get("scope")
        if not isinstance(scope, str):
            raise TypeError("throttle payload missing string `scope`")
        return PrivacyEventThrottle(scope=PrivacyThrottleScope.from_value(scope))
    if kind is PrivacyEventKind.ACTIVE_SAMPLE:
        if payload is None:
            raise TypeError("active sample payload missing")
        return PrivacyEventActiveSample(active_circuits=_require_int(payload, "active_circuits"))
    if kind is PrivacyEventKind.VERIFIED_BYTES:
        if payload is None:
            raise TypeError("verified bytes payload missing")
        return PrivacyEventVerifiedBytes(bytes=_require_int(payload, "bytes"))
    if kind is PrivacyEventKind.GAR_ABUSE_CATEGORY:
        if payload is None:
            raise TypeError("GAR abuse payload missing")
        return PrivacyEventGarAbuseCategory(
            category_hash=_require_fixed_bytes(payload, "category_hash", 8)
        )
    return None


def parse_privacy_event(obj: Mapping[str, Any]) -> PrivacyEvent:
    """Parse a raw JSON object into :class:`PrivacyEvent`."""

    timestamp = _require_int(obj, "timestamp_unix")
    mode_value = obj.get("mode")
    if not isinstance(mode_value, str):
        raise TypeError("privacy event missing string `mode` field")
    mode = PrivacyMode.from_value(mode_value)

    kind_value = obj.get("kind")
    if not isinstance(kind_value, str):
        raise TypeError("privacy event missing string `kind` field")
    try:
        kind = PrivacyEventKind(kind_value)
    except ValueError as exc:
        raise TypeError(f"unknown privacy event kind `{kind_value}`") from exc

    payload_obj_raw = obj.get("payload")
    if payload_obj_raw is not None and not isinstance(payload_obj_raw, Mapping):
        raise TypeError("privacy event payload must be an object when present")
    payload_obj = payload_obj_raw if isinstance(payload_obj_raw, Mapping) else None
    payload = _parse_payload(kind, payload_obj)

    return PrivacyEvent(
        timestamp_unix=timestamp,
        mode=mode,
        kind=kind,
        payload=payload,
    )


def parse_privacy_event_line(line: str) -> PrivacyEvent:
    """Parse a single NDJSON line into :class:`PrivacyEvent`."""

    try:
        obj = json.loads(line)
    except json.JSONDecodeError as exc:
        raise ValueError("privacy event line is not valid JSON") from exc
    if not isinstance(obj, Mapping):
        raise TypeError("privacy event line must decode to an object")
    return parse_privacy_event(obj)


def load_privacy_events_from_ndjson(
    text: str,
    *,
    maximum_line_bytes: int = _DEFAULT_NDJSON_MAX_LINE_BYTES,
    maximum_events: int = _DEFAULT_NDJSON_MAX_EVENTS,
) -> List[PrivacyEvent]:
    """Parse newline-delimited JSON emitted by `/privacy/events`."""

    if not isinstance(text, str):
        raise TypeError("text must be a string")
    _require_positive_int(maximum_line_bytes, "maximum_line_bytes")
    _require_positive_int(maximum_events, "maximum_events")
    events: List[PrivacyEvent] = []
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        if len(line.encode("utf-8")) > maximum_line_bytes:
            raise ValueError(
                f"privacy event line exceeds the {maximum_line_bytes}-byte limit"
            )
        if len(events) >= maximum_events:
            raise ValueError(f"privacy event feed exceeds the {maximum_events}-event limit")
        events.append(parse_privacy_event_line(line))
    return events


def _build_privacy_url(base_url: str, path: str) -> str:
    if not isinstance(base_url, str) or not base_url or base_url != base_url.strip():
        raise ValueError("base_url must be an exact non-empty HTTP(S) URL")
    if "\\" in base_url or any(
        ord(character) <= 0x20 or ord(character) == 0x7F for character in base_url
    ):
        raise ValueError("base_url must not contain backslashes, spaces, or control characters")
    if not isinstance(path, str) or not path.startswith("/") or path.startswith("//"):
        raise ValueError("path must be an origin-relative path beginning with one slash")
    if "\\" in path or any(
        ord(character) <= 0x20 or ord(character) == 0x7F for character in path
    ):
        raise ValueError("path must not contain backslashes, spaces, or control characters")
    target = urlsplit(path)
    if target.scheme or target.netloc or target.query or target.fragment:
        raise ValueError("path must not contain an origin, query, or fragment")
    parsed = urlsplit(base_url)
    if parsed.scheme.lower() not in {"http", "https"} or parsed.hostname is None:
        raise ValueError("base_url must include an HTTP(S) origin")
    if parsed.username is not None or parsed.password is not None:
        raise ValueError("base_url must not include credentials")
    if parsed.query or parsed.fragment:
        raise ValueError("base_url must not include a query or fragment")
    try:
        _ = parsed.port
    except ValueError as exc:
        raise ValueError("base_url contains an invalid port") from exc
    base_path = parsed.path.rstrip("/")
    target_path = target.path.rstrip("/") or "/"
    if base_path not in {"", target_path}:
        raise ValueError("base_url path must be empty or the exact privacy endpoint path")
    return urlunsplit((parsed.scheme.lower(), parsed.netloc, target_path, "", ""))


def _require_positive_int(value: Any, context: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{context} must be a positive integer")
    if value <= 0:
        raise ValueError(f"{context} must be a positive integer")
    return value


def _require_positive_timeout(value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise TypeError("timeout must be a finite positive number")
    timeout = float(value)
    if not math.isfinite(timeout) or timeout <= 0:
        raise ValueError("timeout must be a finite positive number")
    return timeout


def _read_bounded_response(response: Response, maximum_bytes: int) -> bytes:
    content_length = response.headers.get("Content-Length")
    if (
        content_length is not None
        and content_length.isascii()
        and content_length.isdecimal()
        and int(content_length, 10) > maximum_bytes
    ):
        raise ValueError(f"privacy response exceeds the {maximum_bytes}-byte limit")
    body = bytearray()
    for chunk in response.iter_content(chunk_size=64 * 1024):
        if not isinstance(chunk, bytes):
            raise TypeError("privacy response yielded a non-bytes body chunk")
        if len(body) + len(chunk) > maximum_bytes:
            raise ValueError(f"privacy response exceeds the {maximum_bytes}-byte limit")
        body.extend(chunk)
    return bytes(body)


def _iter_bounded_ndjson_lines(
    response: Response,
    *,
    chunk_size: int,
    maximum_line_bytes: int,
) -> Iterator[str]:
    pending = bytearray()
    for chunk in response.iter_content(chunk_size=chunk_size):
        if not isinstance(chunk, bytes):
            raise TypeError("privacy response yielded a non-bytes body chunk")
        pending.extend(chunk)
        while True:
            newline = pending.find(b"\n")
            if newline < 0:
                break
            raw_line = bytes(pending[:newline])
            del pending[: newline + 1]
            if raw_line.endswith(b"\r"):
                raw_line = raw_line[:-1]
            if len(raw_line) > maximum_line_bytes:
                raise ValueError(
                    f"privacy event line exceeds the {maximum_line_bytes}-byte limit"
                )
            yield raw_line.decode("utf-8", "strict")
        if len(pending) > maximum_line_bytes:
            raise ValueError(
                f"privacy event line exceeds the {maximum_line_bytes}-byte limit"
            )
    if pending:
        yield bytes(pending).decode("utf-8", "strict")


def fetch_privacy_events(
    base_url: str,
    *,
    session: Optional[Session] = None,
    timeout: float = 10.0,
    path: str = "/privacy/events",
    maximum_response_bytes: int = _DEFAULT_NDJSON_MAX_RESPONSE_BYTES,
    maximum_line_bytes: int = _DEFAULT_NDJSON_MAX_LINE_BYTES,
    maximum_events: int = _DEFAULT_NDJSON_MAX_EVENTS,
) -> List[PrivacyEvent]:
    """Fetch and parse the relay admin NDJSON feed.

    Parameters
    ----------
    base_url:
        Either the relay admin base URL (e.g. ``http://relay:7070``) or a full
        `/privacy/events` endpoint.
    session:
        Optional :class:`requests.Session` used for HTTP requests.  When
        omitted, the module-level :mod:`requests` helpers are used.
    timeout:
        Request timeout in seconds (passed to :func:`requests.get`).
    path:
        Endpoint appended to ``base_url`` when it does not already end with
        ``/privacy/events``. Override only when relays expose the feed through
        a different path (e.g., `/admin/privacy/events`).
    """

    url = _build_privacy_url(base_url, path)

    _require_positive_int(maximum_response_bytes, "maximum_response_bytes")
    _require_positive_int(maximum_line_bytes, "maximum_line_bytes")
    _require_positive_int(maximum_events, "maximum_events")
    request_timeout = _require_positive_timeout(timeout)
    owned_session = session is None
    active_session = session if session is not None else requests.Session()
    if owned_session:
        active_session.trust_env = False
    response: Optional[Response] = None
    try:
        response = active_session.get(
            url,
            timeout=request_timeout,
            headers={"Accept": "application/x-ndjson"},
            allow_redirects=False,
            stream=True,
        )
        response.raise_for_status()
        body = _read_bounded_response(response, maximum_response_bytes)
    finally:
        if response is not None:
            response.close()
        if owned_session:
            active_session.close()
    return load_privacy_events_from_ndjson(
        body.decode("utf-8", "strict"),
        maximum_line_bytes=maximum_line_bytes,
        maximum_events=maximum_events,
    )


def stream_privacy_events(
    base_url: str,
    *,
    session: Optional[Session] = None,
    timeout: float = 10.0,
    path: str = "/privacy/events",
    chunk_size: int = 65536,
    maximum_line_bytes: int = _DEFAULT_NDJSON_MAX_LINE_BYTES,
) -> Iterator[PrivacyEvent]:
    """Stream privacy events without buffering the entire NDJSON payload."""

    url = _build_privacy_url(base_url, path)

    _require_positive_int(chunk_size, "chunk_size")
    _require_positive_int(maximum_line_bytes, "maximum_line_bytes")
    request_timeout = _require_positive_timeout(timeout)

    def _iterator() -> Iterator[PrivacyEvent]:
        owned_session = session is None
        active_session = session if session is not None else requests.Session()
        if owned_session:
            active_session.trust_env = False
        response: Optional[Response] = None
        try:
            response = active_session.get(
                url,
                timeout=request_timeout,
                headers={"Accept": "application/x-ndjson"},
                allow_redirects=False,
                stream=True,
            )
            response.raise_for_status()
            with closing(response):
                for raw_line in _iter_bounded_ndjson_lines(
                    response,
                    chunk_size=min(chunk_size, 64 * 1024),
                    maximum_line_bytes=maximum_line_bytes,
                ):
                    line = raw_line.strip()
                    if not line:
                        continue
                    yield parse_privacy_event_line(line)
        finally:
            if response is not None:
                response.close()
            if owned_session:
                active_session.close()

    return _iterator()
