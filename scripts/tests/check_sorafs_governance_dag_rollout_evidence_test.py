"""Tests for scripts/check_sorafs_governance_dag_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = (
    Path(__file__).resolve().parents[1] / "check_sorafs_governance_dag_rollout_evidence.py"
)
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_governance_dag_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


NOW_UNIX = 1_800_300_000
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
        "deployment_id": "governance-dag-staging-a",
        "environment": "staging",
    }


def ingest_service() -> dict:
    payload = base("sorafs.governance_dag.ingest_service_canary.v1")
    payload.update(
        {
            "daemonized": True,
            "payload_validation_enabled": True,
            "publisher_signature_verified": True,
            "dedupe_by_digest_enabled": True,
            "quarantine_invalid_blocks": True,
            "source_count": 8,
            "payload_kinds": [
                "deal-settlement",
                "repair-audit",
                "reconciliation",
                "reputation-snapshot",
                "moderation-ballot-event",
                "appeal-finance-report",
                "appeal-finance-settlement-receipt",
                "orderbook-settlement-receipt",
            ],
            "payload_bytes_included": False,
        }
    )
    return payload


def publisher_service(*, head_age: int = 300, block_count: int = 6) -> dict:
    payload = base("sorafs.governance_dag.publisher_service_canary.v1")
    payload.update(
        {
            "dag_builder_daemonized": True,
            "ipfs_cluster_pinning_enabled": True,
            "ipns_head_publication_enabled": True,
            "signed_head_verified": True,
            "parent_chain_verified": True,
            "car_segments_pinned": True,
            "public_head_cid_hex": DIGEST,
            "pin_lag_seconds": 120,
            "head_age_seconds": head_age,
            "block_count": block_count,
            "payload_kind_count": 8,
            "raw_head_included": False,
            "raw_car_included": False,
        }
    )
    return payload


def mirror_datastore(*, drift: bool = False) -> dict:
    payload = base("sorafs.governance_dag.mirror_datastore_canary.v1")
    payload.update(
        {
            "public_head_cid_hex": DIGEST,
            "rocksdb_ipld_enabled": True,
            "query_service_enabled": True,
            "mirror_index_verified": True,
            "head_lookup_verified": True,
            "block_lookup_verified": True,
            "node_lookup_verified": True,
            "digest_lookup_verified": True,
            "mirror_drift_detected": drift,
            "missing_block_count": 0,
            "raw_blocks_included": False,
        }
    )
    return payload


def operator_recovery() -> dict:
    payload = base("sorafs.governance_dag.operator_recovery_canary.v1")
    payload.update(
        {
            "public_head_cid_hex": DIGEST,
            "live_head_fetch_verified": True,
            "public_checkpoint_published": True,
            "checkpoint_recovery_verified": True,
            "public_recovery_cli_verified": True,
            "recovered_head_matches_public_head": True,
            "checkpoint_digest_hex": DIGEST,
            "raw_checkpoint_included": False,
        }
    )
    return payload


def dashboard_api(*, latency_ms: int = 200, passed: bool = True) -> dict:
    routes = [
        {
            "name": name,
            "passed": passed,
            "status_code": 200,
            "latency_ms": latency_ms,
            "publisher_identity_present": True,
            "verification_valid": True,
        }
        for name in (
            "dashboard",
            "head",
            "block_lookup",
            "node_lookup",
            "digest_lookup",
            "checkpoint",
        )
    ]
    payload = base("sorafs.governance_dag.dashboard_api_canary.v1")
    payload.update(
        {
            "public_head_cid_hex": DIGEST,
            "route_count": len(routes),
            "passed_route_count": len(routes) if passed else 0,
            "runtime_ipfs_backed": True,
            "response_bodies_included": False,
            "routes": routes,
        }
    )
    return payload


def observability(*, critical: bool = False) -> dict:
    payload = base("sorafs.governance_dag.observability_canary.v1")
    payload.update(
        {
            "public_head_cid_hex": DIGEST,
            "metrics_scrape_success": True,
            "dashboard_provisioned": True,
            "alert_rules_installed": True,
            "ipfs_ipns_metrics_present": True,
            "critical_alerts_firing": critical,
            "metrics": [
                "sorafs_governance_dag_publish_total",
                "sorafs_governance_dag_published_bytes_total",
                "sorafs_governance_dag_last_publish_timestamp_seconds",
                "sorafs_governance_dag_backlog",
                "sorafs_governance_dag_head_age_seconds",
                "sorafs_governance_dag_ipfs_pin_lag_seconds",
                "sorafs_governance_dag_ipns_update_total",
                "sorafs_governance_dag_mirror_drift",
            ],
            "response_bodies_included": False,
        }
    )
    return payload


def ipfs_ipns_e2e(*, block_count: int = 6) -> dict:
    payload = base("sorafs.governance_dag.ipfs_ipns_e2e_canary.v1")
    payload.update(
        {
            "public_head_cid_hex": DIGEST,
            "local_ipfs_backed_tests_passed": True,
            "public_head_resolved": True,
            "block_replay_verified": True,
            "duplicate_payload_rejected": True,
            "invalid_parent_quarantined": True,
            "pinning_outage_tested": True,
            "publisher_key_failure_tested": True,
            "block_count": block_count,
            "payload_kind_count": 8,
            "raw_blocks_included": False,
        }
    )
    return payload


def governance_approval() -> dict:
    payload = base("sorafs.governance_dag.governance_approval.v1")
    payload.update(
        {
            "public_head_cid_hex": DIGEST,
            "approved": True,
            "governance_vote_recorded": True,
            "iroha_config_bound": True,
            "publisher_keys_governed": True,
            "ipns_name_governed": True,
            "mirror_retention_policy_bound": True,
            "emergency_pause_tested": True,
            "config_source": "iroha_config",
            "policy_digest_hex": DIGEST,
        }
    )
    return payload


def write_complete_evidence(root: Path) -> None:
    write_json(root / "ingest-service.json", ingest_service())
    write_json(root / "publisher-service.json", publisher_service())
    write_json(root / "mirror-datastore.json", mirror_datastore())
    write_json(root / "operator-recovery.json", operator_recovery())
    write_json(root / "dashboard-api.json", dashboard_api())
    write_json(root / "observability.json", observability())
    write_json(root / "ipfs-ipns-e2e.json", ipfs_ipns_e2e())
    write_json(root / "governance-approval.json", governance_approval())


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.governance_dag.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["publisher_service"]["valid"] is True
    assert payload["valid_public_head_cids"] == [DIGEST]


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "governance-dag.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args}"]) == 0


def test_missing_publisher_service_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "publisher-service.json").unlink()

    assert run_gate(tmp_path) == 1


def test_dashboard_requires_public_head_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = dashboard_api()
    payload.pop("public_head_cid_hex")
    write_json(tmp_path / "dashboard-api.json", payload)

    assert run_gate(tmp_path) == 1


def test_ipfs_e2e_public_head_binding_must_match_publisher(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = ipfs_ipns_e2e()
    payload["public_head_cid_hex"] = DIGEST_2
    write_json(tmp_path / "ipfs-ipns-e2e.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["ipfs_ipns_e2e"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "ipfs_ipns_e2e public_head_cid_hex must match a valid "
        "publisher_service public_head_cid_hex"
    ]


def test_stale_publisher_head_does_not_anchor_bound_evidence(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = publisher_service()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "publisher-service.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["mirror_datastore"]
    artifact = required["artifacts"][0]
    assert payload["valid_public_head_cids"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "mirror_datastore public_head_cid_hex requires a valid "
        "publisher_service public_head_cid_hex"
    ]


def test_stale_ingest_service_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = ingest_service()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "ingest-service.json", payload)

    assert run_gate(tmp_path) == 1


def test_raw_payload_leakage_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publisher_service()
    payload["raw_block"] = {"cid": "leaked"}
    write_json(tmp_path / "publisher-service.json", payload)

    assert run_gate(tmp_path) == 1


def test_public_head_age_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "publisher-service.json", publisher_service(head_age=10_000))

    assert run_gate(tmp_path) == 1


def test_dashboard_route_failure_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "dashboard-api.json", dashboard_api(passed=False))

    assert run_gate(tmp_path) == 1


def test_dashboard_route_latency_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "dashboard-api.json", dashboard_api(latency_ms=10_000))

    assert run_gate(tmp_path) == 1


def test_mirror_drift_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "mirror-datastore.json", mirror_datastore(drift=True))

    assert run_gate(tmp_path) == 1


def test_ipfs_e2e_requires_minimum_blocks(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "ipfs-ipns-e2e.json", ipfs_ipns_e2e(block_count=2))

    assert run_gate(tmp_path) == 1


def test_observability_critical_alert_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path) == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(tmp_path / "unknown.json", {"schema": "sorafs.governance_dag.unknown.v1"})

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_unknown_directory_artifact_is_ignored_for_subset(tmp_path: Path) -> None:
    write_json(tmp_path / "ingest-service.json", ingest_service())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.governance_dag.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "ingest_service") == 0


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "ingest-service.json", ingest_service())
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path, "--require-kind", "ingest_service") == 1


def test_unknown_required_kind_returns_usage_error(tmp_path: Path) -> None:
    assert MODULE.main(["--evidence-dir", str(tmp_path), "--require-kind", "unknown"]) == 2
