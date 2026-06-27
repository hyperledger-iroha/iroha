"""Tests for scripts/check_sorafs_reserve_rent_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


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
DEPLOYMENT_ID = "reserve-prod-20260626"
ENVIRONMENT = "production"


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def with_reviewed_context(payload: dict) -> dict:
    payload = dict(payload)
    payload.setdefault("deployment_id", DEPLOYMENT_ID)
    payload.setdefault("environment", ENVIRONMENT)
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
        "instruction_count": 2,
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
    return with_reviewed_context({
        "schema": "sorafs.reserve.reserve_movement_canary.v1",
        "status": "passed",
        "policy_digest_hex": DIGEST,
        "matrix_digest_hex": MATRIX_DIGEST,
        "ledger_digest_hex": LEDGER_DIGEST,
        "movement_count": 4,
        "accepted_movement_count": 4,
        "failed_movement_count": 0,
        "unexpected_failure_count": 0,
        "rent_settlement_present": True,
        "reserve_top_up_present": True,
        "withdrawal_limits_enforced": True,
        "treasury_reconciliation_passed": True,
        "double_spend_rejected": True,
        "raw_transfer_included": False,
        "raw_instruction_included": False,
    })


def credit_line() -> dict:
    return with_reviewed_context({
        "schema": "sorafs.reserve.credit_line_canary.v1",
        "status": "passed",
        "policy_digest_hex": DIGEST,
        "matrix_digest_hex": MATRIX_DIGEST,
        "ledger_digest_hex": LEDGER_DIGEST,
        "credit_line_mutation_count": 2,
        "accrual_cycle_count": 2,
        "credit_draw_cap_enforced": True,
        "apr_accrual_verified": True,
        "manual_approval_tier_blocked": True,
        "credit_shortfall_reported": True,
        "no_negative_balance": True,
        "unexpected_failure_count": 0,
        "raw_ledger_included": False,
    })


def appeal_policy() -> dict:
    return with_reviewed_context({
        "schema": "sorafs.reserve.appeal_policy_canary.v1",
        "status": "passed",
        "policy_digest_hex": DIGEST,
        "matrix_digest_hex": MATRIX_DIGEST,
        "ledger_digest_hex": LEDGER_DIGEST,
        "appeal_probe_count": 2,
        "approved_appeal_count": 1,
        "rejected_appeal_count": 1,
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
        "sorafs_reserve_provider_balance_xor",
        "sorafs_reserve_lifecycle_stage",
        "sorafs_reserve_credit_line_draw_xor",
        "sorafs_reserve_appeal_backlog_total",
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
        "response_bodies_included": False,
    })


def provider_bake(*, failure_count: int = 0) -> dict:
    return with_reviewed_context({
        "schema": "sorafs.reserve.provider_bake.v1",
        "status": "passed",
        "policy_digest_hex": DIGEST,
        "matrix_digest_hex": MATRIX_DIGEST,
        "ledger_digest_hex": LEDGER_DIGEST,
        "bake_id": "reserve-bake-001",
        "started_at_unix": GENERATED_AT - 3_600,
        "completed_at_unix": GENERATED_AT,
        "provider_count": 3,
        "completed_provider_count": 3,
        "failure_count": failure_count,
        "rent_cycle_count": 2,
        "top_up_cycle_count": 2,
        "appeal_cycle_count": 1,
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
    assert payload["valid_policy_matrix_ledger_bindings"] == [
        {
            "policy_digest_hex": DIGEST,
            "matrix_digest_hex": MATRIX_DIGEST,
            "ledger_digest_hex": LEDGER_DIGEST,
        }
    ]
    assert payload["valid_provider_bakes"] == [
        {
            "bake_id": "reserve-bake-001",
            "started_at_unix": GENERATED_AT - 3_600,
            "completed_at_unix": GENERATED_AT,
            "provider_count": 3,
        }
    ]


def test_missing_signed_routes_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "signed-routes.json").unlink()

    assert run_gate(tmp_path) == 1


def test_stale_ledger_digest_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    stale = NOW_UNIX - MODULE.DEFAULT_MAX_LEDGER_AGE_SECS - 1
    write_json(tmp_path / "ledger-digest.json", ledger_digest(generated_at=stale))

    assert run_gate(tmp_path) == 1


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
