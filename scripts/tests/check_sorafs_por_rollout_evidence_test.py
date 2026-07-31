"""Tests for scripts/check_sorafs_por_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_por_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location("check_sorafs_por_rollout_evidence", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

TEST_DIR = Path(__file__).resolve().parent
if str(TEST_DIR) not in sys.path:
    sys.path.insert(0, str(TEST_DIR))
from sorafs_rollout_runner_test_support import TopologyBoundChecker  # noqa: E402


NOW_UNIX = 1_800_500_000
GENERATED_AT = NOW_UNIX - 120
DIGEST = "cd" * 32
DIGEST_2 = "ef" * 32
HANDOFF_DIGEST = "ab" * 32
DEPLOYMENT_ID = "por-production-a"
ENVIRONMENT = "production"
CHECKER = TopologyBoundChecker(
    MODULE.main,
    deployment_id=DEPLOYMENT_ID,
    environment=ENVIRONMENT,
    name="por-checker",
)


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def base(schema: str) -> dict:
    return {
        "schema": schema,
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "deployment_context_reviewed": True,
    }


def randomness(*, provider_count: int = 3, challenge_count: int = 3) -> dict:
    payload = base("sorafs.por.randomness_canary.v1")
    providers = [{"name": f"provider-{index:02d}"} for index in range(provider_count)]
    challenges = [
        {"name": f"por-challenge-{index:02d}"}
        for index in range(challenge_count)
    ]
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
            "providers": providers,
            "challenge_count": challenge_count,
            "challenges": challenges,
            "seed_replay_digest_hex": DIGEST,
            "policy_digest_hex": DIGEST,
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
        "body_blake3_hex": DIGEST,
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


def reporting_archive(
    *,
    latency_ms: int = 300,
    archive_backend: str = "parquet",
    handoff_digest: str | None = HANDOFF_DIGEST,
) -> dict:
    routes = [route(name) for name in ("por_status", "por_export", "por_report")]
    payload = base("sorafs.por.reporting_archive_canary.v1")
    payload.update(
        {
            "route_count": len(routes),
            "passed_route_count": len(routes),
            "weekly_report_generated": True,
            "status_export_verified": True,
            "governance_archive_handoff_verified": True,
            "governance_archive_handoff_digest_hex": handoff_digest,
            "archive_retention_bound": True,
            "operator_archive_decision_recorded": True,
            "archive_backend": archive_backend,
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
            "metric_count": len(MODULE.REQUIRED_METRICS),
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


SEED_REPLAY_BOUND_FIXTURES = (
    ("scheduler_runtime", "scheduler-runtime.json", scheduler_runtime),
    ("validator_replay", "validator-replay.json", validator_replay),
    ("reporting_archive", "reporting-archive.json", reporting_archive),
    ("observability", "observability.json", observability),
    ("governance_approval", "governance-approval.json", governance_approval),
)

POLICY_BOUND_FIXTURES = (
    ("governance_approval", "governance-approval.json", governance_approval),
)


def run_gate(root: Path, *extra: str) -> int:
    return CHECKER(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.por.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["randomness"]["valid"] is True
    assert payload["valid_policy_digests"] == [DIGEST]
    assert payload["archive_backends"] == ["parquet"]
    assert payload["valid_governance_archive_handoff_digests"] == [HANDOFF_DIGEST]
    assert payload["metrics"] == sorted(MODULE.REQUIRED_METRICS)
    assert payload["metric_count_values"] == [len(MODULE.REQUIRED_METRICS)]
    reporting_artifact = payload["required"]["reporting_archive"]["artifacts"][0]
    assert reporting_artifact["fingerprint"]["archive_backend"] == "parquet"
    assert (
        reporting_artifact["fingerprint"]["governance_archive_handoff_digest_hex"]
        == HANDOFF_DIGEST
    )
    observability_artifact = payload["required"]["observability"]["artifacts"][0]
    assert observability_artifact["fingerprint"]["metric_count"] == len(
        MODULE.REQUIRED_METRICS
    )
    assert observability_artifact["fingerprint"]["metrics"] == list(
        MODULE.REQUIRED_METRICS
    )


def test_bound_fixture_tables_cover_checker_bound_kind_sets() -> None:
    assert (
        tuple(
            kind_name
            for kind_name, _file_name, _factory in SEED_REPLAY_BOUND_FIXTURES
        )
        == MODULE.SEED_REPLAY_BOUND_KINDS
    )
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in POLICY_BOUND_FIXTURES)
        == MODULE.POLICY_BOUND_KINDS
    )


def test_fixture_inventories_cover_checker_required_sets() -> None:
    assert tuple(route["name"] for route in scheduler_runtime()["routes"]) == (
        MODULE.REQUIRED_RUNTIME_ROUTES
    )
    assert tuple(route["name"] for route in reporting_archive()["routes"]) == (
        MODULE.REQUIRED_REPORTING_ROUTES
    )
    assert reporting_archive()["archive_backend"] in MODULE.ALLOWED_ARCHIVE_BACKENDS
    assert tuple(observability()["metrics"]) == MODULE.REQUIRED_METRICS


def test_summary_collects_reviewed_archive_backend_set(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "reporting-archive-sql.json",
        reporting_archive(archive_backend="sql", handoff_digest=HANDOFF_DIGEST),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["archive_backends"] == ["parquet", "sql"]
    assert payload["valid_governance_archive_handoff_digests"] == [HANDOFF_DIGEST]
    fingerprints = [
        artifact["fingerprint"]["archive_backend"]
        for artifact in payload["required"]["reporting_archive"]["artifacts"]
    ]
    assert sorted(fingerprints) == ["parquet", "sql"]
    handoff_fingerprints = [
        artifact["fingerprint"]["governance_archive_handoff_digest_hex"]
        for artifact in payload["required"]["reporting_archive"]["artifacts"]
    ]
    assert sorted(handoff_fingerprints) == [HANDOFF_DIGEST, HANDOFF_DIGEST]


def test_payload_safety_flags_are_required(tmp_path: Path) -> None:
    cases = (
        (
            "randomness.json",
            "randomness",
            randomness,
            ("raw_randomness_included", "raw_vrf_included"),
        ),
        (
            "scheduler-runtime.json",
            "scheduler_runtime",
            scheduler_runtime,
            ("response_bodies_included",),
        ),
        (
            "validator-replay.json",
            "validator_replay",
            validator_replay,
            ("raw_challenge_bytes_included", "raw_proof_bytes_included"),
        ),
        (
            "reporting-archive.json",
            "reporting_archive",
            reporting_archive,
            ("raw_report_included", "raw_export_included"),
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
    args = tmp_path / "por.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
        encoding="utf-8",
    )

    assert CHECKER([f"@{args}"]) == 0


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


def test_randomness_requires_policy_digest(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = randomness()
    del payload["policy_digest_hex"]
    write_json(tmp_path / "randomness.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["randomness"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "policy_digest_hex must be a non-empty string" in artifact["errors"]
    assert result["valid_policy_digests"] == []


def test_randomness_requires_minimum_provider_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "randomness.json", randomness(provider_count=2))

    assert run_gate(tmp_path) == 1


def test_randomness_provider_count_must_match_unique_providers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = randomness()
    payload["provider_count"] += 1
    write_json(tmp_path / "randomness.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["randomness"]["artifacts"][0]
    assert "provider_count must match unique providers count" in artifact["errors"]


def test_randomness_providers_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = randomness()
    payload["providers"].append(dict(payload["providers"][0]))
    payload["provider_count"] = len(payload["providers"])
    write_json(tmp_path / "randomness.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["randomness"]["artifacts"][0]
    assert "providers must not contain duplicate values" in artifact["errors"]
    assert "provider_count must match unique providers count" in artifact["errors"]


def test_randomness_provider_names_must_be_reviewed_labels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = randomness()
    payload["providers"][0] = {"name": "provider_00"}
    write_json(tmp_path / "randomness.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["randomness"]["artifacts"][0]
    assert (
        "providers[].name must match canonical lowercase `provider-*`"
        in artifact["errors"]
    )


def test_randomness_provider_names_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = randomness()
    payload["providers"][0] = {"name": "provider-placeholder"}
    write_json(tmp_path / "randomness.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["randomness"]["artifacts"][0]
    assert (
        "providers[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_randomness_requires_minimum_challenge_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "randomness.json", randomness(challenge_count=2))

    assert run_gate(tmp_path) == 1


def test_randomness_challenge_count_must_match_unique_challenges(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = randomness()
    payload["challenge_count"] += 1
    write_json(tmp_path / "randomness.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["randomness"]["artifacts"][0]
    assert "challenge_count must match unique challenges count" in artifact["errors"]


def test_randomness_challenges_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = randomness()
    payload["challenges"].append(dict(payload["challenges"][0]))
    payload["challenge_count"] = len(payload["challenges"])
    write_json(tmp_path / "randomness.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["randomness"]["artifacts"][0]
    assert "challenges must not contain duplicate values" in artifact["errors"]
    assert "challenge_count must match unique challenges count" in artifact["errors"]


def test_randomness_challenge_names_must_be_reviewed_labels(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = randomness()
    payload["challenges"][0] = {"name": "challenge-00"}
    write_json(tmp_path / "randomness.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["randomness"]["artifacts"][0]
    assert MODULE.CHALLENGE_LABEL_ERROR in artifact["errors"]


def test_randomness_challenge_names_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = randomness()
    payload["challenges"][0] = {"name": "por-challenge-placeholder"}
    write_json(tmp_path / "randomness.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["randomness"]["artifacts"][0]
    assert (
        "challenges[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_scheduler_lag_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "scheduler-runtime.json", scheduler_runtime(lag_seconds=10_000))

    assert run_gate(tmp_path) == 1


def test_scheduler_route_without_authz_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "scheduler-runtime.json", scheduler_runtime(authz=False))

    assert run_gate(tmp_path) == 1


def test_scheduler_runtime_route_count_must_match_unique_routes(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = scheduler_runtime()
    payload["route_count"] += 1
    payload["passed_route_count"] = payload["route_count"]
    write_json(tmp_path / "scheduler-runtime.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["scheduler_runtime"]["artifacts"][0]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_scheduler_runtime_routes_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = scheduler_runtime()
    payload["routes"].append(dict(payload["routes"][0]))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "scheduler-runtime.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["scheduler_runtime"]["artifacts"][0]
    assert "routes must not contain duplicate values" in artifact["errors"]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_scheduler_runtime_routes_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = scheduler_runtime()
    payload["routes"].append(route("por_scheduler_debug"))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "scheduler-runtime.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["scheduler_runtime"]["artifacts"][0]
    assert "routes must not include unknown values" in artifact["errors"]


def test_scheduler_runtime_rejects_retired_capacity_challenge_route(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = scheduler_runtime()
    payload["routes"].append(route("capacity_por_challenge"))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "scheduler-runtime.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["scheduler_runtime"]["artifacts"][0]
    assert "routes must not include unknown values" in artifact["errors"]


def test_scheduler_runtime_route_body_hash_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = scheduler_runtime()
    del payload["routes"][0]["body_blake3_hex"]
    write_json(tmp_path / "scheduler-runtime.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["scheduler_runtime"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "routes[0].body_blake3_hex must be a non-empty string"
        in artifact["errors"]
    )


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


def test_all_seed_replay_bound_artifacts_reject_randomness_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in SEED_REPLAY_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["seed_replay_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} seed_replay_digest_hex must reference a valid "
            "randomness seed_replay_digest_hex"
        ) in artifact["errors"]


def test_reporting_archive_route_count_must_match_unique_routes(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = reporting_archive()
    payload["route_count"] += 1
    payload["passed_route_count"] = payload["route_count"]
    write_json(tmp_path / "reporting-archive.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["reporting_archive"]["artifacts"][0]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_reporting_archive_routes_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = reporting_archive()
    payload["routes"].append(dict(payload["routes"][0]))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "reporting-archive.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["reporting_archive"]["artifacts"][0]
    assert "routes must not contain duplicate values" in artifact["errors"]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_reporting_archive_routes_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = reporting_archive()
    payload["routes"].append(route("por_archive_debug"))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "reporting-archive.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["reporting_archive"]["artifacts"][0]
    assert "routes must not include unknown values" in artifact["errors"]


@pytest.mark.parametrize(
    ("field", "value"),
    (
        ("manual_trigger_route_decided", True),
        ("manual_trigger_route_state", "retired"),
    ),
)
def test_reporting_archive_rejects_retired_manual_trigger_fields(
    tmp_path: Path,
    field: str,
    value: object,
) -> None:
    write_complete_evidence(tmp_path)
    payload = reporting_archive()
    payload[field] = value
    write_json(tmp_path / "reporting-archive.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["reporting_archive"]["artifacts"][0]
    assert "payload must not include unknown fields" in artifact["errors"]


def test_reporting_archive_route_body_hash_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = reporting_archive()
    del payload["routes"][0]["body_blake3_hex"]
    write_json(tmp_path / "reporting-archive.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["reporting_archive"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "routes[0].body_blake3_hex must be a non-empty string"
        in artifact["errors"]
    )


def test_governance_approval_policy_digest_must_match_randomness(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_approval()
    payload["policy_digest_hex"] = DIGEST_2
    write_json(tmp_path / "governance-approval.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    required = result["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert result["valid_policy_digests"] == [DIGEST]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "governance_approval policy_digest_hex must reference a valid "
        "randomness policy_digest_hex"
    ]


def test_all_policy_bound_artifacts_reject_randomness_policy_mismatch(
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
        assert result["valid_policy_digests"] == [DIGEST]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} policy_digest_hex must reference a valid "
            "randomness policy_digest_hex"
        ) in artifact["errors"]


def test_multiple_valid_seed_replay_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = randomness()
    payload["seed_replay_digest_hex"] = DIGEST_2
    write_json(tmp_path / "randomness-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_seed_replay_digests"] == []
    assert (
        "valid_seed_replay_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_policy_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = randomness()
    payload["policy_digest_hex"] = DIGEST_2
    write_json(tmp_path / "randomness-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_policy_digests"] == []
    assert (
        "valid_policy_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_governance_archive_handoff_anchors_fail_closed(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "reporting-archive-alt.json",
        reporting_archive(handoff_digest=DIGEST_2),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_governance_archive_handoff_digests"] == []
    assert (
        "valid_governance_archive_handoff_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_stale_randomness_does_not_anchor_policy_bound_evidence(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = randomness()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "randomness.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    required = result["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert result["valid_policy_digests"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert (
        "governance_approval policy_digest_hex requires a valid randomness "
        "policy_digest_hex"
    ) in artifact["errors"]


def test_reporting_archive_rejects_unreviewed_archive_backend(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "reporting-archive.json",
        reporting_archive(archive_backend="object-store"),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["reporting_archive"]["artifacts"][0]
    assert "archive_backend must be `sql` or `parquet`" in artifact["errors"]


def test_reporting_archive_unreviewed_archive_backend_stdout_does_not_echo_backend(
    tmp_path: Path,
    capsys,
) -> None:
    write_complete_evidence(tmp_path)
    invalid_backend = "object-store"
    write_json(
        tmp_path / "reporting-archive.json",
        reporting_archive(archive_backend=invalid_backend),
    )

    assert run_gate(tmp_path) == 1

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = json.dumps(payload, sort_keys=True)
    assert "archive_backend must be `sql` or `parquet`" in diagnostics
    assert invalid_backend not in diagnostics
    assert "archive_backend must be `sql` or `parquet`" in captured.err
    assert invalid_backend not in captured.err


def test_reporting_archive_requires_governance_handoff_digest(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    reporting = reporting_archive()
    del reporting["governance_archive_handoff_digest_hex"]
    write_json(tmp_path / "reporting-archive.json", reporting)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["reporting_archive"]["artifacts"][0]
    assert (
        "governance_archive_handoff_digest_hex must be a non-empty string"
        in artifact["errors"]
    )


def test_reporting_archive_rejects_malformed_governance_handoff_digest(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "reporting-archive.json",
        reporting_archive(handoff_digest="not-a-digest"),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["reporting_archive"]["artifacts"][0]
    assert (
        "governance_archive_handoff_digest_hex must be 64 lowercase hex characters"
        in artifact["errors"]
    )


def test_report_latency_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "reporting-archive.json", reporting_archive(latency_ms=10_000))

    assert run_gate(tmp_path) == 1


def test_rollout_timing_evidence_must_be_integer_units(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    scheduler = scheduler_runtime()
    scheduler["max_scheduler_lag_seconds"] = 12.5
    scheduler["routes"][0]["latency_ms"] = 12.5
    write_json(tmp_path / "scheduler-runtime.json", scheduler)
    reporting = reporting_archive()
    reporting["report_latency_ms"] = 12.5
    write_json(tmp_path / "reporting-archive.json", reporting)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    scheduler_errors = payload["required"]["scheduler_runtime"]["artifacts"][0][
        "errors"
    ]
    reporting_errors = payload["required"]["reporting_archive"]["artifacts"][0][
        "errors"
    ]
    assert (
        "max_scheduler_lag_seconds must be a non-negative integer"
        in scheduler_errors
    )
    assert "routes[0].latency_ms must be a non-negative integer" in scheduler_errors
    assert "report_latency_ms must be a non-negative integer" in reporting_errors


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
    payload["metrics"].append("sorafs_por_debug_metric")
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "observability.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["observability"]["artifacts"][0]
    assert "metrics must not include unknown values" in artifact["errors"]


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(tmp_path / "unknown.json", {"schema": "sorafs.por.unknown.v1"})

    assert CHECKER(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_unknown_directory_artifact_is_ignored_for_subset(tmp_path: Path) -> None:
    write_json(tmp_path / "randomness.json", randomness())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.por.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "randomness") == 0


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "randomness.json", randomness())
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path, "--require-kind", "randomness") == 1


def test_unknown_required_kind_returns_usage_error(tmp_path: Path) -> None:
    assert CHECKER(["--evidence-dir", str(tmp_path), "--require-kind", "unknown"]) == 2
