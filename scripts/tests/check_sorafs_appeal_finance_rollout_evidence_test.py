"""Tests for scripts/check_sorafs_appeal_finance_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_appeal_finance_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_appeal_finance_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


NOW_UNIX = 1_800_200_000
GENERATED_AT = NOW_UNIX - 120
DIGEST = "ab" * 32
DIGEST_2 = "cd" * 32


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def route(name: str, *, authz: bool = True) -> dict:
    return {
        "name": name,
        "passed": True,
        "status_code": 200,
        "authz_enforced": authz,
        "signature_verified": True,
        "latency_ms": 30,
    }


def pricing_config() -> dict:
    return {
        "schema": "sorafs.appeal_finance.pricing_config_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "config_digest_hex": DIGEST,
        "config_version": "baseline-v1",
        "config_source": "iroha_config",
        "class_count": 4,
        "pricing_config_present": True,
        "settlement_config_present": True,
        "quote_ttl_present": True,
        "default_panel_size_present": True,
        "config_route_2xx": True,
        "status_route_2xx": True,
        "config_payload_included": False,
        "response_bodies_included": False,
    }


def quote_api() -> dict:
    routes = [
        route("pricing_quote", authz=False),
        route("finance_settle", authz=False),
        route("finance_disburse", authz=False),
    ]
    return {
        "schema": "sorafs.appeal_finance.quote_api_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "config_digest_hex": DIGEST,
        "route_count": len(routes),
        "passed_route_count": len(routes),
        "routes": routes,
        "quote_count": 8,
        "passed_quote_count": 8,
        "classes": ["content", "access", "fraud", "other"],
        "urgencies": ["normal", "high"],
        "deterministic_replay_passed": True,
        "deposit_bounds_enforced": True,
        "max_route_latency_ms": 250,
        "payloads_included": False,
        "response_bodies_included": False,
    }


def deposit_lifecycle(*, authz: bool = True) -> dict:
    routes = [
        route("deposit_create", authz=authz),
        route("deposit_status", authz=authz),
        route("deposit_confirm", authz=authz),
        route("ballot_announcement_gate", authz=authz),
    ]
    return {
        "schema": "sorafs.appeal_finance.deposit_lifecycle_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "config_digest_hex": DIGEST,
        "route_count": len(routes),
        "passed_route_count": len(routes),
        "routes": routes,
        "deposit_probe_count": 2,
        "confirmed_deposit_count": 2,
        "payer_auth_enforced": True,
        "participant_status_gate_enforced": True,
        "mismatched_escrow_rejected": True,
        "unconfirmed_ballot_rejected": True,
        "ledger_lock_confirmed": True,
        "idempotency_key_bound": True,
        "evidence_hashes_bound": True,
        "max_route_latency_ms": 300,
        "raw_instruction_included": False,
        "deposit_payloads_included": False,
        "response_bodies_included": False,
    }


def settlement_execution() -> dict:
    routes = [
        route("settle_plan"),
        route("disburse_plan"),
        route("deposit_settle"),
        route("deposit_reconcile"),
    ]
    return {
        "schema": "sorafs.appeal_finance.settlement_execution_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "config_digest_hex": DIGEST,
        "route_count": len(routes),
        "passed_route_count": len(routes),
        "routes": routes,
        "settlement_probe_count": 7,
        "instruction_step_count": 2,
        "outcomes": [
            "uphold",
            "overturn",
            "modify",
            "withdrawn_before_panel",
            "withdrawn_after_panel",
            "frivolous",
            "escalated",
        ],
        "reconciliation_statuses": [
            "pending_client_submission",
            "awaiting_refund_cancel",
            "settled",
            "mismatch",
        ],
        "drawdown_instruction_present": True,
        "cancel_instruction_present": True,
        "required_signer_bound": True,
        "deterministic_reconciliation_digest": True,
        "treasury_reconciliation_passed": True,
        "mismatched_ledger_rejected": True,
        "raw_instruction_included": False,
        "signed_transaction_included": False,
        "response_bodies_included": False,
    }


def settlement_submitter() -> dict:
    return {
        "schema": "sorafs.appeal_finance.settlement_submitter_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "config_digest_hex": DIGEST,
        "configured_signer_count": 2,
        "queued_step_count": 2,
        "submitted_step_count": 2,
        "receipt_published": True,
        "required_authority_matched": True,
        "missing_signer_rejected": True,
        "wrong_authority_rejected": True,
        "rejected_or_expired_retry_verified": True,
        "max_settlement_lag_seconds": 60,
        "raw_receipt_included": False,
        "signed_transaction_included": False,
    }


def moderation_worker() -> dict:
    return {
        "schema": "sorafs.appeal_finance.moderation_worker_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "config_digest_hex": DIGEST,
        "worker_enabled": True,
        "storage_configured": True,
        "submitter_keys_configured": True,
        "ballot_replay_count": 3,
        "live_event_subscription_verified": True,
        "deposit_fingerprint_reconstructed": True,
        "evidence_hashes_verified": True,
        "runtime_ledger_validated": True,
        "pending_step_queued": True,
        "idempotent_rescan_verified": True,
        "retry_cap_enforced": True,
        "max_settlement_lag_seconds": 60,
        "raw_ballot_included": False,
        "deposit_confirmation_payload_included": False,
    }


def governance_dag_publication() -> dict:
    return {
        "schema": "sorafs.appeal_finance.governance_dag_publication_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "config_digest_hex": DIGEST,
        "report_count": 2,
        "weekly_rollup_count": 1,
        "settlement_receipt_count": 2,
        "payload_kinds": [
            "appeal_finance_report",
            "appeal_finance_weekly_rollup",
            "appeal_finance_settlement_receipt",
        ],
        "publish_index_verified": True,
        "canonical_to_payloads_verified": True,
        "json_sidecars_verified": True,
        "blake3_sidecars_verified": True,
        "car_queue_verified": True,
        "runtime_signed_dag_verified": True,
        "report_publish_auth_enforced": True,
        "rollup_publish_auth_enforced": True,
        "raw_report_included": False,
        "raw_rollup_included": False,
        "raw_receipt_included": False,
    }


def dashboard_metrics(*, generated_at: int = GENERATED_AT, include_all_metrics: bool = True) -> dict:
    metrics = [
        "sorafs_governance_dag_publish_total",
        "sorafs_governance_dag_last_publish_timestamp_seconds",
        "sorafs_governance_dag_published_bytes_total",
        "sorafs_governance_dag_backlog",
    ]
    if not include_all_metrics:
        metrics.pop()
    return {
        "schema": "sorafs.appeal_finance.dashboard_metrics_canary.v1",
        "status": "passed",
        "generated_at_unix": generated_at,
        "config_digest_hex": DIGEST,
        "metrics_scrape_success": True,
        "dashboard_provisioned": True,
        "alert_rules_installed": True,
        "hosted_public_dashboard_verified": True,
        "critical_alerts_firing": False,
        "metrics": metrics,
        "payload_kinds": [
            "appeal_finance_report",
            "appeal_finance_weekly_rollup",
            "appeal_finance_settlement_receipt",
        ],
        "response_bodies_included": False,
    }


def multi_peer_reconciliation(*, peer_count: int = 4) -> dict:
    return {
        "schema": "sorafs.appeal_finance.multi_peer_reconciliation_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "config_digest_hex": DIGEST,
        "peer_count": peer_count,
        "validator_count": peer_count,
        "case_count": 2,
        "deposit_posted": True,
        "decision_ingested": True,
        "settlement_submitted": True,
        "disbursement_verified": True,
        "treasury_reconciliation_passed": True,
        "governance_dag_receipt_verified": True,
        "all_peers_reconciled": True,
        "qc_quorum_satisfied": True,
        "mismatch_count": 0,
        "unexpected_failure_count": 0,
        "raw_ledger_included": False,
    }


def governance_approval() -> dict:
    return {
        "schema": "sorafs.appeal_finance.governance_approval.v1",
        "status": "passed",
        "approved": True,
        "governance_vote_recorded": True,
        "iroha_config_bound": True,
        "pricing_policy_present": True,
        "config_digest_hex": DIGEST,
        "settlement_policy_present": True,
        "deposit_custody_policy_present": True,
        "settlement_submitter_policy_present": True,
        "worker_retry_policy_present": True,
        "public_dashboard_rollout_accepted": True,
        "multi_peer_reconciliation_accepted": True,
        "config_source": "iroha_config",
        "policy_digest_hex": DIGEST,
    }


def write_complete_evidence(root: Path) -> None:
    write_json(root / "pricing-config.json", pricing_config())
    write_json(root / "quote-api.json", quote_api())
    write_json(root / "deposit-lifecycle.json", deposit_lifecycle())
    write_json(root / "settlement-execution.json", settlement_execution())
    write_json(root / "settlement-submitter.json", settlement_submitter())
    write_json(root / "moderation-worker.json", moderation_worker())
    write_json(root / "governance-dag-publication.json", governance_dag_publication())
    write_json(root / "dashboard-metrics.json", dashboard_metrics())
    write_json(root / "multi-peer-reconciliation.json", multi_peer_reconciliation())
    write_json(root / "governance-approval.json", governance_approval())


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.appeal_finance.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["multi_peer_reconciliation"]["valid"] is True
    assert len(payload["valid_multi_peer_runs"]) == 1


def test_missing_multi_peer_reconciliation_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "multi-peer-reconciliation.json").unlink()

    assert run_gate(tmp_path) == 1


def test_stale_dashboard_metrics_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    stale = NOW_UNIX - MODULE.DEFAULT_MAX_DASHBOARD_AGE_SECS - 1
    write_json(tmp_path / "dashboard-metrics.json", dashboard_metrics(generated_at=stale))

    assert run_gate(tmp_path) == 1


def test_payload_leakage_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = settlement_execution()
    payload["signed_transaction"] = "not-for-evidence"
    write_json(tmp_path / "settlement-execution.json", payload)

    assert run_gate(tmp_path) == 1


def test_quote_api_requires_config_digest_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = quote_api()
    del payload["config_digest_hex"]
    write_json(tmp_path / "quote-api.json", payload)

    assert run_gate(tmp_path) == 1


def test_settlement_execution_config_digest_must_match_pricing(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = settlement_execution()
    payload["config_digest_hex"] = DIGEST_2
    write_json(tmp_path / "settlement-execution.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["settlement_execution"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "settlement_execution config_digest_hex must reference a valid "
        "pricing_config config_digest_hex"
    ]


def test_stale_pricing_config_does_not_anchor_config_bound_evidence(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = pricing_config()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_CANARY_AGE_SECS - 1
    write_json(tmp_path / "pricing-config.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["quote_api"]
    artifact = required["artifacts"][0]
    assert payload["valid_config_digests"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "quote_api config_digest_hex requires a valid pricing_config config_digest_hex"
    ]


def test_missing_required_metric_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "dashboard-metrics.json",
        dashboard_metrics(include_all_metrics=False),
    )

    assert run_gate(tmp_path) == 1


def test_deposit_authz_probe_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "deposit-lifecycle.json", deposit_lifecycle(authz=False))

    assert run_gate(tmp_path) == 1


def test_multi_peer_count_below_minimum_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "multi-peer-reconciliation.json",
        multi_peer_reconciliation(peer_count=3),
    )

    assert run_gate(tmp_path) == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(
        tmp_path / "unknown.json",
        {"schema": "sorafs.appeal_finance.unknown.v1"},
    )

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_unknown_directory_artifact_is_ignored_for_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "pricing-config.json", pricing_config())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.appeal_finance.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "pricing_config") == 0


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "pricing-config.json", pricing_config())
    invalid = dashboard_metrics()
    invalid["critical_alerts_firing"] = True
    write_json(tmp_path / "dashboard-metrics.json", invalid)

    assert run_gate(tmp_path, "--require-kind", "pricing_config") == 1


def test_response_file_complete_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    args = tmp_path / "args.txt"
    args.write_text(
        "\n".join(
            [
                "# response file for the appeal finance gate",
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
    write_json(tmp_path / "pricing-config.json", pricing_config())

    assert run_gate(tmp_path, "--require-kind", "unknown_kind") == 2
