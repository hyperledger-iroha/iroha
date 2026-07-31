"""Tests for scripts/build_sorafs_repair_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


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

from sorafs_rollout_runner_test_support import write_topology_qualification  # noqa: E402


NOW_UNIX = 1_800_400_000
GENERATED_AT = NOW_UNIX - 120
ROSTER_DIGEST = "a" * 64
FAILURE_DIGEST = "b" * 64
HANDOFF_DIGEST = "c" * 64
POLICY_DIGEST = "d" * 64
ROUTE_BODY_DIGEST = "e" * 64


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
            args.extend(["--auditor", f"repair-auditor-{index:02d}"])
    elif kind == "failure_capture":
        args.extend(["--failure-event-count", str(len(MODULE.REQUIRED_FAILURE_SOURCES))])
        for source in MODULE.REQUIRED_FAILURE_SOURCES:
            args.extend(["--failure-source", source])
        for source in MODULE.REQUIRED_FAILURE_SOURCES:
            args.extend(
                [
                    "--failure-event",
                    f"{source}:repair-failure-event-{source}-00",
                ]
            )
    elif kind == "auditor_api":
        args.extend(["--route-body-blake3-hex", ROUTE_BODY_DIGEST])
        for route in MODULE.REQUIRED_AUDITOR_ROUTES:
            args.extend(["--auditor-route", route])
    elif kind == "worker_lifecycle":
        args.extend(["--route-body-blake3-hex", ROUTE_BODY_DIGEST])
        for route in MODULE.REQUIRED_WORKER_ROUTES:
            args.extend(["--worker-route", route])
        for status in MODULE.REQUIRED_LIFECYCLE_STATUSES:
            args.extend(["--lifecycle-status", status])
    elif kind == "event_streams":
        args.extend(["--route-body-blake3-hex", ROUTE_BODY_DIGEST])
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


def test_builds_payload_free_worker_lifecycle_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("worker_lifecycle", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "worker_lifecycle").read_text("utf-8"))

    assert payload["schema"] == "sorafs.repair.worker_lifecycle_canary.v1"
    assert payload["route_count"] == len(MODULE.REQUIRED_WORKER_ROUTES)
    assert payload["passed_route_count"] == len(MODULE.REQUIRED_WORKER_ROUTES)
    assert all(
        route["body_blake3_hex"] == ROUTE_BODY_DIGEST for route in payload["routes"]
    )
    assert payload["status_count"] == len(MODULE.REQUIRED_LIFECYCLE_STATUSES)
    assert payload["statuses_observed"] == list(MODULE.REQUIRED_LIFECYCLE_STATUSES)
    assert payload["finalized_task_projection_verified"] is True
    assert payload["exact_live_lease_execution_verified"] is True
    assert payload["durable_transaction_forwarding_verified"] is True
    assert payload["restart_reconciliation_verified"] is True
    assert payload["single_terminal_outcome_verified"] is True
    assert payload["raw_repair_payloads_included"] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "worker_lifecycle"
    assert errors == []


def test_route_canaries_emit_exact_command_and_query_status_codes(
    tmp_path: Path,
) -> None:
    observed: dict[str, int] = {}
    for kind in ("auditor_api", "worker_lifecycle", "event_streams"):
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        payload = json.loads(canary_path(tmp_path, kind).read_text("utf-8"))
        observed.update(
            {record["name"]: record["status_code"] for record in payload["routes"]}
        )

    assert observed == CHECKER.REQUIRED_ROUTE_STATUS_CODES


def test_retired_global_route_status_override_is_rejected(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("worker_lifecycle", tmp_path)
    args.extend(["--route-status-code", "200"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "unrecognized arguments: --route-status-code 200" in captured.err
    assert not canary_path(tmp_path, "worker_lifecycle").exists()


def test_builds_payload_free_governance_handoff_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("governance_handoff", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "governance_handoff").read_text("utf-8")
    )

    assert payload["schema"] == "sorafs.repair.governance_handoff_canary.v1"
    assert payload["handoff_target_count"] == len(MODULE.REQUIRED_GOVERNANCE_TARGETS)
    assert payload["handoff_targets"] == list(MODULE.REQUIRED_GOVERNANCE_TARGETS)
    assert payload["raw_ledger_included"] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "governance_handoff"
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
    command.extend(
        [
            "--topology-qualification-summary",
            str(
                write_topology_qualification(
                    tmp_path / "topology-qualification.json",
                    deployment_id="repair-prod-20260701",
                )
            ),
        ]
    )

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
        f"repair-auditor-{index:02d}"
        for index in range(CHECKER.DEFAULT_MIN_AUDITORS)
    ]
    assert payload["raw_roster_included"] is False


def test_builds_payload_free_failure_capture_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("failure_capture", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "failure_capture").read_text("utf-8"))

    assert payload["schema"] == "sorafs.repair.failure_capture_canary.v1"
    assert payload["failure_sources"] == list(MODULE.REQUIRED_FAILURE_SOURCES)
    assert payload["failure_source_count"] == len(MODULE.REQUIRED_FAILURE_SOURCES)
    assert payload["failure_event_count"] == len(MODULE.REQUIRED_FAILURE_SOURCES)
    assert payload["failure_events"] == [
        {"name": f"repair-failure-event-{source}-00", "source": source}
        for source in MODULE.REQUIRED_FAILURE_SOURCES
    ]
    assert payload["raw_evidence_included"] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "failure_capture"
    assert errors == []


def test_default_failure_event_count_matches_required_sources(tmp_path: Path) -> None:
    args = args_for("failure_capture", tmp_path)
    count_index = args.index("--failure-event-count")
    del args[count_index : count_index + 2]
    parsed = MODULE.parse_args(args)

    assert parsed.failure_event_count == len(MODULE.REQUIRED_FAILURE_SOURCES)
    assert MODULE.main(args) == 0

    payload = json.loads(canary_path(tmp_path, "failure_capture").read_text("utf-8"))
    assert payload["failure_event_count"] == len(MODULE.REQUIRED_FAILURE_SOURCES)
    assert {event["source"] for event in payload["failure_events"]} == set(
        MODULE.REQUIRED_FAILURE_SOURCES
    )


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


def test_auditor_roster_inventory_must_use_production_family(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("auditor_roster", tmp_path)
    first_auditor = args.index("--auditor") + 1
    args[first_auditor] = "auditor-00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--auditor must match canonical lowercase `repair-auditor-*`"
        in captured.err
    )
    assert not canary_path(tmp_path, "auditor_roster").exists()


def test_auditor_roster_inventory_rejects_placeholder_marker(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("auditor_roster", tmp_path)
    first_auditor = args.index("--auditor") + 1
    args[first_auditor] = "repair-auditor-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--auditor must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "auditor_roster").exists()


@pytest.mark.parametrize(
    "auditor_value",
    (
        "repair-auditor-00, repair-auditor-01",
        "repair-auditor-00,,repair-auditor-01",
    ),
)
def test_auditor_csv_rejects_padded_or_empty_components(
    auditor_value: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("auditor_roster", tmp_path)
    first_auditor = args.index("--auditor") + 1
    args[first_auditor] = auditor_value

    assert_rejected_without_artifact(
        args,
        kind="auditor_roster",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--auditor[1] must be a non-empty canonical string",
    )


def test_failure_event_inventory_must_match_failure_event_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("failure_capture", tmp_path)
    args.extend(["--failure-event-count", "3"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--failure-event unique names must match --failure-event-count" in captured.err
    assert not canary_path(tmp_path, "failure_capture").exists()


def test_failure_event_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("failure_capture", tmp_path)
    first_event = args.index("--failure-event") + 1
    args.extend(["--failure-event", args[first_event]])
    args.extend(["--failure-event-count", "3"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--failure-event names must not contain duplicates" in captured.err
    assert "--failure-event unique names must match --failure-event-count" in captured.err
    assert not canary_path(tmp_path, "failure_capture").exists()


def test_failure_event_inventory_must_cover_required_sources(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("failure_capture", tmp_path)
    first_event = args.index("--failure-event")
    del args[first_event : first_event + 2]
    count_index = args.index("--failure-event-count")
    args[count_index + 1] = "1"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--failure-event must include every required failure source" in captured.err
    assert not canary_path(tmp_path, "failure_capture").exists()


def test_failure_event_inventory_rejects_unknown_source(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("failure_capture", tmp_path)
    first_event = args.index("--failure-event") + 1
    args[first_event] = "unknown:repair-failure-event-por-00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--failure-event source must be a reviewed failure source" in captured.err
    assert not canary_path(tmp_path, "failure_capture").exists()


@pytest.mark.parametrize(
    ("event_value", "diagnostic"),
    (
        (
            " por:repair-failure-event-por-00",
            "argument must be a non-empty canonical string",
        ),
        (
            "por :repair-failure-event-por-00",
            "--failure-event[0].source must be a non-empty canonical string",
        ),
        (
            "por: repair-failure-event-por-00",
            "--failure-event[0].name must be a non-empty canonical string",
        ),
        (
            "por:repair-failure-event-por-00 ",
            "argument must be a non-empty canonical string",
        ),
    ),
)
def test_failure_event_inventory_rejects_padded_tuple_components(
    event_value: str,
    diagnostic: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("failure_capture", tmp_path)
    first_event = args.index("--failure-event") + 1
    args[first_event] = event_value

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert diagnostic in captured.err
    assert not canary_path(tmp_path, "failure_capture").exists()


def test_failure_event_inventory_must_use_production_family(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("failure_capture", tmp_path)
    first_event = args.index("--failure-event") + 1
    args[first_event] = "por:por-failure-00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--failure-event must match canonical lowercase "
        "`repair-failure-event-name`"
    ) in captured.err
    assert not canary_path(tmp_path, "failure_capture").exists()


def test_failure_event_inventory_rejects_placeholder_marker(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("failure_capture", tmp_path)
    first_event = args.index("--failure-event") + 1
    args[first_event] = "por:repair-failure-event-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--failure-event must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "failure_capture").exists()


@pytest.mark.parametrize(
    ("kind", "option", "duplicate_value", "unknown_value"),
    (
        (
            "failure_capture",
            "--failure-source",
            MODULE.REQUIRED_FAILURE_SOURCES[0],
            "unreviewed-failure-source",
        ),
        (
            "auditor_api",
            "--auditor-route",
            MODULE.REQUIRED_AUDITOR_ROUTES[0],
            "unreviewed-auditor-route",
        ),
        (
            "worker_lifecycle",
            "--worker-route",
            MODULE.REQUIRED_WORKER_ROUTES[0],
            "unreviewed-worker-route",
        ),
        (
            "worker_lifecycle",
            "--lifecycle-status",
            MODULE.REQUIRED_LIFECYCLE_STATUSES[0],
            "unreviewed-lifecycle-status",
        ),
        (
            "event_streams",
            "--event-route",
            MODULE.REQUIRED_EVENT_ROUTES[0],
            "unreviewed-event-route",
        ),
        (
            "governance_handoff",
            "--handoff-target",
            MODULE.REQUIRED_GOVERNANCE_TARGETS[0],
            "unreviewed-handoff-target",
        ),
        (
            "observability",
            "--metric",
            MODULE.REQUIRED_METRICS[0],
            "unreviewed-repair-metric",
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

    unknown_args = args_for(kind, tmp_path)
    unknown_args.extend([option, unknown_value])
    assert_rejected_without_artifact(
        unknown_args,
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=f"{option} contains an unknown value",
    )


@pytest.mark.parametrize(
    "kind",
    ("auditor_api", "worker_lifecycle", "event_streams"),
)
def test_route_canaries_require_route_body_digest(
    kind: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for(kind, tmp_path)
    index = args.index("--route-body-blake3-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--route-body-blake3-hex must be exact lowercase 32-byte hex" in captured.err
    assert not canary_path(tmp_path, kind).exists()


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


def test_output_directory_is_rejected(tmp_path: Path, capsys) -> None:
    output_dir = tmp_path / "auditor-roster-output"
    output_dir.mkdir()
    args = args_for("auditor_roster", tmp_path)
    args[args.index("--out") + 1] = str(output_dir)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a directory" in captured.err
    assert output_dir.is_dir()
