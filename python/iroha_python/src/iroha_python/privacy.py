"""Utilities for interacting with the SoraNet privacy telemetry admin surface.

Roadmap item PY6-P4/P5 calls for telemetry/admin helpers so SDKs can ingest the
`/privacy/events` NDJSON stream emitted by relay runtimes.  This module exposes
typed dataclasses plus a convenience fetcher that drains the admin endpoint and
parses each observation into deterministic Python structures.
"""

from __future__ import annotations

import json
from contextlib import closing
from dataclasses import dataclass
from enum import Enum
from typing import Any, Iterator, List, Mapping, Optional, Union

import requests
from requests import Response, Session

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
    DESCRIPTOR_QUOTA = "DescriptorQuota"
    DESCRIPTOR_REPLAY = "DescriptorReplay"

    @classmethod
    def from_value(cls, value: str) -> "PrivacyThrottleScope":
        try:
            return cls(value)
        except ValueError as exc:
            raise TypeError(
                "throttle scope must be one of Congestion, Cooldown, Emergency, "
                "RemoteQuota, DescriptorQuota, DescriptorReplay"
            ) from exc


class PrivacyEventKind(str, Enum):
    """Event kinds surfaced by the privacy admin feed."""

    HANDSHAKE_SUCCESS = "HandshakeSuccess"
    HANDSHAKE_FAILURE = "HandshakeFailure"
    THROTTLE = "Throttle"
    ACTIVE_SAMPLE = "ActiveSample"
    VERIFIED_BYTES = "VerifiedBytes"
    GAR_ABUSE_CATEGORY = "GarAbuseCategory"


@dataclass(frozen=True)
class PrivacyEventHandshakeSuccess:
    rtt_ms: Optional[int]
    active_circuits_after: Optional[int]


@dataclass(frozen=True)
class PrivacyEventHandshakeFailure:
    reason: PrivacyHandshakeFailureReason
    rtt_ms: Optional[int]


@dataclass(frozen=True)
class PrivacyEventThrottle:
    scope: PrivacyThrottleScope


@dataclass(frozen=True)
class PrivacyEventActiveSample:
    active_circuits: int


@dataclass(frozen=True)
class PrivacyEventVerifiedBytes:
    bytes: int


@dataclass(frozen=True)
class PrivacyEventGarAbuseCategory:
    label: str


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


@dataclass(frozen=True)
class PrivacyEvent:
    """Typed representation of a single NDJSON line."""

    timestamp_unix: int
    mode: PrivacyMode
    kind: PrivacyEventKind
    payload: PrivacyEventPayload


def _require_int(obj: Mapping[str, Any], field: str) -> int:
    value = obj.get(field)
    if value is None:
        raise TypeError(f"{field} must be numeric")
    try:
        coerced = int(value)
    except (TypeError, ValueError) as exc:
        raise TypeError(f"{field} must be numeric") from exc
    return coerced


def _require_optional_int(obj: Mapping[str, Any], field: str) -> Optional[int]:
    value = obj.get(field)
    if value is None:
        return None
    return _require_int(obj, field)


def _coerce_str(value: object, field: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{field} must be a string")
    return value


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
        label = payload.get("label")
        if not isinstance(label, str) or not label:
            raise TypeError("GAR abuse payload missing string `label`")
        return PrivacyEventGarAbuseCategory(label=label)
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


def load_privacy_events_from_ndjson(text: str) -> List[PrivacyEvent]:
    """Parse newline-delimited JSON emitted by `/privacy/events`."""

    events: List[PrivacyEvent] = []
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        events.append(parse_privacy_event_line(line))
    return events


def _build_privacy_url(base_url: str, path: str) -> str:
    if not base_url.lower().startswith(("http://", "https://")):
        raise ValueError("base_url must include http:// or https://")
    base = base_url.rstrip("/")
    target = path.lstrip("/")
    if base.endswith(target):
        return base_url
    return f"{base}/{target}"


def fetch_privacy_events(
    base_url: str,
    *,
    session: Optional[Session] = None,
    timeout: float = 10.0,
    path: str = "/privacy/events",
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

    response: Optional[Response] = None
    body = ""
    try:
        if session is None:
            response = requests.get(url, timeout=timeout, headers={"Accept": "application/x-ndjson"})
        else:
            response = session.get(url, timeout=timeout, headers={"Accept": "application/x-ndjson"})
        response.raise_for_status()
        body = response.text
    finally:
        if response is not None:
            response.close()
    return load_privacy_events_from_ndjson(body)


def stream_privacy_events(
    base_url: str,
    *,
    session: Optional[Session] = None,
    timeout: float = 10.0,
    path: str = "/privacy/events",
    chunk_size: int = 65536,
) -> Iterator[PrivacyEvent]:
    """Stream privacy events without buffering the entire NDJSON payload."""

    url = _build_privacy_url(base_url, path)

    if session is None:
        response = requests.get(
            url,
            timeout=timeout,
            headers={"Accept": "application/x-ndjson"},
            stream=True,
        )
    else:
        response = session.get(
            url,
            timeout=timeout,
            headers={"Accept": "application/x-ndjson"},
            stream=True,
        )

    try:
        response.raise_for_status()
    except Exception:
        response.close()
        raise

    def _iterator() -> Iterator[PrivacyEvent]:
        with closing(response):
            for raw_line in response.iter_lines(
                decode_unicode=True,
                chunk_size=chunk_size,
            ):
                if raw_line is None:
                    continue
                line = raw_line.strip()
                if not line:
                    continue
                yield parse_privacy_event_line(line)

    return _iterator()
