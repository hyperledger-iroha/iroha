"""Tests for scripts/check_sorafs_reserve_rent_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_reserve_rent_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_reserve_rent_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


NOW_UNIX = 1_800_100_000
GENERATED_AT = NOW_UNIX - 120
DIGEST = "ab" * 32
MATRIX_DIGEST = "cd" * 32
LEDGER_DIGEST = "ef" * 32
ALT_LEDGER_DIGEST = "12" * 32
ROUTE_BODY_DIGEST = "34" * 32
DEPLOYMENT_ID = "reserve-prod-20260626"
ENVIRONMENT = "production"


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def with_reviewed_context(payload: dict) -> dict:
    payload = dict(payload)
    payload.setdefault("generated_at_unix", GENERATED_AT)
    payload.setdefault("deployment_id", DEPLOYMENT_ID)
    payload.setdefault("environment", ENVIRONMENT)
    payload.setdefault("deployment_context_reviewed", True)
    return payload


def policy_config() -> dict:
    return with_reviewed_context({
        "schema": "sorafs.reserve.policy_config_canary.v1",
        "status": "passed",
        "policy_digest_hex": DIGEST,
        "policy_version": 1,
        "config_source": "iroha_config",
        "governance_approved": True,
        "tier_count": 3,
        "storage_class_count": 3,
        "duration_count": 3,
        "credit_line_caps_present": True,
        "apr_policy_present": True,
        "policy_payload_included": False,
    })


def quote_matrix() -> dict:
    return with_reviewed_context({
        "schema": "sorafs.reserve.quote_matrix_canary.v1",
        "status": "passed",
        "matrix_digest_hex": MATRIX_DIGEST,
        "policy_digest_hex": DIGEST,
        "scenario_count": 27,
        "passed_scenario_count": 27,
        "storage_classes": ["hot", "warm", "archive"],
        "tiers": ["tier-a", "tier-b", "tier-c"],
        "durations": ["monthly", "quarterly", "annual"],
        "quote_payloads_included": False,
    })


def ledger_digest(*, generated_at: int = GENERATED_AT) -> dict:
    return with_reviewed_context({
        "schema": "sorafs.reserve.ledger_digest_canary.v1",
        "status": "passed",
        "policy_digest_hex": DIGEST,
        "matrix_digest_hex": MATRIX_DIGEST,
        "ledger_digest_hex": LEDGER_DIGEST,
        "generated_at_unix": generated_at,
        "ledger_count": 1,
        "ledgers": [{"name": "reserve-ledger-main"}],
        "instruction_count": 2,
        "instructions": [
            {"name": "reserve-instruction-rent-settlement"},
            {"name": "reserve-instruction-reserve-top-up"},
        ],
        "rent_transfer_present": True,
        "reserve_top_up_transfer_present": True,
        "instruction_hashes_verified": True,
        "ledger_projection_verified": True,
        "raw_ledger_included": False,
        "raw_transfer_instructions_included": False,
    })


def route(name: str) -> dict:
    return {
        "name": name,
        "passed": True,
        "status_code": 200,
        "body_blake3_hex": ROUTE_BODY_DIGEST,
        "authz_enforced": True,
        "signature_verified": True,
        "latency_ms": 25,
    }


def lifecycle_service() -> dict:
    routes = [
        route("provider_summary"),
        route("lifecycle_status"),
        route("event_history"),
        route("policy_readback"),
    ]
    return with_reviewed_context({
        "schema": "sorafs.reserve.lifecycle_service_canary.v1",
        "status": "passed",
        "policy_digest_hex": DIGEST,
        "matrix_digest_hex": MATRIX_DIGEST,
        "ledger_digest_hex": LEDGER_DIGEST,
        "route_count": len(routes),
        "passed_route_count": len(routes),
        "routes": routes,
        "max_lifecycle_lag_seconds": 60,
        "persisted_stage_count": 4,
        "persisted_stages": [
            {"name": "reserve-lifecycle-stage-active"},
            {"name": "reserve-lifecycle-stage-warning"},
            {"name": "reserve-lifecycle-stage-defaulted"},
            {"name": "reserve-lifecycle-stage-suspended"},
        ],
        "stage_transition_replay_passed": True,
        "governance_event_emitted": True,
        "manual_override_audited": True,
        "response_bodies_included": False,
    })


def signed_routes(*, wrong_account_rejected: bool = True) -> dict:
    routes = [
        route("top_up"),
        route("withdraw"),
        route("appeal_submit"),
        route("policy_update"),
        route("provider_status"),
    ]
    return with_reviewed_context({
        "schema": "sorafs.reserve.signed_route_canary.v1",
        "status": "passed",
        "policy_digest_hex": DIGEST,
        "matrix_digest_hex": MATRIX_DIGEST,
        "ledger_digest_hex": LEDGER_DIGEST,
        "route_count": len(routes),
        "passed_route_count": len(routes),
        "routes": routes,
        "max_route_latency_ms": 250,
        "replay_attack_rejected": True,
        "unsigned_request_rejected": True,
        "wrong_account_rejected": wrong_account_rejected,
        "response_bodies_included": False,
    })


def reserve_movement() -> dict:
    movements = [
        {
            "action": action,
            "accepted": True,
            "chain_submitted": True,
            "finality_confirmed": True,
            "custody_reconciled": True,
        }
        for action in MODULE.REQUIRED_RESERVE_MOVEMENT_ACTIONS
    ]
    return with_reviewed_context({
        "schema": "sorafs.reserve.reserve_movement_canary.v1",
        "status": "passed",
        "policy_digest_hex": DIGEST,
        "matrix_digest_hex": MATRIX_DIGEST,
        "ledger_digest_hex": LEDGER_DIGEST,
        "movement_count": len(movements),
        "accepted_movement_count": len(movements),
        "failed_movement_count": 0,
        "unexpected_failure_count": 0,
        "movements": movements,
        "rent_settlement_present": True,
        "reserve_top_up_present": True,
        "withdrawal_limits_enforced": True,
        "treasury_reconciliation_passed": True,
        "double_spend_rejected": True,
        "chain_submission_count": 4,
        "finality_poll_attempt_count": 4,
        "live_chain_submission_verified": True,
        "submitted_transaction_hash_readback_verified": True,
        "automatic_finality_polling_verified": True,
        "finality_poll_confirmed_status_verified": True,
        "finality_poll_timeout_rejected": True,
        "custody_status_route_present": True,
        "submitted_custody_evidence_present": True,
        "confirmed_custody_evidence_present": True,
        "rejected_custody_reconciliation_passed": True,
        "confirmed_balance_projection_verified": True,
        "confirmed_withdrawal_underflow_rejected": True,
        "chain_reconciled_readback_verified": True,
        "raw_transfer_included": False,
        "raw_instruction_included": False,
    })


def credit_line() -> dict:
    credit_line_mutations = [
        {
            "name": name,
            "verified": True,
        }
        for name in MODULE.REQUIRED_CREDIT_LINE_MUTATIONS
    ]
    accrual_cycles = [
        {
            "name": name,
            "posted_to_account_state": True,
        }
        for name in MODULE.REQUIRED_CREDIT_LINE_ACCRUAL_CYCLES
    ]
    return with_reviewed_context({
        "schema": "sorafs.reserve.credit_line_canary.v1",
        "status": "passed",
        "policy_digest_hex": DIGEST,
        "matrix_digest_hex": MATRIX_DIGEST,
        "ledger_digest_hex": LEDGER_DIGEST,
        "credit_line_mutation_count": len(credit_line_mutations),
        "credit_line_mutations": credit_line_mutations,
        "accrual_cycle_count": len(accrual_cycles),
        "accrual_cycles": accrual_cycles,
        "credit_draw_cap_enforced": True,
        "apr_accrual_verified": True,
        "manual_approval_tier_blocked": True,
        "credit_shortfall_reported": True,
        "live_account_mutation_verified": True,
        "credit_line_account_state_readback_verified": True,
        "credit_accrual_posted_to_account_state": True,
        "manual_approval_tier_did_not_mutate_account": True,
        "account_state_reconciliation_verified": True,
        "no_negative_balance": True,
        "unexpected_failure_count": 0,
        "raw_ledger_included": False,
    })


def appeal_policy() -> dict:
    appeal_probes = [
        {
            "name": "approved_policy_override",
            "outcome": "approved",
            "governance_recorded": True,
            "policy_digest_bound": True,
        },
        {
            "name": "rejected_unauthorized_appeal",
            "outcome": "rejected",
            "governance_recorded": True,
            "policy_digest_bound": True,
        },
    ]
    return with_reviewed_context({
        "schema": "sorafs.reserve.appeal_policy_canary.v1",
        "status": "passed",
        "policy_digest_hex": DIGEST,
        "matrix_digest_hex": MATRIX_DIGEST,
        "ledger_digest_hex": LEDGER_DIGEST,
        "appeal_probe_count": len(appeal_probes),
        "approved_appeal_count": 1,
        "rejected_appeal_count": 1,
        "appeal_probes": appeal_probes,
        "appeal_route_present": True,
        "policy_update_route_present": True,
        "governance_recorded": True,
        "operator_role_enforced": True,
        "unauthorized_appeal_rejected": True,
        "policy_digest_bound": True,
        "appeal_payloads_included": False,
    })


def metrics_alerts(*, include_all_metrics: bool = True) -> dict:
    metrics = [
        "sorafs_reserve_ledger_rent_due_xor",
        "sorafs_reserve_ledger_reserve_shortfall_xor",
        "sorafs_reserve_ledger_top_up_shortfall_xor",
        "sorafs_reserve_ledger_requires_top_up",
        "sorafs_reserve_ledger_meets_underwriting",
        "sorafs_reserve_ledger_instruction_total",
        "sorafs_reserve_ledger_transfer_xor",
        "torii_sorafs_reserve_lifecycle_stage_providers",
        "torii_sorafs_reserve_credit_draw_micro_xor",
        "torii_sorafs_reserve_credit_shortfall_micro_xor",
        "torii_sorafs_reserve_accrued_interest_micro_xor",
        "torii_sorafs_reserve_defaulted_providers",
        "torii_sorafs_reserve_appeal_backlog",
        "torii_sorafs_reserve_custody_movements",
        "torii_sorafs_reserve_chain_reconciled_movements",
        "torii_sorafs_reserve_service_requests_total",
        "torii_sorafs_reserve_service_rate_limit_total",
    ]
    if not include_all_metrics:
        metrics.pop()
    return with_reviewed_context({
        "schema": "sorafs.reserve.metrics_alert_canary.v1",
        "status": "passed",
        "policy_digest_hex": DIGEST,
        "matrix_digest_hex": MATRIX_DIGEST,
        "ledger_digest_hex": LEDGER_DIGEST,
        "metrics_scrape_success": True,
        "dashboard_provisioned": True,
        "alert_rules_installed": True,
        "critical_alerts_firing": False,
        "metrics": metrics,
        "metric_count": len(metrics),
        "response_bodies_included": False,
    })


def provider_bake(*, failure_count: int = 0) -> dict:
    providers = [
        {
            "name": "provider-alpha",
            "completed": True,
            "defaulted": True,
            "scheduler_tick_observed": True,
        },
        {
            "name": "provider-beta",
            "completed": True,
            "defaulted": False,
            "scheduler_tick_observed": True,
        },
        {
            "name": "provider-gamma",
            "completed": True,
            "defaulted": False,
            "scheduler_tick_observed": True,
        },
    ]
    rent_cycles = [
        {"name": "reserve-rent-cycle-001", "settled": True},
        {"name": "reserve-rent-cycle-002", "settled": True},
    ]
    top_up_cycles = [
        {"name": "reserve-top-up-cycle-001", "reconciled": True},
        {"name": "reserve-top-up-cycle-002", "reconciled": True},
    ]
    appeal_cycles = [{"name": "reserve-appeal-cycle-001", "reviewed": True}]
    return with_reviewed_context({
        "schema": "sorafs.reserve.provider_bake.v1",
        "status": "passed",
        "policy_digest_hex": DIGEST,
        "matrix_digest_hex": MATRIX_DIGEST,
        "ledger_digest_hex": LEDGER_DIGEST,
        "bake_id": "reserve-bake-001",
        "started_at_unix": GENERATED_AT - 3_600,
        "completed_at_unix": GENERATED_AT,
        "provider_count": len(providers),
        "providers": providers,
        "completed_provider_count": len(providers),
        "failure_count": failure_count,
        "rent_cycle_count": len(rent_cycles),
        "rent_cycles": rent_cycles,
        "top_up_cycle_count": len(top_up_cycles),
        "top_up_cycles": top_up_cycles,
        "appeal_cycle_count": len(appeal_cycles),
        "appeal_cycles": appeal_cycles,
        "scheduler_config_bound": True,
        "scheduled_lifecycle_canary_passed": True,
        "scheduled_lifecycle_canary_last_tick_unix": GENERATED_AT - 60,
        "scheduled_lifecycle_canary_tick_count": 2,
        "scheduled_lifecycle_canary_ticks": [
            {"name": "reserve-lifecycle-tick-001"},
            {"name": "reserve-lifecycle-tick-002"},
        ],
        "scheduled_lifecycle_canary_defaulted_provider_count": 1,
        "scheduled_lifecycle_canary_gateway_sync_verified": True,
        "scheduled_lifecycle_canary_orderbook_rejection_verified": True,
        "governance_packet_attached": True,
        "ledger_digest_attached": True,
        "dashboard_snapshot_attached": True,
        "payloads_included": False,
    })


def governance_approval() -> dict:
    return with_reviewed_context({
        "schema": "sorafs.reserve.governance_approval.v1",
        "status": "passed",
        "policy_digest_hex": DIGEST,
        "matrix_digest_hex": MATRIX_DIGEST,
        "ledger_digest_hex": LEDGER_DIGEST,
        "approved": True,
        "governance_vote_recorded": True,
        "iroha_config_bound": True,
        "reserve_movement_policy_present": True,
        "credit_line_policy_present": True,
        "appeal_policy_present": True,
        "manual_override_policy_present": True,
        "provider_bake_accepted": True,
        "governance_source_entries_published": True,
        "downstream_compliance_policy_applied": True,
        "downstream_compliance_consumer_count": 2,
        "downstream_compliance_consumers": [
            {"name": "reserve-compliance-consumer-gateway"},
            {"name": "reserve-compliance-consumer-orderbook"},
        ],
        "non_reserve_compliance_entries_preserved": True,
        "governance_source_entry_handoff_verified": True,
        "denylist_and_policy_consumers_consistent": True,
        "config_source": "iroha_config",
    })


def write_complete_evidence(root: Path) -> None:
    write_json(root / "policy-config.json", policy_config())
    write_json(root / "quote-matrix.json", quote_matrix())
    write_json(root / "ledger-digest.json", ledger_digest())
    write_json(root / "lifecycle-service.json", lifecycle_service())
    write_json(root / "signed-routes.json", signed_routes())
    write_json(root / "reserve-movement.json", reserve_movement())
    write_json(root / "credit-line.json", credit_line())
    write_json(root / "appeal-policy.json", appeal_policy())
    write_json(root / "metrics-alerts.json", metrics_alerts())
    write_json(root / "provider-bake.json", provider_bake())
    write_json(root / "governance-approval.json", governance_approval())


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.reserve_rent.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["signed_routes"]["valid"] is True
    for row in payload["required"].values():
        assert row["present"] is True
        assert row["valid"] is True
        for artifact in row["artifacts"]:
            fingerprint = artifact["fingerprint"]
            assert fingerprint["generated_at_unix"] == GENERATED_AT
            assert fingerprint["deployment_id"] == DEPLOYMENT_ID
            assert fingerprint["environment"] == ENVIRONMENT
            assert fingerprint["deployment_context_reviewed"] is True
    assert payload["valid_policy_matrix_ledger_bindings"] == [
        {
            "policy_digest_hex": DIGEST,
            "matrix_digest_hex": MATRIX_DIGEST,
            "ledger_digest_hex": LEDGER_DIGEST,
        }
    ]
    assert payload["metric_count_values"] == [len(MODULE.REQUIRED_METRICS)]
    assert payload["metrics"] == sorted(MODULE.REQUIRED_METRICS)
    metrics_artifact = payload["required"]["metrics_alerts"]["artifacts"][0]
    assert metrics_artifact["fingerprint"]["metric_count"] == len(
        MODULE.REQUIRED_METRICS
    )
    assert metrics_artifact["fingerprint"]["metrics"] == list(MODULE.REQUIRED_METRICS)
    assert payload["valid_provider_bakes"] == [
        {
            "bake_id": "reserve-bake-001",
            "deployment_id": DEPLOYMENT_ID,
            "environment": ENVIRONMENT,
            "policy_digest_hex": DIGEST,
            "matrix_digest_hex": MATRIX_DIGEST,
            "ledger_digest_hex": LEDGER_DIGEST,
            "started_at_unix": GENERATED_AT - 3_600,
            "completed_at_unix": GENERATED_AT,
            "provider_count": 3,
            "scheduled_lifecycle_canary_last_tick_unix": GENERATED_AT - 60,
            "scheduled_lifecycle_canary_tick_count": 2,
            "scheduled_lifecycle_canary_defaulted_provider_count": 1,
        }
    ]


def test_payload_safety_flags_are_required(tmp_path: Path) -> None:
    cases = (
        (
            "policy_config",
            "policy-config.json",
            policy_config,
            "policy_payload_included",
        ),
        (
            "quote_matrix",
            "quote-matrix.json",
            quote_matrix,
            "quote_payloads_included",
        ),
        (
            "ledger_digest",
            "ledger-digest.json",
            ledger_digest,
            "raw_ledger_included",
        ),
        (
            "ledger_digest",
            "ledger-digest.json",
            ledger_digest,
            "raw_transfer_instructions_included",
        ),
        (
            "lifecycle_service",
            "lifecycle-service.json",
            lifecycle_service,
            "response_bodies_included",
        ),
        (
            "signed_routes",
            "signed-routes.json",
            signed_routes,
            "response_bodies_included",
        ),
        (
            "reserve_movement",
            "reserve-movement.json",
            reserve_movement,
            "raw_transfer_included",
        ),
        (
            "reserve_movement",
            "reserve-movement.json",
            reserve_movement,
            "raw_instruction_included",
        ),
        (
            "credit_line",
            "credit-line.json",
            credit_line,
            "raw_ledger_included",
        ),
        (
            "appeal_policy",
            "appeal-policy.json",
            appeal_policy,
            "appeal_payloads_included",
        ),
        (
            "metrics_alerts",
            "metrics-alerts.json",
            metrics_alerts,
            "critical_alerts_firing",
        ),
        (
            "metrics_alerts",
            "metrics-alerts.json",
            metrics_alerts,
            "response_bodies_included",
        ),
        (
            "provider_bake",
            "provider-bake.json",
            provider_bake,
            "payloads_included",
        ),
    )
    for kind, filename, factory, field in cases:
        root = tmp_path / f"{kind}-{field}"
        root.mkdir()
        write_complete_evidence(root)
        payload = factory()
        del payload[field]
        write_json(root / filename, payload)
        summary = root / "summary.json"

        assert run_gate(root, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert artifact["valid"] is False
        assert f"{field} must be false" in artifact["errors"]


def test_missing_signed_routes_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "signed-routes.json").unlink()

    assert run_gate(tmp_path) == 1


def test_quote_matrix_dimensions_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = quote_matrix()
    payload["storage_classes"].append("hot")
    write_json(tmp_path / "quote-matrix.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["quote_matrix"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "storage_classes must not contain duplicate values" in artifact["errors"]


def test_quote_matrix_scenario_count_must_match_dimension_product(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = quote_matrix()
    payload["storage_classes"].append("cold")
    write_json(tmp_path / "quote-matrix.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["quote_matrix"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "scenario_count must equal unique storage_classes * tiers * durations count"
        in artifact["errors"]
    )


def test_quote_matrix_dimensions_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    cases = (
        ("storage_classes", "cold"),
        ("tiers", "tier-d"),
        ("durations", "weekly"),
    )
    for field, unknown in cases:
        root = tmp_path / field
        root.mkdir()
        write_complete_evidence(root)
        payload = quote_matrix()
        payload[field].append(unknown)
        payload["scenario_count"] = (
            len(payload["storage_classes"])
            * len(payload["tiers"])
            * len(payload["durations"])
        )
        payload["passed_scenario_count"] = payload["scenario_count"]
        write_json(root / "quote-matrix.json", payload)
        summary = root / "summary.json"

        assert run_gate(root, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"]["quote_matrix"]["artifacts"][0]
        assert artifact["valid"] is False
        assert f"{field} must not include unknown values" in artifact["errors"]


def test_route_count_must_match_unique_routes_for_route_artifacts(
    tmp_path: Path,
) -> None:
    route_artifacts = (
        ("lifecycle_service", "lifecycle-service.json", lifecycle_service),
        ("signed_routes", "signed-routes.json", signed_routes),
    )
    for kind, filename, factory in route_artifacts:
        root = tmp_path / kind
        root.mkdir()
        write_complete_evidence(root)
        payload = factory()
        payload["route_count"] += 1
        payload["passed_route_count"] = payload["route_count"]
        write_json(root / filename, payload)
        summary = root / "summary.json"

        assert run_gate(root, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert "route_count must match unique routes count" in artifact["errors"]


def test_routes_must_not_duplicate_for_route_artifacts(tmp_path: Path) -> None:
    route_artifacts = (
        ("lifecycle_service", "lifecycle-service.json", lifecycle_service),
        ("signed_routes", "signed-routes.json", signed_routes),
    )
    for kind, filename, factory in route_artifacts:
        root = tmp_path / kind
        root.mkdir()
        write_complete_evidence(root)
        payload = factory()
        payload["routes"].append(dict(payload["routes"][0]))
        payload["route_count"] = len(payload["routes"])
        payload["passed_route_count"] = len(payload["routes"])
        write_json(root / filename, payload)
        summary = root / "summary.json"

        assert run_gate(root, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert "routes must not contain duplicate values" in artifact["errors"]
        assert "route_count must match unique routes count" in artifact["errors"]


def test_routes_must_not_include_unknown_values_for_route_artifacts(
    tmp_path: Path,
) -> None:
    route_artifacts = (
        ("lifecycle_service", "lifecycle-service.json", lifecycle_service),
        ("signed_routes", "signed-routes.json", signed_routes),
    )
    for kind, filename, factory in route_artifacts:
        root = tmp_path / kind
        root.mkdir()
        write_complete_evidence(root)
        payload = factory()
        payload["routes"].append(route("debug_route"))
        payload["route_count"] = len(payload["routes"])
        payload["passed_route_count"] = len(payload["routes"])
        write_json(root / filename, payload)
        summary = root / "summary.json"

        assert run_gate(root, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert artifact["valid"] is False
        assert "routes must not include unknown values" in artifact["errors"]


def test_route_body_hash_is_required_for_route_artifacts(tmp_path: Path) -> None:
    route_artifacts = (
        ("lifecycle_service", "lifecycle-service.json", lifecycle_service),
        ("signed_routes", "signed-routes.json", signed_routes),
    )
    for kind, filename, factory in route_artifacts:
        root = tmp_path / kind
        root.mkdir()
        write_complete_evidence(root)
        payload = factory()
        del payload["routes"][0]["body_blake3_hex"]
        write_json(root / filename, payload)
        summary = root / "summary.json"

        assert run_gate(root, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert (
            "routes[0].body_blake3_hex must be a non-empty string"
            in artifact["errors"]
        )


def test_route_latency_is_required_for_route_artifacts(tmp_path: Path) -> None:
    route_artifacts = (
        ("lifecycle_service", "lifecycle-service.json", lifecycle_service),
        ("signed_routes", "signed-routes.json", signed_routes),
    )
    for kind, filename, factory in route_artifacts:
        root = tmp_path / kind
        root.mkdir()
        write_complete_evidence(root)
        payload = factory()
        del payload["routes"][0]["latency_ms"]
        write_json(root / filename, payload)
        summary = root / "summary.json"

        assert run_gate(root, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert (
            "routes[0].latency_ms must be a non-negative integer"
            in artifact["errors"]
        )


def test_lifecycle_persisted_stage_count_must_match_unique_stages(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = lifecycle_service()
    payload["persisted_stages"].pop()
    write_json(tmp_path / "lifecycle-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["lifecycle_service"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "persisted_stage_count must match unique persisted_stages count"
        in artifact["errors"]
    )


def test_lifecycle_persisted_stages_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = lifecycle_service()
    payload["persisted_stages"].append(
        {"name": "reserve-lifecycle-stage-active"}
    )
    payload["persisted_stage_count"] = len(payload["persisted_stages"])
    write_json(tmp_path / "lifecycle-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["lifecycle_service"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "persisted_stages must not contain duplicate values" in artifact["errors"]


def test_lifecycle_persisted_stages_must_use_reviewed_labels(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = lifecycle_service()
    payload["persisted_stages"][0]["name"] = "lifecycle-stage-active"
    write_json(tmp_path / "lifecycle-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["lifecycle_service"]["artifacts"][0]
    assert artifact["valid"] is False
    assert MODULE.PERSISTED_STAGE_LABEL_ERROR in artifact["errors"]


def test_lifecycle_persisted_stages_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = lifecycle_service()
    payload["persisted_stages"][0]["name"] = "reserve-lifecycle-stage-placeholder"
    write_json(tmp_path / "lifecycle-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["lifecycle_service"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "persisted_stages[0].name must not contain non-production markers "
        "['placeholder']"
    ) in artifact["errors"]


def test_lifecycle_latency_evidence_must_be_integer_units(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = lifecycle_service()
    payload["max_lifecycle_lag_seconds"] = 12.5
    payload["routes"][0]["latency_ms"] = 7.5
    write_json(tmp_path / "lifecycle-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["lifecycle_service"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "max_lifecycle_lag_seconds must be a non-negative integer"
        in artifact["errors"]
    )
    assert (
        "routes[0].latency_ms must be a non-negative integer"
        in artifact["errors"]
    )


def test_signed_route_latency_threshold_must_be_positive_integer(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = signed_routes()
    payload["max_route_latency_ms"] = 12.5
    payload["routes"][0]["latency_ms"] = 7.5
    write_json(tmp_path / "signed-routes.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["signed_routes"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "max_route_latency_ms must be a positive integer" in artifact["errors"]
    assert (
        "routes[0].latency_ms must be a non-negative integer"
        in artifact["errors"]
    )


def test_stale_ledger_digest_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    stale = NOW_UNIX - MODULE.DEFAULT_MAX_LEDGER_AGE_SECS - 1
    write_json(tmp_path / "ledger-digest.json", ledger_digest(generated_at=stale))

    assert run_gate(tmp_path) == 1


def test_ledger_digest_ledger_count_must_match_unique_ledgers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = ledger_digest()
    payload["ledgers"].append({"name": "reserve-ledger-secondary"})
    write_json(tmp_path / "ledger-digest.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["ledger_digest"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "ledger_count must match unique ledgers count" in artifact["errors"]


def test_ledger_digest_ledgers_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = ledger_digest()
    payload["ledgers"].append({"name": "reserve-ledger-main"})
    payload["ledger_count"] = len(payload["ledgers"])
    write_json(tmp_path / "ledger-digest.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["ledger_digest"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "ledgers must not contain duplicate values" in artifact["errors"]


def test_ledger_digest_ledgers_must_use_reviewed_labels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = ledger_digest()
    payload["ledgers"][0]["name"] = "ledger-main"
    write_json(tmp_path / "ledger-digest.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["ledger_digest"]["artifacts"][0]
    assert artifact["valid"] is False
    assert MODULE.LEDGER_REF_LABEL_ERROR in artifact["errors"]


def test_ledger_digest_ledgers_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = ledger_digest()
    payload["ledgers"][0]["name"] = "reserve-ledger-placeholder"
    write_json(tmp_path / "ledger-digest.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["ledger_digest"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "ledgers[0].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_ledger_digest_instruction_count_must_match_unique_instructions(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = ledger_digest()
    payload["instructions"].pop()
    write_json(tmp_path / "ledger-digest.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["ledger_digest"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "instruction_count must match unique instructions count" in artifact["errors"]


def test_ledger_digest_instructions_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = ledger_digest()
    payload["instructions"].append({"name": "reserve-instruction-rent-settlement"})
    payload["instruction_count"] = len(payload["instructions"])
    write_json(tmp_path / "ledger-digest.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["ledger_digest"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "instructions must not contain duplicate values" in artifact["errors"]


def test_ledger_digest_instructions_must_use_reviewed_labels(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = ledger_digest()
    payload["instructions"][0]["name"] = "instruction-rent-settlement"
    write_json(tmp_path / "ledger-digest.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["ledger_digest"]["artifacts"][0]
    assert artifact["valid"] is False
    assert MODULE.INSTRUCTION_REF_LABEL_ERROR in artifact["errors"]


def test_ledger_digest_instructions_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = ledger_digest()
    payload["instructions"][0]["name"] = "reserve-instruction-placeholder"
    write_json(tmp_path / "ledger-digest.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["ledger_digest"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "instructions[0].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_payload_leakage_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = ledger_digest()
    payload["private_key"] = "not-for-evidence"
    write_json(tmp_path / "ledger-digest.json", payload)

    assert run_gate(tmp_path) == 1


def test_missing_required_metric_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "metrics-alerts.json", metrics_alerts(include_all_metrics=False))

    assert run_gate(tmp_path) == 1


def test_metrics_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = metrics_alerts()
    payload["metrics"].append("torii_sorafs_reserve_debug_metric")
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "metrics-alerts.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["metrics_alerts"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "metrics must not include unknown values" in artifact["errors"]


def test_metrics_payload_free_flags_are_required(tmp_path: Path) -> None:
    for field in ("critical_alerts_firing", "response_bodies_included"):
        root = tmp_path / field
        root.mkdir()
        write_complete_evidence(root)
        payload = metrics_alerts()
        del payload[field]
        write_json(root / "metrics-alerts.json", payload)
        summary = root / "summary.json"

        assert run_gate(root, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"]["metrics_alerts"]["artifacts"][0]
        assert artifact["valid"] is False
        assert f"{field} must be false" in artifact["errors"]


def test_ledger_requires_policy_matrix_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = ledger_digest()
    payload.pop("matrix_digest_hex")
    write_json(tmp_path / "ledger-digest.json", payload)

    assert run_gate(tmp_path) == 1


def test_policy_config_requires_policy_digest(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = policy_config()
    payload["policy_digest_hex"] = "not-hex"
    write_json(tmp_path / "policy-config.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["policy_config"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "policy_digest_hex must be 64 hex characters" in artifact["errors"]


def test_policy_config_requires_generated_at_for_aggregate_freshness(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = policy_config()
    del payload["generated_at_unix"]
    write_json(tmp_path / "policy-config.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["policy_config"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "generated_at_unix must be a positive integer" in artifact["errors"]


def test_quote_matrix_requires_policy_digest(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = quote_matrix()
    payload.pop("policy_digest_hex")
    write_json(tmp_path / "quote-matrix.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["quote_matrix"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "policy_digest_hex must be a non-empty string" in artifact["errors"]


def test_reserve_movement_must_match_valid_ledger_tuple(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = reserve_movement()
    payload["ledger_digest_hex"] = ALT_LEDGER_DIGEST
    write_json(tmp_path / "reserve-movement.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["reserve_movement"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "sorafs.reserve.reserve_movement_canary.v1 policy_digest_hex, "
        "matrix_digest_hex, and ledger_digest_hex must match a valid "
        "ledger_digest artifact"
    ]


def test_reserve_movement_requires_custody_finality_evidence(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = reserve_movement()
    payload["live_chain_submission_verified"] = False
    payload["chain_submission_count"] = 1
    payload["finality_poll_attempt_count"] = 1
    payload.pop("automatic_finality_polling_verified")
    payload["confirmed_custody_evidence_present"] = False
    payload["confirmed_balance_projection_verified"] = False
    payload.pop("chain_reconciled_readback_verified")
    write_json(tmp_path / "reserve-movement.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["reserve_movement"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert "live_chain_submission_verified must be true" in artifact["errors"]
    assert (
        "chain_submission_count must cover every accepted_movement_count"
        in artifact["errors"]
    )
    assert (
        "finality_poll_attempt_count must cover every accepted_movement_count"
        in artifact["errors"]
    )
    assert "automatic_finality_polling_verified must be true" in artifact["errors"]
    assert "confirmed_custody_evidence_present must be true" in artifact["errors"]
    assert "confirmed_balance_projection_verified must be true" in artifact["errors"]
    assert "chain_reconciled_readback_verified must be true" in artifact["errors"]


def test_reserve_movement_actions_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = reserve_movement()
    payload["movements"].append(dict(payload["movements"][0]))
    payload["movement_count"] = len(payload["movements"])
    payload["accepted_movement_count"] = len(payload["movements"])
    write_json(tmp_path / "reserve-movement.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["reserve_movement"]["artifacts"][0]
    assert "movements must not contain duplicate values" in artifact["errors"]
    assert "movement_count must match unique movements count" in artifact["errors"]


def test_reserve_movement_requires_action_coverage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = reserve_movement()
    missing = payload["movements"].pop()["action"]
    payload["movement_count"] = len(payload["movements"])
    payload["accepted_movement_count"] = len(payload["movements"])
    payload["chain_submission_count"] = len(payload["movements"])
    payload["finality_poll_attempt_count"] = len(payload["movements"])
    write_json(tmp_path / "reserve-movement.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["reserve_movement"]["artifacts"][0]
    assert f"movements must include action `{missing}`" in artifact["errors"]


def test_reserve_movement_actions_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = reserve_movement()
    payload["movements"].append({
        "action": "debug_movement",
        "accepted": True,
        "chain_submitted": True,
        "finality_confirmed": True,
        "custody_reconciled": True,
    })
    payload["movement_count"] = len(payload["movements"])
    payload["accepted_movement_count"] = len(payload["movements"])
    payload["chain_submission_count"] = len(payload["movements"])
    payload["finality_poll_attempt_count"] = len(payload["movements"])
    write_json(tmp_path / "reserve-movement.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["reserve_movement"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "movements must not include unknown values" in artifact["errors"]


def test_reserve_movement_accepted_count_must_match_rows(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = reserve_movement()
    payload["movements"][0]["accepted"] = False
    write_json(tmp_path / "reserve-movement.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["reserve_movement"]["artifacts"][0]
    assert "movements[0].accepted must be true" in artifact["errors"]
    assert (
        "accepted_movement_count must match accepted movements count"
        in artifact["errors"]
    )


def test_reserve_movement_payload_free_flags_are_required(tmp_path: Path) -> None:
    for field in ("raw_transfer_included", "raw_instruction_included"):
        root = tmp_path / field
        root.mkdir()
        write_complete_evidence(root)
        payload = reserve_movement()
        del payload[field]
        write_json(root / "reserve-movement.json", payload)
        summary = root / "summary.json"

        assert run_gate(root, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"]["reserve_movement"]["artifacts"][0]
        assert artifact["valid"] is False
        assert f"{field} must be false" in artifact["errors"]


def test_appeal_policy_probes_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = appeal_policy()
    payload["appeal_probes"].append(dict(payload["appeal_probes"][0]))
    payload["appeal_probe_count"] = len(payload["appeal_probes"])
    payload["approved_appeal_count"] = 2
    write_json(tmp_path / "appeal-policy.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["appeal_policy"]["artifacts"][0]
    assert "appeal_probes must not contain duplicate values" in artifact["errors"]
    assert (
        "appeal_probe_count must match unique appeal_probes count"
        in artifact["errors"]
    )


def test_appeal_policy_requires_probe_coverage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = appeal_policy()
    missing = payload["appeal_probes"].pop()["name"]
    payload["appeal_probe_count"] = len(payload["appeal_probes"])
    payload["rejected_appeal_count"] = 0
    write_json(tmp_path / "appeal-policy.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["appeal_policy"]["artifacts"][0]
    assert f"appeal_probes must include name `{missing}`" in artifact["errors"]


def test_appeal_policy_probes_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = appeal_policy()
    payload["appeal_probes"].append({
        "name": "debug_probe",
        "outcome": "approved",
        "governance_recorded": True,
        "policy_digest_bound": True,
    })
    payload["appeal_probe_count"] = len(payload["appeal_probes"])
    payload["approved_appeal_count"] = 2
    payload["rejected_appeal_count"] = 1
    write_json(tmp_path / "appeal-policy.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["appeal_policy"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "appeal_probes must not include unknown values" in artifact["errors"]


def test_appeal_policy_partition_counts_must_match_probe_outcomes(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = appeal_policy()
    payload["appeal_probes"][0]["outcome"] = "rejected"
    payload["approved_appeal_count"] = 1
    payload["rejected_appeal_count"] = 1
    write_json(tmp_path / "appeal-policy.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["appeal_policy"]["artifacts"][0]
    assert (
        "approved_appeal_count must match approved appeal probes count"
        in artifact["errors"]
    )
    assert (
        "rejected_appeal_count must match rejected appeal probes count"
        in artifact["errors"]
    )


def test_appeal_policy_probe_outcome_must_be_known(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = appeal_policy()
    payload["appeal_probes"][0]["outcome"] = "remanded"
    write_json(tmp_path / "appeal-policy.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["appeal_policy"]["artifacts"][0]
    assert "appeal_probes[0].outcome must be approved or rejected" in artifact[
        "errors"
    ]


def test_ledger_bound_subset_requires_ledger_anchor(tmp_path: Path) -> None:
    write_json(tmp_path / "metrics-alerts.json", metrics_alerts())
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-kind",
            "metrics_alerts",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["metrics_alerts"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "reserve rollout evidence requires a valid policy_config policy_digest_hex",
        "ledger-bound reserve evidence requires a valid quote_matrix "
        "policy_digest_hex/matrix_digest_hex tuple",
        "ledger-bound reserve evidence requires a valid ledger_digest "
        "policy_digest_hex/matrix_digest_hex/ledger_digest_hex tuple",
    ]


def test_unsigned_wrong_account_route_probe_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "signed-routes.json",
        signed_routes(wrong_account_rejected=False),
    )

    assert run_gate(tmp_path) == 1


def test_credit_line_requires_live_account_mutation_evidence(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = credit_line()
    payload["live_account_mutation_verified"] = False
    payload["credit_accrual_posted_to_account_state"] = False
    payload.pop("credit_line_account_state_readback_verified")
    write_json(tmp_path / "credit-line.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["credit_line"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert "live_account_mutation_verified must be true" in artifact["errors"]
    assert "credit_accrual_posted_to_account_state must be true" in artifact["errors"]
    assert (
        "credit_line_account_state_readback_verified must be true"
        in artifact["errors"]
    )


def test_credit_line_mutations_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = credit_line()
    payload["credit_line_mutations"].append(dict(payload["credit_line_mutations"][0]))
    payload["credit_line_mutation_count"] = len(payload["credit_line_mutations"])
    write_json(tmp_path / "credit-line.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["credit_line"]["artifacts"][0]
    assert "credit_line_mutations must not contain duplicate values" in artifact[
        "errors"
    ]
    assert (
        "credit_line_mutation_count must match unique "
        "credit_line_mutations count"
    ) in artifact["errors"]


def test_credit_line_requires_mutation_coverage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = credit_line()
    missing = payload["credit_line_mutations"].pop()["name"]
    payload["credit_line_mutation_count"] = len(payload["credit_line_mutations"])
    write_json(tmp_path / "credit-line.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["credit_line"]["artifacts"][0]
    assert (
        f"credit_line_mutations must include name `{missing}`"
        in artifact["errors"]
    )


def test_credit_line_mutations_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = credit_line()
    payload["credit_line_mutations"].append({
        "name": "debug_mutation",
        "verified": True,
    })
    payload["credit_line_mutation_count"] = len(payload["credit_line_mutations"])
    write_json(tmp_path / "credit-line.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["credit_line"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "credit_line_mutations must not include unknown values"
        in artifact["errors"]
    )


def test_credit_line_mutation_rows_must_be_verified(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = credit_line()
    payload["credit_line_mutations"][0]["verified"] = False
    write_json(tmp_path / "credit-line.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["credit_line"]["artifacts"][0]
    assert "credit_line_mutations[0].verified must be true" in artifact["errors"]


def test_credit_line_accrual_cycles_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = credit_line()
    payload["accrual_cycles"].append(dict(payload["accrual_cycles"][0]))
    payload["accrual_cycle_count"] = len(payload["accrual_cycles"])
    write_json(tmp_path / "credit-line.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["credit_line"]["artifacts"][0]
    assert "accrual_cycles must not contain duplicate values" in artifact["errors"]
    assert (
        "accrual_cycle_count must match unique accrual_cycles count"
        in artifact["errors"]
    )


def test_credit_line_requires_accrual_cycle_coverage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = credit_line()
    missing = payload["accrual_cycles"].pop()["name"]
    payload["accrual_cycle_count"] = len(payload["accrual_cycles"])
    write_json(tmp_path / "credit-line.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["credit_line"]["artifacts"][0]
    assert f"accrual_cycles must include name `{missing}`" in artifact["errors"]


def test_credit_line_accrual_cycles_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = credit_line()
    payload["accrual_cycles"].append({
        "name": "debug_accrual",
        "posted_to_account_state": True,
    })
    payload["accrual_cycle_count"] = len(payload["accrual_cycles"])
    write_json(tmp_path / "credit-line.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["credit_line"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "accrual_cycles must not include unknown values" in artifact["errors"]


def test_credit_line_accrual_cycles_must_post_to_account_state(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = credit_line()
    payload["accrual_cycles"][0]["posted_to_account_state"] = False
    write_json(tmp_path / "credit-line.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["credit_line"]["artifacts"][0]
    assert (
        "accrual_cycles[0].posted_to_account_state must be true"
        in artifact["errors"]
    )


def test_provider_bake_failure_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "provider-bake.json", provider_bake(failure_count=1))

    assert run_gate(tmp_path) == 1


def test_provider_bake_completed_at_must_not_precede_started_at(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["completed_at_unix"] = bake["started_at_unix"] - 1
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "completed_at_unix must be >= started_at_unix" in artifact["errors"]


def test_provider_bake_requires_scheduler_lifecycle_canary(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["scheduled_lifecycle_canary_passed"] = False
    bake.pop("scheduled_lifecycle_canary_defaulted_provider_count")
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "scheduled_lifecycle_canary_passed must be true" in artifact["errors"]
    assert (
        "scheduled_lifecycle_canary_defaulted_provider_count must be a positive integer"
        in artifact["errors"]
    )


def test_provider_bake_id_must_be_canonical(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["bake_id"] = "reserve_bake_001"
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert MODULE.BAKE_ID_ERROR in artifact["errors"]


def test_provider_bake_id_rejects_non_production_markers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["bake_id"] = "reserve-bake-prod-placeholder"
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert (
        "bake_id must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_provider_bake_id_accepts_future_production_label(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["bake_id"] = "reserve-bake-prod-a-202607"
    write_json(tmp_path / "provider-bake.json", bake)

    assert run_gate(tmp_path) == 0


def test_provider_bake_providers_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["providers"].append(dict(bake["providers"][1]))
    bake["provider_count"] = len(bake["providers"])
    bake["completed_provider_count"] = len(bake["providers"])
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert "providers must not contain duplicate values" in artifact["errors"]
    assert "provider_count must match unique providers count" in artifact["errors"]


def test_provider_bake_provider_names_must_be_reviewed_labels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["providers"][0] = {
        **bake["providers"][0],
        "name": "provider_alpha",
    }
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert (
        "providers[].name must match canonical lowercase `provider-*`"
        in artifact["errors"]
    )


def test_provider_bake_provider_names_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["providers"][0] = {
        **bake["providers"][0],
        "name": "provider-placeholder",
    }
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert (
        "providers[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_provider_bake_completed_count_must_match_provider_rows(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["providers"][0]["completed"] = False
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert "providers[0].completed must be true" in artifact["errors"]
    assert (
        "completed_provider_count must match completed providers count"
        in artifact["errors"]
    )


def test_provider_bake_defaulted_count_must_match_provider_rows(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["providers"][0]["defaulted"] = False
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert (
        "scheduled_lifecycle_canary_defaulted_provider_count must match "
        "defaulted providers count"
    ) in artifact["errors"]


def test_provider_bake_cycle_counts_must_match_unique_inventories(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["rent_cycles"].append(dict(bake["rent_cycles"][0]))
    bake["rent_cycle_count"] = len(bake["rent_cycles"])
    bake["top_up_cycle_count"] += 1
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert "rent_cycles must not contain duplicate values" in artifact["errors"]
    assert "rent_cycle_count must match unique rent_cycles count" in artifact["errors"]
    assert (
        "top_up_cycle_count must match unique top_up_cycles count"
        in artifact["errors"]
    )


def test_provider_bake_tick_count_must_match_unique_ticks(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["scheduled_lifecycle_canary_ticks"].pop()
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert (
        "scheduled_lifecycle_canary_tick_count must match unique "
        "scheduled_lifecycle_canary_ticks count"
    ) in artifact["errors"]


def test_provider_bake_ticks_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["scheduled_lifecycle_canary_ticks"].append(
        {"name": "reserve-lifecycle-tick-001"}
    )
    bake["scheduled_lifecycle_canary_tick_count"] = len(
        bake["scheduled_lifecycle_canary_ticks"]
    )
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert (
        "scheduled_lifecycle_canary_ticks must not contain duplicate values"
        in artifact["errors"]
    )


def test_provider_bake_ticks_must_use_reviewed_labels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["scheduled_lifecycle_canary_ticks"][0]["name"] = "lifecycle-tick-001"
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert MODULE.SCHEDULED_LIFECYCLE_TICK_LABEL_ERROR in artifact["errors"]


def test_provider_bake_ticks_reject_non_production_markers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["scheduled_lifecycle_canary_ticks"][0][
        "name"
    ] = "reserve-lifecycle-tick-placeholder"
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert (
        "scheduled_lifecycle_canary_ticks[0].name must not contain "
        "non-production markers ['placeholder']"
    ) in artifact["errors"]


@pytest.mark.parametrize(
    ("field", "replacement", "expected_error"),
    (
        (
            "rent_cycles",
            "rent-cycle-001",
            "rent_cycles[].name must match canonical lowercase `reserve-rent-cycle-*`",
        ),
        (
            "top_up_cycles",
            "top-up-cycle-001",
            "top_up_cycles[].name must match canonical lowercase `reserve-top-up-cycle-*`",
        ),
        (
            "appeal_cycles",
            "appeal-cycle-001",
            "appeal_cycles[].name must match canonical lowercase `reserve-appeal-cycle-*`",
        ),
    ),
)
def test_provider_bake_cycle_labels_must_use_reviewed_families(
    tmp_path: Path,
    field: str,
    replacement: str,
    expected_error: str,
) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake[field][0]["name"] = replacement
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert expected_error in artifact["errors"]


@pytest.mark.parametrize(
    ("field", "replacement", "expected_error"),
    (
        (
            "rent_cycles",
            "reserve-rent-cycle-placeholder",
            "rent_cycles[].name must not contain non-production markers ['placeholder']",
        ),
        (
            "top_up_cycles",
            "reserve-top-up-cycle-sample",
            "top_up_cycles[].name must not contain non-production markers ['sample']",
        ),
        (
            "appeal_cycles",
            "reserve-appeal-cycle-test",
            "appeal_cycles[].name must not contain non-production markers ['test']",
        ),
    ),
)
def test_provider_bake_cycle_labels_reject_non_production_markers(
    tmp_path: Path,
    field: str,
    replacement: str,
    expected_error: str,
) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake[field][0]["name"] = replacement
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert expected_error in artifact["errors"]


def test_provider_bake_cycle_rows_must_carry_proof_flags(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["rent_cycles"][0]["settled"] = False
    bake["top_up_cycles"][0]["reconciled"] = False
    bake["appeal_cycles"][0]["reviewed"] = False
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert "rent_cycles[0].settled must be true" in artifact["errors"]
    assert "top_up_cycles[0].reconciled must be true" in artifact["errors"]
    assert "appeal_cycles[0].reviewed must be true" in artifact["errors"]


def test_provider_bake_scheduler_lifecycle_canary_must_be_fresh(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    bake = provider_bake()
    bake["scheduled_lifecycle_canary_last_tick_unix"] = (
        bake["completed_at_unix"] - MODULE.DEFAULT_MAX_LIFECYCLE_LAG_SECS - 1
    )
    write_json(tmp_path / "provider-bake.json", bake)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider_bake"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "scheduled_lifecycle_canary_last_tick_unix must be within "
        f"{MODULE.DEFAULT_MAX_LIFECYCLE_LAG_SECS} seconds of completed_at_unix"
        in artifact["errors"]
    )


def test_governance_approval_requires_downstream_compliance_evidence(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_approval()
    payload["downstream_compliance_policy_applied"] = False
    payload["downstream_compliance_consumer_count"] = 0
    payload.pop("governance_source_entry_handoff_verified")
    write_json(tmp_path / "governance-approval.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_approval"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "downstream_compliance_policy_applied must be true" in artifact["errors"]
    assert (
        "downstream_compliance_consumer_count must be a positive integer"
        in artifact["errors"]
    )
    assert "governance_source_entry_handoff_verified must be true" in artifact["errors"]


def test_governance_approval_consumer_count_must_match_unique_consumers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_approval()
    payload["downstream_compliance_consumers"].pop()
    write_json(tmp_path / "governance-approval.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["governance_approval"]["artifacts"][0]
    assert (
        "downstream_compliance_consumer_count must match unique "
        "downstream_compliance_consumers count"
    ) in artifact["errors"]


def test_governance_approval_consumers_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_approval()
    payload["downstream_compliance_consumers"].append(
        {"name": "reserve-compliance-consumer-gateway"}
    )
    write_json(tmp_path / "governance-approval.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["governance_approval"]["artifacts"][0]
    assert (
        "downstream_compliance_consumers must not contain duplicate values"
        in artifact["errors"]
    )


def test_governance_approval_consumers_must_use_reviewed_labels(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_approval()
    payload["downstream_compliance_consumers"][0]["name"] = "consumer-gateway"
    write_json(tmp_path / "governance-approval.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["governance_approval"]["artifacts"][0]
    assert MODULE.DOWNSTREAM_COMPLIANCE_CONSUMER_LABEL_ERROR in artifact["errors"]


def test_governance_approval_consumers_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_approval()
    payload["downstream_compliance_consumers"][0][
        "name"
    ] = "reserve-compliance-consumer-placeholder"
    write_json(tmp_path / "governance-approval.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["governance_approval"]["artifacts"][0]
    assert (
        "downstream_compliance_consumers[0].name must not contain "
        "non-production markers ['placeholder']"
    ) in artifact["errors"]


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(tmp_path / "unknown.json", {"schema": "sorafs.reserve.unknown.v1"})

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_unknown_directory_artifact_is_ignored_for_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "policy-config.json", policy_config())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.reserve.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "policy_config") == 0


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "policy-config.json", policy_config())
    invalid = metrics_alerts()
    invalid["critical_alerts_firing"] = True
    write_json(tmp_path / "metrics-alerts.json", invalid)

    assert run_gate(tmp_path, "--require-kind", "policy_config") == 1


def test_metrics_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = metrics_alerts()
    payload["metrics"].append(payload["metrics"][0])
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "metrics-alerts.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["metrics_alerts"]["artifacts"][0]
    assert "metrics must not contain duplicate values" in artifact["errors"]
    assert "metric_count must match unique metrics count" in artifact["errors"]


def test_response_file_complete_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    args = tmp_path / "args.txt"
    args.write_text(
        "\n".join(
            [
                "# response file for the reserve gate",
                "--evidence-dir",
                str(tmp_path),
                "--summary-out",
                str(summary),
                "--now-unix",
                str(NOW_UNIX),
            ]
        ),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args}"]) == 0
    assert json.loads(summary.read_text(encoding="utf-8"))["status"] == "ready"


def test_unknown_required_kind_returns_usage_error(tmp_path: Path) -> None:
    write_json(tmp_path / "policy-config.json", policy_config())

    assert run_gate(tmp_path, "--require-kind", "unknown_kind") == 2
