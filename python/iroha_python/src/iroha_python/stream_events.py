"""Typed event records and terminal errors for Torii live streams."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Optional


@dataclass(frozen=True)
class SseEvent:
    """Structured Server-Sent Event returned by Torii SSE endpoints."""

    event: Optional[str]
    data: Any
    id: Optional[str]
    retry: Optional[int]
    raw: str


@dataclass(frozen=True)
class WebSocketEvent:
    """Structured JSON event returned by Torii WebSocket event streams."""

    event: Optional[str]
    data: Any
    raw: str


class SseStreamError(RuntimeError):
    """Terminal error reported after an SSE response has been established.

    Canonical Torii live streams cannot change their HTTP status after sending
    the response headers, so they report a terminal ``event: stream_error``
    frame instead. The exception keeps the stable server error code and the
    loss/replay metadata available to callers.
    """

    MALFORMED_CODE = "malformed_stream_error"

    def __init__(
        self,
        code: str,
        message: str,
        *,
        dropped_messages: Optional[int],
        replay_available: Optional[bool],
        payload: Any,
        raw: str,
        malformed_reason: Optional[str] = None,
    ) -> None:
        self.code = code
        self.message = message
        self.dropped_messages = dropped_messages
        self.replay_available = replay_available
        self.payload = payload
        self.raw = raw
        self.malformed_reason = malformed_reason
        detail = f"{code}: {message}"
        if dropped_messages is not None:
            detail = f"{detail} (dropped_messages={dropped_messages})"
        super().__init__(detail)

    @classmethod
    def from_event(cls, event: SseEvent) -> "SseStreamError":
        """Validate and convert a terminal ``stream_error`` SSE frame."""

        payload = event.data
        if isinstance(payload, str):
            try:
                payload = json.loads(payload)
            except json.JSONDecodeError:
                return cls._malformed(event, "data must be a JSON object")
        if not isinstance(payload, Mapping):
            return cls._malformed(event, "data must be a JSON object")

        code = payload.get("code")
        if not isinstance(code, str) or not code.strip():
            return cls._malformed(event, "code must be a non-empty string")
        message = payload.get("message")
        if not isinstance(message, str) or not message.strip():
            return cls._malformed(event, "message must be a non-empty string")
        if "dropped_messages" not in payload:
            return cls._malformed(event, "dropped_messages is required")
        dropped_messages = payload["dropped_messages"]
        if dropped_messages is not None and (
            isinstance(dropped_messages, bool)
            or not isinstance(dropped_messages, int)
            or dropped_messages < 0
        ):
            return cls._malformed(
                event,
                "dropped_messages must be a non-negative integer or null",
            )
        if "replay_available" not in payload:
            return cls._malformed(event, "replay_available is required")
        replay_available = payload["replay_available"]
        if not isinstance(replay_available, bool):
            return cls._malformed(event, "replay_available must be a boolean")
        return cls(
            code,
            message,
            dropped_messages=dropped_messages,
            replay_available=replay_available,
            payload=dict(payload),
            raw=event.raw,
        )

    @classmethod
    def _malformed(cls, event: SseEvent, reason: str) -> "SseStreamError":
        return cls(
            cls.MALFORMED_CODE,
            f"Torii emitted a malformed stream_error event: {reason}",
            dropped_messages=None,
            replay_available=None,
            payload=event.data,
            raw=event.raw,
            malformed_reason=reason,
        )


@dataclass
class EventCursor:
    """Track the last event id for an SSE endpoint with a replay log."""

    last_event_id: Optional[str] = None

    def advance(self, event: SseEvent) -> None:
        """Record the latest event id if present."""

        if event.id is not None:
            self.last_event_id = event.id
