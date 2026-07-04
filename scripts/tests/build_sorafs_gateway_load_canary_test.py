"""Tests for scripts/build_sorafs_gateway_load_canary.py."""

from __future__ import annotations

import importlib.util
import json
import shlex
import sys
from pathlib import Path

import pytest


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_gateway_load_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_gateway_load_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_gateway_load_canary",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_gateway_load_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


NOW_UNIX = 1_800_600_000
GENERATED_AT = NOW_UNIX - 120
SUITE_DIGEST = "a" * 64
STAGING_DIGEST = "b" * 64
FIXTURE_DIGEST = "c" * 64
POLICY_DIGEST = "d" * 64


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "gateway-load-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(NOW_UNIX),
    ]
    if kind in MODULE.SUITE_DIGEST_KINDS:
        args.extend(["--suite-report-digest-hex", SUITE_DIGEST])
    if kind in MODULE.STAGING_DIGEST_KINDS:
        args.extend(["--staging-report-digest-hex", STAGING_DIGEST])
    if kind in MODULE.POLICY_DIGEST_KINDS:
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    if kind == "local_conformance":
        for scenario in MODULE.REQUIRED_SCENARIOS:
            args.extend(["--scenario", scenario])
    elif kind == "staging_load":
        args.extend(
            [
                "--fixture-bundle-digest-hex",
                FIXTURE_DIGEST,
                "--gateway-version",
                "iroha-gateway 1.0.0",
                "--hardware-profile",
                "gateway-load-hardware-c6i-2xlarge",
                "--cache-state",
                "cold-cache",
                "--duration-seconds",
                str(CHECKER.DEFAULT_MIN_STAGING_DURATION_SECS),
                "--stream-count",
                "1200",
                "--provider-count",
                "4",
                "--success-rate-bps",
                "9950",
                "--error-rate-bps",
                "50",
                "--p95-latency-ms",
                "1200",
                "--p99-latency-ms",
                "2200",
            ]
        )
        for provider in (
            "gateway-load-provider-a",
            "gateway-load-provider-b",
            "gateway-load-provider-c",
            "gateway-load-provider-d",
        ):
            args.extend(["--provider", provider])
    elif kind == "telemetry_slo":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    return args


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=NOW_UNIX,
        max_evidence_age_secs=CHECKER.DEFAULT_MAX_EVIDENCE_AGE_SECS,
        min_staging_duration_secs=CHECKER.DEFAULT_MIN_STAGING_DURATION_SECS,
        min_streams=CHECKER.DEFAULT_MIN_STREAMS,
        min_success_rate_bps=CHECKER.DEFAULT_MIN_SUCCESS_RATE_BPS,
        max_error_rate_bps=CHECKER.DEFAULT_MAX_ERROR_RATE_BPS,
        max_p95_latency_ms=CHECKER.DEFAULT_MAX_P95_LATENCY_MS,
        max_p99_latency_ms=CHECKER.DEFAULT_MAX_P99_LATENCY_MS,
    )


def assert_rejected_without_artifact(
    args: list[str],
    *,
    kind: str,
    tmp_path: Path,
    capsys,
    expected_error: str,
) -> None:
    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, kind).exists()


def test_builds_payload_free_staging_load_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("staging_load", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "staging_load").read_text("utf-8"))

    assert payload["schema"] == "sorafs.gateway_load.staging_load.v1"
    assert payload["suite_report_digest_hex"] == SUITE_DIGEST
    assert payload["staging_report_digest_hex"] == STAGING_DIGEST
    assert payload["policy_digest_hex"] == POLICY_DIGEST
    assert payload["stream_count"] == 1200
    assert payload["streams"][0] == {"name": "gateway-load-stream-0000"}
    assert payload["streams"][-1] == {"name": "gateway-load-stream-1199"}
    assert payload["provider_count"] == 4
    assert payload["providers"] == [
        {"name": "gateway-load-provider-a"},
        {"name": "gateway-load-provider-b"},
        {"name": "gateway-load-provider-c"},
        {"name": "gateway-load-provider-d"},
    ]
    assert payload["response_bodies_included"] is False
    assert payload["raw_payloads_included"] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "staging_load"
    assert errors == []


def test_generated_canaries_pass_full_gateway_load_gate(tmp_path: Path) -> None:
    evidence_paths: list[Path] = []
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        evidence_paths.append(canary_path(tmp_path, kind))
    summary = tmp_path / "summary.json"

    command = ["--now-unix", str(NOW_UNIX)]
    for path in evidence_paths:
        command.extend(["--evidence", str(path)])
    command.extend(["--summary-out", str(summary)])

    assert CHECKER.main(command) == 0

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["valid_suite_report_digests"] == [SUITE_DIGEST]
    assert payload["valid_staging_report_digests"] == [STAGING_DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_response_file_can_build_telemetry_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "telemetry.args"
    args_file.write_text(
        "\n".join(args_for("telemetry_slo", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "telemetry_slo").read_text("utf-8"))
    assert payload["metrics"] == list(MODULE.REQUIRED_METRICS)


def test_response_file_can_build_staging_canary_with_spaced_version(
    tmp_path: Path,
) -> None:
    args_file = tmp_path / "staging.args"
    args_file.write_text(
        "\n".join(shlex.quote(arg) for arg in args_for("staging_load", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "staging_load").read_text("utf-8"))
    assert payload["gateway_version"] == "iroha-gateway 1.0.0"
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "staging_load"
    assert errors == []


def test_success_rate_bps_rejects_out_of_range_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("staging_load", tmp_path)
    args[args.index("--success-rate-bps") + 1] = "10001"

    assert_rejected_without_artifact(
        args,
        kind="staging_load",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--success-rate-bps must be <= 10000",
    )


def test_missing_metric_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("telemetry_slo", tmp_path)
    index = args.index("--metric")
    del args[index : index + 2]

    assert_rejected_without_artifact(
        args,
        kind="telemetry_slo",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--metric must include every required value",
    )


def test_unknown_scenario_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("local_conformance", tmp_path)
    args.extend(["--scenario", "debug-live-http3"])

    assert_rejected_without_artifact(
        args,
        kind="local_conformance",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--scenario contains an unknown value",
    )


def test_duplicate_scenario_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("local_conformance", tmp_path)
    first_scenario = args.index("--scenario") + 1
    args.extend(["--scenario", args[first_scenario]])

    assert_rejected_without_artifact(
        args,
        kind="local_conformance",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--scenario must not contain duplicates",
    )


def test_cargo_command_rejects_unreviewed_values_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("local_conformance", tmp_path)
    args.extend(["--cargo-command", "echo sorafs_gateway_conformance"])

    assert_rejected_without_artifact(
        args,
        kind="local_conformance",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--cargo-command must be a reviewed gateway conformance command",
    )


def test_cargo_command_accepts_locked_reviewed_value(tmp_path: Path) -> None:
    args = args_for("local_conformance", tmp_path)
    args.extend(
        [
            "--cargo-command",
            CHECKER.LOCKED_GATEWAY_CONFORMANCE_CARGO_COMMAND,
        ]
    )

    assert MODULE.main(args) == 0

    payload = json.loads(canary_path(tmp_path, "local_conformance").read_text("utf-8"))
    assert payload["cargo_command"] == CHECKER.LOCKED_GATEWAY_CONFORMANCE_CARGO_COMMAND
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "local_conformance"
    assert errors == []


def test_unknown_metric_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("telemetry_slo", tmp_path)
    args.extend(["--metric", "sorafs_gateway_debug_payload_bytes"])

    assert_rejected_without_artifact(
        args,
        kind="telemetry_slo",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--metric contains an unknown value",
    )


def test_telemetry_metrics_must_not_duplicate(tmp_path: Path, capsys) -> None:
    args = args_for("telemetry_slo", tmp_path)
    first_metric = args.index("--metric") + 1
    args.extend(["--metric", args[first_metric]])

    assert_rejected_without_artifact(
        args,
        kind="telemetry_slo",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--metric must not contain duplicates",
    )


@pytest.mark.parametrize(
    ("kind", "option", "duplicate_value", "unknown_value"),
    (
        (
            "local_conformance",
            "--scenario",
            MODULE.REQUIRED_SCENARIOS[0],
            "debug-live-http3",
        ),
        (
            "telemetry_slo",
            "--metric",
            MODULE.REQUIRED_METRICS[0],
            "sorafs_gateway_debug_payload_bytes",
        ),
    ),
)
def test_closed_set_inputs_reject_duplicate_and_unknown_values_before_write(
    kind: str,
    option: str,
    duplicate_value: str,
    unknown_value: str,
    tmp_path: Path,
    capsys,
) -> None:
    duplicate_args = args_for(kind, tmp_path)
    duplicate_args.extend([option, duplicate_value])
    assert_rejected_without_artifact(
        duplicate_args,
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=f"{option} must not contain duplicates",
    )

    unknown_dir = tmp_path / "unknown"
    unknown_dir.mkdir()
    unknown_args = args_for(kind, unknown_dir)
    unknown_args.extend([option, unknown_value])
    assert_rejected_without_artifact(
        unknown_args,
        kind=kind,
        tmp_path=unknown_dir,
        capsys=capsys,
        expected_error=f"{option} contains an unknown value",
    )


def test_gateway_version_rejects_placeholder_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("staging_load", tmp_path)
    version_index = args.index("--gateway-version")
    args[version_index + 1] = "latest"

    assert_rejected_without_artifact(
        args,
        kind="staging_load",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=CHECKER.GATEWAY_VERSION_ERROR.replace(
            "gateway_version", "--gateway-version"
        ),
    )


def test_gateway_version_accepts_reviewed_rc_label(tmp_path: Path) -> None:
    args = args_for("staging_load", tmp_path)
    version_index = args.index("--gateway-version")
    args[version_index + 1] = "iroha-gateway 1.0.0-rc.1"

    assert MODULE.main(args) == 0

    payload = json.loads(canary_path(tmp_path, "staging_load").read_text("utf-8"))
    assert payload["gateway_version"] == "iroha-gateway 1.0.0-rc.1"
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "staging_load"
    assert errors == []


def test_staging_thresholds_fail_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("staging_load", tmp_path)
    duration_index = args.index("--duration-seconds")
    args[duration_index + 1] = "600"
    p95_index = args.index("--p95-latency-ms")
    args[p95_index + 1] = "2000"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--duration-seconds must be >=" in captured.err
    assert "--p95-latency-ms must be <=" in captured.err
    assert not canary_path(tmp_path, "staging_load").exists()


def test_staging_policy_digest_is_required(tmp_path: Path, capsys) -> None:
    args = args_for("staging_load", tmp_path)
    index = args.index("--policy-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--policy-digest-hex must be exact lowercase 32-byte hex" in captured.err
    assert not canary_path(tmp_path, "staging_load").exists()


def test_staging_provider_inventory_is_required(tmp_path: Path, capsys) -> None:
    args = args_for("staging_load", tmp_path)
    while "--provider" in args:
        index = args.index("--provider")
        del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--provider is required for staging_load" in captured.err
    assert "--provider-count must match the number of unique --provider values" in captured.err
    assert not canary_path(tmp_path, "staging_load").exists()


def test_staging_provider_inventory_must_match_count(tmp_path: Path, capsys) -> None:
    args = args_for("staging_load", tmp_path)
    provider_count_index = args.index("--provider-count")
    args[provider_count_index + 1] = "5"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--provider-count must match the number of unique --provider values" in captured.err
    assert not canary_path(tmp_path, "staging_load").exists()


def test_staging_provider_inventory_must_not_duplicate(tmp_path: Path, capsys) -> None:
    args = args_for("staging_load", tmp_path)
    args.extend(["--provider", "gateway-load-provider-a"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--provider must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "staging_load").exists()


def test_staging_hardware_profile_rejects_placeholder_before_write(
    tmp_path: Path,
    capsys,
    ) -> None:
    args = args_for("staging_load", tmp_path)
    index = args.index("--hardware-profile")
    args[index + 1] = "gateway-load-hardware-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--hardware-profile must not contain non-production markers "
        "['placeholder']"
    ) in captured.err
    assert not canary_path(tmp_path, "staging_load").exists()


def test_staging_cache_state_rejects_unknown_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("staging_load", tmp_path)
    index = args.index("--cache-state")
    args[index + 1] = "debug-cache"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--cache-state must be a reviewed cache-state value" in captured.err
    assert not canary_path(tmp_path, "staging_load").exists()


def test_staging_hardware_profile_requires_gateway_load_family_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("staging_load", tmp_path)
    index = args.index("--hardware-profile")
    args[index + 1] = "staging-c6i-2xlarge"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--hardware-profile must match reviewed `gateway-load-hardware-*` label"
        in captured.err
    )
    assert not canary_path(tmp_path, "staging_load").exists()


def test_staging_provider_rejects_placeholder_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("staging_load", tmp_path)
    first_provider = args.index("--provider") + 1
    args[first_provider] = "gateway-load-provider-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--provider must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "staging_load").exists()


def test_staging_provider_requires_production_family_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("staging_load", tmp_path)
    first_provider = args.index("--provider") + 1
    args[first_provider] = "provider-a"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        CHECKER.PROVIDER_NAME_ERROR.replace("providers[].name", "--provider")
        in captured.err
    )
    assert not canary_path(tmp_path, "staging_load").exists()


def test_output_symlink_is_refused(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    link = tmp_path / "link.json"
    link.symlink_to(target)
    args = args_for("local_conformance", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(link)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a symlink" in captured.err
    assert not target.exists()


def test_output_directory_is_rejected(tmp_path: Path, capsys) -> None:
    output_dir = tmp_path / "local-conformance-output"
    output_dir.mkdir()
    args = args_for("local_conformance", tmp_path)
    args[args.index("--out") + 1] = str(output_dir)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a directory" in captured.err
    assert output_dir.is_dir()
