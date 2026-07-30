"""Tests for scripts/run_sorafs_moderation_panel_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_moderation_panel_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_moderation_panel_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

from sorafs_rollout_runner_test_support import (  # noqa: E402
    write_topology_qualification,
)


def write_payload(path: Path) -> Path:
    path.write_text("{}", encoding="utf-8")
    return path


def topology_args(tmp_path: Path) -> list[str]:
    return [
        "--topology-qualification-summary",
        str(
            write_topology_qualification(
                tmp_path / "topology-qualification.json",
                deployment_id="moderation-panel-production-a",
            )
        ),
    ]


def complete_args(tmp_path: Path) -> list[str]:
    payload_dir = tmp_path / "payloads"
    payload_dir.mkdir()
    return [
        "--out-dir",
        str(tmp_path / "evidence"),
        "--now-unix",
        "1800009000",
        *topology_args(tmp_path),
        "--max-canary-age-secs",
        "86400",
        "--max-event-lag-secs",
        "900",
        "--max-route-latency-ms",
        "1500",
        "--min-panel-size",
        "7",
        "--min-peers",
        "4",
        "--appeal-intake-evidence",
        str(write_payload(payload_dir / "appeal-intake.json")),
        "--sortition-roster-evidence",
        str(write_payload(payload_dir / "sortition-roster.json")),
        "--evidence-viewer-evidence",
        str(write_payload(payload_dir / "evidence-viewer.json")),
        "--operator-workflow-evidence",
        str(write_payload(payload_dir / "operator-workflow.json")),
        "--juror-notifications-evidence",
        str(write_payload(payload_dir / "juror-notifications.json")),
        "--commit-reveal-evidence",
        str(write_payload(payload_dir / "commit-reveal.json")),
        "--decision-publication-evidence",
        str(write_payload(payload_dir / "decision-publication.json")),
        "--settlement-integration-evidence",
        str(write_payload(payload_dir / "settlement-integration.json")),
        "--transparency-reputation-evidence",
        str(write_payload(payload_dir / "transparency-reputation.json")),
        "--e2e-panel-evidence",
        str(write_payload(payload_dir / "e2e-panel.json")),
        "--metrics-alerts-evidence",
        str(write_payload(payload_dir / "metrics-alerts.json")),
        "--governance-approval-evidence",
        str(write_payload(payload_dir / "governance-approval.json")),
    ]


def write_args_file(path: Path, args: list[str]) -> Path:
    lines = [
        "# comments and blank lines are ignored",
        "",
    ]
    for index in range(0, len(args), 2):
        option = args[index]
        value = args[index + 1]
        lines.append(f"{option} {json.dumps(value)}")
    path.write_text("\n".join(lines), encoding="utf-8")
    return path


def write_split_args_file(path: Path, args: list[str]) -> Path:
    lines = [
        "# one token per line also works for long reviewed inputs",
        *args,
    ]
    path.write_text("\n".join(lines), encoding="utf-8")
    return path


def test_dry_run_prints_complete_moderation_panel_rollout_plan(
    tmp_path: Path, capsys
) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.moderation_panel.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.moderation_panel.rollout_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_canary_age_secs": 86400,
        "max_event_lag_secs": 900,
        "max_route_latency_ms": 1500,
        "min_panel_size": 7,
        "min_peers": 4,
        "now_unix": 1800009000,
    }
    assert plan["external_evidence"]["e2e_panel"] == [
        str(tmp_path / "payloads" / "e2e-panel.json")
    ]
    assert plan["evidence_contract"]["appeal_intake"]["schema"] == (
        "sorafs.moderation_panel.appeal_intake_canary.v1"
    )
    assert (
        "case_digest_hex"
        in plan["evidence_contract"]["appeal_intake"]["required_payload_fields"]
    )
    assert (
        "viewer_security_controls"
        in plan["evidence_contract"]["evidence_viewer"]["required_payload_fields"]
    )
    assert "cases" in plan["evidence_contract"]["appeal_intake"]["required_payload_fields"]
    assert "jurors" in plan["evidence_contract"]["sortition_roster"]["required_payload_fields"]
    assert (
        "sessions"
        in plan["evidence_contract"]["evidence_viewer"]["required_payload_fields"]
    )
    assert (
        "notifications"
        in plan["evidence_contract"]["juror_notifications"][
            "required_payload_fields"
        ]
    )
    assert (
        "jurors"
        in plan["evidence_contract"]["juror_notifications"][
            "required_payload_fields"
        ]
    )
    assert (
        "scenarios_exercised"
        in plan["evidence_contract"]["commit_reveal"]["required_payload_fields"]
    )
    assert "commits" in plan["evidence_contract"]["commit_reveal"]["required_payload_fields"]
    assert "reveals" in plan["evidence_contract"]["commit_reveal"]["required_payload_fields"]
    assert (
        "settlements"
        in plan["evidence_contract"]["settlement_integration"][
            "required_payload_fields"
        ]
    )
    assert (
        "policy_digest_hex"
        in plan["evidence_contract"]["e2e_panel"]["required_payload_fields"]
    )
    assert "peers" in plan["evidence_contract"]["e2e_panel"]["required_payload_fields"]
    assert (
        "validators"
        in plan["evidence_contract"]["e2e_panel"]["required_payload_fields"]
    )
    assert "cases" in plan["evidence_contract"]["e2e_panel"]["required_payload_fields"]
    assert "metrics" in plan["evidence_contract"]["metrics_alerts"]["required_payload_fields"]
    assert (
        "iroha_config_bound"
        in plan["evidence_contract"]["governance_approval"][
            "required_payload_fields"
        ]
    )
    assert (
        "policy_digest_hex"
        in plan["evidence_contract"]["governance_approval"][
            "required_payload_fields"
        ]
    )
    assert [step["label"] for step in plan["steps"]] == ["rollout_evidence_gate"]
    verifier = plan["steps"][0]["command"]
    assert "check_sorafs_moderation_panel_rollout_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "commit-reveal.json") in verifier
    assert verifier.count("--commit-reveal-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--min-panel-size" in verifier
    assert "--now-unix" in verifier


def test_plan_json_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    assert MODULE.validate_plan_json(rendered, plan, args) == []

    assert MODULE.validate_plan_json(["step"], plan, args) == [
        "moderation panel rollout runner plan must be an object"
    ]

    rendered["schema"] = "sorafs.moderation_panel.rollout_evidence_collection_plan.v0"
    rendered["unexpected"] = True
    rendered["required_kinds"] = []
    rendered["thresholds"] = {}
    rendered["external_evidence"] = {}
    rendered["evidence_contract"] = {}
    rendered["steps"] = []

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "moderation panel rollout runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan schema must match the contract"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan required_kinds must match args"
        in diagnostics
    )
    assert "moderation panel rollout runner plan thresholds must match args" in diagnostics
    assert (
        "moderation panel rollout runner plan external_evidence must match args"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan evidence_contract must match checker fields"
        in diagnostics
    )
    assert "runner plan steps must match command plan" in diagnostics
    assert "commit-reveal.json" not in diagnostics


def test_plan_json_nested_shapes_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["bad\nfield"] = "runtime-only-key-material"
    rendered["schema"] = "sorafs\nmoderation"
    rendered["verifier_summary_schema"] = "summary\nschema"
    rendered["required_kinds"] = [
        "appeal_intake",
        "appeal_intake",
        "unknown_kind",
        "bad\nkind",
    ]
    rendered["thresholds"] = {
        "max_canary_age_secs": -1,
        "max_event_lag_secs": 0,
        "max_route_latency_ms": "fast",
        "min_panel_size": 0,
        "min_peers": False,
        "now_unix": 0,
        "bad\nfield": 1,
        "private_key": 2,
    }
    rendered["external_evidence"] = {
        "appeal_intake": [],
        "unknown_kind": ["unknown.json"],
        "sortition_roster": "sortition-roster.json",
        "bad\nkind": ["appeal-intake.json"],
        "metrics_alerts": ["bad\npath"],
    }
    rendered["evidence_contract"] = {
        "appeal_intake": {
            "schema": "wrong.schema.v1",
            "required_payload_fields": ["schema", "schema", "bad\nfield"],
            "raw_payload": True,
            "bad\nfield": "runtime-only-key-material",
        },
        "unknown_kind": {
            "schema": "sorafs.moderation_panel.unknown.v1",
            "required_payload_fields": [],
        },
        "sortition_roster": "contract-shaped-entry",
        "bad\nkind": {
            "schema": MODULE.KIND_BY_NAME["appeal_intake"].schema,
            "required_payload_fields": ["schema"],
        },
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert "moderation panel rollout runner plan fields must be canonical strings" in diagnostics
    assert "moderation panel rollout runner plan schema must be canonical" in diagnostics
    assert (
        "moderation panel rollout runner plan verifier schema must be canonical"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan required_kinds must contain canonical strings"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan required_kinds must not contain duplicate kinds"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan required_kinds must use known kind names"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan thresholds keys must be canonical strings"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan thresholds must contain only configured threshold fields"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan thresholds.max_canary_age_secs must be a non-negative integer"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan thresholds.max_event_lag_secs must be a positive integer"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan thresholds.max_route_latency_ms must be a positive integer"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan thresholds.min_panel_size must be a positive integer"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan thresholds.min_peers must be a positive integer"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan thresholds.now_unix must be a positive integer"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan external_evidence keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan external_evidence keys must use known kind names"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan external_evidence must map each kind to non-empty path lists"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan external_evidence paths must be canonical strings"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan evidence_contract keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan evidence_contract keys must use known kind names"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan evidence_contract must map each kind to a contract object"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan evidence_contract fields must be canonical strings"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan evidence_contract fields must be schema and required_payload_fields"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan evidence_contract schemas must match evidence kind"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan evidence_contract required_payload_fields must be non-empty lists"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan evidence_contract required_payload_fields must contain canonical strings"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan evidence_contract required_payload_fields must not contain duplicate fields"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan evidence_contract required_payload_fields must match checker fields"
        in diagnostics
    )
    assert "unknown_kind" not in diagnostics
    assert "bad\nkind" not in diagnostics
    assert "bad\nfield" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "private_key" not in diagnostics
    assert "wrong.schema.v1" not in diagnostics


def test_plan_json_rejects_unrequired_external_evidence_and_contracts(
    tmp_path: Path,
) -> None:
    payload = write_payload(tmp_path / "appeal-intake.json")
    args = MODULE.parse_args(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--now-unix",
            "1800009000",
            *topology_args(tmp_path),
            "--require-kind",
            "appeal_intake",
            "--appeal-intake-evidence",
            str(payload),
        ]
    )
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["external_evidence"]["sortition_roster"] = [
        str(tmp_path / "sortition-roster.json")
    ]
    rendered["evidence_contract"]["sortition_roster"] = {
        "schema": MODULE.KIND_BY_NAME["sortition_roster"].schema,
        "required_payload_fields": list(
            MODULE.EVIDENCE_REQUIRED_FIELDS["sortition_roster"]
        ),
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "moderation panel rollout runner plan external_evidence must contain only required kinds"
        in diagnostics
    )
    assert (
        "moderation panel rollout runner plan evidence_contract must contain only required kinds"
        in diagnostics
    )
    assert "sortition_roster" not in diagnostics


def test_execution_rejects_plan_validation_drift_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_validate_plan_json(rendered, plan, args):
        return ["moderation panel rollout runner plan schema must match the contract"]

    def fake_run_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "validate_plan_json", fake_validate_plan_json)
    monkeypatch.setattr(MODULE, "run_plan", fake_run_plan)

    assert MODULE.main(complete_args(tmp_path)) == 2

    assert not ran_plan
    assert (
        "moderation panel rollout runner plan schema must match the contract"
        in capsys.readouterr().err
    )


def test_response_file_dry_run_prints_complete_plan(tmp_path: Path, capsys) -> None:
    args_file = write_args_file(tmp_path / "moderation-panel-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert plan["external_evidence"]["operator_workflow"]
    assert "operator_workflow" in plan["evidence_contract"]


def test_split_response_file_dry_run_prints_complete_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(
        tmp_path / "split-moderation-panel-rollout.args",
        complete_args(tmp_path),
    )

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.moderation_panel.rollout_evidence_collection_plan.v1"
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert "decision_publication" in plan["evidence_contract"]


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--e2e-panel-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing required rollout evidence input" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-commit-reveal.json"
    evidence_index = args.index("--commit-reveal-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert "--commit-reveal-evidence" not in captured.err
    assert str(missing) not in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "appeal-intake.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--now-unix",
            "1800009000",
            *topology_args(tmp_path),
            "--require-kind",
            "appeal_intake",
            "--appeal-intake-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["appeal_intake"]
    assert list(plan["evidence_contract"]) == ["appeal_intake"]
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "appeal_intake" in verifier


def test_unknown_required_kind_fails_before_plan(tmp_path: Path, capsys) -> None:
    assert (
        MODULE.main(
            [
                "--out-dir",
                str(tmp_path / "evidence"),
                *topology_args(tmp_path),
                "--require-kind",
                "unknown",
                "--dry-run",
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "unknown required evidence kind" in captured.err
    assert captured.out == ""
