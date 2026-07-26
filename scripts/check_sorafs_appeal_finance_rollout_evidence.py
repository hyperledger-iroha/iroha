#!/usr/bin/env python3
"""Validate SoraFS appeal finance rollout evidence artifacts."""

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
from sorafs_evidence_fingerprint import artifact_fingerprint  # noqa: E402
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
    require_iroha_config_binding,
    validate_standard_evidence_payload,
    require_maximum_int,
    require_minimum_int,
    require_non_negative_int,
    require_object,
    require_object_array,
    required_evidence_kind_names,
    require_passed_status,
    require_policy_digest,
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
from sorafs_path_identity import diagnostic_text_is_canonical  # noqa: E402


SUMMARY_SCHEMA = "sorafs.appeal_finance.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_CANARY_AGE_SECS = 24 * 60 * 60
DEFAULT_MAX_DASHBOARD_AGE_SECS = 7 * 24 * 60 * 60
DEFAULT_MAX_ROUTE_LATENCY_MS = 1_500
DEFAULT_MAX_SETTLEMENT_LAG_SECS = 15 * 60
DEFAULT_MIN_PEERS = 4
HEX64_LEN = 64
CONFIG_VERSION_PATTERN = re.compile(
    r"^appeal-finance-config-[a-z0-9]+(?:-[a-z0-9]+)*-v([1-9][0-9]*)\Z"
)
CONFIG_VERSION_ERROR = (
    "config_version must match canonical lowercase `appeal-finance-config-name-vN`"
)
FORBIDDEN_CONFIG_VERSION_MARKERS = frozenset(
    (
        "debug",
        "dev",
        "draft",
        "example",
        "latest",
        "placeholder",
        "sample",
        "test",
        "todo",
    )
)
PEER_LABEL_PATTERN = re.compile(
    r"^appeal-finance-peer-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
VALIDATOR_LABEL_PATTERN = re.compile(
    r"^appeal-finance-validator-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
RECONCILIATION_CASE_LABEL_PATTERN = re.compile(
    r"^appeal-finance-case-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
DEPOSIT_PROBE_LABEL_PATTERN = re.compile(
    r"^appeal-finance-deposit-probe-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
SUBMITTER_SIGNER_LABEL_PATTERN = re.compile(
    r"^appeal-finance-submitter-signer-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
SUBMITTER_STEP_LABEL_PATTERN = re.compile(
    r"^appeal-finance-submitter-step-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
WORKER_BALLOT_LABEL_PATTERN = re.compile(
    r"^appeal-finance-worker-ballot-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
GOVERNANCE_REPORT_LABEL_PATTERN = re.compile(
    r"^appeal-finance-report-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
GOVERNANCE_WEEKLY_ROLLUP_LABEL_PATTERN = re.compile(
    r"^appeal-finance-weekly-rollup-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
GOVERNANCE_SETTLEMENT_RECEIPT_LABEL_PATTERN = re.compile(
    r"^appeal-finance-settlement-receipt-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
PEER_LABEL_ERROR = (
    "peers[].name must match canonical lowercase `appeal-finance-peer-*`"
)
VALIDATOR_LABEL_ERROR = (
    "validators[].name must match canonical lowercase "
    "`appeal-finance-validator-name`"
)
RECONCILIATION_CASE_LABEL_ERROR = (
    "cases[].name must match canonical lowercase `appeal-finance-case-*`"
)
DEPOSIT_PROBE_LABEL_ERROR = (
    "deposit_probes[].name must match canonical lowercase "
    "`appeal-finance-deposit-probe-name`"
)
SUBMITTER_SIGNER_LABEL_ERROR = (
    "signers[].name must match canonical lowercase "
    "`appeal-finance-submitter-signer-name`"
)
SUBMITTER_STEP_LABEL_ERROR = (
    "steps[].name must match canonical lowercase "
    "`appeal-finance-submitter-step-name`"
)
WORKER_BALLOT_LABEL_ERROR = (
    "ballots[].name must match canonical lowercase "
    "`appeal-finance-worker-ballot-name`"
)
GOVERNANCE_REPORT_LABEL_ERROR = (
    "reports[].name must match canonical lowercase `appeal-finance-report-*`"
)
GOVERNANCE_WEEKLY_ROLLUP_LABEL_ERROR = (
    "weekly_rollups[].name must match canonical lowercase "
    "`appeal-finance-weekly-rollup-name`"
)
GOVERNANCE_SETTLEMENT_RECEIPT_LABEL_ERROR = (
    "settlement_receipts[].name must match canonical lowercase "
    "`appeal-finance-settlement-receipt-name`"
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

REQUIRED_APPEAL_CLASSES = ("content", "access", "fraud", "other")
REQUIRED_URGENCIES = ("normal", "high")
REQUIRED_QUOTE_API_QUOTES = len(REQUIRED_APPEAL_CLASSES) * len(REQUIRED_URGENCIES)
REQUIRED_OUTCOMES = (
    "uphold",
    "overturn",
    "modify",
    "withdrawn_before_panel",
    "withdrawn_after_panel",
    "frivolous",
    "escalated",
)
REQUIRED_RECONCILIATION_STATUSES = (
    "pending_forwarder_submission",
    "awaiting_refund_cancel",
    "settled",
    "mismatch",
)
REQUIRED_QUOTE_ROUTES = (
    "pricing_quote",
    "finance_settle",
    "finance_disburse",
)
REQUIRED_DEPOSIT_ROUTES = (
    "deposit_create",
    "deposit_status",
    "deposit_confirm",
    "ballot_announcement_gate",
)
REQUIRED_SETTLEMENT_ROUTES = (
    "settle_plan",
    "disburse_plan",
    "deposit_settle",
    "deposit_reconcile",
)
REQUIRED_SETTLEMENT_INSTRUCTION_STEPS = (
    "drawdown_instruction",
    "cancel_instruction",
)
REQUIRED_PAYLOAD_KINDS = (
    "appeal_finance_report",
    "appeal_finance_weekly_rollup",
    "appeal_finance_settlement_receipt",
)
REQUIRED_METRICS = (
    "sorafs_governance_dag_publish_total",
    "sorafs_governance_dag_last_publish_timestamp_seconds",
    "sorafs_governance_dag_published_bytes_total",
    "sorafs_governance_dag_backlog",
)
CONFIG_BOUND_KINDS = (
    "quote_api",
    "deposit_lifecycle",
    "settlement_execution",
    "settlement_submitter",
    "moderation_worker",
    "governance_dag_publication",
    "dashboard_metrics",
    "multi_peer_reconciliation",
    "governance_approval",
)
POLICY_BOUND_KINDS = ("governance_approval",)

SENSITIVE_KEYS = {
    "account_private_key",
    "authorization",
    "bearer_token",
    "body",
    "deposit_confirmation_payload",
    "evidence_payload",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "private_signer_key",
    "raw_ballot",
    "raw_instruction",
    "raw_ledger",
    "raw_lock",
    "raw_receipt",
    "raw_report",
    "raw_rollup",
    "response_body",
    "secret",
    "signature_key",
    "signed_transaction",
    "token",
}


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


@dataclass(frozen=True)
class EvidenceKind:
    """One SFM-4b2 rollout evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("pricing_config", "sorafs.appeal_finance.pricing_config_canary.v1"),
    EvidenceKind("quote_api", "sorafs.appeal_finance.quote_api_canary.v1"),
    EvidenceKind("deposit_lifecycle", "sorafs.appeal_finance.deposit_lifecycle_canary.v1"),
    EvidenceKind(
        "settlement_execution",
        "sorafs.appeal_finance.settlement_execution_canary.v1",
    ),
    EvidenceKind("settlement_submitter", "sorafs.appeal_finance.settlement_submitter_canary.v1"),
    EvidenceKind("moderation_worker", "sorafs.appeal_finance.moderation_worker_canary.v1"),
    EvidenceKind(
        "governance_dag_publication",
        "sorafs.appeal_finance.governance_dag_publication_canary.v1",
    ),
    EvidenceKind("dashboard_metrics", "sorafs.appeal_finance.dashboard_metrics_canary.v1"),
    EvidenceKind(
        "multi_peer_reconciliation",
        "sorafs.appeal_finance.multi_peer_reconciliation_canary.v1",
    ),
    EvidenceKind("governance_approval", "sorafs.appeal_finance.governance_approval.v1"),
)

SCHEMA_TO_KIND = {kind.schema: kind for kind in EVIDENCE_KINDS}
KIND_BY_NAME = {kind.name: kind for kind in EVIDENCE_KINDS}
DEFAULT_REQUIRED_KINDS = tuple(kind.name for kind in EVIDENCE_KINDS)
COMMON_CANARY_REQUIRED_FIELDS: tuple[str, ...] = (
    "schema",
    "status",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
    "config_digest_hex",
)
COMMON_GOVERNANCE_REQUIRED_FIELDS: tuple[str, ...] = (
    "schema",
    "status",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
)
EVIDENCE_REQUIRED_FIELDS: dict[str, tuple[str, ...]] = {
    "pricing_config": COMMON_CANARY_REQUIRED_FIELDS
    + (
        "config_version",
        "config_source",
        "policy_digest_hex",
        "class_count",
        "classes",
        "pricing_config_present",
        "settlement_config_present",
        "quote_ttl_present",
        "default_panel_size_present",
        "config_route_2xx",
        "status_route_2xx",
        "config_payload_included",
        "response_bodies_included",
    ),
    "quote_api": COMMON_CANARY_REQUIRED_FIELDS
    + (
        "route_count",
        "passed_route_count",
        "routes",
        "quote_count",
        "passed_quote_count",
        "classes",
        "urgencies",
        "deterministic_replay_passed",
        "deposit_bounds_enforced",
        "max_route_latency_ms",
        "payloads_included",
        "response_bodies_included",
    ),
    "deposit_lifecycle": COMMON_CANARY_REQUIRED_FIELDS
    + (
        "route_count",
        "passed_route_count",
        "routes",
        "deposit_probe_count",
        "deposit_probes",
        "confirmed_deposit_count",
        "payer_auth_enforced",
        "participant_status_gate_enforced",
        "mismatched_escrow_rejected",
        "unconfirmed_ballot_rejected",
        "ledger_lock_confirmed",
        "idempotency_key_bound",
        "evidence_hashes_bound",
        "max_route_latency_ms",
        "raw_instruction_included",
        "deposit_payloads_included",
        "response_bodies_included",
    ),
    "settlement_execution": COMMON_CANARY_REQUIRED_FIELDS
    + (
        "route_count",
        "passed_route_count",
        "routes",
        "settlement_probe_count",
        "instruction_step_count",
        "instruction_steps",
        "outcomes",
        "reconciliation_statuses",
        "drawdown_instruction_present",
        "cancel_instruction_present",
        "required_signer_bound",
        "deterministic_reconciliation_digest",
        "treasury_reconciliation_passed",
        "mismatched_ledger_rejected",
        "raw_instruction_included",
        "signed_transaction_included",
        "response_bodies_included",
    ),
    "settlement_submitter": COMMON_CANARY_REQUIRED_FIELDS
    + (
        "configured_signer_count",
        "signers",
        "queued_step_count",
        "steps",
        "submitted_step_count",
        "receipt_published",
        "required_authority_matched",
        "missing_signer_rejected",
        "wrong_authority_rejected",
        "rejected_or_expired_retry_verified",
        "max_settlement_lag_seconds",
        "raw_receipt_included",
        "signed_transaction_included",
    ),
    "moderation_worker": COMMON_CANARY_REQUIRED_FIELDS
    + (
        "worker_enabled",
        "storage_configured",
        "submitter_keys_configured",
        "ballot_replay_count",
        "ballots",
        "live_event_subscription_verified",
        "deposit_fingerprint_reconstructed",
        "evidence_hashes_verified",
        "runtime_ledger_validated",
        "pending_step_queued",
        "idempotent_rescan_verified",
        "retry_cap_enforced",
        "max_settlement_lag_seconds",
        "raw_ballot_included",
        "deposit_confirmation_payload_included",
    ),
    "governance_dag_publication": COMMON_CANARY_REQUIRED_FIELDS
    + (
        "report_count",
        "reports",
        "weekly_rollup_count",
        "weekly_rollups",
        "settlement_receipt_count",
        "settlement_receipts",
        "payload_kind_count",
        "payload_kinds",
        "publish_index_verified",
        "canonical_to_payloads_verified",
        "json_sidecars_verified",
        "blake3_sidecars_verified",
        "car_queue_verified",
        "runtime_signed_dag_verified",
        "report_publish_auth_enforced",
        "rollup_publish_auth_enforced",
        "raw_report_included",
        "raw_rollup_included",
        "raw_receipt_included",
    ),
    "dashboard_metrics": COMMON_CANARY_REQUIRED_FIELDS
    + (
        "metrics_scrape_success",
        "dashboard_provisioned",
        "alert_rules_installed",
        "hosted_public_dashboard_verified",
        "critical_alerts_firing",
        "metrics",
        "metric_count",
        "payload_kind_count",
        "payload_kinds",
        "response_bodies_included",
    ),
    "multi_peer_reconciliation": COMMON_CANARY_REQUIRED_FIELDS
    + (
        "peer_count",
        "peers",
        "validator_count",
        "validators",
        "case_count",
        "cases",
        "deposit_posted",
        "decision_ingested",
        "settlement_submitted",
        "disbursement_verified",
        "treasury_reconciliation_passed",
        "governance_dag_receipt_verified",
        "all_peers_reconciled",
        "qc_quorum_satisfied",
        "mismatch_count",
        "unexpected_failure_count",
        "raw_ledger_included",
    ),
    "governance_approval": COMMON_GOVERNANCE_REQUIRED_FIELDS
    + (
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "pricing_policy_present",
        "config_digest_hex",
        "settlement_policy_present",
        "deposit_custody_policy_present",
        "settlement_submitter_policy_present",
        "worker_retry_policy_present",
        "public_dashboard_rollout_accepted",
        "multi_peer_reconciliation_accepted",
        "config_source",
        "policy_digest_hex",
    ),
}


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SFM-4b2 rollout gate."""

    now_unix: int
    max_canary_age_secs: int
    max_dashboard_age_secs: int
    max_route_latency_ms: int
    max_settlement_lag_secs: int
    min_peers: int



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "schema",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
    "peer_count",
    "validator_count",
    "case_count",
    "config_digest_hex",
    "policy_digest_hex",
    "metric_count",
    "metrics",
)
RECONCILIATION_RUN_FIELDS: tuple[str, ...] = (
    "deployment_id",
    "environment",
    "generated_at_unix",
    "peer_count",
    "validator_count",
    "case_count",
    "config_digest_hex",
)


def validate_route_records(
    payload: dict[str, Any],
    errors: list[str],
    *,
    require_authz: bool,
    require_signature: bool,
    options: ValidationOptions,
) -> None:
    for index, record in require_object_array(payload, "routes", errors):
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
        if require_authz:
            require_bool_true(
                record,
                "authz_enforced",
                errors,
                path=f"routes[{index}].authz_enforced",
            )
        if require_signature:
            require_bool_true(
                record,
                "signature_verified",
                errors,
                path=f"routes[{index}].signature_verified",
            )
        require_maximum_int(
            record,
            "latency_ms",
            options.max_route_latency_ms,
            errors,
            path=f"routes[{index}].latency_ms",
        )


def validate_route_inventory(
    payload: dict[str, Any],
    required_routes: tuple[str, ...],
    errors: list[str],
) -> None:
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", required_routes, errors)
    require_string_inventory_count_match(
        payload,
        "routes",
        "route_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    require_only_required_values(payload, "routes", "name", required_routes, errors)


def require_config_version(payload: dict[str, Any], errors: list[str]) -> str:
    """Require a reviewed lowercase appeal-finance config version label."""

    config_version = require_string(payload, "config_version", errors)
    if not config_version:
        return ""
    match = CONFIG_VERSION_PATTERN.fullmatch(config_version)
    if match is None:
        errors.append(CONFIG_VERSION_ERROR)
        return ""
    name = config_version[: config_version.rfind("-v")]
    forbidden = forbidden_non_production_markers(name, FORBIDDEN_CONFIG_VERSION_MARKERS)
    if forbidden:
        errors.append(
            f"config_version must not contain non-production markers {forbidden}"
        )
        return ""
    return config_version


def require_inventory_label(
    value: Any,
    *,
    path: str,
    pattern: re.Pattern[str],
    label_error: str,
    errors: list[str],
) -> str:
    """Require a reviewed production inventory label."""

    if not isinstance(value, str):
        return ""
    if pattern.fullmatch(value) is None:
        errors.append(label_error)
        return value
    forbidden = forbidden_non_production_markers(value, FORBIDDEN_INVENTORY_LABEL_MARKERS)
    if forbidden:
        errors.append(f"{path} must not contain non-production markers {forbidden}")
    return value


def validate_pricing_config(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_canary_age_secs,
    )
    require_hex(payload, "config_digest_hex", HEX64_LEN, errors)
    require_policy_digest(payload, errors)
    require_config_version(payload, errors)
    require_iroha_config_binding(payload, errors, bound_field=None)
    require_minimum_int(
        payload,
        "class_count",
        len(REQUIRED_APPEAL_CLASSES),
        errors,
    )
    require_string_coverage(payload, "classes", "", REQUIRED_APPEAL_CLASSES, errors)
    require_string_inventory_count_match(
        payload,
        "classes",
        "class_count",
        errors,
    )
    require_only_required_values(payload, "classes", "", REQUIRED_APPEAL_CLASSES, errors)
    require_bool_true(payload, "pricing_config_present", errors)
    require_bool_true(payload, "settlement_config_present", errors)
    require_bool_true(payload, "quote_ttl_present", errors)
    require_bool_true(payload, "default_panel_size_present", errors)
    require_bool_true(payload, "config_route_2xx", errors)
    require_bool_true(payload, "status_route_2xx", errors)
    require_false(payload, "config_payload_included", errors)
    require_false(payload, "response_bodies_included", errors)


def unique_scalar_inventory_count(
    payload: dict[str, Any],
    field: str,
    errors: list[str],
) -> int:
    """Return the unique scalar labels for a rollout inventory field."""

    items = payload.get(field)
    if not isinstance(items, list):
        return 0
    labels: list[str] = []
    malformed = False
    for item in items:
        if not diagnostic_text_is_canonical(item):
            malformed = True
            continue
        labels.append(item)
    if malformed:
        return 0
    unique_labels = set(labels)
    if len(unique_labels) != len(labels):
        errors.append(f"{field} must not contain duplicate values")
    return len(unique_labels)


def validate_quote_api(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_canary_age_secs,
    )
    require_hex(payload, "config_digest_hex", HEX64_LEN, errors)
    validate_route_inventory(payload, REQUIRED_QUOTE_ROUTES, errors)
    quote_count = require_count_equal(payload, "quote_count", "passed_quote_count", errors)
    require_string_coverage(payload, "classes", "", REQUIRED_APPEAL_CLASSES, errors)
    require_string_coverage(payload, "urgencies", "", REQUIRED_URGENCIES, errors)
    require_only_required_values(payload, "classes", "", REQUIRED_APPEAL_CLASSES, errors)
    require_only_required_values(payload, "urgencies", "", REQUIRED_URGENCIES, errors)
    class_count = unique_scalar_inventory_count(payload, "classes", errors)
    urgency_count = unique_scalar_inventory_count(payload, "urgencies", errors)
    if quote_count and class_count and urgency_count:
        expected_quotes = class_count * urgency_count
        if quote_count != expected_quotes:
            errors.append("quote_count must equal unique classes * urgencies count")
    require_bool_true(payload, "deterministic_replay_passed", errors)
    require_bool_true(payload, "deposit_bounds_enforced", errors)
    require_maximum_int(
        payload,
        "max_route_latency_ms",
        options.max_route_latency_ms,
        errors,
        minimum=1,
    )
    require_false(payload, "payloads_included", errors)
    require_false(payload, "response_bodies_included", errors)
    validate_route_records(
        payload,
        errors,
        require_authz=False,
        require_signature=False,
        options=options,
    )


def validate_deposit_lifecycle(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_canary_age_secs,
    )
    require_hex(payload, "config_digest_hex", HEX64_LEN, errors)
    validate_route_inventory(payload, REQUIRED_DEPOSIT_ROUTES, errors)
    deposit_probe_count = require_positive_int(payload, "deposit_probe_count", errors)
    confirmed_deposit_count = require_positive_int(
        payload, "confirmed_deposit_count", errors
    )
    require_string_inventory_count_match(
        payload,
        "deposit_probes",
        "deposit_probe_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    confirmed_probe_count = 0
    for index, record in require_object_array(payload, "deposit_probes", errors):
        name = require_string(record, "name", errors)
        require_inventory_label(
            name,
            path=f"deposit_probes[{index}].name",
            pattern=DEPOSIT_PROBE_LABEL_PATTERN,
            label_error=DEPOSIT_PROBE_LABEL_ERROR,
            errors=errors,
        )
        confirmed = record.get("confirmed")
        if not isinstance(confirmed, bool):
            errors.append(f"deposit_probes[{index}].confirmed must be a boolean")
        elif confirmed:
            confirmed_probe_count += 1
    if (
        isinstance(deposit_probe_count, int)
        and isinstance(confirmed_deposit_count, int)
        and confirmed_deposit_count > deposit_probe_count
    ):
        errors.append("confirmed_deposit_count must be <= deposit_probe_count")
    if (
        isinstance(confirmed_deposit_count, int)
        and confirmed_deposit_count != confirmed_probe_count
    ):
        errors.append(
            "confirmed_deposit_count must match confirmed deposit probes count"
        )
    require_bool_true(payload, "payer_auth_enforced", errors)
    require_bool_true(payload, "participant_status_gate_enforced", errors)
    require_bool_true(payload, "mismatched_escrow_rejected", errors)
    require_bool_true(payload, "unconfirmed_ballot_rejected", errors)
    require_bool_true(payload, "ledger_lock_confirmed", errors)
    require_bool_true(payload, "idempotency_key_bound", errors)
    require_bool_true(payload, "evidence_hashes_bound", errors)
    require_maximum_int(
        payload,
        "max_route_latency_ms",
        options.max_route_latency_ms,
        errors,
        minimum=1,
    )
    require_false(payload, "raw_instruction_included", errors)
    require_false(payload, "deposit_payloads_included", errors)
    require_false(payload, "response_bodies_included", errors)
    validate_route_records(
        payload,
        errors,
        require_authz=True,
        require_signature=True,
        options=options,
    )


def validate_settlement_execution(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_canary_age_secs,
    )
    require_hex(payload, "config_digest_hex", HEX64_LEN, errors)
    validate_route_inventory(payload, REQUIRED_SETTLEMENT_ROUTES, errors)
    settlement_probe_count = require_positive_int(payload, "settlement_probe_count", errors)
    require_positive_int(payload, "instruction_step_count", errors)
    require_string_coverage(
        payload,
        "instruction_steps",
        "",
        REQUIRED_SETTLEMENT_INSTRUCTION_STEPS,
        errors,
    )
    require_string_inventory_count_match(
        payload,
        "instruction_steps",
        "instruction_step_count",
        errors,
    )
    require_only_required_values(
        payload,
        "instruction_steps",
        "",
        REQUIRED_SETTLEMENT_INSTRUCTION_STEPS,
        errors,
    )
    require_string_coverage(payload, "outcomes", "", REQUIRED_OUTCOMES, errors)
    require_string_coverage(
        payload,
        "reconciliation_statuses",
        "",
        REQUIRED_RECONCILIATION_STATUSES,
        errors,
    )
    require_only_required_values(payload, "outcomes", "", REQUIRED_OUTCOMES, errors)
    require_only_required_values(
        payload,
        "reconciliation_statuses",
        "",
        REQUIRED_RECONCILIATION_STATUSES,
        errors,
    )
    outcome_count = unique_scalar_inventory_count(payload, "outcomes", errors)
    unique_scalar_inventory_count(payload, "reconciliation_statuses", errors)
    if settlement_probe_count and outcome_count and settlement_probe_count != outcome_count:
        errors.append("settlement_probe_count must match unique outcomes count")
    require_bool_true(payload, "drawdown_instruction_present", errors)
    require_bool_true(payload, "cancel_instruction_present", errors)
    require_bool_true(payload, "required_signer_bound", errors)
    require_bool_true(payload, "deterministic_reconciliation_digest", errors)
    require_bool_true(payload, "treasury_reconciliation_passed", errors)
    require_bool_true(payload, "mismatched_ledger_rejected", errors)
    require_false(payload, "raw_instruction_included", errors)
    require_false(payload, "signed_transaction_included", errors)
    require_false(payload, "response_bodies_included", errors)
    validate_route_records(
        payload,
        errors,
        require_authz=True,
        require_signature=True,
        options=options,
    )


def validate_settlement_submitter(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_canary_age_secs,
    )
    require_hex(payload, "config_digest_hex", HEX64_LEN, errors)
    require_positive_int(payload, "configured_signer_count", errors)
    require_string_inventory_count_match(
        payload,
        "signers",
        "configured_signer_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for index, record in require_object_array(payload, "signers", errors):
        name = require_string(record, "name", errors)
        require_inventory_label(
            name,
            path=f"signers[{index}].name",
            pattern=SUBMITTER_SIGNER_LABEL_PATTERN,
            label_error=SUBMITTER_SIGNER_LABEL_ERROR,
            errors=errors,
        )
    queued_step_count = require_positive_int(payload, "queued_step_count", errors)
    submitted_step_count = require_positive_int(payload, "submitted_step_count", errors)
    require_string_inventory_count_match(
        payload,
        "steps",
        "queued_step_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    submitted_partition_count = 0
    for index, record in require_object_array(payload, "steps", errors):
        name = require_string(record, "name", errors)
        require_inventory_label(
            name,
            path=f"steps[{index}].name",
            pattern=SUBMITTER_STEP_LABEL_PATTERN,
            label_error=SUBMITTER_STEP_LABEL_ERROR,
            errors=errors,
        )
        submitted = record.get("submitted")
        if not isinstance(submitted, bool):
            errors.append(f"steps[{index}].submitted must be a boolean")
        elif submitted:
            submitted_partition_count += 1
    if (
        isinstance(queued_step_count, int)
        and isinstance(submitted_step_count, int)
        and submitted_step_count > queued_step_count
    ):
        errors.append("submitted_step_count must be <= queued_step_count")
    if (
        isinstance(submitted_step_count, int)
        and submitted_step_count != submitted_partition_count
    ):
        errors.append("submitted_step_count must match submitted steps count")
    require_bool_true(payload, "receipt_published", errors)
    require_bool_true(payload, "required_authority_matched", errors)
    require_bool_true(payload, "missing_signer_rejected", errors)
    require_bool_true(payload, "wrong_authority_rejected", errors)
    require_bool_true(payload, "rejected_or_expired_retry_verified", errors)
    require_maximum_int(
        payload,
        "max_settlement_lag_seconds",
        options.max_settlement_lag_secs,
        errors,
    )
    require_false(payload, "raw_receipt_included", errors)
    require_false(payload, "signed_transaction_included", errors)


def validate_moderation_worker(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_canary_age_secs,
    )
    require_hex(payload, "config_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "worker_enabled", errors)
    require_bool_true(payload, "storage_configured", errors)
    require_bool_true(payload, "submitter_keys_configured", errors)
    require_positive_int(payload, "ballot_replay_count", errors)
    require_string_inventory_count_match(
        payload,
        "ballots",
        "ballot_replay_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for index, record in require_object_array(payload, "ballots", errors):
        name = require_string(record, "name", errors)
        require_inventory_label(
            name,
            path=f"ballots[{index}].name",
            pattern=WORKER_BALLOT_LABEL_PATTERN,
            label_error=WORKER_BALLOT_LABEL_ERROR,
            errors=errors,
        )
    require_bool_true(payload, "live_event_subscription_verified", errors)
    require_bool_true(payload, "deposit_fingerprint_reconstructed", errors)
    require_bool_true(payload, "evidence_hashes_verified", errors)
    require_bool_true(payload, "runtime_ledger_validated", errors)
    require_bool_true(payload, "pending_step_queued", errors)
    require_bool_true(payload, "idempotent_rescan_verified", errors)
    require_bool_true(payload, "retry_cap_enforced", errors)
    require_maximum_int(
        payload,
        "max_settlement_lag_seconds",
        options.max_settlement_lag_secs,
        errors,
    )
    require_false(payload, "raw_ballot_included", errors)
    require_false(payload, "deposit_confirmation_payload_included", errors)


def validate_governance_dag_publication(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_canary_age_secs,
    )
    require_hex(payload, "config_digest_hex", HEX64_LEN, errors)
    require_positive_int(payload, "report_count", errors)
    require_string_inventory_count_match(
        payload,
        "reports",
        "report_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for index, record in require_object_array(payload, "reports", errors):
        name = require_string(record, "name", errors)
        require_inventory_label(
            name,
            path=f"reports[{index}].name",
            pattern=GOVERNANCE_REPORT_LABEL_PATTERN,
            label_error=GOVERNANCE_REPORT_LABEL_ERROR,
            errors=errors,
        )
    require_positive_int(payload, "weekly_rollup_count", errors)
    require_string_inventory_count_match(
        payload,
        "weekly_rollups",
        "weekly_rollup_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for index, record in require_object_array(payload, "weekly_rollups", errors):
        name = require_string(record, "name", errors)
        require_inventory_label(
            name,
            path=f"weekly_rollups[{index}].name",
            pattern=GOVERNANCE_WEEKLY_ROLLUP_LABEL_PATTERN,
            label_error=GOVERNANCE_WEEKLY_ROLLUP_LABEL_ERROR,
            errors=errors,
        )
    require_positive_int(payload, "settlement_receipt_count", errors)
    require_string_inventory_count_match(
        payload,
        "settlement_receipts",
        "settlement_receipt_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for index, record in require_object_array(payload, "settlement_receipts", errors):
        name = require_string(record, "name", errors)
        require_inventory_label(
            name,
            path=f"settlement_receipts[{index}].name",
            pattern=GOVERNANCE_SETTLEMENT_RECEIPT_LABEL_PATTERN,
            label_error=GOVERNANCE_SETTLEMENT_RECEIPT_LABEL_ERROR,
            errors=errors,
        )
    require_string_coverage(payload, "payload_kinds", "", REQUIRED_PAYLOAD_KINDS, errors)
    require_minimum_int(
        payload,
        "payload_kind_count",
        len(REQUIRED_PAYLOAD_KINDS),
        errors,
    )
    require_string_inventory_count_match(
        payload,
        "payload_kinds",
        "payload_kind_count",
        errors,
    )
    require_only_required_values(
        payload,
        "payload_kinds",
        "",
        REQUIRED_PAYLOAD_KINDS,
        errors,
    )
    require_bool_true(payload, "publish_index_verified", errors)
    require_bool_true(payload, "canonical_to_payloads_verified", errors)
    require_bool_true(payload, "json_sidecars_verified", errors)
    require_bool_true(payload, "blake3_sidecars_verified", errors)
    require_bool_true(payload, "car_queue_verified", errors)
    require_bool_true(payload, "runtime_signed_dag_verified", errors)
    require_bool_true(payload, "report_publish_auth_enforced", errors)
    require_bool_true(payload, "rollup_publish_auth_enforced", errors)
    require_false(payload, "raw_report_included", errors)
    require_false(payload, "raw_rollup_included", errors)
    require_false(payload, "raw_receipt_included", errors)


def validate_dashboard_metrics(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_dashboard_age_secs,
    )
    require_hex(payload, "config_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "metrics_scrape_success", errors)
    require_bool_true(payload, "dashboard_provisioned", errors)
    require_bool_true(payload, "alert_rules_installed", errors)
    require_bool_true(payload, "hosted_public_dashboard_verified", errors)
    require_false(payload, "critical_alerts_firing", errors)
    require_string_coverage(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_positive_int(payload, "metric_count", errors)
    require_string_inventory_count_match(payload, "metrics", "metric_count", errors)
    require_only_required_values(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_string_coverage(payload, "payload_kinds", "", REQUIRED_PAYLOAD_KINDS, errors)
    require_minimum_int(
        payload,
        "payload_kind_count",
        len(REQUIRED_PAYLOAD_KINDS),
        errors,
    )
    require_string_inventory_count_match(
        payload,
        "payload_kinds",
        "payload_kind_count",
        errors,
    )
    require_only_required_values(
        payload,
        "payload_kinds",
        "",
        REQUIRED_PAYLOAD_KINDS,
        errors,
    )
    require_false(payload, "response_bodies_included", errors)


def validate_multi_peer_reconciliation(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_canary_age_secs,
    )
    require_hex(payload, "config_digest_hex", HEX64_LEN, errors)
    require_minimum_int(payload, "peer_count", options.min_peers, errors)
    require_string_inventory_count_match(
        payload,
        "peers",
        "peer_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for index, record in require_object_array(payload, "peers", errors):
        name = require_string(record, "name", errors)
        require_inventory_label(
            name,
            path=f"peers[{index}].name",
            pattern=PEER_LABEL_PATTERN,
            label_error=PEER_LABEL_ERROR,
            errors=errors,
        )
    require_minimum_int(payload, "validator_count", options.min_peers, errors)
    require_string_inventory_count_match(
        payload,
        "validators",
        "validator_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for index, record in require_object_array(payload, "validators", errors):
        name = require_string(record, "name", errors)
        require_inventory_label(
            name,
            path=f"validators[{index}].name",
            pattern=VALIDATOR_LABEL_PATTERN,
            label_error=VALIDATOR_LABEL_ERROR,
            errors=errors,
        )
    case_count = require_positive_int(payload, "case_count", errors)
    require_string_inventory_count_match(
        payload,
        "cases",
        "case_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    reconciled_case_count = 0
    for index, record in require_object_array(payload, "cases", errors):
        name = require_string(record, "name", errors)
        require_inventory_label(
            name,
            path=f"cases[{index}].name",
            pattern=RECONCILIATION_CASE_LABEL_PATTERN,
            label_error=RECONCILIATION_CASE_LABEL_ERROR,
            errors=errors,
        )
        reconciled = record.get("reconciled")
        if not isinstance(reconciled, bool):
            errors.append(f"cases[{index}].reconciled must be a boolean")
        elif reconciled:
            reconciled_case_count += 1
    if case_count > 0 and case_count != reconciled_case_count:
        errors.append("case_count must match reconciled cases count")
    require_bool_true(payload, "deposit_posted", errors)
    require_bool_true(payload, "decision_ingested", errors)
    require_bool_true(payload, "settlement_submitted", errors)
    require_bool_true(payload, "disbursement_verified", errors)
    require_bool_true(payload, "treasury_reconciliation_passed", errors)
    require_bool_true(payload, "governance_dag_receipt_verified", errors)
    require_bool_true(payload, "all_peers_reconciled", errors)
    require_bool_true(payload, "qc_quorum_satisfied", errors)
    require_zero_count(payload, "mismatch_count", errors)
    require_zero_count(payload, "unexpected_failure_count", errors)
    require_false(payload, "raw_ledger_included", errors)


def validate_governance_approval(payload: dict[str, Any], errors: list[str]) -> None:
    require_positive_int(payload, "generated_at_unix", errors)
    require_config_backed_governance_approval(payload, errors)
    require_bool_true(payload, "pricing_policy_present", errors)
    require_hex(payload, "config_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "settlement_policy_present", errors)
    require_bool_true(payload, "deposit_custody_policy_present", errors)
    require_bool_true(payload, "settlement_submitter_policy_present", errors)
    require_bool_true(payload, "worker_retry_policy_present", errors)
    require_bool_true(payload, "public_dashboard_rollout_accepted", errors)
    require_bool_true(payload, "multi_peer_reconciliation_accepted", errors)
    require_policy_digest(payload, errors)


def validate_kind_specific(
    kind: EvidenceKind,
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_passed_status(payload, errors)

    if kind.name == "pricing_config":
        validate_pricing_config(payload, errors, options)
    elif kind.name == "quote_api":
        validate_quote_api(payload, errors, options)
    elif kind.name == "deposit_lifecycle":
        validate_deposit_lifecycle(payload, errors, options)
    elif kind.name == "settlement_execution":
        validate_settlement_execution(payload, errors, options)
    elif kind.name == "settlement_submitter":
        validate_settlement_submitter(payload, errors, options)
    elif kind.name == "moderation_worker":
        validate_moderation_worker(payload, errors, options)
    elif kind.name == "governance_dag_publication":
        validate_governance_dag_publication(payload, errors, options)
    elif kind.name == "dashboard_metrics":
        validate_dashboard_metrics(payload, errors, options)
    elif kind.name == "multi_peer_reconciliation":
        validate_multi_peer_reconciliation(payload, errors, options)
    elif kind.name == "governance_approval":
        validate_governance_approval(payload, errors)


def validate_evidence_payload(
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[str | None, list[str]]:
    return validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS SFM-4b2 rollout artifact",
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
    valid_multi_peer_runs: list[dict[str, Any]] = []
    valid_config_digests: set[str] = set()
    valid_policy_digests: set[str] = set()
    valid_config_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_policy_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
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
        if kind_name == "multi_peer_reconciliation":
            run = artifact_fingerprint(payload, RECONCILIATION_RUN_FIELDS)
            if evidence_artifact_is_valid(artifact):
                valid_multi_peer_runs.append(run)
        if evidence_artifact_is_valid(artifact):
            digest = evidence_artifact_fingerprint(artifact).get("config_digest_hex")
            if kind_name == "pricing_config" and isinstance(digest, str):
                valid_config_digests.add(digest)
            elif kind_name in CONFIG_BOUND_KINDS:
                valid_config_bound_artifacts.append((kind_name, artifact))
            if kind_name == "dashboard_metrics":
                record_observed_evidence_value(metric_counts, payload.get("metric_count"))
                metric_names.update(hashable_evidence_values(payload.get("metrics")))
            policy_digest = evidence_artifact_fingerprint(artifact).get(
                "policy_digest_hex"
            )
            if kind_name == "pricing_config" and isinstance(policy_digest, str):
                valid_policy_digests.add(policy_digest)
            elif kind_name in POLICY_BOUND_KINDS:
                valid_policy_bound_artifacts.append((kind_name, artifact))
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact, errors)
        record_evidence_validation_errors(path, validation_errors, errors)

    valid_config_digests = require_single_active_digest(
        valid_config_digests,
        errors,
        label="valid_config_digests",
    )
    valid_policy_digests = require_single_active_digest(
        valid_policy_digests,
        errors,
        label="valid_policy_digests",
    )

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("pricing_config",) + CONFIG_BOUND_KINDS,
        bound_artifacts=valid_config_bound_artifacts,
        valid_anchor_digests=valid_config_digests,
        digest_field="config_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} config_digest_hex must reference a valid "
            "pricing_config config_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} config_digest_hex requires a valid pricing_config "
            "config_digest_hex"
        ),
    )
    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("pricing_config",) + POLICY_BOUND_KINDS,
        bound_artifacts=valid_policy_bound_artifacts,
        valid_anchor_digests=valid_policy_digests,
        digest_field="policy_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} policy_digest_hex must reference a valid "
            "pricing_config policy_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} policy_digest_hex requires a valid pricing_config "
            "policy_digest_hex"
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
            "max_canary_age_secs": options.max_canary_age_secs,
            "max_dashboard_age_secs": options.max_dashboard_age_secs,
            "max_route_latency_ms": options.max_route_latency_ms,
            "max_settlement_lag_secs": options.max_settlement_lag_secs,
            "min_peers": options.min_peers,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "recognized_artifacts": recognized_evidence_artifacts(artifacts_by_kind),
        "valid_multi_peer_runs": valid_multi_peer_runs,
        "valid_config_digests": sorted(valid_config_digests),
        "valid_policy_digests": sorted(valid_policy_digests),
        "metrics": sorted(metric_names),
        "metric_count_values": sorted(metric_counts),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS SFM-4b2 appeal finance rollout evidence artifacts."
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
        help="Required evidence kind, or comma-separated kinds. Defaults to all SFM-4b2 kinds.",
    )
    parser.add_argument("--summary-out", type=Path, help="Optional summary JSON output path.")
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        required=True,
        help="Required reviewed validator clock used for age checks.",
    )
    parser.add_argument(
        "--max-canary-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_CANARY_AGE_SECS,
    )
    parser.add_argument(
        "--max-dashboard-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_DASHBOARD_AGE_SECS,
    )
    parser.add_argument(
        "--max-route-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_ROUTE_LATENCY_MS,
    )
    parser.add_argument(
        "--max-settlement-lag-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_SETTLEMENT_LAG_SECS,
    )
    parser.add_argument("--min-peers", type=positive_int_arg, default=DEFAULT_MIN_PEERS)
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
        max_canary_age_secs=args.max_canary_age_secs,
        max_dashboard_age_secs=args.max_dashboard_age_secs,
        max_route_latency_ms=args.max_route_latency_ms,
        max_settlement_lag_secs=args.max_settlement_lag_secs,
        min_peers=args.min_peers,
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
            "ERROR: SoraFS appeal finance rollout evidence is incomplete:",
            errors,
        )
        return 1

    emit_checker_notice(
        "SoraFS appeal finance rollout evidence is ready: "
        f"{summary['recognized_artifact_count']} recognized artifact(s) cover "
        f"{len(required_kinds)} required kind(s), including "
        f"{len(summary['valid_multi_peer_runs'])} multi-peer reconciliation run(s).",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
