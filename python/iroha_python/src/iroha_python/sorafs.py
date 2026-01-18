"""SoraFS helpers for alias proofs and multi-source orchestrator bindings."""

from __future__ import annotations

import json
import logging
import os
from dataclasses import dataclass
from pathlib import Path
from typing import (
    TYPE_CHECKING,
    Any,
    Callable,
    Dict,
    Iterable,
    Mapping,
    MutableMapping,
    Optional,
    Sequence,
    Tuple,
    Union,
)

from . import _crypto as _crypto_module

_crypto: Any = _crypto_module

if TYPE_CHECKING:
    from .client import (
        SorafsPorIngestionProviderStatus,
        SorafsPorIngestionStatus,
        SorafsPorObservationResponse,
        SorafsPorSubmissionResponse,
        SorafsPorVerdictResponse,
    )
else:
    try:
        from .client import (
            SorafsPorIngestionProviderStatus,
            SorafsPorIngestionStatus,
            SorafsPorObservationResponse,
            SorafsPorSubmissionResponse,
            SorafsPorVerdictResponse,
        )
    except ImportError:  # pragma: no cover - circular import during boot
        class SorafsPorSubmissionResponse:
            """Fallback placeholder until the client module finishes importing."""

            pass

        class SorafsPorObservationResponse(SorafsPorSubmissionResponse):
            """Fallback placeholder until the client module finishes importing."""

            pass

        class SorafsPorVerdictResponse(SorafsPorSubmissionResponse):
            """Fallback placeholder until the client module finishes importing."""

            pass

        class SorafsPorIngestionStatus:
            """Fallback placeholder until the client module finishes importing."""

            pass

        class SorafsPorIngestionProviderStatus(SorafsPorIngestionStatus):
            """Fallback placeholder until the client module finishes importing."""

            pass

__all__ = [
    "SorafsAliasPolicy",
    "SorafsAliasEvaluation",
    "SorafsAliasWarning",
    "SorafsAliasError",
    "evaluate_alias_proof",
    "enforce_alias_policy",
    "alias_proof_fixture",
    "SorafsReplicationAssignment",
    "SorafsReplicationSla",
    "SorafsReplicationMetadataEntry",
    "SorafsReplicationOrder",
    "decode_replication_order",
    "SorafsRangeCapability",
    "SorafsStreamBudget",
    "SorafsTransportHint",
    "SorafsProviderMetadata",
    "SorafsLocalProviderSpec",
    "SorafsTelemetryEntry",
    "SorafsProviderBoost",
    "SorafsMultiFetchOptions",
    "SorafsProviderReport",
    "SorafsChunkReceipt",
    "SorafsScoreboardRow",
    "SorafsMultiFetchResult",
    "SorafsMultiFetchError",
    "multi_fetch_local",
]

_LOGGER = logging.getLogger("iroha_python.sorafs")
_HEADER_SORA_NAME = "Sora-Name"
_HEADER_SORA_PROOF = "Sora-Proof"
_HEADER_SORA_PROOF_STATUS = "Sora-Proof-Status"

if TYPE_CHECKING:
    class SorafsMultiFetchError(RuntimeError):
        """Raised when multi-source fetch fails in the native extension."""

        pass
else:
    try:
        class SorafsMultiFetchError(_crypto.SorafsMultiFetchError):
            """Raised when multi-source fetch fails in the native extension."""

            pass
    except AttributeError:  # pragma: no cover - optional dependency
        class SorafsMultiFetchError(RuntimeError):
            """Stub used when the native extension is unavailable in dev environments."""

            pass


def _coerce_positive_int(value: Any, field: str) -> int:
    try:
        integer = int(value)
    except (TypeError, ValueError) as exc:
        raise ValueError(f"{field} must be a positive integer") from exc
    if integer <= 0:
        raise ValueError(f"{field} must be greater than zero")
    return integer


def _coerce_non_negative_int(value: Any, field: str) -> int:
    try:
        integer = int(value)
    except (TypeError, ValueError) as exc:
        raise ValueError(f"{field} must be a non-negative integer") from exc
    if integer < 0:
        raise ValueError(f"{field} must be non-negative")
    return integer


def _first_present(mapping: Mapping[str, Any], *keys: str) -> Optional[Any]:
    for key in keys:
        if key in mapping:
            return mapping[key]
    return None


def _default_policy_payload() -> Mapping[str, Any]:
    payload = _crypto.sorafs_alias_policy_defaults()
    if not isinstance(payload, Mapping):
        raise TypeError("alias policy defaults returned an unexpected payload")
    # Older `_crypto` wheels (pre successor/governance grace) omit the newer fields.
    if "successor_grace_secs" not in payload or "governance_grace_secs" not in payload:
        merged = dict(payload)
        merged.setdefault("successor_grace_secs", 0)
        merged.setdefault("governance_grace_secs", 0)
        return merged
    return payload


@dataclass(frozen=True)
class SorafsAliasPolicy:
    """Cache policy thresholds mirroring the Torii gateway defaults."""

    positive_ttl_secs: int
    refresh_window_secs: int
    hard_expiry_secs: int
    negative_ttl_secs: int
    revocation_ttl_secs: int
    rotation_max_age_secs: int
    successor_grace_secs: int
    governance_grace_secs: int

    def __post_init__(self) -> None:
        if self.refresh_window_secs > self.positive_ttl_secs:
            raise ValueError("refresh_window_secs must not exceed positive_ttl_secs")
        if self.hard_expiry_secs < self.positive_ttl_secs:
            raise ValueError("hard_expiry_secs must be greater than or equal to positive_ttl_secs")
        if self.successor_grace_secs < 0:
            raise ValueError("successor_grace_secs must be non-negative")
        if self.governance_grace_secs < 0:
            raise ValueError("governance_grace_secs must be non-negative")

    @classmethod
    def defaults(cls) -> "SorafsAliasPolicy":
        """Return the canonical SoraFS alias policy."""

        defaults = _default_policy_payload()
        return cls(
            positive_ttl_secs=_coerce_positive_int(
                defaults.get("positive_ttl_secs"), "positive_ttl_secs"
            ),
            refresh_window_secs=_coerce_positive_int(
                defaults.get("refresh_window_secs"), "refresh_window_secs"
            ),
            hard_expiry_secs=_coerce_positive_int(
                defaults.get("hard_expiry_secs"), "hard_expiry_secs"
            ),
            negative_ttl_secs=_coerce_positive_int(
                defaults.get("negative_ttl_secs"), "negative_ttl_secs"
            ),
            revocation_ttl_secs=_coerce_positive_int(
                defaults.get("revocation_ttl_secs"), "revocation_ttl_secs"
            ),
            rotation_max_age_secs=_coerce_positive_int(
                defaults.get("rotation_max_age_secs"), "rotation_max_age_secs"
            ),
            successor_grace_secs=_coerce_non_negative_int(
                defaults.get("successor_grace_secs"), "successor_grace_secs"
            ),
            governance_grace_secs=_coerce_non_negative_int(
                defaults.get("governance_grace_secs"), "governance_grace_secs"
            ),
        )

    @classmethod
    def from_mapping(cls, mapping: Mapping[str, Any]) -> "SorafsAliasPolicy":
        """Construct a policy from a mapping, accepting snake_case and camelCase keys."""

        if isinstance(mapping, SorafsAliasPolicy):
            return mapping
        positive = _first_present(mapping, "positive_ttl_secs", "positiveTtlSecs")
        refresh = _first_present(mapping, "refresh_window_secs", "refreshWindowSecs")
        hard = _first_present(mapping, "hard_expiry_secs", "hardExpirySecs")
        negative = _first_present(mapping, "negative_ttl_secs", "negativeTtlSecs")
        revocation = _first_present(mapping, "revocation_ttl_secs", "revocationTtlSecs")
        rotation = _first_present(mapping, "rotation_max_age_secs", "rotationMaxAgeSecs")
        successor = _first_present(mapping, "successor_grace_secs", "successorGraceSecs")
        governance = _first_present(mapping, "governance_grace_secs", "governanceGraceSecs")
        defaults_payload = _default_policy_payload()
        return cls(
            positive_ttl_secs=_coerce_positive_int(
                positive if positive is not None else defaults_payload.get("positive_ttl_secs"),
                "positive_ttl_secs",
            ),
            refresh_window_secs=_coerce_positive_int(
                refresh if refresh is not None else defaults_payload.get("refresh_window_secs"),
                "refresh_window_secs",
            ),
            hard_expiry_secs=_coerce_positive_int(
                hard if hard is not None else defaults_payload.get("hard_expiry_secs"),
                "hard_expiry_secs",
            ),
            negative_ttl_secs=_coerce_positive_int(
                negative if negative is not None else defaults_payload.get("negative_ttl_secs"),
                "negative_ttl_secs",
            ),
            revocation_ttl_secs=_coerce_positive_int(
                revocation if revocation is not None else defaults_payload.get("revocation_ttl_secs"),
                "revocation_ttl_secs",
            ),
            rotation_max_age_secs=_coerce_positive_int(
                rotation if rotation is not None else defaults_payload.get("rotation_max_age_secs"),
                "rotation_max_age_secs",
            ),
            successor_grace_secs=_coerce_non_negative_int(
                successor if successor is not None else defaults_payload.get("successor_grace_secs"),
                "successor_grace_secs",
            ),
            governance_grace_secs=_coerce_non_negative_int(
                governance if governance is not None else defaults_payload.get("governance_grace_secs"),
                "governance_grace_secs",
            ),
        )

    def to_mapping(self) -> Dict[str, int]:
        """Return the policy encoded in snake_case keys."""

        return {
            "positive_ttl_secs": self.positive_ttl_secs,
            "refresh_window_secs": self.refresh_window_secs,
            "hard_expiry_secs": self.hard_expiry_secs,
            "negative_ttl_secs": self.negative_ttl_secs,
            "revocation_ttl_secs": self.revocation_ttl_secs,
            "rotation_max_age_secs": self.rotation_max_age_secs,
            "successor_grace_secs": self.successor_grace_secs,
            "governance_grace_secs": self.governance_grace_secs,
        }


@dataclass(frozen=True)
class SorafsAliasEvaluation:
    """Result of validating a `Sora-Proof` bundle."""

    state: str
    status_label: str
    rotation_due: bool
    age_seconds: int
    generated_at_unix: int
    expires_at_unix: int
    expires_in_seconds: Optional[int]
    servable: bool

    @classmethod
    def from_mapping(cls, mapping: Mapping[str, Any]) -> "SorafsAliasEvaluation":
        return cls(
            state=str(mapping.get("state")),
            status_label=str(mapping.get("status_label")),
            rotation_due=bool(mapping.get("rotation_due", False)),
            age_seconds=int(mapping.get("age_seconds", 0)),
            generated_at_unix=int(mapping.get("generated_at_unix", 0)),
            expires_at_unix=int(mapping.get("expires_at_unix", 0)),
            expires_in_seconds=(
                None
                if mapping.get("expires_in_seconds") is None
                else int(mapping["expires_in_seconds"])
            ),
            servable=bool(mapping.get("servable", False)),
        )


@dataclass(frozen=True)
class SorafsAliasWarning:
    """Warning raised when proofs enter the refresh window or rotation deadline."""

    alias: Optional[str]
    evaluation: SorafsAliasEvaluation
    status_header: Optional[str]


class SorafsAliasError(RuntimeError):
    """Raised when alias proofs are missing or fail policy checks."""


@dataclass(frozen=True)
class SorafsReplicationAssignment:
    """Assignment binding a provider to store a manifest slice."""

    provider_id_hex: str
    slice_gib: int
    lane: Optional[str]


@dataclass(frozen=True)
class SorafsReplicationSla:
    """Service-level agreement expectations for a replication order."""

    ingest_deadline_secs: int
    min_availability_percent_milli: int
    min_por_success_percent_milli: int


@dataclass(frozen=True)
class SorafsReplicationMetadataEntry:
    """Metadata entry embedded in a replication order."""

    key: str
    value: str


@dataclass(frozen=True)
class SorafsReplicationOrder:
    """Structured view over a Norito-encoded replication order."""

    schema_version: int
    order_id_hex: str
    manifest_cid_utf8: Optional[str]
    manifest_cid_base64: str
    manifest_digest_hex: str
    chunking_profile: str
    target_replicas: int
    assignments: Tuple[SorafsReplicationAssignment, ...]
    issued_at_unix: int
    deadline_at_unix: int
    sla: SorafsReplicationSla
    metadata: Tuple[SorafsReplicationMetadataEntry, ...]


def evaluate_alias_proof(
    proof_b64: str,
    policy: Optional[SorafsAliasPolicy] = None,
    *,
    now_secs: Optional[int] = None,
) -> SorafsAliasEvaluation:
    """Validate a base64-encoded alias proof bundle."""

    policy_value = policy or SorafsAliasPolicy.defaults()
    result = _crypto.sorafs_evaluate_alias_proof(
        proof_b64,
        policy_value.to_mapping(),
        now_secs,
    )
    if not isinstance(result, Mapping):
        raise TypeError("alias proof evaluation returned unexpected payload")
    return SorafsAliasEvaluation.from_mapping(result)


def enforce_alias_policy(
    response: Any,
    *,
    policy: Optional[SorafsAliasPolicy] = None,
    now_secs: Optional[int] = None,
    warning_hook: Optional[Callable[[SorafsAliasWarning], None]] = None,
    logger: Optional[logging.Logger] = None,
) -> Optional[SorafsAliasEvaluation]:
    """Inspect an HTTP response and enforce SoraFS alias policy when headers are present."""

    status = getattr(response, "status_code", None)
    if status != 200:
        return None
    headers = getattr(response, "headers", None)
    if not isinstance(headers, MutableMapping):
        return None
    proof_b64 = headers.get(_HEADER_SORA_PROOF)
    if not proof_b64:
        return None
    alias = headers.get(_HEADER_SORA_NAME)
    status_header = headers.get(_HEADER_SORA_PROOF_STATUS)

    evaluation = evaluate_alias_proof(
        str(proof_b64),
        policy=policy,
        now_secs=now_secs,
    )
    if not evaluation.servable:
        status_hint = f"; header reported {status_header}" if status_header else ""
        alias_label = alias if alias else "<unknown>"
        raise SorafsAliasError(
            f"alias proof for '{alias_label}' rejected: state {evaluation.status_label}{status_hint} "
            f"(age {evaluation.age_seconds} seconds)"
        )

    needs_warning = evaluation.state == "refresh_window" or evaluation.rotation_due
    if needs_warning:
        warning = SorafsAliasWarning(alias=alias, evaluation=evaluation, status_header=status_header)
        if warning_hook:
            warning_hook(warning)
        (logger or _LOGGER).warning(
            "SoraFS alias '%s' nearing refresh window: status=%s age=%s",
            alias or "<unknown>",
            evaluation.status_label,
            evaluation.age_seconds,
        )

    return evaluation


def alias_proof_fixture(**options: Any) -> Mapping[str, Any]:
    """Return a deterministic alias proof fixture for testing."""

    payload = _crypto.sorafs_alias_proof_fixture(options or None)
    if not isinstance(payload, Mapping):
        raise TypeError("alias_proof_fixture returned an unexpected value")
    return payload


def decode_replication_order(norito_bytes: bytes | bytearray | memoryview) -> SorafsReplicationOrder:
    """Decode a Norito-encoded replication order into typed dataclasses."""

    if isinstance(norito_bytes, memoryview):
        payload = bytes(norito_bytes)
    elif isinstance(norito_bytes, (bytes, bytearray)):
        payload = bytes(norito_bytes)
    else:
        raise TypeError("norito_bytes must be bytes-like")

    mapping = _crypto.sorafs_decode_replication_order(payload)
    if not isinstance(mapping, Mapping):
        raise TypeError("replication order decode returned unexpected payload")

    schema_version = int(mapping.get("schema_version", 0))
    order_id_hex = str(mapping.get("order_id_hex", ""))
    manifest_cid_utf8 = mapping.get("manifest_cid_utf8")
    if manifest_cid_utf8 is not None:
        manifest_cid_utf8 = str(manifest_cid_utf8)
    manifest_cid_base64 = str(mapping.get("manifest_cid_base64", ""))
    manifest_digest_hex = str(mapping.get("manifest_digest_hex", ""))
    chunking_profile = str(mapping.get("chunking_profile", ""))
    target_replicas = int(mapping.get("target_replicas", 0))
    issued_at_unix = int(mapping.get("issued_at_unix", 0))
    deadline_at_unix = int(mapping.get("deadline_at_unix", 0))

    sla_mapping = mapping.get("sla")
    if not isinstance(sla_mapping, Mapping):
        sla_mapping = {}
    sla = SorafsReplicationSla(
        ingest_deadline_secs=int(sla_mapping.get("ingest_deadline_secs", 0)),
        min_availability_percent_milli=int(
            sla_mapping.get("min_availability_percent_milli", 0)
        ),
        min_por_success_percent_milli=int(
            sla_mapping.get("min_por_success_percent_milli", 0)
        ),
    )

    assignments_raw = mapping.get("assignments", [])
    assignments: Tuple[SorafsReplicationAssignment, ...] = tuple(
        SorafsReplicationAssignment(
            provider_id_hex=str(entry.get("provider_id_hex", "")),
            slice_gib=int(entry.get("slice_gib", 0)),
            lane=(
                None
                if entry.get("lane") is None
                else str(entry.get("lane"))
            ),
        )
        for entry in assignments_raw
        if isinstance(entry, Mapping)
    )

    metadata_raw = mapping.get("metadata", [])
    metadata: Tuple[SorafsReplicationMetadataEntry, ...] = tuple(
        SorafsReplicationMetadataEntry(
            key=str(entry.get("key", "")),
            value=str(entry.get("value", "")),
        )
        for entry in metadata_raw
        if isinstance(entry, Mapping)
    )

    return SorafsReplicationOrder(
        schema_version=schema_version,
        order_id_hex=order_id_hex,
        manifest_cid_utf8=manifest_cid_utf8,
        manifest_cid_base64=manifest_cid_base64,
        manifest_digest_hex=manifest_digest_hex,
        chunking_profile=chunking_profile,
        target_replicas=target_replicas,
        assignments=assignments,
        issued_at_unix=issued_at_unix,
        deadline_at_unix=deadline_at_unix,
        sla=sla,
        metadata=metadata,
    )


# -- Multi-source orchestrator bindings ---------------------------------------------------------


def _plan_to_json(plan: Union[str, bytes, bytearray, memoryview, Mapping[str, Any], os.PathLike[str]]) -> str:
    if isinstance(plan, str):
        stripped = plan.strip()
        if not stripped:
            raise ValueError("plan must not be empty")
        return stripped
    if isinstance(plan, memoryview):
        return plan.tobytes().decode("utf-8")
    if isinstance(plan, (bytes, bytearray)):
        text = bytes(plan).decode("utf-8")
        stripped = text.strip()
        if not stripped:
            raise ValueError("plan must not be empty")
        return stripped
    if isinstance(plan, Mapping):
        return json.dumps(plan, sort_keys=True, separators=(",", ":"))
    if isinstance(plan, os.PathLike):
        path = Path(plan)
        return path.read_text(encoding="utf-8").strip()
    raise TypeError("plan must be a JSON string, mapping, bytes, or path-like object")


def _normalize_path(path: Union[str, os.PathLike[str]]) -> str:
    if isinstance(path, os.PathLike):
        value = os.fspath(path)
    else:
        value = path
    if not isinstance(value, str) or not value:
        raise TypeError("provider path must be a non-empty string or path-like object")
    return value


def _maybe_tuple(value: Optional[Iterable[str]]) -> Optional[Tuple[str, ...]]:
    if value is None:
        return None
    return tuple(str(entry) for entry in value)


@dataclass(frozen=True)
class SorafsRangeCapability:
    """Range capability advertised by a provider."""

    max_chunk_span: int
    min_granularity: int
    supports_sparse_offsets: bool = True
    requires_alignment: bool = False
    supports_merkle_proof: bool = True

    def to_mapping(self) -> Mapping[str, Any]:
        return {
            "max_chunk_span": self.max_chunk_span,
            "min_granularity": self.min_granularity,
            "supports_sparse_offsets": self.supports_sparse_offsets,
            "requires_alignment": self.requires_alignment,
            "supports_merkle_proof": self.supports_merkle_proof,
        }


@dataclass(frozen=True)
class SorafsStreamBudget:
    """Per-provider concurrency + bandwidth limits."""

    max_in_flight: int
    max_bytes_per_sec: int
    burst_bytes: Optional[int] = None

    def to_mapping(self) -> Mapping[str, Any]:
        payload: Dict[str, Any] = {
            "max_in_flight": self.max_in_flight,
            "max_bytes_per_sec": self.max_bytes_per_sec,
        }
        if self.burst_bytes is not None:
            payload["burst_bytes"] = self.burst_bytes
        return payload


@dataclass(frozen=True)
class SorafsTransportHint:
    """Protocol hint used by guard/proxy selection."""

    protocol: str
    protocol_id: int
    priority: int

    def to_mapping(self) -> Mapping[str, Any]:
        return {
            "protocol": self.protocol,
            "protocol_id": self.protocol_id,
            "priority": self.priority,
        }


@dataclass(frozen=True)
class SorafsProviderMetadata:
    """Advert metadata mirrored from the scoreboard pipeline."""

    provider_id: Optional[str] = None
    profile_id: Optional[str] = None
    profile_aliases: Optional[Tuple[str, ...]] = None
    availability: Optional[str] = None
    stake_amount: Optional[str] = None
    max_streams: Optional[int] = None
    refresh_deadline: Optional[int] = None
    expires_at: Optional[int] = None
    ttl_secs: Optional[int] = None
    allow_unknown_capabilities: Optional[bool] = None
    capability_names: Optional[Tuple[str, ...]] = None
    rendezvous_topics: Optional[Tuple[str, ...]] = None
    notes: Optional[str] = None
    range_capability: Optional[SorafsRangeCapability] = None
    stream_budget: Optional[SorafsStreamBudget] = None
    transport_hints: Optional[Tuple[SorafsTransportHint, ...]] = None

    def to_mapping(self) -> Mapping[str, Any]:
        payload: Dict[str, Any] = {}
        if self.provider_id is not None:
            payload["provider_id"] = self.provider_id
        if self.profile_id is not None:
            payload["profile_id"] = self.profile_id
        if self.profile_aliases is not None:
            payload["profile_aliases"] = list(self.profile_aliases)
        if self.availability is not None:
            payload["availability"] = self.availability
        if self.stake_amount is not None:
            payload["stake_amount"] = self.stake_amount
        if self.max_streams is not None:
            payload["max_streams"] = self.max_streams
        if self.refresh_deadline is not None:
            payload["refresh_deadline"] = self.refresh_deadline
        if self.expires_at is not None:
            payload["expires_at"] = self.expires_at
        if self.ttl_secs is not None:
            payload["ttl_secs"] = self.ttl_secs
        if self.allow_unknown_capabilities is not None:
            payload["allow_unknown_capabilities"] = self.allow_unknown_capabilities
        if self.capability_names is not None:
            payload["capability_names"] = list(self.capability_names)
        if self.rendezvous_topics is not None:
            payload["rendezvous_topics"] = list(self.rendezvous_topics)
        if self.notes is not None:
            payload["notes"] = self.notes
        if self.range_capability is not None:
            payload["range_capability"] = self.range_capability.to_mapping()
        if self.stream_budget is not None:
            payload["stream_budget"] = self.stream_budget.to_mapping()
        if self.transport_hints is not None:
            payload["transport_hints"] = [hint.to_mapping() for hint in self.transport_hints]
        return payload


@dataclass(frozen=True)
class SorafsLocalProviderSpec:
    """Local provider descriptor used by the multi-fetch orchestrator."""

    name: str
    path: Union[str, os.PathLike[str]]
    max_concurrent: Optional[int] = None
    weight: Optional[int] = None
    metadata: Optional[SorafsProviderMetadata] = None

    def to_mapping(self) -> Mapping[str, Any]:
        payload: Dict[str, Any] = {
            "name": self.name,
            "path": _normalize_path(self.path),
        }
        if self.max_concurrent is not None:
            payload["max_concurrent"] = int(self.max_concurrent)
        if self.weight is not None:
            payload["weight"] = int(self.weight)
        if self.metadata is not None:
            payload["metadata"] = self.metadata.to_mapping()
        return payload


@dataclass(frozen=True)
class SorafsTelemetryEntry:
    """Telemetry snapshot consumed by the scoreboard pipeline."""

    provider_id: str
    qos_score: Optional[float] = None
    latency_p95_ms: Optional[float] = None
    failure_rate_ewma: Optional[float] = None
    token_health: Optional[float] = None
    staking_weight: Optional[float] = None
    penalty: Optional[bool] = None
    last_updated_unix: Optional[int] = None

    def to_mapping(self) -> Mapping[str, Any]:
        payload: Dict[str, Any] = {"provider_id": self.provider_id}
        if self.qos_score is not None:
            payload["qos_score"] = float(self.qos_score)
        if self.latency_p95_ms is not None:
            payload["latency_p95_ms"] = float(self.latency_p95_ms)
        if self.failure_rate_ewma is not None:
            payload["failure_rate_ewma"] = float(self.failure_rate_ewma)
        if self.token_health is not None:
            payload["token_health"] = float(self.token_health)
        if self.staking_weight is not None:
            payload["staking_weight"] = float(self.staking_weight)
        if self.penalty is not None:
            payload["penalty"] = bool(self.penalty)
        if self.last_updated_unix is not None:
            payload["last_updated_unix"] = int(self.last_updated_unix)
        return payload


@dataclass(frozen=True)
class SorafsProviderBoost:
    """Manual boost/deny entry for deterministic policy testing."""

    provider: str
    delta: int

    def to_mapping(self) -> Mapping[str, Any]:
        return {
            "provider": self.provider,
            "delta": self.delta,
        }


@dataclass(frozen=True)
class SorafsMultiFetchOptions:
    """Orchestrator knobs surfaced to Python callers."""

    verify_digests: Optional[bool] = None
    verify_lengths: Optional[bool] = None
    retry_budget: Optional[int] = None
    provider_failure_threshold: Optional[int] = None
    max_parallel: Optional[int] = None
    max_peers: Optional[int] = None
    chunker_handle: Optional[str] = None
    telemetry_region: Optional[str] = None
    telemetry: Optional[Tuple[SorafsTelemetryEntry, ...]] = None
    use_scoreboard: Optional[bool] = None
    deny_providers: Optional[Tuple[str, ...]] = None
    boost_providers: Optional[Tuple[SorafsProviderBoost, ...]] = None
    return_scoreboard: Optional[bool] = None
    scoreboard_out_path: Optional[os.PathLike[str] | str] = None
    scoreboard_now_unix_secs: Optional[int] = None
    scoreboard_telemetry_label: Optional[str] = None

    def to_mapping(self) -> Mapping[str, Any]:
        payload: Dict[str, Any] = {}
        if self.verify_digests is not None:
            payload["verify_digests"] = bool(self.verify_digests)
        if self.verify_lengths is not None:
            payload["verify_lengths"] = bool(self.verify_lengths)
        if self.retry_budget is not None:
            payload["retry_budget"] = int(self.retry_budget)
        if self.provider_failure_threshold is not None:
            payload["provider_failure_threshold"] = int(self.provider_failure_threshold)
        if self.max_parallel is not None:
            payload["max_parallel"] = int(self.max_parallel)
        if self.max_peers is not None:
            payload["max_peers"] = int(self.max_peers)
        if self.chunker_handle is not None:
            payload["chunker_handle"] = self.chunker_handle
        if self.telemetry_region is not None:
            region = self.telemetry_region.strip()
            if not region:
                raise ValueError("telemetry_region must not be empty when provided")
            payload["telemetry_region"] = region
        if self.telemetry is not None:
            payload["telemetry"] = [entry.to_mapping() for entry in self.telemetry]
        if self.use_scoreboard is not None:
            payload["use_scoreboard"] = bool(self.use_scoreboard)
        if self.deny_providers is not None:
            payload["deny_providers"] = list(self.deny_providers)
        if self.boost_providers is not None:
            payload["boost_providers"] = [entry.to_mapping() for entry in self.boost_providers]
        if self.return_scoreboard is not None:
            payload["return_scoreboard"] = bool(self.return_scoreboard)
        if self.scoreboard_out_path is not None:
            payload["scoreboard_out_path"] = os.fspath(self.scoreboard_out_path)
        if self.scoreboard_now_unix_secs is not None:
            payload["scoreboard_now_unix_secs"] = int(self.scoreboard_now_unix_secs)
        if self.scoreboard_telemetry_label is not None:
            if self.scoreboard_out_path is None:
                raise ValueError(
                    "scoreboard_telemetry_label requires scoreboard_out_path to be set"
                )
            label = self.scoreboard_telemetry_label.strip()
            if not label:
                raise ValueError("scoreboard_telemetry_label must not be empty when provided")
            payload["scoreboard_telemetry_label"] = label
        return payload


@dataclass(frozen=True)
class SorafsProviderReport:
    """Outcome summary for a provider session."""

    provider: str
    successes: int
    failures: int
    disabled: bool

    @classmethod
    def from_mapping(cls, mapping: Mapping[str, Any]) -> "SorafsProviderReport":
        return cls(
            provider=str(mapping.get("provider")),
            successes=int(mapping.get("successes", 0)),
            failures=int(mapping.get("failures", 0)),
            disabled=bool(mapping.get("disabled", False)),
        )


@dataclass(frozen=True)
class SorafsChunkReceipt:
    """Detailed chunk receipt emitted by the orchestrator."""

    chunk_index: int
    provider: str
    attempts: int
    latency_ms: int
    bytes: int

    @classmethod
    def from_mapping(cls, mapping: Mapping[str, Any]) -> "SorafsChunkReceipt":
        return cls(
            chunk_index=int(mapping.get("chunk_index", 0)),
            provider=str(mapping.get("provider")),
            attempts=int(mapping.get("attempts", 0)),
            latency_ms=int(mapping.get("latency_ms", 0)),
            bytes=int(mapping.get("bytes", 0)),
        )


@dataclass(frozen=True)
class SorafsScoreboardRow:
    """Row from the deterministic scoreboard export."""

    provider_id: str
    alias: str
    raw_score: float
    normalized_weight: float
    eligibility: str

    @classmethod
    def from_mapping(cls, mapping: Mapping[str, Any]) -> "SorafsScoreboardRow":
        return cls(
            provider_id=str(mapping.get("provider_id")),
            alias=str(mapping.get("alias")),
            raw_score=float(mapping.get("raw_score", 0.0)),
            normalized_weight=float(mapping.get("normalized_weight", 0.0)),
            eligibility=str(mapping.get("eligibility", "")),
        )


@dataclass(frozen=True)
class SorafsMultiFetchResult:
    """Structured result returned by :func:`multi_fetch_local`."""

    chunk_count: int
    payload: bytes
    provider_reports: Tuple[SorafsProviderReport, ...]
    chunk_receipts: Tuple[SorafsChunkReceipt, ...]
    scoreboard: Optional[Tuple[SorafsScoreboardRow, ...]]

    @classmethod
    def from_mapping(cls, mapping: Mapping[str, Any]) -> "SorafsMultiFetchResult":
        chunk_count = int(mapping.get("chunk_count", 0))
        payload = mapping.get("payload")
        if not isinstance(payload, (bytes, bytearray, memoryview)):
            raise TypeError("multi-fetch payload must be bytes-like")
        provider_reports_raw = mapping.get("provider_reports", [])
        receipts_raw = mapping.get("chunk_receipts", [])
        scoreboard_raw = mapping.get("scoreboard")
        provider_reports = tuple(
            SorafsProviderReport.from_mapping(entry)
            for entry in provider_reports_raw
            if isinstance(entry, Mapping)
        )
        receipts = tuple(
            SorafsChunkReceipt.from_mapping(entry)
            for entry in receipts_raw
            if isinstance(entry, Mapping)
        )
        scoreboard = None
        if isinstance(scoreboard_raw, Sequence):
            scoreboard = tuple(
                SorafsScoreboardRow.from_mapping(entry)
                for entry in scoreboard_raw
                if isinstance(entry, Mapping)
            )
        return cls(
            chunk_count=chunk_count,
            payload=bytes(payload),
            provider_reports=provider_reports,
            chunk_receipts=receipts,
            scoreboard=scoreboard,
        )


def multi_fetch_local(
    plan: Union[str, bytes, bytearray, memoryview, Mapping[str, Any], os.PathLike[str]],
    providers: Sequence[Union[SorafsLocalProviderSpec, Mapping[str, Any]]],
    *,
    options: Optional[SorafsMultiFetchOptions] = None,
) -> SorafsMultiFetchResult:
    """Execute the deterministic multi-source orchestrator against local payloads.

    Parameters
    ----------
    plan:
        JSON string (or mapping/path) describing the chunk fetch plan emitted by
        `sorafs_manifest_stub`.
    providers:
        Iterable of :class:`SorafsLocalProviderSpec` definitions. Each entry points at a local
        payload file (tests reuse the shared fixture bundle).
    options:
        Optional :class:`SorafsMultiFetchOptions` controlling scoreboard/verification behaviour.
    """

    if not providers:
        raise ValueError("providers must contain at least one entry")
    plan_json = _plan_to_json(plan)
    provider_payloads = []
    for entry in providers:
        if isinstance(entry, SorafsLocalProviderSpec):
            provider_payloads.append(entry.to_mapping())
        elif isinstance(entry, Mapping):
            provider_payloads.append(dict(entry))
        else:
            raise TypeError("providers must be SorafsLocalProviderSpec or mapping entries")
    options_payload = options.to_mapping() if options else None
    result = _crypto.sorafs_multi_fetch_local(plan_json, provider_payloads, options=options_payload)
    if not isinstance(result, Mapping):
        raise TypeError("multi-fetch returned an unexpected payload")
    return SorafsMultiFetchResult.from_mapping(result)
