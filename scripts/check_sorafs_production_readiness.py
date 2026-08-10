#!/usr/bin/env python3
"""Validate aggregate SoraFS production-readiness gate summaries."""

from __future__ import annotations

import argparse
import hashlib
import ipaddress
import json
import re
import sys
from collections import Counter
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit


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
from sorafs_archive_path_components import (  # noqa: E402
    decoded_text_variants,
    path_component_has_uri_scheme_prefix,
    path_component_has_windows_drive_prefix,
)
from sorafs_evidence_json import (  # noqa: E402
    load_evidence_json_with_sha256_or_record_error,
)
from sorafs_evidence_paths import (  # noqa: E402
    discover_evidence_files,
    evidence_path_identities,
    is_explicit_evidence_path,
)
from sorafs_evidence_sensitivity import (  # noqa: E402
    COMMON_SENSITIVE_KEYS,
    HIGH_RISK_SENSITIVE_KEY_FRAGMENTS,
    PAYLOAD_FREE_SENSITIVE_REFERENCE_SUFFIXES,
    normalize_sensitive_key,
    visit_sensitive_fields,
)
from sorafs_evidence_validation import (  # noqa: E402
    evidence_schema_by_kind,
    evidence_gate_status,
    forbidden_non_production_markers,
    require_rollout_deployment_id,
)
from sorafs_path_identity import diagnostic_text_is_canonical, resolve_path_identity  # noqa: E402
from sorafs_required_kinds import (  # noqa: E402
    parse_required_kinds as parse_required_gates,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)
import sorafs_software_signer_evidence as software_signer_evidence  # noqa: E402
foundational_signing_payload = software_signer_evidence.foundational_signing_payload
parse_foundational_signer_public_key = (
    software_signer_evidence.parse_foundational_signer_public_key)
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
    INGRESS_QUALIFICATION_BOUND_KINDS as GOVERNANCE_DAG_INGRESS_BOUND_KINDS,
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
from sccp_release_common import verify_ed25519  # noqa: E402
from sorafs_topology_qualification import (  # noqa: E402
    add_signed_topology_qualification_arguments,
    load_signed_topology_qualification_from_args,
    validate_authenticated_topology_binding_object,
    validate_topology_binding_object,
)
from sorafs_l1_lane_inventory_integration import (  # noqa: E402
    VerifiedLaneInventory,
    add_signed_lane_inventory_arguments,
    aggregate_inventory_row,
    validate_aggregate_inventory_row,
    validate_aggregate_foundational_lane_digest_bindings,
    validate_disallowed_summary_diagnostics,
    validate_duplicate_summary_diagnostics,
    validate_foundational_inventory_digest,
    validate_foundational_lane_summary_digest_bindings,
    validate_inventory_lane_digest_bindings,
    validate_signer_independence,
    verify_inventory_from_args,
)
from sorafs_l1_lane_evidence_inventory import (  # noqa: E402
    InventoryError,
    parse_summary_specs as parse_inventory_summary_specs,
)
from sorafs_production_readiness_contract import (  # noqa: E402
    FOUNDATIONAL_PREREQUISITE_SCHEMA,
    FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN,
    RESILIENCE_QUALIFICATION_RECEIPT_SCHEMA,
    RESILIENCE_QUALIFICATION_SUMMARY_SCHEMA,
    RESILIENCE_QUALIFICATION_SIGNATURE_DOMAIN,
    RESILIENCE_QUALIFICATION_REQUIREMENTS,
    RESILIENCE_QUALIFICATION_SUMMARY_FIELDS,
    RESILIENCE_QUALIFICATION_ARTIFACT_BINDING_FIELDS,
    RESILIENCE_QUALIFICATION_AUTHENTICATION_FIELDS,
    RESILIENCE_QUALIFICATION_BINDING_SCHEMA,
    RESILIENCE_QUALIFICATION_BINDING_FIELDS,
    AGGREGATE_RESILIENCE_QUALIFICATION_SCHEMA,
    AGGREGATE_RESILIENCE_QUALIFICATION_FIELDS,
    FOUNDATIONAL_PREREQUISITE_IDS,
    FOUNDATIONAL_PREREQUISITE_LANES,
    MAX_FOUNDATIONAL_RELEASE_SEQUENCE,
    FOUNDATIONAL_PREREQUISITE_FIELDS,
    FOUNDATIONAL_PREREQUISITE_DEPLOYMENT_FIELDS,
    FOUNDATIONAL_PREREQUISITE_ROW_FIELDS,
    FOUNDATIONAL_LANE_SUMMARY_ROW_FIELDS,
    AGGREGATE_FOUNDATIONAL_PREREQUISITE_READINESS_SUMMARY_ROW_FIELDS,
    FOUNDATIONAL_PREREQUISITE_SIGNATURE_FIELDS,
    AGGREGATE_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS,
    AGGREGATE_L1_LANE_EVIDENCE_INVENTORY_FIELDS,
    AGGREGATE_L1_LANE_EVIDENCE_INVENTORY_SCHEMA,
    AGGREGATE_MISSING_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS,
    POP_CREDENTIALS_ROOT_BOUND_FINGERPRINT_FIELDS,
    POP_CREDENTIALS_REVOCATION_BOUND_FINGERPRINT_FIELDS,
    MAX_SUMMARY_BYTES,
    DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
    LOWER_HEX_DIGITS,
    PRODUCTION_READY_ENVIRONMENTS,
    FORBIDDEN_PRODUCTION_DEPLOYMENT_MARKERS,
    SUCCESS_ARTIFACT_STATUSES,
    REFERENCE_SDK_SUPPLY_CHAIN_SOURCE_ARTIFACT_FIELDS,
    REFERENCE_SDK_SUPPLY_CHAIN_SOURCE_DIGEST_BINDINGS,
    REFERENCE_SDK_PUBLIC_PROVENANCE_FINGERPRINT_FIELDS,
    REFERENCE_SDK_PUBLIC_PROVENANCE_PATH_COMPONENT_RE,
    REFERENCE_SDK_JWT_LIKE_PATH_COMPONENT_RE,
    REFERENCE_SDK_RESERVED_PUBLIC_HOST_SUFFIXES,
    REFERENCE_SDK_LEGACY_IPV4_COMPONENT_RE,
    PATH_SENSITIVE_KEY_FRAGMENTS,
    GATE_METADATA_FIELDS,
    PAYLOAD_FREE_SUMMARY_CORE_FIELDS,
    PAYLOAD_FREE_SUMMARY_METADATA_FIELDS,
    PAYLOAD_FREE_SUMMARY_FIELDS,
    PAYLOAD_FREE_SUMMARY_LIST_METADATA_FIELDS,
    PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS,
    PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS,
    PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS,
    PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS,
    PAYLOAD_FREE_SUMMARY_OBJECT_LIST_DOMAIN_IDENTITY_FIELDS,
    PAYLOAD_FREE_SUMMARY_OBJECT_LIST_REQUIRED_KIND_COUNTS,
    PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS,
    PAYLOAD_FREE_SUMMARY_OBJECT_LIST_STRING_FIELD_POLICIES,
    PAYLOAD_FREE_SUMMARY_OBJECT_LIST_FINGERPRINT_HEX_BINDINGS,
    PAYLOAD_FREE_SUMMARY_STRING_METADATA_FIELDS,
    PAYLOAD_FREE_SUMMARY_HEX_METADATA_LENGTHS,
    PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_BINDINGS,
    PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_SOURCE_KINDS,
    PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_BINDINGS,
    PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_SOURCE_KINDS,
    PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS,
    PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS,
    PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_BINDINGS,
    PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS,
    PAYLOAD_FREE_SUMMARY_ALLOWED_STRING_LIST_VALUES,
    PAYLOAD_FREE_SUMMARY_REQUIRED_STRING_LIST_VALUES,
    PAYLOAD_FREE_SUMMARY_STRING_LIST_COUNT_BINDINGS,
    PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_BINDINGS,
    PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS,
    PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_FIELDS,
    PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_SOURCE_KINDS,
    PAYLOAD_FREE_SUMMARY_OBJECT_METADATA_FIELDS,
    PAYLOAD_FREE_SUMMARY_ORDERED_LIST_METADATA_FIELDS,
    MAX_SUMMARY_METADATA_DEPTH,
    PAYLOAD_FREE_ARTIFACT_FIELDS,
    PAYLOAD_FREE_REQUIRED_ROW_FIELDS,
    AGGREGATE_REQUIRED_GATE_ROW_FIELDS,
    AGGREGATE_MISSING_GATE_ROW_FIELDS,
    AGGREGATE_SUMMARY_FIELDS,
    GateSummaryKind,
    GATE_SUMMARY_KINDS,
    SCHEMA_TO_GATE,
    GATE_BY_NAME,
    DEFAULT_REQUIRED_GATES,
    GATE_REQUIRED_KIND_SCHEMAS,
)

# Keep the public aggregate schema and diagnostic-sensitivity inventory local:
# release contract tests and shared evidence tooling inspect these literal
# declarations without importing the checker.
SUMMARY_SCHEMA = "sorafs.production_readiness.aggregate_gate.v1"
GATEWAY_MODERATION_CATALOG_MISMATCH_ERROR = (
    "moderation_panel evidence viewer catalog digests must match "
    "gateway_compliance valid_catalog_digests"
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

# The imported contract builds payload-free metadata with
# frozenset().union(*GATE_METADATA_FIELDS.values()) and
# PAYLOAD_FREE_SUMMARY_CORE_FIELDS | PAYLOAD_FREE_SUMMARY_METADATA_FIELDS.
# Keep this source-level index for rollout documentation checks that audit the
# aggregate checker without importing its declarative contract module:
# ("appeal_finance", "metrics"): ("dashboard_metrics",)
# ("appeal_finance", "metric_count_values"): ("dashboard_metrics",)
# ("gateway_compliance", "metrics"): ("observability",)
# ("gateway_compliance", "metric_count_values"): ("observability",)
# ("gateway_load", "metrics"): ("telemetry_slo",)
# ("gateway_load", "metric_count_values"): ("telemetry_slo",)
# ("governance_dag", "metrics"): ("observability",)
# ("governance_dag", "metric_count_values"): ("observability",)
# ("hedging_billing", "metrics"): ("metrics_alerts",)
# ("hedging_billing", "metric_count_values"): ("metrics_alerts",)
# ("moderation_panel", "metrics"): ("metrics_alerts",)
# ("moderation_panel", "metric_count_values"): ("metrics_alerts",)
# ("orderbook", "metrics"): ("observability",)
# ("orderbook", "metric_count_values"): ("observability",)
# ("pdp", "metrics"): ("observability",)
# ("pdp", "metric_count_values"): ("observability",)
# ("pdp", "valid_repair_handoff_digests"): ("governance_repair",)
# ("pop_credentials", "metrics"): ("metrics_alerts",)
# ("pop_credentials", "metric_count_values"): ("metrics_alerts",)
# ("por", "archive_backends"): ("reporting_archive",)
# ("por", "archive_backends"): POR_ALLOWED_ARCHIVE_BACKENDS
# ("por", "metrics"): ("observability",)
# ("por", "metric_count_values"): ("observability",)
# ("por", "valid_governance_archive_handoff_digests"): ("reporting_archive",)
# ("potr", "metrics"): ("observability",)
# ("potr", "metric_count_values"): ("observability",)
# ("reference_sdk_release", "signature_algorithms"): ("signed_manifest",)
# ("repair", "metrics"): ("observability",)
# ("repair", "metric_count_values"): ("observability",)
# ("reputation", "metrics"): ("metrics",)
# ("reputation", "metric_count_values"): ("metrics",)
# ("reputation", "valid_reputation_weight_digests"): ("publish", "latest")
# ("reserve_rent", "metrics"): ("metrics_alerts",)
# ("reserve_rent", "metric_count_values"): ("metrics_alerts",)
# "valid_reputation_weight_digests": "weights_digest_hex"
# "valid_governance_archive_handoff_digests": (
# synced_root_digest_hex
# synced_revocation_list_digest_hex
# "metric_count_values"
# "scheduled_lifecycle_canary_tick_count"
# "scheduled_lifecycle_canary_last_tick_at_unix"
# "scheduled_lifecycle_canary_defaulted_provider_count"
# {"requestbody", "responsebody"}
_ROLLOUT_SOURCE_CONTRACT_INDEX = """
(
        "reference_sdk_release",
        "signature_algorithms",
"""
NON_PROMOTABLE_STATUS = "partial"


def aggregate_summary_status(
    errors: list[str],
    required_gates: Sequence[str],
) -> str:
    """Return ready only for the exact canonical production gate inventory."""

    diagnostic_status = evidence_gate_status(errors)
    if diagnostic_status != "ready":
        return diagnostic_status
    if tuple(required_gates) != DEFAULT_REQUIRED_GATES:
        return NON_PROMOTABLE_STATUS
    return "ready"


@dataclass(frozen=True)
class ValidationOptions:
    """Aggregate SoraFS production-readiness thresholds."""

    now_unix: int
    max_summary_artifact_age_secs: int
    deployment_id: str | None
    environment: str | None
    foundational_signer_public_key: bytes | None = None
    foundational_release_sequence: int | None = None
    foundational_previous_envelope_sha256: str | None = None
    topology_qualification: Mapping[str, str] | None = None
    resilience_qualification: Mapping[str, Any] | None = None
    resilience_qualification_errors: tuple[str, ...] = ()
    l1_lane_evidence_inventory: VerifiedLaneInventory | None = None
    l1_lane_evidence_inventory_errors: tuple[str, ...] = ()


def canonical_string(value: Any) -> str | None:
    """Return a non-empty canonical string, or None."""

    return value if diagnostic_text_is_canonical(value) else None


def canonical_public_provenance_url(value: Any) -> str | None:
    """Return one credential-free canonical public HTTPS provenance URL."""

    label = canonical_string(value)
    if (
        label is None
        or not label.isascii()
        or any(character.isspace() for character in label)
    ):
        return None
    try:
        parsed = urlsplit(label)
        parsed_port = parsed.port
    except ValueError:
        return None
    if (
        parsed.scheme != "https"
        or not parsed.netloc
        or parsed.hostname is None
        or parsed.username is not None
        or parsed.password is not None
        or parsed.query
        or parsed.fragment
        or "\\" in label
        or parsed.netloc != parsed.netloc.lower()
        or parsed_port is not None
    ):
        return None
    hostname = parsed.hostname
    try:
        host_ip = ipaddress.ip_address(hostname)
    except ValueError:
        host_ip = None
    if host_ip is not None:
        if not host_ip.is_global:
            return None
        canonical_netloc = (
            f"[{host_ip.compressed}]" if host_ip.version == 6 else host_ip.compressed
        )
    else:
        if (
            hostname != hostname.lower()
            or hostname.endswith(".")
            or "." not in hostname
            or hostname in {"localhost", "local", "internal"}
            or hostname.endswith(REFERENCE_SDK_RESERVED_PUBLIC_HOST_SUFFIXES)
        ):
            return None
        labels = hostname.split(".")
        if len(labels) <= 4 and all(
            REFERENCE_SDK_LEGACY_IPV4_COMPONENT_RE.fullmatch(component)
            is not None
            for component in labels
        ):
            return None
        if any(
            re.fullmatch(
                r"[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?",
                component,
            )
            is None
            for component in labels
        ):
            return None
        canonical_netloc = hostname
    if parsed.netloc != canonical_netloc:
        return None

    path_components = parsed.path.split("/")
    if path_components and path_components[0] == "":
        path_components = path_components[1:]
    if any(component == "" for component in path_components):
        return None
    for component in path_components:
        if (
            REFERENCE_SDK_PUBLIC_PROVENANCE_PATH_COMPONENT_RE.fullmatch(component)
            is None
        ):
            return None
        for variant in decoded_text_variants(component):
            if (
                variant in {".", ".."}
                or "/" in variant
                or "\\" in variant
                or path_component_has_sensitive_label(variant)
                or REFERENCE_SDK_JWT_LIKE_PATH_COMPONENT_RE.search(variant)
                is not None
            ):
                return None
    return label


def sanitize_reference_sdk_supply_chain_artifact_for_sensitivity(
    artifact: Any,
    *,
    supply_chain_kind_known: bool,
) -> Any:
    """Mask only validated public provenance URLs in a supply-chain artifact."""

    if not isinstance(artifact, dict):
        return artifact
    if (
        not supply_chain_kind_known
        and artifact.get("kind") != "supply_chain"
    ):
        return artifact
    fingerprint = artifact.get("fingerprint")
    if not isinstance(fingerprint, dict):
        return artifact
    sanitized_fingerprint = dict(fingerprint)
    changed = False
    for field in REFERENCE_SDK_PUBLIC_PROVENANCE_FINGERPRINT_FIELDS:
        if canonical_public_provenance_url(fingerprint.get(field)) is not None:
            sanitized_fingerprint[field] = "<public-provenance-metadata>"
            changed = True
    if not changed:
        return artifact
    sanitized_artifact = dict(artifact)
    sanitized_artifact["fingerprint"] = sanitized_fingerprint
    return sanitized_artifact


def reference_sdk_release_sensitivity_view(payload: dict[str, Any]) -> dict[str, Any]:
    """Return a lane-local view that retains all non-public secret scanning."""

    candidate = dict(payload)
    recognized = payload.get("recognized_artifacts")
    if isinstance(recognized, list):
        candidate["recognized_artifacts"] = [
            sanitize_reference_sdk_supply_chain_artifact_for_sensitivity(
                artifact,
                supply_chain_kind_known=False,
            )
            for artifact in recognized
        ]

    required = payload.get("required")
    if not isinstance(required, dict):
        return candidate
    supply_chain = required.get("supply_chain")
    if not isinstance(supply_chain, dict):
        return candidate
    artifacts = supply_chain.get("artifacts")
    if not isinstance(artifacts, list):
        return candidate
    required_copy = dict(required)
    supply_chain_copy = dict(supply_chain)
    supply_chain_copy["artifacts"] = [
        sanitize_reference_sdk_supply_chain_artifact_for_sensitivity(
            artifact,
            supply_chain_kind_known=True,
        )
        for artifact in artifacts
    ]
    required_copy["supply_chain"] = supply_chain_copy
    candidate["required"] = required_copy
    return candidate


def is_production_ready_environment(value: Any) -> bool:
    """Return whether an environment label can promote final production readiness."""

    environment = canonical_string(value)
    return environment in PRODUCTION_READY_ENVIRONMENTS


def require_reviewed_deployment_id_value(
    value: Any,
    errors: list[str],
    path: str,
) -> str:
    """Return a reviewed deployment id value or record path-scoped diagnostics."""

    field_errors: list[str] = []
    deployment_id = require_rollout_deployment_id(
        {"deployment_id": value},
        field_errors,
    )
    for error in field_errors:
        errors.append(error.replace("deployment_id", path, 1))
    return deployment_id


def require_production_deployment_id_value(
    value: Any,
    errors: list[str],
    path: str,
) -> str:
    """Return a final-production deployment id or record path-scoped diagnostics."""

    deployment_id = require_reviewed_deployment_id_value(value, errors, path)
    if not deployment_id:
        return ""
    detected_markers = forbidden_non_production_markers(
        deployment_id,
        FORBIDDEN_PRODUCTION_DEPLOYMENT_MARKERS,
    )
    forbidden = sorted(
        marker
        for marker in detected_markers
        if not any(marker != other and marker in other for other in detected_markers)
    )
    if forbidden:
        errors.append(
            f"{path} must not contain non-production deployment markers {forbidden}"
        )
        return ""
    return deployment_id


def is_payload_free_sensitive_reference(normalized_key: str) -> bool:
    """Return whether a sensitive-looking key is an allowed digest marker."""

    return any(
        normalized_key.endswith(suffix)
        for suffix in PAYLOAD_FREE_SENSITIVE_REFERENCE_SUFFIXES
    )


def is_sensitive_diagnostic_key(key: str) -> bool:
    """Return whether a canonical key name should be hidden in diagnostics."""

    exact_keys = frozenset(key.lower() for key in SENSITIVE_KEYS) | COMMON_SENSITIVE_KEYS
    normalized_keys = frozenset(normalize_sensitive_key(key) for key in exact_keys)
    key_lower = key.lower()
    normalized_key = normalize_sensitive_key(key)
    return (
        key_lower in exact_keys
        or normalized_key in normalized_keys
        or any(
            fragment in normalized_key
            and not is_payload_free_sensitive_reference(normalized_key)
            for fragment in HIGH_RISK_SENSITIVE_KEY_FRAGMENTS
        )
    )


def payload_free_diagnostic_key_label(key: str) -> str:
    """Return a safe key label for aggregate readiness diagnostics."""

    return "<sensitive-key>" if is_sensitive_diagnostic_key(key) else key


def require_string_field(
    payload: dict[str, Any],
    field: str,
    errors: list[str],
) -> str:
    """Return a canonical string field or record an error."""

    value = canonical_string(payload.get(field))
    if value is None:
        errors.append(f"{field} must be a non-empty canonical string")
        return ""
    return value


def require_positive_int_field(
    payload: dict[str, Any],
    field: str,
    errors: list[str],
) -> int | None:
    """Return a positive integer field or record an error."""

    value = payload.get(field)
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        errors.append(f"{field} must be a positive integer")
        return None
    return value


def canonical_int_value(value: Any) -> int | None:
    """Return an integer value while excluding booleans."""

    if isinstance(value, int) and not isinstance(value, bool):
        return value
    return None


def require_empty_error_list(value: Any, path: str, errors: list[str]) -> bool:
    """Require an empty list of canonical errors."""

    if not isinstance(value, list):
        errors.append(f"{path} must be an empty error list")
        return False
    for error in value:
        if canonical_string(error) is None:
            errors.append(f"{path} must contain only canonical strings")
            return False
    if value:
        errors.append(f"{path} must be empty")
        return False
    return True


def require_absent_or_empty_error_list(
    payload: dict[str, Any],
    field: str,
    errors: list[str],
) -> bool:
    """Require an optional diagnostic list to be absent or empty."""

    if field not in payload:
        return True
    return require_empty_error_list(payload.get(field), field, errors)


def require_threshold_map(
    payload: dict[str, Any],
    field: str,
    errors: list[str],
) -> dict[str, int]:
    """Return validated threshold metadata or record closed failures."""

    if field not in payload:
        errors.append(f"{field} must be present")
        return {}
    thresholds = payload.get(field)
    if not isinstance(thresholds, dict):
        errors.append(f"{field} must be an object")
        return {}
    if not thresholds:
        errors.append(f"{field} must not be empty")
        return {}
    valid_thresholds: dict[str, int] = {}
    for key, value in thresholds.items():
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(f"{field} keys must be canonical strings")
        value_is_valid = (
            isinstance(value, int)
            and not isinstance(value, bool)
            and value >= 0
        )
        key_diagnostic_label = (
            payload_free_diagnostic_key_label(key_label)
            if key_label is not None
            else "<invalid>"
        )
        key_is_sensitive = key_label is not None and is_sensitive_diagnostic_key(
            key_label
        )
        if key_is_sensitive:
            errors.append(f"{field}.{key_diagnostic_label} must not be present")
        if not value_is_valid:
            errors.append(
                f"{field}.{key_diagnostic_label} must be a non-negative integer"
            )
        if key_label is not None and value_is_valid and not key_is_sensitive:
            valid_thresholds[key_label] = value
    return valid_thresholds


def path_component_has_sensitive_label(component: str) -> bool:
    """Return whether a path component looks like runtime-only secret material."""

    sensitive_key_labels = (
        frozenset(key.lower() for key in SENSITIVE_KEYS) | COMMON_SENSITIVE_KEYS
    )
    normalized_sensitive_keys = frozenset(
        normalize_sensitive_key(key) for key in sensitive_key_labels
    )
    for variant in decoded_text_variants(component):
        stem = variant.rsplit(".", 1)[0]
        normalized_values = {
            normalize_sensitive_key(variant),
            normalize_sensitive_key(stem),
        }
        if any(value in normalized_sensitive_keys for value in normalized_values):
            return True
        if any(
            fragment in value
            for value in normalized_values
            for fragment in PATH_SENSITIVE_KEY_FRAGMENTS
        ):
            return True
    return False


def archive_path_component_is_portable(component: str) -> bool:
    """Return whether an archive path component is safe in raw or decoded form."""

    for variant in decoded_text_variants(component):
        if (
            canonical_string(variant) is None
            or variant in {".", ".."}
            or "/" in variant
            or "\\" in variant
            or path_component_has_windows_drive_prefix(variant)
            or path_component_has_uri_scheme_prefix(variant)
            or path_component_has_sensitive_label(variant)
        ):
            return False
    return True


def is_archive_portable_artifact_path(path: str) -> bool:
    """Return whether an artifact label is portable inside release archives."""

    if canonical_string(path) is None:
        return False
    if path.startswith(("/", "\\")) or "\\" in path:
        return False
    if len(path) >= 2 and path[1] == ":" and path[0].isalpha():
        return False
    return all(archive_path_component_is_portable(part) for part in path.split("/"))


def aggregate_summary_path_label(path: Path, evidence_dirs: list[Path]) -> str:
    """Return an archive-portable label for a lane summary input."""

    for directory in evidence_dirs:
        resolution_errors: list[str] = []
        resolved_path = resolve_path_identity(
            path, resolution_errors, label="summary path"
        )
        resolved_directory = resolve_path_identity(
            directory, resolution_errors, label="evidence directory"
        )
        if resolved_path is None or resolved_directory is None:
            continue
        try:
            relative_path = resolved_path.relative_to(resolved_directory)
        except ValueError:
            continue
        label = relative_path.as_posix()
        if is_archive_portable_artifact_path(label):
            return label
    name_label = path.name
    if is_archive_portable_artifact_path(name_label):
        return name_label
    return "summary.json"


def require_artifact_identity_fields(
    artifact: dict[str, Any],
    path: str,
    errors: list[str],
) -> None:
    """Require canonical artifact path and digest metadata."""

    artifact_path = canonical_string(artifact.get("path"))
    if artifact_path is None:
        errors.append(f"{path}.path must be canonical")
    elif not is_archive_portable_artifact_path(artifact_path):
        errors.append(
            f"{path}.path must be archive-relative without absolute, empty, "
            "current, parent, encoded, URI-scheme-like, platform-specific, "
            "or secret-looking segments"
        )
    sha256 = artifact.get("sha256")
    if (
        not isinstance(sha256, str)
        or len(sha256) != 64
        or any(character not in LOWER_HEX_DIGITS for character in sha256)
    ):
        errors.append(f"{path}.sha256 must be canonical lowercase SHA-256")


def require_optional_artifact_label(
    artifact: dict[str, Any],
    field: str,
    path: str,
    errors: list[str],
) -> None:
    """Require optional artifact metadata labels to be canonical."""

    if field in artifact and canonical_string(artifact.get(field)) is None:
        errors.append(f"{path}.{field} must be canonical when present")


def require_payload_free_summary_fields(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Reject non-standard top-level lane summary fields."""

    for key in payload:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append("summary keys must be canonical strings")
        elif key_label not in PAYLOAD_FREE_SUMMARY_FIELDS:
            key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
            errors.append(
                f"{key_diagnostic_label} is not allowed in payload-free lane summary"
            )


def validate_payload_free_metadata_value(
    value: Any,
    path: str,
    errors: list[str],
    *,
    depth: int = 0,
) -> None:
    """Validate payload-free lane metadata carried beside required rows."""

    if depth > MAX_SUMMARY_METADATA_DEPTH:
        errors.append(f"{path} nesting exceeds {MAX_SUMMARY_METADATA_DEPTH} levels")
        return
    if isinstance(value, bool):
        return
    if isinstance(value, int):
        if value < 0:
            errors.append(f"{path} must contain only non-negative integers")
        return
    if isinstance(value, str):
        if canonical_string(value) is None:
            errors.append(f"{path} must contain only canonical strings")
        return
    if isinstance(value, list):
        for index, item in enumerate(value):
            validate_payload_free_metadata_value(
                item,
                f"{path}[{index}]",
                errors,
                depth=depth + 1,
            )
        return
    if isinstance(value, dict):
        for key, item in value.items():
            key_label = canonical_string(key)
            if key_label is None:
                errors.append(f"{path} keys must be canonical strings")
                continue
            key_path_label = payload_free_diagnostic_key_label(key_label)
            validate_payload_free_metadata_value(
                item,
                f"{path}.{key_path_label}",
                errors,
                depth=depth + 1,
            )
        return
    errors.append(f"{path} must contain only payload-free canonical metadata")


def payload_free_object_list_metadata_identity(
    item: Any,
    schema: dict[str, Any],
) -> tuple[tuple[str, Any], ...] | None:
    """Return an exact comparable identity for validated object-list metadata."""

    if not isinstance(item, dict):
        return None
    string_fields = schema.get("strings", frozenset())
    positive_int_fields = schema.get("positive_ints", frozenset())
    hex_fields = schema.get("hex", {})
    allowed_fields = set(string_fields) | set(positive_int_fields) | set(hex_fields)
    if set(item) != allowed_fields:
        return None
    identity: list[tuple[str, Any]] = []
    for key in sorted(string_fields):
        value = canonical_string(item.get(key))
        if value is None:
            return None
        identity.append((key, value))
    for key in sorted(positive_int_fields):
        value = item.get(key)
        if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
            return None
        identity.append((key, value))
    for key, expected_hex_length in sorted(hex_fields.items()):
        value = item.get(key)
        if (
            not isinstance(value, str)
            or len(value) != expected_hex_length
            or any(character not in LOWER_HEX_DIGITS for character in value)
        ):
            return None
        identity.append((key, value))
    for start_key, end_key in schema.get("ordered_int_pairs", ()):
        start_value = item.get(start_key)
        end_value = item.get(end_key)
        if (
            isinstance(start_value, int)
            and not isinstance(start_value, bool)
            and isinstance(end_value, int)
            and not isinstance(end_value, bool)
            and start_value > 0
            and end_value > 0
            and end_value < start_value
        ):
            return None
    return tuple(identity)


def payload_free_object_list_metadata_domain_identity(
    field: str,
    item: Any,
    schema: dict[str, Any],
) -> tuple[tuple[str, Any], ...] | None:
    """Return the domain identity for validated object-list metadata."""

    if payload_free_object_list_metadata_identity(item, schema) is None:
        return None
    identity_fields = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_DOMAIN_IDENTITY_FIELDS.get(field)
    if identity_fields is None:
        return None
    string_fields = schema.get("strings", frozenset())
    positive_int_fields = schema.get("positive_ints", frozenset())
    hex_fields = schema.get("hex", {})
    identity: list[tuple[str, Any]] = []
    for key in identity_fields:
        if key in string_fields:
            value = canonical_string(item.get(key))
            if value is None:
                return None
        elif key in positive_int_fields:
            value = item.get(key)
            if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
                return None
        elif key in hex_fields:
            value = item.get(key)
            expected_hex_length = hex_fields[key]
            if (
                not isinstance(value, str)
                or len(value) != expected_hex_length
                or any(character not in LOWER_HEX_DIGITS for character in value)
            ):
                return None
        else:
            return None
        identity.append((key, value))
    return tuple(identity)


def validate_payload_free_object_list_metadata(
    field: str,
    value: Any,
    schema: dict[str, Any],
    errors: list[str],
) -> None:
    """Validate exact payload-free object-list metadata carried by lane summaries."""

    string_fields = schema.get("strings", frozenset())
    positive_int_fields = schema.get("positive_ints", frozenset())
    hex_fields = schema.get("hex", {})
    allowed_fields = set(string_fields) | set(positive_int_fields) | set(hex_fields)
    identities: list[tuple[tuple[str, Any], ...]] = []
    domain_identities: list[tuple[tuple[str, Any], ...]] = []
    for index, item in enumerate(value):
        item_path = f"{field}[{index}]"
        if not isinstance(item, dict):
            errors.append(f"{item_path} must be a payload-free metadata object")
            continue
        identity = payload_free_object_list_metadata_identity(item, schema)
        if identity is not None:
            identities.append(identity)
        domain_identity = payload_free_object_list_metadata_domain_identity(
            field,
            item,
            schema,
        )
        if domain_identity is not None:
            domain_identities.append(domain_identity)
        for key in item:
            key_label = canonical_string(key)
            if key_label is None:
                errors.append(f"{item_path} keys must be canonical strings")
            elif key_label not in allowed_fields:
                key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
                errors.append(
                    f"{item_path}.{key_diagnostic_label} is not allowed in payload-free object metadata"
                )
        for key in sorted(string_fields):
            if canonical_string(item.get(key)) is None:
                errors.append(f"{item_path}.{key} must be a canonical string")
        for key in sorted(positive_int_fields):
            item_value = item.get(key)
            if (
                not isinstance(item_value, int)
                or isinstance(item_value, bool)
                or item_value <= 0
            ):
                errors.append(f"{item_path}.{key} must be a positive integer")
        for key, expected_hex_length in sorted(hex_fields.items()):
            item_value = item.get(key)
            if (
                not isinstance(item_value, str)
                or len(item_value) != expected_hex_length
                or any(character not in LOWER_HEX_DIGITS for character in item_value)
            ):
                errors.append(
                    f"{item_path}.{key} must be {expected_hex_length} lowercase hex characters"
                )
        for start_key, end_key in schema.get("ordered_int_pairs", ()):
            start_value = item.get(start_key)
            end_value = item.get(end_key)
            if (
                isinstance(start_value, int)
                and not isinstance(start_value, bool)
                and isinstance(end_value, int)
                and not isinstance(end_value, bool)
                and start_value > 0
                and end_value > 0
                and end_value < start_value
            ):
                errors.append(f"{item_path}.{end_key} must be >= {start_key}")
    if len(set(identities)) != len(identities):
        errors.append(f"{field} must not contain duplicate metadata entries")
    if len(set(domain_identities)) != len(domain_identities):
        errors.append(f"{field} must not contain duplicate metadata identities")


def validate_payload_free_object_metadata(
    field: str,
    value: Any,
    allowed_fields: frozenset[str],
    errors: list[str],
) -> None:
    """Validate exact payload-free object metadata carried by lane summaries."""

    if not isinstance(value, dict):
        errors.append(f"{field} must be a payload-free metadata object")
        return
    for key in value:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(f"{field} keys must be canonical strings")
        elif key_label not in allowed_fields:
            key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
            errors.append(
                f"{field}.{key_diagnostic_label} is not allowed in payload-free object metadata"
            )
    for key in sorted(allowed_fields):
        if canonical_string(value.get(key)) is None:
            errors.append(f"{field}.{key} must be a canonical string")


def payload_free_metadata_deployment_context(
    value: Any,
) -> tuple[str, str] | None:
    """Return a canonical deployment context carried by payload-free metadata."""

    if not isinstance(value, dict):
        return None
    deployment_id = canonical_string(value.get("deployment_id"))
    environment = canonical_string(value.get("environment"))
    if deployment_id is None or environment is None:
        return None
    return deployment_id, environment


def payload_free_summary_metadata_deployment_contexts(
    gate: GateSummaryKind,
    payload: dict[str, Any],
) -> set[tuple[str, str]]:
    """Return deployment contexts carried by validated top-level lane metadata."""

    allowed_metadata_fields = GATE_METADATA_FIELDS.get(gate.name, frozenset())
    contexts: set[tuple[str, str]] = set()
    if "deployment_context" in allowed_metadata_fields:
        context = payload_free_metadata_deployment_context(
            payload.get("deployment_context")
        )
        if context is not None:
            contexts.add(context)
    for field, schema in PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS.items():
        if field not in allowed_metadata_fields:
            continue
        string_fields = schema.get("strings", frozenset())
        if not {"deployment_id", "environment"} <= set(string_fields):
            continue
        value = payload.get(field)
        if not isinstance(value, list):
            continue
        for item in value:
            context = payload_free_metadata_deployment_context(item)
            if context is not None:
                contexts.add(context)
    return contexts


def canonical_lower_hex(value: Any, expected_hex_length: int) -> str | None:
    """Return a lowercase hex string of the expected length, or None."""

    if (
        isinstance(value, str)
        and len(value) == expected_hex_length
        and all(character in LOWER_HEX_DIGITS for character in value)
    ):
        return value
    return None


def _nonzero_sha256(
    value: Any,
    *,
    path: str,
    errors: list[str],
) -> str | None:
    """Return one canonical non-zero SHA-256 digest."""

    digest = canonical_lower_hex(value, 64)
    if digest is None:
        errors.append(f"{path} must be canonical lowercase SHA-256")
        return None
    if not any(bytes.fromhex(digest)):
        errors.append(f"{path} must not be zero")
        return None
    return digest


def _resilience_timestamp(
    value: Any,
    *,
    path: str,
    now_unix: int,
    max_age_secs: int,
    errors: list[str],
) -> int | None:
    """Validate one fresh positive resilience evidence timestamp."""

    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value <= 0
        or value > MAX_FOUNDATIONAL_RELEASE_SEQUENCE
    ):
        errors.append(f"{path} must be an integer in 1..2^63-1")
        return None
    if value > now_unix:
        errors.append(f"{path} must not be future")
    elif now_unix - value > max_age_secs:
        errors.append(f"{path} exceeds max summary artifact age")
    return value


def validate_resilience_qualification_binding_object(
    value: Any,
    errors: list[str],
    *,
    path: str,
    expected: Mapping[str, Any] | None = None,
) -> dict[str, Any] | None:
    """Validate the signed envelope's payload-free resilience anchor."""

    binding = validate_foundational_exact_fields(
        value,
        RESILIENCE_QUALIFICATION_BINDING_FIELDS,
        path,
        errors,
    )
    if binding is None:
        return None
    if binding.get("schema") != RESILIENCE_QUALIFICATION_BINDING_SCHEMA:
        errors.append(f"{path}.schema must match the resilience binding contract")
    software_signer_evidence.validate_aggregate_software_signer(binding, errors)
    for field in (
        "summary_sha256",
        "receipt_sha256",
        "canonical_receipt_sha256",
        "signer_public_key_fingerprint_sha256",
    ):
        _nonzero_sha256(
            binding.get(field),
            path=f"{path}.{field}",
            errors=errors,
        )
    generated_at_unix = binding.get("receipt_generated_at_unix")
    if (
        not isinstance(generated_at_unix, int)
        or isinstance(generated_at_unix, bool)
        or generated_at_unix <= 0
        or generated_at_unix > MAX_FOUNDATIONAL_RELEASE_SEQUENCE
    ):
        errors.append(
            f"{path}.receipt_generated_at_unix must be an integer in 1..2^63-1"
        )
    if expected is not None and dict(binding) != dict(expected):
        errors.append(f"{path} must match the reviewed resilience qualification")
    return dict(binding)


def load_resilience_qualification_binding(
    path: Path,
    *,
    expected_deployment_id: str | None,
    expected_environment: str | None,
    expected_topology_qualification: Mapping[str, str] | None,
    now_unix: int,
    max_age_secs: int,
    trusted_public_key: bytes | None,
) -> tuple[dict[str, Any] | None, list[str]]:
    """Authenticate one evidence-qualified resilience summary and return its anchor."""

    errors: list[str] = []
    loaded = load_evidence_json_with_sha256_or_record_error(
        path,
        MAX_SUMMARY_BYTES,
        errors,
    )
    if loaded is None:
        return None, errors
    summary, summary_sha256 = loaded
    validate_foundational_exact_fields(
        summary,
        RESILIENCE_QUALIFICATION_SUMMARY_FIELDS,
        "resilience qualification summary",
        errors,
    )
    if summary.get("schema") != RESILIENCE_QUALIFICATION_SUMMARY_SCHEMA:
        errors.append("resilience qualification summary schema must match the contract")
    if summary.get("status") != "evidence-qualified":
        errors.append("resilience qualification summary status must be `evidence-qualified`")
    if summary.get("qualification_scope") != "holistic-deployment-resilience":
        errors.append(
            "resilience qualification summary scope must be holistic-deployment-resilience"
        )
    for field in (
        "live_evidence_recognized",
        "externally_authenticated",
        "promotion_eligible",
    ):
        if summary.get(field) is not True:
            errors.append(f"resilience qualification summary {field} must be true")
    if summary.get("readiness_lane_count_delta") != 0:
        errors.append(
            "resilience qualification summary readiness_lane_count_delta must be zero"
        )
    if summary.get("errors") != []:
        errors.append("resilience qualification summary errors must be empty")

    receipt_sha256 = _nonzero_sha256(
        summary.get("receipt_sha256"),
        path="resilience qualification summary receipt_sha256",
        errors=errors,
    )
    canonical_receipt_digest = _nonzero_sha256(
        summary.get("canonical_receipt_sha256"),
        path="resilience qualification summary canonical_receipt_sha256",
        errors=errors,
    )
    generated_at_unix = _resilience_timestamp(
        summary.get("receipt_generated_at_unix"),
        path="resilience qualification summary receipt_generated_at_unix",
        now_unix=now_unix,
        max_age_secs=max_age_secs,
        errors=errors,
    )

    deployment = validate_foundational_exact_fields(
        summary.get("deployment"),
        FOUNDATIONAL_PREREQUISITE_DEPLOYMENT_FIELDS,
        "resilience qualification summary deployment",
        errors,
    )
    deployment_id: str | None = None
    environment: str | None = None
    if deployment is not None:
        deployment_errors: list[str] = []
        candidate_deployment_id = require_production_deployment_id_value(
            deployment.get("deployment_id"),
            deployment_errors,
            "resilience qualification summary deployment_id",
        )
        errors.extend(deployment_errors)
        candidate_environment = canonical_string(deployment.get("environment"))
        if candidate_environment is None:
            errors.append(
                "resilience qualification summary environment must be canonical"
            )
        elif not is_production_ready_environment(candidate_environment):
            errors.append(
                "resilience qualification summary environment must be production"
            )
        if candidate_deployment_id:
            deployment_id = candidate_deployment_id
        if candidate_environment is not None:
            environment = candidate_environment
        if (
            expected_deployment_id is not None
            and deployment_id != expected_deployment_id
        ):
            errors.append(
                "resilience qualification summary deployment_id must match --deployment-id"
            )
        if expected_environment is not None and environment != expected_environment:
            errors.append(
                "resilience qualification summary environment must match --environment"
            )

    topology = summary.get("topology_qualification")
    errors.extend(
        validate_topology_binding_object(
            topology,
            expected=expected_topology_qualification,
            path="resilience qualification summary topology_qualification",
        )
    )

    required_requirements = summary.get("required_requirements")
    if required_requirements != list(RESILIENCE_QUALIFICATION_REQUIREMENTS):
        errors.append(
            "resilience qualification summary requirements must match the exact "
            "19-requirement contract in canonical order"
        )
    recognized_count = summary.get("recognized_requirement_count")
    if recognized_count != len(RESILIENCE_QUALIFICATION_REQUIREMENTS):
        errors.append(
            "resilience qualification summary recognized_requirement_count must be 19"
        )

    artifacts_value = summary.get("artifact_bindings")
    artifacts: list[dict[str, Any]] = []
    captures: list[int] = []
    artifact_paths: list[str] = []
    artifact_digests: list[str] = []
    observed_requirements: list[str] = []
    if not isinstance(artifacts_value, list):
        errors.append("resilience qualification summary artifact_bindings must be a list")
    else:
        for index, value in enumerate(artifacts_value):
            row_path = f"resilience qualification summary artifact_bindings[{index}]"
            row = validate_foundational_exact_fields(
                value,
                RESILIENCE_QUALIFICATION_ARTIFACT_BINDING_FIELDS,
                row_path,
                errors,
            )
            if row is None:
                continue
            requirement = canonical_string(row.get("requirement"))
            if requirement is None:
                errors.append(f"{row_path}.requirement must be canonical")
            else:
                observed_requirements.append(requirement)
            artifact_path = canonical_string(row.get("artifact_path"))
            if (
                artifact_path is None
                or not is_archive_portable_artifact_path(artifact_path)
            ):
                errors.append(f"{row_path}.artifact_path must be archive-relative and portable")
            else:
                artifact_paths.append(artifact_path)
            artifact_digest = _nonzero_sha256(
                row.get("artifact_sha256"),
                path=f"{row_path}.artifact_sha256",
                errors=errors,
            )
            if artifact_digest is not None:
                artifact_digests.append(artifact_digest)
            captured_at_unix = _resilience_timestamp(
                row.get("captured_at_unix"),
                path=f"{row_path}.captured_at_unix",
                now_unix=now_unix,
                max_age_secs=max_age_secs,
                errors=errors,
            )
            if captured_at_unix is not None:
                captures.append(captured_at_unix)
                if (
                    generated_at_unix is not None
                    and captured_at_unix > generated_at_unix
                ):
                    errors.append(
                        f"{row_path}.captured_at_unix must not exceed receipt time"
                    )
            artifacts.append(dict(row))
    if observed_requirements != list(RESILIENCE_QUALIFICATION_REQUIREMENTS):
        errors.append(
            "resilience qualification summary artifact requirements must match "
            "the exact canonical order"
        )
    if len(artifact_paths) != len(set(artifact_paths)):
        errors.append(
            "resilience qualification summary artifact paths must be unique"
        )
    if len(artifact_digests) != len(set(artifact_digests)):
        errors.append(
            "resilience qualification summary artifact digests must be unique"
        )
    earliest_capture = min(captures) if captures else None
    latest_capture = max(captures) if captures else None
    if summary.get("earliest_capture_unix") != earliest_capture:
        errors.append(
            "resilience qualification summary earliest_capture_unix must match artifacts"
        )
    if summary.get("latest_capture_unix") != latest_capture:
        errors.append(
            "resilience qualification summary latest_capture_unix must match artifacts"
        )

    authentication = validate_foundational_exact_fields(
        summary.get("receipt_authentication"),
        RESILIENCE_QUALIFICATION_AUTHENTICATION_FIELDS,
        "resilience qualification summary receipt_authentication",
        errors,
    )
    signer_fingerprint: str | None = None
    signature_bytes: bytes | None = None
    if authentication is not None:
        if authentication.get("kind") != "external-ed25519":
            errors.append(
                "resilience qualification summary authentication kind must be "
                "`external-ed25519`"
            )
        if authentication.get("algorithm") != "ed25519":
            errors.append(
                "resilience qualification summary authentication algorithm must be "
                "`ed25519`"
            )
        signer_fingerprint = _nonzero_sha256(
            authentication.get("public_key_fingerprint_sha256"),
            path=(
                "resilience qualification summary authentication "
                "public_key_fingerprint_sha256"
            ),
            errors=errors,
        )
        signature_hex = canonical_lower_hex(authentication.get("signature_hex"), 128)
        if signature_hex is None or not any(bytes.fromhex(signature_hex)):
            errors.append(
                "resilience qualification summary authentication signature must "
                "be a non-zero canonical Ed25519 signature"
            )
        else:
            signature_bytes = bytes.fromhex(signature_hex)
    signer_provenance = software_signer_evidence.validate_foundational_software_signer(
        dict(authentication) if authentication is not None else None, errors)

    if (
        not isinstance(trusted_public_key, bytes)
        or len(trusted_public_key) != 32
        or not any(trusted_public_key)
    ):
        errors.append(
            "resilience qualification summary requires an operator-trusted "
            "Ed25519 public key"
        )
        trusted_public_key = None
    else:
        expected_fingerprint = hashlib.sha256(trusted_public_key).hexdigest()
        if (
            signer_fingerprint is not None
            and signer_fingerprint != expected_fingerprint
        ):
            errors.append(
                "resilience qualification summary signer must match the "
                "operator-trusted key"
            )

    reconstructed_receipt: dict[str, Any] | None = None
    if (
        deployment is not None
        and isinstance(topology, Mapping)
        and generated_at_unix is not None
        and authentication is not None
    ):
        reconstructed_receipt = {
            "schema": RESILIENCE_QUALIFICATION_RECEIPT_SCHEMA,
            "deployment": dict(deployment),
            "topology_qualification": dict(topology),
            "generated_at_unix": generated_at_unix,
            "artifacts": artifacts,
            "authentication": dict(authentication),
        }
        reconstructed_digest = hashlib.sha256(
            json.dumps(
                reconstructed_receipt,
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=False,
                allow_nan=False,
            ).encode("utf-8")
        ).hexdigest()
        if (
            canonical_receipt_digest is not None
            and reconstructed_digest != canonical_receipt_digest
        ):
            errors.append(
                "resilience qualification summary canonical receipt digest "
                "does not match reconstructed receipt"
            )

    if (
        reconstructed_receipt is not None
        and trusted_public_key is not None
        and signature_bytes is not None
    ):
        unsigned_receipt = dict(reconstructed_receipt)
        unsigned_authentication = dict(authentication)
        unsigned_authentication.pop("signature_hex", None)
        unsigned_receipt["authentication"] = unsigned_authentication
        signing_payload = RESILIENCE_QUALIFICATION_SIGNATURE_DOMAIN + json.dumps(
            unsigned_receipt,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        ).encode("ascii")
        try:
            signature_valid = verify_ed25519(
                trusted_public_key,
                signature_bytes,
                signing_payload,
            )
        except (TypeError, ValueError):
            signature_valid = False
        if not signature_valid:
            errors.append(
                "resilience qualification summary receipt signature verification failed"
            )

    if (
        receipt_sha256 is None
        or canonical_receipt_digest is None
        or generated_at_unix is None
        or signer_fingerprint is None or any(value is None for value in signer_provenance.values())
    ):
        return None, errors
    return (
        {
            "schema": RESILIENCE_QUALIFICATION_BINDING_SCHEMA,
            "summary_sha256": summary_sha256,
            "receipt_sha256": receipt_sha256,
            "canonical_receipt_sha256": canonical_receipt_digest,
            "receipt_generated_at_unix": generated_at_unix,
            **signer_provenance,
            "signer_public_key_fingerprint_sha256": signer_fingerprint,
        },
        errors,
    )


def validate_foundational_exact_fields(
    value: Any,
    expected_fields: frozenset[str],
    path: str,
    errors: list[str],
) -> dict[str, Any] | None:
    """Require one schema-closed object and sanitize unknown-key diagnostics."""

    if not isinstance(value, dict):
        errors.append(f"{path} must be an object")
        return None
    observed_fields = set(value)
    if observed_fields != expected_fields:
        errors.append(f"{path} fields must match the schema-closed contract")
    for key in value:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(f"{path} keys must be canonical strings")
        elif key_label not in expected_fields:
            errors.append(
                f"{path}.{payload_free_diagnostic_key_label(key_label)} is not allowed"
            )
    return value


def validate_foundational_timestamp(
    value: Any,
    path: str,
    options: ValidationOptions,
    errors: list[str],
) -> int | None:
    """Validate one reviewed prerequisite timestamp against the aggregate clock."""

    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        errors.append(f"{path} must be a positive integer")
        return None
    if value > options.now_unix:
        errors.append(f"{path} must not be future")
    elif options.now_unix - value > options.max_summary_artifact_age_secs:
        errors.append(f"{path} exceeds max summary artifact age")
    return value


def validate_foundational_lane_summary_rows(
    value: Any,
    errors: list[str],
) -> list[dict[str, str]]:
    """Validate the exact ordered lane-summary digests signed by the envelope."""

    rows: list[dict[str, str]] = []
    if not isinstance(value, list):
        errors.append("foundational lane_summaries must be an array")
        return rows
    for index, item in enumerate(value):
        path = f"foundational lane_summaries[{index}]"
        row = validate_foundational_exact_fields(
            item,
            FOUNDATIONAL_LANE_SUMMARY_ROW_FIELDS,
            path,
            errors,
        )
        if row is None:
            continue
        gate = canonical_string(row.get("gate"))
        if gate is None:
            errors.append(f"{path}.gate must be a canonical string")
        digest = canonical_lower_hex(row.get("sha256"), 64)
        if digest is None:
            errors.append(f"{path}.sha256 must be canonical lowercase SHA-256")
        elif not any(bytes.fromhex(digest)):
            errors.append(f"{path}.sha256 must not be zero")
        if gate is not None and digest is not None and any(bytes.fromhex(digest)):
            rows.append({"gate": gate, "sha256": digest})

    observed_gates = [row["gate"] for row in rows]
    expected_gates = list(DEFAULT_REQUIRED_GATES)
    if observed_gates != expected_gates:
        errors.append(
            "foundational lane_summaries must match all 17 readiness lanes in canonical order"
        )
        if len(set(observed_gates)) != len(observed_gates):
            errors.append("foundational lane_summaries must not contain duplicate gates")
        if set(expected_gates) - set(observed_gates):
            errors.append("foundational lane_summaries are missing required gates")
        if set(observed_gates) - set(expected_gates):
            errors.append("foundational lane_summaries contain unknown gates")
    digests = [row["sha256"] for row in rows]
    if len(set(digests)) != len(digests):
        errors.append("foundational lane_summaries must use unique summary digests")
    return rows


def validate_foundational_prerequisite_readiness_summary_rows(
    value: Any,
    prerequisite_id: str | None,
    errors: list[str],
    *,
    path: str,
) -> list[dict[str, str]]:
    """Validate one prerequisite's exact ordered readiness-summary digest group."""

    rows: list[dict[str, str]] = []
    if not isinstance(value, list):
        errors.append(f"{path} must be an array")
        return rows
    for index, item in enumerate(value):
        row_path = f"{path}[{index}]"
        row = validate_foundational_exact_fields(
            item,
            FOUNDATIONAL_LANE_SUMMARY_ROW_FIELDS,
            row_path,
            errors,
        )
        if row is None:
            continue
        gate = canonical_string(row.get("gate"))
        if gate is None:
            errors.append(f"{row_path}.gate must be a canonical string")
        digest = canonical_lower_hex(row.get("sha256"), 64)
        if digest is None:
            errors.append(f"{row_path}.sha256 must be canonical lowercase SHA-256")
        elif not any(bytes.fromhex(digest)):
            errors.append(f"{row_path}.sha256 must not be zero")
        if gate is not None and digest is not None and any(bytes.fromhex(digest)):
            rows.append({"gate": gate, "sha256": digest})

    expected_gates = FOUNDATIONAL_PREREQUISITE_LANES.get(prerequisite_id)
    observed_gates = [row["gate"] for row in rows]
    if expected_gates is None:
        errors.append(f"{path} requires a known prerequisite id")
    elif observed_gates != list(expected_gates):
        errors.append(
            f"{path} must match the exact canonical readiness lanes for its prerequisite id"
        )
        if len(set(observed_gates)) != len(observed_gates):
            errors.append(f"{path} must not contain duplicate gates")
        if set(expected_gates) - set(observed_gates):
            errors.append(f"{path} is missing required gates")
        if set(observed_gates) - set(expected_gates):
            errors.append(f"{path} contains unknown gates")
    digests = [row["sha256"] for row in rows]
    if len(set(digests)) != len(digests):
        errors.append(f"{path} must use unique summary digests")
    return rows


def validate_foundational_grouped_lane_summary_digest_bindings(
    grouped_rows: list[dict[str, Any]],
    lane_summary_rows: list[dict[str, str]],
    errors: list[str],
    *,
    path: str,
) -> None:
    """Require every grouped prerequisite digest to equal the signed global row."""

    global_by_gate = {
        row["gate"]: row["sha256"]
        for row in lane_summary_rows
        if set(row) == FOUNDATIONAL_LANE_SUMMARY_ROW_FIELDS
    }
    for group in grouped_rows:
        rows = group.get("readiness_summary_sha256")
        if canonical_string(group.get("id")) is None or not isinstance(rows, list):
            continue
        for row in rows:
            if not isinstance(row, dict):
                continue
            gate = canonical_string(row.get("gate"))
            digest = canonical_lower_hex(row.get("sha256"), 64)
            if gate is None or digest is None:
                continue
            if global_by_gate.get(gate) != digest:
                errors.append(
                    f"{path} grouped digest must match foundational lane_summaries"
                )


def validate_foundational_prerequisite_summary(
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[dict[str, Any], list[str], tuple[str, str] | None]:
    """Validate the signed, payload-free foundational prerequisite envelope."""

    errors: list[str] = []
    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys=SENSITIVE_KEYS,
        evidence_label="SoraFS foundational prerequisite summary",
    )
    validate_foundational_exact_fields(
        payload,
        FOUNDATIONAL_PREREQUISITE_FIELDS,
        "foundational prerequisite summary",
        errors,
    )

    if payload.get("schema") != FOUNDATIONAL_PREREQUISITE_SCHEMA:
        errors.append("foundational prerequisite schema must match the contract")
    if payload.get("status") != "verified":
        errors.append("foundational prerequisite status must be `verified`")

    deployment = validate_foundational_exact_fields(
        payload.get("deployment"),
        FOUNDATIONAL_PREREQUISITE_DEPLOYMENT_FIELDS,
        "foundational prerequisite deployment",
        errors,
    )
    deployment_id: str | None = None
    environment: str | None = None
    if deployment is not None:
        deployment_field_errors: list[str] = []
        candidate_deployment_id = require_production_deployment_id_value(
            deployment.get("deployment_id"),
            deployment_field_errors,
            "foundational prerequisite deployment_id",
        )
        errors.extend(deployment_field_errors)
        candidate_environment = canonical_string(deployment.get("environment"))
        if candidate_environment is None:
            errors.append(
                "foundational prerequisite environment must be a canonical string"
            )
        elif not is_production_ready_environment(candidate_environment):
            errors.append("foundational prerequisite environment must be production")
        if candidate_deployment_id:
            deployment_id = candidate_deployment_id
        if candidate_environment is not None:
            environment = candidate_environment
        if (
            deployment_id is not None
            and options.deployment_id is not None
            and deployment_id != options.deployment_id
        ):
            errors.append(
                "foundational prerequisite deployment_id must match --deployment-id"
            )
        if (
            environment is not None
            and options.environment is not None
            and environment != options.environment
        ):
            errors.append(
                "foundational prerequisite environment must match --environment"
            )

    generated_at_unix = validate_foundational_timestamp(
        payload.get("generated_at_unix"),
        "foundational prerequisite generated_at_unix",
        options,
        errors,
    )

    release_sequence_value = payload.get("release_sequence")
    release_sequence: int | None = None
    if (
        not isinstance(release_sequence_value, int)
        or isinstance(release_sequence_value, bool)
        or release_sequence_value <= 0
        or release_sequence_value > MAX_FOUNDATIONAL_RELEASE_SEQUENCE
    ):
        errors.append(
            "foundational prerequisite release_sequence must be an integer in 1..2^63-1"
        )
    else:
        release_sequence = release_sequence_value
        if options.foundational_release_sequence is None:
            errors.append(
                "foundational prerequisite release_sequence requires an operator-reviewed expected value"
            )
        elif release_sequence != options.foundational_release_sequence:
            errors.append(
                "foundational prerequisite release_sequence must match the operator-reviewed expected value"
            )

    previous_envelope_sha256 = canonical_lower_hex(
        payload.get("previous_envelope_sha256"),
        64,
    )
    if previous_envelope_sha256 is None:
        errors.append(
            "foundational prerequisite previous_envelope_sha256 must be canonical lowercase SHA-256"
        )
    else:
        is_zero_predecessor = not any(bytes.fromhex(previous_envelope_sha256))
        if release_sequence == 1 and not is_zero_predecessor:
            errors.append(
                "foundational prerequisite sequence 1 must use the zero predecessor"
            )
        if release_sequence is not None and release_sequence > 1 and is_zero_predecessor:
            errors.append(
                "foundational prerequisite sequence after 1 must use a non-zero predecessor"
            )
        expected_predecessor = options.foundational_previous_envelope_sha256
        if expected_predecessor is None:
            errors.append(
                "foundational prerequisite predecessor requires an operator-reviewed expected digest"
            )
        elif previous_envelope_sha256 != expected_predecessor:
            errors.append(
                "foundational prerequisite previous_envelope_sha256 must match the operator-reviewed expected digest"
            )

    inventory_binding = (
        options.l1_lane_evidence_inventory.verification
        if options.l1_lane_evidence_inventory is not None
        else None
    )
    l1_lane_evidence_inventory_sha256 = validate_foundational_inventory_digest(
        payload.get("l1_lane_evidence_inventory_sha256"),
        inventory_binding,
        errors,
        path="foundational prerequisite l1_lane_evidence_inventory_sha256",
    )

    prerequisites = payload.get("prerequisites")
    prerequisite_ids: list[str] = []
    anchors: list[str] = []
    evidence_generated_times: list[int] = []
    prerequisite_readiness_summary_rows: list[dict[str, Any]] = []
    if not isinstance(prerequisites, list):
        errors.append("foundational prerequisites must be an array")
    else:
        for index, item in enumerate(prerequisites):
            path = f"foundational prerequisites[{index}]"
            row = validate_foundational_exact_fields(
                item,
                FOUNDATIONAL_PREREQUISITE_ROW_FIELDS,
                path,
                errors,
            )
            if row is None:
                continue
            prerequisite_id = canonical_string(row.get("id"))
            if prerequisite_id is None:
                errors.append(f"{path}.id must be a canonical string")
            else:
                prerequisite_ids.append(prerequisite_id)
            readiness_summary_rows = (
                validate_foundational_prerequisite_readiness_summary_rows(
                    row.get("readiness_summary_sha256"),
                    prerequisite_id,
                    errors,
                    path=f"{path}.readiness_summary_sha256",
                )
            )
            if prerequisite_id in FOUNDATIONAL_PREREQUISITE_LANES:
                prerequisite_readiness_summary_rows.append(
                    {
                        "id": prerequisite_id,
                        "readiness_summary_sha256": readiness_summary_rows,
                    }
                )
            if row.get("status") != "verified":
                errors.append(f"{path}.status must be `verified`")
            anchor = canonical_lower_hex(row.get("evidence_anchor_sha256"), 64)
            if anchor is None:
                errors.append(
                    f"{path}.evidence_anchor_sha256 must be canonical lowercase SHA-256"
                )
            elif not any(bytes.fromhex(anchor)):
                errors.append(f"{path}.evidence_anchor_sha256 must not be zero")
            else:
                anchors.append(anchor)
            evidence_generated_at = validate_foundational_timestamp(
                row.get("evidence_generated_at_unix"),
                f"{path}.evidence_generated_at_unix",
                options,
                errors,
            )
            if evidence_generated_at is not None:
                evidence_generated_times.append(evidence_generated_at)
                if (
                    generated_at_unix is not None
                    and evidence_generated_at > generated_at_unix
                ):
                    errors.append(
                        f"{path}.evidence_generated_at_unix must not be later than the signed envelope"
                    )

    expected_prerequisite_ids = list(FOUNDATIONAL_PREREQUISITE_IDS)
    if prerequisite_ids != expected_prerequisite_ids:
        errors.append(
            "foundational prerequisites must match the exact required set and canonical order"
        )
        if len(set(prerequisite_ids)) != len(prerequisite_ids):
            errors.append("foundational prerequisites must not contain duplicate ids")
        if set(expected_prerequisite_ids) - set(prerequisite_ids):
            errors.append("foundational prerequisites are missing required ids")
        if set(prerequisite_ids) - set(expected_prerequisite_ids):
            errors.append("foundational prerequisites contain unknown ids")
    if len(set(anchors)) != len(anchors):
        errors.append("foundational prerequisites must use unique evidence anchors")

    lane_summary_rows = validate_foundational_lane_summary_rows(
        payload.get("lane_summaries"),
        errors,
    )
    validate_foundational_grouped_lane_summary_digest_bindings(
        prerequisite_readiness_summary_rows,
        lane_summary_rows,
        errors,
        path="foundational prerequisites readiness_summary_sha256",
    )
    topology_qualification = payload.get("topology_qualification")
    errors.extend(
        validate_authenticated_topology_binding_object(
            topology_qualification,
            expected=options.topology_qualification,
            path="foundational prerequisite topology_qualification",
        )
    )
    resilience_qualification = validate_resilience_qualification_binding_object(
        payload.get("resilience_qualification"),
        errors,
        path="foundational prerequisite resilience_qualification",
        expected=(
            None
            if options.resilience_qualification_errors
            else options.resilience_qualification
        ),
    )

    signature = validate_foundational_exact_fields(
        payload.get("signature"),
        FOUNDATIONAL_PREREQUISITE_SIGNATURE_FIELDS,
        "foundational prerequisite signature",
        errors,
    )
    signer_provenance = software_signer_evidence.validate_foundational_software_signer(
        signature, errors
    )
    signer_fingerprint: str | None = None
    trusted_public_key = options.foundational_signer_public_key
    if (
        not isinstance(trusted_public_key, bytes)
        or len(trusted_public_key) != 32
        or not any(trusted_public_key)
    ):
        errors.append(
            "foundational prerequisite signature requires an operator-trusted Ed25519 public key"
        )
        trusted_public_key = None
    else:
        signer_fingerprint = hashlib.sha256(trusted_public_key).hexdigest()
    signature_bytes: bytes | None = None
    if signature is not None:
        if signature.get("algorithm") != "ed25519":
            errors.append(
                "foundational prerequisite signature algorithm must be `ed25519`"
            )
        declared_fingerprint = canonical_lower_hex(
            signature.get("public_key_fingerprint_sha256"),
            64,
        )
        if declared_fingerprint is None:
            errors.append(
                "foundational prerequisite signer fingerprint must be canonical lowercase SHA-256"
            )
        elif (
            signer_fingerprint is not None
            and declared_fingerprint != signer_fingerprint
        ):
            errors.append(
                "foundational prerequisite signer fingerprint must match the operator-trusted key"
            )
        signature_hex = canonical_lower_hex(signature.get("signature_hex"), 128)
        if signature_hex is None:
            errors.append(
                "foundational prerequisite signature must be a non-zero canonical Ed25519 signature"
            )
        elif not any(bytes.fromhex(signature_hex)):
            errors.append(
                "foundational prerequisite signature must be a non-zero canonical Ed25519 signature"
            )
        else:
            signature_bytes = bytes.fromhex(signature_hex)
    if trusted_public_key is not None and signature_bytes is not None:
        try:
            signature_valid = verify_ed25519(
                trusted_public_key,
                signature_bytes,
                foundational_signing_payload(payload),
            )
        except (TypeError, ValueError):
            signature_valid = False
        if not signature_valid:
            errors.append("foundational prerequisite signature verification failed")
    signer_provenance["signer_public_key_fingerprint_sha256"] = signer_fingerprint
    errors.extend(
        validate_signer_independence(
            ("topology", topology_qualification),
            ("resilience", resilience_qualification),
            ("L1 lane inventory", inventory_binding),
            ("promotion", signer_provenance),
        )
    )

    context = (
        (deployment_id, environment)
        if deployment_id is not None and environment is not None
        else None
    )
    summary = {
        "schema": FOUNDATIONAL_PREREQUISITE_SCHEMA,
        "present": True,
        "valid": not errors,
        "required_ids": expected_prerequisite_ids,
        "prerequisite_count": len(prerequisite_ids),
        "generated_at_unix": generated_at_unix,
        "oldest_evidence_generated_at_unix": (
            min(evidence_generated_times) if evidence_generated_times else None
        ),
        "newest_evidence_generated_at_unix": (
            max(evidence_generated_times) if evidence_generated_times else None
        ),
        "deployment_id": deployment_id,
        "environment": environment,
        "release_sequence": release_sequence,
        "previous_envelope_sha256": previous_envelope_sha256,
        "l1_lane_evidence_inventory_sha256": (
            l1_lane_evidence_inventory_sha256
        ),
        **signer_provenance,
        "topology_qualification": topology_qualification,
        "resilience_qualification": resilience_qualification,
        "evidence_anchor_sha256": anchors,
        "prerequisite_readiness_summary_sha256": (
            prerequisite_readiness_summary_rows
        ),
        "lane_summary_sha256": lane_summary_rows,
        "errors": errors,
    }
    return summary, errors, context


def payload_free_summary_artifact_fingerprints(
    payload: dict[str, Any],
    *,
    kind_name: str | None = None,
    kind_names: tuple[str, ...] | None = None,
) -> list[dict[str, Any]]:
    """Return recognized artifact fingerprints that can anchor metadata."""

    artifacts = payload.get("recognized_artifacts")
    if not isinstance(artifacts, list):
        return []
    fingerprints: list[dict[str, Any]] = []
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            continue
        if kind_name is not None and artifact.get("kind") != kind_name:
            continue
        if kind_names is not None and artifact.get("kind") not in kind_names:
            continue
        fingerprint = artifact.get("fingerprint")
        if isinstance(fingerprint, dict):
            fingerprints.append(fingerprint)
    return fingerprints


def payload_free_hex_list_metadata_values(
    field: str,
    value: Any,
) -> set[str] | None:
    """Return canonical values from a top-level hex metadata list."""

    if not isinstance(value, list):
        return None
    values: set[str] = set()
    for item in value:
        identity = payload_free_list_metadata_identity(field, item)
        if not isinstance(identity, str):
            return None
        values.add(identity)
    return values


def payload_free_hex_binding_metadata_values(
    field: str,
    value: Any,
) -> set[tuple[str, ...]] | None:
    """Return canonical tuples from a top-level hex binding metadata list."""

    if not isinstance(value, list):
        return None
    values: set[tuple[str, ...]] = set()
    for item in value:
        identity = payload_free_list_metadata_identity(field, item)
        if not isinstance(identity, tuple):
            return None
        values.add(identity)
    return values


def payload_free_string_list_metadata_values(value: Any) -> set[str] | None:
    """Return canonical values from top-level string metadata lists."""

    if not isinstance(value, list):
        return None
    values: set[str] = set()
    for item in value:
        item_label = canonical_string(item)
        if item_label is None:
            return None
        values.add(item_label)
    return values


def payload_free_positive_int_list_metadata_values(value: Any) -> set[int] | None:
    """Return positive integers from top-level integer metadata lists."""

    if not isinstance(value, list):
        return None
    values: set[int] = set()
    for item in value:
        if not isinstance(item, int) or isinstance(item, bool) or item <= 0:
            return None
        values.add(item)
    return values


def payload_free_object_list_hex_metadata_values(
    field: str,
    value: Any,
) -> dict[str, set[str]] | None:
    """Return canonical hex values from top-level object-list metadata."""

    if not isinstance(value, list):
        return None
    schema = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field]
    values = {hex_field: set() for hex_field in schema.get("hex", {})}
    for item in value:
        if not isinstance(item, dict):
            return None
        for hex_field, expected_hex_length in schema.get("hex", {}).items():
            item_value = canonical_lower_hex(
                item.get(hex_field),
                expected_hex_length,
            )
            if item_value is None:
                return None
            values[hex_field].add(item_value)
    return values


def payload_free_object_list_hex_tuple_metadata_values(
    field: str,
    value: Any,
    tuple_fields: tuple[str, ...],
) -> set[tuple[str, ...]] | None:
    """Return selected canonical hex tuples from object-list metadata."""

    if not isinstance(value, list):
        return None
    schema = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field]
    hex_schema = schema.get("hex", {})
    if not set(tuple_fields) <= set(hex_schema):
        return None
    values: set[tuple[str, ...]] = set()
    for item in value:
        if not isinstance(item, dict):
            return None
        tuple_value = tuple(
            canonical_lower_hex(item.get(tuple_field), hex_schema[tuple_field])
            for tuple_field in tuple_fields
        )
        if not all(value is not None for value in tuple_value):
            return None
        values.add(tuple_value)
    return values


def payload_free_object_list_string_metadata_values(
    field: str,
    value: Any,
    string_field: str,
) -> set[str] | None:
    """Return selected canonical strings from object-list metadata."""

    if not isinstance(value, list):
        return None
    schema = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field]
    if string_field not in schema.get("strings", frozenset()):
        return None
    values: set[str] = set()
    for item in value:
        if payload_free_object_list_metadata_identity(item, schema) is None:
            return None
        item_value = canonical_string(item.get(string_field))
        if item_value is None:
            return None
        values.add(item_value)
    return values


def payload_free_object_list_metadata_identities(
    field: str,
    value: Any,
) -> set[tuple[tuple[str, Any], ...]] | None:
    """Return exact comparable identities from object-list metadata."""

    if not isinstance(value, list):
        return None
    schema = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field]
    identities: set[tuple[tuple[str, Any], ...]] = set()
    for item in value:
        identity = payload_free_object_list_metadata_identity(item, schema)
        if identity is None:
            return None
        identities.add(identity)
    return identities


def fingerprint_object_list_metadata_identities(
    field: str,
    fingerprints: list[dict[str, Any]],
) -> set[tuple[tuple[str, Any], ...]]:
    """Return exact object-list metadata identities from artifact fingerprints."""

    schema = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[field]
    allowed_fields = (
        set(schema.get("strings", frozenset()))
        | set(schema.get("positive_ints", frozenset()))
        | set(schema.get("hex", {}))
    )
    identities: set[tuple[tuple[str, Any], ...]] = set()
    for fingerprint in fingerprints:
        item = {
            key: fingerprint[key]
            for key in allowed_fields
            if key in fingerprint
        }
        identity = payload_free_object_list_metadata_identity(item, schema)
        if identity is not None:
            identities.add(identity)
    return identities


def fingerprint_hex_values(
    fingerprints: list[dict[str, Any]],
    fingerprint_field: str,
    *,
    expected_hex_length: int = 64,
) -> set[str]:
    """Return canonical hex values carried by recognized artifact fingerprints."""

    return {
        value
        for fingerprint in fingerprints
        for value in [
            canonical_lower_hex(
                fingerprint.get(fingerprint_field),
                expected_hex_length,
            )
        ]
        if value is not None
    }


def fingerprint_string_values(
    fingerprints: list[dict[str, Any]],
    fingerprint_field: str,
) -> set[str]:
    """Return canonical string values carried by recognized artifact fingerprints."""

    return {
        value
        for fingerprint in fingerprints
        for value in [canonical_string(fingerprint.get(fingerprint_field))]
        if value is not None
    }


def fingerprint_string_array_values(
    fingerprints: list[dict[str, Any]],
    fingerprint_field: str,
) -> set[str]:
    """Return canonical string-array values from recognized artifact fingerprints."""

    values: set[str] = set()
    for fingerprint in fingerprints:
        items = fingerprint.get(fingerprint_field)
        if not isinstance(items, list):
            continue
        for item in items:
            item_label = canonical_string(item)
            if item_label is not None:
                values.add(item_label)
    return values


def fingerprint_positive_int_values(
    fingerprints: list[dict[str, Any]],
    fingerprint_field: str,
) -> set[int]:
    """Return positive integer values carried by recognized artifact fingerprints."""

    return {
        value
        for fingerprint in fingerprints
        for value in [fingerprint.get(fingerprint_field)]
        if isinstance(value, int) and not isinstance(value, bool) and value > 0
    }


def fingerprint_hex_binding_values(
    fingerprints: list[dict[str, Any]],
    field: str,
    fingerprint_fields: tuple[str, ...],
) -> set[tuple[str, ...]]:
    """Return canonical binding tuples carried by recognized artifact fingerprints."""

    binding_fields = PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS[field]
    expected_lengths = tuple(binding_fields.values())
    values: set[tuple[str, ...]] = set()
    for fingerprint in fingerprints:
        binding_values = tuple(
            canonical_lower_hex(
                fingerprint.get(fingerprint_field),
                expected_hex_length,
            )
            for fingerprint_field, expected_hex_length in zip(
                fingerprint_fields,
                expected_lengths,
            )
        )
        if all(value is not None for value in binding_values):
            values.add(binding_values)
    return values


def validate_payload_free_summary_metadata_fingerprint_tethers(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require top-level valid metadata claims to be backed by artifacts."""

    allowed_metadata_fields = GATE_METADATA_FIELDS.get(gate.name, frozenset())
    for field, fingerprint_field in (
        PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_BINDINGS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        metadata_values = payload_free_hex_list_metadata_values(
            field,
            payload.get(field),
        )
        if metadata_values is None or not metadata_values:
            continue
        source_kinds = PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_SOURCE_KINDS.get(
            (gate.name, field),
        )
        if source_kinds is None:
            errors.append(
                f"{field} source-kind tether is not configured for `{gate.name}`"
            )
            continue
        source_fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_names=source_kinds,
        )
        fingerprint_values = fingerprint_hex_values(
            source_fingerprints,
            fingerprint_field,
        )
        if not metadata_values <= fingerprint_values:
            errors.append(f"{field} must match recognized artifact fingerprints")

    for field, fingerprint_field in (
        PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_BINDINGS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        expected_hex_length = PAYLOAD_FREE_SUMMARY_HEX_METADATA_LENGTHS[field]
        metadata_value = canonical_lower_hex(payload.get(field), expected_hex_length)
        if metadata_value is None:
            continue
        source_kinds = PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_SCALAR_SOURCE_KINDS.get(
            (gate.name, field),
        )
        if source_kinds is None:
            errors.append(
                f"{field} source-kind tether is not configured for `{gate.name}`"
            )
            continue
        source_fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_names=source_kinds,
        )
        fingerprint_values = fingerprint_hex_values(
            source_fingerprints,
            fingerprint_field,
            expected_hex_length=expected_hex_length,
        )
        if metadata_value not in fingerprint_values:
            errors.append(f"{field} must match recognized artifact fingerprints")

    for field, fingerprint_field in (
        PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        metadata_values = payload_free_string_list_metadata_values(payload.get(field))
        if metadata_values is None or not metadata_values:
            continue
        source_kinds = PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_SOURCE_KINDS.get(
            (gate.name, field),
        )
        if source_kinds is None:
            errors.append(
                f"{field} source-kind tether is not configured for `{gate.name}`"
            )
            continue
        source_fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_names=source_kinds,
        )
        fingerprint_values = fingerprint_string_values(
            source_fingerprints,
            fingerprint_field,
        )
        if not metadata_values <= fingerprint_values:
            errors.append(f"{field} must match recognized artifact fingerprints")

    for field, fingerprint_field in (
        PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_BINDINGS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        metadata_values = payload_free_string_list_metadata_values(payload.get(field))
        if metadata_values is None or not metadata_values:
            continue
        source_kinds = (
            PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_SOURCE_KINDS.get(
                (gate.name, field),
            )
        )
        if source_kinds is None:
            errors.append(
                f"{field} source-kind tether is not configured for `{gate.name}`"
            )
            continue
        source_fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_names=source_kinds,
        )
        fingerprint_values = fingerprint_string_array_values(
            source_fingerprints,
            fingerprint_field,
        )
        if not metadata_values <= fingerprint_values:
            errors.append(f"{field} must match recognized artifact fingerprints")

    for field, fingerprint_field in (
        PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_BINDINGS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        metadata_values = payload_free_positive_int_list_metadata_values(
            payload.get(field),
        )
        if metadata_values is None or not metadata_values:
            continue
        source_kinds = (
            PAYLOAD_FREE_SUMMARY_FINGERPRINT_POSITIVE_INT_LIST_SOURCE_KINDS.get(
                (gate.name, field),
            )
        )
        if source_kinds is None:
            errors.append(
                f"{field} source-kind tether is not configured for `{gate.name}`"
            )
            continue
        source_fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_names=source_kinds,
        )
        fingerprint_values = fingerprint_positive_int_values(
            source_fingerprints,
            fingerprint_field,
        )
        if not metadata_values <= fingerprint_values:
            errors.append(f"{field} must match recognized artifact fingerprints")

    for field, fingerprint_fields in (
        PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_FIELDS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        metadata_values = payload_free_hex_binding_metadata_values(
            field,
            payload.get(field),
        )
        if metadata_values is None or not metadata_values:
            continue
        source_kinds = (
            PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_BINDING_SOURCE_KINDS.get(
                (gate.name, field),
            )
        )
        if source_kinds is None:
            errors.append(
                f"{field} source-kind tether is not configured for `{gate.name}`"
            )
            continue
        source_fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_names=source_kinds,
        )
        fingerprint_values = fingerprint_hex_binding_values(
            source_fingerprints,
            field,
            fingerprint_fields,
        )
        if not metadata_values <= fingerprint_values:
            errors.append(f"{field} must match recognized artifact fingerprints")

    for field, field_bindings in (
        PAYLOAD_FREE_SUMMARY_OBJECT_LIST_FINGERPRINT_HEX_BINDINGS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        metadata_values_by_field = payload_free_object_list_hex_metadata_values(
            field,
            payload.get(field),
        )
        if metadata_values_by_field is None:
            continue
        required_kind = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_REQUIRED_KIND_COUNTS[field]
        source_kind = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_SOURCE_KINDS.get(
            (gate.name, field),
        )
        if source_kind is None:
            errors.append(
                f"{field} source-kind tether is not configured for `{gate.name}`"
            )
            continue
        if source_kind != required_kind:
            errors.append(
                f"{field} source-kind tether must match required artifact count "
                f"kind for `{gate.name}`"
            )
            continue
        kind_fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_name=source_kind,
        )
        metadata_identities = payload_free_object_list_metadata_identities(
            field,
            payload.get(field),
        )
        if metadata_identities is not None:
            fingerprint_identities = fingerprint_object_list_metadata_identities(
                field,
                kind_fingerprints,
            )
            if not metadata_identities <= fingerprint_identities:
                errors.append(
                    f"{field} entries must match recognized artifact fingerprints"
                )
        for metadata_field, fingerprint_field in field_bindings.items():
            metadata_values = metadata_values_by_field.get(metadata_field, set())
            if not metadata_values:
                continue
            expected_hex_length = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS[
                field
            ]["hex"][metadata_field]
            fingerprint_values = fingerprint_hex_values(
                kind_fingerprints,
                fingerprint_field,
                expected_hex_length=expected_hex_length,
            )
            if not metadata_values <= fingerprint_values:
                errors.append(
                    f"{field}.{metadata_field} must match recognized artifact fingerprints"
                )


def required_artifact_count_for_kind(
    required: dict[str, Any],
    kind_name: str,
) -> int | None:
    """Return the artifact count for a required row when it is well-formed."""

    row = required.get(kind_name)
    if not isinstance(row, dict):
        return None
    artifacts = row.get("artifacts")
    if not isinstance(artifacts, list):
        return None
    return len(artifacts)


def validate_payload_free_object_list_metadata_counts(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    required: dict[str, Any],
    errors: list[str],
) -> None:
    """Require detail metadata lists to match their required artifact rows."""

    allowed_metadata_fields = GATE_METADATA_FIELDS.get(gate.name, frozenset())
    for field, kind_name in (
        PAYLOAD_FREE_SUMMARY_OBJECT_LIST_REQUIRED_KIND_COUNTS.items()
    ):
        if field not in allowed_metadata_fields or field not in payload:
            continue
        value = payload.get(field)
        if not isinstance(value, list):
            continue
        expected_count = required_artifact_count_for_kind(required, kind_name)
        if expected_count is None:
            continue
        if len(value) != expected_count:
            errors.append(
                f"{field} length must match `{kind_name}` required artifact count"
            )


def payload_free_list_metadata_identity(field: str, item: Any) -> Any | None:
    """Return a comparable identity for sorted payload-free metadata lists."""

    binding_fields = PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS.get(field)
    if binding_fields is not None:
        if not isinstance(item, dict):
            return None
        identity = []
        for key, expected_hex_length in binding_fields.items():
            value = item.get(key)
            if (
                not isinstance(value, str)
                or len(value) != expected_hex_length
                or any(character not in LOWER_HEX_DIGITS for character in value)
            ):
                return None
            identity.append(value)
        return tuple(identity)
    if field in PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS:
        if isinstance(item, int) and not isinstance(item, bool) and item > 0:
            return item
        return None
    item_label = canonical_string(item)
    if item_label is None:
        return None
    if field in PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS:
        if len(item_label) != 64 or any(
            character not in LOWER_HEX_DIGITS for character in item_label
        ):
            return None
    return item_label


def validate_payload_free_ordered_list_metadata(
    field: str,
    value: list[Any],
    errors: list[str],
) -> None:
    """Require set-derived metadata lists to preserve canonical sorted order."""

    identities = [
        payload_free_list_metadata_identity(field, item)
        for item in value
    ]
    if any(identity is None for identity in identities):
        return
    if len(set(identities)) != len(identities):
        errors.append(f"{field} must not contain duplicate metadata entries")
    if identities != sorted(identities):
        errors.append(f"{field} must be sorted in canonical order")


def validate_payload_free_required_string_list_values(
    gate: GateSummaryKind,
    field: str,
    value: Any,
    errors: list[str],
) -> None:
    """Require exact reviewed values for configured string-list metadata."""

    required_values = PAYLOAD_FREE_SUMMARY_REQUIRED_STRING_LIST_VALUES.get(
        (gate.name, field)
    )
    if required_values is None or not isinstance(value, list):
        return
    metadata_values = payload_free_string_list_metadata_values(value)
    if metadata_values is None:
        return
    required_set = frozenset(required_values)
    for required_value in required_values:
        if required_value not in metadata_values:
            errors.append(f"{field} must include metadata value `{required_value}`")
    if not metadata_values <= required_set:
        errors.append(f"{field} must not include unknown metadata values")


def validate_payload_free_allowed_string_list_values(
    gate: GateSummaryKind,
    field: str,
    value: Any,
    errors: list[str],
) -> None:
    """Reject values outside a reviewed closed set for string-list metadata."""

    allowed_values = PAYLOAD_FREE_SUMMARY_ALLOWED_STRING_LIST_VALUES.get(
        (gate.name, field)
    )
    if allowed_values is None or not isinstance(value, list):
        return
    metadata_values = payload_free_string_list_metadata_values(value)
    if metadata_values is None:
        return
    if not metadata_values <= frozenset(allowed_values):
        errors.append(f"{field} must not include unknown metadata values")


def validate_payload_free_provider_id_metadata_values(
    gate: GateSummaryKind,
    field: str,
    value: Any,
    errors: list[str],
) -> None:
    """Reject forged aggregate provider-id labels outside the reputation policy."""

    if gate.name != "reputation" or field != "provider_ids" or not isinstance(value, list):
        return
    for index, item in enumerate(value):
        provider_id = canonical_string(item)
        if provider_id is None:
            continue
        path = f"{field}[{index}]"
        if REPUTATION_PROVIDER_ID_PATTERN.fullmatch(provider_id) is None:
            errors.append(f"{path} must match canonical lowercase `provider-*`")
            continue
        forbidden = forbidden_non_production_markers(
            provider_id,
            REPUTATION_FORBIDDEN_PROVIDER_ID_MARKERS,
        )
        if forbidden:
            errors.append(
                f"{path} must not contain non-production markers {forbidden}"
            )


def validate_payload_free_object_list_string_field_policies(
    gate: GateSummaryKind,
    field: str,
    value: Any,
    errors: list[str],
) -> None:
    """Replay lane-owned string identity policies for aggregate object metadata."""

    if not isinstance(value, list):
        return
    policies = {
        key_field: policy
        for (gate_name, metadata_field, key_field), policy in (
            PAYLOAD_FREE_SUMMARY_OBJECT_LIST_STRING_FIELD_POLICIES.items()
        )
        if gate_name == gate.name and metadata_field == field
    }
    if not policies:
        return
    for index, item in enumerate(value):
        if not isinstance(item, dict):
            continue
        for key_field, policy in policies.items():
            metadata_value = canonical_string(item.get(key_field))
            if metadata_value is None:
                continue
            path = f"{field}[{index}].{key_field}"
            pattern = policy["pattern"]
            if pattern.fullmatch(metadata_value) is None:
                errors.append(f"{path} {policy['pattern_error']}")
                continue
            forbidden_markers = policy["forbidden_markers"]
            forbidden = forbidden_non_production_markers(
                metadata_value,
                forbidden_markers,
            )
            if forbidden:
                errors.append(
                    f"{path} must not contain non-production markers {forbidden}"
                )


def validate_payload_free_deployment_context_metadata(
    path: str,
    value: Any,
    errors: list[str],
) -> None:
    """Reject non-production deployment contexts in payload-free metadata."""

    if not isinstance(value, dict):
        return
    deployment_id = canonical_string(value.get("deployment_id"))
    if deployment_id is not None:
        require_production_deployment_id_value(
            deployment_id,
            errors,
            f"{path}.deployment_id",
        )
    environment = canonical_string(value.get("environment"))
    if environment is not None and not is_production_ready_environment(environment):
        errors.append(f"{path}.environment must be production")


def validate_payload_free_object_list_deployment_context_metadata(
    field: str,
    value: Any,
    schema: dict[str, Any],
    errors: list[str],
) -> None:
    """Reject non-production deployment contexts in object-list metadata."""

    string_fields = schema.get("strings", frozenset())
    if not {"deployment_id", "environment"} <= set(string_fields):
        return
    if not isinstance(value, list):
        return
    for index, item in enumerate(value):
        validate_payload_free_deployment_context_metadata(
            f"{field}[{index}]",
            item,
            errors,
        )


def validate_payload_free_string_list_count_binding(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    field: str,
    value: Any,
    errors: list[str],
) -> None:
    """Require configured string-list metadata counts to match companion counts."""

    count_field = PAYLOAD_FREE_SUMMARY_STRING_LIST_COUNT_BINDINGS.get(
        (gate.name, field)
    )
    if count_field is None or not isinstance(value, list):
        return
    metadata_values = payload_free_string_list_metadata_values(value)
    count_values = payload_free_positive_int_list_metadata_values(
        payload.get(count_field)
    )
    if metadata_values is None or count_values is None:
        return
    if len(metadata_values) not in count_values:
        errors.append(f"{count_field} must include the unique {field} count")


def validate_transparency_publication_binding_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require transparency cycle claims to carry their source-publication edge."""

    source_values = payload_free_hex_list_metadata_values(
        "valid_source_batch_digests",
        payload.get("valid_source_batch_digests"),
    )
    cycle_values = payload_free_hex_list_metadata_values(
        "valid_cycle_digests",
        payload.get("valid_cycle_digests"),
    )
    publication_bindings = payload_free_hex_binding_metadata_values(
        "valid_publication_bindings",
        payload.get("valid_publication_bindings"),
    )
    if (
        source_values is None
        or cycle_values is None
        or publication_bindings is None
    ):
        return

    binding_source_values = {
        source_batch_digest
        for source_batch_digest, _cycle_digest in publication_bindings
    }
    binding_cycle_values = {
        cycle_digest for _source_batch_digest, cycle_digest in publication_bindings
    }
    if cycle_values != binding_cycle_values:
        errors.append(
            "valid_cycle_digests must match valid_publication_bindings cycle digests"
        )
    if not binding_source_values <= source_values:
        errors.append(
            "valid_publication_bindings source batches must match valid_source_batch_digests"
        )


def validate_transparency_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate transparency metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=TRANSPARENCY_SOURCE_BOUND_KINDS,
        metadata_field="valid_source_batch_digests",
        fingerprint_field="source_batch_digest_hex",
        error=(
            "transparency source-bound artifact fingerprints must match "
            "valid_source_batch_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=TRANSPARENCY_CYCLE_BOUND_KINDS,
        metadata_field="valid_cycle_digests",
        fingerprint_field="cycle_digest_hex",
        error=(
            "transparency cycle-bound artifact fingerprints must match "
            "valid_cycle_digests"
        ),
        errors=errors,
    )


def validate_hedging_billing_cycle_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require hedging billing-cycle metadata to preserve lane bindings."""

    cycle_bindings = payload_free_hex_binding_metadata_values(
        "valid_cycle_bindings",
        payload.get("valid_cycle_bindings"),
    )
    cycle_tuple_values = payload_free_object_list_hex_tuple_metadata_values(
        "valid_billing_cycles",
        payload.get("valid_billing_cycles"),
        ("statement_bundle_digest_hex", "reconciliation_digest_hex"),
    )
    cycle_hex_values = payload_free_object_list_hex_metadata_values(
        "valid_billing_cycles",
        payload.get("valid_billing_cycles"),
    )
    policy_values = payload_free_hex_list_metadata_values(
        "valid_policy_digests",
        payload.get("valid_policy_digests"),
    )
    reference_values = payload_free_hex_list_metadata_values(
        "valid_reference_decision_ids",
        payload.get("valid_reference_decision_ids"),
    )

    if cycle_bindings is not None and cycle_tuple_values is not None:
        if cycle_bindings != cycle_tuple_values:
            errors.append(
                "valid_cycle_bindings must match valid_billing_cycles cycle tuples"
            )
    if cycle_hex_values is None:
        return
    cycle_policy_values = cycle_hex_values.get("policy_digest_hex", set())
    if policy_values is not None and policy_values != cycle_policy_values:
        errors.append(
            "valid_policy_digests must match valid_billing_cycles policy digests"
        )
    cycle_reference_values = cycle_hex_values.get("reference_decision_id_hex", set())
    if reference_values is not None and not cycle_reference_values <= reference_values:
        errors.append(
            "valid_billing_cycles reference decisions must match valid_reference_decision_ids"
        )


def validate_hedging_billing_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate hedging billing metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_binding_metadata(
        payload,
        kind_names=HEDGING_BILLING_CYCLE_BOUND_KINDS,
        metadata_field="valid_cycle_bindings",
        fingerprint_fields=(
            "statement_bundle_digest_hex",
            "reconciliation_digest_hex",
        ),
        error=(
            "hedging_billing cycle-bound artifact fingerprints must match "
            "valid_cycle_bindings"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=HEDGING_BILLING_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "hedging_billing policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_reserve_rent_policy_matrix_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require reserve-rent aggregate metadata to preserve policy/matrix chain."""

    policy_values = payload_free_hex_list_metadata_values(
        "valid_policy_digests",
        payload.get("valid_policy_digests"),
    )
    matrix_bindings = payload_free_hex_binding_metadata_values(
        "valid_policy_matrix_bindings",
        payload.get("valid_policy_matrix_bindings"),
    )
    ledger_bindings = payload_free_hex_binding_metadata_values(
        "valid_policy_matrix_ledger_bindings",
        payload.get("valid_policy_matrix_ledger_bindings"),
    )
    provider_bake_tuples = payload_free_object_list_hex_tuple_metadata_values(
        "valid_provider_bakes",
        payload.get("valid_provider_bakes"),
        ("policy_digest_hex", "matrix_digest_hex", "ledger_digest_hex"),
    )
    provider_bake_ids = payload_free_object_list_string_metadata_values(
        "valid_provider_bakes",
        payload.get("valid_provider_bakes"),
        "bake_id",
    )
    if provider_bake_ids is not None:
        for fingerprint in payload_free_summary_artifact_fingerprints(
            payload,
            kind_name="governance_approval",
        ):
            bake_id = canonical_string(fingerprint.get("bake_id"))
            if bake_id is None or bake_id not in provider_bake_ids:
                errors.append(
                    "reserve_rent governance approval bake_id fingerprints must "
                    "match valid_provider_bakes"
                )
                break

    if policy_values is not None and matrix_bindings is not None:
        matrix_policy_values = {
            policy_digest for policy_digest, _matrix_digest in matrix_bindings
        }
        if not matrix_policy_values <= policy_values:
            errors.append(
                "valid_policy_matrix_bindings policies must match valid_policy_digests"
            )
    if matrix_bindings is None or ledger_bindings is None:
        return
    ledger_matrix_pairs = {
        (policy_digest, matrix_digest)
        for policy_digest, matrix_digest, _ledger_digest in ledger_bindings
    }
    if not ledger_matrix_pairs <= matrix_bindings:
        errors.append(
            "valid_policy_matrix_ledger_bindings matrix pairs must match "
            "valid_policy_matrix_bindings"
        )
    if provider_bake_tuples is not None and not provider_bake_tuples <= ledger_bindings:
        errors.append(
            "valid_provider_bakes ledger tuples must match "
            "valid_policy_matrix_ledger_bindings"
        )


def validate_reserve_rent_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate reserve-rent metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=RESERVE_RENT_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "reserve_rent policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_binding_metadata(
        payload,
        kind_names=RESERVE_RENT_MATRIX_BOUND_KINDS,
        metadata_field="valid_policy_matrix_bindings",
        fingerprint_fields=("policy_digest_hex", "matrix_digest_hex"),
        error=(
            "reserve_rent matrix-bound artifact fingerprints must match "
            "valid_policy_matrix_bindings"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_binding_metadata(
        payload,
        kind_names=RESERVE_RENT_LEDGER_BOUND_KINDS,
        metadata_field="valid_policy_matrix_ledger_bindings",
        fingerprint_fields=(
            "policy_digest_hex",
            "matrix_digest_hex",
            "ledger_digest_hex",
        ),
        error=(
            "reserve_rent ledger-bound artifact fingerprints must match "
            "valid_policy_matrix_ledger_bindings"
        ),
        errors=errors,
    )


def validate_pop_credentials_sync_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require Pop juror sync bindings to reference valid roots and revocations."""

    root_values = payload_free_hex_list_metadata_values(
        "valid_root_digests",
        payload.get("valid_root_digests"),
    )
    revocation_values = payload_free_hex_list_metadata_values(
        "valid_revocation_list_digests",
        payload.get("valid_revocation_list_digests"),
    )
    sync_bindings = payload_free_hex_binding_metadata_values(
        "valid_juror_sync_bindings",
        payload.get("valid_juror_sync_bindings"),
    )
    if sync_bindings is None:
        return
    synced_root_values = {
        synced_root for synced_root, _synced_revocation in sync_bindings
    }
    if root_values is not None and not synced_root_values <= root_values:
        errors.append(
            "valid_juror_sync_bindings roots must match valid_root_digests"
        )
    synced_revocation_values = {
        synced_revocation for _synced_root, synced_revocation in sync_bindings
    }
    if (
        revocation_values is not None
        and not synced_revocation_values <= revocation_values
    ):
        errors.append(
            "valid_juror_sync_bindings revocations must match "
            "valid_revocation_list_digests"
        )


def validate_pop_credentials_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate Pop metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata_by_kind(
        payload,
        kind_fingerprint_fields=POP_CREDENTIALS_ROOT_BOUND_FINGERPRINT_FIELDS,
        metadata_field="valid_root_digests",
        error=(
            "pop_credentials root-bound artifact fingerprints must match "
            "valid_root_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata_by_kind(
        payload,
        kind_fingerprint_fields=POP_CREDENTIALS_REVOCATION_BOUND_FINGERPRINT_FIELDS,
        metadata_field="valid_revocation_list_digests",
        error=(
            "pop_credentials revocation-bound artifact fingerprints must match "
            "valid_revocation_list_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=POP_CREDENTIALS_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "pop_credentials policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_reputation_snapshot_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require reputation scalar snapshot metadata to match binding tuples."""

    snapshot_id = canonical_lower_hex(payload.get("snapshot_id_hex"), 32)
    merkle_root = canonical_lower_hex(payload.get("merkle_root_hex"), 64)
    snapshot_bindings = payload_free_hex_binding_metadata_values(
        "valid_snapshot_bindings",
        payload.get("valid_snapshot_bindings"),
    )
    if snapshot_id is None or merkle_root is None or snapshot_bindings is None:
        return
    if snapshot_bindings != {(snapshot_id, merkle_root)}:
        errors.append(
            "valid_snapshot_bindings must match snapshot_id_hex and merkle_root_hex"
        )


def validate_moderation_panel_chain_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require moderation-panel aggregate metadata to preserve lane bindings."""

    case_values = payload_free_hex_list_metadata_values(
        "valid_case_digests",
        payload.get("valid_case_digests"),
    )
    roster_bindings = payload_free_hex_binding_metadata_values(
        "valid_roster_bindings",
        payload.get("valid_roster_bindings"),
    )
    tally_bindings = payload_free_hex_binding_metadata_values(
        "valid_tally_bindings",
        payload.get("valid_tally_bindings"),
    )
    e2e_tallies = payload_free_object_list_hex_tuple_metadata_values(
        "valid_e2e_runs",
        payload.get("valid_e2e_runs"),
        ("case_digest_hex", "roster_hash_hex", "tally_digest_hex"),
    )
    evidence_viewer_rosters = payload_free_object_list_hex_tuple_metadata_values(
        "valid_evidence_viewer_digest_sets",
        payload.get("valid_evidence_viewer_digest_sets"),
        ("case_digest_hex", "roster_hash_hex"),
    )
    if case_values is None or roster_bindings is None:
        return

    roster_case_values = {
        case_digest for case_digest, _roster_hash in roster_bindings
    }
    if not roster_case_values <= case_values:
        errors.append(
            "valid_roster_bindings case digests must match valid_case_digests"
        )
    if tally_bindings is None:
        return

    tally_roster_pairs = {
        (case_digest, roster_hash)
        for case_digest, roster_hash, _tally_digest in tally_bindings
    }
    if not tally_roster_pairs <= roster_bindings:
        errors.append(
            "valid_tally_bindings roster pairs must match valid_roster_bindings"
        )
    if e2e_tallies is not None and not e2e_tallies <= tally_bindings:
        errors.append(
            "valid_e2e_runs tally bindings must match valid_tally_bindings"
        )
    if (
        evidence_viewer_rosters is not None
        and not evidence_viewer_rosters <= roster_bindings
    ):
        errors.append(
            "valid_evidence_viewer_digest_sets roster pairs must match valid_roster_bindings"
        )


def fingerprint_hex_tuple_value(
    fingerprint: dict[str, Any],
    metadata_field: str,
    fingerprint_fields: tuple[str, ...],
) -> tuple[str, ...] | None:
    """Return one canonical hex tuple from an artifact fingerprint."""

    binding_fields = PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS[metadata_field]
    tuple_value = []
    for fingerprint_field in fingerprint_fields:
        expected_hex_length = binding_fields.get(fingerprint_field)
        if expected_hex_length is None:
            return None
        value = canonical_lower_hex(
            fingerprint.get(fingerprint_field),
            expected_hex_length,
        )
        if value is None:
            return None
        tuple_value.append(value)
    return tuple(tuple_value)


def bound_artifact_fingerprints_match_hex_list_metadata(
    payload: dict[str, Any],
    *,
    kind_names: tuple[str, ...],
    metadata_field: str,
    fingerprint_field: str,
    error: str,
    errors: list[str],
    expected_hex_length: int = 64,
) -> None:
    """Require every bound artifact fingerprint to carry a valid metadata value."""

    metadata_values = payload_free_hex_list_metadata_values(
        metadata_field,
        payload.get(metadata_field),
    )
    if metadata_values is None:
        return
    fingerprints = payload_free_summary_artifact_fingerprints(
        payload,
        kind_names=kind_names,
    )
    for fingerprint in fingerprints:
        value = canonical_lower_hex(
            fingerprint.get(fingerprint_field),
            expected_hex_length,
        )
        if value is None or value not in metadata_values:
            errors.append(error)
            return


def bound_artifact_fingerprints_match_hex_list_metadata_by_kind(
    payload: dict[str, Any],
    *,
    kind_fingerprint_fields: tuple[tuple[str, str], ...],
    metadata_field: str,
    error: str,
    errors: list[str],
    expected_hex_length: int = 64,
) -> None:
    """Require every bound artifact fingerprint to carry a valid metadata value."""

    metadata_values = payload_free_hex_list_metadata_values(
        metadata_field,
        payload.get(metadata_field),
    )
    if metadata_values is None:
        return
    for kind_name, fingerprint_field in kind_fingerprint_fields:
        fingerprints = payload_free_summary_artifact_fingerprints(
            payload,
            kind_name=kind_name,
        )
        for fingerprint in fingerprints:
            value = canonical_lower_hex(
                fingerprint.get(fingerprint_field),
                expected_hex_length,
            )
            if value is None or value not in metadata_values:
                errors.append(error)
                return


def bound_artifact_fingerprints_match_hex_binding_metadata(
    payload: dict[str, Any],
    *,
    kind_names: tuple[str, ...],
    metadata_field: str,
    fingerprint_fields: tuple[str, ...],
    error: str,
    errors: list[str],
) -> None:
    """Require every bound artifact fingerprint to carry a valid metadata tuple."""

    metadata_values = payload_free_hex_binding_metadata_values(
        metadata_field,
        payload.get(metadata_field),
    )
    if metadata_values is None:
        return
    fingerprints = payload_free_summary_artifact_fingerprints(
        payload,
        kind_names=kind_names,
    )
    for fingerprint in fingerprints:
        tuple_value = fingerprint_hex_tuple_value(
            fingerprint,
            metadata_field,
            fingerprint_fields,
        )
        if tuple_value is None or tuple_value not in metadata_values:
            errors.append(error)
            return


def bound_artifact_fingerprints_match_object_list_metadata(
    payload: dict[str, Any],
    *,
    kind_names: tuple[str, ...],
    metadata_field: str,
    error: str,
    errors: list[str],
) -> None:
    """Require every bound artifact fingerprint to carry one complete metadata tuple."""

    metadata_values = payload_free_object_list_metadata_identities(
        metadata_field,
        payload.get(metadata_field),
    )
    if metadata_values is None:
        return
    fingerprints = payload_free_summary_artifact_fingerprints(
        payload,
        kind_names=kind_names,
    )
    for fingerprint in fingerprints:
        fingerprint_values = fingerprint_object_list_metadata_identities(
            metadata_field,
            [fingerprint],
        )
        if len(fingerprint_values) != 1 or not fingerprint_values <= metadata_values:
            errors.append(error)
            return


def validate_moderation_panel_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate moderation-panel metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=MODERATION_PANEL_CASE_BOUND_KINDS,
        metadata_field="valid_case_digests",
        fingerprint_field="case_digest_hex",
        error=(
            "moderation_panel case-bound artifact fingerprints must match "
            "valid_case_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_binding_metadata(
        payload,
        kind_names=MODERATION_PANEL_ROSTER_BOUND_KINDS,
        metadata_field="valid_roster_bindings",
        fingerprint_fields=("case_digest_hex", "roster_hash_hex"),
        error=(
            "moderation_panel roster-bound artifact fingerprints must match "
            "valid_roster_bindings"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_binding_metadata(
        payload,
        kind_names=MODERATION_PANEL_TALLY_BOUND_KINDS,
        metadata_field="valid_tally_bindings",
        fingerprint_fields=(
            "case_digest_hex",
            "roster_hash_hex",
            "tally_digest_hex",
        ),
        error=(
            "moderation_panel tally-bound artifact fingerprints must match "
            "valid_tally_bindings"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=MODERATION_PANEL_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "moderation_panel policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_reputation_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate reputation metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=REPUTATION_SNAPSHOT_ANCHOR_KINDS,
        metadata_field="valid_reputation_weight_digests",
        fingerprint_field="weights_digest_hex",
        error=(
            "reputation publish/latest artifact fingerprints must match "
            "valid_reputation_weight_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_binding_metadata(
        payload,
        kind_names=REPUTATION_SNAPSHOT_BOUND_KINDS,
        metadata_field="valid_snapshot_bindings",
        fingerprint_fields=("snapshot_id_hex", "merkle_root_hex"),
        error=(
            "reputation snapshot-bound artifact fingerprints must match "
            "valid_snapshot_bindings"
        ),
        errors=errors,
    )


def validate_appeal_finance_multi_peer_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require multi-peer run metadata to use lane-proven pricing configs."""

    config_values = payload_free_hex_list_metadata_values(
        "valid_config_digests",
        payload.get("valid_config_digests"),
    )
    run_hex_values = payload_free_object_list_hex_metadata_values(
        "valid_multi_peer_runs",
        payload.get("valid_multi_peer_runs"),
    )
    if config_values is None or run_hex_values is None:
        return

    run_config_values = run_hex_values.get("config_digest_hex", set())
    if not run_config_values <= config_values:
        errors.append(
            "valid_multi_peer_runs config digests must match valid_config_digests"
        )


def validate_appeal_finance_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate appeal finance metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=APPEAL_FINANCE_CONFIG_BOUND_KINDS,
        metadata_field="valid_config_digests",
        fingerprint_field="config_digest_hex",
        error=(
            "appeal_finance config-bound artifact fingerprints must match "
            "valid_config_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=APPEAL_FINANCE_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "appeal_finance policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_ai_prescreen_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate AI pre-screen metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_binding_metadata(
        payload,
        kind_names=AI_PRESCREEN_RUNNER_BOUND_KINDS,
        metadata_field="valid_runner_bindings",
        fingerprint_fields=(
            "manifest_id_hex",
            "runner_hash_hex",
            "subject_digest_hex",
        ),
        error=(
            "ai_prescreen runner-bound artifact fingerprints must match "
            "valid_runner_bindings"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=AI_PRESCREEN_WORKFLOW_BOUND_KINDS,
        metadata_field="valid_workflow_digests",
        fingerprint_field="workflow_digest_hex",
        error=(
            "ai_prescreen workflow-bound artifact fingerprints must match "
            "valid_workflow_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=AI_PRESCREEN_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "ai_prescreen policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_governance_dag_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate Governance DAG metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=GOVERNANCE_DAG_PUBLIC_HEAD_BOUND_KINDS,
        metadata_field="valid_public_head_cids",
        fingerprint_field="public_head_cid_hex",
        error=(
            "governance_dag public-head-bound artifact fingerprints must match "
            "valid_public_head_cids"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=GOVERNANCE_DAG_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "governance_dag policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )
    for metadata_field, fingerprint_field in (
        ("valid_receiver_policy_digests", "receiver_policy_digest_hex"),
        ("valid_replay_namespace_digests", "replay_namespace_digest_hex"),
        ("valid_replica_set_digests", "replica_set_digest_hex"),
        (
            "valid_kubo_ingress_binding_digests",
            "kubo_ingress_binding_digest_hex",
        ),
        (
            "valid_signed_head_ingress_binding_digests",
            "signed_head_ingress_binding_digest_hex",
        ),
    ):
        bound_artifact_fingerprints_match_hex_list_metadata(
            payload,
            kind_names=GOVERNANCE_DAG_INGRESS_BOUND_KINDS,
            metadata_field=metadata_field,
            fingerprint_field=fingerprint_field,
            error=(
                "governance_dag ingress-bound artifact fingerprints must match "
                f"{metadata_field}"
            ),
            errors=errors,
        )


def validate_gateway_load_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate gateway-load metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=("staging_load",),
        metadata_field="valid_suite_report_digests",
        fingerprint_field="suite_report_digest_hex",
        error=(
            "gateway_load suite-bound artifact fingerprints must match "
            "valid_suite_report_digests"
        ),
        errors=errors,
    )
    validate_gateway_load_staging_fingerprint_contract(payload, errors)
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=GATEWAY_LOAD_STAGING_REPORT_BOUND_KINDS,
        metadata_field="valid_staging_report_digests",
        fingerprint_field="staging_report_digest_hex",
        error=(
            "gateway_load staging-bound artifact fingerprints must match "
            "valid_staging_report_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=GATEWAY_LOAD_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "gateway_load policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def _validate_gateway_load_named_inventory(
    fingerprint: dict[str, Any],
    *,
    field: str,
    count_field: str,
    minimum_count: int,
    pattern: re.Pattern[str],
    path: str,
    errors: list[str],
) -> int | None:
    """Validate one schema-closed gateway-load fingerprint inventory."""

    count = fingerprint.get(count_field)
    if (
        not isinstance(count, int)
        or isinstance(count, bool)
        or count < minimum_count
    ):
        errors.append(f"{path}.{count_field} must be an integer >= {minimum_count}")
        count = None
    rows = fingerprint.get(field)
    if not isinstance(rows, list):
        errors.append(f"{path}.{field} must be an array")
        return count
    names: list[str] = []
    for index, row in enumerate(rows):
        row_path = f"{path}.{field}[{index}]"
        if not isinstance(row, dict) or set(row) != {"name"}:
            errors.append(f"{row_path} fields must match the schema-closed contract")
            continue
        name = canonical_string(row.get("name"))
        if name is None or pattern.fullmatch(name) is None:
            errors.append(f"{row_path}.name must match the production load contract")
            continue
        if forbidden_non_production_markers(
            name,
            GATEWAY_LOAD_FORBIDDEN_METADATA_MARKERS,
        ):
            errors.append(
                f"{row_path}.name must not contain non-production markers"
            )
            continue
        names.append(name)
    if count is not None and len(rows) != count:
        errors.append(f"{path}.{field} length must match {count_field}")
    if len(names) == len(rows) and len(set(names)) != len(names):
        errors.append(f"{path}.{field} names must be unique")
    return count


def _validate_gateway_load_bounded_integer(
    fingerprint: dict[str, Any],
    field: str,
    *,
    minimum: int,
    maximum: int | None,
    path: str,
    errors: list[str],
) -> int | None:
    """Validate one integer threshold copied into a staging fingerprint."""

    value = fingerprint.get(field)
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value < minimum
        or (maximum is not None and value > maximum)
    ):
        if maximum is None:
            errors.append(f"{path}.{field} must be an integer >= {minimum}")
        else:
            errors.append(
                f"{path}.{field} must be an integer in {minimum}..{maximum}"
            )
        return None
    return value


def validate_gateway_load_staging_fingerprint_contract(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Recheck the non-waivable 24-hour load contract at promotion time."""

    fingerprints = payload_free_summary_artifact_fingerprints(
        payload,
        kind_name="staging_load",
    )
    if not fingerprints:
        errors.append(
            "gateway_load staging_load must expose its production load fingerprint"
        )
        return
    for index, fingerprint in enumerate(fingerprints):
        path = f"gateway_load staging_load fingerprint[{index}]"
        if fingerprint.get("environment") != "production":
            errors.append(f"{path}.environment must be exactly production")
        fixture_digest = canonical_lower_hex(
            fingerprint.get("fixture_bundle_digest_hex"),
            64,
        )
        if fixture_digest is None or not any(bytes.fromhex(fixture_digest)):
            errors.append(
                f"{path}.fixture_bundle_digest_hex must be canonical non-zero SHA-256"
            )
        gateway_version = canonical_string(fingerprint.get("gateway_version"))
        if (
            gateway_version is None
            or GATEWAY_LOAD_VERSION_PATTERN.fullmatch(gateway_version) is None
        ):
            errors.append(f"{path}.gateway_version must match the release contract")

        hardware = fingerprint.get("hardware_profile")
        if not isinstance(hardware, dict) or set(hardware) != {"name"}:
            errors.append(
                f"{path}.hardware_profile fields must match the schema-closed contract"
            )
        else:
            hardware_name = canonical_string(hardware.get("name"))
            if (
                hardware_name is None
                or GATEWAY_LOAD_HARDWARE_PROFILE_PATTERN.fullmatch(hardware_name)
                is None
                or forbidden_non_production_markers(
                    hardware_name,
                    GATEWAY_LOAD_FORBIDDEN_METADATA_MARKERS,
                )
            ):
                errors.append(
                    f"{path}.hardware_profile.name must match the production load contract"
                )

        cache_coverage = fingerprint.get("cache_coverage")
        if (
            not isinstance(cache_coverage, dict)
            or set(cache_coverage) != GATEWAY_LOAD_REQUIRED_CACHE_COVERAGE_FIELDS
        ):
            errors.append(
                f"{path}.cache_coverage fields must match the schema-closed contract"
            )
        elif any(value is not True for value in cache_coverage.values()):
            errors.append(
                f"{path}.cache_coverage must exercise cold, warm, and mixed caches"
            )

        load_conditions = fingerprint.get("load_conditions")
        if (
            not isinstance(load_conditions, dict)
            or set(load_conditions) != GATEWAY_LOAD_REQUIRED_LOAD_CONDITION_FIELDS
        ):
            errors.append(
                f"{path}.load_conditions fields must match the schema-closed contract"
            )
        else:
            if (
                load_conditions.get("corruption_injection_bps")
                != GATEWAY_LOAD_REQUIRED_CORRUPTION_BPS
            ):
                errors.append(
                    f"{path}.load_conditions.corruption_injection_bps must be "
                    f"exactly {GATEWAY_LOAD_REQUIRED_CORRUPTION_BPS}"
                )
            boolean_fields = (
                GATEWAY_LOAD_REQUIRED_LOAD_CONDITION_FIELDS
                - {"corruption_injection_bps"}
            )
            if any(load_conditions.get(field) is not True for field in boolean_fields):
                errors.append(
                    f"{path}.load_conditions must exercise revocation, malformed "
                    "flood, denylist, rate-limit, and failover pressure"
                )

        _validate_gateway_load_bounded_integer(
            fingerprint,
            "duration_seconds",
            minimum=GATEWAY_LOAD_MIN_STAGING_DURATION_SECS,
            maximum=None,
            path=path,
            errors=errors,
        )
        stream_count = _validate_gateway_load_named_inventory(
            fingerprint,
            field="streams",
            count_field="stream_count",
            minimum_count=GATEWAY_LOAD_MIN_STREAMS,
            pattern=GATEWAY_LOAD_STREAM_NAME_PATTERN,
            path=path,
            errors=errors,
        )
        peak = _validate_gateway_load_bounded_integer(
            fingerprint,
            "peak_concurrent_range_streams",
            minimum=GATEWAY_LOAD_MIN_STREAMS,
            maximum=None,
            path=path,
            errors=errors,
        )
        if (
            stream_count is not None
            and peak is not None
            and peak > stream_count
        ):
            errors.append(
                f"{path}.peak_concurrent_range_streams must be <= stream_count"
            )
        _validate_gateway_load_named_inventory(
            fingerprint,
            field="providers",
            count_field="provider_count",
            minimum_count=GATEWAY_LOAD_MIN_PROVIDER_COUNT,
            pattern=GATEWAY_LOAD_PROVIDER_NAME_PATTERN,
            path=path,
            errors=errors,
        )
        _validate_gateway_load_bounded_integer(
            fingerprint,
            "success_rate_bps",
            minimum=GATEWAY_LOAD_MIN_SUCCESS_RATE_BPS,
            maximum=GATEWAY_LOAD_MAX_SUCCESS_RATE_BPS,
            path=path,
            errors=errors,
        )
        _validate_gateway_load_bounded_integer(
            fingerprint,
            "error_rate_bps",
            minimum=0,
            maximum=GATEWAY_LOAD_MAX_ERROR_RATE_BPS,
            path=path,
            errors=errors,
        )
        _validate_gateway_load_bounded_integer(
            fingerprint,
            "p95_latency_ms",
            minimum=0,
            maximum=GATEWAY_LOAD_MAX_P95_LATENCY_MS,
            path=path,
            errors=errors,
        )
        _validate_gateway_load_bounded_integer(
            fingerprint,
            "p99_latency_ms",
            minimum=0,
            maximum=GATEWAY_LOAD_MAX_P99_LATENCY_MS,
            path=path,
            errors=errors,
        )


def validate_gateway_compliance_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate gateway-compliance metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=GATEWAY_COMPLIANCE_CATALOG_BOUND_KINDS,
        metadata_field="valid_catalog_digests",
        fingerprint_field="catalog_digest_hex",
        error=(
            "gateway_compliance catalog-bound artifact fingerprints must match "
            "valid_catalog_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_object_list_metadata(
        payload,
        kind_names=GATEWAY_COMPLIANCE_PREDECESSOR_BOUND_KINDS,
        metadata_field="valid_catalog_history_bindings",
        error=(
            "gateway_compliance predecessor-bound artifact fingerprints must match "
            "valid_catalog_history_bindings"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=GATEWAY_COMPLIANCE_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "gateway_compliance policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_pdp_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate PDP metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=PDP_PROOF_SUMMARY_BOUND_KINDS,
        metadata_field="valid_proof_summary_digests",
        fingerprint_field="proof_summary_digest_hex",
        error=(
            "pdp proof-summary-bound artifact fingerprints must match "
            "valid_proof_summary_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=PDP_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "pdp policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=PDP_PROVIDER_ROSTER_BOUND_KINDS,
        metadata_field="valid_provider_roster_digests",
        fingerprint_field="provider_roster_digest_hex",
        error=(
            "pdp provider-roster-bound artifact fingerprints must match "
            "valid_provider_roster_digests"
        ),
        errors=errors,
    )


def validate_orderbook_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate orderbook metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=ORDERBOOK_CONTRACT_BOUND_KINDS,
        metadata_field="valid_contract_digests",
        fingerprint_field="contract_digest_hex",
        error=(
            "orderbook contract-bound artifact fingerprints must match "
            "valid_contract_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=ORDERBOOK_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "orderbook policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_potr_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate PoTR metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=POTR_RECEIPT_SUMMARY_BOUND_KINDS,
        metadata_field="valid_receipt_summary_digests",
        fingerprint_field="receipt_summary_digest_hex",
        error=(
            "potr receipt-summary-bound artifact fingerprints must match "
            "valid_receipt_summary_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=POTR_PQ_KEY_ROSTER_BOUND_KINDS,
        metadata_field="valid_pq_key_roster_digests",
        fingerprint_field="pq_key_roster_digest_hex",
        error=(
            "potr pq-key-roster-bound artifact fingerprints must match "
            "valid_pq_key_roster_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=POTR_REPUTATION_WEIGHT_BOUND_KINDS,
        metadata_field="valid_reputation_weight_policy_digests",
        fingerprint_field="reputation_weight_policy_digest_hex",
        error=(
            "potr reputation-weight-bound artifact fingerprints must match "
            "valid_reputation_weight_policy_digests"
        ),
        errors=errors,
    )


def validate_por_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate PoR metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=POR_SEED_REPLAY_BOUND_KINDS,
        metadata_field="valid_seed_replay_digests",
        fingerprint_field="seed_replay_digest_hex",
        error=(
            "por seed-replay-bound artifact fingerprints must match "
            "valid_seed_replay_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=POR_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "por policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_repair_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate repair metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=REPAIR_ROSTER_BOUND_KINDS,
        metadata_field="valid_roster_digests",
        fingerprint_field="roster_digest_hex",
        error=(
            "repair roster-bound artifact fingerprints must match "
            "valid_roster_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=REPAIR_FAILURE_BOUND_KINDS,
        metadata_field="valid_failure_bundle_digests",
        fingerprint_field="evidence_bundle_digest_hex",
        error=(
            "repair failure-bound artifact fingerprints must match "
            "valid_failure_bundle_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=REPAIR_HANDOFF_BOUND_KINDS,
        metadata_field="valid_handoff_digests",
        fingerprint_field="handoff_digest_hex",
        error=(
            "repair handoff-bound artifact fingerprints must match "
            "valid_handoff_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=REPAIR_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "repair policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )


def validate_reference_sdk_release_bound_artifact_metadata(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require aggregate reference SDK release metadata to preserve lane bindings."""

    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=REFERENCE_SDK_RELEASE_MANIFEST_BOUND_KINDS,
        metadata_field="valid_release_manifest_digests",
        fingerprint_field="release_manifest_digest_hex",
        error=(
            "reference_sdk_release manifest-bound artifact fingerprints must match "
            "valid_release_manifest_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=REFERENCE_SDK_POLICY_BOUND_KINDS,
        metadata_field="valid_policy_digests",
        fingerprint_field="policy_digest_hex",
        error=(
            "reference_sdk_release policy-bound artifact fingerprints must match "
            "valid_policy_digests"
        ),
        errors=errors,
    )
    bound_artifact_fingerprints_match_hex_list_metadata(
        payload,
        kind_names=("governance_approval",),
        metadata_field="valid_release_key_fingerprints",
        fingerprint_field="public_key_fingerprint_hex",
        error=(
            "reference_sdk_release governance approval release-key fingerprints "
            "must match valid_release_key_fingerprints"
        ),
        errors=errors,
    )
    for metadata_field, fingerprint_field, diagnostic in (
        (
            "valid_sbom_index_digests",
            "sbom_index_digest_hex",
            "SBOM index",
        ),
        (
            "valid_vulnerability_report_digests",
            "vulnerability_report_digest_hex",
            "vulnerability report",
        ),
        (
            "valid_provenance_bundle_digests",
            "provenance_bundle_digest_hex",
            "provenance bundle",
        ),
    ):
        bound_artifact_fingerprints_match_hex_list_metadata(
            payload,
            kind_names=("supply_chain",),
            metadata_field=metadata_field,
            fingerprint_field=fingerprint_field,
            error=(
                "reference_sdk_release supply-chain "
                f"{diagnostic} fingerprints must match {metadata_field}"
            ),
            errors=errors,
        )
    supply_chain_fingerprints = payload_free_summary_artifact_fingerprints(
        payload,
        kind_name="supply_chain",
    )
    trust_tuples: set[tuple[str, str, str]] = set()
    for index, fingerprint in enumerate(supply_chain_fingerprints):
        validate_reference_sdk_supply_chain_fingerprint(
            fingerprint,
            errors,
            path=(
                "reference_sdk_release supply_chain "
                f"fingerprint[{index}]"
            ),
        )
        certificate_identity = canonical_public_provenance_url(
            fingerprint.get("provenance_certificate_identity")
        )
        oidc_issuer = canonical_public_provenance_url(
            fingerprint.get("provenance_oidc_issuer")
        )
        verification_key_fingerprint = canonical_lower_hex(
            fingerprint.get("provenance_verification_key_fingerprint_hex"),
            64,
        )
        if (
            certificate_identity is not None
            and oidc_issuer is not None
            and verification_key_fingerprint is not None
            and any(
                character != "0"
                for character in verification_key_fingerprint
            )
        ):
            trust_tuples.add(
                (
                    certificate_identity,
                    oidc_issuer,
                    verification_key_fingerprint,
                )
            )
    if len(trust_tuples) > 1:
        errors.append(
            "reference_sdk_release supply-chain fingerprints must share one "
            "operator provenance trust tuple"
        )


def validate_reference_sdk_supply_chain_fingerprint(
    fingerprint: dict[str, Any],
    errors: list[str],
    *,
    path: str,
) -> None:
    """Require the source-bound, public SF-11 supply-chain fingerprint."""

    source_artifacts = fingerprint.get("source_artifacts")
    if not isinstance(source_artifacts, list):
        errors.append(f"{path}.source_artifacts must be an array")
        source_artifacts = []
    expected_kinds = REFERENCE_SDK_SUPPLY_CHAIN_SOURCE_ARTIFACT_KINDS
    if len(source_artifacts) != len(expected_kinds):
        errors.append(
            f"{path}.source_artifacts must contain exactly four bindings"
        )

    observed_kinds: list[str] = []
    artifact_paths: list[str] = []
    artifact_digests: list[str] = []
    source_digests: dict[str, str] = {}
    for index, value in enumerate(source_artifacts):
        row_path = f"{path}.source_artifacts[{index}]"
        if not isinstance(value, dict):
            errors.append(f"{row_path} must be an object")
            continue
        if set(value) != REFERENCE_SDK_SUPPLY_CHAIN_SOURCE_ARTIFACT_FIELDS:
            errors.append(
                f"{row_path} fields must match the exact source binding contract"
            )
        kind = canonical_string(value.get("kind"))
        if kind is None:
            errors.append(f"{row_path}.kind must be canonical")
        else:
            observed_kinds.append(kind)
        artifact_path = canonical_string(value.get("artifact_path"))
        if (
            artifact_path is None
            or not is_archive_portable_artifact_path(artifact_path)
        ):
            errors.append(
                f"{row_path}.artifact_path must be archive-relative and portable"
            )
        else:
            artifact_paths.append(artifact_path)
        digest = _nonzero_sha256(
            value.get("sha256"),
            path=f"{row_path}.sha256",
            errors=errors,
        )
        if digest is not None:
            artifact_digests.append(digest)
            if kind in expected_kinds and kind not in source_digests:
                source_digests[kind] = digest

    if observed_kinds != list(expected_kinds):
        errors.append(
            f"{path}.source_artifacts kinds must match the exact canonical order"
        )
    if len(artifact_paths) != len(set(artifact_paths)):
        errors.append(f"{path}.source_artifacts paths must be unique")
    if len(artifact_digests) != len(set(artifact_digests)):
        errors.append(f"{path}.source_artifacts SHA-256 digests must be unique")

    for source_kind, fingerprint_field in (
        REFERENCE_SDK_SUPPLY_CHAIN_SOURCE_DIGEST_BINDINGS
    ):
        fingerprint_digest = _nonzero_sha256(
            fingerprint.get(fingerprint_field),
            path=f"{path}.{fingerprint_field}",
            errors=errors,
        )
        source_digest = source_digests.get(source_kind)
        if (
            source_digest is not None
            and fingerprint_digest is not None
            and source_digest != fingerprint_digest
        ):
            errors.append(
                f"{path}.{source_kind} source digest must match "
                f"{fingerprint_field}"
            )

    if (
        canonical_public_provenance_url(
            fingerprint.get("provenance_certificate_identity")
        )
        is None
    ):
        errors.append(
            f"{path}.provenance_certificate_identity must be a canonical "
            "public HTTPS identity"
        )
    if (
        canonical_public_provenance_url(
            fingerprint.get("provenance_oidc_issuer")
        )
        is None
    ):
        errors.append(
            f"{path}.provenance_oidc_issuer must be a canonical public "
            "HTTPS issuer"
        )
    _nonzero_sha256(
        fingerprint.get("provenance_verification_key_fingerprint_hex"),
        path=f"{path}.provenance_verification_key_fingerprint_hex",
        errors=errors,
    )


def validate_payload_free_cross_metadata_bindings(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Validate lane-specific relationships between metadata fields."""

    if gate.name == "ai_prescreen":
        validate_ai_prescreen_bound_artifact_metadata(payload, errors)
    if gate.name == "appeal_finance":
        validate_appeal_finance_multi_peer_metadata(payload, errors)
        validate_appeal_finance_bound_artifact_metadata(payload, errors)
    if gate.name == "governance_dag":
        validate_governance_dag_bound_artifact_metadata(payload, errors)
    if gate.name == "gateway_load":
        validate_gateway_load_bound_artifact_metadata(payload, errors)
    if gate.name == "gateway_compliance":
        validate_gateway_compliance_bound_artifact_metadata(payload, errors)
    if gate.name == "hedging_billing":
        validate_hedging_billing_cycle_metadata(payload, errors)
        validate_hedging_billing_bound_artifact_metadata(payload, errors)
    if gate.name == "moderation_panel":
        validate_moderation_panel_chain_metadata(payload, errors)
        validate_moderation_panel_bound_artifact_metadata(payload, errors)
    if gate.name == "orderbook":
        validate_orderbook_bound_artifact_metadata(payload, errors)
    if gate.name == "pdp":
        validate_pdp_bound_artifact_metadata(payload, errors)
    if gate.name == "pop_credentials":
        validate_pop_credentials_sync_metadata(payload, errors)
        validate_pop_credentials_bound_artifact_metadata(payload, errors)
    if gate.name == "por":
        validate_por_bound_artifact_metadata(payload, errors)
    if gate.name == "potr":
        validate_potr_bound_artifact_metadata(payload, errors)
    if gate.name == "repair":
        validate_repair_bound_artifact_metadata(payload, errors)
    if gate.name == "reference_sdk_release":
        validate_reference_sdk_release_bound_artifact_metadata(payload, errors)
    if gate.name == "reputation":
        validate_reputation_snapshot_metadata(payload, errors)
        validate_reputation_bound_artifact_metadata(payload, errors)
    if gate.name == "reserve_rent":
        validate_reserve_rent_policy_matrix_metadata(payload, errors)
        validate_reserve_rent_bound_artifact_metadata(payload, errors)
    if gate.name == "transparency":
        validate_transparency_publication_binding_metadata(payload, errors)
        validate_transparency_bound_artifact_metadata(payload, errors)


def validate_payload_free_summary_metadata(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    errors: list[str],
    *,
    enforce_production_deployment_context: bool = False,
) -> None:
    """Validate required top-level lane metadata shapes."""

    allowed_metadata_fields = GATE_METADATA_FIELDS.get(gate.name, frozenset())
    for field in sorted(allowed_metadata_fields):
        if field not in payload:
            errors.append(f"{field} is required for `{gate.name}` lane metadata")
    for field in sorted(PAYLOAD_FREE_SUMMARY_METADATA_FIELDS):
        if field not in payload:
            continue
        if field not in allowed_metadata_fields:
            errors.append(f"{field} is not allowed for `{gate.name}` lane metadata")
            continue
        value = payload.get(field)
        if field in PAYLOAD_FREE_SUMMARY_LIST_METADATA_FIELDS and not isinstance(
            value,
            list,
        ):
            errors.append(f"{field} must be a payload-free metadata list")
            continue
        if field in PAYLOAD_FREE_SUMMARY_LIST_METADATA_FIELDS and not value:
            errors.append(f"{field} must not be empty for `{gate.name}` lane metadata")
        if field in PAYLOAD_FREE_SUMMARY_ORDERED_LIST_METADATA_FIELDS:
            validate_payload_free_ordered_list_metadata(field, value, errors)
        if (
            field in PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS
            or field in PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_BINDINGS
        ):
            validate_payload_free_allowed_string_list_values(
                gate,
                field,
                value,
                errors,
            )
            validate_payload_free_required_string_list_values(
                gate,
                field,
                value,
                errors,
            )
            validate_payload_free_provider_id_metadata_values(
                gate,
                field,
                value,
                errors,
            )
            validate_payload_free_string_list_count_binding(
                gate,
                payload,
                field,
                value,
                errors,
            )
        object_list_schema = PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS.get(field)
        if object_list_schema is not None:
            validate_payload_free_object_list_metadata(
                field,
                value,
                object_list_schema,
                errors,
            )
            validate_payload_free_object_list_string_field_policies(
                gate,
                field,
                value,
                errors,
            )
            if enforce_production_deployment_context:
                validate_payload_free_object_list_deployment_context_metadata(
                    field,
                    value,
                    object_list_schema,
                    errors,
                )
            continue
        binding_fields = PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS.get(field)
        if binding_fields is not None:
            for index, item in enumerate(value):
                item_path = f"{field}[{index}]"
                if not isinstance(item, dict):
                    errors.append(f"{item_path} must be a payload-free binding object")
                    continue
                for key in item:
                    key_label = canonical_string(key)
                    if key_label is None:
                        errors.append(f"{item_path} keys must be canonical strings")
                    elif key_label not in binding_fields:
                        key_diagnostic_label = payload_free_diagnostic_key_label(
                            key_label
                        )
                        errors.append(
                            f"{item_path}.{key_diagnostic_label} is not allowed in payload-free binding metadata"
                        )
                for key, expected_hex_length in binding_fields.items():
                    item_value = item.get(key)
                    if (
                        not isinstance(item_value, str)
                        or len(item_value) != expected_hex_length
                        or any(
                            character not in LOWER_HEX_DIGITS
                            for character in item_value
                        )
                    ):
                        errors.append(
                            f"{item_path}.{key} must be "
                            f"{expected_hex_length} lowercase hex characters"
                        )
            continue
        if field in PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS:
            for index, item in enumerate(value):
                if (
                    not isinstance(item, str)
                    or len(item) != 64
                    or any(character not in LOWER_HEX_DIGITS for character in item)
                ):
                    errors.append(
                        f"{field}[{index}] must be 64 lowercase hex characters"
                    )
            continue
        if field in PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS:
            for index, item in enumerate(value):
                if not isinstance(item, int) or isinstance(item, bool) or item <= 0:
                    errors.append(f"{field}[{index}] must be a positive integer")
            continue
        if (
            field in PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_LIST_BINDINGS
            or field in PAYLOAD_FREE_SUMMARY_FINGERPRINT_STRING_ARRAY_LIST_BINDINGS
        ):
            for index, item in enumerate(value):
                if canonical_string(item) is None:
                    errors.append(f"{field}[{index}] must be a canonical string")
            continue
        if field in PAYLOAD_FREE_SUMMARY_STRING_METADATA_FIELDS:
            expected_hex_length = PAYLOAD_FREE_SUMMARY_HEX_METADATA_LENGTHS[field]
            if (
                canonical_string(value) is None
                or not isinstance(value, str)
                or len(value) != expected_hex_length
                or any(character not in LOWER_HEX_DIGITS for character in value)
            ):
                errors.append(
                    f"{field} must be {expected_hex_length} lowercase hex characters"
                )
            continue
        object_fields = PAYLOAD_FREE_SUMMARY_OBJECT_METADATA_FIELDS.get(field)
        if object_fields is not None:
            validate_payload_free_object_metadata(field, value, object_fields, errors)
            if enforce_production_deployment_context:
                validate_payload_free_deployment_context_metadata(field, value, errors)
            continue
        errors.append(f"{field} validator is not configured for `{gate.name}`")
    if payload.get("required_kinds") == list(gate.required_kinds):
        validate_payload_free_cross_metadata_bindings(gate, payload, errors)


def require_payload_free_artifact_fields(
    artifact: dict[str, Any],
    path: str,
    errors: list[str],
) -> None:
    """Reject non-standard artifact fields before aggregate promotion."""

    for key in artifact:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(f"{path} keys must be canonical strings")
        elif key_label not in PAYLOAD_FREE_ARTIFACT_FIELDS:
            key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
            errors.append(
                f"{path}.{key_diagnostic_label} is not allowed in payload-free artifact summary"
            )


def require_payload_free_required_row_fields(
    row: dict[str, Any],
    path: str,
    errors: list[str],
) -> None:
    """Reject non-standard required-row fields before aggregate promotion."""

    for key in row:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(f"{path} keys must be canonical strings")
        elif key_label not in PAYLOAD_FREE_REQUIRED_ROW_FIELDS:
            key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
            errors.append(
                f"{path}.{key_diagnostic_label} is not allowed in payload-free required row"
            )


def artifact_identity(
    kind_name: str,
    artifact: dict[str, Any],
) -> tuple[str, str, str] | None:
    """Return a comparable artifact identity when path and digest are canonical."""

    artifact_path = canonical_string(artifact.get("path"))
    sha256 = artifact.get("sha256")
    if (
        artifact_path is None
        or not is_archive_portable_artifact_path(artifact_path)
        or not isinstance(sha256, str)
        or len(sha256) != 64
        or any(character not in LOWER_HEX_DIGITS for character in sha256)
    ):
        return None
    return kind_name, artifact_path, sha256


def artifact_generated_at(
    fingerprint: dict[str, Any],
    path: str,
    errors: list[str],
) -> int | None:
    """Return an artifact generation timestamp from its fingerprint."""

    generated_at = fingerprint.get("generated_at_unix")
    if (
        not isinstance(generated_at, int)
        or isinstance(generated_at, bool)
        or generated_at <= 0
    ):
        errors.append(f"{path}.fingerprint.generated_at_unix must be positive")
        return None
    return generated_at


def validate_payload_free_artifact_fingerprint(
    artifact: dict[str, Any],
    path: str,
    errors: list[str],
) -> dict[str, Any] | None:
    """Require artifact fingerprints to carry only payload-free metadata."""

    fingerprint = artifact.get("fingerprint")
    if not isinstance(fingerprint, dict):
        errors.append(f"{path}.fingerprint must be an object")
        return None
    validate_payload_free_metadata_value(fingerprint, f"{path}.fingerprint", errors)
    return fingerprint


def artifact_deployment_context(
    fingerprint: dict[str, Any],
    path: str,
    errors: list[str],
    *,
    enforce_production_deployment_context: bool = False,
) -> tuple[str, str] | None:
    """Return the deployment context recorded in an artifact fingerprint."""

    deployment_id = canonical_string(fingerprint.get("deployment_id"))
    environment = canonical_string(fingerprint.get("environment"))
    reviewed = fingerprint.get("deployment_context_reviewed")
    if deployment_id is None:
        errors.append(f"{path}.fingerprint.deployment_id must be canonical")
    if environment is None:
        errors.append(f"{path}.fingerprint.environment must be canonical")
    if reviewed is not True:
        errors.append(f"{path}.fingerprint.deployment_context_reviewed must be true")
    if enforce_production_deployment_context and deployment_id is not None:
        require_production_deployment_id_value(
            deployment_id,
            errors,
            f"{path}.fingerprint.deployment_id",
        )
    if (
        enforce_production_deployment_context
        and environment is not None
        and not is_production_ready_environment(environment)
    ):
        errors.append(f"{path}.fingerprint.environment must be production")
    if deployment_id is None or environment is None or reviewed is not True:
        return None
    return deployment_id, environment


def validate_summary_artifact(
    artifact: dict[str, Any],
    path: str,
    options: ValidationOptions,
    errors: list[str],
) -> tuple[list[int], set[tuple[str, str]]]:
    """Validate one payload-free artifact summary row."""

    require_payload_free_artifact_fields(artifact, path, errors)
    require_artifact_identity_fields(artifact, path, errors)
    require_optional_artifact_label(artifact, "schema", path, errors)
    require_optional_artifact_label(artifact, "status", path, errors)
    status = canonical_string(artifact.get("status"))
    if status is None:
        errors.append(f"{path}.status must be canonical")
    elif status not in SUCCESS_ARTIFACT_STATUSES:
        errors.append(f"{path}.status must be a successful status")
    if artifact.get("valid") is not True:
        errors.append(f"{path}.valid must be true")
    require_empty_error_list(
        artifact.get("errors"),
        f"{path}.errors",
        errors,
    )
    fingerprint = validate_payload_free_artifact_fingerprint(artifact, path, errors)
    generated_times: list[int] = []
    deployment_contexts: set[tuple[str, str]] = set()
    if fingerprint is not None:
        generated_at = artifact_generated_at(fingerprint, path, errors)
        if generated_at is not None:
            if generated_at > options.now_unix:
                errors.append(f"{path}.fingerprint.generated_at_unix must not be future")
            elif (
                options.now_unix - generated_at
                > options.max_summary_artifact_age_secs
            ):
                errors.append(
                    f"{path}.fingerprint.generated_at_unix exceeds max summary artifact age"
                )
            generated_times.append(generated_at)
        context = artifact_deployment_context(
            fingerprint,
            path,
            errors,
            enforce_production_deployment_context=is_production_ready_environment(
                options.environment
            ),
        )
        if context is not None:
            deployment_contexts.add(context)
    return generated_times, deployment_contexts


def validate_required_row(
    gate_name: str,
    kind_name: str,
    expected_schema: str | None,
    row: Any,
    options: ValidationOptions,
    errors: list[str],
) -> tuple[int, list[int], set[tuple[str, str]]]:
    """Validate one required-kind row from a lane summary."""

    path = f"{gate_name}.required.{kind_name}"
    if not isinstance(row, dict):
        errors.append(f"{path} must be an object")
        return 0, [], set()
    require_payload_free_required_row_fields(row, path, errors)
    row_schema = canonical_string(row.get("schema"))
    if row_schema is None:
        errors.append(f"{path}.schema must be canonical")
    elif expected_schema is None:
        errors.append(f"{path}.schema gate contract is not configured")
    elif row_schema != expected_schema:
        errors.append(f"{path}.schema must match required evidence schema")
    if row.get("present") is not True:
        errors.append(f"{path}.present must be true")
    if row.get("valid") is not True:
        errors.append(f"{path}.valid must be true")
    artifact_count = row.get("artifact_count")
    if (
        not isinstance(artifact_count, int)
        or isinstance(artifact_count, bool)
        or artifact_count <= 0
    ):
        errors.append(f"{path}.artifact_count must be positive")
        artifact_count = 0
    require_empty_error_list(row.get("errors"), f"{path}.errors", errors)
    artifacts = row.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        errors.append(f"{path}.artifacts must be a non-empty array")
        if artifact_count:
            errors.append(f"{path}.artifact_count must match artifact object count")
        return 0, [], set()
    observed_artifact_count = sum(
        1 for artifact in artifacts if isinstance(artifact, dict)
    )
    if artifact_count and artifact_count != observed_artifact_count:
        errors.append(f"{path}.artifact_count must match artifact object count")
    artifact_count = observed_artifact_count

    generated_times: list[int] = []
    deployment_contexts: set[tuple[str, str]] = set()
    artifact_paths: Counter[str] = Counter()
    artifact_identities: Counter[tuple[str, str, str]] = Counter()
    for index, artifact in enumerate(artifacts):
        artifact_path = f"{path}.artifacts[{index}]"
        if not isinstance(artifact, dict):
            errors.append(f"{artifact_path} must be an object")
            continue
        artifact_file_path = canonical_string(artifact.get("path"))
        if artifact_file_path is not None:
            artifact_paths[artifact_file_path] += 1
        if "kind" in artifact:
            artifact_kind = canonical_string(artifact.get("kind"))
            if artifact_kind is None:
                errors.append(f"{artifact_path}.kind must be canonical when present")
            elif artifact_kind != kind_name:
                errors.append(f"{artifact_path}.kind must match required row kind")
        artifact_schema = canonical_string(artifact.get("schema"))
        if artifact_schema is None:
            errors.append(f"{artifact_path}.schema must be canonical")
        elif expected_schema is None:
            errors.append(f"{artifact_path}.schema gate contract is not configured")
        elif artifact_schema != expected_schema:
            errors.append(f"{artifact_path}.schema must match required evidence schema")
        identity = artifact_identity(kind_name, artifact)
        if identity is not None:
            artifact_identities[identity] += 1
        row_times, row_contexts = validate_summary_artifact(
            artifact,
            artifact_path,
            options,
            errors,
        )
        generated_times.extend(row_times)
        deployment_contexts.update(row_contexts)
    if any(count > 1 for count in artifact_paths.values()):
        errors.append(f"{path}.artifacts must not duplicate artifact paths")
    if any(count > 1 for count in artifact_identities.values()):
        errors.append(f"{path}.artifacts must not duplicate artifact identities")
    return artifact_count, generated_times, deployment_contexts


def validate_recognized_artifacts(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    required: dict[str, Any],
    recognized_artifact_count: int | None,
    options: ValidationOptions,
    errors: list[str],
) -> tuple[list[int], set[tuple[str, str]], int | None, int | None]:
    """Validate required top-level recognized artifact inventory rows."""

    if "recognized_artifacts" not in payload:
        errors.append("recognized_artifacts must be present")
        return [], set(), None, None
    artifacts = payload.get("recognized_artifacts")
    path = "recognized_artifacts"
    if not isinstance(artifacts, list) or not artifacts:
        errors.append(f"{path} must be a non-empty array")
        return [], set(), None, None
    recognized_artifact_object_count = sum(
        1 for artifact in artifacts if isinstance(artifact, dict)
    )
    if (
        recognized_artifact_count is not None
        and len(artifacts) != recognized_artifact_count
    ):
        errors.append(
            "recognized_artifacts length must match recognized_artifact_count"
        )
    if (
        recognized_artifact_count is not None
        and recognized_artifact_object_count != recognized_artifact_count
    ):
        errors.append(
            "recognized_artifact_count must match recognized artifact object count"
        )

    generated_times: list[int] = []
    deployment_contexts: set[tuple[str, str]] = set()
    recognized_artifact_paths: set[str] = set()
    recognized_artifact_path_counts: Counter[str] = Counter()
    expected_required_kinds = set(gate.required_kinds)
    expected_artifact_counts = {
        kind_name: (
            sum(
                1
                for artifact in row.get("artifacts")
                if isinstance(artifact, dict)
            )
            if isinstance(row, dict) and isinstance(row.get("artifacts"), list)
            else 0
        )
        for kind_name, row in required.items()
        if kind_name in expected_required_kinds
    }
    recognized_artifact_counts = {kind_name: 0 for kind_name in expected_required_kinds}
    expected_artifact_identities: Counter[tuple[str, str, str]] = Counter()
    required_artifacts_by_identity: dict[tuple[str, str, str], dict[str, Any]] = {}
    for kind_name, row in required.items():
        if kind_name not in expected_required_kinds or not isinstance(row, dict):
            continue
        required_artifacts = row.get("artifacts")
        if not isinstance(required_artifacts, list):
            continue
        for artifact in required_artifacts:
            if not isinstance(artifact, dict):
                continue
            identity = artifact_identity(kind_name, artifact)
            if identity is not None:
                expected_artifact_identities[identity] += 1
                required_artifacts_by_identity.setdefault(identity, artifact)
    recognized_artifact_identities: Counter[tuple[str, str, str]] = Counter()
    for index, artifact in enumerate(artifacts):
        artifact_path = f"{path}[{index}]"
        if not isinstance(artifact, dict):
            errors.append(f"{artifact_path} must be an object")
            continue
        artifact_file_path = canonical_string(artifact.get("path"))
        if artifact_file_path is not None and is_archive_portable_artifact_path(
            artifact_file_path
        ):
            recognized_artifact_paths.add(artifact_file_path)
            recognized_artifact_path_counts[artifact_file_path] += 1
        kind_name = canonical_string(artifact.get("kind"))
        if kind_name is None:
            errors.append(f"{artifact_path}.kind must be canonical")
        elif kind_name not in expected_required_kinds:
            errors.append(
                f"{artifact_path}.kind must be part of the full `{gate.name}` "
                "gate contract"
            )
        else:
            recognized_artifact_counts[kind_name] += 1
            identity = artifact_identity(kind_name, artifact)
            if identity is not None:
                recognized_artifact_identities[identity] += 1
                required_artifact = required_artifacts_by_identity.get(identity)
                if required_artifact is not None:
                    for metadata_field in ("schema", "status", "fingerprint"):
                        if artifact.get(metadata_field) != required_artifact.get(
                            metadata_field
                        ):
                            errors.append(
                                f"{artifact_path}.{metadata_field} must match "
                                "the required artifact metadata"
                            )
        row_times, row_contexts = validate_summary_artifact(
            artifact,
            artifact_path,
            options,
            errors,
        )
        generated_times.extend(row_times)
        deployment_contexts.update(row_contexts)
    mismatched_counts = [
        kind_name
        for kind_name in sorted(expected_required_kinds)
        if expected_artifact_counts.get(kind_name, 0)
        != recognized_artifact_counts.get(kind_name, 0)
    ]
    if mismatched_counts:
        errors.append("recognized_artifacts must match required artifact counts")
    if any(count > 1 for count in recognized_artifact_path_counts.values()):
        errors.append("recognized_artifacts must not duplicate artifact paths")
    missing_identities = expected_artifact_identities - recognized_artifact_identities
    unexpected_identities = recognized_artifact_identities - expected_artifact_identities
    if missing_identities or unexpected_identities:
        errors.append("recognized_artifacts must match required artifact identities")
    return (
        generated_times,
        deployment_contexts,
        recognized_artifact_object_count,
        len(recognized_artifact_paths),
    )


def validate_aggregate_gate_row_output(
    gate: GateSummaryKind,
    row: dict[str, Any],
    errors: list[str],
) -> None:
    """Validate the schema-closed aggregate lane row emitted for release review."""

    row_fields = set(row)
    missing_fields = AGGREGATE_REQUIRED_GATE_ROW_FIELDS - row_fields
    extra_fields = row_fields - AGGREGATE_REQUIRED_GATE_ROW_FIELDS
    if missing_fields or extra_fields:
        errors.append(
            f"{gate.name} aggregate row fields must match the schema-closed output contract"
        )
    for key in extra_fields:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(f"{gate.name} aggregate row keys must be canonical strings")
        else:
            key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
            errors.append(
                f"{gate.name} aggregate row {key_diagnostic_label} is not allowed"
            )
    if row.get("schema") != gate.schema:
        errors.append(f"{gate.name} aggregate row schema must match gate schema")
    errors.extend(
        validate_topology_binding_object(
            row.get("topology_qualification"),
            path=f"{gate.name} aggregate row topology_qualification",
        )
    )
    if row.get("present") is not True:
        errors.append(f"{gate.name} aggregate row present must be true")
    if row.get("valid") is not True:
        errors.append(f"{gate.name} aggregate row valid must be true")
    for field in (
        "required_kind_count",
        "expected_required_kind_count",
        "evidence_file_count",
        "recognized_artifact_count",
        "artifact_count",
        "oldest_generated_at_unix",
        "newest_generated_at_unix",
    ):
        value = row.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
            errors.append(f"{gate.name} aggregate row {field} must be a positive integer")
    evidence_file_count = canonical_int_value(row.get("evidence_file_count"))
    recognized_artifact_count = canonical_int_value(
        row.get("recognized_artifact_count")
    )
    artifact_count = canonical_int_value(row.get("artifact_count"))
    if (
        evidence_file_count is not None
        and recognized_artifact_count is not None
        and evidence_file_count > 0
        and recognized_artifact_count > 0
        and evidence_file_count > recognized_artifact_count
    ):
        errors.append(
            f"{gate.name} aggregate row evidence_file_count must not exceed recognized_artifact_count"
        )
    if (
        recognized_artifact_count is not None
        and artifact_count is not None
        and recognized_artifact_count > 0
        and artifact_count > 0
        and recognized_artifact_count != artifact_count
    ):
        errors.append(
            f"{gate.name} aggregate row recognized_artifact_count must match artifact_count"
        )
    oldest = row.get("oldest_generated_at_unix")
    newest = row.get("newest_generated_at_unix")
    if (
        isinstance(oldest, int)
        and not isinstance(oldest, bool)
        and isinstance(newest, int)
        and not isinstance(newest, bool)
        and oldest > 0
        and newest > 0
        and newest < oldest
    ):
        errors.append(
            f"{gate.name} aggregate row newest_generated_at_unix must be >= oldest_generated_at_unix"
        )
    if row.get("required_kind_count") != len(gate.required_kinds):
        errors.append(
            f"{gate.name} aggregate row required_kind_count must match gate contract"
        )
    if row.get("expected_required_kind_count") != len(gate.required_kinds):
        errors.append(
            f"{gate.name} aggregate row expected_required_kind_count must match gate contract"
        )
    thresholds_errors: list[str] = []
    require_threshold_map(row, "thresholds", thresholds_errors)
    for threshold_error in thresholds_errors:
        errors.append(f"{gate.name} aggregate row {threshold_error}")
    require_production_deployment_id_value(
        row.get("deployment_id"),
        errors,
        f"{gate.name} aggregate row deployment_id",
    )
    if canonical_string(row.get("environment")) is None:
        errors.append(f"{gate.name} aggregate row environment must be canonical")
    elif not is_production_ready_environment(row.get("environment")):
        errors.append(f"{gate.name} aggregate row environment must be production")
    expected_required_kinds = row.get("expected_required_kinds")
    if list(gate.required_kinds) != expected_required_kinds:
        errors.append(
            f"{gate.name} aggregate row expected_required_kinds must match gate contract"
        )
    require_empty_error_list(
        row.get("errors"),
        f"{gate.name} aggregate row errors",
        errors,
    )
    if canonical_string(row.get("path")) is None:
        errors.append(f"{gate.name} aggregate row path must be canonical")
    elif not is_archive_portable_artifact_path(row["path"]):
        errors.append(
            f"{gate.name} aggregate row path must be archive-relative without "
            "absolute, empty, current, parent, or platform-specific segments"
        )
    sha256 = row.get("sha256")
    if (
        not isinstance(sha256, str)
        or len(sha256) != 64
        or any(character not in LOWER_HEX_DIGITS for character in sha256)
    ):
        errors.append(
            f"{gate.name} aggregate row sha256 must be canonical lowercase SHA-256"
        )


def validate_aggregate_row_error_list(
    value: Any,
    path: str,
    errors: list[str],
    *,
    require_non_empty: bool,
) -> None:
    """Validate canonical aggregate row diagnostics without requiring emptiness."""

    if not isinstance(value, list):
        errors.append(f"{path} must be a list")
        return
    if require_non_empty and not value:
        errors.append(f"{path} must not be empty")
    seen_errors: set[str] = set()
    for error in value:
        error_label = canonical_string(error)
        if error_label is None:
            errors.append(f"{path} must contain canonical strings")
            return
        if error_label in seen_errors:
            errors.append(f"{path} must not contain duplicate diagnostics")
            return
        seen_errors.add(error_label)


def validate_aggregate_required_row_output(
    gate: GateSummaryKind,
    row: Any,
    errors: list[str],
) -> None:
    """Validate final aggregate required-row shape before summary emission."""

    if not isinstance(row, dict):
        errors.append(f"{gate.name} aggregate required row must be an object")
        return
    present = row.get("present")
    valid = row.get("valid")
    expected_fields = (
        AGGREGATE_REQUIRED_GATE_ROW_FIELDS
        if present is True
        else AGGREGATE_MISSING_GATE_ROW_FIELDS
    )
    row_fields = set(row)
    missing_fields = expected_fields - row_fields
    extra_fields = row_fields - expected_fields
    if missing_fields or extra_fields:
        errors.append(
            f"{gate.name} aggregate required row fields must match the schema-closed output contract"
        )
    for key in extra_fields:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(
                f"{gate.name} aggregate required row keys must be canonical strings"
            )
        else:
            key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
            errors.append(
                f"{gate.name} aggregate required row {key_diagnostic_label} is not allowed"
            )
    if row.get("schema") != gate.schema:
        errors.append(f"{gate.name} aggregate required row schema must match gate schema")
    if not isinstance(present, bool):
        errors.append(f"{gate.name} aggregate required row present must be boolean")
    if not isinstance(valid, bool):
        errors.append(f"{gate.name} aggregate required row valid must be boolean")
    if present is False and valid is not False:
        errors.append(f"{gate.name} aggregate missing row valid must be false")
    if present is False:
        validate_aggregate_row_error_list(
            row.get("errors"),
            f"{gate.name} aggregate missing row errors",
            errors,
            require_non_empty=True,
        )
        if row.get("errors") != [
            f"missing required {gate.name} production readiness summary"
        ]:
            errors.append(
                f"{gate.name} aggregate missing row errors must match the deterministic missing summary diagnostic"
            )
        return
    if present is not True:
        return
    if valid is True:
        validate_aggregate_gate_row_output(gate, row, errors)
        return
    for field in (
        "evidence_file_count",
        "recognized_artifact_count",
        "artifact_count",
    ):
        value = row.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            errors.append(
                f"{gate.name} aggregate invalid row {field} must be a non-negative integer"
            )
    evidence_file_count = canonical_int_value(row.get("evidence_file_count"))
    recognized_artifact_count = canonical_int_value(
        row.get("recognized_artifact_count")
    )
    artifact_count = canonical_int_value(row.get("artifact_count"))
    if (
        evidence_file_count is not None
        and recognized_artifact_count is not None
        and evidence_file_count >= 0
        and recognized_artifact_count >= 0
        and evidence_file_count > recognized_artifact_count
    ):
        errors.append(
            f"{gate.name} aggregate invalid row evidence_file_count must not exceed recognized_artifact_count"
        )
    if (
        recognized_artifact_count is not None
        and artifact_count is not None
        and recognized_artifact_count >= 0
        and artifact_count >= 0
        and recognized_artifact_count != artifact_count
    ):
        errors.append(
            f"{gate.name} aggregate invalid row recognized_artifact_count must match artifact_count"
        )
    for field in ("oldest_generated_at_unix", "newest_generated_at_unix"):
        value = row.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
            errors.append(
                f"{gate.name} aggregate invalid row {field} must be a positive integer"
            )
    for field in ("required_kind_count", "expected_required_kind_count"):
        value = row.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
            errors.append(
                f"{gate.name} aggregate invalid row {field} must be a positive integer"
            )
    if row.get("required_kind_count") != len(gate.required_kinds):
        errors.append(
            f"{gate.name} aggregate invalid row required_kind_count must match gate contract"
        )
    if row.get("expected_required_kind_count") != len(gate.required_kinds):
        errors.append(
            f"{gate.name} aggregate invalid row expected_required_kind_count must match gate contract"
        )
    oldest = row.get("oldest_generated_at_unix")
    newest = row.get("newest_generated_at_unix")
    if (
        isinstance(oldest, int)
        and not isinstance(oldest, bool)
        and isinstance(newest, int)
        and not isinstance(newest, bool)
        and newest < oldest
    ):
        errors.append(
            f"{gate.name} aggregate invalid row newest_generated_at_unix must be >= oldest_generated_at_unix"
        )
    thresholds = row.get("thresholds")
    if thresholds is not None:
        thresholds_errors: list[str] = []
        require_threshold_map(row, "thresholds", thresholds_errors)
        for threshold_error in thresholds_errors:
            errors.append(f"{gate.name} aggregate invalid row {threshold_error}")
    if (
        row.get("deployment_id") is not None
    ):
        require_production_deployment_id_value(
            row.get("deployment_id"),
            errors,
            f"{gate.name} aggregate invalid row deployment_id",
        )
    environment = row.get("environment")
    if environment is not None:
        if canonical_string(environment) is None:
            errors.append(
                f"{gate.name} aggregate invalid row environment must be canonical when present"
            )
        elif not is_production_ready_environment(environment):
            errors.append(
                f"{gate.name} aggregate invalid row environment must be production when present"
            )
    if row.get("expected_required_kinds") != list(gate.required_kinds):
        errors.append(
            f"{gate.name} aggregate invalid row expected_required_kinds must match gate contract"
        )
    if canonical_string(row.get("path")) is None:
        errors.append(f"{gate.name} aggregate invalid row path must be canonical")
    elif not is_archive_portable_artifact_path(row["path"]):
        errors.append(
            f"{gate.name} aggregate invalid row path must be archive-relative without "
            "absolute, empty, current, parent, or platform-specific segments"
        )
    sha256 = row.get("sha256")
    if (
        not isinstance(sha256, str)
        or len(sha256) != 64
        or any(character not in LOWER_HEX_DIGITS for character in sha256)
    ):
        errors.append(
            f"{gate.name} aggregate invalid row sha256 must be canonical lowercase SHA-256"
        )
    validate_aggregate_row_error_list(
        row.get("errors"),
        f"{gate.name} aggregate invalid row errors",
        errors,
        require_non_empty=True,
    )


def validate_aggregate_foundational_prerequisite_readiness_summary_output(
    value: Any,
    lane_summary_rows: list[dict[str, str]],
    errors: list[str],
) -> list[dict[str, Any]]:
    """Validate the aggregate's preserved prerequisite-to-lane digest groups."""

    path = "aggregate foundational prerequisites prerequisite_readiness_summary_sha256"
    groups: list[dict[str, Any]] = []
    if not isinstance(value, list):
        errors.append(f"{path} must be an array")
        return groups
    observed_ids: list[str] = []
    for index, item in enumerate(value):
        row_path = f"{path}[{index}]"
        row = validate_foundational_exact_fields(
            item,
            AGGREGATE_FOUNDATIONAL_PREREQUISITE_READINESS_SUMMARY_ROW_FIELDS,
            row_path,
            errors,
        )
        if row is None:
            continue
        prerequisite_id = canonical_string(row.get("id"))
        if prerequisite_id is None:
            errors.append(f"{row_path}.id must be a canonical string")
        else:
            observed_ids.append(prerequisite_id)
        readiness_summary_rows = (
            validate_foundational_prerequisite_readiness_summary_rows(
                row.get("readiness_summary_sha256"),
                prerequisite_id,
                errors,
                path=f"{row_path}.readiness_summary_sha256",
            )
        )
        if prerequisite_id in FOUNDATIONAL_PREREQUISITE_LANES:
            groups.append(
                {
                    "id": prerequisite_id,
                    "readiness_summary_sha256": readiness_summary_rows,
                }
            )

    if observed_ids != list(FOUNDATIONAL_PREREQUISITE_IDS):
        errors.append(
            f"{path} must match the exact prerequisite set and canonical order"
        )
        if len(set(observed_ids)) != len(observed_ids):
            errors.append(f"{path} must not contain duplicate ids")
        if set(FOUNDATIONAL_PREREQUISITE_IDS) - set(observed_ids):
            errors.append(f"{path} is missing required ids")
        if set(observed_ids) - set(FOUNDATIONAL_PREREQUISITE_IDS):
            errors.append(f"{path} contains unknown ids")
    validate_foundational_grouped_lane_summary_digest_bindings(
        groups,
        lane_summary_rows,
        errors,
        path=path,
    )
    return groups


def validate_aggregate_foundational_prerequisite_output(
    row: Any,
    errors: list[str],
) -> None:
    """Validate the payload-free foundational row emitted by the aggregate gate."""

    path = "aggregate foundational prerequisites"
    if not isinstance(row, dict):
        errors.append(f"{path} must be an object")
        return
    present = row.get("present")
    valid = row.get("valid")
    expected_fields = (
        AGGREGATE_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS
        if present is True
        else AGGREGATE_MISSING_FOUNDATIONAL_PREREQUISITE_ROW_FIELDS
    )
    row_fields = set(row)
    if row_fields != expected_fields:
        errors.append(f"{path} fields must match the schema-closed output contract")
    for key in row:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append(f"{path} keys must be canonical strings")
        elif key_label not in expected_fields:
            errors.append(
                f"{path} {payload_free_diagnostic_key_label(key_label)} is not allowed"
            )
    if row.get("schema") != FOUNDATIONAL_PREREQUISITE_SCHEMA:
        errors.append(f"{path} schema must match the prerequisite contract")
    if not isinstance(present, bool):
        errors.append(f"{path} present must be boolean")
    if not isinstance(valid, bool):
        errors.append(f"{path} valid must be boolean")
    if present is not True:
        if valid is not False:
            errors.append(f"{path} missing row valid must be false")
        validate_aggregate_row_error_list(
            row.get("errors"),
            f"{path} missing row errors",
            errors,
            require_non_empty=True,
        )
        return

    required_ids = row.get("required_ids")
    if required_ids != list(FOUNDATIONAL_PREREQUISITE_IDS):
        errors.append(f"{path} required_ids must match the exact prerequisite set")
    prerequisite_count = row.get("prerequisite_count")
    if (
        not isinstance(prerequisite_count, int)
        or isinstance(prerequisite_count, bool)
        or prerequisite_count < 0
    ):
        errors.append(f"{path} prerequisite_count must be a non-negative integer")
    elif valid is True and prerequisite_count != len(FOUNDATIONAL_PREREQUISITE_IDS):
        errors.append(f"{path} prerequisite_count must match the prerequisite set")

    timestamp_fields = (
        "generated_at_unix",
        "oldest_evidence_generated_at_unix",
        "newest_evidence_generated_at_unix",
    )
    for field in timestamp_fields:
        value = row.get(field)
        if value is None and valid is not True:
            continue
        if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
            errors.append(f"{path} {field} must be a positive integer")
    oldest = row.get("oldest_evidence_generated_at_unix")
    newest = row.get("newest_evidence_generated_at_unix")
    generated = row.get("generated_at_unix")
    if (
        isinstance(oldest, int)
        and not isinstance(oldest, bool)
        and isinstance(newest, int)
        and not isinstance(newest, bool)
        and newest < oldest
    ):
        errors.append(f"{path} newest evidence timestamp must be >= oldest")
    if (
        isinstance(newest, int)
        and not isinstance(newest, bool)
        and isinstance(generated, int)
        and not isinstance(generated, bool)
        and newest > generated
    ):
        errors.append(f"{path} evidence timestamps must not exceed envelope time")

    deployment_id = row.get("deployment_id")
    environment = row.get("environment")
    if deployment_id is not None or valid is True:
        require_production_deployment_id_value(
            deployment_id,
            errors,
            f"{path} deployment_id",
        )
    if environment is not None or valid is True:
        if canonical_string(environment) is None:
            errors.append(f"{path} environment must be canonical")
        elif not is_production_ready_environment(environment):
            errors.append(f"{path} environment must be production")

    release_sequence = row.get("release_sequence")
    if release_sequence is not None or valid is True:
        if (
            not isinstance(release_sequence, int)
            or isinstance(release_sequence, bool)
            or release_sequence <= 0
            or release_sequence > MAX_FOUNDATIONAL_RELEASE_SEQUENCE
        ):
            errors.append(f"{path} release_sequence must be in 1..2^63-1")
    previous_digest = row.get("previous_envelope_sha256")
    if previous_digest is not None or valid is True:
        if canonical_lower_hex(previous_digest, 64) is None:
            errors.append(
                f"{path} previous_envelope_sha256 must be canonical lowercase SHA-256"
            )
    validate_foundational_inventory_digest(
        row.get("l1_lane_evidence_inventory_sha256"),
        None,
        errors,
        path=f"{path} l1_lane_evidence_inventory_sha256",
    )
    signer_fingerprint = row.get("signer_public_key_fingerprint_sha256")
    if signer_fingerprint is not None or valid is True:
        if canonical_lower_hex(signer_fingerprint, 64) is None:
            errors.append(
                f"{path} signer fingerprint must be canonical lowercase SHA-256"
            )
    software_signer_evidence.validate_aggregate_software_signer(row, errors)
    errors.extend(
        validate_authenticated_topology_binding_object(
            row.get("topology_qualification"),
            path=f"{path} topology_qualification",
        )
    )
    resilience_qualification = row.get("resilience_qualification")
    if resilience_qualification is not None or valid is True:
        validate_resilience_qualification_binding_object(
            resilience_qualification,
            errors,
            path=f"{path} resilience_qualification",
        )
    errors.extend(
        validate_signer_independence(
            ("topology", row.get("topology_qualification")),
            ("resilience", resilience_qualification),
            ("promotion", row),
        )
    )

    anchors = row.get("evidence_anchor_sha256")
    if not isinstance(anchors, list):
        errors.append(f"{path} evidence anchors must be a list")
    else:
        canonical_anchors = [
            anchor
            for anchor in anchors
            if canonical_lower_hex(anchor, 64) is not None
            and any(bytes.fromhex(anchor))
        ]
        if len(canonical_anchors) != len(anchors):
            errors.append(f"{path} evidence anchors must be non-zero lowercase SHA-256")
        if len(set(canonical_anchors)) != len(canonical_anchors):
            errors.append(f"{path} evidence anchors must be unique")
        if valid is True and len(anchors) != len(FOUNDATIONAL_PREREQUISITE_IDS):
            errors.append(f"{path} evidence anchors must cover every prerequisite")

    lane_summary_rows = validate_foundational_lane_summary_rows(
        row.get("lane_summary_sha256"),
        errors,
    )
    if valid is True and len(lane_summary_rows) != len(DEFAULT_REQUIRED_GATES):
        errors.append(f"{path} lane summary digests must cover every readiness lane")
    grouped_rows = validate_aggregate_foundational_prerequisite_readiness_summary_output(
        row.get("prerequisite_readiness_summary_sha256"),
        lane_summary_rows,
        errors,
    )
    if valid is True and len(grouped_rows) != len(FOUNDATIONAL_PREREQUISITE_IDS):
        errors.append(
            f"{path} prerequisite readiness summary digests must cover every prerequisite"
        )

    artifact_path = row.get("path")
    if canonical_string(artifact_path) is None:
        errors.append(f"{path} path must be canonical")
    elif not is_archive_portable_artifact_path(artifact_path):
        errors.append(f"{path} path must be archive-relative and portable")
    if canonical_lower_hex(row.get("sha256"), 64) is None:
        errors.append(f"{path} sha256 must be canonical lowercase SHA-256")

    validate_aggregate_row_error_list(
        row.get("errors"),
        f"{path} errors",
        errors,
        require_non_empty=valid is not True,
    )
    if valid is True and row.get("errors") != []:
        errors.append(f"{path} valid row errors must be empty")


def validate_aggregate_resilience_qualification_output(
    row: Any,
    errors: list[str],
) -> dict[str, Any] | None:
    """Validate the aggregate's schema-closed resilience status row."""

    path = "aggregate resilience qualification"
    if not isinstance(row, dict):
        errors.append(f"{path} must be an object")
        return None
    if set(row) != AGGREGATE_RESILIENCE_QUALIFICATION_FIELDS:
        errors.append(f"{path} fields must match the schema-closed output contract")
    if row.get("schema") != AGGREGATE_RESILIENCE_QUALIFICATION_SCHEMA:
        errors.append(f"{path} schema must match the aggregate contract")
    present = row.get("present")
    valid = row.get("valid")
    if not isinstance(present, bool):
        errors.append(f"{path} present must be boolean")
    if not isinstance(valid, bool):
        errors.append(f"{path} valid must be boolean")
    binding_value = row.get("binding")
    binding = None
    if binding_value is not None:
        binding = validate_resilience_qualification_binding_object(
            binding_value,
            errors,
            path=f"{path} binding",
        )
    elif present is True or valid is True:
        errors.append(f"{path} present row binding must be an object")
    if present is not True and valid is not False:
        errors.append(f"{path} missing row valid must be false")
    if valid is True and present is not True:
        errors.append(f"{path} valid row must be present")
    validate_aggregate_row_error_list(
        row.get("errors"),
        f"{path} errors",
        errors,
        require_non_empty=valid is not True,
    )
    if valid is True and row.get("errors") != []:
        errors.append(f"{path} valid row errors must be empty")
    return binding


def validate_aggregate_summary_output(
    summary: dict[str, Any],
    required_gates: tuple[str, ...],
    errors: list[str],
) -> None:
    """Validate the schema-closed aggregate summary envelope before writing it."""

    summary_fields = set(summary)
    missing_fields = AGGREGATE_SUMMARY_FIELDS - summary_fields
    extra_fields = summary_fields - AGGREGATE_SUMMARY_FIELDS
    if missing_fields or extra_fields:
        errors.append(
            "aggregate summary fields must match the schema-closed output contract"
        )
    for key in extra_fields:
        key_label = canonical_string(key)
        if key_label is None:
            errors.append("aggregate summary keys must be canonical strings")
        else:
            key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
            errors.append(f"aggregate summary {key_diagnostic_label} is not allowed")
    if summary.get("schema") != SUMMARY_SCHEMA:
        errors.append("aggregate summary schema must match production readiness schema")
    if summary.get("status") not in {
        "ready",
        NON_PROMOTABLE_STATUS,
        "failed",
        "blocked",
    }:
        errors.append(
            "aggregate summary status must be ready, partial, failed, or blocked"
        )
    signer_qualification = summary.get("signer_qualification")
    if signer_qualification not in {"software-key-qualified", "unqualified"}:
        errors.append(
            "aggregate summary signer_qualification must be software-key-qualified or unqualified"
        )
    required_gates_value = summary.get("required_gates")
    if not isinstance(required_gates_value, list):
        errors.append("aggregate summary required_gates must be a list")
    else:
        seen_required_gates: set[str] = set()
        emitted_required_gate_diagnostics: set[str] = set()
        for gate_name in required_gates_value:
            gate_name_label = canonical_string(gate_name)
            if gate_name_label is None:
                diagnostic = (
                    "aggregate summary required_gates must contain canonical strings"
                )
                if diagnostic not in emitted_required_gate_diagnostics:
                    errors.append(diagnostic)
                    emitted_required_gate_diagnostics.add(diagnostic)
                continue
            if gate_name_label in seen_required_gates:
                diagnostic = (
                    "aggregate summary required_gates must not contain duplicate gates"
                )
                if diagnostic not in emitted_required_gate_diagnostics:
                    errors.append(diagnostic)
                    emitted_required_gate_diagnostics.add(diagnostic)
            else:
                seen_required_gates.add(gate_name_label)
            if gate_name_label not in GATE_BY_NAME:
                diagnostic = (
                    "aggregate summary required_gates must use known gate names"
                )
                if diagnostic not in emitted_required_gate_diagnostics:
                    errors.append(diagnostic)
                    emitted_required_gate_diagnostics.add(diagnostic)
    if list(required_gates) != required_gates_value:
        errors.append("aggregate summary required_gates must match requested gates")
    thresholds_errors: list[str] = []
    thresholds = require_threshold_map(summary, "thresholds", thresholds_errors)
    if "max_summary_artifact_age_secs" not in thresholds:
        thresholds_errors.append("thresholds.max_summary_artifact_age_secs must be present")
    if thresholds and set(thresholds) != {"max_summary_artifact_age_secs"}:
        thresholds_errors.append(
            "thresholds must contain only max_summary_artifact_age_secs"
        )
    for threshold_error in thresholds_errors:
        errors.append(f"aggregate summary {threshold_error}")
    for field in ("summary_file_count", "recognized_summary_count"):
        value = summary.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            errors.append(f"aggregate summary {field} must be a non-negative integer")
    summary_file_count = summary.get("summary_file_count")
    recognized_summary_count = summary.get("recognized_summary_count")
    if (
        isinstance(summary_file_count, int)
        and not isinstance(summary_file_count, bool)
        and isinstance(recognized_summary_count, int)
        and not isinstance(recognized_summary_count, bool)
        and recognized_summary_count > summary_file_count
    ):
        errors.append(
            "aggregate summary recognized_summary_count must not exceed summary_file_count"
        )
    if (
        isinstance(recognized_summary_count, int)
        and not isinstance(recognized_summary_count, bool)
        and recognized_summary_count > len(required_gates)
    ):
        errors.append(
            "aggregate summary recognized_summary_count must not exceed required gate count"
        )
    deployment = summary.get("deployment")
    allowed_deployment_fields = {"deployment_id", "environment"}
    aggregate_deployment_id = None
    aggregate_environment = None
    if not isinstance(deployment, dict):
        errors.append("aggregate summary deployment must be an object")
    else:
        deployment_fields = set(deployment)
        if deployment_fields and deployment_fields != allowed_deployment_fields:
            errors.append(
                "aggregate summary deployment fields must be deployment_id and environment"
            )
        for key in deployment:
            key_label = canonical_string(key)
            if key_label is None:
                errors.append("aggregate summary deployment keys must be canonical strings")
            elif key_label not in allowed_deployment_fields:
                key_diagnostic_label = payload_free_diagnostic_key_label(key_label)
                errors.append(
                    f"aggregate summary deployment {key_diagnostic_label} is not allowed"
                )
        if deployment_fields == allowed_deployment_fields:
            aggregate_deployment_id = require_production_deployment_id_value(
                deployment.get("deployment_id"),
                errors,
                "aggregate summary deployment_id",
            )
            aggregate_environment = canonical_string(deployment.get("environment"))
            if aggregate_environment is None:
                errors.append("aggregate summary environment must be a canonical string")
            elif not is_production_ready_environment(aggregate_environment):
                errors.append("aggregate summary environment must be production")
    topology_qualification = summary.get("topology_qualification")
    errors.extend(
        validate_authenticated_topology_binding_object(
            topology_qualification,
            path="aggregate summary topology_qualification",
        )
    )
    resilience_row = summary.get("resilience_qualification")
    resilience_qualification = validate_aggregate_resilience_qualification_output(
        resilience_row,
        errors,
    )
    inventory_row = summary.get("l1_lane_evidence_inventory")
    inventory_binding = validate_aggregate_inventory_row(inventory_row, errors)
    required = summary.get("required")
    if not isinstance(required, dict):
        errors.append("aggregate summary required must be an object")
    else:
        if set(required) != set(required_gates):
            errors.append("aggregate summary required rows must match requested gates")
        if (
            isinstance(recognized_summary_count, int)
            and not isinstance(recognized_summary_count, bool)
            and recognized_summary_count
            != sum(
                1
                for row in required.values()
                if isinstance(row, dict) and row.get("present") is True
            )
        ):
            errors.append(
                "aggregate summary recognized_summary_count must match present required rows"
            )
        for gate_name, row in required.items():
            gate_name_label = canonical_string(gate_name)
            if gate_name_label is None:
                errors.append("aggregate summary required row keys must be canonical strings")
            elif gate_name_label not in GATE_BY_NAME:
                errors.append("aggregate summary required rows must use known gate names")
            if not isinstance(row, dict):
                errors.append("aggregate summary required rows must be objects")
            elif gate_name_label in GATE_BY_NAME:
                validate_aggregate_required_row_output(
                    GATE_BY_NAME[gate_name_label],
                    row,
                    errors,
                )
                if row.get("present") is True:
                    errors.extend(
                        validate_topology_binding_object(
                            row.get("topology_qualification"),
                            expected=(
                                topology_qualification
                                if isinstance(topology_qualification, Mapping)
                                else None
                            ),
                            path=(
                                f"{gate_name_label} aggregate required row "
                                "topology_qualification"
                            ),
                        )
                    )
                    row_deployment_id = canonical_string(row.get("deployment_id"))
                    if (
                        aggregate_deployment_id is not None
                        and row_deployment_id is not None
                        and row_deployment_id != aggregate_deployment_id
                    ):
                        errors.append(
                            f"{gate_name_label} aggregate required row deployment_id must match aggregate deployment_id"
                        )
                    row_environment = canonical_string(row.get("environment"))
                    if (
                        aggregate_environment is not None
                        and row_environment is not None
                        and row_environment != aggregate_environment
                    ):
                        errors.append(
                            f"{gate_name_label} aggregate required row environment must match aggregate environment"
                        )
    foundational_prerequisites = summary.get("foundational_prerequisites")
    validate_aggregate_foundational_prerequisite_output(
        foundational_prerequisites,
        errors,
    )
    foundation_valid = (
        isinstance(foundational_prerequisites, dict)
        and foundational_prerequisites.get("valid") is True
    )
    inventory_valid = (
        isinstance(inventory_row, dict) and inventory_row.get("valid") is True
    )
    expected_qualification = (
        "software-key-qualified" if foundation_valid and inventory_valid else "unqualified"
    )
    if signer_qualification != expected_qualification:
        errors.append(
            "aggregate summary signer_qualification must match the validated software signer"
        )
    validate_aggregate_foundational_lane_digest_bindings(
        foundational_prerequisites, required, errors
    )
    if (
        isinstance(foundational_prerequisites, dict)
        and foundational_prerequisites.get("present") is True
    ):
        errors.extend(
            validate_authenticated_topology_binding_object(
                foundational_prerequisites.get("topology_qualification"),
                expected=(
                    topology_qualification
                    if isinstance(topology_qualification, Mapping)
                    else None
                ),
                path="aggregate foundational prerequisite topology_qualification",
            )
        )
        foundational_deployment_id = canonical_string(
            foundational_prerequisites.get("deployment_id")
        )
        foundational_environment = canonical_string(
            foundational_prerequisites.get("environment")
        )
        if (
            aggregate_deployment_id is not None
            and foundational_deployment_id is not None
            and foundational_deployment_id != aggregate_deployment_id
        ):
            errors.append(
                "aggregate foundational prerequisite deployment_id must match aggregate deployment_id"
            )
        if (
            aggregate_environment is not None
            and foundational_environment is not None
            and foundational_environment != aggregate_environment
        ):
            errors.append(
                "aggregate foundational prerequisite environment must match aggregate environment"
            )
        foundational_resilience = foundational_prerequisites.get(
            "resilience_qualification"
        )
        if (
            isinstance(resilience_row, Mapping)
            and resilience_row.get("valid") is True
            and resilience_qualification is not None
            and foundational_resilience != resilience_qualification
        ):
            errors.append(
                "aggregate foundational prerequisite resilience_qualification "
                "must match aggregate resilience_qualification"
            )
        validate_foundational_inventory_digest(
            foundational_prerequisites.get("l1_lane_evidence_inventory_sha256"),
            inventory_binding,
            errors,
            path="aggregate foundational prerequisite L1 inventory digest",
        )
        errors.extend(
            validate_signer_independence(
                ("topology", topology_qualification),
                ("resilience", resilience_qualification),
                ("L1 lane inventory", inventory_binding),
                ("promotion", foundational_prerequisites),
            )
        )
    if summary.get("status") == "ready":
        if tuple(required_gates) != DEFAULT_REQUIRED_GATES:
            errors.append(
                "aggregate summary ready required_gates must match the exact "
                "canonical 17-gate inventory"
            )
        if not isinstance(deployment, dict) or set(deployment) != allowed_deployment_fields:
            errors.append(
                "aggregate summary ready deployment must include deployment_id and environment"
            )
        if (
            isinstance(summary_file_count, int)
            and not isinstance(summary_file_count, bool)
            and summary_file_count != len(required_gates)
        ):
            errors.append(
                "aggregate summary ready summary_file_count must match required gate count"
            )
        if (
            isinstance(recognized_summary_count, int)
            and not isinstance(recognized_summary_count, bool)
            and recognized_summary_count != len(required_gates)
        ):
            errors.append(
                "aggregate summary ready recognized_summary_count must match required gate count"
            )
        if not isinstance(required, dict) or any(
            not isinstance(row, dict)
            or row.get("present") is not True
            or row.get("valid") is not True
            for row in required.values()
        ):
            errors.append("aggregate summary ready rows must all be present and valid")
        if (
            not isinstance(foundational_prerequisites, dict)
            or foundational_prerequisites.get("present") is not True
            or foundational_prerequisites.get("valid") is not True
        ):
            errors.append(
                "aggregate summary ready foundational prerequisites must be present and valid"
            )
        if (
            not isinstance(resilience_row, Mapping)
            or resilience_row.get("present") is not True
            or resilience_row.get("valid") is not True
            or resilience_qualification is None
        ):
            errors.append(
                "aggregate summary ready resilience qualification must be present and valid"
            )
        if (
            not isinstance(inventory_row, dict)
            or inventory_row.get("present") is not True
            or inventory_row.get("valid") is not True
            or inventory_binding is None
        ):
            errors.append(
                "aggregate summary ready L1 lane evidence inventory must be present and valid"
            )
    error_values = summary.get("errors")
    if not isinstance(error_values, list):
        errors.append("aggregate summary errors must be a list")
    else:
        seen_errors: set[str] = set()
        for error in error_values:
            error_label = canonical_string(error)
            if error_label is None:
                errors.append("aggregate summary errors must contain canonical strings")
                break
            if error_label in seen_errors:
                errors.append(
                    "aggregate summary errors must not contain duplicate diagnostics"
                )
                break
            seen_errors.add(error_label)
        if summary.get("status") != aggregate_summary_status(
            error_values,
            required_gates,
        ):
            errors.append("aggregate summary status must match aggregate diagnostics")


def validate_gate_summary(
    gate: GateSummaryKind,
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[dict[str, Any], list[str]]:
    """Validate one existing lane-gate summary."""

    errors: list[str] = []
    sensitivity_payload = (
        reference_sdk_release_sensitivity_view(payload)
        if gate.name == "reference_sdk_release"
        else payload
    )
    visit_sensitive_fields(
        sensitivity_payload,
        "",
        errors,
        sensitive_keys=SENSITIVE_KEYS,
        evidence_label="SoraFS production readiness summary",
    )
    require_payload_free_summary_fields(payload, errors)
    errors.extend(
        validate_topology_binding_object(
            payload.get("topology_qualification"),
            expected=options.topology_qualification,
        )
    )
    validate_payload_free_summary_metadata(
        gate,
        payload,
        errors,
        enforce_production_deployment_context=is_production_ready_environment(
            options.environment
        ),
    )
    require_string_field(payload, "schema", errors)
    if payload.get("schema") != gate.schema:
        errors.append(f"schema must be `{gate.schema}`")
    if payload.get("status") != "ready":
        errors.append("status must be `ready`")
    require_empty_error_list(payload.get("errors"), "errors", errors)
    require_absent_or_empty_error_list(payload, "load_errors", errors)
    thresholds = require_threshold_map(payload, "thresholds", errors)
    evidence_file_count = require_positive_int_field(
        payload,
        "evidence_file_count",
        errors,
    )
    recognized_artifact_count = require_positive_int_field(
        payload,
        "recognized_artifact_count",
        errors,
    )

    required_kinds_raw = payload.get("required_kinds")
    if not isinstance(required_kinds_raw, list) or not required_kinds_raw:
        errors.append("required_kinds must be a non-empty array")
        required_kinds: list[str] = []
    else:
        required_kinds = []
        for index, kind_name in enumerate(required_kinds_raw):
            kind_label = canonical_string(kind_name)
            if kind_label is None:
                errors.append(f"required_kinds[{index}] must be canonical")
                continue
            if kind_label in required_kinds:
                errors.append("required_kinds contains duplicate kind")
                continue
            required_kinds.append(kind_label)
    expected_required_kinds = list(gate.required_kinds)
    if required_kinds != expected_required_kinds:
        missing = sorted(set(expected_required_kinds) - set(required_kinds))
        extra = sorted(set(required_kinds) - set(expected_required_kinds))
        errors.append(
            f"required_kinds must match the full `{gate.name}` gate contract"
        )
        if missing:
            errors.append("required_kinds missing full-contract kinds")
        if extra:
            errors.append("required_kinds contains unknown full-contract kinds")

    required = payload.get("required")
    if not isinstance(required, dict):
        errors.append("required must be an object")
        required = {}
    else:
        noncanonical_required_rows = [
            row_label for row_label in required if canonical_string(row_label) is None
        ]
        if noncanonical_required_rows:
            errors.append("required row labels must be canonical strings")
        extra_required_rows = set(required) - set(expected_required_kinds)
        if extra_required_rows:
            errors.append(
                f"required contains rows outside the full `{gate.name}` gate contract"
            )

    artifact_total = 0
    generated_times: list[int] = []
    deployment_contexts: set[tuple[str, str]] = set()
    required_kind_schemas = GATE_REQUIRED_KIND_SCHEMAS.get(gate.name, {})
    expected_required_kind_set = set(expected_required_kinds)
    for kind_name in required_kinds:
        if kind_name not in expected_required_kind_set:
            continue
        artifact_count, row_times, row_contexts = validate_required_row(
            gate.name,
            kind_name,
            required_kind_schemas.get(kind_name),
            required.get(kind_name),
            options,
            errors,
        )
        artifact_total += artifact_count
        generated_times.extend(row_times)
        deployment_contexts.update(row_contexts)

    if (
        recognized_artifact_count is not None
        and recognized_artifact_count != artifact_total
    ):
        errors.append(
            "recognized_artifact_count must match required row artifact total"
        )
    if (
        evidence_file_count is not None
        and recognized_artifact_count is not None
        and evidence_file_count > recognized_artifact_count
    ):
        errors.append(
            "evidence_file_count must not exceed recognized_artifact_count"
        )
    (
        recognized_times,
        recognized_contexts,
        recognized_artifact_object_count,
        recognized_artifact_path_count,
    ) = validate_recognized_artifacts(
        gate,
        payload,
        required,
        recognized_artifact_count,
        options,
        errors,
    )
    generated_times.extend(recognized_times)
    deployment_contexts.update(recognized_contexts)
    validate_payload_free_summary_metadata_fingerprint_tethers(
        gate,
        payload,
        errors,
    )
    validate_payload_free_object_list_metadata_counts(
        gate,
        payload,
        required,
        errors,
    )
    deployment_contexts.update(
        payload_free_summary_metadata_deployment_contexts(gate, payload)
    )
    if (
        evidence_file_count is not None
        and recognized_artifact_path_count is not None
        and evidence_file_count != recognized_artifact_path_count
    ):
        errors.append("evidence_file_count must match recognized artifact path count")

    if len(deployment_contexts) > 1:
        errors.append(
            f"{gate.name} deployment context must match across artifacts and metadata"
        )
    deployment_id = None
    environment = None
    if len(deployment_contexts) == 1:
        deployment_id, environment = next(iter(deployment_contexts))
        if options.deployment_id is not None and deployment_id != options.deployment_id:
            errors.append(f"{gate.name} deployment_id must match --deployment-id")
        if options.environment is not None and environment != options.environment:
            errors.append(f"{gate.name} environment must match --environment")
    summary_recognized_artifact_count = (
        recognized_artifact_object_count
        if recognized_artifact_object_count is not None
        else recognized_artifact_count
    )

    summary = {
        "schema": gate.schema,
        "present": True,
        "valid": not errors,
        "required_kind_count": len(required_kinds),
        "expected_required_kind_count": len(gate.required_kinds),
        "evidence_file_count": evidence_file_count,
        "recognized_artifact_count": summary_recognized_artifact_count,
        "artifact_count": artifact_total,
        "thresholds": thresholds,
        "oldest_generated_at_unix": min(generated_times) if generated_times else None,
        "newest_generated_at_unix": max(generated_times) if generated_times else None,
        "deployment_id": deployment_id,
        "environment": environment,
        "expected_required_kinds": expected_required_kinds,
        "topology_qualification": payload.get("topology_qualification"),
        "errors": errors,
    }
    return summary, errors


def validate_joint_gateway_moderation_catalog_binding(
    valid_lane_payloads: dict[str, dict[str, Any]],
    errors: list[str],
) -> None:
    """Bind individually valid moderation viewer evidence to the gateway catalog."""

    gateway_payload = valid_lane_payloads.get("gateway_compliance")
    moderation_payload = valid_lane_payloads.get("moderation_panel")
    if gateway_payload is None or moderation_payload is None:
        return
    gateway_catalog_digests = payload_free_hex_list_metadata_values(
        "valid_catalog_digests",
        gateway_payload.get("valid_catalog_digests"),
    )
    viewer_digest_values = payload_free_object_list_hex_metadata_values(
        "valid_evidence_viewer_digest_sets",
        moderation_payload.get("valid_evidence_viewer_digest_sets"),
    )
    if gateway_catalog_digests is None or viewer_digest_values is None:
        return
    if (
        viewer_digest_values.get("catalog_digest_hex", set())
        != gateway_catalog_digests
    ):
        errors.append(GATEWAY_MODERATION_CATALOG_MISMATCH_ERROR)


def build_summary(
    evidence_dirs: list[Path],
    evidence_files: list[Path],
    required_gates: tuple[str, ...],
    options: ValidationOptions,
    summary_out: Path | None,
) -> tuple[dict[str, Any], list[str]]:
    """Build the aggregate production-readiness summary."""

    errors: list[str] = []
    inventory_errors = list(options.l1_lane_evidence_inventory_errors)
    inventory_row = aggregate_inventory_row(
        options.l1_lane_evidence_inventory, inventory_errors
    )
    errors.extend(inventory_errors)
    resilience_errors = list(options.resilience_qualification_errors)
    resilience_binding: dict[str, Any] | None = None
    if options.resilience_qualification is None:
        if not resilience_errors:
            resilience_errors.append(
                "missing required trusted resilience qualification summary"
            )
    else:
        resilience_binding = validate_resilience_qualification_binding_object(
            dict(options.resilience_qualification),
            resilience_errors,
            path="reviewed resilience qualification",
        )
    resilience_qualification = {
        "schema": AGGREGATE_RESILIENCE_QUALIFICATION_SCHEMA,
        "present": options.resilience_qualification is not None,
        "valid": resilience_binding is not None and not resilience_errors,
        "binding": resilience_binding,
        "errors": resilience_errors,
    }
    errors.extend(resilience_errors)
    files = discover_evidence_files(
        evidence_dirs,
        evidence_files,
        errors,
        reserved_output_paths=() if summary_out is None else (summary_out,),
    )
    explicit = evidence_path_identities(evidence_files, errors)
    required: dict[str, dict[str, Any]] = {
        name: {
            "schema": GATE_BY_NAME[name].schema,
            "present": False,
            "valid": False,
            "errors": [f"missing required {name} production readiness summary"],
        }
        for name in required_gates
    }
    foundational_prerequisites: dict[str, Any] = {
        "schema": FOUNDATIONAL_PREREQUISITE_SCHEMA,
        "present": False,
        "valid": False,
        "errors": ["missing required foundational prerequisite summary"],
    }
    summary_contexts: set[tuple[str, str]] = set()
    recognized_summaries = 0
    foundational_file_count = 0
    duplicate_foundational_summary = False
    duplicate_summary_gates: set[str] = set()
    duplicate_summary_count = 0
    unknown_schema_count = 0
    explicit_unrequired_count = 0
    observed_summary_sha256: dict[str, str] = {}
    valid_lane_payloads: dict[str, dict[str, Any]] = {}

    for path in files:
        loaded = load_evidence_json_with_sha256_or_record_error(
            path,
            MAX_SUMMARY_BYTES,
            errors,
        )
        if loaded is None:
            continue
        payload, digest = loaded
        schema = payload.get("schema")
        if schema == FOUNDATIONAL_PREREQUISITE_SCHEMA:
            foundational_file_count += 1
            if foundational_prerequisites.get("present") is True:
                if not duplicate_foundational_summary:
                    duplicate_foundational_summary = True
                    duplicate_error = (
                        "duplicate foundational prerequisite summary"
                    )
                    foundational_prerequisites["valid"] = False
                    row_errors = foundational_prerequisites.setdefault("errors", [])
                    if duplicate_error not in row_errors:
                        row_errors.append(duplicate_error)
                    errors.append(duplicate_error)
                continue
            (
                foundation_summary,
                foundation_errors,
                foundation_context,
            ) = validate_foundational_prerequisite_summary(payload, options)
            foundation_summary["path"] = aggregate_summary_path_label(
                path,
                evidence_dirs,
            )
            foundation_summary["sha256"] = digest
            if foundation_summary.get("valid") is True:
                foundation_output_errors: list[str] = []
                validate_aggregate_foundational_prerequisite_output(
                    foundation_summary,
                    foundation_output_errors,
                )
                if foundation_output_errors:
                    foundation_summary["valid"] = False
                    foundation_summary["errors"].extend(foundation_output_errors)
                    foundation_errors.extend(foundation_output_errors)
            foundational_prerequisites = foundation_summary
            for error in foundation_errors:
                errors.append(f"foundational prerequisites: {error}")
            if foundation_context is not None:
                summary_contexts.add(foundation_context)
            continue
        gate = SCHEMA_TO_GATE.get(schema)
        if gate is None:
            unknown_schema_count += 1
            errors.append("unknown SoraFS readiness summary schema")
            continue
        if gate.name not in required:
            if is_explicit_evidence_path(path, explicit, errors):
                explicit_unrequired_count += 1
                errors.append(
                    "explicit production readiness summary belongs to unrequired gate"
                )
            continue
        if required[gate.name]["present"] is True:
            duplicate_summary_gates.add(gate.name)
            duplicate_summary_count += 1
            valid_lane_payloads.pop(gate.name, None)
            required[gate.name]["valid"] = False
            duplicate_error = f"duplicate {gate.name} production readiness summary"
            row_errors = required[gate.name].setdefault("errors", [])
            if duplicate_error not in row_errors:
                row_errors.append(duplicate_error)
            errors.append(duplicate_error)
            continue
        recognized_summaries += 1
        observed_summary_sha256[gate.name] = digest
        gate_summary, validation_errors = validate_gate_summary(gate, payload, options)
        gate_summary["path"] = aggregate_summary_path_label(path, evidence_dirs)
        gate_summary["sha256"] = digest
        if gate_summary.get("valid") is True:
            row_output_errors: list[str] = []
            validate_aggregate_gate_row_output(gate, gate_summary, row_output_errors)
            if row_output_errors:
                gate_summary["valid"] = False
                gate_summary["errors"].extend(row_output_errors)
                validation_errors.extend(row_output_errors)
            else:
                valid_lane_payloads[gate.name] = payload
        required[gate.name] = gate_summary
        for error in validation_errors:
            errors.append(f"{gate.name}: {error}")
        deployment_id = gate_summary.get("deployment_id")
        environment = gate_summary.get("environment")
        if isinstance(deployment_id, str) and isinstance(environment, str):
            summary_contexts.add((deployment_id, environment))

    validate_foundational_lane_summary_digest_bindings(
        foundational_prerequisites,
        observed_summary_sha256,
        required_gates,
        errors,
    )
    if (
        options.l1_lane_evidence_inventory is not None
        and required_gates == DEFAULT_REQUIRED_GATES
    ):
        validate_inventory_lane_digest_bindings(
            options.l1_lane_evidence_inventory.summary_sha256,
            observed_summary_sha256,
            errors,
        )
    validate_joint_gateway_moderation_catalog_binding(
        valid_lane_payloads,
        errors,
    )

    for name, row in required.items():
        if row.get("present") is False:
            errors.extend(row.get("errors", []))
        elif row.get("valid") is not True:
            errors.append(f"{name} production readiness summary is invalid")
    if foundational_prerequisites.get("present") is False:
        errors.extend(foundational_prerequisites.get("errors", []))
    elif foundational_prerequisites.get("valid") is not True:
        errors.append("foundational prerequisite summary is invalid")

    if options.deployment_id is None or options.environment is None:
        errors.append(
            "aggregate production readiness requires --deployment-id and --environment"
        )

    validate_duplicate_summary_diagnostics(
        required,
        duplicate_summary_gates,
        duplicate_summary_count,
        errors,
    )
    validate_disallowed_summary_diagnostics(
        errors,
        unknown_schema_count=unknown_schema_count,
        explicit_unrequired_count=explicit_unrequired_count,
    )

    if len(summary_contexts) > 1:
        errors.append("production readiness deployment context must match across gates")
    deployment: dict[str, str] = {}
    if len(summary_contexts) == 1:
        deployment_id, environment = next(iter(summary_contexts))
        deployment = {"deployment_id": deployment_id, "environment": environment}
        if options.deployment_id is not None and deployment_id != options.deployment_id:
            errors.append("aggregate deployment_id must match --deployment-id")
        if options.environment is not None and environment != options.environment:
            errors.append("aggregate environment must match --environment")
        require_production_deployment_id_value(
            deployment_id,
            errors,
            "aggregate deployment_id",
        )
        if not is_production_ready_environment(environment):
            errors.append("aggregate environment must be production")

    summary = {
        "schema": SUMMARY_SCHEMA,
        "status": aggregate_summary_status(errors, required_gates),
        "signer_qualification": (
            "software-key-qualified"
            if foundational_prerequisites.get("valid") is True
            and inventory_row.get("valid") is True
            else "unqualified"
        ),
        "required_gates": list(required_gates),
        "thresholds": {
            "max_summary_artifact_age_secs": options.max_summary_artifact_age_secs,
        },
        "summary_file_count": len(files) - foundational_file_count,
        "recognized_summary_count": recognized_summaries,
        "deployment": deployment,
        "topology_qualification": options.topology_qualification,
        "resilience_qualification": resilience_qualification,
        "l1_lane_evidence_inventory": inventory_row,
        "foundational_prerequisites": foundational_prerequisites,
        "required": required,
        "errors": errors,
    }
    validate_aggregate_summary_output(summary, required_gates, errors)
    summary["status"] = aggregate_summary_status(errors, required_gates)
    return summary, errors


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    """Parse aggregate readiness checker arguments."""

    parser = EvidenceArgumentParser(
        description="Validate aggregate SoraFS production-readiness summaries.",
    )
    add_signed_topology_qualification_arguments(parser)
    add_signed_lane_inventory_arguments(parser, summary_flag="--l1-lane-summary")
    parser.add_argument(
        "--resilience-qualification-summary",
        type=Path,
        help=(
            "Required evidence-qualified holistic resilience/DR summary. This is "
            "a signed deployment attachment, not an eighteenth readiness lane."
        ),
    )
    parser.add_argument(
        "--resilience-qualification-signer-public-key-hex",
        help=(
            "Required operator-trusted 32-byte Ed25519 public key used to "
            "authenticate the resilience receipt. The key remains runtime-only."
        ),
    )
    parser.add_argument(
        "--evidence-dir",
        action="append",
        type=Path,
        default=[],
        help="Directory containing per-lane readiness summary JSON files.",
    )
    parser.add_argument(
        "--evidence",
        action="append",
        type=Path,
        default=[],
        help="Explicit per-lane readiness summary JSON file.",
    )
    parser.add_argument(
        "--require-gate",
        action="append",
        default=[],
        help="Required aggregate gate name, or comma-separated names. Defaults to every SoraFS gate.",
    )
    parser.add_argument("--summary-out", type=Path, help="Optional summary JSON output path.")
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        required=True,
        help="Required reviewed validator clock used for artifact age checks.",
    )
    parser.add_argument(
        "--max-summary-artifact-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
    )
    parser.add_argument(
        "--deployment-id",
        help=(
            "Required final deployment id shared by every lane summary artifact "
            "before production readiness can pass."
        ),
    )
    parser.add_argument(
        "--environment",
        help=(
            "Required final prod/production environment shared by every lane "
            "summary artifact before production readiness can pass."
        ),
    )
    parser.add_argument(
        "--foundational-prerequisite-signer-public-key-hex",
        dest="foundational_signer_public_key_hex",
        help=(
            "Required operator-trusted 32-byte Ed25519 public key for the signed "
            "foundational prerequisite envelope. The key is runtime-only input "
            "and is not copied into the aggregate summary."
        ),
    )
    parser.add_argument(
        "--foundational-prerequisite-release-sequence",
        dest="foundational_release_sequence",
        type=positive_int_arg,
        help=(
            "Required operator-reviewed monotonic release sequence expected in "
            "the foundational prerequisite envelope."
        ),
    )
    parser.add_argument(
        "--foundational-prerequisite-previous-envelope-sha256",
        dest="foundational_previous_envelope_sha256",
        help=(
            "Required operator-reviewed lowercase SHA-256 of the preceding "
            "foundational envelope (all zeroes only for sequence 1)."
        ),
    )
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_checker_exception(error)
        raise SystemExit(2) from error
    return parser.parse_args(expanded_args)


def main(argv: list[str] | None = None) -> int:
    """Run the aggregate SoraFS production-readiness checker."""

    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    try:
        required_gates = parse_required_gates(
            args.require_gate,
            allowed_kinds=GATE_BY_NAME,
            default_required=DEFAULT_REQUIRED_GATES,
        )
    except ValueError as error:
        emit_checker_exception(error)
        return 2

    if args.deployment_id is not None:
        deployment_id_errors: list[str] = []
        require_production_deployment_id_value(
            args.deployment_id,
            deployment_id_errors,
            "--deployment-id",
        )
        if deployment_id_errors:
            emit_checker_error_lines(deployment_id_errors)
            return 2
    if args.environment is not None and canonical_string(args.environment) is None:
        emit_checker_error_lines(["--environment must be a non-empty canonical string"])
        return 2
    if args.environment is not None and not is_production_ready_environment(
        args.environment
    ):
        emit_checker_error_lines(["--environment must be production for this gate"])
        return 2
    foundational_signer_public_key: bytes | None = None
    if args.foundational_signer_public_key_hex is not None:
        foundational_key_errors: list[str] = []
        foundational_signer_public_key = parse_foundational_signer_public_key(
            args.foundational_signer_public_key_hex,
            foundational_key_errors,
            path="--foundational-prerequisite-signer-public-key-hex",
        )
        if foundational_key_errors:
            emit_checker_error_lines(foundational_key_errors)
            return 2
    resilience_signer_public_key: bytes | None = None
    if args.resilience_qualification_signer_public_key_hex is not None:
        resilience_key_errors: list[str] = []
        resilience_signer_public_key = parse_foundational_signer_public_key(
            args.resilience_qualification_signer_public_key_hex,
            resilience_key_errors,
            path="--resilience-qualification-signer-public-key-hex",
        )
        if resilience_key_errors:
            emit_checker_error_lines(resilience_key_errors)
            return 2
    foundational_previous_envelope_sha256 = None
    if (
        args.foundational_release_sequence is not None
        and args.foundational_release_sequence > MAX_FOUNDATIONAL_RELEASE_SEQUENCE
    ):
        emit_checker_error_lines(
            [
                "--foundational-prerequisite-release-sequence must be in 1..2^63-1"
            ]
        )
        return 2
    if args.foundational_previous_envelope_sha256 is not None:
        foundational_previous_envelope_sha256 = canonical_lower_hex(
            args.foundational_previous_envelope_sha256,
            64,
        )
        if foundational_previous_envelope_sha256 is None:
            emit_checker_error_lines(
                [
                    "--foundational-prerequisite-previous-envelope-sha256 must be canonical lowercase SHA-256"
                ]
            )
            return 2

    preflight_errors = validate_checker_preflight(args)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2
    topology_qualification, topology_errors = (
        load_signed_topology_qualification_from_args(
            args,
            expected_deployment_id=args.deployment_id,
            expected_environment=args.environment,
        )
    )
    if topology_errors or topology_qualification is None:
        emit_checker_error_lines(topology_errors)
        return 2
    if args.resilience_qualification_summary is None:
        emit_checker_error_lines(["--resilience-qualification-summary is required"])
        return 2
    if resilience_signer_public_key is None:
        emit_checker_error_lines(
            ["--resilience-qualification-signer-public-key-hex is required"]
        )
        return 2
    resilience_qualification, resilience_errors = (
        load_resilience_qualification_binding(
            args.resilience_qualification_summary,
            expected_deployment_id=args.deployment_id,
            expected_environment=args.environment,
            expected_topology_qualification=topology_qualification,
            now_unix=args.now_unix,
            max_age_secs=args.max_summary_artifact_age_secs,
            trusted_public_key=resilience_signer_public_key,
        )
    )
    try:
        inventory_specs = parse_inventory_summary_specs(args.l1_lane_summary)
    except InventoryError as error:
        l1_inventory = None
        inventory_errors = [f"signed L1 lane evidence inventory: {error}"]
    else:
        l1_inventory, inventory_errors = verify_inventory_from_args(
            args,
            inventory_specs,
            topology_qualification,
            deployment_id=args.deployment_id,
            environment=args.environment,
            now_unix=args.now_unix,
        )
    options = ValidationOptions(
        now_unix=args.now_unix,
        max_summary_artifact_age_secs=args.max_summary_artifact_age_secs,
        deployment_id=args.deployment_id,
        environment=args.environment,
        foundational_signer_public_key=foundational_signer_public_key,
        foundational_release_sequence=args.foundational_release_sequence,
        foundational_previous_envelope_sha256=(
            foundational_previous_envelope_sha256
        ),
        topology_qualification=topology_qualification,
        resilience_qualification=resilience_qualification,
        resilience_qualification_errors=tuple(resilience_errors),
        l1_lane_evidence_inventory=l1_inventory,
        l1_lane_evidence_inventory_errors=tuple(inventory_errors),
    )
    summary, errors = build_summary(
        args.evidence_dir,
        args.evidence,
        required_gates,
        options,
        args.summary_out,
    )
    errors.extend(topology_errors)
    summary["status"] = aggregate_summary_status(errors, required_gates)
    _, render_errors = render_and_write_checker_summary(args.summary_out, summary)
    if render_errors:
        emit_checker_error_lines(render_errors)
        return 2
    if errors:
        emit_checker_error_block("SoraFS production readiness is blocked:", errors)
        return 1
    if summary["status"] == NON_PROMOTABLE_STATUS:
        emit_checker_notice(
            "SoraFS production readiness inputs validated as a non-promotable "
            f"partial inventory of {len(required_gates)} gate(s)."
        )
        return 0
    emit_checker_notice(
        "SoraFS production readiness validated for "
        f"{len(required_gates)} required gate(s) and "
        f"{len(FOUNDATIONAL_PREREQUISITE_IDS)} foundational prerequisite(s)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
