"""Typed event records and terminal errors for Torii live streams.

``/v1/events/sse`` sends one JSON object per ``data`` line with ``category``
(``Pipeline``, ``Data`` or ``Other``) and ``event``; :func:`decode_event` turns
it into a typed record (``specs/torii/collection_queries.md``, "Event
streams"). Kinds without a dedicated class, including kinds added after this
SDK, decode as :class:`GenericEvent`, so new server events never break a
stream.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from dataclasses import field as dataclass_field
from decimal import Decimal
from typing import Any, ClassVar, Dict, List, Mapping, Optional, Tuple, Union


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


# ---------------------------------------------------------------------------
# /v1/events/sse payloads
# ---------------------------------------------------------------------------

_U64_MAX = (1 << 64) - 1


def _raw() -> Any:
    return dataclass_field(default_factory=dict, repr=False, compare=False)


def _context(payload: Mapping[str, Any]) -> str:
    return f"{payload.get('category')}/{payload.get('event')} event"


def _required_string(payload: Mapping[str, Any], name: str) -> str:
    value = payload.get(name)
    if not isinstance(value, str):
        raise ValueError(f"{_context(payload)} `{name}` must be a string")
    return value


def _optional_string(payload: Mapping[str, Any], name: str) -> Optional[str]:
    value = payload.get(name)
    if value is not None and not isinstance(value, str):
        raise ValueError(f"{_context(payload)} `{name}` must be a string or null")
    return value


def _optional_u64(payload: Mapping[str, Any], name: str) -> Optional[int]:
    value = payload.get(name)
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= _U64_MAX:
        raise ValueError(f"{_context(payload)} `{name}` must be an unsigned 64-bit integer")
    return value


@dataclass(frozen=True)
class TransactionEvent:
    """``Pipeline``/``Transaction``: a transaction moved through the pipeline.

    ``status`` is ``Queued``, ``Expired``, ``Approved`` or ``Rejected``. A
    rejection carries the stable ``rejection_code`` (``account_does_not_exist``,
    ``limit_check``, ``validation``, ``instruction_execution``,
    ``ivm_execution`` or ``trigger_execution``) and the fixed public
    ``rejection_reason``. ``block_height`` is ``None`` until the transaction is
    in a block.
    """

    category: ClassVar[str] = "Pipeline"
    event: ClassVar[str] = "Transaction"

    hash: str
    status: str
    lane_id: Optional[int] = None
    dataspace_id: Optional[int] = None
    block_height: Optional[int] = None
    rejection_code: Optional[str] = None
    rejection_reason: Optional[str] = None
    raw: Dict[str, Any] = _raw()

    @classmethod
    def from_json(cls, payload: Mapping[str, Any]) -> "TransactionEvent":
        return cls(
            hash=_required_string(payload, "hash"),
            status=_required_string(payload, "status"),
            lane_id=_optional_u64(payload, "lane_id"),
            dataspace_id=_optional_u64(payload, "dataspace_id"),
            block_height=_optional_u64(payload, "block_height"),
            rejection_code=_optional_string(payload, "rejection_code"),
            rejection_reason=_optional_string(payload, "rejection_reason"),
            raw=dict(payload),
        )


@dataclass(frozen=True)
class BlockEvent:
    """``Pipeline``/``Block``: a block moved through the pipeline.

    ``status`` is ``Created``, ``Approved``, ``Rejected``, ``Committed`` or
    ``Applied``; a rejected block carries the rejection variant name in
    ``rejection_code``.
    """

    category: ClassVar[str] = "Pipeline"
    event: ClassVar[str] = "Block"

    status: str
    rejection_code: Optional[str] = None
    raw: Dict[str, Any] = _raw()

    @classmethod
    def from_json(cls, payload: Mapping[str, Any]) -> "BlockEvent":
        return cls(
            status=_required_string(payload, "status"),
            rejection_code=_optional_string(payload, "rejection_code"),
            raw=dict(payload),
        )


@dataclass(frozen=True)
class PipelineWarningEvent:
    """``Pipeline``/``Warning``: a pipeline warning for the block at ``height``."""

    category: ClassVar[str] = "Pipeline"
    event: ClassVar[str] = "Warning"

    kind: str
    details: Optional[str] = None
    height: Optional[int] = None
    raw: Dict[str, Any] = _raw()

    @classmethod
    def from_json(cls, payload: Mapping[str, Any]) -> "PipelineWarningEvent":
        return cls(
            kind=_required_string(payload, "kind"),
            details=_optional_string(payload, "details"),
            height=_optional_u64(payload, "height"),
            raw=dict(payload),
        )


@dataclass(frozen=True)
class WitnessEvent:
    """``Pipeline``/``Witness``: the execution witness of a block (read/write set sizes)."""

    category: ClassVar[str] = "Pipeline"
    event: ClassVar[str] = "Witness"

    block_hash: str
    height: Optional[int] = None
    view: Optional[int] = None
    epoch: Optional[int] = None
    read_count: Optional[int] = None
    write_count: Optional[int] = None
    raw: Dict[str, Any] = _raw()

    @classmethod
    def from_json(cls, payload: Mapping[str, Any]) -> "WitnessEvent":
        return cls(
            block_hash=_required_string(payload, "block_hash"),
            height=_optional_u64(payload, "height"),
            view=_optional_u64(payload, "view"),
            epoch=_optional_u64(payload, "epoch"),
            read_count=_optional_u64(payload, "read_count"),
            write_count=_optional_u64(payload, "write_count"),
            raw=dict(payload),
        )


@dataclass(frozen=True)
class _ProofOutcome:
    backend: str
    proof_hash: str
    call_hash: Optional[str] = None
    envelope_hash: Optional[str] = None
    vk_ref: Optional[str] = None
    vk_commitment: Optional[str] = None
    raw: Dict[str, Any] = _raw()

    @classmethod
    def from_json(cls, payload: Mapping[str, Any]) -> Any:
        return cls(
            backend=_required_string(payload, "backend"),
            proof_hash=_required_string(payload, "proof_hash"),
            call_hash=_optional_string(payload, "call_hash"),
            envelope_hash=_optional_string(payload, "envelope_hash"),
            vk_ref=_optional_string(payload, "vk_ref"),
            vk_commitment=_optional_string(payload, "vk_commitment"),
            raw=dict(payload),
        )


@dataclass(frozen=True)
class ProofVerifiedEvent(_ProofOutcome):
    """``Data``/``ProofVerified``: a proof verified.

    Hashes and ``vk_commitment`` are hex; ``vk_ref`` is ``backend::name``.
    """

    category: ClassVar[str] = "Data"
    event: ClassVar[str] = "ProofVerified"


@dataclass(frozen=True)
class ProofRejectedEvent(_ProofOutcome):
    """``Data``/``ProofRejected``: a proof failed verification.

    Same members as :class:`ProofVerifiedEvent`.
    """

    category: ClassVar[str] = "Data"
    event: ClassVar[str] = "ProofRejected"


@dataclass(frozen=True)
class ProofId:
    """One pruned proof: its ``backend`` and hex ``proof_hash``."""

    backend: str
    proof_hash: str


@dataclass(frozen=True)
class ProofPrunedEvent:
    """``Data``/``ProofPruned``: stored proofs of ``backend`` were pruned."""

    category: ClassVar[str] = "Data"
    event: ClassVar[str] = "ProofPruned"

    backend: str
    removed: Tuple[ProofId, ...] = ()
    removed_count: Optional[int] = None
    remaining: Optional[int] = None
    cap: Optional[int] = None
    grace_blocks: Optional[int] = None
    prune_batch: Optional[int] = None
    pruned_at_height: Optional[int] = None
    pruned_by: Optional[str] = None
    origin: Optional[str] = None
    raw: Dict[str, Any] = _raw()

    @classmethod
    def from_json(cls, payload: Mapping[str, Any]) -> "ProofPrunedEvent":
        entries = payload.get("removed")
        if entries is None:
            entries = []
        if not isinstance(entries, list):
            raise ValueError(f"{_context(payload)} `removed` must be an array")
        removed: List[ProofId] = []
        for entry in entries:
            if not isinstance(entry, Mapping):
                raise ValueError(f"{_context(payload)} `removed` entries must be objects")
            backend, proof_hash = entry.get("backend"), entry.get("proof_hash")
            if not isinstance(backend, str) or not isinstance(proof_hash, str):
                raise ValueError(
                    f"{_context(payload)} `removed` entries need string `backend` and `proof_hash`"
                )
            removed.append(ProofId(backend, proof_hash))
        return cls(
            backend=_required_string(payload, "backend"),
            removed=tuple(removed),
            removed_count=_optional_u64(payload, "removed_count"),
            remaining=_optional_u64(payload, "remaining"),
            cap=_optional_u64(payload, "cap"),
            grace_blocks=_optional_u64(payload, "grace_blocks"),
            prune_batch=_optional_u64(payload, "prune_batch"),
            pruned_at_height=_optional_u64(payload, "pruned_at_height"),
            pruned_by=_optional_string(payload, "pruned_by"),
            origin=_optional_string(payload, "origin"),
            raw=dict(payload),
        )


@dataclass(frozen=True)
class GenericEvent:
    """An event without a dedicated class: ``category`` and ``event`` name it.

    Data-event kinds such as ``Asset`` or ``Domain``, the ``Other`` category
    (``Time``, ``ExecuteTrigger``, ``TriggerCompleted``) and kinds newer than
    this SDK decode to this class. ``summary`` is diagnostic text without a
    stable format; ``raw`` keeps every member Torii sent.
    """

    category: str
    event: str
    summary: Optional[str] = None
    raw: Dict[str, Any] = _raw()


ToriiEvent = Union[
    TransactionEvent,
    BlockEvent,
    PipelineWarningEvent,
    WitnessEvent,
    ProofVerifiedEvent,
    ProofRejectedEvent,
    ProofPrunedEvent,
    GenericEvent,
]

_EVENT_TYPES: Dict[Tuple[str, str], Any] = {
    (kind.category, kind.event): kind
    for kind in (
        TransactionEvent,
        BlockEvent,
        PipelineWarningEvent,
        WitnessEvent,
        ProofVerifiedEvent,
        ProofRejectedEvent,
        ProofPrunedEvent,
    )
}


def _unique_members(pairs: List[Tuple[str, Any]]) -> Dict[str, Any]:
    members: Dict[str, Any] = {}
    for key, value in pairs:
        if key in members:
            raise ValueError(f"duplicate JSON member `{key}`")
        members[key] = value
    return members


def _reject_constant(name: str) -> Any:
    raise ValueError(f"JSON constant {name} is not allowed")


def decode_event(data: Any) -> ToriiEvent:
    """Decode one ``/v1/events/sse`` payload (JSON text or a parsed object).

    Raises :class:`ValueError` when the payload is not a JSON object with
    string ``category`` and ``event`` members, or when a known event carries a
    member of the wrong type. Unknown kinds decode as :class:`GenericEvent`.
    """

    payload = data
    if isinstance(data, (str, bytes, bytearray)):
        try:
            payload = json.loads(
                data,
                object_pairs_hook=_unique_members,
                parse_constant=_reject_constant,
                parse_float=Decimal,
            )
        except ValueError as error:
            raise ValueError(f"event data must be a JSON object: {error}") from None
    if not isinstance(payload, Mapping):
        raise ValueError("event data must be a JSON object")
    category, event = payload.get("category"), payload.get("event")
    if not isinstance(category, str) or not isinstance(event, str):
        raise ValueError("event data must carry string `category` and `event` members")
    kind = _EVENT_TYPES.get((category, event))
    if kind is not None:
        return kind.from_json(payload)
    summary = payload.get("summary")
    if summary is not None and not isinstance(summary, str):
        raise ValueError(f"{_context(payload)} `summary` must be a string")
    return GenericEvent(category=category, event=event, summary=summary, raw=dict(payload))
