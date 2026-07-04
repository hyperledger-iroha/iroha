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
    APPEAL_CYCLE_LABEL_ERROR,
    APPEAL_CYCLE_LABEL_PATTERN,
    BAKE_ID_ERROR,
    BAKE_ID_PATTERN,
    DEFAULT_MAX_BAKE_AGE_SECS,
    DEFAULT_MAX_LEDGER_AGE_SECS,
    DEFAULT_MAX_LIFECYCLE_LAG_SECS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DOWNSTREAM_COMPLIANCE_CONSUMER_LABEL_ERROR,
    DOWNSTREAM_COMPLIANCE_CONSUMER_LABEL_PATTERN,
    FORBIDDEN_BAKE_ID_MARKERS,
    FORBIDDEN_CYCLE_LABEL_MARKERS,
    FORBIDDEN_PROVIDER_LABEL_MARKERS,
    INSTRUCTION_REF_LABEL_ERROR,
    INSTRUCTION_REF_LABEL_PATTERN,
    KIND_BY_NAME,
    LEDGER_REF_LABEL_ERROR,
    LEDGER_REF_LABEL_PATTERN,
    PERSISTED_STAGE_LABEL_ERROR,
    PERSISTED_STAGE_LABEL_PATTERN,
    PROVIDER_LABEL_ERROR,
    PROVIDER_LABEL_PATTERN,
    RENT_CYCLE_LABEL_ERROR,
    RENT_CYCLE_LABEL_PATTERN,
    REQUIRED_APPEAL_POLICY_PROBES,
    REQUIRED_CREDIT_LINE_ACCRUAL_CYCLES,
    REQUIRED_CREDIT_LINE_MUTATIONS,
    REQUIRED_DURATIONS,
    REQUIRED_LIFECYCLE_ROUTES,
    REQUIRED_METRICS,
    REQUIRED_QUOTE_MATRIX_SCENARIOS,
    REQUIRED_RESERVE_MOVEMENT_ACTIONS,
    REQUIRED_SIGNED_ROUTES,
    REQUIRED_STORAGE_CLASSES,
    REQUIRED_TIERS,
    SCHEDULED_LIFECYCLE_TICK_LABEL_ERROR,
    SCHEDULED_LIFECYCLE_TICK_LABEL_PATTERN,
    TOP_UP_CYCLE_LABEL_ERROR,
    TOP_UP_CYCLE_LABEL_PATTERN,
    ValidationOptions,
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


def validate_unique_values(
    values: Iterable[str],
    *,
    option: str,
    errors: list[str],
) -> list[str]:
    """Return canonical non-duplicate reviewed values in input order."""

    values = tuple(values)
    if not values:
        errors.append(f"{option} must include at least one value")
        return []
    seen: set[str] = set()
    unique: list[str] = []
    duplicate = False
    for value in values:
        before = len(errors)
        validate_canonical_string(value, label=option, errors=errors)
        if len(errors) != before:
            continue
        if value in seen:
            duplicate = True
            continue
        seen.add(value)
        unique.append(value)
    if duplicate:
        errors.append(f"{option} must not contain duplicates")
    return unique


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


def validate_bake_id_arg(value: str | None, *, errors: list[str]) -> None:
    """Require a reviewed lowercase reserve provider-bake identifier."""

    validate_canonical_string(value, label="--bake-id", errors=errors)
    if not isinstance(value, str):
        return
    if BAKE_ID_PATTERN.fullmatch(value) is None:
        errors.append(BAKE_ID_ERROR.replace("bake_id", "--bake-id"))
        return
    forbidden = sorted(
        marker for marker in FORBIDDEN_BAKE_ID_MARKERS if marker in value.split("-")
    )
    if forbidden:
        errors.append(f"--bake-id must not contain non-production markers {forbidden}")


def validate_provider_label_arg(
    value: str | None,
    *,
    option: str,
    errors: list[str],
) -> None:
    """Require a reviewed lowercase production provider inventory label."""

    if not isinstance(value, str):
        return
    if PROVIDER_LABEL_PATTERN.fullmatch(value) is None:
        errors.append(PROVIDER_LABEL_ERROR.replace("providers[].name", option))
        return
    forbidden = sorted(
        marker
        for marker in FORBIDDEN_PROVIDER_LABEL_MARKERS
        if marker in value.split("-")
    )
    if forbidden:
        errors.append(f"{option} must not contain non-production markers {forbidden}")


def validate_cycle_label_arg(
    value: str | None,
    *,
    option: str,
    pattern,
    label_error: str,
    errors: list[str],
) -> None:
    """Require a reviewed lowercase production provider-bake cycle label."""

    if not isinstance(value, str):
        return
    if pattern.fullmatch(value) is None:
        errors.append(label_error.replace(option_to_payload_path(option), option))
        return
    forbidden = sorted(
        marker for marker in FORBIDDEN_CYCLE_LABEL_MARKERS if marker in value.split("-")
    )
    if forbidden:
        errors.append(f"{option} must not contain non-production markers {forbidden}")


def validate_inventory_label_arg(
    value: str | None,
    *,
    option: str,
    pattern,
    label_error: str,
    payload_path: str,
    errors: list[str],
) -> None:
    """Require a reviewed lowercase production inventory label."""

    if not isinstance(value, str):
        return
    if pattern.fullmatch(value) is None:
        errors.append(label_error.replace(payload_path, option))
        return
    forbidden = sorted(
        marker for marker in FORBIDDEN_CYCLE_LABEL_MARKERS if marker in value.split("-")
    )
    if forbidden:
        errors.append(f"{option} must not contain non-production markers {forbidden}")


def option_to_payload_path(option: str) -> str:
    """Map provider-bake cycle CLI options to payload diagnostic paths."""

    return {
        "--rent-cycle": "rent_cycles[].name",
        "--top-up-cycle": "top_up_cycles[].name",
        "--appeal-cycle": "appeal_cycles[].name",
    }[option]


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
            "body_blake3_hex": args.route_body_blake3_hex,
            "authz_enforced": True,
            "signature_verified": True,
            "latency_ms": args.route_latency_ms,
        }
        for route in routes
    ]


def build_movement_records(actions: Sequence[str]) -> list[dict[str, Any]]:
    """Build payload-free reserve movement inventory records."""

    return [
        {
            "action": action,
            "accepted": True,
            "chain_submitted": True,
            "finality_confirmed": True,
            "custody_reconciled": True,
        }
        for action in actions
    ]


def build_appeal_probe_records(probes: Sequence[str]) -> list[dict[str, Any]]:
    """Build payload-free appeal policy probe inventory records."""

    return [
        {
            "name": probe,
            "outcome": "approved" if probe.startswith("approved_") else "rejected",
            "governance_recorded": True,
            "policy_digest_bound": True,
        }
        for probe in probes
    ]


def build_credit_line_mutation_records(names: Sequence[str]) -> list[dict[str, Any]]:
    """Build payload-free credit-line mutation inventory records."""

    return [{"name": name, "verified": True} for name in names]


def build_accrual_cycle_records(names: Sequence[str]) -> list[dict[str, Any]]:
    """Build payload-free credit-line accrual inventory records."""

    return [
        {
            "name": name,
            "posted_to_account_state": True,
        }
        for name in names
    ]


def build_provider_records(
    names: Sequence[str],
    defaulted_count: int,
) -> list[dict[str, Any]]:
    """Build payload-free provider bake inventory records."""

    return [
        {
            "name": name,
            "completed": True,
            "defaulted": index < defaulted_count,
            "scheduler_tick_observed": True,
        }
        for index, name in enumerate(names)
    ]


def build_cycle_records(names: Sequence[str], proof_field: str) -> list[dict[str, Any]]:
    """Build payload-free provider-bake cycle inventory records."""

    return [{"name": name, proof_field: True} for name in names]


def build_inventory_records(names: Sequence[str]) -> list[dict[str, Any]]:
    """Build payload-free named inventory records."""

    return [{"name": name} for name in names]


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
                "ledgers": build_inventory_records(args.ledgers),
                "instruction_count": args.instruction_count,
                "instructions": build_inventory_records(args.instructions),
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
                "persisted_stages": build_inventory_records(args.persisted_stages),
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
        movements = build_movement_records(args.movement_actions)
        payload.update(
            {
                "movement_count": args.movement_count,
                "accepted_movement_count": args.movement_count,
                "failed_movement_count": 0,
                "unexpected_failure_count": 0,
                "movements": movements,
                "chain_submission_count": args.chain_submission_count,
                "finality_poll_attempt_count": args.finality_poll_attempt_count,
            }
        )
    elif args.kind == "credit_line":
        payload.update(
            {
                "credit_line_mutation_count": args.credit_line_mutation_count,
                "credit_line_mutations": build_credit_line_mutation_records(
                    args.credit_line_mutations
                ),
                "accrual_cycle_count": args.accrual_cycle_count,
                "accrual_cycles": build_accrual_cycle_records(args.accrual_cycles),
                "unexpected_failure_count": 0,
            }
        )
    elif args.kind == "appeal_policy":
        appeal_probes = build_appeal_probe_records(args.appeal_probes)
        payload.update(
            {
                "appeal_probe_count": args.appeal_probe_count,
                "approved_appeal_count": args.approved_appeal_count,
                "rejected_appeal_count": args.rejected_appeal_count,
                "appeal_probes": appeal_probes,
            }
        )
    elif args.kind == "metrics_alerts":
        payload.update(
            {
                "metrics": args.metrics,
                "metric_count": len(args.metrics),
            }
        )
    elif args.kind == "provider_bake":
        payload.update(
            {
                "bake_id": args.bake_id,
                "started_at_unix": args.started_at_unix,
                "completed_at_unix": args.completed_at_unix,
                "provider_count": args.provider_count,
                "providers": build_provider_records(
                    args.providers,
                    args.scheduled_lifecycle_canary_defaulted_provider_count,
                ),
                "completed_provider_count": args.provider_count,
                "failure_count": 0,
                "rent_cycle_count": args.rent_cycle_count,
                "rent_cycles": build_cycle_records(args.rent_cycles, "settled"),
                "top_up_cycle_count": args.top_up_cycle_count,
                "top_up_cycles": build_cycle_records(
                    args.top_up_cycles,
                    "reconciled",
                ),
                "appeal_cycle_count": args.appeal_cycle_count,
                "appeal_cycles": build_cycle_records(args.appeal_cycles, "reviewed"),
                "scheduled_lifecycle_canary_last_tick_unix": (
                    args.scheduled_lifecycle_canary_last_tick_unix
                ),
                "scheduled_lifecycle_canary_tick_count": (
                    args.scheduled_lifecycle_canary_tick_count
                ),
                "scheduled_lifecycle_canary_ticks": build_inventory_records(
                    args.scheduled_lifecycle_canary_ticks
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
                "downstream_compliance_consumers": build_inventory_records(
                    args.downstream_compliance_consumers
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
        if (
            args.scenario_count is not None
            and args.scenario_count != REQUIRED_QUOTE_MATRIX_SCENARIOS
        ):
            errors.append("--scenario-count must match required quote-matrix product")
    elif args.kind == "ledger_digest":
        require_kind_options(
            args,
            errors,
            (
                ("--ledger-count", args.ledger_count),
                ("--instruction-count", args.instruction_count),
            ),
        )
        args.ledgers = validate_unique_values(
            split_csv_values(args.ledger_ref),
            option="--ledger-ref",
            errors=errors,
        )
        for ledger in args.ledgers:
            validate_inventory_label_arg(
                ledger,
                option="--ledger-ref",
                pattern=LEDGER_REF_LABEL_PATTERN,
                label_error=LEDGER_REF_LABEL_ERROR,
                payload_path="ledgers[].name",
                errors=errors,
            )
        args.instructions = validate_unique_values(
            split_csv_values(args.instruction_ref),
            option="--instruction-ref",
            errors=errors,
        )
        for instruction in args.instructions:
            validate_inventory_label_arg(
                instruction,
                option="--instruction-ref",
                pattern=INSTRUCTION_REF_LABEL_PATTERN,
                label_error=INSTRUCTION_REF_LABEL_ERROR,
                payload_path="instructions[].name",
                errors=errors,
            )
        if (
            args.ledger_count is not None
            and args.ledgers
            and args.ledger_count != len(args.ledgers)
        ):
            errors.append("--ledger-count must match --ledger-ref inventory")
        if (
            args.instruction_count is not None
            and args.instructions
            and args.instruction_count != len(args.instructions)
        ):
            errors.append(
                "--instruction-count must match --instruction-ref inventory"
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
        validate_hex64(
            args.route_body_blake3_hex,
            option="--route-body-blake3-hex",
            errors=errors,
        )
        args.lifecycle_routes = validate_name_set(
            split_csv_values(args.lifecycle_route),
            allowed=REQUIRED_LIFECYCLE_ROUTES,
            option="--lifecycle-route",
            errors=errors,
        )
        args.persisted_stages = validate_unique_values(
            split_csv_values(args.persisted_stage),
            option="--persisted-stage",
            errors=errors,
        )
        for stage in args.persisted_stages:
            validate_inventory_label_arg(
                stage,
                option="--persisted-stage",
                pattern=PERSISTED_STAGE_LABEL_PATTERN,
                label_error=PERSISTED_STAGE_LABEL_ERROR,
                payload_path="persisted_stages[].name",
                errors=errors,
            )
        if (
            args.persisted_stage_count is not None
            and args.persisted_stages
            and args.persisted_stage_count != len(args.persisted_stages)
        ):
            errors.append(
                "--persisted-stage-count must match --persisted-stage inventory"
            )
    elif args.kind == "signed_routes":
        require_kind_options(
            args,
            errors,
            (("--max-route-latency-ms", args.max_route_latency_ms),),
        )
        validate_hex64(
            args.route_body_blake3_hex,
            option="--route-body-blake3-hex",
            errors=errors,
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
        args.movement_actions = validate_name_set(
            split_csv_values(args.movement_action),
            allowed=REQUIRED_RESERVE_MOVEMENT_ACTIONS,
            option="--movement-action",
            errors=errors,
        )
        if (
            args.movement_count is not None
            and args.movement_actions
            and args.movement_count != len(args.movement_actions)
        ):
            errors.append("--movement-count must match --movement-action inventory")
    elif args.kind == "credit_line":
        require_kind_options(
            args,
            errors,
            (
                ("--credit-line-mutation-count", args.credit_line_mutation_count),
                ("--accrual-cycle-count", args.accrual_cycle_count),
            ),
        )
        args.credit_line_mutations = validate_name_set(
            split_csv_values(args.credit_line_mutation),
            allowed=REQUIRED_CREDIT_LINE_MUTATIONS,
            option="--credit-line-mutation",
            errors=errors,
        )
        args.accrual_cycles = validate_name_set(
            split_csv_values(args.accrual_cycle),
            allowed=REQUIRED_CREDIT_LINE_ACCRUAL_CYCLES,
            option="--accrual-cycle",
            errors=errors,
        )
        if (
            args.credit_line_mutation_count is not None
            and args.credit_line_mutations
            and args.credit_line_mutation_count != len(args.credit_line_mutations)
        ):
            errors.append(
                "--credit-line-mutation-count must match "
                "--credit-line-mutation inventory"
            )
        if (
            args.accrual_cycle_count is not None
            and args.accrual_cycles
            and args.accrual_cycle_count != len(args.accrual_cycles)
        ):
            errors.append("--accrual-cycle-count must match --accrual-cycle inventory")
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
        args.appeal_probes = validate_name_set(
            split_csv_values(args.appeal_probe),
            allowed=REQUIRED_APPEAL_POLICY_PROBES,
            option="--appeal-probe",
            errors=errors,
        )
        if args.appeal_probes:
            approved_count = sum(
                1 for probe in args.appeal_probes if probe.startswith("approved_")
            )
            rejected_count = len(args.appeal_probes) - approved_count
            if args.appeal_probe_count != len(args.appeal_probes):
                errors.append("--appeal-probe-count must match --appeal-probe inventory")
            if args.approved_appeal_count != approved_count:
                errors.append(
                    "--approved-appeal-count must match approved --appeal-probe inventory"
                )
            if args.rejected_appeal_count != rejected_count:
                errors.append(
                    "--rejected-appeal-count must match rejected --appeal-probe inventory"
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
        validate_bake_id_arg(args.bake_id, errors=errors)
        args.providers = validate_unique_values(
            split_csv_values(args.provider),
            option="--provider",
            errors=errors,
        )
        for provider in args.providers:
            validate_provider_label_arg(provider, option="--provider", errors=errors)
        args.rent_cycles = validate_unique_values(
            split_csv_values(args.rent_cycle),
            option="--rent-cycle",
            errors=errors,
        )
        for cycle in args.rent_cycles:
            validate_cycle_label_arg(
                cycle,
                option="--rent-cycle",
                pattern=RENT_CYCLE_LABEL_PATTERN,
                label_error=RENT_CYCLE_LABEL_ERROR,
                errors=errors,
            )
        args.top_up_cycles = validate_unique_values(
            split_csv_values(args.top_up_cycle),
            option="--top-up-cycle",
            errors=errors,
        )
        for cycle in args.top_up_cycles:
            validate_cycle_label_arg(
                cycle,
                option="--top-up-cycle",
                pattern=TOP_UP_CYCLE_LABEL_PATTERN,
                label_error=TOP_UP_CYCLE_LABEL_ERROR,
                errors=errors,
            )
        args.appeal_cycles = validate_unique_values(
            split_csv_values(args.appeal_cycle),
            option="--appeal-cycle",
            errors=errors,
        )
        for cycle in args.appeal_cycles:
            validate_cycle_label_arg(
                cycle,
                option="--appeal-cycle",
                pattern=APPEAL_CYCLE_LABEL_PATTERN,
                label_error=APPEAL_CYCLE_LABEL_ERROR,
                errors=errors,
            )
        args.scheduled_lifecycle_canary_ticks = validate_unique_values(
            split_csv_values(args.scheduled_lifecycle_canary_tick),
            option="--scheduled-lifecycle-canary-tick",
            errors=errors,
        )
        for tick in args.scheduled_lifecycle_canary_ticks:
            validate_inventory_label_arg(
                tick,
                option="--scheduled-lifecycle-canary-tick",
                pattern=SCHEDULED_LIFECYCLE_TICK_LABEL_PATTERN,
                label_error=SCHEDULED_LIFECYCLE_TICK_LABEL_ERROR,
                payload_path="scheduled_lifecycle_canary_ticks[].name",
                errors=errors,
            )
        if (
            args.provider_count is not None
            and args.providers
            and args.provider_count != len(args.providers)
        ):
            errors.append("--provider-count must match --provider inventory")
        if (
            args.scheduled_lifecycle_canary_defaulted_provider_count is not None
            and args.providers
            and args.scheduled_lifecycle_canary_defaulted_provider_count
            > len(args.providers)
        ):
            errors.append(
                "--scheduled-lifecycle-canary-defaulted-provider-count must "
                "not exceed --provider inventory"
            )
        if (
            args.rent_cycle_count is not None
            and args.rent_cycles
            and args.rent_cycle_count != len(args.rent_cycles)
        ):
            errors.append("--rent-cycle-count must match --rent-cycle inventory")
        if (
            args.top_up_cycle_count is not None
            and args.top_up_cycles
            and args.top_up_cycle_count != len(args.top_up_cycles)
        ):
            errors.append("--top-up-cycle-count must match --top-up-cycle inventory")
        if (
            args.appeal_cycle_count is not None
            and args.appeal_cycles
            and args.appeal_cycle_count != len(args.appeal_cycles)
        ):
            errors.append("--appeal-cycle-count must match --appeal-cycle inventory")
        if (
            args.scheduled_lifecycle_canary_tick_count is not None
            and args.scheduled_lifecycle_canary_ticks
            and args.scheduled_lifecycle_canary_tick_count
            != len(args.scheduled_lifecycle_canary_ticks)
        ):
            errors.append(
                "--scheduled-lifecycle-canary-tick-count must match "
                "--scheduled-lifecycle-canary-tick inventory"
            )
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
        args.downstream_compliance_consumers = validate_unique_values(
            split_csv_values(args.downstream_compliance_consumer),
            option="--downstream-compliance-consumer",
            errors=errors,
        )
        for consumer in args.downstream_compliance_consumers:
            validate_inventory_label_arg(
                consumer,
                option="--downstream-compliance-consumer",
                pattern=DOWNSTREAM_COMPLIANCE_CONSUMER_LABEL_PATTERN,
                label_error=DOWNSTREAM_COMPLIANCE_CONSUMER_LABEL_ERROR,
                payload_path="downstream_compliance_consumers[].name",
                errors=errors,
            )
        if (
            args.downstream_compliance_consumer_count is not None
            and args.downstream_compliance_consumers
            and args.downstream_compliance_consumer_count
            != len(args.downstream_compliance_consumers)
        ):
            errors.append(
                "--downstream-compliance-consumer-count must match "
                "--downstream-compliance-consumer inventory"
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
        write_all_checker_summary_bytes(fd, text.encode("utf-8"))
        os.fsync(fd)
        os.close(fd)
        fd = -1
        os.replace(tmp_path, path)
        parent_sync_errors = fsync_checker_output_parent(path, label="--out")
        if parent_sync_errors:
            return parent_sync_errors
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
    parser.add_argument("--ledger-ref", action="append", default=[])
    parser.add_argument("--instruction-count", type=positive_int_arg)
    parser.add_argument("--instruction-ref", action="append", default=[])
    parser.add_argument("--lifecycle-route", action="append", default=[])
    parser.add_argument("--max-lifecycle-lag-seconds", type=non_negative_int_arg)
    parser.add_argument("--persisted-stage-count", type=positive_int_arg)
    parser.add_argument("--persisted-stage", action="append", default=[])
    parser.add_argument("--signed-route", action="append", default=[])
    parser.add_argument("--max-route-latency-ms", type=positive_int_arg)
    parser.add_argument("--route-body-blake3-hex")
    parser.add_argument("--route-status-code", type=positive_int_arg, default=200)
    parser.add_argument("--route-latency-ms", type=non_negative_int_arg, default=25)
    parser.add_argument("--movement-count", type=positive_int_arg)
    parser.add_argument("--movement-action", action="append", default=[])
    parser.add_argument("--chain-submission-count", type=positive_int_arg)
    parser.add_argument("--finality-poll-attempt-count", type=positive_int_arg)
    parser.add_argument("--credit-line-mutation-count", type=positive_int_arg)
    parser.add_argument("--credit-line-mutation", action="append", default=[])
    parser.add_argument("--accrual-cycle-count", type=positive_int_arg)
    parser.add_argument("--accrual-cycle", action="append", default=[])
    parser.add_argument("--appeal-probe-count", type=positive_int_arg)
    parser.add_argument("--appeal-probe", action="append", default=[])
    parser.add_argument("--approved-appeal-count", type=non_negative_int_arg)
    parser.add_argument("--rejected-appeal-count", type=non_negative_int_arg)
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--bake-id")
    parser.add_argument("--started-at-unix", type=positive_int_arg)
    parser.add_argument("--completed-at-unix", type=positive_int_arg)
    parser.add_argument("--provider-count", type=positive_int_arg)
    parser.add_argument("--provider", action="append", default=[])
    parser.add_argument("--rent-cycle-count", type=positive_int_arg)
    parser.add_argument("--rent-cycle", action="append", default=[])
    parser.add_argument("--top-up-cycle-count", type=positive_int_arg)
    parser.add_argument("--top-up-cycle", action="append", default=[])
    parser.add_argument("--appeal-cycle-count", type=positive_int_arg)
    parser.add_argument("--appeal-cycle", action="append", default=[])
    parser.add_argument(
        "--scheduled-lifecycle-canary-last-tick-unix",
        type=positive_int_arg,
    )
    parser.add_argument("--scheduled-lifecycle-canary-tick-count", type=positive_int_arg)
    parser.add_argument("--scheduled-lifecycle-canary-tick", action="append", default=[])
    parser.add_argument(
        "--scheduled-lifecycle-canary-defaulted-provider-count",
        type=positive_int_arg,
    )
    parser.add_argument("--downstream-compliance-consumer-count", type=positive_int_arg)
    parser.add_argument("--downstream-compliance-consumer", action="append", default=[])
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
