"""Tests for scripts/check_sorafs_repair_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_repair_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location("check_sorafs_repair_rollout_evidence", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


NOW_UNIX = 1_800_400_000
GENERATED_AT = NOW_UNIX - 120
DIGEST = "ab" * 32
DIGEST_2 = "cd" * 32


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def base(schema: str) -> dict:
    return {
        "schema": schema,
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "deployment_id": "repair-staging-a",
        "environment": "staging",
        "deployment_context_reviewed": True,
    }


def auditor_roster(*, auditor_count: int = 3) -> dict:
    payload = base("sorafs.repair.auditor_roster_canary.v1")
    payload.update(
        {
            "roster_published": True,
            "roster_signature_verified": True,
            "sf9_coordinator_bound": True,
            "runbook_published": True,
            "auditor_notifications_configured": True,
            "auditor_count": auditor_count,
            "roster_digest_hex": DIGEST,
            "raw_roster_included": False,
        }
    )
    return payload


def failure_capture() -> dict:
    payload = base("sorafs.repair.failure_capture_canary.v1")
    payload.update(
        {
            "failure_sources": ["por", "potr"],
            "por_history_replayed": True,
            "potr_receipt_replayed": True,
            "coordinator_event_verified": True,
            "merkle_or_receipt_inclusion_verified": True,
            "object_storage_retention_bound": True,
            "failure_event_count": 2,
            "evidence_bundle_digest_hex": DIGEST,
            "raw_evidence_included": False,
        }
    )
    return payload


def route(name: str, *, authz: bool = True, latency_ms: int = 200) -> dict:
    return {
        "name": name,
        "passed": True,
        "status_code": 200,
        "latency_ms": latency_ms,
        "authz_enforced": authz,
        "signature_verified": True,
    }


def auditor_api(*, authz: bool = True) -> dict:
    routes = [
        route(name, authz=authz)
        for name in (
            "repair_report",
            "repair_slash",
            "repair_status",
            "repair_status_manifest",
        )
    ]
    payload = base("sorafs.repair.auditor_api_canary.v1")
    payload.update(
        {
            "route_count": len(routes),
            "passed_route_count": len(routes),
            "roster_digest_hex": DIGEST,
            "signed_auditor_envelope_required": True,
            "nonce_replay_rejected": True,
            "legacy_raw_payload_rejected": True,
            "per_auditor_rate_limit_enforced": True,
            "response_bodies_included": False,
            "routes": routes,
        }
    )
    return payload


def worker_lifecycle(*, repair_latency_seconds: int = 900, missing_status: bool = False) -> dict:
    routes = [
        route(name)
        for name in (
            "repair_claim",
            "repair_heartbeat",
            "repair_complete",
            "repair_fail",
        )
    ]
    statuses = ["queued", "in_progress", "completed", "escalated"]
    if missing_status:
        statuses.remove("escalated")
    payload = base("sorafs.repair.worker_lifecycle_canary.v1")
    payload.update(
        {
            "route_count": len(routes),
            "passed_route_count": len(routes),
            "roster_digest_hex": DIGEST,
            "evidence_bundle_digest_hex": DIGEST,
            "statuses_observed": statuses,
            "worker_permission_enforced": True,
            "lease_heartbeat_enforced": True,
            "idempotency_enforced": True,
            "norito_snapshot_persisted": True,
            "gc_protection_verified": True,
            "repair_latency_seconds": repair_latency_seconds,
            "raw_repair_payloads_included": False,
            "routes": routes,
        }
    )
    return payload


def event_streams(*, event_lag_seconds: int = 30) -> dict:
    routes = [
        route(name)
        for name in ("repair_events", "repair_events_sse", "repair_events_ws")
    ]
    payload = base("sorafs.repair.event_streams_canary.v1")
    payload.update(
        {
            "route_count": len(routes),
            "passed_route_count": len(routes),
            "roster_digest_hex": DIGEST,
            "evidence_bundle_digest_hex": DIGEST,
            "backlog_replay_verified": True,
            "sse_delivery_verified": True,
            "websocket_delivery_verified": True,
            "event_lag_seconds": event_lag_seconds,
            "response_bodies_included": False,
            "routes": routes,
        }
    )
    return payload


def governance_handoff() -> dict:
    payload = base("sorafs.repair.governance_handoff_canary.v1")
    payload.update(
        {
            "roster_digest_hex": DIGEST,
            "evidence_bundle_digest_hex": DIGEST,
            "slash_proposal_generated": True,
            "governance_dag_published": True,
            "escalation_policy_enforced": True,
            "appeal_window_enforced": True,
            "reserve_rent_handoff_verified": True,
            "transparency_publication_verified": True,
            "reputation_handoff_verified": True,
            "handoff_targets": [
                "governance_dag",
                "repair_slash_proposal",
                "reserve_rent",
                "transparency_ledger",
                "reputation",
            ],
            "handoff_digest_hex": DIGEST,
            "raw_ledger_included": False,
        }
    )
    return payload


def observability(*, critical: bool = False) -> dict:
    payload = base("sorafs.repair.observability_canary.v1")
    payload.update(
        {
            "metrics_scrape_success": True,
            "dashboard_provisioned": True,
            "alert_rules_installed": True,
            "critical_alerts_firing": critical,
            "metrics": [
                "torii_sorafs_repair_tasks_total",
                "torii_sorafs_repair_latency_minutes_bucket",
                "torii_sorafs_repair_queue_depth",
                "torii_sorafs_repair_backlog_oldest_age_seconds",
                "torii_sorafs_repair_lease_expired_total",
                "torii_sorafs_slash_proposals_total",
            ],
            "response_bodies_included": False,
        }
    )
    return payload


def governance_approval() -> dict:
    payload = base("sorafs.repair.governance_approval.v1")
    payload.update(
        {
            "approved": True,
            "governance_vote_recorded": True,
            "iroha_config_bound": True,
            "repair_policy_bound": True,
            "auditor_roster_bound": True,
            "roster_digest_hex": DIGEST,
            "slash_policy_bound": True,
            "config_source": "iroha_config",
            "policy_digest_hex": DIGEST,
        }
    )
    return payload


def write_complete_evidence(root: Path) -> None:
    write_json(root / "auditor-roster.json", auditor_roster())
    write_json(root / "failure-capture.json", failure_capture())
    write_json(root / "auditor-api.json", auditor_api())
    write_json(root / "worker-lifecycle.json", worker_lifecycle())
    write_json(root / "event-streams.json", event_streams())
    write_json(root / "governance-handoff.json", governance_handoff())
    write_json(root / "observability.json", observability())
    write_json(root / "governance-approval.json", governance_approval())


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.repair.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["failure_capture"]["valid"] is True


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "repair.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args}"]) == 0


def test_missing_failure_capture_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "failure-capture.json").unlink()

    assert run_gate(tmp_path) == 1


def test_stale_auditor_roster_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = auditor_roster()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "auditor-roster.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["auditor_api"]
    artifact = required["artifacts"][0]
    assert payload["valid_roster_digests"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "auditor_api roster_digest_hex requires a valid auditor_roster roster_digest_hex"
    ]


def test_raw_evidence_leakage_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = failure_capture()
    payload["raw_evidence"] = {"por": "leaked"}
    write_json(tmp_path / "failure-capture.json", payload)

    assert run_gate(tmp_path) == 1


def test_auditor_roster_requires_minimum_auditors(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "auditor-roster.json", auditor_roster(auditor_count=2))

    assert run_gate(tmp_path) == 1


def test_auditor_api_without_authz_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "auditor-api.json", auditor_api(authz=False))

    assert run_gate(tmp_path) == 1


def test_auditor_api_requires_roster_digest_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = auditor_api()
    del payload["roster_digest_hex"]
    write_json(tmp_path / "auditor-api.json", payload)

    assert run_gate(tmp_path) == 1


def test_worker_lifecycle_roster_digest_must_match_roster(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = worker_lifecycle()
    payload["roster_digest_hex"] = DIGEST_2
    write_json(tmp_path / "worker-lifecycle.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["worker_lifecycle"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "worker_lifecycle roster_digest_hex must reference a valid auditor_roster roster_digest_hex"
    ]


def test_governance_handoff_failure_digest_must_match_capture(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_handoff()
    payload["evidence_bundle_digest_hex"] = DIGEST_2
    write_json(tmp_path / "governance-handoff.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["governance_handoff"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "governance_handoff evidence_bundle_digest_hex must reference a valid "
        "failure_capture evidence_bundle_digest_hex"
    ]


def test_stale_failure_capture_does_not_anchor_failure_bound_evidence(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = failure_capture()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "failure-capture.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["worker_lifecycle"]
    artifact = required["artifacts"][0]
    assert payload["valid_failure_bundle_digests"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "worker_lifecycle evidence_bundle_digest_hex requires a valid failure_capture "
        "evidence_bundle_digest_hex"
    ]


def test_repair_latency_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "worker-lifecycle.json", worker_lifecycle(repair_latency_seconds=10_000))

    assert run_gate(tmp_path) == 1


def test_worker_lifecycle_requires_escalation_status(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "worker-lifecycle.json", worker_lifecycle(missing_status=True))

    assert run_gate(tmp_path) == 1


def test_event_lag_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "event-streams.json", event_streams(event_lag_seconds=10_000))

    assert run_gate(tmp_path) == 1


def test_observability_critical_alert_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path) == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(tmp_path / "unknown.json", {"schema": "sorafs.repair.unknown.v1"})

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_unknown_directory_artifact_is_ignored_for_subset(tmp_path: Path) -> None:
    write_json(tmp_path / "auditor-roster.json", auditor_roster())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.repair.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "auditor_roster") == 0


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "auditor-roster.json", auditor_roster())
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path, "--require-kind", "auditor_roster") == 1


def test_unknown_required_kind_returns_usage_error(tmp_path: Path) -> None:
    assert MODULE.main(["--evidence-dir", str(tmp_path), "--require-kind", "unknown"]) == 2
