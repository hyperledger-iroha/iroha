"""Tests for scripts/check_sorafs_potr_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_potr_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location("check_sorafs_potr_rollout_evidence", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


NOW_UNIX = 1_800_600_000
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
        "deployment_id": "potr-staging-a",
        "environment": "staging",
    }


def multi_provider_probe(
    *,
    provider_count: int = 3,
    receipt_count: int = 6,
    hot_latency_ms: int = 80_000,
    warm_latency_ms: int = 260_000,
    tiers: list[str] | None = None,
) -> dict:
    payload = base("sorafs.potr.multi_provider_probe_canary.v1")
    payload.update(
        {
            "tiers_observed": ["hot", "warm"] if tiers is None else tiers,
            "gateway_receipts_captured": True,
            "range_fetch_verified": True,
            "deadline_headers_verified": True,
            "proof_stream_replay_verified": True,
            "trace_correlation_verified": True,
            "provider_count": provider_count,
            "receipt_count": receipt_count,
            "max_hot_latency_ms": hot_latency_ms,
            "max_warm_latency_ms": warm_latency_ms,
            "receipt_summary_digest_hex": DIGEST,
            "raw_receipts_included": False,
            "fetch_transcripts_included": False,
        }
    )
    return payload


def receipt_validation(*, pq_keys: bool = True) -> dict:
    payload = base("sorafs.potr.receipt_validation_canary.v1")
    payload.update(
        {
            "sorafs_validate_potr_passed": True,
            "schema_version_verified": True,
            "range_bounds_verified": True,
            "timestamp_ordering_verified": True,
            "deadline_policy_verified": True,
            "gateway_signature_verified": True,
            "provider_signature_policy_enforced": True,
            "provider_pq_keys_governed": pq_keys,
            "ml_dsa_provider_signature_verified": True,
            "receipts_validated": 6,
            "receipt_summary_digest_hex": DIGEST,
            "validation_bundle_digest_hex": DIGEST,
            "raw_receipt_bytes_included": False,
        }
    )
    return payload


def route(name: str, *, latency_ms: int = 200, norito: bool = True) -> dict:
    return {
        "name": name,
        "passed": True,
        "status_code": 200,
        "latency_ms": latency_ms,
        "norito_verified": norito,
    }


def proof_stream(*, norito: bool = True) -> dict:
    routes = [
        route(name, norito=norito)
        for name in ("gateway_range_fetch", "proof_stream_potr", "proof_stream_filter")
    ]
    payload = base("sorafs.potr.proof_stream_canary.v1")
    payload.update(
        {
            "route_count": len(routes),
            "passed_route_count": len(routes),
            "manifest_filter_verified": True,
            "provider_filter_verified": True,
            "tier_filter_verified": True,
            "replay_window_bounded": True,
            "invalid_receipts_suppressed": True,
            "receipt_summary_digest_hex": DIGEST,
            "response_bodies_included": False,
            "routes": routes,
        }
    )
    return payload


def reputation_integration(*, governed: bool = True) -> dict:
    payload = base("sorafs.potr.reputation_integration_canary.v1")
    payload.update(
        {
            "reputation_pipeline_consumed_receipts": True,
            "success_ratio_updated": True,
            "latency_percentiles_updated": True,
            "degradation_alert_linked": True,
            "reputation_weight_governed": governed,
            "missed_deadline_penalty_bound": True,
            "receipt_summary_digest_hex": DIGEST,
            "stats_digest_hex": DIGEST,
            "raw_reputation_inputs_included": False,
        }
    )
    return payload


def observability(*, critical: bool = False) -> dict:
    payload = base("sorafs.potr.observability_canary.v1")
    payload.update(
        {
            "metrics_scrape_success": True,
            "dashboard_provisioned": True,
            "alert_rules_installed": True,
            "deadline_breach_alert_tested": True,
            "critical_alerts_firing": critical,
            "metrics": [
                "torii_sorafs_proof_stream_events_total",
                "torii_sorafs_proof_stream_latency_ms_bucket",
                "torii_sorafs_proof_stream_inflight",
                "torii_sorafs_proof_health_potr_breaches",
                "torii_da_potr_bonus_micro_total",
            ],
            "receipt_summary_digest_hex": DIGEST,
            "response_bodies_included": False,
        }
    )
    return payload


def governance_approval() -> dict:
    payload = base("sorafs.potr.governance_approval.v1")
    payload.update(
        {
            "approved": True,
            "governance_vote_recorded": True,
            "iroha_config_bound": True,
            "potr_policy_bound": True,
            "pq_key_roster_bound": True,
            "reputation_weight_bound": True,
            "governance_dag_bound": True,
            "config_source": "iroha_config",
            "receipt_summary_digest_hex": DIGEST,
            "policy_digest_hex": DIGEST,
        }
    )
    return payload


def write_complete_evidence(root: Path) -> None:
    write_json(root / "multi-provider-probe.json", multi_provider_probe())
    write_json(root / "receipt-validation.json", receipt_validation())
    write_json(root / "proof-stream.json", proof_stream())
    write_json(root / "reputation-integration.json", reputation_integration())
    write_json(root / "observability.json", observability())
    write_json(root / "governance-approval.json", governance_approval())


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.potr.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["multi_provider_probe"]["valid"] is True


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "potr.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args}"]) == 0


def test_missing_probe_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "multi-provider-probe.json").unlink()

    assert run_gate(tmp_path) == 1


def test_stale_probe_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = multi_provider_probe()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "multi-provider-probe.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["receipt_validation"]
    artifact = required["artifacts"][0]
    assert payload["valid_receipt_summary_digests"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "receipt_validation receipt_summary_digest_hex requires a valid "
        "multi_provider_probe receipt_summary_digest_hex"
    ]


def test_raw_receipt_leakage_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = multi_provider_probe()
    payload["raw_receipt"] = "leaked"
    write_json(tmp_path / "multi-provider-probe.json", payload)

    assert run_gate(tmp_path) == 1


def test_probe_requires_hot_and_warm_tiers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "multi-provider-probe.json", multi_provider_probe(tiers=["hot"]))

    assert run_gate(tmp_path) == 1


def test_probe_requires_minimum_provider_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "multi-provider-probe.json", multi_provider_probe(provider_count=2))

    assert run_gate(tmp_path) == 1


def test_probe_requires_minimum_receipt_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "multi-provider-probe.json", multi_provider_probe(receipt_count=5))

    assert run_gate(tmp_path) == 1


def test_hot_latency_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "multi-provider-probe.json", multi_provider_probe(hot_latency_ms=100_000))

    assert run_gate(tmp_path) == 1


def test_receipt_validation_requires_pq_key_roster(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "receipt-validation.json", receipt_validation(pq_keys=False))

    assert run_gate(tmp_path) == 1


def test_receipt_validation_requires_receipt_summary_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = receipt_validation()
    del payload["receipt_summary_digest_hex"]
    write_json(tmp_path / "receipt-validation.json", payload)

    assert run_gate(tmp_path) == 1


def test_proof_stream_requires_norito_routes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "proof-stream.json", proof_stream(norito=False))

    assert run_gate(tmp_path) == 1


def test_reputation_receipt_summary_digest_must_match(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = reputation_integration()
    payload["receipt_summary_digest_hex"] = DIGEST_2
    write_json(tmp_path / "reputation-integration.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["reputation_integration"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "reputation_integration receipt_summary_digest_hex must reference a valid "
        "multi_provider_probe receipt_summary_digest_hex"
    ]


def test_reputation_weight_must_be_governed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "reputation-integration.json", reputation_integration(governed=False))

    assert run_gate(tmp_path) == 1


def test_observability_critical_alert_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path) == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(tmp_path / "unknown.json", {"schema": "sorafs.potr.unknown.v1"})

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_unknown_directory_artifact_is_ignored_for_subset(tmp_path: Path) -> None:
    write_json(tmp_path / "multi-provider-probe.json", multi_provider_probe())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.potr.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "multi_provider_probe") == 0


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "multi-provider-probe.json", multi_provider_probe())
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path, "--require-kind", "multi_provider_probe") == 1


def test_unknown_required_kind_returns_usage_error(tmp_path: Path) -> None:
    assert MODULE.main(["--evidence-dir", str(tmp_path), "--require-kind", "unknown"]) == 2
