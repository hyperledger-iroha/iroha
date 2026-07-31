"""Tests for scripts/check_sorafs_pdp_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_pdp_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location("check_sorafs_pdp_rollout_evidence", MODULE_PATH)
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
DIGEST = "ef" * 32
DIGEST_2 = "12" * 32
ROSTER_DIGEST = "34" * 32
HANDOFF_DIGEST = "56" * 32
ALT_DIGEST = "78" * 32
DEPLOYMENT_ID = "pdp-production-a"
ENVIRONMENT = "production"
CHECKER = TopologyBoundChecker(
    MODULE.main,
    deployment_id=DEPLOYMENT_ID,
    environment=ENVIRONMENT,
    name="pdp-checker",
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


def provider_transport(*, authz: bool = True, guard_removed: bool = True) -> dict:
    routes = [
        route(name, authz=authz)
        for name in (
            "pdp_challenge_fetch",
            "pdp_proof_submit",
            "pdp_status",
            "proof_stream_pdp",
            "pdp_export",
        )
    ]
    payload = base("sorafs.pdp.provider_transport_canary.v1")
    payload.update(
        {
            "route_count": len(routes),
            "passed_route_count": len(routes),
            "provider_protocol_enabled": True,
            "torii_pdp_fail_closed_guard_removed": guard_removed,
            "challenge_fetch_verified": True,
            "proof_submit_verified": True,
            "deadline_headers_verified": True,
            "provider_authz_enforced": True,
            "proof_stream_pdp_enabled": True,
            "response_bodies_included": False,
            "routes": routes,
        }
    )
    return payload


def proof_generation(
    *,
    provider_count: int = 3,
    challenge_count: int = 3,
    proof_count: int = 3,
    proof_latency_ms: int = 1_000,
) -> dict:
    payload = base("sorafs.pdp.proof_generation_canary.v1")
    providers = [{"name": f"provider-{index:02d}"} for index in range(provider_count)]
    challenges = [
        {"name": f"pdp-challenge-{index:02d}"} for index in range(challenge_count)
    ]
    proofs = [{"name": f"pdp-proof-{index:02d}"} for index in range(proof_count)]
    payload.update(
        {
            "provider_count": provider_count,
            "providers": providers,
            "challenge_count": challenge_count,
            "challenges": challenges,
            "proof_count": proof_count,
            "proofs": proofs,
            "provider_signatures_verified": True,
            "manifest_binding_verified": True,
            "commitment_binding_verified": True,
            "segment_merkle_paths_verified": True,
            "hot_leaf_merkle_paths_verified": True,
            "deadline_policy_verified": True,
            "hardware_determinism_reviewed": True,
            "max_proof_latency_ms": proof_latency_ms,
            "proof_summary_digest_hex": DIGEST,
            "policy_digest_hex": DIGEST,
            "provider_roster_digest_hex": ROSTER_DIGEST,
            "raw_challenge_bytes_included": False,
            "raw_proof_bytes_included": False,
        }
    )
    return payload


def validator_replay(*, expanded_fixtures: bool = True) -> dict:
    payload = base("sorafs.pdp.validator_replay_canary.v1")
    payload.update(
        {
            "sorafs_validate_pdp_passed": True,
            "commitment_challenge_binding_verified": True,
            "challenge_proof_binding_verified": True,
            "segment_coverage_verified": True,
            "hot_leaf_coverage_verified": True,
            "deadline_policy_verified": True,
            "missing_merkle_path_negative_verified": True,
            "expanded_negative_fixtures_committed": expanded_fixtures,
            "validation_outcome_schema_verified": True,
            "pairs_replayed": 3,
            "proof_summary_digest_hex": DIGEST,
            "validation_bundle_digest_hex": DIGEST,
            "raw_challenge_bytes_included": False,
            "raw_proof_bytes_included": False,
        }
    )
    return payload


def governance_repair(
    *,
    handoff: bool = True,
    handoff_digest: str | None = HANDOFF_DIGEST,
) -> dict:
    payload = base("sorafs.pdp.governance_repair_canary.v1")
    payload.update(
        {
            "governance_dag_challenge_published": True,
            "governance_dag_verdict_published": True,
            "repair_handoff_verified": handoff,
            "repair_handoff_digest_hex": handoff_digest,
            "archive_retention_bound": True,
            "slash_policy_bound": True,
            "operator_export_verified": True,
            "proof_summary_digest_hex": DIGEST,
            "archive_summary_digest_hex": DIGEST,
            "raw_export_included": False,
            "raw_report_included": False,
        }
    )
    return payload


def observability(*, critical: bool = False) -> dict:
    payload = base("sorafs.pdp.observability_canary.v1")
    payload.update(
        {
            "metrics_scrape_success": True,
            "dashboard_provisioned": True,
            "alert_rules_installed": True,
            "deadline_breach_alert_tested": True,
            "proof_failure_alert_tested": True,
            "repair_handoff_alert_tested": True,
            "critical_alerts_firing": critical,
            "metrics": [
                "torii_sorafs_pdp_challenges_total",
                "torii_sorafs_pdp_proofs_total",
                "torii_sorafs_pdp_failures_total",
                "torii_sorafs_proof_stream_events_total",
                "sorafs_pdp_response_latency_seconds_bucket",
                "sorafs_pdp_repair_handoffs_total",
            ],
            "metric_count": len(MODULE.REQUIRED_METRICS),
            "proof_summary_digest_hex": DIGEST,
            "response_bodies_included": False,
        }
    )
    return payload


def governance_approval() -> dict:
    payload = base("sorafs.pdp.governance_approval.v1")
    payload.update(
        {
            "approved": True,
            "governance_vote_recorded": True,
            "iroha_config_bound": True,
            "pdp_policy_bound": True,
            "provider_roster_bound": True,
            "repair_policy_bound": True,
            "governance_dag_bound": True,
            "config_source": "iroha_config",
            "proof_summary_digest_hex": DIGEST,
            "policy_digest_hex": DIGEST,
            "provider_roster_digest_hex": ROSTER_DIGEST,
        }
    )
    return payload


def write_complete_evidence(root: Path) -> None:
    write_json(root / "provider-transport.json", provider_transport())
    write_json(root / "proof-generation.json", proof_generation())
    write_json(root / "validator-replay.json", validator_replay())
    write_json(root / "governance-repair.json", governance_repair())
    write_json(root / "observability.json", observability())
    write_json(root / "governance-approval.json", governance_approval())


PROOF_SUMMARY_BOUND_FIXTURES = (
    ("validator_replay", "validator-replay.json", validator_replay),
    ("governance_repair", "governance-repair.json", governance_repair),
    ("observability", "observability.json", observability),
    ("governance_approval", "governance-approval.json", governance_approval),
)

POLICY_BOUND_FIXTURES = (
    ("governance_approval", "governance-approval.json", governance_approval),
)

PROVIDER_ROSTER_BOUND_FIXTURES = (
    ("governance_approval", "governance-approval.json", governance_approval),
)


def run_gate(root: Path, *extra: str) -> int:
    return CHECKER(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.pdp.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["provider_transport"]["valid"] is True
    assert payload["valid_policy_digests"] == [DIGEST]
    assert payload["valid_provider_roster_digests"] == [ROSTER_DIGEST]
    assert payload["valid_repair_handoff_digests"] == [HANDOFF_DIGEST]
    assert payload["metrics"] == sorted(MODULE.REQUIRED_METRICS)
    assert payload["metric_count_values"] == [len(MODULE.REQUIRED_METRICS)]
    observability_artifact = payload["required"]["observability"]["artifacts"][0]
    assert observability_artifact["fingerprint"]["metric_count"] == len(
        MODULE.REQUIRED_METRICS
    )
    assert observability_artifact["fingerprint"]["metrics"] == list(
        MODULE.REQUIRED_METRICS
    )
    governance_repair_artifact = payload["required"]["governance_repair"][
        "artifacts"
    ][0]
    assert (
        governance_repair_artifact["fingerprint"]["repair_handoff_digest_hex"]
        == HANDOFF_DIGEST
    )


def test_bound_fixture_tables_cover_checker_bound_kind_sets() -> None:
    assert (
        tuple(
            kind_name
            for kind_name, _file_name, _factory in PROOF_SUMMARY_BOUND_FIXTURES
        )
        == MODULE.PROOF_SUMMARY_BOUND_KINDS
    )
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in POLICY_BOUND_FIXTURES)
        == MODULE.POLICY_BOUND_KINDS
    )
    assert (
        tuple(
            kind_name
            for kind_name, _file_name, _factory in PROVIDER_ROSTER_BOUND_FIXTURES
        )
        == MODULE.PROVIDER_ROSTER_BOUND_KINDS
    )


def test_fixture_inventories_cover_checker_required_sets() -> None:
    assert tuple(route["name"] for route in provider_transport()["routes"]) == (
        MODULE.REQUIRED_ROUTES
    )


def test_payload_safety_flags_are_required(tmp_path: Path) -> None:
    cases = (
        (
            "provider-transport.json",
            "provider_transport",
            provider_transport,
            ("response_bodies_included",),
        ),
        (
            "proof-generation.json",
            "proof_generation",
            proof_generation,
            ("raw_challenge_bytes_included", "raw_proof_bytes_included"),
        ),
        (
            "validator-replay.json",
            "validator_replay",
            validator_replay,
            ("raw_challenge_bytes_included", "raw_proof_bytes_included"),
        ),
        (
            "governance-repair.json",
            "governance_repair",
            governance_repair,
            ("raw_export_included", "raw_report_included"),
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
    args = tmp_path / "pdp.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
        encoding="utf-8",
    )

    assert CHECKER([f"@{args}"]) == 0


def test_missing_provider_transport_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "provider-transport.json").unlink()

    assert run_gate(tmp_path) == 1


def test_stale_provider_transport_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = provider_transport()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "provider-transport.json", payload)

    assert run_gate(tmp_path) == 1


def test_raw_proof_leakage_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["raw_proof"] = "leaked"
    write_json(tmp_path / "proof-generation.json", payload)

    assert run_gate(tmp_path) == 1


def test_provider_transport_requires_guard_removed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "provider-transport.json",
        provider_transport(guard_removed=False),
    )

    assert run_gate(tmp_path) == 1


def test_provider_transport_route_without_authz_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "provider-transport.json", provider_transport(authz=False))

    assert run_gate(tmp_path) == 1


def test_provider_transport_route_count_must_match_unique_routes(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = provider_transport()
    payload["route_count"] += 1
    payload["passed_route_count"] = payload["route_count"]
    write_json(tmp_path / "provider-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["provider_transport"]["artifacts"][0]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_provider_transport_routes_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = provider_transport()
    payload["routes"].append(dict(payload["routes"][0]))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "provider-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["provider_transport"]["artifacts"][0]
    assert "routes must not contain duplicate values" in artifact["errors"]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_provider_transport_routes_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = provider_transport()
    payload["routes"].append(route("pdp_debug_route"))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "provider-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["provider_transport"]["artifacts"][0]
    assert "routes must not include unknown values" in artifact["errors"]


def test_provider_transport_route_body_hash_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = provider_transport()
    del payload["routes"][0]["body_blake3_hex"]
    write_json(tmp_path / "provider-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["provider_transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "routes[0].body_blake3_hex must be a non-empty string"
        in artifact["errors"]
    )


def test_provider_transport_route_latency_must_be_non_negative(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = provider_transport()
    payload["routes"][0]["latency_ms"] = -1
    write_json(tmp_path / "provider-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["provider_transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "routes[0].latency_ms must be a non-negative integer"
        in artifact["errors"]
    )


def test_provider_transport_route_latency_must_be_integer(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = provider_transport()
    payload["routes"][0]["latency_ms"] = 12.5
    write_json(tmp_path / "provider-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["provider_transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "routes[0].latency_ms must be a non-negative integer"
        in artifact["errors"]
    )


def test_proof_generation_requires_minimum_provider_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "proof-generation.json", proof_generation(provider_count=2))

    assert run_gate(tmp_path) == 1


def test_proof_generation_provider_count_must_match_unique_providers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["provider_count"] += 1
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert "provider_count must match unique providers count" in artifact["errors"]


def test_proof_generation_providers_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["providers"].append(dict(payload["providers"][0]))
    payload["provider_count"] = len(payload["providers"])
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert "providers must not contain duplicate values" in artifact["errors"]
    assert "provider_count must match unique providers count" in artifact["errors"]


def test_proof_generation_provider_names_must_be_reviewed_labels(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["providers"][0] = {"name": "provider_00"}
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert (
        "providers[].name must match canonical lowercase `provider-*`"
        in artifact["errors"]
    )


def test_proof_generation_provider_names_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["providers"][0] = {"name": "provider-placeholder"}
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert (
        "providers[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_proof_generation_challenge_count_must_match_unique_challenges(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["challenge_count"] += 1
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert "challenge_count must match unique challenges count" in artifact["errors"]


def test_proof_generation_challenges_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["challenges"].append(dict(payload["challenges"][0]))
    payload["challenge_count"] = len(payload["challenges"])
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert "challenges must not contain duplicate values" in artifact["errors"]
    assert "challenge_count must match unique challenges count" in artifact["errors"]


def test_proof_generation_challenge_names_must_be_reviewed_labels(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["challenges"][0]["name"] = "challenge-00"
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert MODULE.CHALLENGE_LABEL_ERROR in artifact["errors"]


def test_proof_generation_challenge_names_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["challenges"][0]["name"] = "pdp-challenge-placeholder"
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert (
        "challenges[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_proof_generation_requires_minimum_proof_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "proof-generation.json", proof_generation(proof_count=2))

    assert run_gate(tmp_path) == 1


def test_proof_generation_proof_count_must_match_unique_proofs(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["proof_count"] += 1
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert "proof_count must match unique proofs count" in artifact["errors"]


def test_proof_generation_proofs_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["proofs"].append(dict(payload["proofs"][0]))
    payload["proof_count"] = len(payload["proofs"])
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert "proofs must not contain duplicate values" in artifact["errors"]
    assert "proof_count must match unique proofs count" in artifact["errors"]


def test_proof_generation_proof_names_must_be_reviewed_labels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["proofs"][0]["name"] = "proof-00"
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert MODULE.PROOF_LABEL_ERROR in artifact["errors"]


def test_proof_generation_proof_names_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["proofs"][0]["name"] = "pdp-proof-placeholder"
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert (
        "proofs[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_proof_latency_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "proof-generation.json",
        proof_generation(proof_latency_ms=MODULE.DEFAULT_MAX_PROOF_LATENCY_MS + 1),
    )

    assert run_gate(tmp_path) == 1


def test_proof_latency_must_be_positive(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "proof-generation.json",
        proof_generation(proof_latency_ms=-1),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "max_proof_latency_ms must be a positive integer" in artifact["errors"]


def test_proof_latency_must_be_integer(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "proof-generation.json",
        proof_generation(proof_latency_ms=12.5),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "max_proof_latency_ms must be a positive integer" in artifact["errors"]


def test_validator_replay_requires_expanded_fixtures(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "validator-replay.json", validator_replay(expanded_fixtures=False))

    assert run_gate(tmp_path) == 1


def test_validator_replay_requires_proof_summary_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = validator_replay()
    del payload["proof_summary_digest_hex"]
    write_json(tmp_path / "validator-replay.json", payload)

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
    payload["metrics"].append("sorafs_pdp_debug_metric")
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "observability.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["observability"]["artifacts"][0]
    assert "metrics must not include unknown values" in artifact["errors"]


def test_proof_generation_requires_policy_digest(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    del payload["policy_digest_hex"]
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "policy_digest_hex must be a non-empty string" in artifact["errors"]
    assert result["valid_policy_digests"] == []


def test_proof_generation_requires_provider_roster_digest(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    del payload["provider_roster_digest_hex"]
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_generation"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "provider_roster_digest_hex must be a non-empty string" in artifact["errors"]
    assert result["valid_provider_roster_digests"] == []


def test_governance_repair_proof_summary_digest_must_match(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_repair()
    payload["proof_summary_digest_hex"] = DIGEST_2
    write_json(tmp_path / "governance-repair.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["governance_repair"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "governance_repair proof_summary_digest_hex must reference a valid "
        "proof_generation proof_summary_digest_hex"
    ]


def test_all_proof_summary_bound_artifacts_reject_generation_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in PROOF_SUMMARY_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["proof_summary_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} proof_summary_digest_hex must reference a valid "
            "proof_generation proof_summary_digest_hex"
        ) in artifact["errors"]


def test_governance_approval_policy_digest_must_match_proof_generation(
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
        "proof_generation policy_digest_hex"
    ]


def test_all_policy_bound_artifacts_reject_generation_policy_mismatch(
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
            "proof_generation policy_digest_hex"
        ) in artifact["errors"]


def test_governance_approval_provider_roster_digest_must_match_proof_generation(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_approval()
    payload["provider_roster_digest_hex"] = DIGEST_2
    write_json(tmp_path / "governance-approval.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    required = result["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert result["valid_provider_roster_digests"] == [ROSTER_DIGEST]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "governance_approval provider_roster_digest_hex must reference a valid "
        "proof_generation provider_roster_digest_hex"
    ]


def test_all_provider_roster_bound_artifacts_reject_generation_roster_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in PROVIDER_ROSTER_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["provider_roster_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert result["valid_provider_roster_digests"] == [ROSTER_DIGEST]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} provider_roster_digest_hex must reference a valid "
            "proof_generation provider_roster_digest_hex"
        ) in artifact["errors"]


def test_multiple_valid_proof_summary_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["proof_summary_digest_hex"] = ALT_DIGEST
    write_json(tmp_path / "proof-generation-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_proof_summary_digests"] == []
    assert (
        "valid_proof_summary_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_policy_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["policy_digest_hex"] = ALT_DIGEST
    write_json(tmp_path / "proof-generation-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_policy_digests"] == []
    assert (
        "valid_policy_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_provider_roster_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["provider_roster_digest_hex"] = ALT_DIGEST
    write_json(tmp_path / "proof-generation-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_provider_roster_digests"] == []
    assert (
        "valid_provider_roster_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_repair_handoff_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_repair(handoff_digest=ALT_DIGEST)
    write_json(tmp_path / "governance-repair-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_repair_handoff_digests"] == []
    assert (
        "valid_repair_handoff_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_stale_proof_generation_does_not_anchor_bound_evidence(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["validator_replay"]
    artifact = required["artifacts"][0]
    assert payload["valid_proof_summary_digests"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "validator_replay proof_summary_digest_hex requires a valid "
        "proof_generation proof_summary_digest_hex"
    ]


def test_stale_proof_generation_does_not_anchor_policy_bound_evidence(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    required = result["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert result["valid_policy_digests"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert (
        "governance_approval policy_digest_hex requires a valid "
        "proof_generation policy_digest_hex"
    ) in artifact["errors"]


def test_stale_proof_generation_does_not_anchor_provider_roster_bound_evidence(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_generation()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "proof-generation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    required = result["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert result["valid_provider_roster_digests"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert (
        "governance_approval provider_roster_digest_hex requires a valid "
        "proof_generation provider_roster_digest_hex"
    ) in artifact["errors"]


def test_governance_repair_requires_handoff(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "governance-repair.json", governance_repair(handoff=False))

    assert run_gate(tmp_path) == 1


def test_governance_repair_requires_handoff_digest(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_repair()
    del payload["repair_handoff_digest_hex"]
    write_json(tmp_path / "governance-repair.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["governance_repair"]["artifacts"][0]
    assert "repair_handoff_digest_hex must be a non-empty string" in artifact["errors"]
    assert result["valid_repair_handoff_digests"] == []


def test_governance_repair_rejects_malformed_handoff_digest(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "governance-repair.json",
        governance_repair(handoff_digest="not-a-digest"),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["governance_repair"]["artifacts"][0]
    assert "repair_handoff_digest_hex must be 64 lowercase hex characters" in artifact["errors"]
    assert result["valid_repair_handoff_digests"] == []


def test_observability_critical_alert_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path) == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(tmp_path / "unknown.json", {"schema": "sorafs.pdp.unknown.v1"})

    assert CHECKER(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_unknown_directory_artifact_is_ignored_for_subset(tmp_path: Path) -> None:
    write_json(tmp_path / "provider-transport.json", provider_transport())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.pdp.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "provider_transport") == 0


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "provider-transport.json", provider_transport())
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path, "--require-kind", "provider_transport") == 1


def test_unknown_required_kind_returns_usage_error(tmp_path: Path) -> None:
    assert CHECKER(["--evidence-dir", str(tmp_path), "--require-kind", "unknown"]) == 2
