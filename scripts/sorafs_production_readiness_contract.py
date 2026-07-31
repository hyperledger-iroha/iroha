"""Declarative contracts for the SoraFS production-readiness aggregate gate.

This module has no runtime inputs. It imports the per-lane checker contracts and
exposes the schema-closed aggregate metadata consumed by the CLI checker.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Any

from check_sorafs_ai_prescreen_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as AI_PRESCREEN_REQUIRED_KINDS,
    KIND_BY_NAME as AI_PRESCREEN_KIND_BY_NAME,
    POLICY_BOUND_KINDS as AI_PRESCREEN_POLICY_BOUND_KINDS,
    RUNNER_BOUND_KINDS as AI_PRESCREEN_RUNNER_BOUND_KINDS,
    WORKFLOW_BOUND_KINDS as AI_PRESCREEN_WORKFLOW_BOUND_KINDS,
)
from check_sorafs_appeal_finance_rollout_evidence import (  # noqa: E402
    CONFIG_BOUND_KINDS as APPEAL_FINANCE_CONFIG_BOUND_KINDS,
    DEFAULT_REQUIRED_KINDS as APPEAL_FINANCE_REQUIRED_KINDS,
    KIND_BY_NAME as APPEAL_FINANCE_KIND_BY_NAME,
    POLICY_BOUND_KINDS as APPEAL_FINANCE_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as APPEAL_FINANCE_REQUIRED_METRICS,
)
from check_sorafs_gateway_compliance_rollout_evidence import (  # noqa: E402
    CATALOG_BOUND_KINDS as GATEWAY_COMPLIANCE_CATALOG_BOUND_KINDS,
    DEFAULT_REQUIRED_KINDS as GATEWAY_COMPLIANCE_REQUIRED_KINDS,
    KIND_BY_NAME as GATEWAY_COMPLIANCE_KIND_BY_NAME,
    POLICY_BOUND_KINDS as GATEWAY_COMPLIANCE_POLICY_BOUND_KINDS,
    PREDECESSOR_BOUND_KINDS as GATEWAY_COMPLIANCE_PREDECESSOR_BOUND_KINDS,
    REQUIRED_METRICS as GATEWAY_COMPLIANCE_REQUIRED_METRICS,
)
from check_sorafs_gateway_load_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_ERROR_RATE_BPS as GATEWAY_LOAD_MAX_ERROR_RATE_BPS,
    DEFAULT_MAX_P95_LATENCY_MS as GATEWAY_LOAD_MAX_P95_LATENCY_MS,
    DEFAULT_MAX_P99_LATENCY_MS as GATEWAY_LOAD_MAX_P99_LATENCY_MS,
    DEFAULT_MIN_PROVIDER_COUNT as GATEWAY_LOAD_MIN_PROVIDER_COUNT,
    DEFAULT_MIN_STAGING_DURATION_SECS as GATEWAY_LOAD_MIN_STAGING_DURATION_SECS,
    DEFAULT_MIN_STREAMS as GATEWAY_LOAD_MIN_STREAMS,
    DEFAULT_MIN_SUCCESS_RATE_BPS as GATEWAY_LOAD_MIN_SUCCESS_RATE_BPS,
    DEFAULT_REQUIRED_KINDS as GATEWAY_LOAD_REQUIRED_KINDS,
    FORBIDDEN_STAGING_METADATA_MARKERS as GATEWAY_LOAD_FORBIDDEN_METADATA_MARKERS,
    GATEWAY_VERSION_PATTERN as GATEWAY_LOAD_VERSION_PATTERN,
    HARDWARE_PROFILE_PATTERN as GATEWAY_LOAD_HARDWARE_PROFILE_PATTERN,
    KIND_BY_NAME as GATEWAY_LOAD_KIND_BY_NAME,
    MAX_SUCCESS_RATE_BPS as GATEWAY_LOAD_MAX_SUCCESS_RATE_BPS,
    POLICY_BOUND_KINDS as GATEWAY_LOAD_POLICY_BOUND_KINDS,
    PROVIDER_NAME_PATTERN as GATEWAY_LOAD_PROVIDER_NAME_PATTERN,
    REQUIRED_CACHE_COVERAGE_FIELDS as GATEWAY_LOAD_REQUIRED_CACHE_COVERAGE_FIELDS,
    REQUIRED_CORRUPTION_INJECTION_BPS as GATEWAY_LOAD_REQUIRED_CORRUPTION_BPS,
    REQUIRED_LOAD_CONDITION_FIELDS as GATEWAY_LOAD_REQUIRED_LOAD_CONDITION_FIELDS,
    REQUIRED_METRICS as GATEWAY_LOAD_REQUIRED_METRICS,
    STAGING_REPORT_BOUND_KINDS as GATEWAY_LOAD_STAGING_REPORT_BOUND_KINDS,
    STREAM_NAME_PATTERN as GATEWAY_LOAD_STREAM_NAME_PATTERN,
)
from check_sorafs_governance_dag_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as GOVERNANCE_DAG_REQUIRED_KINDS,
    KIND_BY_NAME as GOVERNANCE_DAG_KIND_BY_NAME,
    POLICY_BOUND_KINDS as GOVERNANCE_DAG_POLICY_BOUND_KINDS,
    PUBLIC_HEAD_BOUND_KINDS as GOVERNANCE_DAG_PUBLIC_HEAD_BOUND_KINDS,
    REQUIRED_METRICS as GOVERNANCE_DAG_REQUIRED_METRICS,
)
from check_sorafs_hedging_rollout_evidence import (  # noqa: E402
    CYCLE_ID_PATTERN as HEDGING_BILLING_CYCLE_ID_PATTERN,
    CYCLE_BOUND_KINDS as HEDGING_BILLING_CYCLE_BOUND_KINDS,
    DEFAULT_REQUIRED_KINDS as HEDGING_BILLING_REQUIRED_KINDS,
    FORBIDDEN_CYCLE_ID_MARKERS as HEDGING_BILLING_FORBIDDEN_CYCLE_ID_MARKERS,
    KIND_BY_NAME as HEDGING_BILLING_KIND_BY_NAME,
    POLICY_BOUND_KINDS as HEDGING_BILLING_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as HEDGING_BILLING_REQUIRED_METRICS,
)
from check_sorafs_moderation_panel_rollout_evidence import (  # noqa: E402
    CASE_BOUND_KINDS as MODERATION_PANEL_CASE_BOUND_KINDS,
    DEFAULT_REQUIRED_KINDS as MODERATION_PANEL_REQUIRED_KINDS,
    KIND_BY_NAME as MODERATION_PANEL_KIND_BY_NAME,
    POLICY_BOUND_KINDS as MODERATION_PANEL_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as MODERATION_PANEL_REQUIRED_METRICS,
    ROSTER_BOUND_KINDS as MODERATION_PANEL_ROSTER_BOUND_KINDS,
    TALLY_BOUND_KINDS as MODERATION_PANEL_TALLY_BOUND_KINDS,
)
from check_sorafs_orderbook_rollout_evidence import (  # noqa: E402
    CONTRACT_BOUND_KINDS as ORDERBOOK_CONTRACT_BOUND_KINDS,
    DEFAULT_REQUIRED_KINDS as ORDERBOOK_REQUIRED_KINDS,
    KIND_BY_NAME as ORDERBOOK_KIND_BY_NAME,
    POLICY_BOUND_KINDS as ORDERBOOK_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as ORDERBOOK_REQUIRED_METRICS,
)
from check_sorafs_pdp_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as PDP_REQUIRED_KINDS,
    KIND_BY_NAME as PDP_KIND_BY_NAME,
    POLICY_BOUND_KINDS as PDP_POLICY_BOUND_KINDS,
    PROOF_SUMMARY_BOUND_KINDS as PDP_PROOF_SUMMARY_BOUND_KINDS,
    PROVIDER_ROSTER_BOUND_KINDS as PDP_PROVIDER_ROSTER_BOUND_KINDS,
    REQUIRED_METRICS as PDP_REQUIRED_METRICS,
)
from check_sorafs_pop_credentials_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as POP_CREDENTIALS_REQUIRED_KINDS,
    KIND_BY_NAME as POP_CREDENTIALS_KIND_BY_NAME,
    POLICY_BOUND_KINDS as POP_CREDENTIALS_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as POP_CREDENTIALS_REQUIRED_METRICS,
    REVOCATION_BOUND_KINDS as POP_CREDENTIALS_REVOCATION_BOUND_KINDS,
    ROOT_BOUND_KINDS as POP_CREDENTIALS_ROOT_BOUND_KINDS,
)
from check_sorafs_por_rollout_evidence import (  # noqa: E402
    ALLOWED_ARCHIVE_BACKENDS as POR_ALLOWED_ARCHIVE_BACKENDS,
    DEFAULT_REQUIRED_KINDS as POR_REQUIRED_KINDS,
    KIND_BY_NAME as POR_KIND_BY_NAME,
    POLICY_BOUND_KINDS as POR_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as POR_REQUIRED_METRICS,
    SEED_REPLAY_BOUND_KINDS as POR_SEED_REPLAY_BOUND_KINDS,
)
from check_sorafs_potr_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as POTR_REQUIRED_KINDS,
    KIND_BY_NAME as POTR_KIND_BY_NAME,
    PQ_KEY_ROSTER_BOUND_KINDS as POTR_PQ_KEY_ROSTER_BOUND_KINDS,
    RECEIPT_SUMMARY_BOUND_KINDS as POTR_RECEIPT_SUMMARY_BOUND_KINDS,
    REPUTATION_WEIGHT_BOUND_KINDS as POTR_REPUTATION_WEIGHT_BOUND_KINDS,
    REQUIRED_METRICS as POTR_REQUIRED_METRICS,
)
from check_sorafs_reference_sdk_release_evidence import (  # noqa: E402
    ALLOWED_MANIFEST_SIGNATURE_ALGORITHMS as REFERENCE_SDK_ALLOWED_SIGNATURE_ALGORITHMS,
    DEFAULT_REQUIRED_KINDS as REFERENCE_SDK_REQUIRED_KINDS,
    KIND_BY_NAME as REFERENCE_SDK_KIND_BY_NAME,
    POLICY_BOUND_KINDS as REFERENCE_SDK_POLICY_BOUND_KINDS,
    RELEASE_MANIFEST_BOUND_KINDS as REFERENCE_SDK_RELEASE_MANIFEST_BOUND_KINDS,
    SOURCE_ARTIFACT_KINDS as REFERENCE_SDK_SUPPLY_CHAIN_SOURCE_ARTIFACT_KINDS,
)
from check_sorafs_repair_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as REPAIR_REQUIRED_KINDS,
    FAILURE_BOUND_KINDS as REPAIR_FAILURE_BOUND_KINDS,
    HANDOFF_BOUND_KINDS as REPAIR_HANDOFF_BOUND_KINDS,
    KIND_BY_NAME as REPAIR_KIND_BY_NAME,
    POLICY_BOUND_KINDS as REPAIR_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as REPAIR_REQUIRED_METRICS,
    ROSTER_BOUND_KINDS as REPAIR_ROSTER_BOUND_KINDS,
)
from check_sorafs_reputation_rollout_evidence import (  # noqa: E402
    DEFAULT_REQUIRED_KINDS as REPUTATION_REQUIRED_KINDS,
    FORBIDDEN_PROVIDER_ID_MARKERS as REPUTATION_FORBIDDEN_PROVIDER_ID_MARKERS,
    KIND_BY_NAME as REPUTATION_KIND_BY_NAME,
    PROVIDER_ID_PATTERN as REPUTATION_PROVIDER_ID_PATTERN,
    REQUIRED_METRICS as REPUTATION_REQUIRED_METRICS,
    SNAPSHOT_ANCHOR_KINDS as REPUTATION_SNAPSHOT_ANCHOR_KINDS,
    SNAPSHOT_BOUND_KINDS as REPUTATION_SNAPSHOT_BOUND_KINDS,
)
from check_sorafs_reserve_rent_rollout_evidence import (  # noqa: E402
    BAKE_ID_PATTERN as RESERVE_RENT_BAKE_ID_PATTERN,
    DEFAULT_REQUIRED_KINDS as RESERVE_RENT_REQUIRED_KINDS,
    FORBIDDEN_BAKE_ID_MARKERS as RESERVE_RENT_FORBIDDEN_BAKE_ID_MARKERS,
    KIND_BY_NAME as RESERVE_RENT_KIND_BY_NAME,
    LEDGER_BOUND_KINDS as RESERVE_RENT_LEDGER_BOUND_KINDS,
    MATRIX_BOUND_KINDS as RESERVE_RENT_MATRIX_BOUND_KINDS,
    POLICY_BOUND_KINDS as RESERVE_RENT_POLICY_BOUND_KINDS,
    REQUIRED_METRICS as RESERVE_RENT_REQUIRED_METRICS,
)
from check_sorafs_transparency_rollout_evidence import (  # noqa: E402
    CYCLE_BOUND_KINDS as TRANSPARENCY_CYCLE_BOUND_KINDS,
    DEFAULT_REQUIRED_KINDS as TRANSPARENCY_REQUIRED_KINDS,
    KIND_BY_NAME as TRANSPARENCY_KIND_BY_NAME,
    SOURCE_BOUND_KINDS as TRANSPARENCY_SOURCE_BOUND_KINDS,
)
from sorafs_evidence_sensitivity import HIGH_RISK_SENSITIVE_KEY_FRAGMENTS
from sorafs_evidence_validation import evidence_schema_by_kind


SUMMARY_SCHEMA = "sorafs.production_readiness.aggregate_gate.v1"
GATEWAY_MODERATION_CATALOG_MISMATCH_ERROR = (
    "moderation_panel evidence viewer catalog digests must match "
    "gateway_compliance valid_catalog_digests"
)
FOUNDATIONAL_PREREQUISITE_SCHEMA = (
    "sorafs.production_readiness.foundational_prerequisites.v1"
)
FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN = (
    b"iroha:sorafs:production-readiness:foundational-prerequisites:v1\x00"
)
RESILIENCE_QUALIFICATION_RECEIPT_SCHEMA = (
    "sorafs.l1.resilience_qualification.v1"
)
RESILIENCE_QUALIFICATION_SUMMARY_SCHEMA = (
    "sorafs.l1.resilience_qualification.summary.v1"
)
RESILIENCE_QUALIFICATION_SIGNATURE_DOMAIN = (
    b"iroha:sorafs:l1-resilience-qualification:v1\x00"
)
RESILIENCE_QUALIFICATION_REQUIREMENTS = (
    "network_partition_recovery",
    "consensus_view_change",
    "validator_restart",
    "torii_restart",
    "provider_restart",
    "simultaneous_peer_submission",
    "signer_rotation",
    "root_rotation",
    "catalog_rotation",
    "gateway_failover",
    "governance_dag_failover",
    "stale_fork_rejection",
    "crash_recovery",
    "identical_post_recovery_peer_state",
    "repair_outcome",
    "settlement_outcome",
    "backup_restore",
    "release_rollback",
    "package_yank",
)
RESILIENCE_QUALIFICATION_SUMMARY_FIELDS = frozenset(
    {
        "schema",
        "status",
        "qualification_scope",
        "live_evidence_recognized",
        "externally_authenticated",
        "promotion_eligible",
        "readiness_lane_count_delta",
        "receipt_sha256",
        "canonical_receipt_sha256",
        "receipt_generated_at_unix",
        "receipt_authentication",
        "deployment",
        "topology_qualification",
        "required_requirements",
        "recognized_requirement_count",
        "artifact_bindings",
        "earliest_capture_unix",
        "latest_capture_unix",
        "errors",
    }
)
RESILIENCE_QUALIFICATION_ARTIFACT_BINDING_FIELDS = frozenset(
    {"requirement", "artifact_path", "artifact_sha256", "captured_at_unix"}
)
RESILIENCE_QUALIFICATION_AUTHENTICATION_FIELDS = frozenset(
    {
        "kind",
        "algorithm",
        "public_key_fingerprint_sha256",
        "signature_hex",
    }
)
RESILIENCE_QUALIFICATION_BINDING_SCHEMA = (
    "sorafs.production_readiness.resilience_qualification_binding.v1"
)
RESILIENCE_QUALIFICATION_BINDING_FIELDS = frozenset(
    {
        "schema",
        "summary_sha256",
        "receipt_sha256",
        "canonical_receipt_sha256",
        "receipt_generated_at_unix",
        "signer_public_key_fingerprint_sha256",
    }
)
AGGREGATE_RESILIENCE_QUALIFICATION_SCHEMA = (
    "sorafs.production_readiness.aggregate_resilience_qualification.v1"
)
AGGREGATE_RESILIENCE_QUALIFICATION_FIELDS = frozenset(
    {"schema", "present", "valid", "binding", "errors"}
)
FOUNDATIONAL_PREREQUISITE_IDS = (
    "SFM-1",
    "SF-1",
    "SF-2",
    "SF-2c",
    "SF-3",
    "SF-4",
    "SF-5b",
    "SF-6",
    "SF-8a",
)
FOUNDATIONAL_PREREQUISITE_LANES: dict[str, tuple[str, ...]] = {
    "SFM-1": ("reputation",),
    "SF-1": ("reference_sdk_release",),
    "SF-2": ("pdp",),
    "SF-2c": ("por", "potr"),
    "SF-3": ("gateway_compliance",),
    "SF-4": ("repair",),
    "SF-5b": ("gateway_load",),
    "SF-6": (
        "appeal_finance",
        "governance_dag",
        "hedging_billing",
        "orderbook",
        "reserve_rent",
    ),
    "SF-8a": (
        "ai_prescreen",
        "moderation_panel",
        "pop_credentials",
        "transparency",
    ),
}
if tuple(FOUNDATIONAL_PREREQUISITE_LANES) != FOUNDATIONAL_PREREQUISITE_IDS:
    raise RuntimeError("foundational prerequisite lane map must follow the canonical IDs")
_FOUNDATIONAL_MAPPED_LANES = tuple(
    gate
    for prerequisite_id in FOUNDATIONAL_PREREQUISITE_IDS
    for gate in FOUNDATIONAL_PREREQUISITE_LANES[prerequisite_id]
)
if (
    any(not lanes for lanes in FOUNDATIONAL_PREREQUISITE_LANES.values())
    or len(set(_FOUNDATIONAL_MAPPED_LANES)) != len(_FOUNDATIONAL_MAPPED_LANES)
):
    raise RuntimeError(
        "foundational prerequisite lane map must use non-empty disjoint lane groups"
    )
MAX_FOUNDATIONAL_RELEASE_SEQUENCE = (1 << 63) - 1
FOUNDATIONAL_PREREQUISITE_FIELDS = frozenset(
    {
        "schema",
        "status",
        "deployment",
        "generated_at_unix",
        "release_sequence",
        "previous_envelope_sha256",
        "topology_qualification",
        "resilience_qualification",
        "prerequisites",
        "lane_summaries",
        "signature",
    }
)
FOUNDATIONAL_PREREQUISITE_DEPLOYMENT_FIELDS = frozenset(
    {"deployment_id", "environment"}
)
FOUNDATIONAL_PREREQUISITE_ROW_FIELDS = frozenset(
    {
        "id",
        "status",
        "evidence_anchor_sha256",
        "evidence_generated_at_unix",
        "readiness_summary_sha256",
    }
)
FOUNDATIONAL_LANE_SUMMARY_ROW_FIELDS = frozenset({"gate", "sha256"})
AGGREGATE_FOUNDATIONAL_PREREQUISITE_READINESS_SUMMARY_ROW_FIELDS = frozenset(
    {"id", "readiness_summary_sha256"}
)
FOUNDATIONAL_PREREQUISITE_SIGNATURE_FIELDS = frozenset(
    {"algorithm", "public_key_fingerprint_sha256", "signature_hex"}
)
AGGREGATE_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS = frozenset(
    {
        "schema",
        "present",
        "valid",
        "required_ids",
        "prerequisite_count",
        "generated_at_unix",
        "oldest_evidence_generated_at_unix",
        "newest_evidence_generated_at_unix",
        "deployment_id",
        "environment",
        "release_sequence",
        "previous_envelope_sha256",
        "signer_public_key_fingerprint_sha256",
        "topology_qualification",
        "resilience_qualification",
        "evidence_anchor_sha256",
        "prerequisite_readiness_summary_sha256",
        "lane_summary_sha256",
        "path",
        "sha256",
        "errors",
    }
)
AGGREGATE_MISSING_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS = frozenset(
    {"schema", "present", "valid", "errors"}
)
POP_CREDENTIALS_ROOT_BOUND_FINGERPRINT_FIELDS = tuple(
    (
        kind_name,
        "synced_root_digest_hex" if kind_name == "juror_client" else "root_digest_hex",
    )
    for kind_name in POP_CREDENTIALS_ROOT_BOUND_KINDS
)
POP_CREDENTIALS_REVOCATION_BOUND_FINGERPRINT_FIELDS = tuple(
    (
        kind_name,
        (
            "synced_revocation_list_digest_hex"
            if kind_name == "juror_client"
            else "revocation_list_digest_hex"
        ),
    )
    for kind_name in POP_CREDENTIALS_REVOCATION_BOUND_KINDS
)
MAX_SUMMARY_BYTES = 4 * 1024 * 1024
DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS = 14 * 24 * 60 * 60
LOWER_HEX_DIGITS = set("0123456789abcdef")
PRODUCTION_READY_ENVIRONMENTS = frozenset({"prod", "production"})
FORBIDDEN_PRODUCTION_DEPLOYMENT_MARKERS = frozenset({"stage", "staging"})
SUCCESS_ARTIFACT_STATUSES = frozenset({"passed", "verified"})
REFERENCE_SDK_SUPPLY_CHAIN_SOURCE_ARTIFACT_FIELDS = frozenset(
    {"kind", "artifact_path", "sha256"}
)
REFERENCE_SDK_SUPPLY_CHAIN_SOURCE_DIGEST_BINDINGS = (
    ("sbom_index", "sbom_index_digest_hex"),
    ("vulnerability_report", "vulnerability_report_digest_hex"),
    ("provenance_bundle", "provenance_bundle_digest_hex"),
)
REFERENCE_SDK_PUBLIC_PROVENANCE_FINGERPRINT_FIELDS = (
    "provenance_certificate_identity",
    "provenance_oidc_issuer",
)
REFERENCE_SDK_PUBLIC_PROVENANCE_PATH_COMPONENT_RE = re.compile(
    r"^[A-Za-z0-9._~!$&'()*+,;=:@-]+\Z"
)
REFERENCE_SDK_JWT_LIKE_PATH_COMPONENT_RE = re.compile(
    r"(?<![A-Za-z0-9_-])"
    r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}"
    r"(?![A-Za-z0-9_-])"
)
REFERENCE_SDK_RESERVED_PUBLIC_HOST_SUFFIXES = (
    ".internal",
    ".invalid",
    ".local",
    ".localhost",
    ".onion",
    ".test",
)
REFERENCE_SDK_LEGACY_IPV4_COMPONENT_RE = re.compile(
    r"(?:0x[0-9a-f]+|[0-9]+)\Z"
)

SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "evidence_json",
    "manifest_signing_key",
    "mnemonic",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "raw_archive",
    "raw_body",
    "raw_evidence",
    "raw",
    "raw_payload",
    "raw_request",
    "raw_response",
    "request_body",
    "response_body",
    "secret",
    "seed",
    "signed_transaction",
    "token",
}
PATH_SENSITIVE_KEY_FRAGMENTS = HIGH_RISK_SENSITIVE_KEY_FRAGMENTS - frozenset(
    {"requestbody", "responsebody"}
)

GATE_METADATA_FIELDS: dict[str, frozenset[str]] = {
    "ai_prescreen": frozenset(
        {
            "valid_executor_summary_digests",
            "deployment_context",
            "valid_notification_manifest_digests",
            "valid_policy_digests",
            "valid_runner_bindings",
            "valid_workflow_digests",
        }
    ),
    "appeal_finance": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_config_digests",
            "valid_multi_peer_runs",
            "valid_policy_digests",
        }
    ),
    "gateway_compliance": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_catalog_digests",
            "valid_catalog_history_bindings",
            "valid_policy_digests",
        }
    ),
    "gateway_load": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_policy_digests",
            "valid_staging_report_digests",
            "valid_suite_report_digests",
        }
    ),
    "governance_dag": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_checkpoint_digests",
            "valid_policy_digests",
            "valid_public_head_cids",
        }
    ),
    "hedging_billing": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_billing_cycles",
            "valid_cycle_bindings",
            "valid_policy_digests",
            "valid_reference_decision_ids",
        }
    ),
    "moderation_panel": frozenset(
        {
            "deployment_context",
            "metric_count_values",
            "metrics",
            "valid_case_digests",
            "valid_e2e_runs",
            "valid_evidence_viewer_digest_sets",
            "valid_policy_digests",
            "valid_roster_bindings",
            "valid_tally_bindings",
        }
    ),
    "orderbook": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_contract_digests",
            "valid_policy_digests",
        }
    ),
    "pdp": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_policy_digests",
            "valid_proof_summary_digests",
            "valid_provider_roster_digests",
            "valid_repair_handoff_digests",
        }
    ),
    "pop_credentials": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_juror_sync_bindings",
            "valid_policy_digests",
            "valid_pop_snapshot_digests",
            "valid_revocation_list_digests",
            "valid_root_digests",
        }
    ),
    "por": frozenset(
        {
            "archive_backends",
            "metric_count_values",
            "metrics",
            "valid_governance_archive_handoff_digests",
            "valid_policy_digests",
            "valid_seed_replay_digests",
        }
    ),
    "potr": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_policy_digests",
            "valid_pq_key_roster_digests",
            "valid_receipt_summary_digests",
            "valid_reputation_weight_policy_digests",
        }
    ),
    "reference_sdk_release": frozenset(
        {
            "signature_algorithms",
            "valid_archive_index_digests",
            "valid_ffi_contract_digests",
            "valid_header_digests",
            "valid_package_index_digests",
            "valid_policy_digests",
            "valid_provenance_bundle_digests",
            "valid_release_key_fingerprints",
            "valid_release_manifest_digests",
            "valid_release_manifest_reference_digests",
            "valid_sbom_index_digests",
            "valid_smoke_output_digests",
            "valid_vulnerability_report_digests",
        }
    ),
    "repair": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_failure_bundle_digests",
            "valid_handoff_digests",
            "valid_policy_digests",
            "valid_roster_digests",
        }
    ),
    "reputation": frozenset(
        {
            "metric_count_values",
            "metrics",
            "merkle_root_hex",
            "provider_count_values",
            "provider_ids",
            "snapshot_id_hex",
            "valid_reputation_weight_digests",
            "valid_snapshot_bindings",
        }
    ),
    "reserve_rent": frozenset(
        {
            "metric_count_values",
            "metrics",
            "valid_policy_digests",
            "valid_policy_matrix_bindings",
            "valid_policy_matrix_ledger_bindings",
            "valid_provider_bakes",
        }
    ),
    "transparency": frozenset(
        {
            "valid_cycle_digests",
            "valid_publication_bindings",
            "valid_source_batch_digests",
        }
    ),
}
PAYLOAD_FREE_SUMMARY_CORE_FIELDS = frozenset(
    {
        "schema",
        "status",
        "required_kinds",
        "thresholds",
        "evidence_file_count",
        "recognized_artifact_count",
        "recognized_artifacts",
        "required",
        "errors",
        "load_errors",
        "topology_qualification",
    }
)
PAYLOAD_FREE_SUMMARY_METADATA_FIELDS = frozenset().union(
    *GATE_METADATA_FIELDS.values()
)
PAYLOAD_FREE_SUMMARY_FIELDS = (
    PAYLOAD_FREE_SUMMARY_CORE_FIELDS | PAYLOAD_FREE_SUMMARY_METADATA_FIELDS
)
PAYLOAD_FREE_SUMMARY_LIST_METADATA_FIELDS = frozenset(
    field
    for field in PAYLOAD_FREE_SUMMARY_METADATA_FIELDS
    if field.startswith("valid_")
) | frozenset(
    {
        "archive_backends",
        "metric_count_values",
        "metrics",
        "provider_count_values",
        "provider_ids",
        "signature_algorithms",
    }
)
PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS = frozenset(
    {
        "valid_catalog_digests",
        "valid_case_digests",
        "valid_checkpoint_digests",
        "valid_config_digests",
        "valid_contract_digests",
        "valid_cycle_digests",
        "valid_executor_summary_digests",
        "valid_failure_bundle_digests",
        "valid_governance_archive_handoff_digests",
        "valid_handoff_digests",
        "valid_notification_manifest_digests",
        "valid_policy_digests",
        "valid_pop_snapshot_digests",
        "valid_pq_key_roster_digests",
        "valid_proof_summary_digests",
        "valid_provider_roster_digests",
        "valid_public_head_cids",
        "valid_receipt_summary_digests",
        "valid_reputation_weight_digests",
        "valid_reputation_weight_policy_digests",
        "valid_repair_handoff_digests",
        "valid_reference_decision_ids",
        "valid_archive_index_digests",
        "valid_ffi_contract_digests",
        "valid_header_digests",
        "valid_package_index_digests",
        "valid_provenance_bundle_digests",
        "valid_release_key_fingerprints",
        "valid_release_manifest_digests",
        "valid_release_manifest_reference_digests",
        "valid_sbom_index_digests",
        "valid_smoke_output_digests",
        "valid_vulnerability_report_digests",
        "valid_revocation_list_digests",
        "valid_root_digests",
        "valid_roster_digests",
        "valid_seed_replay_digests",
        "valid_source_batch_digests",
        "valid_staging_report_digests",
        "valid_suite_report_digests",
        "valid_workflow_digests",
    }
)
PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS = {
    "valid_cycle_bindings": {
        "statement_bundle_digest_hex": 64,
        "reconciliation_digest_hex": 64,
    },
    "valid_juror_sync_bindings": {
        "synced_root_digest_hex": 64,
        "synced_revocation_list_digest_hex": 64,
    },
    "valid_policy_matrix_bindings": {
        "policy_digest_hex": 64,
        "matrix_digest_hex": 64,
    },
    "valid_policy_matrix_ledger_bindings": {
        "policy_digest_hex": 64,
        "matrix_digest_hex": 64,
        "ledger_digest_hex": 64,
    },
    "valid_publication_bindings": {
        "source_batch_digest_hex": 64,
        "cycle_digest_hex": 64,
    },
    "valid_roster_bindings": {
        "case_digest_hex": 64,
        "roster_hash_hex": 64,
    },
    "valid_runner_bindings": {
        "manifest_id_hex": 32,
        "runner_hash_hex": 64,
        "subject_digest_hex": 64,
    },
    "valid_snapshot_bindings": {
        "snapshot_id_hex": 32,
        "merkle_root_hex": 64,
    },
    "valid_tally_bindings": {
        "case_digest_hex": 64,
        "roster_hash_hex": 64,
        "tally_digest_hex": 64,
    },
}
PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS = frozenset(
    {"metric_count_values", "provider_count_values"}
)
PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS: dict[str, dict[str, Any]] = {
    "valid_billing_cycles": {
        "strings": frozenset({"cycle_id", "deployment_id", "environment"}),
        "positive_ints": frozenset(
            {"cycle_index", "generated_at_unix", "statement_count"}
        ),
        "hex": {
            "policy_digest_hex": 64,
            "reference_decision_id_hex": 64,
            "statement_bundle_digest_hex": 64,
            "reconciliation_digest_hex": 64,
        },
    },
    "valid_e2e_runs": {
        "strings": frozenset({"deployment_id", "environment"}),
        "positive_ints": frozenset(
            {"case_count", "generated_at_unix", "peer_count", "validator_count"}
        ),
        "hex": {
            "case_digest_hex": 64,
            "roster_hash_hex": 64,
            "tally_digest_hex": 64,
        },
    },
    "valid_evidence_viewer_digest_sets": {
        "strings": frozenset(),
        "positive_ints": frozenset(),
        "hex": {
            "catalog_digest_hex": 64,
            "case_digest_hex": 64,
            "roster_hash_hex": 64,
            "session_manifest_digest_hex": 64,
            "watermark_metadata_digest_hex": 64,
            "access_log_digest_hex": 64,
            "legal_hold_receipt_digest_hex": 64,
            "transparency_report_digest_hex": 64,
            "audit_digest_hex": 64,
        },
    },
    "valid_catalog_history_bindings": {
        "strings": frozenset(),
        "positive_ints": frozenset(
            {"catalog_sequence", "predecessor_catalog_sequence"}
        ),
        "hex": {
            "catalog_digest_hex": 64,
            "predecessor_catalog_digest_hex": 64,
        },
    },
    "valid_multi_peer_runs": {
        "strings": frozenset({"deployment_id", "environment"}),
        "positive_ints": frozenset(
            {"case_count", "generated_at_unix", "peer_count", "validator_count"}
        ),
        "hex": {"config_digest_hex": 64},
    },
    "valid_provider_bakes": {
        "strings": frozenset({"bake_id", "deployment_id", "environment"}),
        "positive_ints": frozenset(
            {
                "completed_at_unix",
                "provider_count",
                "scheduled_lifecycle_canary_defaulted_provider_count",
                "scheduled_lifecycle_canary_last_tick_at_unix",
                "scheduled_lifecycle_canary_tick_count",
                "started_at_unix",
            }
        ),
        "hex": {
            "ledger_digest_hex": 64,
            "matrix_digest_hex": 64,
            "policy_digest_hex": 64,
        },
        "ordered_int_pairs": (
            ("started_at_unix", "completed_at_unix"),
            ("scheduled_lifecycle_canary_last_tick_at_unix", "completed_at_unix"),
        ),
    },
}
PAYLOAD_FREE_SUMMARY_OBJECT_LIST_DOMAIN_IDENTITY_FIELDS = {
    "valid_billing_cycles": ("cycle_id",),
    "valid_catalog_history_bindings": (
        "catalog_digest_hex",
        "catalog_sequence",
    ),
    "valid_e2e_runs": ("case_digest_hex", "roster_hash_hex", "tally_digest_hex"),
    "valid_evidence_viewer_digest_sets": ("case_digest_hex", "roster_hash_hex"),
    "valid_multi_peer_runs": ("deployment_id", "environment", "generated_at_unix"),
    "valid_provider_bakes": ("bake_id",),
}
PAYLOAD_FREE_SUMMARY_OBJECT_LIST_REQUIRED_KIND_COUNTS = {
    "valid_billing_cycles": "billing_cycle",
    "valid_catalog_history_bindings": "catalog_promotion",
    "valid_e2e_runs": "e2e_panel",
    "valid_evidence_viewer_digest_sets": "evidence_viewer",
    "valid_multi_peer_runs": "multi_peer_reconciliation",
    "valid_provider_bakes": "provider_bake",
}
PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS = {
    ("appeal_finance", "valid_multi_peer_runs"): "multi_peer_reconciliation",
    (
        "gateway_compliance",
        "valid_catalog_history_bindings",
    ): "catalog_promotion",
    ("hedging_billing", "valid_billing_cycles"): "billing_cycle",
    ("moderation_panel", "valid_e2e_runs"): "e2e_panel",
    ("moderation_panel", "valid_evidence_viewer_digest_sets"): "evidence_viewer",
    ("reserve_rent", "valid_provider_bakes"): "provider_bake",
}
PAYLOAD_FREE_SUMMARY_OBJECT_LIST_STRING_FIELD_POLICIES = {
    ("hedging_billing", "valid_billing_cycles", "cycle_id"): {
        "forbidden_markers": HEDGING_BILLING_FORBIDDEN_CYCLE_ID_MARKERS,
        "pattern": HEDGING_BILLING_CYCLE_ID_PATTERN,
        "pattern_error": "must match canonical lowercase `billing-cycle-*`",
    },
    ("reserve_rent", "valid_provider_bakes", "bake_id"): {
        "forbidden_markers": RESERVE_RENT_FORBIDDEN_BAKE_ID_MARKERS,
        "pattern": RESERVE_RENT_BAKE_ID_PATTERN,
        "pattern_error": "must match canonical lowercase `reserve-bake-*`",
    },
}
PAYLOAD_FREE_SUMMARY_OBJECT_LIST_FINGERPRINT_HEX_BINDINGS = {
    field: {
        metadata_field: metadata_field
        for metadata_field in schema.get("hex", {})
    }
    for field, schema in PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS.items()
}
PAYLOAD_FREE_SUMMARY_STRING_METADATA_FIELDS = frozenset(
    {"merkle_root_hex", "snapshot_id_hex"}
)
PAYLOAD_FREE_SUMMARY_HEX_METADATA_LENGTHS = {
    "merkle_root_hex": 64,
    "snapshot_id_hex": 32,
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_BINDINGS = {
    "valid_catalog_digests": "catalog_digest_hex",
    "valid_case_digests": "case_digest_hex",
    "valid_checkpoint_digests": "checkpoint_digest_hex",
    "valid_config_digests": "config_digest_hex",
    "valid_contract_digests": "contract_digest_hex",
    "valid_cycle_digests": "cycle_digest_hex",
    "valid_executor_summary_digests": "execution_summary_digest_hex",
    "valid_failure_bundle_digests": "evidence_bundle_digest_hex",
    "valid_governance_archive_handoff_digests": (
        "governance_archive_handoff_digest_hex"
    ),
    "valid_handoff_digests": "handoff_digest_hex",
    "valid_notification_manifest_digests": "manifest_body_blake3_hex",
    "valid_policy_digests": "policy_digest_hex",
    "valid_pop_snapshot_digests": "pop_snapshot_digest_hex",
    "valid_pq_key_roster_digests": "pq_key_roster_digest_hex",
    "valid_proof_summary_digests": "proof_summary_digest_hex",
    "valid_provider_roster_digests": "provider_roster_digest_hex",
    "valid_public_head_cids": "public_head_cid_hex",
    "valid_receipt_summary_digests": "receipt_summary_digest_hex",
    "valid_reputation_weight_digests": "weights_digest_hex",
    "valid_reputation_weight_policy_digests": "reputation_weight_policy_digest_hex",
    "valid_repair_handoff_digests": "repair_handoff_digest_hex",
    "valid_reference_decision_ids": "decision_id_hex",
    "valid_archive_index_digests": "archive_index_digest_hex",
    "valid_ffi_contract_digests": "ffi_contract_digest_hex",
    "valid_header_digests": "header_digest_hex",
    "valid_package_index_digests": "package_index_digest_hex",
    "valid_provenance_bundle_digests": "provenance_bundle_digest_hex",
    "valid_release_key_fingerprints": "public_key_fingerprint_hex",
    "valid_release_manifest_digests": "manifest_digest_hex",
    "valid_release_manifest_reference_digests": "release_manifest_digest_hex",
    "valid_sbom_index_digests": "sbom_index_digest_hex",
    "valid_smoke_output_digests": "smoke_output_digest_hex",
    "valid_vulnerability_report_digests": "vulnerability_report_digest_hex",
    "valid_revocation_list_digests": "revocation_list_digest_hex",
    "valid_root_digests": "root_digest_hex",
    "valid_roster_digests": "roster_digest_hex",
    "valid_seed_replay_digests": "seed_replay_digest_hex",
    "valid_source_batch_digests": "source_batch_digest_hex",
    "valid_staging_report_digests": "staging_report_digest_hex",
    "valid_suite_report_digests": "suite_report_digest_hex",
    "valid_workflow_digests": "workflow_digest_hex",
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_SOURCE_KINDS = {
    ("ai_prescreen", "valid_executor_summary_digests"): ("commit_reveal_executor",),
    ("ai_prescreen", "valid_notification_manifest_digests"): (
        "notification_transport",
    ),
    ("ai_prescreen", "valid_policy_digests"): ("runner",),
    ("ai_prescreen", "valid_workflow_digests"): ("end_to_end_workflow",),
    ("appeal_finance", "valid_config_digests"): ("pricing_config",),
    ("appeal_finance", "valid_policy_digests"): ("pricing_config",),
    ("gateway_compliance", "valid_catalog_digests"): ("catalog_promotion",),
    ("gateway_compliance", "valid_policy_digests"): ("catalog_promotion",),
    ("gateway_load", "valid_policy_digests"): ("staging_load",),
    ("gateway_load", "valid_staging_report_digests"): ("staging_load",),
    ("gateway_load", "valid_suite_report_digests"): ("local_conformance",),
    ("governance_dag", "valid_checkpoint_digests"): ("operator_recovery",),
    ("governance_dag", "valid_policy_digests"): ("publisher_service",),
    ("governance_dag", "valid_public_head_cids"): ("publisher_service",),
    ("hedging_billing", "valid_policy_digests"): ("billing_cycle",),
    ("hedging_billing", "valid_reference_decision_ids"): ("reference_price",),
    ("moderation_panel", "valid_case_digests"): ("appeal_intake",),
    ("moderation_panel", "valid_policy_digests"): ("e2e_panel",),
    ("orderbook", "valid_contract_digests"): ("contract_surface",),
    ("orderbook", "valid_policy_digests"): ("contract_surface",),
    ("pdp", "valid_policy_digests"): ("proof_generation",),
    ("pdp", "valid_proof_summary_digests"): ("proof_generation",),
    ("pdp", "valid_provider_roster_digests"): ("proof_generation",),
    ("pdp", "valid_repair_handoff_digests"): ("governance_repair",),
    ("pop_credentials", "valid_policy_digests"): ("verifier_service",),
    ("pop_credentials", "valid_pop_snapshot_digests"): ("moderation_integration",),
    ("pop_credentials", "valid_revocation_list_digests"): (
        "issuer_bundle",
        "revocation_registry",
    ),
    ("pop_credentials", "valid_root_digests"): ("issuer_bundle", "commitment_root"),
    ("por", "valid_governance_archive_handoff_digests"): ("reporting_archive",),
    ("por", "valid_policy_digests"): ("randomness",),
    ("por", "valid_seed_replay_digests"): ("randomness",),
    ("potr", "valid_policy_digests"): ("governance_approval",),
    ("potr", "valid_pq_key_roster_digests"): ("governance_approval",),
    ("potr", "valid_receipt_summary_digests"): ("multi_provider_probe",),
    ("potr", "valid_reputation_weight_policy_digests"): ("governance_approval",),
    ("reference_sdk_release", "valid_policy_digests"): ("signed_manifest",),
    ("reference_sdk_release", "valid_archive_index_digests"): ("release_archive",),
    ("reference_sdk_release", "valid_ffi_contract_digests"): (
        "ffi_header_contract",
    ),
    ("reference_sdk_release", "valid_header_digests"): ("ffi_header_contract",),
    ("reference_sdk_release", "valid_package_index_digests"): (
        "downstream_bindings",
    ),
    ("reference_sdk_release", "valid_provenance_bundle_digests"): (
        "supply_chain",
    ),
    ("reference_sdk_release", "valid_release_key_fingerprints"): (
        "signed_manifest",
    ),
    ("reference_sdk_release", "valid_release_manifest_digests"): (
        "signed_manifest",
    ),
    ("reference_sdk_release", "valid_release_manifest_reference_digests"): (
        "release_archive",
        "supply_chain",
        "downstream_bindings",
        "cookbook_smoke",
        "ffi_header_contract",
        "governance_approval",
    ),
    ("reference_sdk_release", "valid_sbom_index_digests"): ("supply_chain",),
    ("reference_sdk_release", "valid_smoke_output_digests"): ("cookbook_smoke",),
    ("reference_sdk_release", "valid_vulnerability_report_digests"): (
        "supply_chain",
    ),
    ("repair", "valid_failure_bundle_digests"): ("failure_capture",),
    ("repair", "valid_handoff_digests"): ("governance_handoff",),
    ("repair", "valid_policy_digests"): ("governance_handoff",),
    ("repair", "valid_roster_digests"): ("auditor_roster",),
    ("reputation", "valid_reputation_weight_digests"): ("publish", "latest"),
    ("reserve_rent", "valid_policy_digests"): ("policy_config",),
    ("transparency", "valid_cycle_digests"): ("publication",),
    ("transparency", "valid_source_batch_digests"): ("source_entry",),
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_BINDINGS = {
    "merkle_root_hex": "merkle_root_hex",
    "snapshot_id_hex": "snapshot_id_hex",
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_SOURCE_KINDS = {
    ("reputation", "merkle_root_hex"): ("publish", "latest"),
    ("reputation", "snapshot_id_hex"): ("publish", "latest"),
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS = {
    "archive_backends": "archive_backend",
    "provider_ids": "provider_id",
    "signature_algorithms": "signature_algorithm",
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS = {
    ("por", "archive_backends"): ("reporting_archive",),
    ("reference_sdk_release", "signature_algorithms"): ("signed_manifest",),
    ("reputation", "provider_ids"): ("provider",),
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_BINDINGS = {
    "metrics": "metrics",
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS = {
    ("appeal_finance", "metrics"): ("dashboard_metrics",),
    ("gateway_compliance", "metrics"): ("observability",),
    ("gateway_load", "metrics"): ("telemetry_slo",),
    ("governance_dag", "metrics"): ("observability",),
    ("hedging_billing", "metrics"): ("metrics_alerts",),
    ("moderation_panel", "metrics"): ("metrics_alerts",),
    ("orderbook", "metrics"): ("observability",),
    ("pdp", "metrics"): ("observability",),
    ("pop_credentials", "metrics"): ("metrics_alerts",),
    ("por", "metrics"): ("observability",),
    ("potr", "metrics"): ("observability",),
    ("repair", "metrics"): ("observability",),
    ("reputation", "metrics"): ("metrics",),
    ("reserve_rent", "metrics"): ("metrics_alerts",),
}
PAYLOAD_FREE_SUMMARY_ALLOWED_STRING_LIST_VALUES = {
    ("por", "archive_backends"): POR_ALLOWED_ARCHIVE_BACKENDS,
    (
        "reference_sdk_release",
        "signature_algorithms",
    ): REFERENCE_SDK_ALLOWED_SIGNATURE_ALGORITHMS,
}
PAYLOAD_FREE_SUMMARY_REQUIRED_STRING_LIST_VALUES = {
    ("appeal_finance", "metrics"): APPEAL_FINANCE_REQUIRED_METRICS,
    ("gateway_compliance", "metrics"): GATEWAY_COMPLIANCE_REQUIRED_METRICS,
    ("gateway_load", "metrics"): GATEWAY_LOAD_REQUIRED_METRICS,
    ("governance_dag", "metrics"): GOVERNANCE_DAG_REQUIRED_METRICS,
    ("hedging_billing", "metrics"): HEDGING_BILLING_REQUIRED_METRICS,
    ("moderation_panel", "metrics"): MODERATION_PANEL_REQUIRED_METRICS,
    ("orderbook", "metrics"): ORDERBOOK_REQUIRED_METRICS,
    ("pdp", "metrics"): PDP_REQUIRED_METRICS,
    ("pop_credentials", "metrics"): POP_CREDENTIALS_REQUIRED_METRICS,
    ("por", "metrics"): POR_REQUIRED_METRICS,
    ("potr", "metrics"): POTR_REQUIRED_METRICS,
    ("repair", "metrics"): REPAIR_REQUIRED_METRICS,
    ("reputation", "metrics"): REPUTATION_REQUIRED_METRICS,
    ("reserve_rent", "metrics"): RESERVE_RENT_REQUIRED_METRICS,
}
PAYLOAD_FREE_SUMMARY_STRING_LIST_COUNT_BINDINGS = {
    ("appeal_finance", "metrics"): "metric_count_values",
    ("gateway_compliance", "metrics"): "metric_count_values",
    ("gateway_load", "metrics"): "metric_count_values",
    ("governance_dag", "metrics"): "metric_count_values",
    ("hedging_billing", "metrics"): "metric_count_values",
    ("moderation_panel", "metrics"): "metric_count_values",
    ("orderbook", "metrics"): "metric_count_values",
    ("pdp", "metrics"): "metric_count_values",
    ("pop_credentials", "metrics"): "metric_count_values",
    ("por", "metrics"): "metric_count_values",
    ("potr", "metrics"): "metric_count_values",
    ("repair", "metrics"): "metric_count_values",
    ("reputation", "metrics"): "metric_count_values",
    ("reserve_rent", "metrics"): "metric_count_values",
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_BINDINGS = {
    "metric_count_values": "metric_count",
    "provider_count_values": "provider_count",
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS = {
    ("appeal_finance", "metric_count_values"): ("dashboard_metrics",),
    ("gateway_compliance", "metric_count_values"): ("observability",),
    ("gateway_load", "metric_count_values"): ("telemetry_slo",),
    ("governance_dag", "metric_count_values"): ("observability",),
    ("hedging_billing", "metric_count_values"): ("metrics_alerts",),
    ("moderation_panel", "metric_count_values"): ("metrics_alerts",),
    ("orderbook", "metric_count_values"): ("observability",),
    ("pdp", "metric_count_values"): ("observability",),
    ("pop_credentials", "metric_count_values"): ("metrics_alerts",),
    ("por", "metric_count_values"): ("observability",),
    ("potr", "metric_count_values"): ("observability",),
    ("repair", "metric_count_values"): ("observability",),
    ("reputation", "metric_count_values"): ("metrics",),
    ("reputation", "provider_count_values"): ("publish", "latest"),
    ("reserve_rent", "metric_count_values"): ("metrics_alerts",),
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_FIELDS = {
    "valid_cycle_bindings": (
        "statement_bundle_digest_hex",
        "reconciliation_digest_hex",
    ),
    "valid_juror_sync_bindings": (
        "synced_root_digest_hex",
        "synced_revocation_list_digest_hex",
    ),
    "valid_policy_matrix_bindings": (
        "policy_digest_hex",
        "matrix_digest_hex",
    ),
    "valid_policy_matrix_ledger_bindings": (
        "policy_digest_hex",
        "matrix_digest_hex",
        "ledger_digest_hex",
    ),
    "valid_publication_bindings": (
        "source_batch_digest_hex",
        "cycle_digest_hex",
    ),
    "valid_roster_bindings": (
        "case_digest_hex",
        "roster_hash_hex",
    ),
    "valid_runner_bindings": (
        "manifest_id_hex",
        "runner_hash_hex",
        "subject_digest_hex",
    ),
    "valid_snapshot_bindings": (
        "snapshot_id_hex",
        "merkle_root_hex",
    ),
    "valid_tally_bindings": (
        "case_digest_hex",
        "roster_hash_hex",
        "tally_digest_hex",
    ),
}
PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_SOURCE_KINDS = {
    ("ai_prescreen", "valid_runner_bindings"): ("runner",),
    ("hedging_billing", "valid_cycle_bindings"): ("billing_cycle",),
    ("moderation_panel", "valid_roster_bindings"): ("sortition_roster",),
    ("moderation_panel", "valid_tally_bindings"): ("commit_reveal",),
    ("pop_credentials", "valid_juror_sync_bindings"): ("juror_client",),
    ("reputation", "valid_snapshot_bindings"): ("publish", "latest"),
    ("reserve_rent", "valid_policy_matrix_bindings"): ("quote_matrix",),
    ("reserve_rent", "valid_policy_matrix_ledger_bindings"): ("ledger_digest",),
    ("transparency", "valid_publication_bindings"): ("publication",),
}
PAYLOAD_FREE_SUMMARY_OBJECT_METADATA_FIELDS = {
    "deployment_context": frozenset({"deployment_id", "environment"}),
}
PAYLOAD_FREE_SUMMARY_ORDERED_LIST_METADATA_FIELDS = (
    PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS
    | frozenset(PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS)
    | PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS
    | frozenset(
        {"archive_backends", "metrics", "provider_ids", "signature_algorithms"}
    )
)
MAX_SUMMARY_METADATA_DEPTH = 32
PAYLOAD_FREE_ARTIFACT_FIELDS = frozenset(
    {
        "kind",
        "path",
        "sha256",
        "schema",
        "status",
        "fingerprint",
        "valid",
        "errors",
    }
)
PAYLOAD_FREE_REQUIRED_ROW_FIELDS = frozenset(
    {
        "schema",
        "present",
        "valid",
        "artifact_count",
        "artifacts",
        "errors",
    }
)
AGGREGATE_REQUIRED_GATE_ROW_FIELDS = frozenset(
    {
        "schema",
        "present",
        "valid",
        "required_kind_count",
        "expected_required_kind_count",
        "evidence_file_count",
        "recognized_artifact_count",
        "artifact_count",
        "thresholds",
        "oldest_generated_at_unix",
        "newest_generated_at_unix",
        "deployment_id",
        "environment",
        "expected_required_kinds",
        "topology_qualification",
        "errors",
        "path",
        "sha256",
    }
)
AGGREGATE_MISSING_GATE_ROW_FIELDS = frozenset(
    {
        "schema",
        "present",
        "valid",
        "errors",
    }
)
AGGREGATE_SUMMARY_FIELDS = frozenset(
    {
        "schema",
        "status",
        "required_gates",
        "thresholds",
        "summary_file_count",
        "recognized_summary_count",
        "deployment",
        "topology_qualification",
        "resilience_qualification",
        "foundational_prerequisites",
        "required",
        "errors",
    }
)


@dataclass(frozen=True)
class GateSummaryKind:
    """One SoraFS production-readiness lane summary."""

    name: str
    schema: str
    required_kinds: tuple[str, ...]


GATE_SUMMARY_KINDS: tuple[GateSummaryKind, ...] = (
    GateSummaryKind(
        "ai_prescreen",
        "sorafs.moderation.ai_prescreen.rollout_evidence_gate.v1",
        AI_PRESCREEN_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "appeal_finance",
        "sorafs.appeal_finance.rollout_evidence_gate.v1",
        APPEAL_FINANCE_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "gateway_compliance",
        "sorafs.gateway_compliance.rollout_evidence_gate.v1",
        GATEWAY_COMPLIANCE_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "gateway_load",
        "sorafs.gateway_load.rollout_evidence_gate.v1",
        GATEWAY_LOAD_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "governance_dag",
        "sorafs.governance_dag.rollout_evidence_gate.v1",
        GOVERNANCE_DAG_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "hedging_billing",
        "sorafs.hedging_billing.rollout_evidence_gate.v1",
        HEDGING_BILLING_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "moderation_panel",
        "sorafs.moderation_panel.rollout_evidence_gate.v1",
        MODERATION_PANEL_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "orderbook",
        "sorafs.orderbook.rollout_evidence_gate.v1",
        ORDERBOOK_REQUIRED_KINDS,
    ),
    GateSummaryKind("pdp", "sorafs.pdp.rollout_evidence_gate.v1", PDP_REQUIRED_KINDS),
    GateSummaryKind(
        "pop_credentials",
        "sorafs.pop_credentials.rollout_evidence_gate.v1",
        POP_CREDENTIALS_REQUIRED_KINDS,
    ),
    GateSummaryKind("por", "sorafs.por.rollout_evidence_gate.v1", POR_REQUIRED_KINDS),
    GateSummaryKind(
        "potr",
        "sorafs.potr.rollout_evidence_gate.v1",
        POTR_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "reference_sdk_release",
        "sorafs.reference_sdk.release_evidence_gate.v1",
        REFERENCE_SDK_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "repair",
        "sorafs.repair.rollout_evidence_gate.v1",
        REPAIR_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "reputation",
        "sorafs.reputation.rollout_evidence_gate.v1",
        REPUTATION_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "reserve_rent",
        "sorafs.reserve_rent.rollout_evidence_gate.v1",
        RESERVE_RENT_REQUIRED_KINDS,
    ),
    GateSummaryKind(
        "transparency",
        "sorafs.transparency.rollout_evidence_gate.v1",
        TRANSPARENCY_REQUIRED_KINDS,
    ),
)

SCHEMA_TO_GATE = {kind.schema: kind for kind in GATE_SUMMARY_KINDS}
GATE_BY_NAME = {kind.name: kind for kind in GATE_SUMMARY_KINDS}
DEFAULT_REQUIRED_GATES = tuple(kind.name for kind in GATE_SUMMARY_KINDS)
if set(_FOUNDATIONAL_MAPPED_LANES) != set(DEFAULT_REQUIRED_GATES):
    raise RuntimeError(
        "foundational prerequisite lane map must cover the canonical readiness lanes"
    )


GATE_REQUIRED_KIND_SCHEMAS = {
    "ai_prescreen": evidence_schema_by_kind(AI_PRESCREEN_KIND_BY_NAME),
    "appeal_finance": evidence_schema_by_kind(APPEAL_FINANCE_KIND_BY_NAME),
    "gateway_compliance": evidence_schema_by_kind(GATEWAY_COMPLIANCE_KIND_BY_NAME),
    "gateway_load": evidence_schema_by_kind(GATEWAY_LOAD_KIND_BY_NAME),
    "governance_dag": evidence_schema_by_kind(GOVERNANCE_DAG_KIND_BY_NAME),
    "hedging_billing": evidence_schema_by_kind(HEDGING_BILLING_KIND_BY_NAME),
    "moderation_panel": evidence_schema_by_kind(MODERATION_PANEL_KIND_BY_NAME),
    "orderbook": evidence_schema_by_kind(ORDERBOOK_KIND_BY_NAME),
    "pdp": evidence_schema_by_kind(PDP_KIND_BY_NAME),
    "pop_credentials": evidence_schema_by_kind(POP_CREDENTIALS_KIND_BY_NAME),
    "por": evidence_schema_by_kind(POR_KIND_BY_NAME),
    "potr": evidence_schema_by_kind(POTR_KIND_BY_NAME),
    "reference_sdk_release": evidence_schema_by_kind(REFERENCE_SDK_KIND_BY_NAME),
    "repair": evidence_schema_by_kind(REPAIR_KIND_BY_NAME),
    "reputation": evidence_schema_by_kind(REPUTATION_KIND_BY_NAME),
    "reserve_rent": evidence_schema_by_kind(RESERVE_RENT_KIND_BY_NAME),
    "transparency": evidence_schema_by_kind(TRANSPARENCY_KIND_BY_NAME),
}


__all__ = (
    "SUMMARY_SCHEMA",
    "GATEWAY_MODERATION_CATALOG_MISMATCH_ERROR",
    "FOUNDATIONAL_PREREQUISITE_SCHEMA",
    "FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN",
    "RESILIENCE_QUALIFICATION_RECEIPT_SCHEMA",
    "RESILIENCE_QUALIFICATION_SUMMARY_SCHEMA",
    "RESILIENCE_QUALIFICATION_SIGNATURE_DOMAIN",
    "RESILIENCE_QUALIFICATION_REQUIREMENTS",
    "RESILIENCE_QUALIFICATION_SUMMARY_FIELDS",
    "RESILIENCE_QUALIFICATION_ARTIFACT_BINDING_FIELDS",
    "RESILIENCE_QUALIFICATION_AUTHENTICATION_FIELDS",
    "RESILIENCE_QUALIFICATION_BINDING_SCHEMA",
    "RESILIENCE_QUALIFICATION_BINDING_FIELDS",
    "AGGREGATE_RESILIENCE_QUALIFICATION_SCHEMA",
    "AGGREGATE_RESILIENCE_QUALIFICATION_FIELDS",
    "FOUNDATIONAL_PREREQUISITE_IDS",
    "FOUNDATIONAL_PREREQUISITE_LANES",
    "MAX_FOUNDATIONAL_RELEASE_SEQUENCE",
    "FOUNDATIONAL_PREREQUISITE_FIELDS",
    "FOUNDATIONAL_PREREQUISITE_DEPLOYMENT_FIELDS",
    "FOUNDATIONAL_PREREQUISITE_ROW_FIELDS",
    "FOUNDATIONAL_LANE_SUMMARY_ROW_FIELDS",
    "AGGREGATE_FOUNDATIONAL_PREREQUISITE_READINESS_SUMMARY_ROW_FIELDS",
    "FOUNDATIONAL_PREREQUISITE_SIGNATURE_FIELDS",
    "AGGREGATE_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS",
    "AGGREGATE_MISSING_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS",
    "POP_CREDENTIALS_ROOT_BOUND_FINGERPRINT_FIELDS",
    "POP_CREDENTIALS_REVOCATION_BOUND_FINGERPRINT_FIELDS",
    "MAX_SUMMARY_BYTES",
    "DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS",
    "LOWER_HEX_DIGITS",
    "PRODUCTION_READY_ENVIRONMENTS",
    "FORBIDDEN_PRODUCTION_DEPLOYMENT_MARKERS",
    "SUCCESS_ARTIFACT_STATUSES",
    "REFERENCE_SDK_SUPPLY_CHAIN_SOURCE_ARTIFACT_FIELDS",
    "REFERENCE_SDK_SUPPLY_CHAIN_SOURCE_DIGEST_BINDINGS",
    "REFERENCE_SDK_PUBLIC_PROVENANCE_FINGERPRINT_FIELDS",
    "REFERENCE_SDK_PUBLIC_PROVENANCE_PATH_COMPONENT_RE",
    "REFERENCE_SDK_JWT_LIKE_PATH_COMPONENT_RE",
    "REFERENCE_SDK_RESERVED_PUBLIC_HOST_SUFFIXES",
    "REFERENCE_SDK_LEGACY_IPV4_COMPONENT_RE",
    "SENSITIVE_KEYS",
    "PATH_SENSITIVE_KEY_FRAGMENTS",
    "GATE_METADATA_FIELDS",
    "PAYLOAD_FREE_SUMMARY_CORE_FIELDS",
    "PAYLOAD_FREE_SUMMARY_METADATA_FIELDS",
    "PAYLOAD_FREE_SUMMARY_FIELDS",
    "PAYLOAD_FREE_SUMMARY_LIST_METADATA_FIELDS",
    "PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS",
    "PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS",
    "PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS",
    "PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS",
    "PAYLOAD_FREE_SUMMARY_OBJECT_LIST_DOMAIN_IDENTITY_FIELDS",
    "PAYLOAD_FREE_SUMMARY_OBJECT_LIST_REQUIRED_KIND_COUNTS",
    "PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS",
    "PAYLOAD_FREE_SUMMARY_OBJECT_LIST_STRING_FIELD_POLICIES",
    "PAYLOAD_FREE_SUMMARY_OBJECT_LIST_FINGERPRINT_HEX_BINDINGS",
    "PAYLOAD_FREE_SUMMARY_STRING_METADATA_FIELDS",
    "PAYLOAD_FREE_SUMMARY_HEX_METADATA_LENGTHS",
    "PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_BINDINGS",
    "PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_SOURCE_KINDS",
    "PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_BINDINGS",
    "PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_SOURCE_KINDS",
    "PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS",
    "PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS",
    "PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_BINDINGS",
    "PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS",
    "PAYLOAD_FREE_SUMMARY_ALLOWED_STRING_LIST_VALUES",
    "PAYLOAD_FREE_SUMMARY_REQUIRED_STRING_LIST_VALUES",
    "PAYLOAD_FREE_SUMMARY_STRING_LIST_COUNT_BINDINGS",
    "PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_BINDINGS",
    "PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS",
    "PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_FIELDS",
    "PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_SOURCE_KINDS",
    "PAYLOAD_FREE_SUMMARY_OBJECT_METADATA_FIELDS",
    "PAYLOAD_FREE_SUMMARY_ORDERED_LIST_METADATA_FIELDS",
    "MAX_SUMMARY_METADATA_DEPTH",
    "PAYLOAD_FREE_ARTIFACT_FIELDS",
    "PAYLOAD_FREE_REQUIRED_ROW_FIELDS",
    "AGGREGATE_REQUIRED_GATE_ROW_FIELDS",
    "AGGREGATE_MISSING_GATE_ROW_FIELDS",
    "AGGREGATE_SUMMARY_FIELDS",
    "GateSummaryKind",
    "GATE_SUMMARY_KINDS",
    "SCHEMA_TO_GATE",
    "GATE_BY_NAME",
    "DEFAULT_REQUIRED_GATES",
    "GATE_REQUIRED_KIND_SCHEMAS",
)

