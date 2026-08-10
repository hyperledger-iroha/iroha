"""Connect, Kaigi, and Sumeragi status models exposed by the Torii client."""

from __future__ import annotations

import json
from dataclasses import dataclass
from decimal import Decimal
from typing import Any, Dict, List, Literal, Optional, Sequence, Tuple, Union


@dataclass(frozen=True)
class ConnectPerIpSessions:
    """Per-IP session counter inside a Connect status snapshot."""

    ip: str
    sessions: int


@dataclass(frozen=True)
class ConnectStatusPolicy:
    """Policy knobs currently enforced by the Connect service."""

    relay_enabled: Optional[bool]
    ws_max_sessions: Optional[int]
    ws_per_ip_max_sessions: Optional[int]
    ws_rate_per_ip_per_min: Optional[int]
    session_ttl_ms: Optional[int]
    frame_max_bytes: Optional[int]
    session_buffer_max_bytes: Optional[int]
    relay_strategy: Optional[str]
    relay_effective_strategy: Optional[str]
    relay_p2p_attached: Optional[bool]
    p2p_ttl_hops: Optional[int]
    heartbeat_interval_ms: Optional[int]
    heartbeat_miss_tolerance: Optional[int]
    heartbeat_min_interval_ms: Optional[int]
    extra: Dict[str, Any]


@dataclass(frozen=True)
class ConnectStatusSnapshot:
    """Aggregate Connect status metrics."""

    enabled: bool
    sessions_total: int
    sessions_active: int
    per_ip_sessions: List[ConnectPerIpSessions]
    buffered_sessions: int
    total_buffer_bytes: int
    dedupe_size: int
    frames_in_total: int
    frames_out_total: int
    ciphertext_total: int
    dedupe_drops_total: int
    buffer_drops_total: int
    plaintext_control_drops_total: int
    monotonic_drops_total: int
    sequence_violation_closes_total: int
    role_direction_mismatch_total: int
    ping_miss_total: int
    p2p_rebroadcasts_total: int
    p2p_rebroadcast_skipped_total: int
    p2p_auth_failures_total: int
    p2p_ttl_drops_total: int
    p2p_unknown_session_drops_total: int
    p2p_session_claims_in_total: int
    p2p_session_claims_installed_total: int
    p2p_session_claim_conflicts_total: int
    p2p_role_consumed_total: int
    p2p_session_terminated_total: int
    policy: Optional[ConnectStatusPolicy]


@dataclass(frozen=True)
class ConnectSessionInfo:
    """Session tokens returned by ``POST /v1/connect/session``."""

    sid: str
    wallet_uri: str
    app_uri: str
    token_app: str
    token_wallet: str
    token_management: str
    token_relay: str
    extra: Dict[str, Any]


@dataclass(frozen=True)
class ConnectAppRecord:
    """Registered Connect application descriptor."""

    app_id: str
    display_name: Optional[str]
    description: Optional[str]
    icon_url: Optional[str]
    namespaces: List[str]
    metadata: Dict[str, Any]
    policy: Dict[str, Any]
    extra: Dict[str, Any]


@dataclass(frozen=True)
class ConnectAppRegistryPage:
    """Paginated Connect app registry response."""

    items: List[ConnectAppRecord]
    total: Optional[int]
    next_cursor: Optional[str]
    extra: Dict[str, Any]


@dataclass(frozen=True)
class ConnectAppPolicyControls:
    """Mutable Connect app policy toggles."""

    relay_enabled: Optional[bool]
    ws_max_sessions: Optional[int]
    ws_per_ip_max_sessions: Optional[int]
    ws_rate_per_ip_per_min: Optional[int]
    session_ttl_ms: Optional[int]
    frame_max_bytes: Optional[int]
    session_buffer_max_bytes: Optional[int]
    heartbeat_interval_ms: Optional[int]
    heartbeat_miss_tolerance: Optional[int]
    heartbeat_min_interval_ms: Optional[int]
    extra: Dict[str, Any]


@dataclass(frozen=True)
class ConnectAdmissionManifestEntry:
    """Admission manifest entry describing permitted Connect apps."""

    app_id: str
    namespaces: List[str]
    metadata: Dict[str, Any]
    policy: Dict[str, Any]
    extra: Dict[str, Any]


@dataclass(frozen=True)
class ConnectAdmissionManifest:
    """Admission manifest contents returned by Connect governance endpoints."""

    version: Optional[int]
    manifest_hash: Optional[str]
    updated_at: Optional[str]
    entries: List[ConnectAdmissionManifestEntry]
    extra: Dict[str, Any]


SUMERAGI_EVIDENCE_KIND_FILTERS = {
    "DoublePrepare",
    "DoubleCommit",
    "InvalidQc",
    "InvalidProposal",
    "Censorship",
    "SumeragiV2Equivocation",
}
SUMERAGI_EVIDENCE_PHASES = {"Prepare", "Commit", "NewView"}
SUMERAGI_EVIDENCE_EQUIVOCATION_CLASSES = {
    "proposal",
    "phase_vote",
    "timeout_vote",
}


@dataclass(frozen=True)
class SumeragiEvidenceRecordBase:
    """Fields common to exact evidence records returned by Torii."""

    kind: str
    recorded_height: int
    recorded_view: int
    recorded_ms: int
    consensus_admitted_height: Optional[int]


@dataclass(frozen=True)
class SumeragiDoubleVoteEvidenceRecord(SumeragiEvidenceRecordBase):
    """Exact ``DoublePrepare`` or ``DoubleCommit`` evidence projection."""

    kind: Literal["DoublePrepare", "DoubleCommit"]
    phase: Literal["Prepare", "Commit", "NewView"]
    height: int
    view: int
    epoch: int
    signer: int
    block_hash_1: str
    block_hash_2: str


@dataclass(frozen=True)
class SumeragiInvalidQcEvidenceRecord(SumeragiEvidenceRecordBase):
    """Exact ``InvalidQc`` evidence projection."""

    kind: Literal["InvalidQc"]
    height: int
    view: int
    epoch: int
    subject_block_hash: str
    phase: Literal["Prepare", "Commit", "NewView"]
    reason: str


@dataclass(frozen=True)
class SumeragiInvalidProposalEvidenceRecord(SumeragiEvidenceRecordBase):
    """Exact ``InvalidProposal`` evidence projection."""

    kind: Literal["InvalidProposal"]
    height: int
    view: int
    epoch: int
    subject_block_hash: str
    payload_hash: str
    reason: str


@dataclass(frozen=True)
class SumeragiCensorshipEvidenceRecord(SumeragiEvidenceRecordBase):
    """Exact ``Censorship`` evidence projection."""

    kind: Literal["Censorship"]
    tx_hash: str
    receipt_count: int
    signers: List[str]
    submitted_at_height_min: Optional[int]
    submitted_at_height_max: Optional[int]


@dataclass(frozen=True)
class SumeragiV2EquivocationEvidenceRecord(SumeragiEvidenceRecordBase):
    """Exact ``SumeragiV2Equivocation`` evidence projection."""

    kind: Literal["SumeragiV2Equivocation"]
    class_: Literal["proposal", "phase_vote", "timeout_vote"]
    height: int
    view: int
    epoch: int
    signer: int
    context_id: str
    artifact_hash_1: str
    artifact_hash_2: str


SumeragiEvidenceRecord = Union[
    SumeragiDoubleVoteEvidenceRecord,
    SumeragiInvalidQcEvidenceRecord,
    SumeragiInvalidProposalEvidenceRecord,
    SumeragiCensorshipEvidenceRecord,
    SumeragiV2EquivocationEvidenceRecord,
]


@dataclass(frozen=True)
class SumeragiEvidenceListPage:
    """Paginated evidence listing."""

    items: List[SumeragiEvidenceRecord]
    total: int


_KAIGI_HEALTH_STATUSES = {"healthy", "degraded", "unavailable"}


@dataclass(frozen=True)
class KaigiRelaySummary:
    """Summary entry returned by ``GET /v1/kaigi/relays``."""

    relay_id: str
    domain: str
    bandwidth_class: int
    hpke_fingerprint_hex: str
    status: Optional[str]
    reported_at_ms: Optional[int]


@dataclass(frozen=True)
class KaigiRelaySummaryList:
    """Response envelope for ``GET /v1/kaigi/relays``."""

    total: int
    items: List[KaigiRelaySummary]


@dataclass(frozen=True)
class KaigiRelayReportedCall:
    """Call metadata reported alongside Kaigi relay health snapshots."""

    domain_id: str
    call_name: str


@dataclass(frozen=True)
class KaigiRelayDomainMetrics:
    """Per-domain metrics emitted by Kaigi relay endpoints."""

    domain: str
    registrations_total: int
    manifest_updates_total: int
    failovers_total: int
    health_reports_total: int


@dataclass(frozen=True)
class KaigiRelayDetail:
    """Detailed relay metadata returned by ``GET /v1/kaigi/relays/{relay_id}``."""

    relay: KaigiRelaySummary
    hpke_public_key_b64: str
    reported_call: Optional[KaigiRelayReportedCall]
    reported_by: Optional[str]
    notes: Optional[str]
    metrics: Optional[KaigiRelayDomainMetrics]


@dataclass(frozen=True)
class KaigiRelayHealthSnapshot:
    """Aggregated relay health counters returned by ``GET /v1/kaigi/relays/health``."""

    healthy_total: int
    degraded_total: int
    unavailable_total: int
    reports_total: int
    registrations_total: int
    failovers_total: int
    domains: List[KaigiRelayDomainMetrics]


@dataclass(frozen=True)
class SumeragiQcEntry:
    """`HighestQC`/`LockedQC` entry returned by ``GET /v1/sumeragi/qc``."""

    height: int
    view: int
    subject_block_hash: Optional[str]


@dataclass(frozen=True)
class SumeragiQcSnapshot:
    """QC snapshot returned by ``GET /v1/sumeragi/qc``."""

    highest_qc: SumeragiQcEntry
    locked_qc: SumeragiQcEntry


@dataclass(frozen=True)
class SumeragiPacemakerSnapshot:
    """Pacemaker metrics returned by ``GET /v1/sumeragi/pacemaker``."""

    backoff_ms: int
    rtt_floor_ms: int
    jitter_ms: int
    backoff_multiplier: int
    rtt_floor_multiplier: int
    max_backoff_ms: int
    jitter_frac_permille: int
    round_elapsed_ms: int
    view_timeout_target_ms: int
    view_timeout_remaining_ms: int


@dataclass(frozen=True)
class SumeragiPhasesEma:
    """Smoothed latency metrics returned alongside ``/v1/sumeragi/phases``."""

    propose_ms: int
    collect_da_ms: int
    collect_prevote_ms: int
    collect_precommit_ms: int
    collect_aggregator_ms: int
    commit_ms: int
    pipeline_total_ms: int


@dataclass(frozen=True)
class SumeragiPhasesSnapshot:
    """Latest latency counters returned by ``GET /v1/sumeragi/phases``."""

    propose_ms: int
    collect_da_ms: int
    collect_prevote_ms: int
    collect_precommit_ms: int
    collect_aggregator_ms: int
    commit_ms: int
    pipeline_total_ms: int
    collect_aggregator_gossip_total: int
    block_created_dropped_by_lock_total: int
    block_created_hint_mismatch_total: int
    block_created_proposal_mismatch_total: int
    ema_ms: SumeragiPhasesEma


@dataclass(frozen=True)
class SumeragiPrfContext:
    """PRF state returned by Sumeragi inspection endpoints."""

    height: int
    view: int
    epoch_seed: Optional[str]


@dataclass(frozen=True)
class SumeragiLeaderSnapshot:
    """Leader metadata returned by ``GET /v1/sumeragi/leader``."""

    leader_index: int
    prf: SumeragiPrfContext


@dataclass(frozen=True)
class SumeragiParamsSnapshot:
    """Consensus parameter snapshot returned by ``GET /v1/sumeragi/params``."""

    block_time_ms: int
    commit_time_ms: int
    max_clock_drift_ms: int
    collectors_k: int
    redundant_send_r: int
    da_enabled: bool
    next_mode: Optional[str]
    mode_activation_height: Optional[int]
    chain_height: int


def parse_sumeragi_json_object(payload: bytes, label: str) -> Dict[str, Any]:
    """Decode strict UTF-8 JSON while retaining typed-parser numeric diagnostics."""

    try:
        text = payload.decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise ValueError(f"{label} must be UTF-8 JSON") from error

    def unique_object(pairs: Sequence[Tuple[str, Any]]) -> Dict[str, Any]:
        result: Dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"{label} contains duplicate field `{key}`")
            result[key] = value
        return result

    def reject_constant(token: str) -> Any:
        raise ValueError(f"{label} contains a non-finite numeric value `{token}`")

    try:
        value = json.loads(
            text,
            object_pairs_hook=unique_object,
            parse_float=Decimal,
            parse_constant=reject_constant,
        )
    except (json.JSONDecodeError, RecursionError) as error:
        raise ValueError(f"{label} must be valid JSON") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be a JSON object")
    return value

# Keep the public model identity stable even though definitions live in this
# private support module.
for _model in tuple(globals().values()):
    if isinstance(_model, type) and _model.__module__ == __name__:
        _model.__module__ = f"{__package__}.client"
del _model
