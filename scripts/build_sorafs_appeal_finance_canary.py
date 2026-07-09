#!/usr/bin/env python3
"""Build payload-free SoraFS appeal finance rollout canary artifacts."""

from __future__ import annotations

import argparse
import json
import os
import re
import secrets
import sys
from collections.abc import Iterable, Sequence
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_appeal_finance_rollout_evidence import (  # noqa: E402
    CONFIG_VERSION_ERROR,
    CONFIG_VERSION_PATTERN,
    DEFAULT_MAX_CANARY_AGE_SECS,
    DEFAULT_MAX_DASHBOARD_AGE_SECS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MAX_SETTLEMENT_LAG_SECS,
    DEFAULT_MIN_PEERS,
    DEPOSIT_PROBE_LABEL_ERROR,
    DEPOSIT_PROBE_LABEL_PATTERN,
    FORBIDDEN_CONFIG_VERSION_MARKERS,
    FORBIDDEN_INVENTORY_LABEL_MARKERS,
    GOVERNANCE_REPORT_LABEL_ERROR,
    GOVERNANCE_REPORT_LABEL_PATTERN,
    GOVERNANCE_SETTLEMENT_RECEIPT_LABEL_ERROR,
    GOVERNANCE_SETTLEMENT_RECEIPT_LABEL_PATTERN,
    GOVERNANCE_WEEKLY_ROLLUP_LABEL_ERROR,
    GOVERNANCE_WEEKLY_ROLLUP_LABEL_PATTERN,
    KIND_BY_NAME,
    PEER_LABEL_ERROR,
    PEER_LABEL_PATTERN,
    REQUIRED_APPEAL_CLASSES,
    REQUIRED_DEPOSIT_ROUTES,
    REQUIRED_METRICS,
    REQUIRED_OUTCOMES,
    REQUIRED_PAYLOAD_KINDS,
    REQUIRED_QUOTE_API_QUOTES,
    REQUIRED_QUOTE_ROUTES,
    REQUIRED_RECONCILIATION_STATUSES,
    REQUIRED_SETTLEMENT_INSTRUCTION_STEPS,
    REQUIRED_SETTLEMENT_ROUTES,
    REQUIRED_URGENCIES,
    RECONCILIATION_CASE_LABEL_ERROR,
    RECONCILIATION_CASE_LABEL_PATTERN,
    SUBMITTER_SIGNER_LABEL_ERROR,
    SUBMITTER_SIGNER_LABEL_PATTERN,
    SUBMITTER_STEP_LABEL_ERROR,
    SUBMITTER_STEP_LABEL_PATTERN,
    VALIDATOR_LABEL_ERROR,
    VALIDATOR_LABEL_PATTERN,
    ValidationOptions,
    WORKER_BALLOT_LABEL_ERROR,
    WORKER_BALLOT_LABEL_PATTERN,
    validate_evidence_payload,
)
from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_exception,
    fsync_checker_output_parent,
    write_all_checker_summary_bytes,
    validate_checker_output_parent,
)
from sorafs_path_identity import (  # noqa: E402
    diagnostic_text_is_canonical,
    error_diagnostic_label,
    path_diagnostic_label,
)
from sorafs_evidence_validation import (  # noqa: E402
    forbidden_non_production_markers,
    require_rollout_deployment_id,
    require_rollout_environment,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)


CANARY_KINDS = tuple(KIND_BY_NAME)
HEX64_LEN = 64
POLICY_DIGEST_KINDS = ("pricing_config", "governance_approval")
TRUE_CLAIMS: dict[str, tuple[str, ...]] = {
    "pricing_config": (
        "pricing_config_present",
        "settlement_config_present",
        "quote_ttl_present",
        "default_panel_size_present",
        "config_route_2xx",
        "status_route_2xx",
    ),
    "quote_api": (
        "deterministic_replay_passed",
        "deposit_bounds_enforced",
    ),
    "deposit_lifecycle": (
        "payer_auth_enforced",
        "participant_status_gate_enforced",
        "mismatched_escrow_rejected",
        "unconfirmed_ballot_rejected",
        "ledger_lock_confirmed",
        "idempotency_key_bound",
        "evidence_hashes_bound",
    ),
    "settlement_execution": (
        "drawdown_instruction_present",
        "cancel_instruction_present",
        "required_signer_bound",
        "deterministic_reconciliation_digest",
        "treasury_reconciliation_passed",
        "mismatched_ledger_rejected",
    ),
    "settlement_submitter": (
        "receipt_published",
        "required_authority_matched",
        "missing_signer_rejected",
        "wrong_authority_rejected",
        "rejected_or_expired_retry_verified",
    ),
    "moderation_worker": (
        "worker_enabled",
        "storage_configured",
        "submitter_keys_configured",
        "live_event_subscription_verified",
        "deposit_fingerprint_reconstructed",
        "evidence_hashes_verified",
        "runtime_ledger_validated",
        "pending_step_queued",
        "idempotent_rescan_verified",
        "retry_cap_enforced",
    ),
    "governance_dag_publication": (
        "publish_index_verified",
        "canonical_to_payloads_verified",
        "json_sidecars_verified",
        "blake3_sidecars_verified",
        "car_queue_verified",
        "runtime_signed_dag_verified",
        "report_publish_auth_enforced",
        "rollup_publish_auth_enforced",
    ),
    "dashboard_metrics": (
        "metrics_scrape_success",
        "dashboard_provisioned",
        "alert_rules_installed",
        "hosted_public_dashboard_verified",
    ),
    "multi_peer_reconciliation": (
        "deposit_posted",
        "decision_ingested",
        "settlement_submitted",
        "disbursement_verified",
        "treasury_reconciliation_passed",
        "governance_dag_receipt_verified",
        "all_peers_reconciled",
        "qc_quorum_satisfied",
    ),
    "governance_approval": (
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "pricing_policy_present",
        "settlement_policy_present",
        "deposit_custody_policy_present",
        "settlement_submitter_policy_present",
        "worker_retry_policy_present",
        "public_dashboard_rollout_accepted",
        "multi_peer_reconciliation_accepted",
    ),
}
FORCED_FALSE_FIELDS: dict[str, tuple[str, ...]] = {
    "pricing_config": ("config_payload_included", "response_bodies_included"),
    "quote_api": ("payloads_included", "response_bodies_included"),
    "deposit_lifecycle": (
        "raw_instruction_included",
        "deposit_payloads_included",
        "response_bodies_included",
    ),
    "settlement_execution": (
        "raw_instruction_included",
        "signed_transaction_included",
        "response_bodies_included",
    ),
    "settlement_submitter": ("raw_receipt_included", "signed_transaction_included"),
    "moderation_worker": (
        "raw_ballot_included",
        "deposit_confirmation_payload_included",
    ),
    "governance_dag_publication": (
        "raw_report_included",
        "raw_rollup_included",
        "raw_receipt_included",
    ),
    "dashboard_metrics": ("critical_alerts_firing", "response_bodies_included"),
    "multi_peer_reconciliation": (
        "raw_ledger_included",
    ),
    "governance_approval": (),
}


def split_csv_values(values: Sequence[str]) -> list[str]:
    """Split repeated comma-separated CLI values into exact strings."""

    items: list[str] = []
    for value in values:
        items.extend(value.split(","))
    return items


def validate_name_set(
    values: Iterable[str],
    *,
    allowed: Sequence[str],
    option: str,
    errors: list[str],
) -> list[str]:
    """Return allowed-order values, requiring complete known non-duplicate coverage."""

    values = tuple(values)
    allowed_set = frozenset(allowed)
    value_set = frozenset(values)
    if len(value_set) != len(values):
        errors.append(f"{option} must not contain duplicates")
    if any(name not in allowed_set for name in value_set):
        errors.append(f"{option} contains an unknown value")
    missing = [name for name in allowed if name not in value_set]
    if missing:
        errors.append(f"{option} must include every required value")
    return [name for name in allowed if name in value_set]


def validate_config_version_arg(value: str | None, *, errors: list[str]) -> None:
    """Require a reviewed lowercase appeal-finance config version label."""

    validate_canonical_string(value, label="--config-version", errors=errors)
    if not isinstance(value, str):
        return
    if CONFIG_VERSION_PATTERN.fullmatch(value) is None:
        errors.append(CONFIG_VERSION_ERROR.replace("config_version", "--config-version"))
        return
    name = value[: value.rfind("-v")]
    forbidden = forbidden_non_production_markers(name, FORBIDDEN_CONFIG_VERSION_MARKERS)
    if forbidden:
        errors.append(
            f"--config-version must not contain non-production markers {forbidden}"
        )


def validate_reviewed_inventory(
    values: Iterable[str],
    *,
    expected_count: int,
    option: str,
    kind: str,
    count_option: str,
    errors: list[str],
    pattern: re.Pattern[str] | None = None,
    label_error: str | None = None,
) -> list[str]:
    """Return reviewed unique inventory labels whose count matches a CLI count."""

    items = list(values)
    if not items:
        errors.append(f"{option} is required for {kind}")
    for index, item in enumerate(items):
        validate_canonical_string(item, label=f"{option}[{index}]", errors=errors)
        if pattern is None:
            continue
        if pattern.fullmatch(item) is None:
            if label_error is None:
                errors.append(f"{option} has malformed inventory label")
            else:
                errors.append(render_inventory_label_error(label_error, option))
            continue
        forbidden = forbidden_non_production_markers(item, FORBIDDEN_INVENTORY_LABEL_MARKERS)
        if forbidden:
            errors.append(f"{option} must not contain non-production markers {forbidden}")
    unique_items = set(items)
    if len(unique_items) != len(items):
        errors.append(f"{option} must not contain duplicates")
    if len(unique_items) != expected_count:
        errors.append(f"{option} unique values must match {count_option}")
    return items


def validate_optional_reviewed_inventory(
    values: Iterable[str],
    *,
    expected_count: int,
    option: str,
    kind: str,
    count_option: str,
    errors: list[str],
    pattern: re.Pattern[str] | None = None,
    label_error: str | None = None,
) -> list[str]:
    """Return reviewed inventory labels, allowing an empty zero-count inventory."""

    items = list(values)
    if expected_count == 0 and not items:
        return []
    return validate_reviewed_inventory(
        items,
        expected_count=expected_count,
        option=option,
        kind=kind,
        count_option=count_option,
        errors=errors,
        pattern=pattern,
        label_error=label_error,
    )


def render_inventory_label_error(label_error: str, option: str) -> str:
    """Render checker inventory-label diagnostics against a CLI option."""

    return (
        label_error.replace("deposit_probes[].name", option)
        .replace("signers[].name", option)
        .replace("steps[].name", option)
        .replace("ballots[].name", option)
        .replace("reports[].name", option)
        .replace("weekly_rollups[].name", option)
        .replace("settlement_receipts[].name", option)
        .replace("peers[].name", option)
        .replace("validators[].name", option)
        .replace("cases[].name", option)
    )


def validate_output_path(path: Path, errors: list[str]) -> None:
    """Reject unsafe output targets before writing a canary artifact."""

    if not isinstance(path, Path):
        errors.append(f"--out `{path_diagnostic_label(path)}` must be a path")
        return
    try:
        if path.is_symlink():
            errors.append(f"--out `{path_diagnostic_label(path)}` must not be a symlink")
            return
        if path.exists() and path.is_dir():
            errors.append(f"--out `{path_diagnostic_label(path)}` must not be a directory")
            return
    except (OSError, RuntimeError) as error:
        del error
        errors.append(f"--out `{path_diagnostic_label(path)}` cannot be inspected")
        return
    validate_checker_output_parent(path, errors, label="--out")


def validate_hex64(value: str | None, *, option: str, errors: list[str]) -> None:
    """Validate an exact lowercase 32-byte digest hex string."""

    if (
        not isinstance(value, str)
        or len(value) != HEX64_LEN
        or any(character not in "0123456789abcdef" for character in value)
    ):
        errors.append(f"{option} must be exact lowercase 32-byte hex")


def validate_canonical_string(value: str | None, *, label: str, errors: list[str]) -> None:
    """Require a non-empty canonical string without control/format text."""

    if not diagnostic_text_is_canonical(value):
        errors.append(f"{label} must be a non-empty canonical string")


def require_kind_options(
    args: argparse.Namespace,
    errors: list[str],
    required: Sequence[tuple[str, Any]],
) -> None:
    """Require kind-specific options by stable CLI flag."""

    for option, value in required:
        if value is None:
            errors.append(f"{option} is required for {args.kind}")


def build_common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by appeal finance canary payloads."""

    return {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
        "generated_at_unix": args.generated_at_unix,
        "config_digest_hex": args.config_digest_hex,
    }


def apply_verified_claims(payload: dict[str, Any], args: argparse.Namespace) -> None:
    """Populate explicitly verified true claims and forced payload-free false flags."""

    for claim in TRUE_CLAIMS[args.kind]:
        payload[claim] = claim in args.verified_claims
    for field in FORCED_FALSE_FIELDS[args.kind]:
        payload[field] = False


def build_route_records(
    args: argparse.Namespace,
    routes: Sequence[str],
    *,
    authz_enforced: bool,
) -> list[dict[str, Any]]:
    """Build payload-free finance route probe records."""

    return [
        {
            "name": route,
            "passed": True,
            "status_code": args.route_status_code,
            "body_blake3_hex": args.route_body_blake3_hex,
            "authz_enforced": authz_enforced,
            "signature_verified": True,
            "latency_ms": args.route_latency_ms,
        }
        for route in routes
    ]


def build_inventory_records(names: Sequence[str]) -> list[dict[str, str]]:
    """Build reviewed payload-free inventory records."""

    return [{"name": name} for name in names]


def build_deposit_probe_records(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build reviewed payload-free deposit probe records."""

    return [
        {"name": name, "confirmed": True}
        for name in args.confirmed_deposit_probes
    ] + [
        {"name": name, "confirmed": False}
        for name in args.unconfirmed_deposit_probes
    ]


def build_submitter_step_records(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build reviewed payload-free submitter step records."""

    return [
        {"name": name, "submitted": True}
        for name in args.submitted_steps
    ] + [
        {"name": name, "submitted": False}
        for name in args.queued_only_steps
    ]


def build_reconciliation_case_records(names: Sequence[str]) -> list[dict[str, Any]]:
    """Build reviewed payload-free reconciliation case records."""

    return [{"name": name, "reconciled": True} for name in names]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free appeal finance rollout canary payload."""

    payload = build_common_payload(args)
    apply_verified_claims(payload, args)
    if args.kind == "pricing_config":
        payload.update(
            {
                "config_version": args.config_version,
                "config_source": "iroha_config",
                "policy_digest_hex": args.policy_digest_hex,
                "class_count": args.class_count,
                "classes": args.appeal_classes,
            }
        )
    elif args.kind == "quote_api":
        routes = build_route_records(args, args.quote_routes, authz_enforced=False)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
                "quote_count": args.quote_count,
                "passed_quote_count": args.quote_count,
                "classes": args.appeal_classes,
                "urgencies": args.urgencies,
                "max_route_latency_ms": args.max_route_latency_ms,
            }
        )
    elif args.kind == "deposit_lifecycle":
        routes = build_route_records(args, args.deposit_routes, authz_enforced=True)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
                "deposit_probe_count": args.deposit_probe_count,
                "deposit_probes": build_deposit_probe_records(args),
                "confirmed_deposit_count": args.confirmed_deposit_count,
                "max_route_latency_ms": args.max_route_latency_ms,
            }
        )
    elif args.kind == "settlement_execution":
        routes = build_route_records(args, args.settlement_routes, authz_enforced=True)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
                "settlement_probe_count": args.settlement_probe_count,
                "instruction_step_count": args.instruction_step_count,
                "instruction_steps": args.instruction_steps,
                "outcomes": args.outcomes,
                "reconciliation_statuses": args.reconciliation_statuses,
            }
        )
    elif args.kind == "settlement_submitter":
        payload.update(
            {
                "configured_signer_count": args.configured_signer_count,
                "signers": build_inventory_records(args.signers),
                "queued_step_count": args.queued_step_count,
                "steps": build_submitter_step_records(args),
                "submitted_step_count": args.submitted_step_count,
                "max_settlement_lag_seconds": args.max_settlement_lag_seconds,
            }
        )
    elif args.kind == "moderation_worker":
        payload.update(
            {
                "ballot_replay_count": args.ballot_replay_count,
                "ballots": build_inventory_records(args.replayed_ballots),
                "max_settlement_lag_seconds": args.max_settlement_lag_seconds,
            }
        )
    elif args.kind == "governance_dag_publication":
        payload.update(
            {
                "report_count": args.report_count,
                "reports": build_inventory_records(args.reports),
                "weekly_rollup_count": args.weekly_rollup_count,
                "weekly_rollups": build_inventory_records(args.weekly_rollups),
                "settlement_receipt_count": args.settlement_receipt_count,
                "settlement_receipts": build_inventory_records(
                    args.settlement_receipts
                ),
                "payload_kind_count": len(args.payload_kinds),
                "payload_kinds": args.payload_kinds,
            }
        )
    elif args.kind == "dashboard_metrics":
        payload.update(
            {
                "metrics": args.metrics,
                "metric_count": len(args.metrics),
                "payload_kind_count": len(args.payload_kinds),
                "payload_kinds": args.payload_kinds,
            }
        )
    elif args.kind == "multi_peer_reconciliation":
        payload.update(
            {
                "peer_count": args.peer_count,
                "peers": build_inventory_records(args.peers),
                "validator_count": args.validator_count,
                "validators": build_inventory_records(args.validators),
                "case_count": args.case_count,
                "cases": build_reconciliation_case_records(args.reconciliation_cases),
                "mismatch_count": 0,
                "unexpected_failure_count": 0,
            }
        )
    elif args.kind == "governance_approval":
        payload.update(
            {
                "config_source": "iroha_config",
                "policy_digest_hex": args.policy_digest_hex,
            }
        )
    return payload


def validate_route_thresholds(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate route and settlement thresholds before payload construction."""

    if args.route_latency_ms > DEFAULT_MAX_ROUTE_LATENCY_MS:
        errors.append(f"--route-latency-ms must be <= {DEFAULT_MAX_ROUTE_LATENCY_MS}")
    if (
        args.max_route_latency_ms is not None
        and args.max_route_latency_ms > DEFAULT_MAX_ROUTE_LATENCY_MS
    ):
        errors.append(f"--max-route-latency-ms must be <= {DEFAULT_MAX_ROUTE_LATENCY_MS}")
    if (
        args.max_settlement_lag_seconds is not None
        and args.max_settlement_lag_seconds > DEFAULT_MAX_SETTLEMENT_LAG_SECS
    ):
        errors.append(
            f"--max-settlement-lag-seconds must be <= {DEFAULT_MAX_SETTLEMENT_LAG_SECS}"
        )


def validate_kind_inputs(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate kind-specific reviewed operator inputs."""

    args.verified_claims = validate_name_set(
        split_csv_values(args.verified_claim),
        allowed=TRUE_CLAIMS[args.kind],
        option="--verified-claim",
        errors=errors,
    )
    if args.kind == "pricing_config":
        require_kind_options(
            args,
            errors,
            (
                ("--config-version", args.config_version),
                ("--class-count", args.class_count),
            ),
        )
        args.appeal_classes = validate_name_set(
            split_csv_values(args.appeal_class),
            allowed=REQUIRED_APPEAL_CLASSES,
            option="--appeal-class",
            errors=errors,
        )
        if (
            args.class_count is not None
            and args.appeal_classes
            and args.class_count != len(args.appeal_classes)
        ):
            errors.append("--class-count must match --appeal-class inventory")
        validate_config_version_arg(args.config_version, errors=errors)
    elif args.kind == "quote_api":
        require_kind_options(
            args,
            errors,
            (
                ("--quote-count", args.quote_count),
                ("--max-route-latency-ms", args.max_route_latency_ms),
            ),
        )
        args.quote_routes = validate_name_set(
            split_csv_values(args.quote_route),
            allowed=REQUIRED_QUOTE_ROUTES,
            option="--quote-route",
            errors=errors,
        )
        validate_hex64(
            args.route_body_blake3_hex,
            option="--route-body-blake3-hex",
            errors=errors,
        )
        args.appeal_classes = validate_name_set(
            split_csv_values(args.appeal_class),
            allowed=REQUIRED_APPEAL_CLASSES,
            option="--appeal-class",
            errors=errors,
        )
        args.urgencies = validate_name_set(
            split_csv_values(args.urgency),
            allowed=REQUIRED_URGENCIES,
            option="--urgency",
            errors=errors,
        )
        if (
            args.quote_count is not None
            and args.quote_count != REQUIRED_QUOTE_API_QUOTES
        ):
            errors.append("--quote-count must match required class/urgency product")
    elif args.kind == "deposit_lifecycle":
        require_kind_options(
            args,
            errors,
            (
                ("--deposit-probe-count", args.deposit_probe_count),
                ("--confirmed-deposit-count", args.confirmed_deposit_count),
                ("--max-route-latency-ms", args.max_route_latency_ms),
            ),
        )
        args.deposit_routes = validate_name_set(
            split_csv_values(args.deposit_route),
            allowed=REQUIRED_DEPOSIT_ROUTES,
            option="--deposit-route",
            errors=errors,
        )
        validate_hex64(
            args.route_body_blake3_hex,
            option="--route-body-blake3-hex",
            errors=errors,
        )
        args.confirmed_deposit_probes = validate_reviewed_inventory(
            split_csv_values(args.confirmed_deposit_probe),
            expected_count=args.confirmed_deposit_count or 0,
            option="--confirmed-deposit-probe",
            kind="deposit_lifecycle",
            count_option="--confirmed-deposit-count",
            errors=errors,
            pattern=DEPOSIT_PROBE_LABEL_PATTERN,
            label_error=DEPOSIT_PROBE_LABEL_ERROR,
        )
        unconfirmed_probe_count = 0
        if args.deposit_probe_count is not None and args.confirmed_deposit_count is not None:
            if args.confirmed_deposit_count > args.deposit_probe_count:
                errors.append(
                    "--confirmed-deposit-count must be <= --deposit-probe-count"
                )
            else:
                unconfirmed_probe_count = (
                    args.deposit_probe_count - args.confirmed_deposit_count
                )
        args.unconfirmed_deposit_probes = validate_optional_reviewed_inventory(
            split_csv_values(args.unconfirmed_deposit_probe),
            expected_count=unconfirmed_probe_count,
            option="--unconfirmed-deposit-probe",
            kind="deposit_lifecycle",
            count_option="--deposit-probe-count",
            errors=errors,
            pattern=DEPOSIT_PROBE_LABEL_PATTERN,
            label_error=DEPOSIT_PROBE_LABEL_ERROR,
        )
        deposit_probe_names = (
            args.confirmed_deposit_probes + args.unconfirmed_deposit_probes
        )
        if len(set(deposit_probe_names)) != len(deposit_probe_names):
            errors.append(
                "--confirmed-deposit-probe and --unconfirmed-deposit-probe "
                "must not overlap"
            )
    elif args.kind == "settlement_execution":
        require_kind_options(
            args,
            errors,
            (
                ("--settlement-probe-count", args.settlement_probe_count),
                ("--instruction-step-count", args.instruction_step_count),
            ),
        )
        args.settlement_routes = validate_name_set(
            split_csv_values(args.settlement_route),
            allowed=REQUIRED_SETTLEMENT_ROUTES,
            option="--settlement-route",
            errors=errors,
        )
        validate_hex64(
            args.route_body_blake3_hex,
            option="--route-body-blake3-hex",
            errors=errors,
        )
        args.outcomes = validate_name_set(
            split_csv_values(args.outcome),
            allowed=REQUIRED_OUTCOMES,
            option="--outcome",
            errors=errors,
        )
        args.instruction_steps = validate_name_set(
            split_csv_values(args.instruction_step),
            allowed=REQUIRED_SETTLEMENT_INSTRUCTION_STEPS,
            option="--instruction-step",
            errors=errors,
        )
        args.reconciliation_statuses = validate_name_set(
            split_csv_values(args.reconciliation_status),
            allowed=REQUIRED_RECONCILIATION_STATUSES,
            option="--reconciliation-status",
            errors=errors,
        )
    elif args.kind == "settlement_submitter":
        require_kind_options(
            args,
            errors,
            (
                ("--configured-signer-count", args.configured_signer_count),
                ("--queued-step-count", args.queued_step_count),
                ("--submitted-step-count", args.submitted_step_count),
                ("--max-settlement-lag-seconds", args.max_settlement_lag_seconds),
            ),
        )
        args.signers = validate_reviewed_inventory(
            split_csv_values(args.signer),
            expected_count=args.configured_signer_count or 0,
            option="--signer",
            kind="settlement_submitter",
            count_option="--configured-signer-count",
            errors=errors,
            pattern=SUBMITTER_SIGNER_LABEL_PATTERN,
            label_error=SUBMITTER_SIGNER_LABEL_ERROR,
        )
        args.submitted_steps = validate_reviewed_inventory(
            split_csv_values(args.submitted_step),
            expected_count=args.submitted_step_count or 0,
            option="--submitted-step",
            kind="settlement_submitter",
            count_option="--submitted-step-count",
            errors=errors,
            pattern=SUBMITTER_STEP_LABEL_PATTERN,
            label_error=SUBMITTER_STEP_LABEL_ERROR,
        )
        queued_only_step_count = 0
        if args.queued_step_count is not None and args.submitted_step_count is not None:
            if args.submitted_step_count > args.queued_step_count:
                errors.append("--submitted-step-count must be <= --queued-step-count")
            else:
                queued_only_step_count = args.queued_step_count - args.submitted_step_count
        args.queued_only_steps = validate_optional_reviewed_inventory(
            split_csv_values(args.queued_only_step),
            expected_count=queued_only_step_count,
            option="--queued-only-step",
            kind="settlement_submitter",
            count_option="--queued-step-count",
            errors=errors,
            pattern=SUBMITTER_STEP_LABEL_PATTERN,
            label_error=SUBMITTER_STEP_LABEL_ERROR,
        )
        step_names = args.submitted_steps + args.queued_only_steps
        if len(set(step_names)) != len(step_names):
            errors.append("--submitted-step and --queued-only-step must not overlap")
    elif args.kind == "moderation_worker":
        require_kind_options(
            args,
            errors,
            (
                ("--ballot-replay-count", args.ballot_replay_count),
                ("--max-settlement-lag-seconds", args.max_settlement_lag_seconds),
            ),
        )
        args.replayed_ballots = validate_reviewed_inventory(
            split_csv_values(args.replayed_ballot),
            expected_count=args.ballot_replay_count or 0,
            option="--replayed-ballot",
            kind="moderation_worker",
            count_option="--ballot-replay-count",
            errors=errors,
            pattern=WORKER_BALLOT_LABEL_PATTERN,
            label_error=WORKER_BALLOT_LABEL_ERROR,
        )
    elif args.kind == "governance_dag_publication":
        require_kind_options(
            args,
            errors,
            (
                ("--report-count", args.report_count),
                ("--weekly-rollup-count", args.weekly_rollup_count),
                ("--settlement-receipt-count", args.settlement_receipt_count),
            ),
        )
        args.reports = validate_reviewed_inventory(
            split_csv_values(args.report),
            expected_count=args.report_count or 0,
            option="--report",
            kind="governance_dag_publication",
            count_option="--report-count",
            errors=errors,
            pattern=GOVERNANCE_REPORT_LABEL_PATTERN,
            label_error=GOVERNANCE_REPORT_LABEL_ERROR,
        )
        args.weekly_rollups = validate_reviewed_inventory(
            split_csv_values(args.weekly_rollup),
            expected_count=args.weekly_rollup_count or 0,
            option="--weekly-rollup",
            kind="governance_dag_publication",
            count_option="--weekly-rollup-count",
            errors=errors,
            pattern=GOVERNANCE_WEEKLY_ROLLUP_LABEL_PATTERN,
            label_error=GOVERNANCE_WEEKLY_ROLLUP_LABEL_ERROR,
        )
        args.settlement_receipts = validate_reviewed_inventory(
            split_csv_values(args.settlement_receipt),
            expected_count=args.settlement_receipt_count or 0,
            option="--settlement-receipt",
            kind="governance_dag_publication",
            count_option="--settlement-receipt-count",
            errors=errors,
            pattern=GOVERNANCE_SETTLEMENT_RECEIPT_LABEL_PATTERN,
            label_error=GOVERNANCE_SETTLEMENT_RECEIPT_LABEL_ERROR,
        )
        args.payload_kinds = validate_name_set(
            split_csv_values(args.payload_kind),
            allowed=REQUIRED_PAYLOAD_KINDS,
            option="--payload-kind",
            errors=errors,
        )
    elif args.kind == "dashboard_metrics":
        args.metrics = validate_name_set(
            split_csv_values(args.metric),
            allowed=REQUIRED_METRICS,
            option="--metric",
            errors=errors,
        )
        args.payload_kinds = validate_name_set(
            split_csv_values(args.payload_kind),
            allowed=REQUIRED_PAYLOAD_KINDS,
            option="--payload-kind",
            errors=errors,
        )
    elif args.kind == "multi_peer_reconciliation":
        require_kind_options(
            args,
            errors,
            (
                ("--peer-count", args.peer_count),
                ("--validator-count", args.validator_count),
                ("--case-count", args.case_count),
            ),
        )
        if args.peer_count is not None and args.peer_count < DEFAULT_MIN_PEERS:
            errors.append(f"--peer-count must be >= {DEFAULT_MIN_PEERS}")
        if args.validator_count is not None and args.validator_count < DEFAULT_MIN_PEERS:
            errors.append(f"--validator-count must be >= {DEFAULT_MIN_PEERS}")
        args.peers = validate_reviewed_inventory(
            split_csv_values(args.peer),
            expected_count=args.peer_count or 0,
            option="--peer",
            kind="multi_peer_reconciliation",
            count_option="--peer-count",
            pattern=PEER_LABEL_PATTERN,
            label_error=PEER_LABEL_ERROR,
            errors=errors,
        )
        args.validators = validate_reviewed_inventory(
            split_csv_values(args.validator),
            expected_count=args.validator_count or 0,
            option="--validator",
            kind="multi_peer_reconciliation",
            count_option="--validator-count",
            pattern=VALIDATOR_LABEL_PATTERN,
            label_error=VALIDATOR_LABEL_ERROR,
            errors=errors,
        )
        args.reconciliation_cases = validate_reviewed_inventory(
            split_csv_values(args.reconciliation_case),
            expected_count=args.case_count or 0,
            option="--reconciliation-case",
            kind="multi_peer_reconciliation",
            count_option="--case-count",
            pattern=RECONCILIATION_CASE_LABEL_PATTERN,
            label_error=RECONCILIATION_CASE_LABEL_ERROR,
            errors=errors,
        )
    if args.kind in POLICY_DIGEST_KINDS:
        require_kind_options(
            args,
            errors,
            (("--policy-digest-hex", args.policy_digest_hex),),
        )
        validate_hex64(args.policy_digest_hex, option="--policy-digest-hex", errors=errors)


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    require_rollout_deployment_id(
        {"--deployment-id": args.deployment_id},
        errors,
        field="--deployment-id",
    )
    require_rollout_environment(
        {"--environment": args.environment},
        errors,
        field="--environment",
    )
    validate_hex64(args.config_digest_hex, option="--config-digest-hex", errors=errors)
    validate_route_thresholds(args, errors)
    validate_kind_inputs(args, errors)
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix,
        max_canary_age_secs=DEFAULT_MAX_CANARY_AGE_SECS,
        max_dashboard_age_secs=DEFAULT_MAX_DASHBOARD_AGE_SECS,
        max_route_latency_ms=DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_settlement_lag_secs=DEFAULT_MAX_SETTLEMENT_LAG_SECS,
        min_peers=DEFAULT_MIN_PEERS,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the appeal finance gate contract."""

    kind, errors = validate_evidence_payload(payload, validation_options(args))
    if kind != args.kind:
        errors.append(f"generated canary must validate as {args.kind}")
    return errors


def write_payload_atomic(path: Path, payload: dict[str, Any]) -> list[str]:
    """Write the canary JSON atomically without following output symlinks."""

    text = json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
    parent = path.parent
    try:
        parent.mkdir(parents=True, exist_ok=True)
    except (OSError, RuntimeError) as error:
        parent_label = path_diagnostic_label(parent)
        return [
            f"--out parent `{parent_label}` cannot be created: "
            f"{error_diagnostic_label(error, path_label=parent_label)}"
        ]
    tmp_name = f".{path.name}.{os.getpid()}.{secrets.token_hex(8)}.tmp"
    tmp_path = parent / tmp_name
    fd = -1
    try:
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        nofollow = getattr(os, "O_NOFOLLOW", 0)
        if nofollow:
            flags |= nofollow
        fd = os.open(tmp_path, flags, 0o600)
        write_all_checker_summary_bytes(fd, text.encode("utf-8"))
        os.fsync(fd)
        os.close(fd)
        fd = -1
        os.replace(tmp_path, path)
        parent_sync_errors = fsync_checker_output_parent(path, label="--out")
        if parent_sync_errors:
            return parent_sync_errors
    except (OSError, RuntimeError) as error:
        path_label = path_diagnostic_label(path)
        try:
            if fd >= 0:
                os.close(fd)
        finally:
            try:
                tmp_path.unlink()
            except FileNotFoundError:
                pass
            except (OSError, RuntimeError):
                pass
        return [
            f"--out `{path_label}` cannot be written: "
            f"{error_diagnostic_label(error, path_label=path_label)}"
        ]
    return []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Build payload-free SoraFS SFM-4b2 appeal finance canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg, required=True)
    parser.add_argument("--config-digest-hex", required=True)
    parser.add_argument("--verified-claim", action="append", default=[])
    parser.add_argument("--config-version")
    parser.add_argument("--class-count", type=positive_int_arg)
    parser.add_argument("--quote-route", action="append", default=[])
    parser.add_argument("--appeal-class", action="append", default=[])
    parser.add_argument("--urgency", action="append", default=[])
    parser.add_argument("--quote-count", type=positive_int_arg)
    parser.add_argument("--deposit-route", action="append", default=[])
    parser.add_argument("--deposit-probe-count", type=positive_int_arg)
    parser.add_argument("--confirmed-deposit-probe", action="append", default=[])
    parser.add_argument("--unconfirmed-deposit-probe", action="append", default=[])
    parser.add_argument("--confirmed-deposit-count", type=positive_int_arg)
    parser.add_argument("--settlement-route", action="append", default=[])
    parser.add_argument("--settlement-probe-count", type=positive_int_arg)
    parser.add_argument("--instruction-step-count", type=positive_int_arg)
    parser.add_argument("--instruction-step", action="append", default=[])
    parser.add_argument("--outcome", action="append", default=[])
    parser.add_argument("--reconciliation-status", action="append", default=[])
    parser.add_argument("--route-status-code", type=positive_int_arg, default=200)
    parser.add_argument("--route-latency-ms", type=non_negative_int_arg, default=30)
    parser.add_argument("--route-body-blake3-hex")
    parser.add_argument("--max-route-latency-ms", type=positive_int_arg)
    parser.add_argument("--configured-signer-count", type=positive_int_arg)
    parser.add_argument("--signer", action="append", default=[])
    parser.add_argument("--queued-step-count", type=positive_int_arg)
    parser.add_argument("--submitted-step", action="append", default=[])
    parser.add_argument("--queued-only-step", action="append", default=[])
    parser.add_argument("--submitted-step-count", type=positive_int_arg)
    parser.add_argument("--max-settlement-lag-seconds", type=non_negative_int_arg)
    parser.add_argument("--ballot-replay-count", type=positive_int_arg)
    parser.add_argument("--replayed-ballot", action="append", default=[])
    parser.add_argument("--report-count", type=positive_int_arg)
    parser.add_argument("--report", action="append", default=[])
    parser.add_argument("--weekly-rollup-count", type=positive_int_arg)
    parser.add_argument("--weekly-rollup", action="append", default=[])
    parser.add_argument("--settlement-receipt-count", type=positive_int_arg)
    parser.add_argument("--settlement-receipt", action="append", default=[])
    parser.add_argument("--payload-kind", action="append", default=[])
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--peer-count", type=positive_int_arg)
    parser.add_argument("--peer", action="append", default=[])
    parser.add_argument("--validator-count", type=positive_int_arg)
    parser.add_argument("--validator", action="append", default=[])
    parser.add_argument("--case-count", type=positive_int_arg)
    parser.add_argument("--reconciliation-case", action="append", default=[])
    parser.add_argument("--policy-digest-hex")
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
        return parser.parse_args(expanded_args)
    except ValueError as error:
        emit_checker_exception(error)
        raise SystemExit(2) from error


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    errors = validate_inputs(args)
    if errors:
        emit_checker_error_block(
            "ERROR: SoraFS appeal finance canary inputs are incomplete:",
            errors,
        )
        return 2

    payload = build_payload(args)
    payload_errors = validate_generated_payload(payload, args)
    if payload_errors:
        emit_checker_error_lines(payload_errors)
        return 2

    write_errors = write_payload_atomic(args.out, payload)
    if write_errors:
        emit_checker_error_lines(write_errors)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
