"""Tests for scripts/build_sorafs_repair_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_repair_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_repair_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location("build_sorafs_repair_canary", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_repair_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


NOW_UNIX = 1_800_400_000
GENERATED_AT = NOW_UNIX - 120
ROSTER_DIGEST = "a" * 64
FAILURE_DIGEST = "b" * 64
HANDOFF_DIGEST = "c" * 64
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
        "repair-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(NOW_UNIX),
    ]
    if kind in MODULE.ROSTER_DIGEST_KINDS:
        args.extend(["--roster-digest-hex", ROSTER_DIGEST])
    if kind in MODULE.FAILURE_BUNDLE_DIGEST_KINDS:
        args.extend(["--evidence-bundle-digest-hex", FAILURE_DIGEST])
    if kind in MODULE.POLICY_DIGEST_KINDS:
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    if kind == "auditor_roster":
        args.extend(["--auditor-count", str(CHECKER.DEFAULT_MIN_AUDITORS)])
        for index in range(CHECKER.DEFAULT_MIN_AUDITORS):
            args.extend(["--auditor", f"auditor-{index:02d}"])
    elif kind == "failure_capture":
        for source in MODULE.REQUIRED_FAILURE_SOURCES:
            args.extend(["--failure-source", source])
    elif kind == "auditor_api":
        for route in MODULE.REQUIRED_AUDITOR_ROUTES:
            args.extend(["--auditor-route", route])
    elif kind == "worker_lifecycle":
        for route in MODULE.REQUIRED_WORKER_ROUTES:
            args.extend(["--worker-route", route])
        for status in MODULE.REQUIRED_LIFECYCLE_STATUSES:
            args.extend(["--lifecycle-status", status])
    elif kind == "event_streams":
        for route in MODULE.REQUIRED_EVENT_ROUTES:
            args.extend(["--event-route", route])
    elif kind == "governance_handoff":
        args.extend(["--handoff-digest-hex", HANDOFF_DIGEST])
        for target in MODULE.REQUIRED_GOVERNANCE_TARGETS:
            args.extend(["--handoff-target", target])
    elif kind == "observability":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    elif kind == "governance_approval":
        args.extend(
            [
                "--handoff-digest-hex",
                HANDOFF_DIGEST,
            ]
        )
    return args


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=NOW_UNIX,
        max_evidence_age_secs=CHECKER.DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=CHECKER.DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_event_lag_secs=CHECKER.DEFAULT_MAX_EVENT_LAG_SECS,
        max_repair_latency_secs=CHECKER.DEFAULT_MAX_REPAIR_LATENCY_SECS,
        min_auditors=CHECKER.DEFAULT_MIN_AUDITORS,
    )


def test_builds_payload_free_worker_lifecycle_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("worker_lifecycle", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "worker_lifecycle").read_text("utf-8"))

    assert payload["schema"] == "sorafs.repair.worker_lifecycle_canary.v1"
    assert payload["route_count"] == len(MODULE.REQUIRED_WORKER_ROUTES)
    assert payload["passed_route_count"] == len(MODULE.REQUIRED_WORKER_ROUTES)
    assert payload["statuses_observed"] == list(MODULE.REQUIRED_LIFECYCLE_STATUSES)
    assert payload["raw_repair_payloads_included"] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "worker_lifecycle"
    assert errors == []


def test_generated_canaries_pass_full_repair_gate(tmp_path: Path) -> None:
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
    assert payload["valid_roster_digests"] == [ROSTER_DIGEST]
    assert payload["valid_failure_bundle_digests"] == [FAILURE_DIGEST]
    assert payload["valid_handoff_digests"] == [HANDOFF_DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_response_file_can_build_auditor_roster_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "auditor-roster.args"
    args_file.write_text(
        "\n".join(args_for("auditor_roster", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "auditor_roster").read_text("utf-8"))
    assert payload["auditor_count"] == CHECKER.DEFAULT_MIN_AUDITORS
    assert [auditor["name"] for auditor in payload["auditors"]] == [
        f"auditor-{index:02d}" for index in range(CHECKER.DEFAULT_MIN_AUDITORS)
    ]
    assert payload["raw_roster_included"] is False


def test_auditor_roster_inventory_must_match_auditor_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("auditor_roster", tmp_path)
    auditor_count_index = args.index("--auditor-count")
    args[auditor_count_index + 1] = str(CHECKER.DEFAULT_MIN_AUDITORS + 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--auditor unique values must match --auditor-count" in captured.err
    assert not canary_path(tmp_path, "auditor_roster").exists()


def test_auditor_roster_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("auditor_roster", tmp_path)
    first_auditor = args.index("--auditor") + 1
    args.extend(["--auditor", args[first_auditor]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--auditor must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "auditor_roster").exists()


def test_missing_worker_route_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("worker_lifecycle", tmp_path)
    index = args.index("--worker-route")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--worker-route must include every required value" in captured.err
    assert not canary_path(tmp_path, "worker_lifecycle").exists()


def test_governance_approval_requires_handoff_digest(tmp_path: Path, capsys) -> None:
    args = args_for("governance_approval", tmp_path)
    index = args.index("--handoff-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--handoff-digest-hex is required for governance_approval" in captured.err
    assert not canary_path(tmp_path, "governance_approval").exists()


def test_policy_digest_required_for_policy_bound_canaries(
    tmp_path: Path,
    capsys,
) -> None:
    for kind in MODULE.POLICY_DIGEST_KINDS:
        args = args_for(kind, tmp_path)
        index = args.index("--policy-digest-hex")
        del args[index : index + 2]

        assert MODULE.main(args) == 2

        captured = capsys.readouterr()
        assert f"--policy-digest-hex is required for {kind}" in captured.err
        assert not canary_path(tmp_path, kind).exists()


def test_event_and_repair_latency_thresholds_fail_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("event_streams", tmp_path)
    args.extend(["--event-lag-seconds", "901", "--repair-latency-seconds", "7201"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--event-lag-seconds must be <=" in captured.err
    assert "--repair-latency-seconds must be <=" in captured.err
    assert not canary_path(tmp_path, "event_streams").exists()


def test_output_symlink_is_refused(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    link = tmp_path / "link.json"
    link.symlink_to(target)
    args = args_for("auditor_roster", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(link)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a symlink" in captured.err
    assert not target.exists()
