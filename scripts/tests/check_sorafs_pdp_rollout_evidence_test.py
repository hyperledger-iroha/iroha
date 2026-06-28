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


NOW_UNIX = 1_800_500_000
GENERATED_AT = NOW_UNIX - 120
DIGEST = "ef" * 32
DIGEST_2 = "12" * 32


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def base(schema: str) -> dict:
    return {
        "schema": schema,
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "deployment_id": "pdp-staging-a",
        "environment": "staging",
        "deployment_context_reviewed": True,
    }


def route(name: str, *, latency_ms: int = 200, authz: bool = True) -> dict:
    return {
        "name": name,
        "passed": True,
        "status_code": 200,
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
    payload.update(
        {
            "provider_count": provider_count,
            "challenge_count": challenge_count,
            "proof_count": proof_count,
            "provider_signatures_verified": True,
            "manifest_binding_verified": True,
            "commitment_binding_verified": True,
            "segment_merkle_paths_verified": True,
            "hot_leaf_merkle_paths_verified": True,
            "deadline_policy_verified": True,
            "hardware_determinism_reviewed": True,
            "max_proof_latency_ms": proof_latency_ms,
            "proof_summary_digest_hex": DIGEST,
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


def governance_repair(*, handoff: bool = True) -> dict:
    payload = base("sorafs.pdp.governance_repair_canary.v1")
    payload.update(
        {
            "governance_dag_challenge_published": True,
            "governance_dag_verdict_published": True,
            "repair_handoff_verified": handoff,
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


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.pdp.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["provider_transport"]["valid"] is True


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "pdp.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args}"]) == 0


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


def test_proof_generation_requires_minimum_provider_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "proof-generation.json", proof_generation(provider_count=2))

    assert run_gate(tmp_path) == 1


def test_proof_generation_requires_minimum_proof_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "proof-generation.json", proof_generation(proof_count=2))

    assert run_gate(tmp_path) == 1


def test_proof_latency_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "proof-generation.json",
        proof_generation(proof_latency_ms=MODULE.DEFAULT_MAX_PROOF_LATENCY_MS + 1),
    )

    assert run_gate(tmp_path) == 1


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


def test_governance_repair_requires_handoff(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "governance-repair.json", governance_repair(handoff=False))

    assert run_gate(tmp_path) == 1


def test_observability_critical_alert_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path) == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(tmp_path / "unknown.json", {"schema": "sorafs.pdp.unknown.v1"})

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_unknown_directory_artifact_is_ignored_for_subset(tmp_path: Path) -> None:
    write_json(tmp_path / "provider-transport.json", provider_transport())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.pdp.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "provider_transport") == 0


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "provider-transport.json", provider_transport())
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path, "--require-kind", "provider_transport") == 1


def test_unknown_required_kind_returns_usage_error(tmp_path: Path) -> None:
    assert MODULE.main(["--evidence-dir", str(tmp_path), "--require-kind", "unknown"]) == 2
