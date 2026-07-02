#!/usr/bin/env python3
"""Build payload-free SoraFS reserve/rent rollout canary artifacts."""

from __future__ import annotations

import argparse
import json
import os
import secrets
import sys
from collections.abc import Iterable, Sequence
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_reserve_rent_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_BAKE_AGE_SECS,
    DEFAULT_MAX_LEDGER_AGE_SECS,
    DEFAULT_MAX_LIFECYCLE_LAG_SECS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    KIND_BY_NAME,
    REQUIRED_DURATIONS,
    REQUIRED_LIFECYCLE_ROUTES,
    REQUIRED_METRICS,
    REQUIRED_SIGNED_ROUTES,
    REQUIRED_STORAGE_CLASSES,
    REQUIRED_TIERS,
    ValidationOptions,
    validate_evidence_payload,
)
from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_exception,
    validate_checker_output_parent,
)
from sorafs_path_identity import path_diagnostic_label  # noqa: E402
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)


CANARY_KINDS = tuple(KIND_BY_NAME)
MATRIX_DIGEST_KINDS = tuple(kind for kind in CANARY_KINDS if kind != "policy_config")
LEDGER_DIGEST_KINDS = tuple(
    kind for kind in CANARY_KINDS if kind not in ("policy_config", "quote_matrix")
)
HEX64_LEN = 64
TRUE_CLAIMS: dict[str, tuple[str, ...]] = {
    "policy_config": (
        "governance_approved",
        "credit_line_caps_present",
        "apr_policy_present",
    ),
    "quote_matrix": (),
    "ledger_digest": (
        "rent_transfer_present",
        "reserve_top_up_transfer_present",
        "instruction_hashes_verified",
        "ledger_projection_verified",
    ),
    "lifecycle_service": (
        "stage_transition_replay_passed",
        "governance_event_emitted",
        "manual_override_audited",
    ),
    "signed_routes": (
        "replay_attack_rejected",
        "unsigned_request_rejected",
        "wrong_account_rejected",
    ),
    "reserve_movement": (
        "rent_settlement_present",
        "reserve_top_up_present",
        "withdrawal_limits_enforced",
        "treasury_reconciliation_passed",
        "double_spend_rejected",
        "live_chain_submission_verified",
        "submitted_transaction_hash_readback_verified",
        "automatic_finality_polling_verified",
        "finality_poll_confirmed_status_verified",
        "finality_poll_timeout_rejected",
        "custody_status_route_present",
        "submitted_custody_evidence_present",
        "confirmed_custody_evidence_present",
        "rejected_custody_reconciliation_passed",
        "confirmed_balance_projection_verified",
        "confirmed_withdrawal_underflow_rejected",
        "chain_reconciled_readback_verified",
    ),
    "credit_line": (
        "credit_draw_cap_enforced",
        "apr_accrual_verified",
        "manual_approval_tier_blocked",
        "credit_shortfall_reported",
        "live_account_mutation_verified",
        "credit_line_account_state_readback_verified",
        "credit_accrual_posted_to_account_state",
        "manual_approval_tier_did_not_mutate_account",
        "account_state_reconciliation_verified",
        "no_negative_balance",
    ),
    "appeal_policy": (
        "appeal_route_present",
        "policy_update_route_present",
        "governance_recorded",
        "operator_role_enforced",
        "unauthorized_appeal_rejected",
        "policy_digest_bound",
    ),
    "metrics_alerts": (
        "metrics_scrape_success",
        "dashboard_provisioned",
        "alert_rules_installed",
    ),
    "provider_bake": (
        "scheduler_config_bound",
        "scheduled_lifecycle_canary_passed",
        "scheduled_lifecycle_canary_gateway_sync_verified",
        "scheduled_lifecycle_canary_orderbook_rejection_verified",
        "governance_packet_attached",
        "ledger_digest_attached",
        "dashboard_snapshot_attached",
    ),
    "governance_approval": (
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "reserve_movement_policy_present",
        "credit_line_policy_present",
        "appeal_policy_present",
        "manual_override_policy_present",
        "provider_bake_accepted",
        "governance_source_entries_published",
        "downstream_compliance_policy_applied",
        "non_reserve_compliance_entries_preserved",
        "governance_source_entry_handoff_verified",
        "denylist_and_policy_consumers_consistent",
    ),
}
FORCED_FALSE_FIELDS: dict[str, tuple[str, ...]] = {
    "policy_config": ("policy_payload_included",),
    "quote_matrix": ("quote_payloads_included",),
    "ledger_digest": ("raw_ledger_included", "raw_transfer_instructions_included"),
    "lifecycle_service": ("response_bodies_included",),
    "signed_routes": ("response_bodies_included",),
    "reserve_movement": ("raw_transfer_included", "raw_instruction_included"),
    "credit_line": ("raw_ledger_included",),
    "appeal_policy": ("appeal_payloads_included",),
    "metrics_alerts": ("critical_alerts_firing", "response_bodies_included"),
    "provider_bake": ("payloads_included",),
    "governance_approval": (),
}


def split_csv_values(values: Sequence[str]) -> list[str]:
    """Split repeated comma-separated CLI values into canonical strings."""

    items: list[str] = []
    for value in values:
        for item in value.split(","):
            stripped = item.strip()
            if stripped:
                items.append(stripped)
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
    """Require a non-empty canonical string without control characters."""

    if (
        not isinstance(value, str)
        or not value.strip()
        or value != value.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
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
    """Build fields shared by reserve/rent canary payloads."""

    payload: dict[str, Any] = {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
        "generated_at_unix": args.generated_at_unix,
        "policy_digest_hex": args.policy_digest_hex,
    }
    if args.kind in MATRIX_DIGEST_KINDS:
        payload["matrix_digest_hex"] = args.matrix_digest_hex
    if args.kind in LEDGER_DIGEST_KINDS:
        payload["ledger_digest_hex"] = args.ledger_digest_hex
    return payload


def apply_verified_claims(payload: dict[str, Any], args: argparse.Namespace) -> None:
    """Populate explicitly verified true claims and forced payload-free false flags."""

    for claim in TRUE_CLAIMS[args.kind]:
        payload[claim] = claim in args.verified_claims
    for field in FORCED_FALSE_FIELDS[args.kind]:
        payload[field] = False


def build_route_records(args: argparse.Namespace, routes: Sequence[str]) -> list[dict[str, Any]]:
    """Build payload-free reserve route probe records."""

    return [
        {
            "name": route,
            "passed": True,
            "status_code": args.route_status_code,
            "authz_enforced": True,
            "signature_verified": True,
            "latency_ms": args.route_latency_ms,
        }
        for route in routes
    ]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free reserve/rent rollout canary payload."""

    payload = build_common_payload(args)
    apply_verified_claims(payload, args)
    if args.kind == "policy_config":
        payload.update(
            {
                "policy_version": args.policy_version,
                "config_source": "iroha_config",
                "tier_count": args.tier_count,
                "storage_class_count": args.storage_class_count,
                "duration_count": args.duration_count,
            }
        )
    elif args.kind == "quote_matrix":
        payload.update(
            {
                "scenario_count": args.scenario_count,
                "passed_scenario_count": args.scenario_count,
                "storage_classes": args.storage_classes,
                "tiers": args.tiers,
                "durations": args.durations,
            }
        )
    elif args.kind == "ledger_digest":
        payload.update(
            {
                "ledger_count": args.ledger_count,
                "instruction_count": args.instruction_count,
            }
        )
    elif args.kind == "lifecycle_service":
        routes = build_route_records(args, args.lifecycle_routes)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
                "max_lifecycle_lag_seconds": args.max_lifecycle_lag_seconds,
                "persisted_stage_count": args.persisted_stage_count,
            }
        )
    elif args.kind == "signed_routes":
        routes = build_route_records(args, args.signed_routes)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
                "max_route_latency_ms": args.max_route_latency_ms,
            }
        )
    elif args.kind == "reserve_movement":
        payload.update(
            {
                "movement_count": args.movement_count,
                "accepted_movement_count": args.movement_count,
                "failed_movement_count": 0,
                "unexpected_failure_count": 0,
                "chain_submission_count": args.chain_submission_count,
                "finality_poll_attempt_count": args.finality_poll_attempt_count,
            }
        )
    elif args.kind == "credit_line":
        payload.update(
            {
                "credit_line_mutation_count": args.credit_line_mutation_count,
                "accrual_cycle_count": args.accrual_cycle_count,
                "unexpected_failure_count": 0,
            }
        )
    elif args.kind == "appeal_policy":
        payload.update(
            {
                "appeal_probe_count": args.appeal_probe_count,
                "approved_appeal_count": args.approved_appeal_count,
                "rejected_appeal_count": args.rejected_appeal_count,
            }
        )
    elif args.kind == "metrics_alerts":
        payload["metrics"] = args.metrics
    elif args.kind == "provider_bake":
        payload.update(
            {
                "bake_id": args.bake_id,
                "started_at_unix": args.started_at_unix,
                "completed_at_unix": args.completed_at_unix,
                "provider_count": args.provider_count,
                "completed_provider_count": args.provider_count,
                "failure_count": 0,
                "rent_cycle_count": args.rent_cycle_count,
                "top_up_cycle_count": args.top_up_cycle_count,
                "appeal_cycle_count": args.appeal_cycle_count,
                "scheduled_lifecycle_canary_last_tick_unix": (
                    args.scheduled_lifecycle_canary_last_tick_unix
                ),
                "scheduled_lifecycle_canary_tick_count": (
                    args.scheduled_lifecycle_canary_tick_count
                ),
                "scheduled_lifecycle_canary_defaulted_provider_count": (
                    args.scheduled_lifecycle_canary_defaulted_provider_count
                ),
            }
        )
    elif args.kind == "governance_approval":
        payload.update(
            {
                "downstream_compliance_consumer_count": (
                    args.downstream_compliance_consumer_count
                ),
                "config_source": "iroha_config",
            }
        )
    return payload


def validate_kind_inputs(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate kind-specific reviewed operator inputs."""

    args.verified_claims = validate_name_set(
        split_csv_values(args.verified_claim),
        allowed=TRUE_CLAIMS[args.kind],
        option="--verified-claim",
        errors=errors,
    )
    if args.kind in MATRIX_DIGEST_KINDS:
        validate_hex64(args.matrix_digest_hex, option="--matrix-digest-hex", errors=errors)
    if args.kind in LEDGER_DIGEST_KINDS:
        validate_hex64(args.ledger_digest_hex, option="--ledger-digest-hex", errors=errors)
    if args.kind == "policy_config":
        require_kind_options(
            args,
            errors,
            (
                ("--policy-version", args.policy_version),
                ("--tier-count", args.tier_count),
                ("--storage-class-count", args.storage_class_count),
                ("--duration-count", args.duration_count),
            ),
        )
    elif args.kind == "quote_matrix":
        require_kind_options(args, errors, (("--scenario-count", args.scenario_count),))
        args.storage_classes = validate_name_set(
            split_csv_values(args.storage_class),
            allowed=REQUIRED_STORAGE_CLASSES,
            option="--storage-class",
            errors=errors,
        )
        args.tiers = validate_name_set(
            split_csv_values(args.tier),
            allowed=REQUIRED_TIERS,
            option="--tier",
            errors=errors,
        )
        args.durations = validate_name_set(
            split_csv_values(args.duration),
            allowed=REQUIRED_DURATIONS,
            option="--duration",
            errors=errors,
        )
    elif args.kind == "ledger_digest":
        require_kind_options(
            args,
            errors,
            (
                ("--ledger-count", args.ledger_count),
                ("--instruction-count", args.instruction_count),
            ),
        )
    elif args.kind == "lifecycle_service":
        require_kind_options(
            args,
            errors,
            (
                ("--max-lifecycle-lag-seconds", args.max_lifecycle_lag_seconds),
                ("--persisted-stage-count", args.persisted_stage_count),
            ),
        )
        args.lifecycle_routes = validate_name_set(
            split_csv_values(args.lifecycle_route),
            allowed=REQUIRED_LIFECYCLE_ROUTES,
            option="--lifecycle-route",
            errors=errors,
        )
    elif args.kind == "signed_routes":
        require_kind_options(
            args,
            errors,
            (("--max-route-latency-ms", args.max_route_latency_ms),),
        )
        args.signed_routes = validate_name_set(
            split_csv_values(args.signed_route),
            allowed=REQUIRED_SIGNED_ROUTES,
            option="--signed-route",
            errors=errors,
        )
    elif args.kind == "reserve_movement":
        require_kind_options(
            args,
            errors,
            (
                ("--movement-count", args.movement_count),
                ("--chain-submission-count", args.chain_submission_count),
                ("--finality-poll-attempt-count", args.finality_poll_attempt_count),
            ),
        )
    elif args.kind == "credit_line":
        require_kind_options(
            args,
            errors,
            (
                ("--credit-line-mutation-count", args.credit_line_mutation_count),
                ("--accrual-cycle-count", args.accrual_cycle_count),
            ),
        )
    elif args.kind == "appeal_policy":
        require_kind_options(
            args,
            errors,
            (
                ("--appeal-probe-count", args.appeal_probe_count),
                ("--approved-appeal-count", args.approved_appeal_count),
                ("--rejected-appeal-count", args.rejected_appeal_count),
            ),
        )
    elif args.kind == "metrics_alerts":
        args.metrics = validate_name_set(
            split_csv_values(args.metric),
            allowed=REQUIRED_METRICS,
            option="--metric",
            errors=errors,
        )
    elif args.kind == "provider_bake":
        require_kind_options(
            args,
            errors,
            (
                ("--bake-id", args.bake_id),
                ("--started-at-unix", args.started_at_unix),
                ("--completed-at-unix", args.completed_at_unix),
                ("--provider-count", args.provider_count),
                ("--rent-cycle-count", args.rent_cycle_count),
                ("--top-up-cycle-count", args.top_up_cycle_count),
                ("--appeal-cycle-count", args.appeal_cycle_count),
                (
                    "--scheduled-lifecycle-canary-last-tick-unix",
                    args.scheduled_lifecycle_canary_last_tick_unix,
                ),
                (
                    "--scheduled-lifecycle-canary-tick-count",
                    args.scheduled_lifecycle_canary_tick_count,
                ),
                (
                    "--scheduled-lifecycle-canary-defaulted-provider-count",
                    args.scheduled_lifecycle_canary_defaulted_provider_count,
                ),
            ),
        )
        validate_canonical_string(args.bake_id, label="--bake-id", errors=errors)
    elif args.kind == "governance_approval":
        require_kind_options(
            args,
            errors,
            (
                (
                    "--downstream-compliance-consumer-count",
                    args.downstream_compliance_consumer_count,
                ),
            ),
        )


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    validate_canonical_string(args.deployment_id, label="--deployment-id", errors=errors)
    validate_canonical_string(args.environment, label="--environment", errors=errors)
    validate_hex64(args.policy_digest_hex, option="--policy-digest-hex", errors=errors)
    if args.route_latency_ms > DEFAULT_MAX_ROUTE_LATENCY_MS:
        errors.append(f"--route-latency-ms must be <= {DEFAULT_MAX_ROUTE_LATENCY_MS}")
    if (
        args.max_route_latency_ms is not None
        and args.max_route_latency_ms > DEFAULT_MAX_ROUTE_LATENCY_MS
    ):
        errors.append(f"--max-route-latency-ms must be <= {DEFAULT_MAX_ROUTE_LATENCY_MS}")
    if (
        args.max_lifecycle_lag_seconds is not None
        and args.max_lifecycle_lag_seconds > DEFAULT_MAX_LIFECYCLE_LAG_SECS
    ):
        errors.append(
            f"--max-lifecycle-lag-seconds must be <= {DEFAULT_MAX_LIFECYCLE_LAG_SECS}"
        )
    validate_kind_inputs(args, errors)
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix or args.generated_at_unix,
        max_ledger_age_secs=DEFAULT_MAX_LEDGER_AGE_SECS,
        max_lifecycle_lag_secs=DEFAULT_MAX_LIFECYCLE_LAG_SECS,
        max_route_latency_ms=DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_bake_age_secs=DEFAULT_MAX_BAKE_AGE_SECS,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the reserve/rent gate contract."""

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
        del error
        return [f"--out parent `{path_diagnostic_label(parent)}` cannot be created"]
    tmp_name = f".{path.name}.{os.getpid()}.{secrets.token_hex(8)}.tmp"
    tmp_path = parent / tmp_name
    fd = -1
    try:
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        nofollow = getattr(os, "O_NOFOLLOW", 0)
        if nofollow:
            flags |= nofollow
        fd = os.open(tmp_path, flags, 0o600)
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            fd = -1
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(tmp_path, path)
    except (OSError, RuntimeError) as error:
        del error
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
        return [f"--out `{path_diagnostic_label(path)}` cannot be written"]
    return []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Build payload-free SoraFS SFM-6 reserve/rent canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument("--policy-digest-hex", required=True)
    parser.add_argument("--matrix-digest-hex")
    parser.add_argument("--ledger-digest-hex")
    parser.add_argument("--verified-claim", action="append", default=[])
    parser.add_argument("--policy-version", type=positive_int_arg)
    parser.add_argument("--tier-count", type=positive_int_arg)
    parser.add_argument("--storage-class-count", type=positive_int_arg)
    parser.add_argument("--duration-count", type=positive_int_arg)
    parser.add_argument("--scenario-count", type=positive_int_arg)
    parser.add_argument("--storage-class", action="append", default=[])
    parser.add_argument("--tier", action="append", default=[])
    parser.add_argument("--duration", action="append", default=[])
    parser.add_argument("--ledger-count", type=positive_int_arg)
    parser.add_argument("--instruction-count", type=positive_int_arg)
    parser.add_argument("--lifecycle-route", action="append", default=[])
    parser.add_argument("--max-lifecycle-lag-seconds", type=non_negative_int_arg)
    parser.add_argument("--persisted-stage-count", type=positive_int_arg)
    parser.add_argument("--signed-route", action="append", default=[])
    parser.add_argument("--max-route-latency-ms", type=positive_int_arg)
    parser.add_argument("--route-status-code", type=positive_int_arg, default=200)
    parser.add_argument("--route-latency-ms", type=non_negative_int_arg, default=25)
    parser.add_argument("--movement-count", type=positive_int_arg)
    parser.add_argument("--chain-submission-count", type=positive_int_arg)
    parser.add_argument("--finality-poll-attempt-count", type=positive_int_arg)
    parser.add_argument("--credit-line-mutation-count", type=positive_int_arg)
    parser.add_argument("--accrual-cycle-count", type=positive_int_arg)
    parser.add_argument("--appeal-probe-count", type=positive_int_arg)
    parser.add_argument("--approved-appeal-count", type=non_negative_int_arg)
    parser.add_argument("--rejected-appeal-count", type=non_negative_int_arg)
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--bake-id")
    parser.add_argument("--started-at-unix", type=positive_int_arg)
    parser.add_argument("--completed-at-unix", type=positive_int_arg)
    parser.add_argument("--provider-count", type=positive_int_arg)
    parser.add_argument("--rent-cycle-count", type=positive_int_arg)
    parser.add_argument("--top-up-cycle-count", type=positive_int_arg)
    parser.add_argument("--appeal-cycle-count", type=positive_int_arg)
    parser.add_argument(
        "--scheduled-lifecycle-canary-last-tick-unix",
        type=positive_int_arg,
    )
    parser.add_argument("--scheduled-lifecycle-canary-tick-count", type=positive_int_arg)
    parser.add_argument(
        "--scheduled-lifecycle-canary-defaulted-provider-count",
        type=positive_int_arg,
    )
    parser.add_argument("--downstream-compliance-consumer-count", type=positive_int_arg)
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
            "ERROR: SoraFS reserve/rent canary inputs are incomplete:",
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
