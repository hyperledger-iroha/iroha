"""Tests for scripts/build_sorafs_gateway_load_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


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
                "staging-c6i-2xlarge",
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


def test_builds_payload_free_staging_load_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("staging_load", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "staging_load").read_text("utf-8"))

    assert payload["schema"] == "sorafs.gateway_load.staging_load.v1"
    assert payload["suite_report_digest_hex"] == SUITE_DIGEST
    assert payload["staging_report_digest_hex"] == STAGING_DIGEST
    assert payload["policy_digest_hex"] == POLICY_DIGEST
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


def test_missing_metric_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("telemetry_slo", tmp_path)
    index = args.index("--metric")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--metric must include every required value" in captured.err
    assert not canary_path(tmp_path, "telemetry_slo").exists()


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
