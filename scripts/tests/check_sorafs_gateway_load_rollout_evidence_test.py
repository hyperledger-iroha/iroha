"""Tests for scripts/check_sorafs_gateway_load_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = (
    Path(__file__).resolve().parents[1]
    / "check_sorafs_gateway_load_rollout_evidence.py"
)
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_gateway_load_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

TOPOLOGY = sys.modules["sorafs_topology_qualification"]


NOW_UNIX = 1_800_600_000
GENERATED_AT = NOW_UNIX - 120
SUITE_DIGEST = "ab" * 32
STAGING_DIGEST = "cd" * 32
POLICY_DIGEST = "ef" * 32
FIXTURE_DIGEST = "34" * 32
ALT_DIGEST = "12" * 32


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def write_topology_qualification(
    root: Path,
    *,
    deployment_id: str = "gateway-load-prod-a",
    environment: str = "production",
) -> Path:
    path = root.parent / f"{root.name}-topology-qualification.json"
    return write_json(
        path,
        {
            "schema": TOPOLOGY.SUMMARY_SCHEMA,
            "status": "configuration-qualified",
            "qualification_scope": "pre-deployment-configuration",
            "live_evidence_recognized": False,
            "promotion_eligible": False,
            "manifest_sha256": "11" * 32,
            "canonical_manifest_sha256": "22" * 32,
            "deployment": {
                "deployment_id": deployment_id,
                "environment": environment,
            },
            "validator_count": 4,
            "storage_provider_count": 2,
            "gateway_count": 2,
            "governance_dag_instance_count": 2,
            "runtime_handle_kinds": ["monitoring", "hsm", "kms", "webauthn"],
            "runtime_material_policy_valid": True,
            "signed_model_artifact_count": 1,
            "required_lane_slots": list(TOPOLOGY.CANONICAL_READINESS_LANES),
            "recognized_lane_slot_count": len(TOPOLOGY.CANONICAL_READINESS_LANES),
            "errors": [],
        },
    )


def base(schema: str) -> dict:
    return {
        "schema": schema,
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "deployment_id": "gateway-load-prod-a",
        "environment": "production",
        "deployment_context_reviewed": True,
    }


def local_conformance() -> dict:
    payload = base("sorafs.gateway_load.local_conformance.v1")
    payload.update(
        {
            "ci_script": "ci/check_sorafs_gateway_conformance.sh",
            "cargo_command": (
                "cargo test -p integration_tests --test nexus_and_streaming "
                "sorafs_gateway_conformance -- --nocapture"
            ),
            "deterministic_harness_passed": True,
            "attestation_verified": True,
            "suite_report_digest_hex": SUITE_DIGEST,
            "scenario_count": len(MODULE.REQUIRED_SCENARIOS),
            "load_profile_streams": 1_000,
            "load_profile_window_seconds": 60,
            "scenarios": list(MODULE.REQUIRED_SCENARIOS),
            "raw_report_included": False,
            "private_keys_included": False,
        }
    )
    return payload


def staging_load(*, duration: int = 86_400, p95: int = 1_200) -> dict:
    streams = [{"name": f"gateway-load-stream-{index:04d}"} for index in range(1_200)]
    providers = [
        {"name": "gateway-load-provider-a"},
        {"name": "gateway-load-provider-b"},
        {"name": "gateway-load-provider-c"},
        {"name": "gateway-load-provider-d"},
    ]
    payload = base("sorafs.gateway_load.staging_load.v1")
    payload.update(
        {
            "suite_report_digest_hex": SUITE_DIGEST,
            "staging_report_digest_hex": STAGING_DIGEST,
            "fixture_bundle_digest_hex": FIXTURE_DIGEST,
            "policy_digest_hex": POLICY_DIGEST,
            "gateway_version": "iroha-gateway 1.0.0-rc.1",
            "hardware_profile": {"name": "gateway-load-hardware-c6i-2xlarge"},
            "cache_coverage": {
                "cold_cache_exercised": True,
                "warm_cache_exercised": True,
                "mixed_cache_exercised": True,
            },
            "duration_seconds": duration,
            "stream_count": len(streams),
            "streams": streams,
            "peak_concurrent_range_streams": 1_000,
            "provider_count": len(providers),
            "providers": providers,
            "load_conditions": {
                "corruption_injection_bps": 100,
                "revocation_exercised": True,
                "malformed_flood_exercised": True,
                "denylist_pressure_exercised": True,
                "rate_limit_pressure_exercised": True,
                "failover_exercised": True,
            },
            "success_rate_bps": 9_950,
            "error_rate_bps": 50,
            "p95_latency_ms": p95,
            "p99_latency_ms": 2_200,
            "response_bodies_included": False,
            "raw_payloads_included": False,
        }
    )
    return payload


def telemetry_slo() -> dict:
    payload = base("sorafs.gateway_load.telemetry_slo.v1")
    payload.update(
        {
            "staging_report_digest_hex": STAGING_DIGEST,
            "metrics_scrape_success": True,
            "dashboard_archived": True,
            "slo_baseline_recorded": True,
            "cold_cache_baseline_recorded": True,
            "critical_alerts_firing": False,
            "metrics": list(MODULE.REQUIRED_METRICS),
            "metric_count": len(MODULE.REQUIRED_METRICS),
            "response_bodies_included": False,
        }
    )
    return payload


def transport_scope(*, http3_committed: bool = False) -> dict:
    payload = base("sorafs.gateway_load.transport_scope.v1")
    payload.update(
        {
            "staging_report_digest_hex": STAGING_DIGEST,
            "http3_endpoint_committed": http3_committed,
            "http3_scenarios_deferred": not http3_committed,
            "http3_config_surface_documented": http3_committed,
            "http3_scenarios_passed": http3_committed,
            "transport_scope_reviewed": True,
            "response_bodies_included": False,
        }
    )
    return payload


def governance_approval() -> dict:
    payload = base("sorafs.gateway_load.governance_approval.v1")
    payload.update(
        {
            "approved": True,
            "governance_vote_recorded": True,
            "iroha_config_bound": True,
            "gateway_release_bound": True,
            "local_conformance_bound": True,
            "staging_load_bound": True,
            "telemetry_bound": True,
            "transport_scope_bound": True,
            "suite_report_digest_hex": SUITE_DIGEST,
            "staging_report_digest_hex": STAGING_DIGEST,
            "config_source": "iroha_config",
            "policy_digest_hex": POLICY_DIGEST,
        }
    )
    return payload


def write_complete_evidence(root: Path) -> None:
    write_json(root / "local-conformance.json", local_conformance())
    write_json(root / "staging-load.json", staging_load())
    write_json(root / "telemetry-slo.json", telemetry_slo())
    write_json(root / "transport-scope.json", transport_scope())
    write_json(root / "governance-approval.json", governance_approval())


STAGING_REPORT_BOUND_FIXTURES = (
    ("telemetry_slo", "telemetry-slo.json", telemetry_slo),
    ("transport_scope", "transport-scope.json", transport_scope),
    ("governance_approval", "governance-approval.json", governance_approval),
)

POLICY_BOUND_FIXTURES = (
    ("governance_approval", "governance-approval.json", governance_approval),
)


def run_gate(root: Path, *extra: str) -> int:
    topology = write_topology_qualification(root)
    return MODULE.main(
        [
            "--evidence-dir",
            str(root),
            "--now-unix",
            str(NOW_UNIX),
            "--topology-qualification-summary",
            str(topology),
            *extra,
        ]
    )


def validation_options() -> object:
    return MODULE.ValidationOptions(
        now_unix=NOW_UNIX,
        max_evidence_age_secs=MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS,
        min_staging_duration_secs=MODULE.DEFAULT_MIN_STAGING_DURATION_SECS,
        min_streams=MODULE.DEFAULT_MIN_STREAMS,
        min_success_rate_bps=MODULE.DEFAULT_MIN_SUCCESS_RATE_BPS,
        max_error_rate_bps=MODULE.DEFAULT_MAX_ERROR_RATE_BPS,
        max_p95_latency_ms=MODULE.DEFAULT_MAX_P95_LATENCY_MS,
        max_p99_latency_ms=MODULE.DEFAULT_MAX_P99_LATENCY_MS,
    )


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.gateway_load.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["staging_load"]["valid"] is True
    assert payload["valid_suite_report_digests"] == [SUITE_DIGEST]
    assert payload["valid_staging_report_digests"] == [STAGING_DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    assert payload["metrics"] == sorted(MODULE.REQUIRED_METRICS)
    assert payload["metric_count_values"] == [len(MODULE.REQUIRED_METRICS)]
    telemetry_artifact = payload["required"]["telemetry_slo"]["artifacts"][0]
    assert telemetry_artifact["fingerprint"]["metric_count"] == len(
        MODULE.REQUIRED_METRICS
    )
    assert telemetry_artifact["fingerprint"]["metrics"] == list(
        MODULE.REQUIRED_METRICS
    )
    staging_artifact = payload["required"]["staging_load"]["artifacts"][0]
    staging_fingerprint = staging_artifact["fingerprint"]
    assert staging_fingerprint["fixture_bundle_digest_hex"] == FIXTURE_DIGEST
    assert staging_fingerprint["gateway_version"] == "iroha-gateway 1.0.0-rc.1"
    assert staging_fingerprint["hardware_profile"] == {
        "name": "gateway-load-hardware-c6i-2xlarge"
    }
    assert staging_fingerprint["duration_seconds"] == 86_400
    assert staging_fingerprint["stream_count"] == 1_200
    assert staging_fingerprint["streams"] == staging_load()["streams"]
    assert staging_fingerprint["peak_concurrent_range_streams"] == 1_000
    assert staging_fingerprint["provider_count"] == 4
    assert staging_fingerprint["providers"] == staging_load()["providers"]
    assert staging_fingerprint["cache_coverage"] == {
        "cold_cache_exercised": True,
        "warm_cache_exercised": True,
        "mixed_cache_exercised": True,
    }
    assert staging_fingerprint["load_conditions"] == {
        "corruption_injection_bps": 100,
        "revocation_exercised": True,
        "malformed_flood_exercised": True,
        "denylist_pressure_exercised": True,
        "rate_limit_pressure_exercised": True,
        "failover_exercised": True,
    }


def test_topology_binding_must_match_gateway_load_deployment_context(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    topology = write_topology_qualification(
        tmp_path,
        deployment_id="different-gateway-load-prod",
    )
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
                "--topology-qualification-summary",
                str(topology),
            ]
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["status"] == "blocked"
    assert (
        "topology qualification deployment_id must match the reviewed lane context"
        in payload["errors"]
    )


def test_bound_fixture_tables_cover_checker_bound_kind_sets() -> None:
    assert (
        tuple(
            kind_name
            for kind_name, _file_name, _factory in STAGING_REPORT_BOUND_FIXTURES
        )
        == MODULE.STAGING_REPORT_BOUND_KINDS
    )
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in POLICY_BOUND_FIXTURES)
        == MODULE.POLICY_BOUND_KINDS
    )


def test_fixture_inventories_cover_checker_required_sets() -> None:
    assert (
        local_conformance()["cargo_command"]
        in MODULE.ALLOWED_GATEWAY_CONFORMANCE_CARGO_COMMANDS
    )
    assert tuple(local_conformance()["scenarios"]) == MODULE.REQUIRED_SCENARIOS
    assert frozenset(staging_load()["cache_coverage"]) == (
        MODULE.REQUIRED_CACHE_COVERAGE_FIELDS
    )
    assert frozenset(staging_load()["load_conditions"]) == (
        MODULE.REQUIRED_LOAD_CONDITION_FIELDS
    )
    assert tuple(telemetry_slo()["metrics"]) == MODULE.REQUIRED_METRICS


def test_payload_safety_flags_are_required(tmp_path: Path) -> None:
    cases = (
        (
            "local-conformance.json",
            "local_conformance",
            local_conformance,
            ("raw_report_included", "private_keys_included"),
        ),
        (
            "staging-load.json",
            "staging_load",
            staging_load,
            ("response_bodies_included", "raw_payloads_included"),
        ),
        (
            "telemetry-slo.json",
            "telemetry_slo",
            telemetry_slo,
            ("critical_alerts_firing", "response_bodies_included"),
        ),
        (
            "transport-scope.json",
            "transport_scope",
            transport_scope,
            (
                "http3_endpoint_committed",
                "http3_config_surface_documented",
                "http3_scenarios_passed",
                "response_bodies_included",
            ),
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


def test_transport_scope_requires_explicit_http3_non_applicability() -> None:
    payload = transport_scope()
    payload.pop("http3_scenarios_deferred")

    kind, errors = MODULE.validate_evidence_payload(payload, validation_options())

    assert kind == "transport_scope"
    assert "http3_scenarios_deferred must be true" in errors


def test_staging_gateway_version_must_be_concrete() -> None:
    for version in ("iroha-gateway 1.0.0", "iroha-gateway 1.0.0-rc.1"):
        payload = staging_load()
        payload["gateway_version"] = version
        kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert kind == "staging_load"
        assert errors == []

    for version in (
        "latest",
        "1.0.0",
        "iroha-gateway 01.0.0",
        "iroha-gateway 1.0.0-rc.0",
        "iroha-gateway 1.0.0-dev",
    ):
        payload = staging_load()
        payload["gateway_version"] = version
        kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert kind == "staging_load"
        assert MODULE.GATEWAY_VERSION_ERROR in errors


def test_gateway_load_environment_rejects_production_aliases() -> None:
    for environment in ("prod", "staging", "Production"):
        payload = staging_load()
        payload["environment"] = environment
        kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert kind == "staging_load"
        assert "environment must be `production`" in errors


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    topology = write_topology_qualification(tmp_path)
    args = tmp_path / "gateway-load.args"
    args.write_text(
        (
            f"--evidence-dir {tmp_path}\n"
            f"--now-unix {NOW_UNIX}\n"
            f"--topology-qualification-summary {topology}\n"
        ),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args}"]) == 0


def test_local_conformance_cargo_command_must_be_reviewed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = local_conformance()
    payload["cargo_command"] = MODULE.LOCKED_GATEWAY_CONFORMANCE_CARGO_COMMAND
    write_json(tmp_path / "local-conformance.json", payload)
    assert run_gate(tmp_path) == 0

    for command in (
        "echo sorafs_gateway_conformance",
        (
            f"{MODULE.DEFAULT_GATEWAY_CONFORMANCE_CARGO_COMMAND} "
            "&& cat /tmp/private-key"
        ),
    ):
        payload = local_conformance()
        payload["cargo_command"] = command
        write_json(tmp_path / "local-conformance.json", payload)
        summary = tmp_path / "summary.json"
        summary.unlink(missing_ok=True)

        assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"]["local_conformance"]["artifacts"][0]
        assert "cargo_command must be cargo test" in "\n".join(
            artifact["errors"]
        )


def test_raw_report_leakage_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = local_conformance()
    payload["raw_report"] = {"private": "details"}
    write_json(tmp_path / "local-conformance.json", payload)

    assert run_gate(tmp_path) == 1


def test_staging_load_thresholds_fail(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    staging = staging_load(duration=600, p95=2_000)
    staging["peak_concurrent_range_streams"] = 999
    write_json(tmp_path / "staging-load.json", staging)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert "duration_seconds must be at least 86400" in artifact["errors"]
    assert (
        "peak_concurrent_range_streams must be at least 1000"
        in artifact["errors"]
    )
    assert "p95_latency_ms must be <= 1500" in artifact["errors"]


def test_staging_load_integer_unit_metrics_must_be_non_negative_ints(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["success_rate_bps"] = 9_950.5
    payload["error_rate_bps"] = 12.5
    payload["p95_latency_ms"] = -1
    payload["p99_latency_ms"] = 2_200.5
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert "success_rate_bps must be a positive integer" in artifact["errors"]
    assert "error_rate_bps must be a non-negative integer" in artifact["errors"]
    assert "p95_latency_ms must be a non-negative integer" in artifact["errors"]
    assert "p99_latency_ms must be a non-negative integer" in artifact["errors"]


def test_staging_load_success_rate_bps_must_be_basis_points(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["success_rate_bps"] = 10_001
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert "success_rate_bps must be <= 10000" in artifact["errors"]


def test_staging_load_error_rate_bps_must_be_basis_points() -> None:
    payload = staging_load()
    payload["error_rate_bps"] = 10_001
    options = MODULE.ValidationOptions(
        now_unix=NOW_UNIX,
        max_evidence_age_secs=MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS,
        min_staging_duration_secs=MODULE.DEFAULT_MIN_STAGING_DURATION_SECS,
        min_streams=MODULE.DEFAULT_MIN_STREAMS,
        min_success_rate_bps=MODULE.DEFAULT_MIN_SUCCESS_RATE_BPS,
        max_error_rate_bps=MODULE.MAX_ERROR_RATE_BPS + 1,
        max_p95_latency_ms=MODULE.DEFAULT_MAX_P95_LATENCY_MS,
        max_p99_latency_ms=MODULE.DEFAULT_MAX_P99_LATENCY_MS,
    )

    kind, errors = MODULE.validate_evidence_payload(payload, options)

    assert kind == "staging_load"
    assert "error_rate_bps must be <= 10000" in errors


def test_basis_point_thresholds_must_be_possible(
    tmp_path: Path,
    capsys,
) -> None:
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--summary-out",
                str(summary),
                "--now-unix",
                str(NOW_UNIX),
                "--topology-qualification-summary",
                str(write_topology_qualification(tmp_path)),
                "--min-success-rate-bps",
                "10001",
                "--max-error-rate-bps",
                "10001",
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "--min-success-rate-bps must be <= 10000" in captured.err
    assert "--max-error-rate-bps must be <= 10000" in captured.err
    assert not summary.exists()


def test_production_duration_and_concurrency_thresholds_cannot_be_weakened(
    tmp_path: Path,
    capsys,
) -> None:
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--summary-out",
                str(summary),
                "--now-unix",
                str(NOW_UNIX),
                "--topology-qualification-summary",
                str(write_topology_qualification(tmp_path)),
                "--min-staging-duration-secs",
                "86399",
                "--min-streams",
                "999",
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "--min-staging-duration-secs must be >= 86400" in captured.err
    assert "--min-streams must be >= 1000" in captured.err
    assert not summary.exists()

    options = validation_options()
    weakened_options = MODULE.ValidationOptions(
        now_unix=options.now_unix,
        max_evidence_age_secs=options.max_evidence_age_secs,
        min_staging_duration_secs=1,
        min_streams=1,
        min_success_rate_bps=options.min_success_rate_bps,
        max_error_rate_bps=options.max_error_rate_bps,
        max_p95_latency_ms=options.max_p95_latency_ms,
        max_p99_latency_ms=options.max_p99_latency_ms,
    )
    payload = staging_load(duration=86_399)
    payload["peak_concurrent_range_streams"] = 999
    kind, errors = MODULE.validate_evidence_payload(payload, weakened_options)
    assert kind == "staging_load"
    assert "duration_seconds must be at least 86400" in errors
    assert "peak_concurrent_range_streams must be at least 1000" in errors


def test_telemetry_requires_gateway_metrics(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = telemetry_slo()
    payload["metrics"] = ["sorafs_gateway_latency_ms_bucket"]
    write_json(tmp_path / "telemetry-slo.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["telemetry_slo"]["artifacts"][0]
    assert (
        "metrics must include value `sorafs_gateway_refusals_total`"
        in artifact["errors"]
    )


def test_telemetry_metrics_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = telemetry_slo()
    payload["metrics"].append(payload["metrics"][0])
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "telemetry-slo.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["telemetry_slo"]["artifacts"][0]
    assert "metrics must not contain duplicate values" in artifact["errors"]
    assert "metric_count must match unique metrics count" in artifact["errors"]


def test_telemetry_metrics_must_not_include_unknown_values(tmp_path: Path) -> None:
    allowed_metric = MODULE.REQUIRED_METRICS[0]
    for index, metric in enumerate(
        (
            "sorafs_gateway_debug_metric",
            f" {allowed_metric}",
            f"{allowed_metric} ",
            f"{allowed_metric}\u200d",
            f"{allowed_metric}\u202e",
        )
    ):
        evidence_dir = tmp_path / f"case-{index}"
        evidence_dir.mkdir()
        write_complete_evidence(evidence_dir)
        payload = telemetry_slo()
        payload["metrics"].append(metric)
        payload["metric_count"] = len(payload["metrics"])
        write_json(evidence_dir / "telemetry-slo.json", payload)
        summary = evidence_dir / "summary.json"

        assert run_gate(evidence_dir, "--summary-out", str(summary)) == 1
        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"]["telemetry_slo"]["artifacts"][0]
        assert "metrics must not include unknown values" in artifact["errors"]
        diagnostics = "\n".join(artifact["errors"])
        assert metric not in diagnostics
        assert metric.encode("unicode_escape").decode("ascii") not in diagnostics


def test_local_conformance_scenario_count_must_match_unique_scenarios(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = local_conformance()
    payload["scenario_count"] = len(MODULE.REQUIRED_SCENARIOS) + 1
    write_json(tmp_path / "local-conformance.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["local_conformance"]["artifacts"][0]
    assert "scenario_count must match unique scenarios count" in artifact["errors"]


def test_local_conformance_scenarios_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = local_conformance()
    payload["scenarios"].append(payload["scenarios"][0])
    payload["scenario_count"] = len(payload["scenarios"])
    write_json(tmp_path / "local-conformance.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["local_conformance"]["artifacts"][0]
    assert "scenarios must not contain duplicate values" in artifact["errors"]
    assert "scenario_count must match unique scenarios count" in artifact["errors"]


def test_local_conformance_scenarios_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = local_conformance()
    payload["scenarios"].append("debug_gateway_path")
    payload["scenario_count"] = len(payload["scenarios"])
    write_json(tmp_path / "local-conformance.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["local_conformance"]["artifacts"][0]
    assert "scenarios must not include unknown values" in artifact["errors"]


def test_staging_load_stream_count_must_match_unique_streams(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["stream_count"] += 1
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert "stream_count must match unique streams count" in artifact["errors"]


def test_staging_load_streams_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["streams"].append({"name": "gateway-load-stream-0000"})
    payload["stream_count"] = len(payload["streams"])
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert "streams must not contain duplicate values" in artifact["errors"]
    assert "stream_count must match unique streams count" in artifact["errors"]


def test_staging_load_provider_count_must_match_unique_providers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["provider_count"] += 1
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert "provider_count must match unique providers count" in artifact["errors"]


def test_staging_load_providers_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["providers"].append({"name": "gateway-load-provider-a"})
    payload["provider_count"] = len(payload["providers"])
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert "providers must not contain duplicate values" in artifact["errors"]
    assert "provider_count must match unique providers count" in artifact["errors"]


def test_staging_hardware_profile_must_be_reviewed_label(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["hardware_profile"] = {"name": "gateway-load-hardware-placeholder"}
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert (
        "hardware_profile.name must not contain non-production markers "
        "['placeholder']"
    ) in artifact["errors"]


def test_staging_hardware_profile_must_be_object(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["hardware_profile"] = "gateway-load-hardware-c6i-2xlarge"
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert "hardware_profile must be an object" in artifact["errors"]


def test_staging_cache_coverage_requires_every_exact_flag() -> None:
    for field in sorted(MODULE.REQUIRED_CACHE_COVERAGE_FIELDS):
        payload = staging_load()
        payload["cache_coverage"][field] = False
        kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert kind == "staging_load"
        assert f"{field} must be true" in errors

        payload = staging_load()
        payload["cache_coverage"].pop(field)
        kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert kind == "staging_load"
        assert f"cache_coverage is missing required fields: {field}" in errors


def test_staging_load_conditions_require_exact_corruption_and_every_flag() -> None:
    for value in (0, 99, 101, 1_000, True, 100.0, "100"):
        payload = staging_load()
        payload["load_conditions"]["corruption_injection_bps"] = value
        kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert kind == "staging_load"
        assert "corruption_injection_bps must be exactly 100" in errors

    exercise_fields = MODULE.REQUIRED_LOAD_CONDITION_FIELDS - frozenset(
        ("corruption_injection_bps",)
    )
    for field in sorted(exercise_fields):
        payload = staging_load()
        payload["load_conditions"][field] = False
        kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert kind == "staging_load"
        assert f"{field} must be true" in errors

        payload = staging_load()
        payload["load_conditions"].pop(field)
        kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert kind == "staging_load"
        assert f"load_conditions is missing required fields: {field}" in errors


def test_staging_requires_two_distinct_providers() -> None:
    payload = staging_load()
    payload["providers"] = payload["providers"][:2]
    payload["provider_count"] = 2
    kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
    assert kind == "staging_load"
    assert errors == []

    payload = staging_load()
    payload["providers"] = payload["providers"][:1]
    payload["provider_count"] = 1

    kind, errors = MODULE.validate_evidence_payload(payload, validation_options())

    assert kind == "staging_load"
    assert "provider_count must be at least 2" in errors


def test_peak_concurrency_cannot_exceed_stream_inventory() -> None:
    payload = staging_load()
    payload["peak_concurrent_range_streams"] = payload["stream_count"] + 1

    kind, errors = MODULE.validate_evidence_payload(payload, validation_options())

    assert kind == "staging_load"
    assert "peak_concurrent_range_streams must be <= unique streams count" in errors


def test_staging_rejects_legacy_aliases_and_unknown_fields() -> None:
    for alias in sorted(MODULE.REMOVED_STAGING_LOAD_FIELDS):
        payload = staging_load()
        payload[alias] = True
        kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert kind == "staging_load"
        assert f"{alias} is a removed staging-load V1 field" in errors
        assert f"staging_load payload contains unknown fields: {alias}" in errors


def test_gateway_load_objects_are_schema_closed() -> None:
    cases = (
        (local_conformance(), "local_conformance payload", "unexpected"),
        (staging_load(), "staging_load payload", "unexpected"),
        (telemetry_slo(), "telemetry_slo payload", "unexpected"),
        (transport_scope(), "transport_scope payload", "unexpected"),
        (governance_approval(), "governance_approval payload", "unexpected"),
    )
    for payload, path, extra in cases:
        payload[extra] = True
        _kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert f"{path} contains unknown fields: {extra}" in errors

    nested_cases = (
        ("hardware_profile", "hardware_profile"),
        ("cache_coverage", "cache_coverage"),
        ("load_conditions", "load_conditions"),
    )
    for field, path in nested_cases:
        payload = staging_load()
        payload[field]["unexpected"] = True
        _kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert f"{path} contains unknown fields: unexpected" in errors

    for field in ("streams", "providers"):
        payload = staging_load()
        payload[field][0]["unexpected"] = True
        _kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert f"{field}[] contains unknown fields: unexpected" in errors


def test_staging_new_contract_fields_cannot_be_omitted() -> None:
    for field in (
        "cache_coverage",
        "duration_seconds",
        "peak_concurrent_range_streams",
        "provider_count",
        "providers",
        "load_conditions",
    ):
        payload = staging_load()
        payload.pop(field)
        kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert kind == "staging_load"
        assert f"staging_load payload is missing required fields: {field}" in errors


def test_staging_hardware_profile_must_use_gateway_load_family(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["hardware_profile"] = {"name": "staging-c6i-2xlarge"}
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert MODULE.HARDWARE_PROFILE_ERROR in artifact["errors"]


def test_staging_stream_names_must_be_generated_labels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["streams"][0] = {"name": "debug-stream"}
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert MODULE.STREAM_NAME_ERROR in artifact["errors"]


def test_staging_stream_names_reject_generic_stream_family(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["streams"][0] = {"name": "stream-0000"}
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert MODULE.STREAM_NAME_ERROR in artifact["errors"]


def test_staging_provider_names_must_be_reviewed_labels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["providers"][0] = {"name": "gateway-load-provider-placeholder"}
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert (
        "providers[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_staging_provider_names_must_use_production_family(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["providers"][0] = {"name": "provider-a"}
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert MODULE.PROVIDER_NAME_ERROR in artifact["errors"]


def test_http3_is_not_a_v1_gateway_load_requirement(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = transport_scope(http3_committed=True)
    write_json(tmp_path / "transport-scope.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transport_scope"]["artifacts"][0]
    assert "http3_endpoint_committed must be false" in artifact["errors"]
    assert "http3_scenarios_deferred must be true" in artifact["errors"]
    assert "http3_config_surface_documented must be false" in artifact["errors"]
    assert "http3_scenarios_passed must be false" in artifact["errors"]


def test_staging_digest_binding_must_match_load_report(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_approval()
    payload["staging_report_digest_hex"] = "12" * 32
    write_json(tmp_path / "governance-approval.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_approval"]["artifacts"][0]
    assert (
        "governance_approval staging_report_digest_hex must reference a valid "
        "staging_load staging_report_digest_hex"
    ) in artifact["errors"]


def test_all_staging_bound_artifacts_reject_staging_report_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in STAGING_REPORT_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["staging_report_digest_hex"] = "12" * 32
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} staging_report_digest_hex must reference a valid "
            "staging_load staging_report_digest_hex"
        ) in artifact["errors"]


def test_staging_load_policy_digest_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload.pop("policy_digest_hex")
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "policy_digest_hex must be a non-empty string" in artifact["errors"]
    assert payload["valid_policy_digests"] == []


def test_governance_approval_policy_digest_must_match_staging_load(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_approval()
    payload["policy_digest_hex"] = "12" * 32
    write_json(tmp_path / "governance-approval.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    assert (
        "governance_approval policy_digest_hex must reference a valid "
        "staging_load policy_digest_hex"
    ) in artifact["errors"]


def test_all_policy_bound_artifacts_reject_staging_load_policy_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in POLICY_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["policy_digest_hex"] = "12" * 32
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert result["valid_policy_digests"] == [POLICY_DIGEST]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} policy_digest_hex must reference a valid "
            "staging_load policy_digest_hex"
        ) in artifact["errors"]


def test_multiple_valid_suite_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = local_conformance()
    payload["suite_report_digest_hex"] = ALT_DIGEST
    write_json(tmp_path / "local-conformance-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_suite_report_digests"] == []
    assert (
        "valid_suite_report_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_staging_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["staging_report_digest_hex"] = ALT_DIGEST
    write_json(tmp_path / "staging-load-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_staging_report_digests"] == []
    assert (
        "valid_staging_report_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_policy_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["policy_digest_hex"] = ALT_DIGEST
    write_json(tmp_path / "staging-load-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_policy_digests"] == []
    assert (
        "valid_policy_digests must contain exactly one active digest"
        in result["errors"]
    )
