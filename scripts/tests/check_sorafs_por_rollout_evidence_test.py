"""Tests for scripts/check_sorafs_por_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_por_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location("check_sorafs_por_rollout_evidence", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


NOW_UNIX = 1_800_500_000
GENERATED_AT = NOW_UNIX - 120
DIGEST = "cd" * 32
DIGEST_2 = "ef" * 32


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def base(schema: str) -> dict:
    return {
        "schema": schema,
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "deployment_id": "por-staging-a",
        "environment": "staging",
    }


def randomness(*, provider_count: int = 3, challenge_count: int = 3) -> dict:
    payload = base("sorafs.por.randomness_canary.v1")
    payload.update(
        {
            "drand_round_verified": True,
            "drand_signature_verified": True,
            "drand_round_fresh": True,
            "vrf_proofs_verified": True,
            "provider_manifest_binding_verified": True,
            "deterministic_seed_replay_verified": True,
            "forced_challenge_policy_verified": True,
            "provider_count": provider_count,
            "challenge_count": challenge_count,
            "seed_replay_digest_hex": DIGEST,
            "raw_randomness_included": False,
            "raw_vrf_included": False,
        }
    )
    return payload


def route(name: str, *, latency_ms: int = 200, authz: bool = True) -> dict:
    return {
        "name": name,
        "passed": True,
        "status_code": 200,
        "latency_ms": latency_ms,
        "authz_enforced": authz,
        "norito_verified": True,
    }


def scheduler_runtime(*, lag_seconds: int = 60, authz: bool = True) -> dict:
    routes = [
        route(name, authz=authz)
        for name in (
            "por_status",
            "por_export",
            "por_report",
            "por_ingestion",
            "capacity_por_challenge",
            "capacity_por_proof",
            "capacity_por_verdict",
        )
    ]
    payload = base("sorafs.por.scheduler_runtime_canary.v1")
    payload.update(
        {
            "route_count": len(routes),
            "passed_route_count": len(routes),
            "scheduler_runtime_enabled": True,
            "norito_snapshot_persisted": True,
            "governance_dag_challenge_published": True,
            "repair_handoff_verified": True,
            "ingestion_backlog_bounded": True,
            "duplicate_samples_within_budget": True,
            "seed_replay_digest_hex": DIGEST,
            "max_scheduler_lag_seconds": lag_seconds,
            "response_bodies_included": False,
            "routes": routes,
        }
    )
    return payload


def validator_replay(*, merkle_replay: bool = True) -> dict:
    payload = base("sorafs.por.validator_replay_canary.v1")
    payload.update(
        {
            "sorafs_validate_por_passed": True,
            "challenge_proof_binding_verified": True,
            "sample_coverage_verified": True,
            "deadline_policy_verified": True,
            "merkle_replay_verified": merkle_replay,
            "validation_outcome_schema_verified": True,
            "pairs_replayed": 3,
            "seed_replay_digest_hex": DIGEST,
            "validation_bundle_digest_hex": DIGEST,
            "raw_challenge_bytes_included": False,
            "raw_proof_bytes_included": False,
        }
    )
    return payload


def reporting_archive(*, route_state: str = "wired", latency_ms: int = 300) -> dict:
    routes = [route(name) for name in ("por_status", "por_export", "por_report")]
    payload = base("sorafs.por.reporting_archive_canary.v1")
    payload.update(
        {
            "route_count": len(routes),
            "passed_route_count": len(routes),
            "weekly_report_generated": True,
            "status_export_verified": True,
            "governance_archive_handoff_verified": True,
            "archive_retention_bound": True,
            "operator_archive_decision_recorded": True,
            "manual_trigger_route_decided": True,
            "manual_trigger_route_state": route_state,
            "report_latency_ms": latency_ms,
            "seed_replay_digest_hex": DIGEST,
            "report_digest_hex": DIGEST,
            "raw_report_included": False,
            "raw_export_included": False,
            "routes": routes,
        }
    )
    return payload


def observability(*, critical: bool = False) -> dict:
    payload = base("sorafs.por.observability_canary.v1")
    payload.update(
        {
            "metrics_scrape_success": True,
            "dashboard_provisioned": True,
            "alert_rules_installed": True,
            "forced_challenge_alert_tested": True,
            "ingest_backlog_alert_tested": True,
            "critical_alerts_firing": critical,
            "metrics": [
                "torii_sorafs_por_challenges_total",
                "torii_sorafs_por_forced_challenges_total",
                "torii_sorafs_por_sampling_duplicates_total",
                "torii_sorafs_por_ingest_backlog",
                "torii_sorafs_por_ingest_failures_total",
                "sorafs_por_response_latency_seconds_bucket",
                "sorafs_vrf_missing_total",
                "sorafs_por_seed_verification_failures_total",
            ],
            "seed_replay_digest_hex": DIGEST,
            "response_bodies_included": False,
        }
    )
    return payload


def governance_approval() -> dict:
    payload = base("sorafs.por.governance_approval.v1")
    payload.update(
        {
            "approved": True,
            "governance_vote_recorded": True,
            "iroha_config_bound": True,
            "por_policy_bound": True,
            "auditor_roster_bound": True,
            "archive_policy_bound": True,
            "governance_dag_bound": True,
            "config_source": "iroha_config",
            "seed_replay_digest_hex": DIGEST,
            "policy_digest_hex": DIGEST,
        }
    )
    return payload


def write_complete_evidence(root: Path) -> None:
    write_json(root / "randomness.json", randomness())
    write_json(root / "scheduler-runtime.json", scheduler_runtime())
    write_json(root / "validator-replay.json", validator_replay())
    write_json(root / "reporting-archive.json", reporting_archive())
    write_json(root / "observability.json", observability())
    write_json(root / "governance-approval.json", governance_approval())


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.por.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["randomness"]["valid"] is True


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "por.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args}"]) == 0


def test_missing_randomness_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "randomness.json").unlink()

    assert run_gate(tmp_path) == 1


def test_stale_randomness_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = randomness()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "randomness.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["scheduler_runtime"]
    artifact = required["artifacts"][0]
    assert payload["valid_seed_replay_digests"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "scheduler_runtime seed_replay_digest_hex requires a valid randomness "
        "seed_replay_digest_hex"
    ]


def test_raw_randomness_leakage_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = randomness()
    payload["raw_randomness"] = "leaked"
    write_json(tmp_path / "randomness.json", payload)

    assert run_gate(tmp_path) == 1


def test_randomness_requires_minimum_provider_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "randomness.json", randomness(provider_count=2))

    assert run_gate(tmp_path) == 1


def test_randomness_requires_minimum_challenge_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "randomness.json", randomness(challenge_count=2))

    assert run_gate(tmp_path) == 1


def test_scheduler_lag_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "scheduler-runtime.json", scheduler_runtime(lag_seconds=10_000))

    assert run_gate(tmp_path) == 1


def test_scheduler_route_without_authz_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "scheduler-runtime.json", scheduler_runtime(authz=False))

    assert run_gate(tmp_path) == 1


def test_scheduler_runtime_requires_seed_replay_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = scheduler_runtime()
    del payload["seed_replay_digest_hex"]
    write_json(tmp_path / "scheduler-runtime.json", payload)

    assert run_gate(tmp_path) == 1


def test_validator_replay_requires_merkle_replay(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "validator-replay.json", validator_replay(merkle_replay=False))

    assert run_gate(tmp_path) == 1


def test_reporting_archive_seed_replay_digest_must_match(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = reporting_archive()
    payload["seed_replay_digest_hex"] = DIGEST_2
    write_json(tmp_path / "reporting-archive.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["reporting_archive"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "reporting_archive seed_replay_digest_hex must reference a valid randomness "
        "seed_replay_digest_hex"
    ]


def test_reporting_archive_rejects_missing_manual_trigger_decision(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "reporting-archive.json", reporting_archive(route_state="missing"))
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["reporting_archive"]["artifacts"][0]
    assert (
        "manual_trigger_route_state must be `wired` or `retired`"
        in artifact["errors"]
    )


def test_report_latency_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "reporting-archive.json", reporting_archive(latency_ms=10_000))

    assert run_gate(tmp_path) == 1


def test_observability_critical_alert_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path) == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(tmp_path / "unknown.json", {"schema": "sorafs.por.unknown.v1"})

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_unknown_directory_artifact_is_ignored_for_subset(tmp_path: Path) -> None:
    write_json(tmp_path / "randomness.json", randomness())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.por.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "randomness") == 0


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "randomness.json", randomness())
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path, "--require-kind", "randomness") == 1


def test_unknown_required_kind_returns_usage_error(tmp_path: Path) -> None:
    assert MODULE.main(["--evidence-dir", str(tmp_path), "--require-kind", "unknown"]) == 2
