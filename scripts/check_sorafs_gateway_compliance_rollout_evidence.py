#!/usr/bin/env python3
"""Validate canonical SoraFS gateway-compliance rollout evidence."""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping

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
from sorafs_evidence_json import (  # noqa: E402
    load_evidence_json_with_sha256_or_record_error,
)
from sorafs_evidence_paths import (  # noqa: E402
    discover_evidence_files,
    evidence_path_identities,
)
from sorafs_evidence_validation import (  # noqa: E402
    archive_artifact_path_label,
    build_evidence_artifact,
    build_required_evidence_summary,
    count_evidence_artifacts,
    count_evidence_files,
    evidence_artifact_fingerprint,
    evidence_artifact_is_valid,
    evidence_gate_status,
    evidence_schema_by_kind,
    forbidden_non_production_markers,
    hashable_evidence_values,
    init_evidence_artifact_buckets,
    recognized_evidence_artifacts,
    record_evidence_artifact,
    record_evidence_validation_errors,
    record_explicit_evidence_validation_errors,
    record_observed_evidence_value,
    record_string_tuple_binding_errors,
    require_2xx_status,
    require_bool_true,
    require_config_backed_governance_approval,
    require_false,
    require_hex,
    require_iroha_config_binding,
    require_maximum_int,
    require_minimum_int,
    require_object,
    require_object_array,
    require_passed_status,
    require_policy_digest,
    require_positive_int,
    require_recent_timestamp,
    require_safe_url,
    require_string,
    require_string_coverage,
    require_string_equal,
    require_string_inventory_count_match,
    required_evidence_kind_names,
    validate_bound_evidence_digest_references,
    validate_standard_evidence_payload,
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

SUMMARY_SCHEMA = "sorafs.gateway_compliance.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_EVIDENCE_AGE_SECS = 24 * 60 * 60
DEFAULT_MAX_ROUTE_LATENCY_MS = 1_500
DEFAULT_MAX_RELOAD_LATENCY_MS = 300_000
DEFAULT_MIN_GATEWAYS = 2
DEFAULT_MIN_CATALOG_ENTRIES = 4
DEFAULT_MIN_CATALOG_CHANGES = 1
DEFAULT_MIN_HONEY_PROBES = 4
MAX_GATEWAYS = 8
MAX_CATALOG_ENTRIES = 4_096
MAX_CATALOG_CHANGES = 4_096
MAX_SOURCE_ANCHORS = 64
MAX_HONEY_PROBES = 64
MAX_APPROVAL_SIGNERS = 64
HEX64_LEN = 64

CONTROLLER_INSTANCE_ID_PATTERN = re.compile(
    r"^gateway-compliance-controller-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
GATEWAY_LABEL_PATTERN = re.compile(
    r"^gateway-compliance-gateway-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
CATALOG_ENTRY_LABEL_PATTERN = re.compile(
    r"^gateway-compliance-entry-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
HONEY_PROBE_LABEL_PATTERN = re.compile(
    r"^gateway-compliance-probe-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
SIGNER_LABEL_PATTERN = re.compile(
    r"^gateway-compliance-signer-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
ADMINISTRATION_LABEL_PATTERN = re.compile(
    r"^gateway-administration-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
REGION_LABEL_PATTERN = re.compile(
    r"^gateway-region-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
SOURCE_LABEL_PATTERN = re.compile(
    r"^gateway-compliance-source-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)

FORBIDDEN_INVENTORY_LABEL_MARKERS = frozenset(
    {
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
    }
)
LEGACY_FIELD_FRAGMENTS = (
    "denylist",
    "bundle",
    "proof_token",
    "cache_version_binding",
    "feed",
    "override",
    "report",
    "local_",
)
LEGACY_DENIAL_CODES = frozenset({"denylisted", "policy_denied"})

REQUIRED_DENIAL_SOURCES = ("baseline", "legal_safety_hold")
REQUIRED_CONTROLLER_SOURCES = (
    "gateway-compliance-source-sanctions",
    "gateway-compliance-source-malware",
    "gateway-compliance-source-safety",
    "gateway-compliance-source-legal",
)
REQUIRED_MODERATION_CONTROLS = (
    "provider-deny",
    "accepted-appeal",
    "legal-safety-hold",
    "regional-emergency",
)
REQUIRED_ENFORCEMENT_ROUTES = ("manifest", "cid", "provider")
REQUIRED_HONEY_ATTACKS = (
    "stale_catalog",
    "wrong_predecessor",
    "invalid_signature",
    "split_gateway_catalog",
)
REQUIRED_PRECEDENCE_CASES: Mapping[str, tuple[str, bool]] = {
    "legal_safety_hold_over_accepted_appeal": ("legal_safety_hold", True),
    "accepted_appeal_over_baseline": ("accepted_appeal", False),
    "baseline_without_accepted_appeal": ("baseline", True),
}
REQUIRED_METRICS = (
    "torii_sorafs_gateway_compliance_requests_total",
    "torii_sorafs_gateway_compliance_serving_decisions_total",
    "torii_sorafs_gateway_compliance_failures_total",
    "torii_sorafs_gateway_compliance_serving_catalog_sequence",
    "torii_sorafs_gateway_compliance_serving_catalog_valid_until_seconds",
    "torii_sorafs_gateway_compliance_ready",
)

CATALOG_BOUND_KINDS = (
    "controller_runtime",
    "moderation_toggle",
    "gateway_reload",
    "enforcement_probe",
    "honey_audit",
    "precedence",
    "transparency_publication",
    "observability",
    "governance_approval",
)
PREDECESSOR_BOUND_KINDS = ("controller_runtime", "gateway_reload")
POLICY_BOUND_KINDS = ("governance_approval",)
CATALOG_HISTORY_FINGERPRINT_FIELDS = (
    "catalog_digest_hex",
    "catalog_sequence",
    "predecessor_catalog_digest_hex",
    "predecessor_catalog_sequence",
)

SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "message_body",
    "mnemonic",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "raw_catalog",
    "raw_evidence",
    "raw_probe_response",
    "request_body",
    "response_body",
    "secret",
    "signed_transaction",
    "token",
}


@dataclass(frozen=True)
class EvidenceKind:
    """One canonical gateway-compliance rollout evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind(
        "catalog_promotion",
        "sorafs.gateway_compliance.catalog_promotion_canary.v1",
    ),
    EvidenceKind(
        "controller_runtime",
        "sorafs.gateway_compliance.controller_runtime_canary.v1",
    ),
    EvidenceKind(
        "moderation_toggle",
        "sorafs.gateway_compliance.moderation_toggle_canary.v1",
    ),
    EvidenceKind(
        "gateway_reload",
        "sorafs.gateway_compliance.gateway_reload_canary.v1",
    ),
    EvidenceKind(
        "enforcement_probe",
        "sorafs.gateway_compliance.enforcement_probe_canary.v1",
    ),
    EvidenceKind(
        "honey_audit",
        "sorafs.gateway_compliance.honey_audit_canary.v1",
    ),
    EvidenceKind(
        "precedence",
        "sorafs.gateway_compliance.precedence_canary.v1",
    ),
    EvidenceKind(
        "transparency_publication",
        "sorafs.gateway_compliance.transparency_publication_canary.v1",
    ),
    EvidenceKind(
        "observability",
        "sorafs.gateway_compliance.observability_canary.v1",
    ),
    EvidenceKind(
        "governance_approval",
        "sorafs.gateway_compliance.governance_approval.v1",
    ),
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
    "evidence_scope",
    "catalog_digest_hex",
    "catalog_sequence",
)
EVIDENCE_REQUIRED_FIELDS: dict[str, tuple[str, ...]] = {
    "catalog_promotion": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "predecessor_catalog_digest_hex",
        "predecessor_catalog_sequence",
        "promoted_catalog_digest_hex",
        "catalog_entry_count",
        "catalog_entries",
        "catalog_change_count",
        "catalog_changes",
        "approval_threshold",
        "approval_signer_count",
        "approval_signer_ids",
        "catalog_signatures_verified",
        "gateway_ack_count",
        "gateway_acknowledgements",
        "policy_digest_hex",
    ),
    "controller_runtime": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "predecessor_catalog_digest_hex",
        "predecessor_catalog_sequence",
        "controller_instance_id",
        "iroha_config_bound",
        "config_source",
        "source_anchor_count",
        "source_anchors",
        "controller_service_enabled",
        "catalog_signatures_verified",
        "predecessor_link_verified",
        "durable_history_reconciled",
        "last_known_good_available",
        "atomic_catalog_replacement",
    ),
    "moderation_toggle": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "control_api_url",
        "control_api_status_code",
        "control_count",
        "approved_control_count",
        "controls",
        "control_digest_hex",
        "iroha_config_bound",
        "config_source",
        "operator_role_enforced",
        "approval_workflow_enforced",
        "expiry_enforced",
        "catalog_reconciliation_observed",
        "operator_audit_trail_persisted",
    ),
    "gateway_reload": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "predecessor_catalog_digest_hex",
        "predecessor_catalog_sequence",
        "reload_ack_count",
        "gateway_acknowledgements",
        "max_reload_latency_ms",
        "atomic_catalog_replacement",
        "persisted_catalog_readback",
        "stale_catalog_rejected",
        "predecessor_mismatch_rejected",
        "rollback_catalog_digest_hex",
        "rollback_available",
    ),
    "enforcement_probe": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "fail_closed_missing_catalog",
        "fail_closed_expired_catalog",
        "rate_limit_enforced",
        "route_count",
        "routes",
    ),
    "honey_audit": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "probe_count",
        "probes",
    ),
    "precedence": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "case_count",
        "cases",
        "finalized_chain_projection",
    ),
    "transparency_publication": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "catalog_history_published",
        "catalog_acknowledgements_published",
        "moderation_events_published",
        "legal_hold_redaction_summaries_published",
        "governance_dag_bound",
        "publication_digest_hex",
    ),
    "observability": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "metrics_scrape_success",
        "dashboard_provisioned",
        "alert_rules_installed",
        "critical_alerts_firing",
        "metrics",
        "metric_count",
    ),
    "governance_approval": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "config_source",
        "catalog_policy_bound",
        "catalog_source_roster_bound",
        "transparency_policy_bound",
        "operator_roles_bound",
        "retention_policy_bound",
        "policy_digest_hex",
    ),
}


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the canonical gateway-compliance rollout gate."""

    now_unix: int
    max_evidence_age_secs: int
    max_route_latency_ms: int
    max_reload_latency_ms: int
    min_gateways: int
    min_catalog_entries: int
    min_catalog_changes: int
    min_honey_probes: int


FINGERPRINT_FIELDS: tuple[str, ...] = (
    "schema",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
    "catalog_digest_hex",
    "catalog_sequence",
    "predecessor_catalog_digest_hex",
    "predecessor_catalog_sequence",
    "policy_digest_hex",
    "metric_count",
    "metrics",
)


def require_exact_fields(
    payload: Mapping[str, Any],
    expected: tuple[str, ...] | frozenset[str],
    errors: list[str],
    *,
    path: str,
) -> None:
    """Reject missing and unknown fields in a bounded canonical object."""

    expected_set = frozenset(expected)
    actual = frozenset(payload)
    missing = sorted(expected_set - actual)
    unknown = sorted(actual - expected_set)
    if missing:
        errors.append(f"{path} is missing required fields: {', '.join(missing)}")
    if unknown:
        errors.append(f"{path} contains unknown fields: {', '.join(unknown)}")


def reject_legacy_fields(value: Any, errors: list[str], path: str = "") -> None:
    """Reject every removed local-compliance field at any nesting depth."""

    if isinstance(value, dict):
        for key, nested in value.items():
            key_path = key if not path else f"{path}.{key}"
            normalized = key.replace("-", "_")
            if any(fragment in normalized for fragment in LEGACY_FIELD_FRAGMENTS):
                errors.append(f"{key_path} is a removed gateway-compliance V1 field")
            reject_legacy_fields(nested, errors, key_path)
    elif isinstance(value, list):
        for index, nested in enumerate(value):
            reject_legacy_fields(nested, errors, f"{path}[{index}]")
    elif isinstance(value, str) and value in LEGACY_DENIAL_CODES:
        errors.append(f"{path} uses a removed gateway-compliance denial code")


def require_canonical_label(
    record: Mapping[str, Any],
    field: str,
    pattern: re.Pattern[str],
    errors: list[str],
    *,
    path: str,
) -> str:
    value = require_string(record, field, errors)
    if not value:
        return ""
    if pattern.fullmatch(value) is None:
        errors.append(f"{path} is not a canonical gateway-compliance label")
        return ""
    markers = forbidden_non_production_markers(
        value, FORBIDDEN_INVENTORY_LABEL_MARKERS
    )
    if markers:
        errors.append(f"{path} contains non-production markers {markers}")
        return ""
    return value


def require_nonzero_distinct_predecessor(
    payload: Mapping[str, Any], errors: list[str]
) -> None:
    catalog_digest = require_hex(payload, "catalog_digest_hex", HEX64_LEN, errors)
    predecessor = require_hex(
        payload, "predecessor_catalog_digest_hex", HEX64_LEN, errors
    )
    if predecessor and predecessor == "0" * HEX64_LEN:
        errors.append("predecessor_catalog_digest_hex must be non-zero")
    if predecessor and catalog_digest and predecessor == catalog_digest:
        errors.append("predecessor_catalog_digest_hex must differ from catalog_digest_hex")
    sequence = require_positive_int(payload, "catalog_sequence", errors)
    predecessor_sequence = require_positive_int(
        payload, "predecessor_catalog_sequence", errors
    )
    if sequence and predecessor_sequence and predecessor_sequence + 1 != sequence:
        errors.append(
            "catalog_sequence must immediately follow predecessor_catalog_sequence"
        )


def validate_gateway_acknowledgements(
    payload: Mapping[str, Any],
    *,
    count_field: str,
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_minimum_int(payload, count_field, options.min_gateways, errors)
    require_maximum_int(payload, count_field, MAX_GATEWAYS, errors)
    require_string_inventory_count_match(
        payload,
        "gateway_acknowledgements",
        count_field,
        errors,
        field="gateway_id",
        allow_scalar_items=False,
    )
    expected_digest = payload.get("catalog_digest_hex")
    administrations: set[str] = set()
    regions: set[str] = set()
    for index, record in require_object_array(
        payload, "gateway_acknowledgements", errors
    ):
        path = f"gateway_acknowledgements[{index}]"
        require_exact_fields(
            record,
            frozenset(
                {
                    "gateway_id",
                    "administration_id",
                    "region_id",
                    "catalog_digest_hex",
                    "acknowledged",
                    "signature_verified",
                    "acknowledged_at_unix",
                }
            ),
            errors,
            path=path,
        )
        require_canonical_label(
            record,
            "gateway_id",
            GATEWAY_LABEL_PATTERN,
            errors,
            path=f"{path}.gateway_id",
        )
        administration = require_canonical_label(
            record,
            "administration_id",
            ADMINISTRATION_LABEL_PATTERN,
            errors,
            path=f"{path}.administration_id",
        )
        if administration:
            administrations.add(administration)
        region = require_canonical_label(
            record,
            "region_id",
            REGION_LABEL_PATTERN,
            errors,
            path=f"{path}.region_id",
        )
        if region:
            regions.add(region)
        require_hex(record, "catalog_digest_hex", HEX64_LEN, errors)
        if (
            isinstance(expected_digest, str)
            and record.get("catalog_digest_hex") != expected_digest
        ):
            errors.append(
                f"{path}.catalog_digest_hex must match promoted catalog_digest_hex"
            )
        require_bool_true(record, "acknowledged", errors, path=f"{path}.acknowledged")
        require_bool_true(
            record,
            "signature_verified",
            errors,
            path=f"{path}.signature_verified",
        )
        require_recent_timestamp(
            record,
            "acknowledged_at_unix",
            errors,
            now_unix=options.now_unix,
            max_age_secs=options.max_evidence_age_secs,
            path=f"{path}.acknowledged_at_unix",
        )
    if len(administrations) < options.min_gateways:
        errors.append(
            "gateway_acknowledgements must cover independently administered gateways"
        )
    if len(regions) < options.min_gateways:
        errors.append(
            "gateway_acknowledgements must cover distinct deployment regions"
        )


def validate_catalog_promotion(
    payload: dict[str, Any], errors: list[str], options: ValidationOptions
) -> None:
    require_nonzero_distinct_predecessor(payload, errors)
    promoted = require_hex(
        payload, "promoted_catalog_digest_hex", HEX64_LEN, errors
    )
    if promoted and promoted != payload.get("catalog_digest_hex"):
        errors.append(
            "promoted_catalog_digest_hex must match catalog_digest_hex"
        )
    require_minimum_int(
        payload, "catalog_entry_count", options.min_catalog_entries, errors
    )
    require_maximum_int(
        payload, "catalog_entry_count", MAX_CATALOG_ENTRIES, errors
    )
    require_string_inventory_count_match(
        payload,
        "catalog_entries",
        "catalog_entry_count",
        errors,
        field="entry_id",
        allow_scalar_items=False,
    )
    for index, record in require_object_array(payload, "catalog_entries", errors):
        path = f"catalog_entries[{index}]"
        require_exact_fields(
            record,
            frozenset({"entry_id", "entry_kind", "source_id"}),
            errors,
            path=path,
        )
        require_canonical_label(
            record,
            "entry_id",
            CATALOG_ENTRY_LABEL_PATTERN,
            errors,
            path=f"{path}.entry_id",
        )
        entry_kind = require_string(record, "entry_kind", errors)
        if entry_kind not in {
            "baseline_rule",
            "accepted_appeal",
            "legal_safety_hold",
            "scoped_toggle",
        }:
            errors.append(f"{path}.entry_kind is not recognized")
        require_canonical_label(
            record,
            "source_id",
            SOURCE_LABEL_PATTERN,
            errors,
            path=f"{path}.source_id",
        )

    require_minimum_int(
        payload, "catalog_change_count", options.min_catalog_changes, errors
    )
    require_maximum_int(
        payload, "catalog_change_count", MAX_CATALOG_CHANGES, errors
    )
    require_string_inventory_count_match(
        payload,
        "catalog_changes",
        "catalog_change_count",
        errors,
        field="change_id",
        allow_scalar_items=False,
    )
    for index, record in require_object_array(payload, "catalog_changes", errors):
        path = f"catalog_changes[{index}]"
        require_exact_fields(
            record,
            frozenset({"change_id", "entry_id", "operation"}),
            errors,
            path=path,
        )
        require_canonical_label(
            record,
            "change_id",
            CATALOG_ENTRY_LABEL_PATTERN,
            errors,
            path=f"{path}.change_id",
        )
        require_canonical_label(
            record,
            "entry_id",
            CATALOG_ENTRY_LABEL_PATTERN,
            errors,
            path=f"{path}.entry_id",
        )
        operation = require_string(record, "operation", errors)
        if operation not in {"add", "replace", "remove"}:
            errors.append(f"{path}.operation is not recognized")

    threshold = require_positive_int(payload, "approval_threshold", errors)
    signer_count = require_positive_int(payload, "approval_signer_count", errors)
    require_maximum_int(
        payload, "approval_signer_count", MAX_APPROVAL_SIGNERS, errors
    )
    require_string_inventory_count_match(
        payload,
        "approval_signer_ids",
        "approval_signer_count",
        errors,
    )
    if threshold and signer_count < threshold:
        errors.append("approval_signer_count must satisfy approval_threshold")
    for signer in payload.get("approval_signer_ids", []):
        if not isinstance(signer, str) or SIGNER_LABEL_PATTERN.fullmatch(signer) is None:
            errors.append("approval_signer_ids contains a non-canonical signer")
            break
        markers = forbidden_non_production_markers(
            signer, FORBIDDEN_INVENTORY_LABEL_MARKERS
        )
        if markers:
            errors.append(
                f"approval_signer_ids contains non-production markers {markers}"
            )
            break
    require_bool_true(payload, "catalog_signatures_verified", errors)
    validate_gateway_acknowledgements(
        payload,
        count_field="gateway_ack_count",
        errors=errors,
        options=options,
    )
    require_policy_digest(payload, errors)


def validate_controller_runtime(
    payload: dict[str, Any], errors: list[str], options: ValidationOptions
) -> None:
    require_nonzero_distinct_predecessor(payload, errors)
    require_canonical_label(
        payload,
        "controller_instance_id",
        CONTROLLER_INSTANCE_ID_PATTERN,
        errors,
        path="controller_instance_id",
    )
    require_iroha_config_binding(payload, errors)
    require_minimum_int(
        payload, "source_anchor_count", len(REQUIRED_CONTROLLER_SOURCES), errors
    )
    require_maximum_int(payload, "source_anchor_count", MAX_SOURCE_ANCHORS, errors)
    require_string_inventory_count_match(
        payload,
        "source_anchors",
        "source_anchor_count",
        errors,
        field="source_id",
        allow_scalar_items=False,
    )
    require_string_coverage(
        payload,
        "source_anchors",
        "source_id",
        REQUIRED_CONTROLLER_SOURCES,
        errors,
        allow_scalar_items=False,
    )
    for index, record in require_object_array(payload, "source_anchors", errors):
        path = f"source_anchors[{index}]"
        require_exact_fields(
            record,
            frozenset(
                {
                    "source_id",
                    "source_digest_hex",
                    "generated_at_unix",
                    "signature_verified",
                }
            ),
            errors,
            path=path,
        )
        require_canonical_label(
            record,
            "source_id",
            SOURCE_LABEL_PATTERN,
            errors,
            path=f"{path}.source_id",
        )
        require_hex(record, "source_digest_hex", HEX64_LEN, errors)
        require_recent_timestamp(
            record,
            "generated_at_unix",
            errors,
            now_unix=options.now_unix,
            max_age_secs=options.max_evidence_age_secs,
            path=f"{path}.generated_at_unix",
        )
        require_bool_true(
            record,
            "signature_verified",
            errors,
            path=f"{path}.signature_verified",
        )
    for field in (
        "controller_service_enabled",
        "catalog_signatures_verified",
        "predecessor_link_verified",
        "durable_history_reconciled",
        "last_known_good_available",
        "atomic_catalog_replacement",
    ):
        require_bool_true(payload, field, errors)


def validate_moderation_toggle(payload: dict[str, Any], errors: list[str]) -> None:
    require_2xx_status(
        payload,
        "control_api_status_code",
        errors,
        path="control_api_status_code",
    )
    require_safe_url(payload, "control_api_url", errors)
    require_minimum_int(
        payload, "control_count", len(REQUIRED_MODERATION_CONTROLS), errors
    )
    require_string_inventory_count_match(
        payload,
        "controls",
        "control_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    require_string_coverage(
        payload,
        "controls",
        "name",
        REQUIRED_MODERATION_CONTROLS,
        errors,
        allow_scalar_items=False,
    )
    if {
        record.get("name")
        for record in payload.get("controls", [])
        if isinstance(record, dict)
    } != set(REQUIRED_MODERATION_CONTROLS):
        errors.append("controls contains an unknown control")
    control_count = require_positive_int(payload, "control_count", errors)
    approved_count = require_positive_int(
        payload, "approved_control_count", errors
    )
    if control_count and approved_count != control_count:
        errors.append("approved_control_count must equal control_count")
    for index, record in require_object_array(payload, "controls", errors):
        require_exact_fields(
            record, frozenset({"name"}), errors, path=f"controls[{index}]"
        )
    require_hex(payload, "control_digest_hex", HEX64_LEN, errors)
    require_iroha_config_binding(payload, errors)
    for field in (
        "operator_role_enforced",
        "approval_workflow_enforced",
        "expiry_enforced",
        "catalog_reconciliation_observed",
        "operator_audit_trail_persisted",
    ):
        require_bool_true(payload, field, errors)


def validate_gateway_reload(
    payload: dict[str, Any], errors: list[str], options: ValidationOptions
) -> None:
    require_nonzero_distinct_predecessor(payload, errors)
    validate_gateway_acknowledgements(
        payload,
        count_field="reload_ack_count",
        errors=errors,
        options=options,
    )
    require_maximum_int(
        payload,
        "max_reload_latency_ms",
        options.max_reload_latency_ms,
        errors,
    )
    for field in (
        "atomic_catalog_replacement",
        "persisted_catalog_readback",
        "stale_catalog_rejected",
        "predecessor_mismatch_rejected",
        "rollback_available",
    ):
        require_bool_true(payload, field, errors)
    rollback = require_hex(
        payload, "rollback_catalog_digest_hex", HEX64_LEN, errors
    )
    if rollback and rollback != payload.get("predecessor_catalog_digest_hex"):
        errors.append(
            "rollback_catalog_digest_hex must match predecessor_catalog_digest_hex"
        )


def validate_http_451_record(
    record: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
    *,
    path: str,
    id_field: str,
) -> None:
    expected = {
        id_field,
        "status_code",
        "error",
        "source",
        "catalog_digest_hex",
        "cache_control",
        "latency_ms",
    }
    if id_field == "probe_id":
        expected.add("attack")
    require_exact_fields(record, frozenset(expected), errors, path=path)
    if record.get("status_code") != 451:
        errors.append(f"{path}.status_code must be exactly 451")
    if record.get("error") != "gateway_compliance_denied":
        errors.append(
            f"{path}.error must be exactly gateway_compliance_denied"
        )
    source = require_string(record, "source", errors)
    if source not in REQUIRED_DENIAL_SOURCES:
        errors.append(f"{path}.source is not recognized")
    require_hex(record, "catalog_digest_hex", HEX64_LEN, errors)
    if record.get("cache_control") != "private, no-store, max-age=0":
        errors.append(f"{path}.cache_control is not canonical")
    require_maximum_int(
        record,
        "latency_ms",
        options.max_route_latency_ms,
        errors,
        path=f"{path}.latency_ms",
    )


def validate_enforcement_probe(
    payload: dict[str, Any], errors: list[str], options: ValidationOptions
) -> None:
    require_bool_true(payload, "fail_closed_missing_catalog", errors)
    require_bool_true(payload, "fail_closed_expired_catalog", errors)
    require_bool_true(payload, "rate_limit_enforced", errors)
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
    if {
        record.get("name")
        for record in payload.get("routes", [])
        if isinstance(record, dict)
    } != set(REQUIRED_ENFORCEMENT_ROUTES):
        errors.append("routes contains an unknown route")
    for index, record in require_object_array(payload, "routes", errors):
        path = f"routes[{index}]"
        validate_http_451_record(
            record, errors, options, path=path, id_field="name"
        )
        if record.get("catalog_digest_hex") != payload.get("catalog_digest_hex"):
            errors.append(
                f"{path}.catalog_digest_hex must match promoted catalog_digest_hex"
            )
    observed_sources = {
        record["source"]
        for record in payload.get("routes", [])
        if isinstance(record, dict) and isinstance(record.get("source"), str)
    }
    if observed_sources != set(REQUIRED_DENIAL_SOURCES):
        errors.append("routes must cover exactly the required denial sources")


def validate_honey_audit(
    payload: dict[str, Any], errors: list[str], options: ValidationOptions
) -> None:
    require_minimum_int(
        payload, "probe_count", options.min_honey_probes, errors
    )
    require_maximum_int(payload, "probe_count", MAX_HONEY_PROBES, errors)
    require_string_inventory_count_match(
        payload,
        "probes",
        "probe_count",
        errors,
        field="probe_id",
        allow_scalar_items=False,
    )
    for index, record in require_object_array(payload, "probes", errors):
        path = f"probes[{index}]"
        validate_http_451_record(
            record, errors, options, path=path, id_field="probe_id"
        )
        require_canonical_label(
            record,
            "probe_id",
            HONEY_PROBE_LABEL_PATTERN,
            errors,
            path=f"{path}.probe_id",
        )
        if record.get("attack") not in REQUIRED_HONEY_ATTACKS:
            errors.append(f"{path}.attack is not recognized")
        if record.get("catalog_digest_hex") != payload.get("catalog_digest_hex"):
            errors.append(
                f"{path}.catalog_digest_hex must match promoted catalog_digest_hex"
            )
    observed_attacks = {
        record["attack"]
        for record in payload.get("probes", [])
        if isinstance(record, dict) and isinstance(record.get("attack"), str)
    }
    if observed_attacks != set(REQUIRED_HONEY_ATTACKS):
        errors.append("probes must cover exactly the required honey attacks")


def validate_precedence(payload: dict[str, Any], errors: list[str]) -> None:
    require_string_inventory_count_match(
        payload,
        "cases",
        "case_count",
        errors,
        field="case_id",
        allow_scalar_items=False,
    )
    seen: set[str] = set()
    for index, record in require_object_array(payload, "cases", errors):
        path = f"cases[{index}]"
        require_exact_fields(
            record,
            frozenset({"case_id", "source", "denied"}),
            errors,
            path=path,
        )
        case_id = require_string(record, "case_id", errors)
        expected = REQUIRED_PRECEDENCE_CASES.get(case_id)
        if expected is None:
            errors.append(f"{path}.case_id is not recognized")
            continue
        seen.add(case_id)
        expected_source, expected_denied = expected
        if record.get("source") != expected_source:
            errors.append(f"{path}.source violates canonical precedence")
        if record.get("denied") is not expected_denied:
            errors.append(f"{path}.denied violates canonical precedence")
    if seen != set(REQUIRED_PRECEDENCE_CASES):
        errors.append(
            "cases must cover legal/safety hold, accepted appeal, and baseline precedence"
        )
    require_bool_true(payload, "finalized_chain_projection", errors)


def validate_transparency_publication(
    payload: dict[str, Any], errors: list[str]
) -> None:
    for field in (
        "catalog_history_published",
        "catalog_acknowledgements_published",
        "moderation_events_published",
        "legal_hold_redaction_summaries_published",
        "governance_dag_bound",
    ):
        require_bool_true(payload, field, errors)
    require_hex(payload, "publication_digest_hex", HEX64_LEN, errors)


def validate_observability(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "metrics_scrape_success", errors)
    require_bool_true(payload, "dashboard_provisioned", errors)
    require_bool_true(payload, "alert_rules_installed", errors)
    require_false(payload, "critical_alerts_firing", errors)
    require_positive_int(payload, "metric_count", errors)
    require_string_inventory_count_match(payload, "metrics", "metric_count", errors)
    require_string_coverage(payload, "metrics", "", REQUIRED_METRICS, errors)
    if set(payload.get("metrics", [])) != set(REQUIRED_METRICS):
        errors.append("metrics contains an unknown metric")


def validate_governance_approval(payload: dict[str, Any], errors: list[str]) -> None:
    require_config_backed_governance_approval(payload, errors)
    for field in (
        "catalog_policy_bound",
        "catalog_source_roster_bound",
        "transparency_policy_bound",
        "operator_roles_bound",
        "retention_policy_bound",
    ):
        require_bool_true(payload, field, errors)
    require_policy_digest(payload, errors)


def validate_kind_specific(
    kind: EvidenceKind,
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
    *,
    require_production: bool,
) -> None:
    require_object(payload, f"{kind.name} payload", errors)
    require_exact_fields(
        payload,
        EVIDENCE_REQUIRED_FIELDS[kind.name],
        errors,
        path=f"{kind.name} payload",
    )
    reject_legacy_fields(payload, errors)
    if require_production:
        require_passed_status(payload, errors)
        require_string_equal(payload, "evidence_scope", "production", errors)
    else:
        if payload.get("status") != "non_production":
            errors.append("fixture status must be non_production")
        if payload.get("evidence_scope") != "non_production_fixture":
            errors.append("fixture evidence_scope must be non_production_fixture")
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_evidence_age_secs,
    )
    require_hex(payload, "catalog_digest_hex", HEX64_LEN, errors)
    require_positive_int(payload, "catalog_sequence", errors)

    if kind.name == "catalog_promotion":
        validate_catalog_promotion(payload, errors, options)
    elif kind.name == "controller_runtime":
        validate_controller_runtime(payload, errors, options)
    elif kind.name == "moderation_toggle":
        validate_moderation_toggle(payload, errors)
    elif kind.name == "gateway_reload":
        validate_gateway_reload(payload, errors, options)
    elif kind.name == "enforcement_probe":
        validate_enforcement_probe(payload, errors, options)
    elif kind.name == "honey_audit":
        validate_honey_audit(payload, errors, options)
    elif kind.name == "precedence":
        validate_precedence(payload, errors)
    elif kind.name == "transparency_publication":
        validate_transparency_publication(payload, errors)
    elif kind.name == "observability":
        validate_observability(payload, errors)
    elif kind.name == "governance_approval":
        validate_governance_approval(payload, errors)


def validate_evidence_payload(
    payload: dict[str, Any],
    options: ValidationOptions,
    *,
    require_production: bool = True,
) -> tuple[str | None, list[str]]:
    """Validate one exact canonical evidence artifact."""

    return validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS gateway-compliance rollout artifact",
        SENSITIVE_KEYS,
        "gateway-compliance rollout evidence",
        lambda kind, checked_payload, errors: validate_kind_specific(
            kind,
            checked_payload,
            errors,
            options,
            require_production=require_production,
        ),
        require_reviewed_deployment_context=True,
    )


def require_single_active_digest(
    digests: set[str], errors: list[str], *, label: str
) -> set[str]:
    if len(digests) <= 1:
        return digests
    errors.append(f"{label} must contain exactly one active digest")
    return set()


def catalog_history_binding(
    fingerprint: Mapping[str, Any],
) -> tuple[str, int, str, int] | None:
    """Return the complete catalog-history tuple from an artifact fingerprint."""

    catalog_digest = fingerprint.get("catalog_digest_hex")
    catalog_sequence = fingerprint.get("catalog_sequence")
    predecessor_digest = fingerprint.get("predecessor_catalog_digest_hex")
    predecessor_sequence = fingerprint.get("predecessor_catalog_sequence")
    if (
        not isinstance(catalog_digest, str)
        or type(catalog_sequence) is not int
        or not isinstance(predecessor_digest, str)
        or type(predecessor_sequence) is not int
    ):
        return None
    return (
        catalog_digest,
        catalog_sequence,
        predecessor_digest,
        predecessor_sequence,
    )


def require_single_active_catalog_history(
    bindings: set[tuple[str, int, str, int]],
    errors: list[str],
) -> set[tuple[str, int, str, int]]:
    """Return the one promoted catalog history or fail closed on mixed histories."""

    if len(bindings) <= 1:
        return bindings
    errors.append(
        "valid_catalog_history_bindings must contain exactly one active binding"
    )
    return set()


def validate_catalog_history_references(
    *,
    required_kinds: tuple[str, ...],
    bound_artifacts: list[tuple[str, dict[str, Any]]],
    valid_bindings: set[tuple[str, int, str, int]],
    errors: list[str],
) -> None:
    """Invalidate predecessor-bound artifacts outside the promoted history."""

    canonical_valid_bindings = {
        (
            catalog_digest,
            str(catalog_sequence),
            predecessor_digest,
            str(predecessor_sequence),
        )
        for (
            catalog_digest,
            catalog_sequence,
            predecessor_digest,
            predecessor_sequence,
        ) in valid_bindings
    }
    if valid_bindings:
        for kind_name, artifact in bound_artifacts:
            if not evidence_artifact_is_valid(artifact):
                continue
            binding = catalog_history_binding(
                evidence_artifact_fingerprint(artifact)
            )
            canonical_binding = (
                ()
                if binding is None
                else (
                    binding[0],
                    str(binding[1]),
                    binding[2],
                    str(binding[3]),
                )
            )
            record_string_tuple_binding_errors(
                artifact,
                canonical_binding,
                canonical_valid_bindings,
                errors,
                message=(
                    f"{kind_name} catalog history must match a valid "
                    "catalog_promotion catalog history"
                ),
            )
        return
    if not set(required_kinds).intersection(PREDECESSOR_BOUND_KINDS):
        return
    for kind_name, artifact in bound_artifacts:
        if not evidence_artifact_is_valid(artifact):
            continue
        record_string_tuple_binding_errors(
            artifact,
            (),
            canonical_valid_bindings,
            errors,
            message=(
                f"{kind_name} catalog history requires exactly one valid "
                "catalog_promotion catalog history"
            ),
        )


def build_summary(
    evidence_dirs: list[Path],
    evidence_files: list[Path],
    required_kinds: tuple[str, ...],
    options: ValidationOptions,
    summary_out: Path | None,
) -> tuple[dict[str, Any], list[str]]:
    """Build the payload-free aggregate for this readiness lane."""

    errors: list[str] = []
    artifacts_by_kind = init_evidence_artifact_buckets(DEFAULT_REQUIRED_KINDS)
    valid_catalog_digests: set[str] = set()
    valid_catalog_history_bindings: set[tuple[str, int, str, int]] = set()
    catalog_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    predecessor_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
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
            catalog_digest = fingerprint.get("catalog_digest_hex")
            if kind_name == "catalog_promotion":
                if isinstance(catalog_digest, str):
                    valid_catalog_digests.add(catalog_digest)
                history_binding = catalog_history_binding(fingerprint)
                if history_binding is not None:
                    valid_catalog_history_bindings.add(history_binding)
                policy_digest = fingerprint.get("policy_digest_hex")
                if isinstance(policy_digest, str):
                    valid_policy_digests.add(policy_digest)
            if kind_name in CATALOG_BOUND_KINDS:
                catalog_bound_artifacts.append((kind_name, artifact))
            if kind_name in PREDECESSOR_BOUND_KINDS:
                predecessor_bound_artifacts.append((kind_name, artifact))
            if kind_name in POLICY_BOUND_KINDS:
                policy_bound_artifacts.append((kind_name, artifact))
        record_evidence_validation_errors(path, validation_errors, errors)

    valid_catalog_digests = require_single_active_digest(
        valid_catalog_digests, errors, label="valid_catalog_digests"
    )
    valid_catalog_history_bindings = require_single_active_catalog_history(
        valid_catalog_history_bindings,
        errors,
    )
    valid_policy_digests = require_single_active_digest(
        valid_policy_digests, errors, label="valid_policy_digests"
    )
    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=CATALOG_BOUND_KINDS,
        bound_artifacts=catalog_bound_artifacts,
        valid_anchor_digests=valid_catalog_digests,
        digest_field="catalog_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} catalog_digest_hex must match a valid "
            "catalog_promotion catalog_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} catalog_digest_hex requires a valid "
            "catalog_promotion catalog_digest_hex"
        ),
    )
    validate_catalog_history_references(
        required_kinds=required_kinds,
        bound_artifacts=predecessor_bound_artifacts,
        valid_bindings=valid_catalog_history_bindings,
        errors=errors,
    )
    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("catalog_promotion",),
        bound_artifacts=policy_bound_artifacts,
        valid_anchor_digests=valid_policy_digests,
        digest_field="policy_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} policy_digest_hex must match a valid "
            "catalog_promotion policy_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} policy_digest_hex requires a valid "
            "catalog_promotion policy_digest_hex"
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
            "min_catalog_entries": options.min_catalog_entries,
            "min_catalog_changes": options.min_catalog_changes,
            "min_honey_probes": options.min_honey_probes,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "recognized_artifacts": recognized_evidence_artifacts(artifacts_by_kind),
        "valid_catalog_digests": sorted(valid_catalog_digests),
        "valid_catalog_history_bindings": [
            {
                "catalog_digest_hex": catalog_digest,
                "catalog_sequence": catalog_sequence,
                "predecessor_catalog_digest_hex": predecessor_digest,
                "predecessor_catalog_sequence": predecessor_sequence,
            }
            for (
                catalog_digest,
                catalog_sequence,
                predecessor_digest,
                predecessor_sequence,
            ) in sorted(valid_catalog_history_bindings)
        ],
        "valid_policy_digests": sorted(valid_policy_digests),
        "metrics": sorted(metric_names),
        "metric_count_values": sorted(metric_counts),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description=(
            "Validate canonical SoraFS gateway-compliance rollout evidence."
        )
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
        help="Required evidence kind, or comma-separated kinds. Defaults to all.",
    )
    parser.add_argument("--summary-out", type=Path)
    parser.add_argument("--now-unix", type=positive_int_arg, required=True)
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
        "--min-gateways", type=positive_int_arg, default=DEFAULT_MIN_GATEWAYS
    )
    parser.add_argument(
        "--min-catalog-entries",
        type=positive_int_arg,
        default=DEFAULT_MIN_CATALOG_ENTRIES,
    )
    parser.add_argument(
        "--min-catalog-changes",
        type=positive_int_arg,
        default=DEFAULT_MIN_CATALOG_CHANGES,
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
        min_catalog_entries=args.min_catalog_entries,
        min_catalog_changes=args.min_catalog_changes,
        min_honey_probes=args.min_honey_probes,
    )
    preflight_errors = validate_checker_preflight(args)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2
    summary, errors = build_summary(
        args.evidence_dir,
        args.evidence,
        required_kinds,
        options,
        args.summary_out,
    )
    errors.extend(
        bind_lane_summary_to_topology(
            summary, args.topology_qualification_summary
        )
    )
    summary["status"] = evidence_gate_status(errors)
    _rendered, summary_errors = render_and_write_checker_summary(
        args.summary_out, summary
    )
    if summary_errors:
        emit_checker_error_lines(summary_errors)
        return 2
    if errors:
        emit_checker_error_block(
            "ERROR: SoraFS gateway-compliance rollout evidence is incomplete:",
            errors,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
