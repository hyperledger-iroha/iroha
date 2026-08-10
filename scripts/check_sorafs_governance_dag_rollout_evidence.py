#!/usr/bin/env python3
"""Validate SoraFS Governance DAG rollout evidence artifacts."""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_exception,
    emit_checker_notice,
    render_and_write_checker_summary,
    validate_checker_preflight,
)
from sorafs_evidence_paths import (  # noqa: E402
    discover_evidence_files,
    evidence_path_identities,
)
from sorafs_evidence_json import (  # noqa: E402
    load_evidence_json_with_sha256_or_record_error,
)
from sorafs_evidence_validation import (  # noqa: E402
    archive_artifact_path_label,
    forbidden_non_production_markers,
    build_evidence_artifact,
    count_evidence_artifacts,
    recognized_evidence_artifacts,
    count_evidence_files,
    evidence_gate_status,
    evidence_artifact_is_valid,
    evidence_artifact_fingerprint,
    evidence_schema_by_kind,
    hashable_evidence_values,
    init_evidence_artifact_buckets,
    build_required_evidence_summary,
    record_explicit_evidence_validation_errors,
    record_evidence_artifact,
    record_evidence_validation_errors,
    record_observed_evidence_value,
    validate_bound_evidence_digest_references,
    require_2xx_status,
    require_bool_true,
    require_count_equal,
    require_false,
    require_hex,
    require_config_backed_governance_approval,
    validate_standard_evidence_payload,
    require_maximum_int,
    require_minimum_int,
    require_object,
    require_object_array,
    require_policy_digest,
    required_evidence_kind_names,
    require_passed_status,
    require_positive_int,
    require_recent_timestamp,
    require_string,
    require_string_coverage,
    require_string_equal,
    require_string_inventory_count_match,
    require_zero_count,
)
from sorafs_required_kinds import (  # noqa: E402
    parse_required_kinds as parse_required_evidence_kinds,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)


from sorafs_topology_qualification import (  # noqa: E402
    add_topology_qualification_argument,
    bind_lane_summary_to_topology,
)

SUMMARY_SCHEMA = "sorafs.governance_dag.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_EVIDENCE_AGE_SECS = 7 * 24 * 60 * 60
DEFAULT_MAX_ROUTE_LATENCY_MS = 1_500
DEFAULT_MAX_PIN_LAG_SECS = 30 * 60
DEFAULT_MAX_HEAD_AGE_SECS = 30 * 60
DEFAULT_MIN_BLOCKS = 4
DEFAULT_MIN_PAYLOAD_KINDS = 6
HEX64_LEN = 64
KUBO_UNIXFS_PROFILE = "kubo_unixfs_v1_balanced_raw_leaves"
KUBO_UNIXFS_CHUNK_SIZE_BYTES = 1024 * 1024
KUBO_UNIXFS_MAX_LINKS_PER_NODE = 1024
KUBO_CID_VERSION = 1
KUBO_CID_MULTIHASH = "sha2-256"
MIRROR_RETENTION_MAX_ENTRIES = 65_536
MIRROR_RETENTION_MAX_BYTES = 512 * 1024 * 1024
STEADY_AUDIT_MAX_ENTRIES_PER_POLL = 64
STEADY_AUDIT_MAX_BYTES_PER_POLL = 16 * 1024 * 1024
INGRESS_ENFORCEMENT = "exclusive_authenticated_receiver"
REPLAY_POSTURE = "shared_sealed_atomic_consume_until_expiry"
BLOCK_REF_LABEL_PATTERN = re.compile(
    r"^governance-dag-block-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
BLOCK_REF_LABEL_ERROR = (
    "block_refs entries must match canonical lowercase `governance-dag-block-*`"
)
FORBIDDEN_INVENTORY_LABEL_MARKERS = frozenset(
    (
        "debug",
        "dev",
        "draft",
        "example",
        "fake",
        "latest",
        "local",
        "mock",
        "placeholder",
        "sample",
        "sandbox",
        "test",
        "todo",
    )
)

REQUIRED_PAYLOAD_KINDS = (
    "deal-settlement",
    "repair-audit",
    "reconciliation",
    "reputation-snapshot",
    "moderation-ballot-event",
    "appeal-finance-report",
    "appeal-finance-settlement-receipt",
    "orderbook-settlement-receipt",
)
REQUIRED_DASHBOARD_ROUTES = (
    "dashboard",
    "head",
    "block_lookup",
    "node_lookup",
    "digest_lookup",
    "checkpoint",
)
REQUIRED_METRICS = (
    "sorafs_governance_dag_publish_total",
    "sorafs_governance_dag_published_bytes_total",
    "sorafs_governance_dag_last_publish_timestamp_seconds",
    "sorafs_governance_dag_backlog",
    "sorafs_governance_dag_head_age_seconds",
    "sorafs_governance_dag_ipfs_pin_lag_seconds",
    "sorafs_governance_dag_validation_failure_total",
    "sorafs_governance_dag_mirror_drift",
)
PUBLIC_HEAD_BOUND_KINDS = (
    "mirror_datastore",
    "operator_recovery",
    "dashboard_api",
    "observability",
    "publication_e2e",
    "governance_approval",
)
POLICY_BOUND_KINDS = ("governance_approval",)
INGRESS_QUALIFICATION_BOUND_KINDS = ("governance_approval",)
INGRESS_QUALIFICATION_DIGEST_FIELDS = (
    "receiver_policy_digest_hex",
    "replay_namespace_digest_hex",
    "replica_set_digest_hex",
)
INGRESS_BINDING_DIGEST_FIELDS = (
    "kubo_ingress_binding_digest_hex",
    "signed_head_ingress_binding_digest_hex",
)
REQUIRED_PAYLOAD_KIND_SET = frozenset(REQUIRED_PAYLOAD_KINDS)

SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "car_payload",
    "dag_block",
    "dag_head",
    "head_bytes",
    "ledger",
    "mnemonic",
    "node_payload",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "raw_block",
    "raw_blocks",
    "raw_car",
    "raw_checkpoint",
    "raw_head",
    "raw_ledger",
    "raw_node",
    "raw_nodes",
    "raw_payload",
    "raw_response",
    "request_body",
    "response_body",
    "secret",
    "seed",
    "signed_transaction",
    "token",
}


@dataclass(frozen=True)
class EvidenceKind:
    """One SF-12 rollout evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("ingest_service", "sorafs.governance_dag.ingest_service_canary.v1"),
    EvidenceKind("publisher_service", "sorafs.governance_dag.publisher_service_canary.v1"),
    EvidenceKind("mirror_datastore", "sorafs.governance_dag.mirror_datastore_canary.v1"),
    EvidenceKind("operator_recovery", "sorafs.governance_dag.operator_recovery_canary.v1"),
    EvidenceKind("dashboard_api", "sorafs.governance_dag.dashboard_api_canary.v1"),
    EvidenceKind("observability", "sorafs.governance_dag.observability_canary.v1"),
    EvidenceKind("publication_e2e", "sorafs.governance_dag.publication_e2e_canary.v1"),
    EvidenceKind("governance_approval", "sorafs.governance_dag.governance_approval.v1"),
)

SCHEMA_TO_KIND = {kind.schema: kind for kind in EVIDENCE_KINDS}
KIND_BY_NAME = {kind.name: kind for kind in EVIDENCE_KINDS}
DEFAULT_REQUIRED_KINDS = tuple(kind.name for kind in EVIDENCE_KINDS)
COMMON_EVIDENCE_REQUIRED_FIELDS: tuple[str, ...] = (
    "schema",
    "status",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
)
EVIDENCE_REQUIRED_FIELDS: dict[str, tuple[str, ...]] = {
    "ingest_service": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "daemonized",
        "payload_validation_enabled",
        "publisher_signature_verified",
        "dedupe_by_digest_enabled",
        "quarantine_invalid_blocks",
        "source_count",
        "payload_kinds",
        "payload_bytes_included",
    ),
    "publisher_service": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "dag_builder_daemonized",
        "kubo_unixfs_profile",
        "unixfs_chunk_size_bytes",
        "unixfs_raw_leaves",
        "unixfs_balanced_layout",
        "unixfs_max_links_per_node",
        "cid_version",
        "cid_multihash",
        "locally_derived_cids_verified",
        "signed_http_head_cas_enabled",
        "strong_single_etag_verified",
        "conditional_cas_readback_verified",
        "signed_head_verified",
        "parent_chain_verified",
        "objects_pinned",
        "authenticated_ingress_qualified",
        "ingress_enforcement",
        "replay_posture",
        "ingress_scope_binding_verified",
        "receiver_policy_digest_hex",
        "replay_namespace_digest_hex",
        "replica_set_digest_hex",
        "kubo_ingress_binding_digest_hex",
        "signed_head_ingress_binding_digest_hex",
        "public_head_cid_hex",
        "policy_digest_hex",
        "pin_lag_seconds",
        "head_age_seconds",
        "block_count",
        "block_refs",
        "payload_kind_count",
        "payload_kinds",
        "raw_head_included",
    ),
    "mirror_datastore": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "public_head_cid_hex",
        "sealed_typed_store_enabled",
        "query_service_enabled",
        "mirror_index_verified",
        "head_lookup_verified",
        "block_lookup_verified",
        "node_lookup_verified",
        "digest_lookup_verified",
        "retention_max_entries",
        "retention_max_bytes",
        "exact_retained_source_suffix_verified",
        "fresh_checkpoint_coherent_reads_verified",
        "liveness_bound_reader_verified",
        "mirror_drift_detected",
        "missing_block_count",
        "raw_blocks_included",
    ),
    "operator_recovery": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "public_head_cid_hex",
        "live_head_fetch_verified",
        "public_checkpoint_published",
        "checkpoint_recovery_verified",
        "derived_mirror_recovery_verified",
        "recovered_head_matches_public_head",
        "post_loss_repair_verified",
        "head_object_repaired_with_same_cid",
        "block_object_repaired_with_same_cid",
        "public_head_unchanged_during_repair",
        "checkpoint_digest_hex",
        "raw_checkpoint_included",
    ),
    "dashboard_api": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "public_head_cid_hex",
        "route_count",
        "passed_route_count",
        "routes",
        "service_mirror_capability_installed",
        "fresh_checkpoint_coherent_reads_verified",
        "liveness_bound_reader_verified",
        "unready_reader_rejected",
        "reader_withdrawal_verified",
        "response_bodies_included",
    ),
    "observability": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "public_head_cid_hex",
        "metrics_scrape_success",
        "dashboard_provisioned",
        "alert_rules_installed",
        "publication_metrics_present",
        "first_full_audit_verified",
        "readiness_withheld_until_full_audit",
        "bounded_rotating_audit_verified",
        "audit_max_entries_per_poll",
        "audit_max_bytes_per_poll",
        "critical_alerts_firing",
        "metrics",
        "metric_count",
        "response_bodies_included",
    ),
    "publication_e2e": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "public_head_cid_hex",
        "local_kubo_tests_passed",
        "deterministic_unixfs_profile_verified",
        "signed_http_head_resolved",
        "strong_single_etag_cas_verified",
        "authenticated_ingress_qualification_verified",
        "replay_attack_rejected",
        "block_replay_verified",
        "duplicate_payload_rejected",
        "invalid_parent_quarantined",
        "post_loss_same_cid_repair_verified",
        "bounded_rotating_audit_verified",
        "fresh_torii_reads_verified",
        "stopped_service_reads_rejected",
        "publisher_key_failure_tested",
        "block_count",
        "block_refs",
        "payload_kind_count",
        "payload_kinds",
        "raw_blocks_included",
    ),
    "governance_approval": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "public_head_cid_hex",
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "publisher_keys_governed",
        "signed_http_head_endpoint_governed",
        "ingress_receiver_policy_governed",
        "replay_namespace_governed",
        "fixed_retention_contract_bound",
        "receiver_policy_digest_hex",
        "replay_namespace_digest_hex",
        "replica_set_digest_hex",
        "kubo_ingress_binding_digest_hex",
        "signed_head_ingress_binding_digest_hex",
        "retention_max_entries",
        "retention_max_bytes",
        "config_source",
        "policy_digest_hex",
    ),
}


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SF-12 Governance DAG rollout gate."""

    now_unix: int
    max_evidence_age_secs: int
    max_route_latency_ms: int
    max_pin_lag_secs: int
    max_head_age_secs: int
    min_blocks: int
    min_payload_kinds: int



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "schema",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
    "public_head_cid_hex",
    "checkpoint_digest_hex",
    "policy_digest_hex",
    "receiver_policy_digest_hex",
    "replay_namespace_digest_hex",
    "replica_set_digest_hex",
    "kubo_ingress_binding_digest_hex",
    "signed_head_ingress_binding_digest_hex",
    "metric_count",
    "metrics",
)


ROUTE_REQUIRED_FIELDS: tuple[str, ...] = (
    "name",
    "passed",
    "status_code",
    "body_blake3_hex",
    "latency_ms",
    "publisher_identity_present",
    "verification_valid",
)


def require_exact_fields(
    payload: dict[str, Any],
    expected: tuple[str, ...] | frozenset[str],
    errors: list[str],
    *,
    path: str,
) -> None:
    """Reject missing and unknown fields in a schema-closed object."""

    expected_set = frozenset(expected)
    actual = frozenset(payload)
    missing = sorted(expected_set - actual)
    unknown = sorted(actual - expected_set)
    if missing:
        errors.append(f"{path} is missing required fields: {', '.join(missing)}")
    if unknown:
        errors.append(f"{path} contains unknown fields: {', '.join(unknown)}")


def require_exact_positive_int(
    payload: dict[str, Any],
    field: str,
    expected: int,
    errors: list[str],
) -> None:
    """Require a positive integer to equal one first-release protocol constant."""

    value = require_positive_int(payload, field, errors)
    if value is not None and value != expected:
        errors.append(f"{field} must equal the V1 protocol value {expected}")


def require_nonzero_digest(
    payload: dict[str, Any],
    field: str,
    errors: list[str],
) -> str:
    """Require one exact lowercase digest with a non-zero identity."""

    value = require_hex(payload, field, HEX64_LEN, errors)
    if value and value == "0" * HEX64_LEN:
        errors.append(f"{field} must not be the zero digest")
        return ""
    return value


def validate_routes(payload: dict[str, Any], errors: list[str], options: ValidationOptions) -> None:
    for index, record in require_object_array(payload, "routes", errors):
        require_exact_fields(
            record,
            ROUTE_REQUIRED_FIELDS,
            errors,
            path=f"routes[{index}]",
        )
        require_bool_true(record, "passed", errors, path=f"routes[{index}].passed")
        require_2xx_status(
            record,
            "status_code",
            errors,
            path=f"routes[{index}].status_code",
        )
        require_hex(
            record,
            "body_blake3_hex",
            HEX64_LEN,
            errors,
            path=f"routes[{index}].body_blake3_hex",
        )
        require_maximum_int(
            record,
            "latency_ms",
            options.max_route_latency_ms,
            errors,
            path=f"routes[{index}].latency_ms",
        )
        for field in ("publisher_identity_present", "verification_valid"):
            require_bool_true(record, field, errors, path=f"routes[{index}].{field}")


def require_only_required_payload_kinds(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Reject payload-kind rows outside the reviewed Governance DAG inventory."""

    values = payload.get("payload_kinds")
    if not isinstance(values, list):
        return
    if any(
        not isinstance(value, str) or value not in REQUIRED_PAYLOAD_KIND_SET
        for value in values
    ):
        errors.append("payload_kinds must not include unknown values")


def require_only_required_values(
    payload: dict[str, Any],
    array_field: str,
    field: str,
    required_values: tuple[str, ...],
    errors: list[str],
) -> None:
    """Reject reviewed inventory rows outside a required closed string set."""

    values = payload.get(array_field)
    if not isinstance(values, list):
        return
    allowed = frozenset(required_values)
    for item in values:
        if field:
            if not isinstance(item, dict):
                continue
            value = item.get(field)
        else:
            value = item
        if not isinstance(value, str) or value not in allowed:
            errors.append(f"{array_field} must not include unknown values")
            return


def require_scalar_inventory_labels(
    payload: dict[str, Any],
    field: str,
    errors: list[str],
    *,
    pattern: re.Pattern[str],
    label_error: str,
) -> None:
    """Require reviewed production labels for scalar inventory entries."""

    values = payload.get(field)
    if not isinstance(values, list):
        return
    for index, value in enumerate(values):
        if not isinstance(value, str):
            continue
        if pattern.fullmatch(value) is None:
            errors.append(label_error)
            continue
        forbidden = forbidden_non_production_markers(value, FORBIDDEN_INVENTORY_LABEL_MARKERS)
        if forbidden:
            errors.append(
                f"{field}[{index}] must not contain non-production markers "
                f"{forbidden}"
            )


def validate_ingest_service(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "daemonized", errors)
    require_bool_true(payload, "payload_validation_enabled", errors)
    require_bool_true(payload, "publisher_signature_verified", errors)
    require_bool_true(payload, "dedupe_by_digest_enabled", errors)
    require_bool_true(payload, "quarantine_invalid_blocks", errors)
    require_positive_int(payload, "source_count", errors)
    require_string_coverage(payload, "payload_kinds", "", REQUIRED_PAYLOAD_KINDS, errors)
    require_only_required_payload_kinds(payload, errors)
    require_string_inventory_count_match(
        payload,
        "payload_kinds",
        "source_count",
        errors,
    )
    require_false(payload, "payload_bytes_included", errors)


def validate_publisher_service(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_bool_true(payload, "dag_builder_daemonized", errors)
    require_string_equal(payload, "kubo_unixfs_profile", KUBO_UNIXFS_PROFILE, errors)
    require_exact_positive_int(
        payload,
        "unixfs_chunk_size_bytes",
        KUBO_UNIXFS_CHUNK_SIZE_BYTES,
        errors,
    )
    require_bool_true(payload, "unixfs_raw_leaves", errors)
    require_bool_true(payload, "unixfs_balanced_layout", errors)
    require_exact_positive_int(
        payload,
        "unixfs_max_links_per_node",
        KUBO_UNIXFS_MAX_LINKS_PER_NODE,
        errors,
    )
    require_exact_positive_int(payload, "cid_version", KUBO_CID_VERSION, errors)
    require_string_equal(payload, "cid_multihash", KUBO_CID_MULTIHASH, errors)
    require_bool_true(payload, "locally_derived_cids_verified", errors)
    require_bool_true(payload, "signed_http_head_cas_enabled", errors)
    require_bool_true(payload, "strong_single_etag_verified", errors)
    require_bool_true(payload, "conditional_cas_readback_verified", errors)
    require_bool_true(payload, "signed_head_verified", errors)
    require_bool_true(payload, "parent_chain_verified", errors)
    require_bool_true(payload, "objects_pinned", errors)
    require_bool_true(payload, "authenticated_ingress_qualified", errors)
    require_string_equal(payload, "ingress_enforcement", INGRESS_ENFORCEMENT, errors)
    require_string_equal(payload, "replay_posture", REPLAY_POSTURE, errors)
    require_bool_true(payload, "ingress_scope_binding_verified", errors)
    for field in INGRESS_QUALIFICATION_DIGEST_FIELDS + INGRESS_BINDING_DIGEST_FIELDS:
        require_nonzero_digest(payload, field, errors)
    require_hex(payload, "public_head_cid_hex", HEX64_LEN, errors)
    require_nonzero_digest(payload, "policy_digest_hex", errors)
    require_maximum_int(payload, "pin_lag_seconds", options.max_pin_lag_secs, errors)
    require_maximum_int(payload, "head_age_seconds", options.max_head_age_secs, errors)
    require_minimum_int(payload, "block_count", options.min_blocks, errors)
    require_string_inventory_count_match(
        payload,
        "block_refs",
        "block_count",
        errors,
    )
    require_scalar_inventory_labels(
        payload,
        "block_refs",
        errors,
        pattern=BLOCK_REF_LABEL_PATTERN,
        label_error=BLOCK_REF_LABEL_ERROR,
    )
    require_minimum_int(
        payload,
        "payload_kind_count",
        options.min_payload_kinds,
        errors,
    )
    require_string_coverage(payload, "payload_kinds", "", REQUIRED_PAYLOAD_KINDS, errors)
    require_only_required_payload_kinds(payload, errors)
    require_string_inventory_count_match(
        payload,
        "payload_kinds",
        "payload_kind_count",
        errors,
    )
    require_false(payload, "raw_head_included", errors)


def validate_mirror_datastore(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "public_head_cid_hex", HEX64_LEN, errors)
    require_bool_true(payload, "sealed_typed_store_enabled", errors)
    require_bool_true(payload, "query_service_enabled", errors)
    require_bool_true(payload, "mirror_index_verified", errors)
    require_bool_true(payload, "head_lookup_verified", errors)
    require_bool_true(payload, "block_lookup_verified", errors)
    require_bool_true(payload, "node_lookup_verified", errors)
    require_bool_true(payload, "digest_lookup_verified", errors)
    require_exact_positive_int(
        payload,
        "retention_max_entries",
        MIRROR_RETENTION_MAX_ENTRIES,
        errors,
    )
    require_exact_positive_int(
        payload,
        "retention_max_bytes",
        MIRROR_RETENTION_MAX_BYTES,
        errors,
    )
    require_bool_true(payload, "exact_retained_source_suffix_verified", errors)
    require_bool_true(payload, "fresh_checkpoint_coherent_reads_verified", errors)
    require_bool_true(payload, "liveness_bound_reader_verified", errors)
    require_false(payload, "mirror_drift_detected", errors)
    require_zero_count(payload, "missing_block_count", errors)
    require_false(payload, "raw_blocks_included", errors)


def validate_operator_recovery(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "public_head_cid_hex", HEX64_LEN, errors)
    require_bool_true(payload, "live_head_fetch_verified", errors)
    require_bool_true(payload, "public_checkpoint_published", errors)
    require_bool_true(payload, "checkpoint_recovery_verified", errors)
    require_bool_true(payload, "derived_mirror_recovery_verified", errors)
    require_bool_true(payload, "recovered_head_matches_public_head", errors)
    require_bool_true(payload, "post_loss_repair_verified", errors)
    require_bool_true(payload, "head_object_repaired_with_same_cid", errors)
    require_bool_true(payload, "block_object_repaired_with_same_cid", errors)
    require_bool_true(payload, "public_head_unchanged_during_repair", errors)
    require_hex(payload, "checkpoint_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_checkpoint_included", errors)


def validate_dashboard_api(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_hex(payload, "public_head_cid_hex", HEX64_LEN, errors)
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_DASHBOARD_ROUTES, errors)
    require_only_required_values(payload, "routes", "name", REQUIRED_DASHBOARD_ROUTES, errors)
    require_string_inventory_count_match(
        payload,
        "routes",
        "route_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    require_bool_true(payload, "service_mirror_capability_installed", errors)
    require_bool_true(payload, "fresh_checkpoint_coherent_reads_verified", errors)
    require_bool_true(payload, "liveness_bound_reader_verified", errors)
    require_bool_true(payload, "unready_reader_rejected", errors)
    require_bool_true(payload, "reader_withdrawal_verified", errors)
    require_false(payload, "response_bodies_included", errors)
    validate_routes(payload, errors, options)


def validate_observability(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "public_head_cid_hex", HEX64_LEN, errors)
    require_bool_true(payload, "metrics_scrape_success", errors)
    require_bool_true(payload, "dashboard_provisioned", errors)
    require_bool_true(payload, "alert_rules_installed", errors)
    require_bool_true(payload, "publication_metrics_present", errors)
    require_bool_true(payload, "first_full_audit_verified", errors)
    require_bool_true(payload, "readiness_withheld_until_full_audit", errors)
    require_bool_true(payload, "bounded_rotating_audit_verified", errors)
    require_exact_positive_int(
        payload,
        "audit_max_entries_per_poll",
        STEADY_AUDIT_MAX_ENTRIES_PER_POLL,
        errors,
    )
    require_exact_positive_int(
        payload,
        "audit_max_bytes_per_poll",
        STEADY_AUDIT_MAX_BYTES_PER_POLL,
        errors,
    )
    require_false(payload, "critical_alerts_firing", errors)
    require_string_coverage(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_only_required_values(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_positive_int(payload, "metric_count", errors)
    require_string_inventory_count_match(payload, "metrics", "metric_count", errors)
    require_false(payload, "response_bodies_included", errors)


def validate_publication_e2e(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_hex(payload, "public_head_cid_hex", HEX64_LEN, errors)
    require_bool_true(payload, "local_kubo_tests_passed", errors)
    require_bool_true(payload, "deterministic_unixfs_profile_verified", errors)
    require_bool_true(payload, "signed_http_head_resolved", errors)
    require_bool_true(payload, "strong_single_etag_cas_verified", errors)
    require_bool_true(payload, "authenticated_ingress_qualification_verified", errors)
    require_bool_true(payload, "replay_attack_rejected", errors)
    require_bool_true(payload, "block_replay_verified", errors)
    require_bool_true(payload, "duplicate_payload_rejected", errors)
    require_bool_true(payload, "invalid_parent_quarantined", errors)
    require_bool_true(payload, "post_loss_same_cid_repair_verified", errors)
    require_bool_true(payload, "bounded_rotating_audit_verified", errors)
    require_bool_true(payload, "fresh_torii_reads_verified", errors)
    require_bool_true(payload, "stopped_service_reads_rejected", errors)
    require_bool_true(payload, "publisher_key_failure_tested", errors)
    require_minimum_int(payload, "block_count", options.min_blocks, errors)
    require_string_inventory_count_match(
        payload,
        "block_refs",
        "block_count",
        errors,
    )
    require_scalar_inventory_labels(
        payload,
        "block_refs",
        errors,
        pattern=BLOCK_REF_LABEL_PATTERN,
        label_error=BLOCK_REF_LABEL_ERROR,
    )
    require_minimum_int(
        payload,
        "payload_kind_count",
        options.min_payload_kinds,
        errors,
    )
    require_string_coverage(payload, "payload_kinds", "", REQUIRED_PAYLOAD_KINDS, errors)
    require_only_required_payload_kinds(payload, errors)
    require_string_inventory_count_match(
        payload,
        "payload_kinds",
        "payload_kind_count",
        errors,
    )
    require_false(payload, "raw_blocks_included", errors)


def validate_governance_approval(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "public_head_cid_hex", HEX64_LEN, errors)
    require_config_backed_governance_approval(payload, errors)
    require_bool_true(payload, "publisher_keys_governed", errors)
    require_bool_true(payload, "signed_http_head_endpoint_governed", errors)
    require_bool_true(payload, "ingress_receiver_policy_governed", errors)
    require_bool_true(payload, "replay_namespace_governed", errors)
    require_bool_true(payload, "fixed_retention_contract_bound", errors)
    for field in INGRESS_QUALIFICATION_DIGEST_FIELDS + INGRESS_BINDING_DIGEST_FIELDS:
        require_nonzero_digest(payload, field, errors)
    require_exact_positive_int(
        payload,
        "retention_max_entries",
        MIRROR_RETENTION_MAX_ENTRIES,
        errors,
    )
    require_exact_positive_int(
        payload,
        "retention_max_bytes",
        MIRROR_RETENTION_MAX_BYTES,
        errors,
    )
    policy_digest = require_policy_digest(payload, errors)
    if policy_digest == "0" * HEX64_LEN:
        errors.append("policy_digest_hex must not be the zero digest")


def validate_kind_specific(
    kind: EvidenceKind,
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_exact_fields(
        payload,
        EVIDENCE_REQUIRED_FIELDS[kind.name],
        errors,
        path=f"{kind.name} payload",
    )
    require_passed_status(payload, errors)
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_evidence_age_secs,
    )

    if kind.name == "ingest_service":
        validate_ingest_service(payload, errors)
    elif kind.name == "publisher_service":
        validate_publisher_service(payload, errors, options)
    elif kind.name == "mirror_datastore":
        validate_mirror_datastore(payload, errors)
    elif kind.name == "operator_recovery":
        validate_operator_recovery(payload, errors)
    elif kind.name == "dashboard_api":
        validate_dashboard_api(payload, errors, options)
    elif kind.name == "observability":
        validate_observability(payload, errors)
    elif kind.name == "publication_e2e":
        validate_publication_e2e(payload, errors, options)
    elif kind.name == "governance_approval":
        validate_governance_approval(payload, errors)


def validate_evidence_payload(
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[str | None, list[str]]:
    return validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS SF-12 rollout artifact",
        SENSITIVE_KEYS,
        "rollout evidence",
        lambda kind, checked_payload, errors: validate_kind_specific(
            kind, checked_payload, errors, options
        ),
        require_reviewed_deployment_context=True,
    )


def require_single_active_digest(
    digests: set[str],
    errors: list[str],
    *,
    label: str,
) -> set[str]:
    """Return one active rollout digest or fail closed on mixed anchors."""

    if len(digests) <= 1:
        return digests
    errors.append(f"{label} must contain exactly one active digest")
    return set()



def build_summary(
    evidence_dirs: list[Path],
    evidence_files: list[Path],
    required_kinds: tuple[str, ...],
    options: ValidationOptions,
    summary_out: Path | None,
) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []


    artifacts_by_kind = init_evidence_artifact_buckets(DEFAULT_REQUIRED_KINDS)
    valid_public_head_cids: set[str] = set()
    public_head_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_policy_digests: set[str] = set()
    policy_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_receiver_policy_digests: set[str] = set()
    valid_replay_namespace_digests: set[str] = set()
    valid_replica_set_digests: set[str] = set()
    valid_kubo_ingress_binding_digests: set[str] = set()
    valid_signed_head_ingress_binding_digests: set[str] = set()
    ingress_qualification_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_checkpoint_digests: set[str] = set()
    metric_counts: set[int] = set()
    metric_names: set[str] = set()
    files = discover_evidence_files(
        evidence_dirs,
        evidence_files,
        errors,
        reserved_output_paths=() if summary_out is None else (summary_out,),
    )
    explicit = evidence_path_identities(evidence_files, errors)

    for path in files:
        loaded = load_evidence_json_with_sha256_or_record_error(
            path, MAX_EVIDENCE_BYTES, errors
        )
        if loaded is None:
            continue
        payload, digest = loaded
        kind_name, validation_errors = validate_evidence_payload(payload, options)
        if kind_name is None:
            record_explicit_evidence_validation_errors(
                path, explicit, validation_errors, errors
            )
            continue
        artifact = build_evidence_artifact(
            archive_artifact_path_label(path, evidence_dirs),
            digest,
            payload,
            validation_errors,
            FINGERPRINT_FIELDS,
        )
        if kind_name == "observability":
            record_observed_evidence_value(metric_counts, payload.get("metric_count"))
            metric_names.update(hashable_evidence_values(payload.get("metrics")))
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact, errors)
        if evidence_artifact_is_valid(artifact):
            fingerprint = evidence_artifact_fingerprint(artifact)
            digest = fingerprint.get("public_head_cid_hex")
            if kind_name == "publisher_service":
                if isinstance(digest, str):
                    valid_public_head_cids.add(digest)
                policy_digest = fingerprint.get("policy_digest_hex")
                if isinstance(policy_digest, str):
                    valid_policy_digests.add(policy_digest)
                receiver_policy_digest = fingerprint.get("receiver_policy_digest_hex")
                if isinstance(receiver_policy_digest, str):
                    valid_receiver_policy_digests.add(receiver_policy_digest)
                replay_namespace_digest = fingerprint.get("replay_namespace_digest_hex")
                if isinstance(replay_namespace_digest, str):
                    valid_replay_namespace_digests.add(replay_namespace_digest)
                replica_set_digest = fingerprint.get("replica_set_digest_hex")
                if isinstance(replica_set_digest, str):
                    valid_replica_set_digests.add(replica_set_digest)
                kubo_ingress_binding_digest = fingerprint.get(
                    "kubo_ingress_binding_digest_hex"
                )
                if isinstance(kubo_ingress_binding_digest, str):
                    valid_kubo_ingress_binding_digests.add(kubo_ingress_binding_digest)
                signed_head_ingress_binding_digest = fingerprint.get(
                    "signed_head_ingress_binding_digest_hex"
                )
                if isinstance(signed_head_ingress_binding_digest, str):
                    valid_signed_head_ingress_binding_digests.add(
                        signed_head_ingress_binding_digest
                    )
            if kind_name == "operator_recovery":
                checkpoint_digest = fingerprint.get("checkpoint_digest_hex")
                if isinstance(checkpoint_digest, str):
                    valid_checkpoint_digests.add(checkpoint_digest)
            if kind_name in PUBLIC_HEAD_BOUND_KINDS:
                public_head_bound_artifacts.append((kind_name, artifact))
            if kind_name in POLICY_BOUND_KINDS:
                policy_bound_artifacts.append((kind_name, artifact))
            if kind_name in INGRESS_QUALIFICATION_BOUND_KINDS:
                ingress_qualification_bound_artifacts.append((kind_name, artifact))
        record_evidence_validation_errors(path, validation_errors, errors)

    valid_public_head_cids = require_single_active_digest(
        valid_public_head_cids,
        errors,
        label="valid_public_head_cids",
    )
    valid_policy_digests = require_single_active_digest(
        valid_policy_digests,
        errors,
        label="valid_policy_digests",
    )
    valid_checkpoint_digests = require_single_active_digest(
        valid_checkpoint_digests,
        errors,
        label="valid_checkpoint_digests",
    )
    valid_receiver_policy_digests = require_single_active_digest(
        valid_receiver_policy_digests,
        errors,
        label="valid_receiver_policy_digests",
    )
    valid_replay_namespace_digests = require_single_active_digest(
        valid_replay_namespace_digests,
        errors,
        label="valid_replay_namespace_digests",
    )
    valid_replica_set_digests = require_single_active_digest(
        valid_replica_set_digests,
        errors,
        label="valid_replica_set_digests",
    )
    valid_kubo_ingress_binding_digests = require_single_active_digest(
        valid_kubo_ingress_binding_digests,
        errors,
        label="valid_kubo_ingress_binding_digests",
    )
    valid_signed_head_ingress_binding_digests = require_single_active_digest(
        valid_signed_head_ingress_binding_digests,
        errors,
        label="valid_signed_head_ingress_binding_digests",
    )

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=DEFAULT_REQUIRED_KINDS,
        bound_artifacts=public_head_bound_artifacts,
        valid_anchor_digests=valid_public_head_cids,
        digest_field="public_head_cid_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} public_head_cid_hex must match a valid "
            "publisher_service public_head_cid_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} public_head_cid_hex requires a valid "
            "publisher_service public_head_cid_hex"
        ),
    )

    for digest_field, valid_digests in (
        ("receiver_policy_digest_hex", valid_receiver_policy_digests),
        ("replay_namespace_digest_hex", valid_replay_namespace_digests),
        ("replica_set_digest_hex", valid_replica_set_digests),
        ("kubo_ingress_binding_digest_hex", valid_kubo_ingress_binding_digests),
        (
            "signed_head_ingress_binding_digest_hex",
            valid_signed_head_ingress_binding_digests,
        ),
    ):
        validate_bound_evidence_digest_references(
            required_kinds=required_kinds,
            missing_anchor_required_kinds=DEFAULT_REQUIRED_KINDS,
            bound_artifacts=ingress_qualification_bound_artifacts,
            valid_anchor_digests=valid_digests,
            digest_field=digest_field,
            errors=errors,
            binding_error_template=(
                f"{{kind_name}} {digest_field} must match a valid "
                f"publisher_service {digest_field}"
            ),
            missing_anchor_error_template=(
                f"{{kind_name}} {digest_field} requires a valid "
                f"publisher_service {digest_field}"
            ),
        )

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=DEFAULT_REQUIRED_KINDS,
        bound_artifacts=policy_bound_artifacts,
        valid_anchor_digests=valid_policy_digests,
        digest_field="policy_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} policy_digest_hex must match a valid "
            "publisher_service policy_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} policy_digest_hex requires a valid "
            "publisher_service policy_digest_hex"
        ),
    )

    required = build_required_evidence_summary(
        required_kinds,
        artifacts_by_kind,
        evidence_schema_by_kind(KIND_BY_NAME),
        errors,
        evidence_label="rollout",
    )

    summary = {
        "schema": SUMMARY_SCHEMA,
        "status": evidence_gate_status(errors),
        "required_kinds": required_evidence_kind_names(required_kinds),
        "thresholds": {
            "max_evidence_age_secs": options.max_evidence_age_secs,
            "max_route_latency_ms": options.max_route_latency_ms,
            "max_pin_lag_secs": options.max_pin_lag_secs,
            "max_head_age_secs": options.max_head_age_secs,
            "min_blocks": options.min_blocks,
            "min_payload_kinds": options.min_payload_kinds,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "recognized_artifacts": recognized_evidence_artifacts(artifacts_by_kind),
        "valid_checkpoint_digests": sorted(valid_checkpoint_digests),
        "valid_public_head_cids": sorted(valid_public_head_cids),
        "valid_policy_digests": sorted(valid_policy_digests),
        "valid_receiver_policy_digests": sorted(valid_receiver_policy_digests),
        "valid_replay_namespace_digests": sorted(valid_replay_namespace_digests),
        "valid_replica_set_digests": sorted(valid_replica_set_digests),
        "valid_kubo_ingress_binding_digests": sorted(
            valid_kubo_ingress_binding_digests
        ),
        "valid_signed_head_ingress_binding_digests": sorted(
            valid_signed_head_ingress_binding_digests
        ),
        "metrics": sorted(metric_names),
        "metric_count_values": sorted(metric_counts),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS SF-12 Governance DAG rollout evidence artifacts."
    )
    add_topology_qualification_argument(parser)
    parser.add_argument(
        "--evidence-dir",
        action="append",
        type=Path,
        default=[],
        help="Directory containing rollout evidence JSON artifacts.",
    )
    parser.add_argument(
        "--evidence",
        action="append",
        type=Path,
        default=[],
        help="Explicit rollout evidence JSON artifact.",
    )
    parser.add_argument(
        "--require-kind",
        action="append",
        default=[],
        help="Required evidence kind, or comma-separated kinds. Defaults to all SF-12 kinds.",
    )
    parser.add_argument("--summary-out", type=Path, help="Optional summary JSON output path.")
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        required=True,
        help="Required reviewed validator clock used for age checks.",
    )
    parser.add_argument(
        "--max-evidence-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_EVIDENCE_AGE_SECS,
    )
    parser.add_argument(
        "--max-route-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_ROUTE_LATENCY_MS,
    )
    parser.add_argument(
        "--max-pin-lag-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_PIN_LAG_SECS,
    )
    parser.add_argument(
        "--max-head-age-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_HEAD_AGE_SECS,
    )
    parser.add_argument("--min-blocks", type=positive_int_arg, default=DEFAULT_MIN_BLOCKS)
    parser.add_argument(
        "--min-payload-kinds",
        type=positive_int_arg,
        default=DEFAULT_MIN_PAYLOAD_KINDS,
    )
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_checker_exception(error)
        return 2
    try:
        args = parser.parse_args(expanded_args)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    try:
        required_kinds = parse_required_evidence_kinds(
            args.require_kind,
            allowed_kinds=KIND_BY_NAME,
            default_required=DEFAULT_REQUIRED_KINDS,
        )
    except ValueError as error:
        emit_checker_exception(error)
        return 2

    options = ValidationOptions(
        now_unix=args.now_unix,
        max_evidence_age_secs=args.max_evidence_age_secs,
        max_route_latency_ms=args.max_route_latency_ms,
        max_pin_lag_secs=args.max_pin_lag_secs,
        max_head_age_secs=args.max_head_age_secs,
        min_blocks=args.min_blocks,
        min_payload_kinds=args.min_payload_kinds,
    )
    preflight_errors = validate_checker_preflight(args)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2

    summary, errors = build_summary(
        args.evidence_dir, args.evidence, required_kinds, options, args.summary_out
    )
    errors.extend(
        bind_lane_summary_to_topology(
            summary, args.topology_qualification_summary
        )
    )
    summary["status"] = evidence_gate_status(errors)
    rendered_summary, summary_errors = render_and_write_checker_summary(
        args.summary_out, summary
    )
    if summary_errors:
        emit_checker_error_lines(summary_errors)
        return 2

    if errors:
        emit_checker_error_block("ERROR: SoraFS Governance DAG rollout evidence is incomplete:", errors)
        return 1

    emit_checker_notice(
        "SoraFS Governance DAG rollout evidence is ready: "
        f"{summary['recognized_artifact_count']} recognized artifact(s) cover "
        f"{len(required_kinds)} required kind(s).",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
