#!/usr/bin/env python3
"""Validate SoraFS gateway compliance rollout evidence artifacts."""

from __future__ import annotations

import argparse
import re
import sys
import time
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
    EVIDENCE_URL_FIELD_ERROR,
    archive_artifact_path_label,
    build_evidence_artifact,
    count_evidence_artifacts,
    recognized_evidence_artifacts,
    count_evidence_files,
    evidence_gate_status,
    evidence_artifact_is_valid,
    evidence_artifact_fingerprint,
    evidence_schema_by_kind,
    forbidden_non_production_markers,
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
    require_iroha_config_binding,
    validate_standard_evidence_payload,
    require_maximum_int,
    require_minimum_int,
    require_object,
    require_object_array,
    required_evidence_kind_names,
    require_passed_status,
    require_policy_digest,
    require_positive_int,
    require_recent_timestamp,
    require_safe_url,
    require_string,
    require_string_coverage,
    require_string_equal,
    require_string_inventory_count_match,
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


SUMMARY_SCHEMA = "sorafs.gateway_compliance.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_EVIDENCE_AGE_SECS = 24 * 60 * 60
DEFAULT_MAX_ROUTE_LATENCY_MS = 1_500
DEFAULT_MAX_RELOAD_LATENCY_MS = 300_000
DEFAULT_MIN_GATEWAYS = 3
DEFAULT_MIN_DENYLIST_ENTRIES = 5
DEFAULT_MIN_HONEY_PROBES = 4
HEX64_LEN = 64
CONTROLLER_INSTANCE_ID_PATTERN = re.compile(
    r"^gateway-compliance-controller-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
CONTROLLER_INSTANCE_ID_ERROR = (
    "controller_instance_id must match canonical lowercase "
    "`gateway-compliance-controller-name`"
)
GATEWAY_LABEL_PATTERN = re.compile(
    r"^gateway-compliance-gateway-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
GATEWAY_LABEL_ERROR = (
    "gateways[].name must match canonical lowercase "
    "`gateway-compliance-gateway-name`"
)
DENYLIST_ENTRY_LABEL_PATTERN = re.compile(
    r"^gateway-denylist-entry-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
DENYLIST_ENTRY_LABEL_ERROR = (
    "denylist_entries[].name must match canonical lowercase "
    "`gateway-denylist-entry-name`"
)
HONEY_PROBE_LABEL_PATTERN = re.compile(
    r"^gateway-honey-probe-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
HONEY_PROBE_LABEL_ERROR = (
    "probes[].name must match canonical lowercase `gateway-honey-probe-*`"
)
FORBIDDEN_CONTROLLER_INSTANCE_ID_MARKERS = frozenset(
    (
        "debug",
        "dev",
        "draft",
        "example",
        "fake",
        "latest",
        "placeholder",
        "sample",
        "secret",
        "test",
        "todo",
    )
)
FORBIDDEN_INVENTORY_LABEL_MARKERS = frozenset(
    (
        "debug",
        "dev",
        "draft",
        "example",
        "fake",
        "latest",
        "placeholder",
        "sample",
        "secret",
        "test",
        "todo",
    )
)

REQUIRED_DENIAL_REASONS = (
    "provider",
    "manifest_digest",
    "cid",
    "url",
    "account_id",
    "account_alias",
    "perceptual_family",
    "gar_ttl",
    "legal_hold",
)
REQUIRED_CONTROLLER_FEEDS = (
    "ofac",
    "eu-sanctions",
    "malware",
    "csam-hash",
    "legal-hold",
    "regional-blocklist",
    "appeal-overrides",
)
REQUIRED_MODERATION_TOGGLES = (
    "provider-deny",
    "appeal-override",
    "legal-hold",
    "regional-emergency",
)
REQUIRED_ENFORCEMENT_ROUTES = (
    "manifest",
    "cid",
    "provider",
)
REQUIRED_METRICS = (
    "sorafs_gateway_policy_denials_total",
    "sorafs_gateway_denylist_reload_total",
    "sorafs_gateway_denylist_reload_latency_ms",
    "sorafs_gateway_honey_audit_failures_total",
    "sorafs_gateway_compliance_override_total",
    "sorafs_transparency_source_entries_total",
)
BUNDLE_BOUND_KINDS = (
    "controller_runtime",
    "moderation_toggle",
    "gateway_reload",
    "enforcement_probe",
    "honey_audit",
    "appeal_override",
    "transparency_publication",
    "observability",
    "governance_approval",
)
POLICY_BOUND_KINDS = ("governance_approval",)

SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "feed_payload",
    "feed_payloads",
    "honey_response",
    "honey_responses",
    "message_body",
    "mnemonic",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "raw_appeal_payload",
    "raw_catalog",
    "raw_evidence",
    "raw_feed",
    "raw_feeds",
    "raw_probe_response",
    "raw_probe_responses",
    "raw_receipt",
    "raw_receipts",
    "request_body",
    "response_body",
    "response_bodies",
    "secret",
    "signed_transaction",
    "token",
}


@dataclass(frozen=True)
class EvidenceKind:
    """One SFM-4 gateway compliance rollout evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("feed_promotion", "sorafs.gateway_compliance.feed_promotion_canary.v1"),
    EvidenceKind(
        "controller_runtime",
        "sorafs.gateway_compliance.controller_runtime_canary.v1",
    ),
    EvidenceKind(
        "moderation_toggle",
        "sorafs.gateway_compliance.moderation_toggle_canary.v1",
    ),
    EvidenceKind("gateway_reload", "sorafs.gateway_compliance.gateway_reload_canary.v1"),
    EvidenceKind("enforcement_probe", "sorafs.gateway_compliance.enforcement_probe_canary.v1"),
    EvidenceKind("honey_audit", "sorafs.gateway_compliance.honey_audit_canary.v1"),
    EvidenceKind("appeal_override", "sorafs.gateway_compliance.appeal_override_canary.v1"),
    EvidenceKind(
        "transparency_publication",
        "sorafs.gateway_compliance.transparency_publication_canary.v1",
    ),
    EvidenceKind("observability", "sorafs.gateway_compliance.observability_canary.v1"),
    EvidenceKind("governance_approval", "sorafs.gateway_compliance.governance_approval.v1"),
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
    "feed_promotion": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "external_feeds_normalized",
        "feed_signature_verified",
        "bundle_pack_verified",
        "bundle_diff_reviewed",
        "merkle_root_bound",
        "update_history_persisted",
        "gateway_ack_count",
        "gateways",
        "denylist_entry_count",
        "denylist_entries",
        "bundle_digest_hex",
        "policy_digest_hex",
        "raw_feeds_included",
        "feed_payloads_included",
    ),
    "controller_runtime": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "bundle_digest_hex",
        "controller_instance_id",
        "iroha_config_bound",
        "config_source",
        "external_feed_count",
        "fetched_feed_count",
        "normalized_feed_count",
        "signed_feed_count",
        "feeds",
        "controller_service_enabled",
        "scheduler_config_bound",
        "external_feeds_fetched",
        "feed_signature_verified",
        "normalization_deterministic",
        "bundle_pack_verified",
        "update_history_persisted",
        "gateway_reload_requested",
        "failure_backoff_configured",
        "rollback_plan_verified",
        "raw_feeds_included",
        "feed_payloads_included",
        "response_bodies_included",
    ),
    "moderation_toggle": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "bundle_digest_hex",
        "toggle_api_url",
        "toggle_count",
        "approved_toggle_count",
        "toggles",
        "toggle_digest_hex",
        "iroha_config_bound",
        "config_source",
        "operator_role_enforced",
        "approval_workflow_verified",
        "expiry_enforced",
        "cache_invalidation_verified",
        "operator_audit_trail_persisted",
        "rollback_verified",
        "raw_toggle_payloads_included",
        "response_bodies_included",
    ),
    "gateway_reload": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "bundle_digest_hex",
        "reload_ack_count",
        "gateways",
        "max_reload_latency_ms",
        "hot_reload_verified",
        "cache_version_bound",
        "denylist_catalog_readback_verified",
        "persistence_path_configured",
        "stale_bundle_rejected",
        "rollback_plan_verified",
        "raw_catalog_included",
    ),
    "enforcement_probe": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "bundle_digest_hex",
        "denial_reasons_observed",
        "denial_reason_count",
        "structured_error_labels_verified",
        "telemetry_labels_stable",
        "fail_closed_missing_envelope",
        "fail_closed_unadmitted_provider",
        "rate_limit_verified",
        "geofence_verified",
        "proof_token_required",
        "response_bodies_included",
        "route_count",
        "passed_route_count",
        "routes",
    ),
    "honey_audit": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "bundle_digest_hex",
        "honey_probe_count",
        "probes",
        "denied_response_verified",
        "cache_version_binding_verified",
        "proof_token_verified",
        "json_report_generated",
        "markdown_report_generated",
        "audit_digest_hex",
        "raw_probe_responses_included",
    ),
    "appeal_override": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "bundle_digest_hex",
        "appeal_outcome_consumed",
        "policy_override_signed",
        "cache_invalidation_verified",
        "override_expiry_enforced",
        "operator_audit_trail_persisted",
        "denylist_override_scoped",
        "override_digest_hex",
        "raw_appeal_payload_included",
    ),
    "transparency_publication": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "bundle_digest_hex",
        "gar_receipts_published",
        "proof_token_index_published",
        "moderation_events_published",
        "legal_hold_redaction_summaries_published",
        "governance_dag_bound",
        "transparency_cycle_verified",
        "publication_digest_hex",
        "raw_receipts_included",
    ),
    "observability": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "bundle_digest_hex",
        "metrics_scrape_success",
        "dashboard_provisioned",
        "alert_rules_installed",
        "critical_alerts_firing",
        "metrics",
        "metric_count",
        "response_bodies_included",
    ),
    "governance_approval": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "bundle_digest_hex",
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "config_source",
        "compliance_policy_bound",
        "denylist_feed_roster_bound",
        "transparency_policy_bound",
        "operator_roles_bound",
        "retention_policy_bound",
        "policy_digest_hex",
    ),
}


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SFM-4 gateway compliance rollout gate."""

    now_unix: int
    max_evidence_age_secs: int
    max_route_latency_ms: int
    max_reload_latency_ms: int
    min_gateways: int
    min_denylist_entries: int
    min_honey_probes: int



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "schema",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
    "bundle_digest_hex",
    "policy_digest_hex",
    "metric_count",
    "metrics",
)


def validate_routes(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_minimum_int(
        payload,
        "route_count",
        len(REQUIRED_ENFORCEMENT_ROUTES),
        errors,
    )
    require_string_inventory_count_match(
        payload,
        "routes",
        "route_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    require_string_coverage(
        payload,
        "routes",
        "name",
        REQUIRED_ENFORCEMENT_ROUTES,
        errors,
        allow_scalar_items=False,
    )
    require_only_required_values(
        payload,
        "routes",
        "name",
        REQUIRED_ENFORCEMENT_ROUTES,
        errors,
    )
    for index, record in require_object_array(payload, "routes", errors):
        require_string(record, "name", errors)
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
        require_bool_true(
            record,
            "authz_enforced",
            errors,
            path=f"routes[{index}].authz_enforced",
        )
        require_maximum_int(
            record,
            "latency_ms",
            options.max_route_latency_ms,
            errors,
            path=f"routes[{index}].latency_ms",
        )


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


def require_controller_instance_id(
    payload: dict[str, Any], errors: list[str]
) -> str:
    """Require a reviewed lowercase gateway compliance controller identifier."""

    controller_instance_id = require_string(payload, "controller_instance_id", errors)
    if not controller_instance_id:
        return ""
    if CONTROLLER_INSTANCE_ID_PATTERN.fullmatch(controller_instance_id) is None:
        errors.append(CONTROLLER_INSTANCE_ID_ERROR)
        return ""
    forbidden = forbidden_non_production_markers(controller_instance_id, FORBIDDEN_CONTROLLER_INSTANCE_ID_MARKERS)
    if forbidden:
        errors.append(
            "controller_instance_id must not contain non-production markers "
            f"{forbidden}"
        )
        return ""
    return controller_instance_id


def require_inventory_label(
    record: dict[str, Any],
    errors: list[str],
    *,
    pattern: re.Pattern[str],
    label_error: str,
    path: str,
) -> str:
    """Require a reviewed lowercase production inventory label."""

    label = require_string(record, "name", errors)
    if not label:
        return ""
    if pattern.fullmatch(label) is None:
        errors.append(label_error)
        return ""
    forbidden = forbidden_non_production_markers(label, FORBIDDEN_INVENTORY_LABEL_MARKERS)
    if forbidden:
        errors.append(f"{path} must not contain non-production markers {forbidden}")
        return ""
    return label


def validate_feed_promotion(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_bool_true(payload, "external_feeds_normalized", errors)
    require_bool_true(payload, "feed_signature_verified", errors)
    require_bool_true(payload, "bundle_pack_verified", errors)
    require_bool_true(payload, "bundle_diff_reviewed", errors)
    require_bool_true(payload, "merkle_root_bound", errors)
    require_bool_true(payload, "update_history_persisted", errors)
    require_minimum_int(payload, "gateway_ack_count", options.min_gateways, errors)
    require_string_inventory_count_match(
        payload,
        "gateways",
        "gateway_ack_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for _, record in require_object_array(payload, "gateways", errors):
        require_inventory_label(
            record,
            errors,
            pattern=GATEWAY_LABEL_PATTERN,
            label_error=GATEWAY_LABEL_ERROR,
            path="gateways[].name",
        )
    require_minimum_int(
        payload,
        "denylist_entry_count",
        options.min_denylist_entries,
        errors,
    )
    require_string_inventory_count_match(
        payload,
        "denylist_entries",
        "denylist_entry_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for _, record in require_object_array(payload, "denylist_entries", errors):
        require_inventory_label(
            record,
            errors,
            pattern=DENYLIST_ENTRY_LABEL_PATTERN,
            label_error=DENYLIST_ENTRY_LABEL_ERROR,
            path="denylist_entries[].name",
        )
    require_hex(payload, "bundle_digest_hex", HEX64_LEN, errors)
    require_policy_digest(payload, errors)
    require_false(payload, "raw_feeds_included", errors)
    require_false(payload, "feed_payloads_included", errors)


def validate_controller_runtime(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "bundle_digest_hex", HEX64_LEN, errors)
    require_controller_instance_id(payload, errors)
    require_iroha_config_binding(payload, errors)
    require_count_equal(payload, "external_feed_count", "fetched_feed_count", errors)
    require_count_equal(payload, "external_feed_count", "normalized_feed_count", errors)
    require_count_equal(payload, "external_feed_count", "signed_feed_count", errors)
    require_minimum_int(
        payload,
        "external_feed_count",
        len(REQUIRED_CONTROLLER_FEEDS),
        errors,
    )
    require_string_inventory_count_match(
        payload,
        "feeds",
        "external_feed_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    require_string_coverage(
        payload,
        "feeds",
        "name",
        REQUIRED_CONTROLLER_FEEDS,
        errors,
        allow_scalar_items=False,
    )
    require_only_required_values(payload, "feeds", "name", REQUIRED_CONTROLLER_FEEDS, errors)
    for _, record in require_object_array(payload, "feeds", errors):
        require_string(record, "name", errors)
    require_bool_true(payload, "controller_service_enabled", errors)
    require_bool_true(payload, "scheduler_config_bound", errors)
    require_bool_true(payload, "external_feeds_fetched", errors)
    require_bool_true(payload, "feed_signature_verified", errors)
    require_bool_true(payload, "normalization_deterministic", errors)
    require_bool_true(payload, "bundle_pack_verified", errors)
    require_bool_true(payload, "update_history_persisted", errors)
    require_bool_true(payload, "gateway_reload_requested", errors)
    require_bool_true(payload, "failure_backoff_configured", errors)
    require_bool_true(payload, "rollback_plan_verified", errors)
    require_false(payload, "raw_feeds_included", errors)
    require_false(payload, "feed_payloads_included", errors)
    require_false(payload, "response_bodies_included", errors)


def validate_moderation_toggle(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "bundle_digest_hex", HEX64_LEN, errors)
    require_safe_url(payload, "toggle_api_url", errors)
    require_count_equal(payload, "toggle_count", "approved_toggle_count", errors)
    require_minimum_int(
        payload,
        "toggle_count",
        len(REQUIRED_MODERATION_TOGGLES),
        errors,
    )
    require_string_inventory_count_match(
        payload,
        "toggles",
        "toggle_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    require_string_coverage(
        payload,
        "toggles",
        "name",
        REQUIRED_MODERATION_TOGGLES,
        errors,
        allow_scalar_items=False,
    )
    require_only_required_values(
        payload,
        "toggles",
        "name",
        REQUIRED_MODERATION_TOGGLES,
        errors,
    )
    for _, record in require_object_array(payload, "toggles", errors):
        require_string(record, "name", errors)
    require_hex(payload, "toggle_digest_hex", HEX64_LEN, errors)
    require_iroha_config_binding(payload, errors)
    require_bool_true(payload, "operator_role_enforced", errors)
    require_bool_true(payload, "approval_workflow_verified", errors)
    require_bool_true(payload, "expiry_enforced", errors)
    require_bool_true(payload, "cache_invalidation_verified", errors)
    require_bool_true(payload, "operator_audit_trail_persisted", errors)
    require_bool_true(payload, "rollback_verified", errors)
    require_false(payload, "raw_toggle_payloads_included", errors)
    require_false(payload, "response_bodies_included", errors)


def validate_gateway_reload(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_hex(payload, "bundle_digest_hex", HEX64_LEN, errors)
    require_minimum_int(payload, "reload_ack_count", options.min_gateways, errors)
    require_string_inventory_count_match(
        payload,
        "gateways",
        "reload_ack_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for _, record in require_object_array(payload, "gateways", errors):
        require_inventory_label(
            record,
            errors,
            pattern=GATEWAY_LABEL_PATTERN,
            label_error=GATEWAY_LABEL_ERROR,
            path="gateways[].name",
        )
    require_maximum_int(
        payload,
        "max_reload_latency_ms",
        options.max_reload_latency_ms,
        errors,
    )
    require_bool_true(payload, "hot_reload_verified", errors)
    require_bool_true(payload, "cache_version_bound", errors)
    require_bool_true(payload, "denylist_catalog_readback_verified", errors)
    require_bool_true(payload, "persistence_path_configured", errors)
    require_bool_true(payload, "stale_bundle_rejected", errors)
    require_bool_true(payload, "rollback_plan_verified", errors)
    require_false(payload, "raw_catalog_included", errors)


def validate_enforcement_probe(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_hex(payload, "bundle_digest_hex", HEX64_LEN, errors)
    require_string_coverage(payload, "denial_reasons_observed", "", REQUIRED_DENIAL_REASONS, errors)
    require_only_required_values(
        payload,
        "denial_reasons_observed",
        "",
        REQUIRED_DENIAL_REASONS,
        errors,
    )
    require_positive_int(payload, "denial_reason_count", errors)
    require_string_inventory_count_match(
        payload,
        "denial_reasons_observed",
        "denial_reason_count",
        errors,
    )
    require_bool_true(payload, "structured_error_labels_verified", errors)
    require_bool_true(payload, "telemetry_labels_stable", errors)
    require_bool_true(payload, "fail_closed_missing_envelope", errors)
    require_bool_true(payload, "fail_closed_unadmitted_provider", errors)
    require_bool_true(payload, "rate_limit_verified", errors)
    require_bool_true(payload, "geofence_verified", errors)
    require_bool_true(payload, "proof_token_required", errors)
    require_false(payload, "response_bodies_included", errors)
    validate_routes(payload, errors, options)


def validate_honey_audit(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_hex(payload, "bundle_digest_hex", HEX64_LEN, errors)
    require_minimum_int(payload, "honey_probe_count", options.min_honey_probes, errors)
    require_string_inventory_count_match(
        payload,
        "probes",
        "honey_probe_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for _, record in require_object_array(payload, "probes", errors):
        require_inventory_label(
            record,
            errors,
            pattern=HONEY_PROBE_LABEL_PATTERN,
            label_error=HONEY_PROBE_LABEL_ERROR,
            path="probes[].name",
        )
    require_bool_true(payload, "denied_response_verified", errors)
    require_bool_true(payload, "cache_version_binding_verified", errors)
    require_bool_true(payload, "proof_token_verified", errors)
    require_bool_true(payload, "json_report_generated", errors)
    require_bool_true(payload, "markdown_report_generated", errors)
    require_hex(payload, "audit_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_probe_responses_included", errors)


def validate_appeal_override(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "bundle_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "appeal_outcome_consumed", errors)
    require_bool_true(payload, "policy_override_signed", errors)
    require_bool_true(payload, "cache_invalidation_verified", errors)
    require_bool_true(payload, "override_expiry_enforced", errors)
    require_bool_true(payload, "operator_audit_trail_persisted", errors)
    require_bool_true(payload, "denylist_override_scoped", errors)
    require_hex(payload, "override_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_appeal_payload_included", errors)


def validate_transparency_publication(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "bundle_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "gar_receipts_published", errors)
    require_bool_true(payload, "proof_token_index_published", errors)
    require_bool_true(payload, "moderation_events_published", errors)
    require_bool_true(payload, "legal_hold_redaction_summaries_published", errors)
    require_bool_true(payload, "governance_dag_bound", errors)
    require_bool_true(payload, "transparency_cycle_verified", errors)
    require_hex(payload, "publication_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_receipts_included", errors)


def validate_observability(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "bundle_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "metrics_scrape_success", errors)
    require_bool_true(payload, "dashboard_provisioned", errors)
    require_bool_true(payload, "alert_rules_installed", errors)
    require_false(payload, "critical_alerts_firing", errors)
    require_string_coverage(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_only_required_values(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_positive_int(payload, "metric_count", errors)
    require_string_inventory_count_match(payload, "metrics", "metric_count", errors)
    require_false(payload, "response_bodies_included", errors)


def validate_governance_approval(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "bundle_digest_hex", HEX64_LEN, errors)
    require_config_backed_governance_approval(payload, errors)
    require_bool_true(payload, "compliance_policy_bound", errors)
    require_bool_true(payload, "denylist_feed_roster_bound", errors)
    require_bool_true(payload, "transparency_policy_bound", errors)
    require_bool_true(payload, "operator_roles_bound", errors)
    require_bool_true(payload, "retention_policy_bound", errors)
    require_policy_digest(payload, errors)


def validate_kind_specific(
    kind: EvidenceKind,
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_passed_status(payload, errors)
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_evidence_age_secs,
    )

    if kind.name == "feed_promotion":
        validate_feed_promotion(payload, errors, options)
    elif kind.name == "controller_runtime":
        validate_controller_runtime(payload, errors)
    elif kind.name == "moderation_toggle":
        validate_moderation_toggle(payload, errors)
    elif kind.name == "gateway_reload":
        validate_gateway_reload(payload, errors, options)
    elif kind.name == "enforcement_probe":
        validate_enforcement_probe(payload, errors, options)
    elif kind.name == "honey_audit":
        validate_honey_audit(payload, errors, options)
    elif kind.name == "appeal_override":
        validate_appeal_override(payload, errors)
    elif kind.name == "transparency_publication":
        validate_transparency_publication(payload, errors)
    elif kind.name == "observability":
        validate_observability(payload, errors)
    elif kind.name == "governance_approval":
        validate_governance_approval(payload, errors)


def validate_evidence_payload(
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[str | None, list[str]]:
    return validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS SFM-4 rollout artifact",
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
    valid_bundle_digests: set[str] = set()
    bundle_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_policy_digests: set[str] = set()
    policy_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
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
            digest = fingerprint.get("bundle_digest_hex")
            if kind_name == "feed_promotion":
                if isinstance(digest, str):
                    valid_bundle_digests.add(digest)
                policy_digest = fingerprint.get("policy_digest_hex")
                if isinstance(policy_digest, str):
                    valid_policy_digests.add(policy_digest)
            if kind_name in BUNDLE_BOUND_KINDS:
                bundle_bound_artifacts.append((kind_name, artifact))
            if kind_name in POLICY_BOUND_KINDS:
                policy_bound_artifacts.append((kind_name, artifact))
        record_evidence_validation_errors(path, validation_errors, errors)

    valid_bundle_digests = require_single_active_digest(
        valid_bundle_digests,
        errors,
        label="valid_bundle_digests",
    )
    valid_policy_digests = require_single_active_digest(
        valid_policy_digests,
        errors,
        label="valid_policy_digests",
    )

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=BUNDLE_BOUND_KINDS,
        bound_artifacts=bundle_bound_artifacts,
        valid_anchor_digests=valid_bundle_digests,
        digest_field="bundle_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} bundle_digest_hex must match a valid "
            "feed_promotion bundle_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} bundle_digest_hex requires a valid feed_promotion "
            "bundle_digest_hex"
        ),
    )

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("feed_promotion",),
        bound_artifacts=policy_bound_artifacts,
        valid_anchor_digests=valid_policy_digests,
        digest_field="policy_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} policy_digest_hex must match a valid "
            "feed_promotion policy_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} policy_digest_hex requires a valid "
            "feed_promotion policy_digest_hex"
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
            "max_reload_latency_ms": options.max_reload_latency_ms,
            "min_gateways": options.min_gateways,
            "min_denylist_entries": options.min_denylist_entries,
            "min_honey_probes": options.min_honey_probes,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "recognized_artifacts": recognized_evidence_artifacts(artifacts_by_kind),
        "valid_bundle_digests": sorted(valid_bundle_digests),
        "valid_policy_digests": sorted(valid_policy_digests),
        "metrics": sorted(metric_names),
        "metric_count_values": sorted(metric_counts),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS SFM-4 gateway compliance rollout evidence artifacts."
    )
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
        help="Required evidence kind, or comma-separated kinds. Defaults to all SFM-4 kinds.",
    )
    parser.add_argument("--summary-out", type=Path, help="Optional summary JSON output path.")
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        default=int(time.time()),
        help="Validator clock used for age checks. Defaults to current Unix time.",
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
        "--max-reload-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_RELOAD_LATENCY_MS,
    )
    parser.add_argument(
        "--min-gateways",
        type=positive_int_arg,
        default=DEFAULT_MIN_GATEWAYS,
    )
    parser.add_argument(
        "--min-denylist-entries",
        type=positive_int_arg,
        default=DEFAULT_MIN_DENYLIST_ENTRIES,
    )
    parser.add_argument(
        "--min-honey-probes",
        type=positive_int_arg,
        default=DEFAULT_MIN_HONEY_PROBES,
    )
    try:
        expanded = expand_response_args(sys.argv[1:] if argv is None else argv, parser)
        try:
            args = parser.parse_args(expanded)
        except SystemExit as error:
            return error.code if isinstance(error.code, int) else 1
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
        max_reload_latency_ms=args.max_reload_latency_ms,
        min_gateways=args.min_gateways,
        min_denylist_entries=args.min_denylist_entries,
        min_honey_probes=args.min_honey_probes,
    )
    preflight_errors = validate_checker_preflight(args)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2

    summary, errors = build_summary(
        args.evidence_dir, args.evidence, required_kinds, options, args.summary_out
    )
    rendered_summary, summary_errors = render_and_write_checker_summary(
        args.summary_out, summary
    )
    if summary_errors:
        emit_checker_error_lines(summary_errors)
        return 2
    if errors:
        emit_checker_error_block(
            "ERROR: SoraFS gateway compliance rollout evidence is incomplete:",
            errors,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
