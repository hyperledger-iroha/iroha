#!/usr/bin/env python3
"""Validate SoraFS appeal finance rollout evidence artifacts."""

from __future__ import annotations

import argparse
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
    build_evidence_artifact,
    count_evidence_artifacts,
    count_evidence_files,
    evidence_gate_status,
    evidence_artifact_is_valid,
    evidence_artifact_fingerprint,
    evidence_schema_by_kind,
    init_evidence_artifact_buckets,
    build_required_evidence_summary,
    record_explicit_evidence_validation_errors,
    record_evidence_artifact,
    record_evidence_validation_errors,
    validate_bound_evidence_digest_references,
    require_2xx_status,
    require_bool_true,
    require_count_equal,
    require_false,
    require_hex,
    require_config_backed_governance_approval,
    require_iroha_config_binding,
    validate_standard_evidence_payload,
    require_maximum_number,
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


SUMMARY_SCHEMA = "sorafs.appeal_finance.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_CANARY_AGE_SECS = 24 * 60 * 60
DEFAULT_MAX_DASHBOARD_AGE_SECS = 7 * 24 * 60 * 60
DEFAULT_MAX_ROUTE_LATENCY_MS = 1_500
DEFAULT_MAX_SETTLEMENT_LAG_SECS = 15 * 60
DEFAULT_MIN_PEERS = 4
HEX64_LEN = 64

REQUIRED_APPEAL_CLASSES = ("content", "access", "fraud", "other")
REQUIRED_URGENCIES = ("normal", "high")
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
    "pending_client_submission",
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


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SFM-4b2 rollout gate."""

    now_unix: int
    max_canary_age_secs: int
    max_dashboard_age_secs: int
    max_route_latency_ms: int
    max_settlement_lag_secs: int
    min_peers: int



FINGERPRINT_FIELDS: tuple[str, ...] = ("schema", "generated_at_unix", "config_digest_hex")


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
        if record.get("latency_ms") is not None:
            require_maximum_number(
                record,
                "latency_ms",
                options.max_route_latency_ms,
                errors,
                path=f"routes[{index}].latency_ms",
            )


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
    require_string(payload, "config_version", errors)
    require_iroha_config_binding(payload, errors, bound_field=None)
    require_minimum_int(
        payload,
        "class_count",
        len(REQUIRED_APPEAL_CLASSES),
        errors,
    )
    require_bool_true(payload, "pricing_config_present", errors)
    require_bool_true(payload, "settlement_config_present", errors)
    require_bool_true(payload, "quote_ttl_present", errors)
    require_bool_true(payload, "default_panel_size_present", errors)
    require_bool_true(payload, "config_route_2xx", errors)
    require_bool_true(payload, "status_route_2xx", errors)
    require_false(payload, "config_payload_included", errors)
    require_false(payload, "response_bodies_included", errors)


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
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_QUOTE_ROUTES, errors)
    require_count_equal(payload, "quote_count", "passed_quote_count", errors)
    require_string_coverage(payload, "classes", "", REQUIRED_APPEAL_CLASSES, errors)
    require_string_coverage(payload, "urgencies", "", REQUIRED_URGENCIES, errors)
    require_bool_true(payload, "deterministic_replay_passed", errors)
    require_bool_true(payload, "deposit_bounds_enforced", errors)
    require_maximum_number(
        payload,
        "max_route_latency_ms",
        options.max_route_latency_ms,
        errors,
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
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_DEPOSIT_ROUTES, errors)
    require_positive_int(payload, "deposit_probe_count", errors)
    require_positive_int(payload, "confirmed_deposit_count", errors)
    require_bool_true(payload, "payer_auth_enforced", errors)
    require_bool_true(payload, "participant_status_gate_enforced", errors)
    require_bool_true(payload, "mismatched_escrow_rejected", errors)
    require_bool_true(payload, "unconfirmed_ballot_rejected", errors)
    require_bool_true(payload, "ledger_lock_confirmed", errors)
    require_bool_true(payload, "idempotency_key_bound", errors)
    require_bool_true(payload, "evidence_hashes_bound", errors)
    require_maximum_number(
        payload,
        "max_route_latency_ms",
        options.max_route_latency_ms,
        errors,
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
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_SETTLEMENT_ROUTES, errors)
    require_positive_int(payload, "settlement_probe_count", errors)
    require_positive_int(payload, "instruction_step_count", errors)
    require_string_coverage(payload, "outcomes", "", REQUIRED_OUTCOMES, errors)
    require_string_coverage(
        payload,
        "reconciliation_statuses",
        "",
        REQUIRED_RECONCILIATION_STATUSES,
        errors,
    )
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
    require_positive_int(payload, "queued_step_count", errors)
    require_positive_int(payload, "submitted_step_count", errors)
    require_bool_true(payload, "receipt_published", errors)
    require_bool_true(payload, "required_authority_matched", errors)
    require_bool_true(payload, "missing_signer_rejected", errors)
    require_bool_true(payload, "wrong_authority_rejected", errors)
    require_bool_true(payload, "rejected_or_expired_retry_verified", errors)
    require_maximum_number(
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
    require_bool_true(payload, "live_event_subscription_verified", errors)
    require_bool_true(payload, "deposit_fingerprint_reconstructed", errors)
    require_bool_true(payload, "evidence_hashes_verified", errors)
    require_bool_true(payload, "runtime_ledger_validated", errors)
    require_bool_true(payload, "pending_step_queued", errors)
    require_bool_true(payload, "idempotent_rescan_verified", errors)
    require_bool_true(payload, "retry_cap_enforced", errors)
    require_maximum_number(
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
    require_positive_int(payload, "weekly_rollup_count", errors)
    require_positive_int(payload, "settlement_receipt_count", errors)
    require_string_coverage(payload, "payload_kinds", "", REQUIRED_PAYLOAD_KINDS, errors)
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
    require_string_coverage(payload, "payload_kinds", "", REQUIRED_PAYLOAD_KINDS, errors)
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
    require_minimum_int(payload, "validator_count", options.min_peers, errors)
    require_positive_int(payload, "case_count", errors)
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
    )



def reconciliation_fingerprint(payload: dict[str, Any]) -> dict[str, Any]:
    return {
        "generated_at_unix": payload.get("generated_at_unix"),
        "peer_count": payload.get("peer_count"),
        "validator_count": payload.get("validator_count"),
        "case_count": payload.get("case_count"),
        "config_digest_hex": payload.get("config_digest_hex"),
    }


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
    valid_config_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
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
            path,
            digest,
            payload,
            validation_errors,
            FINGERPRINT_FIELDS,
        )
        if kind_name == "multi_peer_reconciliation":
            run = reconciliation_fingerprint(payload)
            artifact["run"] = run
            if evidence_artifact_is_valid(artifact):
                valid_multi_peer_runs.append(run)
        if evidence_artifact_is_valid(artifact):
            digest = evidence_artifact_fingerprint(artifact).get("config_digest_hex")
            if kind_name == "pricing_config" and isinstance(digest, str):
                valid_config_digests.add(digest.lower())
            elif kind_name in CONFIG_BOUND_KINDS:
                valid_config_bound_artifacts.append((kind_name, artifact))
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact)
        record_evidence_validation_errors(path, validation_errors, errors)

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
        "valid_multi_peer_runs": valid_multi_peer_runs,
        "valid_config_digests": sorted(valid_config_digests),
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
        default=int(time.time()),
        help="Validator clock used for age checks. Defaults to current Unix time.",
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
        emit_checker_error_lines((str(error),))
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
        emit_checker_error_lines((str(error),))
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
