"""Tests for scripts/build_sorafs_por_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_por_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_por_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location("build_sorafs_por_canary", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_por_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


NOW_UNIX = 1_800_500_000
GENERATED_AT = NOW_UNIX - 120
DIGEST = "a" * 64
POLICY_DIGEST = "b" * 64
VALIDATION_DIGEST = "c" * 64
REPORT_DIGEST = "d" * 64


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "por-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(NOW_UNIX),
    ]
    if kind in MODULE.SEED_REPLAY_DIGEST_KINDS:
        args.extend(["--seed-replay-digest-hex", DIGEST])
    if kind in MODULE.POLICY_DIGEST_KINDS:
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    if kind == "scheduler_runtime":
        for route in MODULE.REQUIRED_RUNTIME_ROUTES:
            args.extend(["--runtime-route", route])
    elif kind == "validator_replay":
        args.extend(["--validation-bundle-digest-hex", VALIDATION_DIGEST])
    elif kind == "reporting_archive":
        args.extend(["--report-digest-hex", REPORT_DIGEST])
        for route in MODULE.REQUIRED_REPORTING_ROUTES:
            args.extend(["--reporting-route", route])
    elif kind == "observability":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    return args


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=NOW_UNIX,
        max_evidence_age_secs=CHECKER.DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=CHECKER.DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_scheduler_lag_secs=CHECKER.DEFAULT_MAX_SCHEDULER_LAG_SECS,
        max_report_latency_ms=CHECKER.DEFAULT_MAX_REPORT_LATENCY_MS,
        min_providers=CHECKER.DEFAULT_MIN_PROVIDERS,
        min_challenges=CHECKER.DEFAULT_MIN_CHALLENGES,
    )


def test_builds_payload_free_scheduler_runtime_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("scheduler_runtime", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "scheduler_runtime").read_text("utf-8"))

    assert payload["schema"] == "sorafs.por.scheduler_runtime_canary.v1"
    assert payload["route_count"] == len(MODULE.REQUIRED_RUNTIME_ROUTES)
    assert payload["passed_route_count"] == len(MODULE.REQUIRED_RUNTIME_ROUTES)
    assert payload["response_bodies_included"] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "scheduler_runtime"
    assert errors == []


def test_generated_canaries_pass_full_por_gate(tmp_path: Path) -> None:
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
    assert payload["valid_seed_replay_digests"] == [DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_response_file_can_build_reporting_archive_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "reporting.args"
    args_file.write_text(
        "\n".join(args_for("reporting_archive", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "reporting_archive").read_text("utf-8"))
    assert payload["manual_trigger_route_state"] == "retired"
    assert payload["archive_backend"] == "parquet"
    assert payload["routes"][0]["name"] == MODULE.REQUIRED_REPORTING_ROUTES[0]


def test_missing_runtime_route_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("scheduler_runtime", tmp_path)
    index = args.index("--runtime-route")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--runtime-route must include every required value" in captured.err
    assert not canary_path(tmp_path, "scheduler_runtime").exists()


def test_scheduler_and_report_thresholds_fail_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("reporting_archive", tmp_path)
    args.extend(["--scheduler-lag-seconds", "901", "--report-latency-ms", "3001"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--scheduler-lag-seconds must be <=" in captured.err
    assert "--report-latency-ms must be <=" in captured.err
    assert not canary_path(tmp_path, "reporting_archive").exists()


def test_randomness_requires_policy_digest_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("randomness", tmp_path)
    index = args.index("--policy-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--policy-digest-hex is required for randomness" in captured.err
    assert not canary_path(tmp_path, "randomness").exists()


def test_output_symlink_is_refused(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    link = tmp_path / "link.json"
    link.symlink_to(target)
    args = args_for("randomness", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(link)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a symlink" in captured.err
    assert not target.exists()
