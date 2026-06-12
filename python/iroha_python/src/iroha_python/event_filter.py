"""Helpers for building Torii event filter payloads."""

from __future__ import annotations

import json
from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Any, Dict, Mapping, Optional

from ._privacy_backends import _require_production_verify_backend_label

__all__ = [
    "EventFilter",
    "DataEventFilter",
    "VerifyingKeyFilter",
    "ProofEventFilter",
    "TriggerEventFilter",
    "PipelineTransactionFilter",
    "PipelineBlockFilter",
    "PipelineWitnessFilter",
    "PipelineMergeFilter",
]


class EventFilter(ABC):
    """Base class for event filter helpers."""

    @abstractmethod
    def to_dict(self) -> Dict[str, Any]:
        """Return the Torii event-filter JSON payload."""

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), separators=(",", ":"), sort_keys=False)


def _normalize_proof_hash_hex(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a 32-byte hex string")
    normalized = value.strip().lower()
    if normalized.startswith("0x"):
        normalized = normalized[2:]
    if len(normalized) != 64:
        raise ValueError(f"{context} must be a 32-byte hex string")
    try:
        bytes.fromhex(normalized)
    except ValueError as exc:
        raise ValueError(f"{context} must be a 32-byte hex string") from exc
    return normalized


def _normalize_verifying_key_name(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    normalized = value.strip()
    if not normalized:
        raise ValueError(f"{context} must be a non-empty string")
    if normalized != value:
        raise ValueError(f"{context} must not contain surrounding whitespace")
    if ":" in normalized:
        raise ValueError(f"{context} must not contain ':' characters")
    return normalized


@dataclass(frozen=True)
class VerifyingKeyFilter(EventFilter):
    """Filter verifying-key registry lifecycle events."""

    backend: Optional[str] = None
    name: Optional[str] = None
    registered: bool = True
    updated: bool = True

    def to_dict(self) -> Dict[str, Any]:
        id_matcher: Optional[Dict[str, str]]
        if self.backend is None and self.name is None:
            id_matcher = None
        else:
            if self.backend is None or self.name is None:
                raise ValueError("both backend and name must be provided for verifying key filters")
            backend = _require_production_verify_backend_label(
                self.backend,
                "verifying_key_filter.backend",
            )
            name = _normalize_verifying_key_name(
                self.name,
                "verifying_key_filter.name",
            )
            id_matcher = {"backend": backend, "name": name}
        event_set = {
            "Registered": bool(self.registered),
            "Updated": bool(self.updated),
        }
        if not any(event_set.values()):
            raise ValueError("verifying key filter must enable at least one event type")
        return {
            "VerifyingKey": {
                "id_matcher": id_matcher,
                "event_set": event_set,
            }
        }


@dataclass(frozen=True)
class ProofEventFilter(EventFilter):
    """Filter proof verification events."""

    backend: Optional[str] = None
    proof_hash_hex: Optional[str] = None
    verified: bool = True
    rejected: bool = True

    def to_dict(self) -> Dict[str, Any]:
        if (self.backend is None) != (self.proof_hash_hex is None):
            raise ValueError(
                "proof filter requires both backend and proof_hash_hex when filtering by id"
            )
        id_matcher: Optional[Dict[str, Any]]
        if self.backend is not None and self.proof_hash_hex is not None:
            backend = _require_production_verify_backend_label(
                self.backend,
                "proof_filter.backend",
            )
            proof_hash = _normalize_proof_hash_hex(
                self.proof_hash_hex,
                "proof_filter.proof_hash_hex",
            )
            id_matcher = {"backend": backend, "hash_hex": proof_hash}
        else:
            id_matcher = None
        event_set = {
            "Verified": bool(self.verified),
            "Rejected": bool(self.rejected),
        }
        if not any(event_set.values()):
            raise ValueError("proof filter must enable at least one event type")
        return {
            "Proof": {
                "id_matcher": id_matcher,
                "event_set": event_set,
            }
        }


@dataclass(frozen=True)
class TriggerEventFilter(EventFilter):
    """Filter trigger lifecycle events."""

    trigger_id: Optional[str] = None
    created: bool = True
    deleted: bool = True
    extended: bool = True
    shortened: bool = True
    metadata_inserted: bool = True
    metadata_removed: bool = True

    def to_dict(self) -> Dict[str, Any]:
        event_set = {
            "Created": bool(self.created),
            "Deleted": bool(self.deleted),
            "Extended": bool(self.extended),
            "Shortened": bool(self.shortened),
            "MetadataInserted": bool(self.metadata_inserted),
            "MetadataRemoved": bool(self.metadata_removed),
        }
        if not any(event_set.values()):
            raise ValueError("trigger filter must enable at least one event type")
        return {
            "Trigger": {
                "id_matcher": self.trigger_id,
                "event_set": event_set,
            }
        }


@dataclass(frozen=True)
class PipelineTransactionFilter(EventFilter):
    """Filter pipeline transaction events."""

    hash_hex: Optional[str] = None
    block_height: Optional[int] = None
    status: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        body: Dict[str, Any] = {}
        if self.hash_hex is not None:
            body["hash"] = self.hash_hex
        if self.block_height is not None:
            body["block_height"] = int(self.block_height)
        if self.status is not None:
            body["status"] = self.status
        return {"Pipeline": {"Transaction": body}}


@dataclass(frozen=True)
class PipelineBlockFilter(EventFilter):
    """Filter pipeline block events."""

    height: Optional[int] = None
    status: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        body: Dict[str, Any] = {}
        if self.height is not None:
            body["height"] = int(self.height)
        if self.status is not None:
            body["status"] = self.status
        return {"Pipeline": {"Block": body}}


@dataclass(frozen=True)
class PipelineWitnessFilter(EventFilter):
    """Filter pipeline witness events."""

    block_hash_hex: Optional[str] = None
    height: Optional[int] = None
    view: Optional[int] = None

    def to_dict(self) -> Dict[str, Any]:
        body: Dict[str, Any] = {}
        if self.block_hash_hex is not None:
            body["block_hash"] = self.block_hash_hex
        if self.height is not None:
            body["height"] = int(self.height)
        if self.view is not None:
            body["view"] = int(self.view)
        return {"Pipeline": {"Witness": body}}


@dataclass(frozen=True)
class PipelineMergeFilter(EventFilter):
    """Filter pipeline merge-ledger events."""

    epoch_id: Optional[int] = None

    def to_dict(self) -> Dict[str, Any]:
        body: Dict[str, Any] = {}
        if self.epoch_id is not None:
            body["epoch_id"] = int(self.epoch_id)
        return {"Pipeline": {"Merge": body}}


def _normalize_event_filter_mapping(value: Mapping[str, Any], context: str) -> Dict[str, Any]:
    normalized = dict(value)
    for event_kind in ("VerifyingKey", "Proof"):
        body = value.get(event_kind)
        if not isinstance(body, Mapping):
            continue
        matcher = body.get("id_matcher")
        if not isinstance(matcher, Mapping) or "backend" not in matcher:
            continue
        normalized_matcher = dict(matcher)
        normalized_matcher["backend"] = _require_production_verify_backend_label(
            matcher.get("backend"),
            f"{context}.{event_kind}.id_matcher.backend",
        )
        if event_kind == "Proof":
            if "hash_hex" in matcher:
                normalized_matcher["hash_hex"] = _normalize_proof_hash_hex(
                    matcher.get("hash_hex"),
                    f"{context}.{event_kind}.id_matcher.hash_hex",
                )
            if "proof_hash_hex" in matcher:
                normalized_matcher["proof_hash_hex"] = _normalize_proof_hash_hex(
                    matcher.get("proof_hash_hex"),
                    f"{context}.{event_kind}.id_matcher.proof_hash_hex",
                )
        elif "name" in matcher:
            normalized_matcher["name"] = _normalize_verifying_key_name(
                matcher.get("name"),
                f"{context}.{event_kind}.id_matcher.name",
            )
        normalized_body = dict(body)
        normalized_body["id_matcher"] = normalized_matcher
        normalized[event_kind] = normalized_body
    return normalized


class DataEventFilter(EventFilter):
    """Wrapper to compose data event filters."""

    def __init__(self, inner: Mapping[str, Any]) -> None:
        self._inner = _normalize_event_filter_mapping(inner, "data_event_filter")

    @classmethod
    def verifying_key(cls, *, backend: Optional[str] = None, name: Optional[str] = None,
                      registered: bool = True, updated: bool = True) -> "DataEventFilter":
        return cls(VerifyingKeyFilter(backend=backend, name=name, registered=registered,
                                      updated=updated).to_dict())

    @classmethod
    def proof(cls, *, backend: Optional[str] = None, proof_hash_hex: Optional[str] = None,
              verified: bool = True, rejected: bool = True) -> "DataEventFilter":
        return cls(ProofEventFilter(backend=backend, proof_hash_hex=proof_hash_hex,
                                    verified=verified, rejected=rejected).to_dict())

    @classmethod
    def trigger(
        cls,
        *,
        trigger_id: Optional[str] = None,
        created: bool = True,
        deleted: bool = True,
        extended: bool = True,
        shortened: bool = True,
        metadata_inserted: bool = True,
        metadata_removed: bool = True,
    ) -> "DataEventFilter":
        return cls(
            TriggerEventFilter(
                trigger_id=trigger_id,
                created=created,
                deleted=deleted,
                extended=extended,
                shortened=shortened,
                metadata_inserted=metadata_inserted,
                metadata_removed=metadata_removed,
            ).to_dict()
        )

    @classmethod
    def pipeline_transaction(
        cls,
        *,
        hash_hex: Optional[str] = None,
        block_height: Optional[int] = None,
        status: Optional[str] = None,
    ) -> "DataEventFilter":
        return cls(
            PipelineTransactionFilter(
                hash_hex=hash_hex,
                block_height=block_height,
                status=status,
            ).to_dict()
        )

    @classmethod
    def pipeline_block(
        cls,
        *,
        height: Optional[int] = None,
        status: Optional[str] = None,
    ) -> "DataEventFilter":
        return cls(
            PipelineBlockFilter(
                height=height,
                status=status,
            ).to_dict()
        )

    @classmethod
    def pipeline_witness(
        cls,
        *,
        block_hash_hex: Optional[str] = None,
        height: Optional[int] = None,
        view: Optional[int] = None,
    ) -> "DataEventFilter":
        return cls(
            PipelineWitnessFilter(
                block_hash_hex=block_hash_hex,
                height=height,
                view=view,
            ).to_dict()
        )

    @classmethod
    def pipeline_merge(cls, *, epoch_id: Optional[int] = None) -> "DataEventFilter":
        return cls(PipelineMergeFilter(epoch_id=epoch_id).to_dict())

    def to_dict(self) -> Dict[str, Any]:
        return dict(self._inner)

    def to_json(self) -> str:
        return json.dumps(self._inner, separators=(",", ":"), sort_keys=False)


def ensure_event_filter(value: Optional[object]) -> Optional[str]:
    """Normalize event filters to JSON strings accepted by Torii."""

    if value is None:
        return None
    if isinstance(value, EventFilter):
        return json.dumps(
            _normalize_event_filter_mapping(value.to_dict(), "event_filter"),
            separators=(",", ":"),
            sort_keys=False,
        )
    if isinstance(value, Mapping):
        return json.dumps(
            _normalize_event_filter_mapping(value, "event_filter"),
            separators=(",", ":"),
            sort_keys=False,
        )
    if isinstance(value, str):
        trimmed = value.strip()
        if trimmed and trimmed[0] in "{[":
            try:
                parsed = json.loads(trimmed)
            except json.JSONDecodeError:
                return value
            if isinstance(parsed, Mapping):
                normalized = _normalize_event_filter_mapping(parsed, "event_filter")
                if normalized != parsed:
                    return json.dumps(normalized, separators=(",", ":"), sort_keys=False)
        return value
    raise TypeError("event filter must be EventFilter, mapping, or JSON string")
