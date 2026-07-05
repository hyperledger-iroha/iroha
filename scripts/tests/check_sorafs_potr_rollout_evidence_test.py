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
PQ_KEY_ROSTER_DIGEST = "34" * 32
REPUTATION_POLICY_DIGEST = "56" * 32


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
        "deployment_context_reviewed": True,
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
    providers = [{"name": f"provider-{index:02d}"} for index in range(provider_count)]
    receipts = [
        {"name": f"potr-receipt-{index:02d}"} for index in range(receipt_count)
    ]
    payload.update(
        {
            "tier_count": len(["hot", "warm"] if tiers is None else tiers),
            "tiers_observed": ["hot", "warm"] if tiers is None else tiers,
            "gateway_receipts_captured": True,
            "range_fetch_verified": True,
            "deadline_headers_verified": True,
            "proof_stream_replay_verified": True,
            "trace_correlation_verified": True,
            "provider_count": provider_count,
            "providers": providers,
            "receipt_count": receipt_count,
            "receipts": receipts,
            "max_hot_latency_ms": hot_latency_ms,
            "max_warm_latency_ms": warm_latency_ms,
            "receipt_summary_digest_hex": DIGEST,
            "raw_receipts_included": False,
            "fetch_transcripts_included": False,
        }
    )
    return payload


def receipt_validation(
    *,
    pq_keys: bool = True,
    pq_key_roster_digest: str = PQ_KEY_ROSTER_DIGEST,
) -> dict:
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
            "pq_key_roster_digest_hex": pq_key_roster_digest,
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
        "body_blake3_hex": DIGEST,
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


def reputation_integration(
    *,
    governed: bool = True,
    reputation_weight_policy_digest: str = REPUTATION_POLICY_DIGEST,
) -> dict:
    payload = base("sorafs.potr.reputation_integration_canary.v1")
    payload.update(
        {
            "reputation_pipeline_consumed_receipts": True,
            "success_ratio_updated": True,
            "latency_percentiles_updated": True,
            "degradation_alert_linked": True,
            "reputation_weight_governed": governed,
            "reputation_weight_policy_digest_hex": reputation_weight_policy_digest,
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
            "metric_count": len(MODULE.REQUIRED_METRICS),
            "receipt_summary_digest_hex": DIGEST,
            "response_bodies_included": False,
        }
    )
    return payload


def governance_approval(
    *,
    pq_key_roster_digest: str = PQ_KEY_ROSTER_DIGEST,
    reputation_weight_policy_digest: str = REPUTATION_POLICY_DIGEST,
) -> dict:
    payload = base("sorafs.potr.governance_approval.v1")
    payload.update(
        {
            "approved": True,
            "governance_vote_recorded": True,
            "iroha_config_bound": True,
            "potr_policy_bound": True,
            "pq_key_roster_bound": True,
            "pq_key_roster_digest_hex": pq_key_roster_digest,
            "reputation_weight_bound": True,
            "reputation_weight_policy_digest_hex": reputation_weight_policy_digest,
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


RECEIPT_SUMMARY_BOUND_FIXTURES = (
    ("receipt_validation", "receipt-validation.json", receipt_validation),
    ("proof_stream", "proof-stream.json", proof_stream),
    ("reputation_integration", "reputation-integration.json", reputation_integration),
    ("observability", "observability.json", observability),
    ("governance_approval", "governance-approval.json", governance_approval),
)

PQ_KEY_ROSTER_BOUND_FIXTURES = (
    ("receipt_validation", "receipt-validation.json", receipt_validation),
)

REPUTATION_WEIGHT_BOUND_FIXTURES = (
    ("reputation_integration", "reputation-integration.json", reputation_integration),
)


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.potr.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["valid_policy_digests"] == [DIGEST]
    assert payload["required"]["multi_provider_probe"]["valid"] is True
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
        tuple(
            kind_name
            for kind_name, _file_name, _factory in RECEIPT_SUMMARY_BOUND_FIXTURES
        )
        == MODULE.RECEIPT_SUMMARY_BOUND_KINDS
    )
    assert (
        tuple(
            kind_name
            for kind_name, _file_name, _factory in PQ_KEY_ROSTER_BOUND_FIXTURES
        )
        == MODULE.PQ_KEY_ROSTER_BOUND_KINDS
    )
    assert (
        tuple(
            kind_name
            for kind_name, _file_name, _factory in REPUTATION_WEIGHT_BOUND_FIXTURES
        )
        == MODULE.REPUTATION_WEIGHT_BOUND_KINDS
    )


def test_fixture_inventories_cover_checker_required_sets() -> None:
    assert tuple(multi_provider_probe()["tiers_observed"]) == MODULE.REQUIRED_TIERS
    assert tuple(route["name"] for route in proof_stream()["routes"]) == (
        MODULE.REQUIRED_ROUTES
    )


def test_payload_safety_flags_are_required(tmp_path: Path) -> None:
    cases = (
        (
            "multi-provider-probe.json",
            "multi_provider_probe",
            multi_provider_probe,
            ("raw_receipts_included", "fetch_transcripts_included"),
        ),
        (
            "receipt-validation.json",
            "receipt_validation",
            receipt_validation,
            ("raw_receipt_bytes_included",),
        ),
        (
            "proof-stream.json",
            "proof_stream",
            proof_stream,
            ("response_bodies_included",),
        ),
        (
            "reputation-integration.json",
            "reputation_integration",
            reputation_integration,
            ("raw_reputation_inputs_included",),
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


def test_all_receipt_summary_bound_artifacts_reject_probe_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in RECEIPT_SUMMARY_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["receipt_summary_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} receipt_summary_digest_hex must reference a valid "
            "multi_provider_probe receipt_summary_digest_hex"
        ) in artifact["errors"]


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


def test_probe_provider_count_must_match_unique_providers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = multi_provider_probe()
    payload["provider_count"] += 1
    write_json(tmp_path / "multi-provider-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["multi_provider_probe"]["artifacts"][0]
    assert "provider_count must match unique providers count" in artifact["errors"]


def test_probe_providers_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = multi_provider_probe()
    payload["providers"].append(dict(payload["providers"][0]))
    payload["provider_count"] = len(payload["providers"])
    write_json(tmp_path / "multi-provider-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["multi_provider_probe"]["artifacts"][0]
    assert "providers must not contain duplicate values" in artifact["errors"]
    assert "provider_count must match unique providers count" in artifact["errors"]


def test_probe_provider_names_must_be_reviewed_labels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = multi_provider_probe()
    payload["providers"][0] = {"name": "provider_00"}
    write_json(tmp_path / "multi-provider-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["multi_provider_probe"]["artifacts"][0]
    assert (
        "providers[].name must match canonical lowercase `provider-*`"
        in artifact["errors"]
    )


def test_probe_provider_names_reject_non_production_markers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = multi_provider_probe()
    payload["providers"][0] = {"name": "provider-placeholder"}
    write_json(tmp_path / "multi-provider-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["multi_provider_probe"]["artifacts"][0]
    assert (
        "providers[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_probe_tiers_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = multi_provider_probe()
    payload["tiers_observed"].append(payload["tiers_observed"][0])
    write_json(tmp_path / "multi-provider-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["multi_provider_probe"]["artifacts"][0]
    assert "tiers_observed must not contain duplicate values" in artifact["errors"]


def test_probe_tiers_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = multi_provider_probe()
    payload["tiers_observed"].append("archive")
    payload["tier_count"] = len(payload["tiers_observed"])
    write_json(tmp_path / "multi-provider-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["multi_provider_probe"]["artifacts"][0]
    assert "tiers_observed must not include unknown values" in artifact["errors"]


def test_probe_tier_count_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = multi_provider_probe()
    del payload["tier_count"]
    write_json(tmp_path / "multi-provider-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["multi_provider_probe"]["artifacts"][0]
    assert "tier_count must be a positive integer" in artifact["errors"]
    assert "tier_count must be at least 2" in artifact["errors"]


def test_probe_tier_count_must_match_inventory(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = multi_provider_probe()
    payload["tier_count"] += 1
    write_json(tmp_path / "multi-provider-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["multi_provider_probe"]["artifacts"][0]
    assert "tier_count must match unique tiers_observed count" in artifact["errors"]


def test_probe_receipt_count_must_match_unique_receipts(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = multi_provider_probe()
    payload["receipt_count"] += 1
    write_json(tmp_path / "multi-provider-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["multi_provider_probe"]["artifacts"][0]
    assert "receipt_count must match unique receipts count" in artifact["errors"]


def test_probe_receipts_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = multi_provider_probe()
    payload["receipts"].append(dict(payload["receipts"][0]))
    payload["receipt_count"] = len(payload["receipts"])
    write_json(tmp_path / "multi-provider-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["multi_provider_probe"]["artifacts"][0]
    assert "receipts must not contain duplicate values" in artifact["errors"]
    assert "receipt_count must match unique receipts count" in artifact["errors"]


def test_probe_receipt_names_must_be_reviewed_labels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = multi_provider_probe()
    payload["receipts"][0]["name"] = "receipt-00"
    write_json(tmp_path / "multi-provider-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["multi_provider_probe"]["artifacts"][0]
    assert MODULE.RECEIPT_LABEL_ERROR in artifact["errors"]


def test_probe_receipt_names_reject_non_production_markers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = multi_provider_probe()
    payload["receipts"][0]["name"] = "potr-receipt-placeholder"
    write_json(tmp_path / "multi-provider-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["multi_provider_probe"]["artifacts"][0]
    assert (
        "receipts[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_hot_latency_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "multi-provider-probe.json", multi_provider_probe(hot_latency_ms=100_000))

    assert run_gate(tmp_path) == 1


def test_rollout_latency_evidence_must_be_integer_units(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    probe = multi_provider_probe()
    probe["max_hot_latency_ms"] = 80_000.5
    probe["max_warm_latency_ms"] = 260_000.5
    write_json(tmp_path / "multi-provider-probe.json", probe)
    stream = proof_stream()
    stream["routes"][0]["latency_ms"] = 12.5
    write_json(tmp_path / "proof-stream.json", stream)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    probe_errors = payload["required"]["multi_provider_probe"]["artifacts"][0][
        "errors"
    ]
    stream_errors = payload["required"]["proof_stream"]["artifacts"][0]["errors"]
    assert "max_hot_latency_ms must be a non-negative integer" in probe_errors
    assert "max_warm_latency_ms must be a non-negative integer" in probe_errors
    assert "routes[0].latency_ms must be a non-negative integer" in stream_errors


def test_receipt_validation_requires_pq_key_roster(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "receipt-validation.json", receipt_validation(pq_keys=False))

    assert run_gate(tmp_path) == 1


def test_receipt_validation_pq_key_roster_digest_must_match_governance(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    write_json(
        tmp_path / "receipt-validation.json",
        receipt_validation(pq_key_roster_digest=DIGEST_2),
    )

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["receipt_validation"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "receipt_validation pq_key_roster_digest_hex must reference a valid "
        "governance_approval pq_key_roster_digest_hex"
    ]


def test_all_pq_key_roster_bound_artifacts_reject_governance_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in PQ_KEY_ROSTER_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["pq_key_roster_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} pq_key_roster_digest_hex must reference a valid "
            "governance_approval pq_key_roster_digest_hex"
        ) in artifact["errors"]


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


def test_proof_stream_route_count_must_match_unique_routes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_stream()
    payload["route_count"] += 1
    payload["passed_route_count"] = payload["route_count"]
    write_json(tmp_path / "proof-stream.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_stream"]["artifacts"][0]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_proof_stream_routes_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_stream()
    payload["routes"].append(dict(payload["routes"][0]))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "proof-stream.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_stream"]["artifacts"][0]
    assert "routes must not contain duplicate values" in artifact["errors"]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_proof_stream_routes_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_stream()
    payload["routes"].append(route("proof_stream_debug"))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "proof-stream.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_stream"]["artifacts"][0]
    assert "routes must not include unknown values" in artifact["errors"]


def test_proof_stream_route_body_hash_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_stream()
    del payload["routes"][0]["body_blake3_hex"]
    write_json(tmp_path / "proof-stream.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_stream"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "routes[0].body_blake3_hex must be a non-empty string"
        in artifact["errors"]
    )


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


def test_reputation_weight_policy_digest_must_match_governance(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    write_json(
        tmp_path / "reputation-integration.json",
        reputation_integration(reputation_weight_policy_digest=DIGEST_2),
    )

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["reputation_integration"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "reputation_integration reputation_weight_policy_digest_hex must "
        "reference a valid governance_approval reputation_weight_policy_digest_hex"
    ]


def test_all_reputation_weight_bound_artifacts_reject_governance_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in REPUTATION_WEIGHT_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["reputation_weight_policy_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} reputation_weight_policy_digest_hex must reference a "
            "valid governance_approval reputation_weight_policy_digest_hex"
        ) in artifact["errors"]


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
    payload["metrics"].append("torii_sorafs_potr_debug_metric")
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "observability.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["observability"]["artifacts"][0]
    assert "metrics must not include unknown values" in artifact["errors"]


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
