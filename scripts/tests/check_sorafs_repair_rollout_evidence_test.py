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
    auditors = [
        {"name": f"repair-auditor-{index:02d}"}
        for index in range(auditor_count)
    ]
    payload = base("sorafs.repair.auditor_roster_canary.v1")
    payload.update(
        {
            "roster_published": True,
            "roster_signature_verified": True,
            "sf9_coordinator_bound": True,
            "runbook_published": True,
            "auditor_notifications_configured": True,
            "auditor_count": auditor_count,
            "auditors": auditors,
            "roster_digest_hex": DIGEST,
            "raw_roster_included": False,
        }
    )
    return payload


def failure_capture() -> dict:
    failure_sources = ["por", "potr"]
    failure_events = [
        {"name": "repair-failure-event-por-00", "source": "por"},
        {"name": "repair-failure-event-potr-00", "source": "potr"},
    ]
    payload = base("sorafs.repair.failure_capture_canary.v1")
    payload.update(
        {
            "failure_sources": failure_sources,
            "failure_source_count": len(failure_sources),
            "por_history_replayed": True,
            "potr_receipt_replayed": True,
            "coordinator_event_verified": True,
            "merkle_or_receipt_inclusion_verified": True,
            "object_storage_retention_bound": True,
            "failure_event_count": len(failure_events),
            "failure_events": failure_events,
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
        "body_blake3_hex": DIGEST,
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
            "status_count": len(statuses),
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


def governance_handoff(*, handoff_digest: str = DIGEST) -> dict:
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
            "handoff_target_count": 5,
            "handoff_targets": [
                "governance_dag",
                "repair_slash_proposal",
                "reserve_rent",
                "transparency_ledger",
                "reputation",
            ],
            "handoff_digest_hex": handoff_digest,
            "policy_digest_hex": DIGEST,
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
            "metric_count": len(MODULE.REQUIRED_METRICS),
            "response_bodies_included": False,
        }
    )
    return payload


def governance_approval(*, handoff_digest: str = DIGEST) -> dict:
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
            "handoff_digest_hex": handoff_digest,
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


ROSTER_BOUND_FIXTURES = (
    ("auditor_api", "auditor-api.json", auditor_api),
    ("worker_lifecycle", "worker-lifecycle.json", worker_lifecycle),
    ("event_streams", "event-streams.json", event_streams),
    ("governance_handoff", "governance-handoff.json", governance_handoff),
    ("governance_approval", "governance-approval.json", governance_approval),
)

FAILURE_BOUND_FIXTURES = (
    ("worker_lifecycle", "worker-lifecycle.json", worker_lifecycle),
    ("event_streams", "event-streams.json", event_streams),
    ("governance_handoff", "governance-handoff.json", governance_handoff),
)

HANDOFF_BOUND_FIXTURES = (
    ("governance_approval", "governance-approval.json", governance_approval),
)

POLICY_BOUND_FIXTURES = (
    ("governance_approval", "governance-approval.json", governance_approval),
)


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
    assert payload["valid_handoff_digests"] == [DIGEST]
    assert payload["valid_policy_digests"] == [DIGEST]
    assert payload["metrics"] == sorted(MODULE.REQUIRED_METRICS)
    assert payload["metric_count_values"] == [len(MODULE.REQUIRED_METRICS)]
    observability_artifact = payload["required"]["observability"]["artifacts"][0]
    assert observability_artifact["fingerprint"]["metric_count"] == len(
        MODULE.REQUIRED_METRICS
    )
    assert observability_artifact["fingerprint"]["metrics"] == list(
        MODULE.REQUIRED_METRICS
    )


def test_bound_fixture_tables_cover_checker_bound_kind_sets() -> None:
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in ROSTER_BOUND_FIXTURES)
        == MODULE.ROSTER_BOUND_KINDS
    )
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in FAILURE_BOUND_FIXTURES)
        == MODULE.FAILURE_BOUND_KINDS
    )
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in HANDOFF_BOUND_FIXTURES)
        == MODULE.HANDOFF_BOUND_KINDS
    )
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in POLICY_BOUND_FIXTURES)
        == MODULE.POLICY_BOUND_KINDS
    )


def test_fixture_inventories_cover_checker_required_sets() -> None:
    assert tuple(source for source in failure_capture()["failure_sources"]) == (
        MODULE.REQUIRED_FAILURE_SOURCES
    )
    assert tuple(route["name"] for route in auditor_api()["routes"]) == (
        MODULE.REQUIRED_AUDITOR_ROUTES
    )
    assert tuple(route["name"] for route in worker_lifecycle()["routes"]) == (
        MODULE.REQUIRED_WORKER_ROUTES
    )
    assert tuple(worker_lifecycle()["statuses_observed"]) == (
        MODULE.REQUIRED_LIFECYCLE_STATUSES
    )
    assert tuple(route["name"] for route in event_streams()["routes"]) == (
        MODULE.REQUIRED_EVENT_ROUTES
    )
    assert tuple(governance_handoff()["handoff_targets"]) == (
        MODULE.REQUIRED_GOVERNANCE_TARGETS
    )
    assert tuple(observability()["metrics"]) == MODULE.REQUIRED_METRICS


def test_payload_safety_flags_are_required(tmp_path: Path) -> None:
    cases = (
        (
            "auditor-roster.json",
            "auditor_roster",
            auditor_roster,
            ("raw_roster_included",),
        ),
        (
            "failure-capture.json",
            "failure_capture",
            failure_capture,
            ("raw_evidence_included",),
        ),
        (
            "auditor-api.json",
            "auditor_api",
            auditor_api,
            ("response_bodies_included",),
        ),
        (
            "worker-lifecycle.json",
            "worker_lifecycle",
            worker_lifecycle,
            ("raw_repair_payloads_included",),
        ),
        (
            "event-streams.json",
            "event_streams",
            event_streams,
            ("response_bodies_included",),
        ),
        (
            "governance-handoff.json",
            "governance_handoff",
            governance_handoff,
            ("raw_ledger_included",),
        ),
        (
            "observability.json",
            "observability",
            observability,
            ("critical_alerts_firing", "response_bodies_included"),
        ),
    )

    for artifact_file, kind, make_payload, fields in cases:
        for field in fields:
            case_dir = tmp_path / kind / field
            case_dir.mkdir(parents=True)
            write_complete_evidence(case_dir)
            payload = make_payload()
            payload.pop(field)
            write_json(case_dir / artifact_file, payload)
            summary = case_dir / "summary.json"

            assert run_gate(case_dir, "--summary-out", str(summary)) == 1

            result = json.loads(summary.read_text(encoding="utf-8"))
            artifact = result["required"][kind]["artifacts"][0]
            assert artifact["valid"] is False
            assert f"{field} must be false" in artifact["errors"]


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


def test_auditor_roster_auditor_count_must_match_unique_auditors(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = auditor_roster()
    payload["auditor_count"] += 1
    write_json(tmp_path / "auditor-roster.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["auditor_roster"]["artifacts"][0]
    assert "auditor_count must match unique auditors count" in artifact["errors"]


def test_auditor_roster_auditors_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = auditor_roster()
    payload["auditors"].append(dict(payload["auditors"][0]))
    payload["auditor_count"] = len(payload["auditors"])
    write_json(tmp_path / "auditor-roster.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["auditor_roster"]["artifacts"][0]
    assert "auditors must not contain duplicate values" in artifact["errors"]
    assert "auditor_count must match unique auditors count" in artifact["errors"]


def test_auditor_roster_auditor_labels_must_use_production_family(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = auditor_roster()
    payload["auditors"][0]["name"] = "auditor-00"
    write_json(tmp_path / "auditor-roster.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["auditor_roster"]["artifacts"][0]
    assert MODULE.AUDITOR_LABEL_ERROR in artifact["errors"]


def test_auditor_roster_auditor_labels_reject_placeholder_marker(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = auditor_roster()
    payload["auditors"][0]["name"] = "repair-auditor-placeholder"
    write_json(tmp_path / "auditor-roster.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["auditor_roster"]["artifacts"][0]
    assert (
        "auditors[0].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_failure_sources_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = failure_capture()
    payload["failure_sources"].append(payload["failure_sources"][0])
    write_json(tmp_path / "failure-capture.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["failure_capture"]["artifacts"][0]
    assert "failure_sources must not contain duplicate values" in artifact["errors"]


def test_failure_source_count_must_match_unique_sources(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = failure_capture()
    payload["failure_source_count"] += 1
    write_json(tmp_path / "failure-capture.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["failure_capture"]["artifacts"][0]
    assert "failure_source_count must match unique failure_sources count" in artifact[
        "errors"
    ]


def test_failure_sources_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = failure_capture()
    payload["failure_sources"].append("pdp")
    payload["failure_source_count"] = len(payload["failure_sources"])
    write_json(tmp_path / "failure-capture.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["failure_capture"]["artifacts"][0]
    assert "failure_sources must not include unknown values" in artifact["errors"]


def test_failure_event_count_must_match_unique_events(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = failure_capture()
    payload["failure_event_count"] += 1
    write_json(tmp_path / "failure-capture.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["failure_capture"]["artifacts"][0]
    assert "failure_event_count must match unique failure_events count" in artifact[
        "errors"
    ]


def test_failure_events_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = failure_capture()
    payload["failure_events"].append(dict(payload["failure_events"][0]))
    payload["failure_event_count"] = len(payload["failure_events"])
    write_json(tmp_path / "failure-capture.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["failure_capture"]["artifacts"][0]
    assert "failure_events must not contain duplicate values" in artifact["errors"]
    assert "failure_event_count must match unique failure_events count" in artifact[
        "errors"
    ]


def test_failure_events_must_cover_required_sources(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = failure_capture()
    payload["failure_events"] = [payload["failure_events"][0]]
    payload["failure_event_count"] = len(payload["failure_events"])
    write_json(tmp_path / "failure-capture.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["failure_capture"]["artifacts"][0]
    assert "failure_events must include source `potr`" in artifact["errors"]


def test_failure_events_must_use_reviewed_sources(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = failure_capture()
    payload["failure_events"][0]["source"] = "unknown"
    write_json(tmp_path / "failure-capture.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["failure_capture"]["artifacts"][0]
    assert "failure_events source must be one of failure_sources" in artifact["errors"]


def test_failure_events_must_use_production_family(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = failure_capture()
    payload["failure_events"][0]["name"] = "por-failure-00"
    write_json(tmp_path / "failure-capture.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["failure_capture"]["artifacts"][0]
    assert (
        "failure_events[].name must match canonical lowercase "
        "`repair-failure-event-name`"
    ) in artifact["errors"]


def test_failure_events_reject_placeholder_marker(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = failure_capture()
    payload["failure_events"][0]["name"] = "repair-failure-event-placeholder"
    write_json(tmp_path / "failure-capture.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["failure_capture"]["artifacts"][0]
    assert (
        "failure_events[0].name must not contain non-production markers "
        "['placeholder']"
    ) in artifact["errors"]


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


def test_route_count_must_match_unique_routes_for_route_artifacts(
    tmp_path: Path,
) -> None:
    route_artifacts = (
        ("auditor_api", "auditor-api.json", auditor_api),
        ("worker_lifecycle", "worker-lifecycle.json", worker_lifecycle),
        ("event_streams", "event-streams.json", event_streams),
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
        ("auditor_api", "auditor-api.json", auditor_api),
        ("worker_lifecycle", "worker-lifecycle.json", worker_lifecycle),
        ("event_streams", "event-streams.json", event_streams),
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
        ("auditor_api", "auditor-api.json", auditor_api),
        ("worker_lifecycle", "worker-lifecycle.json", worker_lifecycle),
        ("event_streams", "event-streams.json", event_streams),
    )
    for kind, filename, factory in route_artifacts:
        root = tmp_path / kind
        root.mkdir()
        write_complete_evidence(root)
        payload = factory()
        payload["routes"].append(route("repair_debug_route"))
        payload["route_count"] = len(payload["routes"])
        payload["passed_route_count"] = len(payload["routes"])
        write_json(root / filename, payload)
        summary = root / "summary.json"

        assert run_gate(root, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert "routes must not include unknown values" in artifact["errors"]


def test_route_body_hash_is_required_for_route_artifacts(tmp_path: Path) -> None:
    route_artifacts = (
        ("auditor_api", "auditor-api.json", auditor_api),
        ("worker_lifecycle", "worker-lifecycle.json", worker_lifecycle),
        ("event_streams", "event-streams.json", event_streams),
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
        assert artifact["valid"] is False
        assert (
            "routes[0].body_blake3_hex must be a non-empty string"
            in artifact["errors"]
        )


def test_route_latency_is_required_for_route_artifacts(tmp_path: Path) -> None:
    route_artifacts = (
        ("auditor_api", "auditor-api.json", auditor_api),
        ("worker_lifecycle", "worker-lifecycle.json", worker_lifecycle),
        ("event_streams", "event-streams.json", event_streams),
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
        assert artifact["valid"] is False
        assert (
            "routes[0].latency_ms must be a non-negative integer"
            in artifact["errors"]
        )


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


def test_all_roster_bound_artifacts_reject_auditor_roster_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in ROSTER_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["roster_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} roster_digest_hex must reference a valid "
            "auditor_roster roster_digest_hex"
        ) in artifact["errors"]


def test_worker_statuses_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = worker_lifecycle()
    payload["statuses_observed"].append(payload["statuses_observed"][0])
    write_json(tmp_path / "worker-lifecycle.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["worker_lifecycle"]["artifacts"][0]
    assert "statuses_observed must not contain duplicate values" in artifact["errors"]


def test_worker_statuses_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = worker_lifecycle()
    payload["statuses_observed"].append("blocked")
    payload["status_count"] = len(payload["statuses_observed"])
    write_json(tmp_path / "worker-lifecycle.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["worker_lifecycle"]["artifacts"][0]
    assert "statuses_observed must not include unknown values" in artifact["errors"]


def test_worker_status_count_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = worker_lifecycle()
    del payload["status_count"]
    write_json(tmp_path / "worker-lifecycle.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["worker_lifecycle"]["artifacts"][0]
    assert "status_count must be a positive integer" in artifact["errors"]
    assert "status_count must be at least 4" in artifact["errors"]


def test_worker_status_count_must_match_inventory(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = worker_lifecycle()
    payload["status_count"] += 1
    write_json(tmp_path / "worker-lifecycle.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["worker_lifecycle"]["artifacts"][0]
    assert (
        "status_count must match unique statuses_observed count"
        in artifact["errors"]
    )


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


def test_all_failure_bound_artifacts_reject_failure_capture_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in FAILURE_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["evidence_bundle_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} evidence_bundle_digest_hex must reference a valid "
            "failure_capture evidence_bundle_digest_hex"
        ) in artifact["errors"]


def test_governance_handoff_targets_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_handoff()
    payload["handoff_targets"].append(payload["handoff_targets"][0])
    write_json(tmp_path / "governance-handoff.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["governance_handoff"]["artifacts"][0]
    assert "handoff_targets must not contain duplicate values" in artifact["errors"]


def test_governance_handoff_targets_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_handoff()
    payload["handoff_targets"].append("manual_review_board")
    payload["handoff_target_count"] = len(payload["handoff_targets"])
    write_json(tmp_path / "governance-handoff.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["governance_handoff"]["artifacts"][0]
    assert "handoff_targets must not include unknown values" in artifact["errors"]


def test_governance_handoff_target_count_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_handoff()
    del payload["handoff_target_count"]
    write_json(tmp_path / "governance-handoff.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["governance_handoff"]["artifacts"][0]
    assert "handoff_target_count must be a positive integer" in artifact["errors"]
    assert "handoff_target_count must be at least 5" in artifact["errors"]


def test_governance_handoff_target_count_must_match_inventory(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_handoff()
    payload["handoff_target_count"] += 1
    write_json(tmp_path / "governance-handoff.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["governance_handoff"]["artifacts"][0]
    assert (
        "handoff_target_count must match unique handoff_targets count"
        in artifact["errors"]
    )


def test_governance_approval_handoff_digest_must_match_handoff(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "governance-approval.json",
        governance_approval(handoff_digest=DIGEST_2),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert payload["valid_handoff_digests"] == [DIGEST]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "governance_approval handoff_digest_hex must reference a valid "
        "governance_handoff handoff_digest_hex"
    ]


def test_all_handoff_bound_artifacts_reject_governance_handoff_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in HANDOFF_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["handoff_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} handoff_digest_hex must reference a valid "
            "governance_handoff handoff_digest_hex"
        ) in artifact["errors"]


def test_governance_handoff_requires_policy_digest(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = governance_handoff()
    del payload["policy_digest_hex"]
    write_json(tmp_path / "governance-handoff.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["governance_handoff"]["artifacts"][0]
    assert "policy_digest_hex must be a non-empty string" in artifact["errors"]
    assert result["valid_policy_digests"] == []


def test_governance_approval_policy_digest_must_match_handoff(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = governance_approval()
    payload["policy_digest_hex"] = DIGEST_2
    write_json(tmp_path / "governance-approval.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    required = result["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert result["valid_policy_digests"] == [DIGEST]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "governance_approval policy_digest_hex must reference a valid "
        "governance_handoff policy_digest_hex"
    ]


def test_all_policy_bound_artifacts_reject_governance_policy_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in POLICY_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["policy_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} policy_digest_hex must reference a valid "
            "governance_handoff policy_digest_hex"
        ) in artifact["errors"]


def test_multiple_valid_roster_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = auditor_roster()
    payload["roster_digest_hex"] = DIGEST_2
    write_json(tmp_path / "auditor-roster-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_roster_digests"] == []
    assert (
        "valid_roster_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_failure_bundle_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = failure_capture()
    payload["evidence_bundle_digest_hex"] = DIGEST_2
    write_json(tmp_path / "failure-capture-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_failure_bundle_digests"] == []
    assert (
        "valid_failure_bundle_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_handoff_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "governance-handoff-alt.json",
        governance_handoff(handoff_digest=DIGEST_2),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_handoff_digests"] == []
    assert (
        "valid_handoff_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_policy_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_handoff()
    payload["policy_digest_hex"] = DIGEST_2
    write_json(tmp_path / "governance-handoff-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_policy_digests"] == []
    assert (
        "valid_policy_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_policy_bound_subset_requires_governance_handoff_anchor(tmp_path: Path) -> None:
    write_json(tmp_path / "governance-approval.json", governance_approval())
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-kind",
            "governance_approval",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["governance_approval"]["artifacts"][0]
    assert result["valid_policy_digests"] == []
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "governance_approval roster_digest_hex requires a valid auditor_roster "
        "roster_digest_hex",
        "governance_approval policy_digest_hex requires a valid governance_handoff "
        "policy_digest_hex",
        "governance_approval handoff_digest_hex requires a valid governance_handoff "
        "handoff_digest_hex",
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


def test_rollout_timing_evidence_must_be_integer_units(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    worker = worker_lifecycle()
    worker["repair_latency_seconds"] = 900.5
    worker["routes"][0]["latency_ms"] = 12.5
    write_json(tmp_path / "worker-lifecycle.json", worker)
    streams = event_streams()
    streams["event_lag_seconds"] = 30.5
    write_json(tmp_path / "event-streams.json", streams)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    worker_errors = payload["required"]["worker_lifecycle"]["artifacts"][0][
        "errors"
    ]
    stream_errors = payload["required"]["event_streams"]["artifacts"][0]["errors"]
    assert "repair_latency_seconds must be a non-negative integer" in worker_errors
    assert "routes[0].latency_ms must be a non-negative integer" in worker_errors
    assert "event_lag_seconds must be a non-negative integer" in stream_errors


def test_observability_critical_alert_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path) == 1


def test_observability_metrics_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = observability()
    payload["metrics"].append(payload["metrics"][0])
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "observability.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["observability"]["artifacts"][0]
    assert "metrics must not contain duplicate values" in artifact["errors"]
    assert "metric_count must match unique metrics count" in artifact["errors"]


def test_observability_metrics_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = observability()
    payload["metrics"].append("torii_sorafs_repair_debug_metric")
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "observability.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["observability"]["artifacts"][0]
    assert "metrics must not include unknown values" in artifact["errors"]


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
