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


NOW_UNIX = 1_800_600_000
GENERATED_AT = NOW_UNIX - 120
SUITE_DIGEST = "ab" * 32
STAGING_DIGEST = "cd" * 32
POLICY_DIGEST = "ef" * 32


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def base(schema: str) -> dict:
    return {
        "schema": schema,
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "deployment_id": "gateway-load-staging-a",
        "environment": "staging",
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


def staging_load(*, duration: int = 3_600, p95: int = 1_200) -> dict:
    streams = [{"name": f"stream-{index:04d}"} for index in range(1_200)]
    providers = [
        {"name": "provider-a"},
        {"name": "provider-b"},
        {"name": "provider-c"},
        {"name": "provider-d"},
    ]
    payload = base("sorafs.gateway_load.staging_load.v1")
    payload.update(
        {
            "suite_report_digest_hex": SUITE_DIGEST,
            "staging_report_digest_hex": STAGING_DIGEST,
            "fixture_bundle_digest_hex": POLICY_DIGEST,
            "policy_digest_hex": POLICY_DIGEST,
            "gateway_version": "iroha-gateway 1.0.0-rc.1",
            "hardware_profile": {"name": "staging-c6i-2xlarge"},
            "cache_state": {"mode": "cold-cache"},
            "duration_seconds": duration,
            "stream_count": len(streams),
            "streams": streams,
            "provider_count": len(providers),
            "providers": providers,
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


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


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


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "gateway-load.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
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
    write_json(tmp_path / "staging-load.json", staging_load(duration=600, p95=2_000))
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert "duration_seconds must be at least 3600" in artifact["errors"]
    assert "p95_latency_ms must be <= 1500" in artifact["errors"]


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
    write_complete_evidence(tmp_path)
    payload = telemetry_slo()
    payload["metrics"].append("sorafs_gateway_debug_metric")
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "telemetry-slo.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["telemetry_slo"]["artifacts"][0]
    assert "metrics must not include unknown values" in artifact["errors"]


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
    payload["streams"].append({"name": "stream-0000"})
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
    payload["providers"].append({"name": "provider-a"})
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
    payload["hardware_profile"] = {"name": "placeholder-hardware"}
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
    payload["hardware_profile"] = "staging-c6i-2xlarge"
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert "hardware_profile must be an object" in artifact["errors"]


def test_staging_cache_state_must_be_reviewed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["cache_state"] = {"mode": "debug-cache"}
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert MODULE.CACHE_STATE_ERROR in artifact["errors"]


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


def test_staging_provider_names_must_be_reviewed_labels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = staging_load()
    payload["providers"][0] = {"name": "provider-placeholder"}
    write_json(tmp_path / "staging-load.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["staging_load"]["artifacts"][0]
    assert (
        "providers[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_http3_committed_requires_passed_scenarios(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = transport_scope(http3_committed=True)
    payload["http3_scenarios_passed"] = False
    write_json(tmp_path / "transport-scope.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transport_scope"]["artifacts"][0]
    assert "http3_scenarios_passed must be true" in artifact["errors"]


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
