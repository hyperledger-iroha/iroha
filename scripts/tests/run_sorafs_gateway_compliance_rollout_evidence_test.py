"""Tests for scripts/run_sorafs_gateway_compliance_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_gateway_compliance_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_gateway_compliance_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def write_payload(path: Path) -> Path:
    path.write_text("{}", encoding="utf-8")
    return path


def complete_args(tmp_path: Path) -> list[str]:
    payload_dir = tmp_path / "payloads"
    payload_dir.mkdir()
    return [
        "--out-dir",
        str(tmp_path / "evidence"),
        "--now-unix",
        "1800009000",
        "--max-evidence-age-secs",
        "86400",
        "--max-route-latency-ms",
        "1500",
        "--max-reload-latency-ms",
        "300000",
        "--min-gateways",
        "3",
        "--min-denylist-entries",
        "5",
        "--min-honey-probes",
        "4",
        "--feed-promotion-evidence",
        str(write_payload(payload_dir / "feed-promotion.json")),
        "--controller-runtime-evidence",
        str(write_payload(payload_dir / "controller-runtime.json")),
        "--moderation-toggle-evidence",
        str(write_payload(payload_dir / "moderation-toggle.json")),
        "--gateway-reload-evidence",
        str(write_payload(payload_dir / "gateway-reload.json")),
        "--enforcement-probe-evidence",
        str(write_payload(payload_dir / "enforcement-probe.json")),
        "--honey-audit-evidence",
        str(write_payload(payload_dir / "honey-audit.json")),
        "--appeal-override-evidence",
        str(write_payload(payload_dir / "appeal-override.json")),
        "--transparency-publication-evidence",
        str(write_payload(payload_dir / "transparency-publication.json")),
        "--observability-evidence",
        str(write_payload(payload_dir / "observability.json")),
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
    path.write_text("\n".join(["# split response file", *args]), encoding="utf-8")
    return path


def test_dry_run_prints_complete_gateway_compliance_rollout_plan(
    tmp_path: Path, capsys
) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.gateway_compliance.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.gateway_compliance.rollout_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_evidence_age_secs": 86400,
        "max_reload_latency_ms": 300000,
        "max_route_latency_ms": 1500,
        "min_denylist_entries": 5,
        "min_gateways": 3,
        "min_honey_probes": 4,
        "now_unix": 1800009000,
    }
    assert plan["external_evidence"]["enforcement_probe"] == [
        str(tmp_path / "payloads" / "enforcement-probe.json")
    ]
    assert plan["evidence_contract"]["feed_promotion"]["schema"] == (
        "sorafs.gateway_compliance.feed_promotion_canary.v1"
    )
    assert (
        "policy_digest_hex"
        in plan["evidence_contract"]["feed_promotion"]["required_payload_fields"]
    )
    assert (
        "denylist_entries"
        in plan["evidence_contract"]["feed_promotion"]["required_payload_fields"]
    )
    assert plan["evidence_contract"]["controller_runtime"]["schema"] == (
        "sorafs.gateway_compliance.controller_runtime_canary.v1"
    )
    assert (
        "bundle_digest_hex"
        in plan["evidence_contract"]["controller_runtime"]["required_payload_fields"]
    )
    assert (
        "iroha_config_bound"
        in plan["evidence_contract"]["controller_runtime"]["required_payload_fields"]
    )
    assert plan["evidence_contract"]["moderation_toggle"]["schema"] == (
        "sorafs.gateway_compliance.moderation_toggle_canary.v1"
    )
    assert (
        "approval_workflow_verified"
        in plan["evidence_contract"]["moderation_toggle"]["required_payload_fields"]
    )
    assert (
        "expiry_enforced"
        in plan["evidence_contract"]["moderation_toggle"]["required_payload_fields"]
    )
    assert (
        "bundle_digest_hex"
        in plan["evidence_contract"]["gateway_reload"]["required_payload_fields"]
    )
    assert (
        "denial_reasons_observed"
        in plan["evidence_contract"]["enforcement_probe"]["required_payload_fields"]
    )
    assert "probes" in plan["evidence_contract"]["honey_audit"]["required_payload_fields"]
    assert "metrics" in plan["evidence_contract"]["observability"]["required_payload_fields"]
    assert (
        "iroha_config_bound"
        in plan["evidence_contract"]["governance_approval"][
            "required_payload_fields"
        ]
    )
    assert [step["label"] for step in plan["steps"]] == ["rollout_evidence_gate"]
    verifier = plan["steps"][0]["command"]
    assert "check_sorafs_gateway_compliance_rollout_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "gateway-reload.json") in verifier
    assert verifier.count("--gateway-reload-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--min-denylist-entries" in verifier
    assert "--now-unix" in verifier


def test_plan_json_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    assert MODULE.validate_plan_json(rendered, plan, args) == []

    assert MODULE.validate_plan_json(["step"], plan, args) == [
        "gateway compliance rollout runner plan must be an object"
    ]

    rendered["schema"] = "sorafs.gateway_compliance.rollout_evidence_collection_plan.v0"
    rendered["unexpected"] = True
    rendered["required_kinds"] = []
    rendered["thresholds"] = {}
    rendered["external_evidence"] = {}
    rendered["evidence_contract"] = {}
    rendered["steps"] = []

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "gateway compliance rollout runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan schema must match the contract"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan required_kinds must match args"
        in diagnostics
    )
    assert "gateway compliance rollout runner plan thresholds must match args" in diagnostics
    assert (
        "gateway compliance rollout runner plan external_evidence must match args"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan evidence_contract must match checker fields"
        in diagnostics
    )
    assert "runner plan steps must match command plan" in diagnostics
    assert "gateway-reload.json" not in diagnostics


def test_plan_json_nested_shapes_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["bad\nfield"] = "runtime-only-key-material"
    rendered["schema"] = "sorafs\ngateway-compliance"
    rendered["verifier_summary_schema"] = "summary\nschema"
    rendered["required_kinds"] = [
        "feed_promotion",
        "feed_promotion",
        "unknown_kind",
        "bad\nkind",
    ]
    rendered["thresholds"] = {
        "max_evidence_age_secs": -1,
        "max_route_latency_ms": 0,
        "max_reload_latency_ms": False,
        "min_gateways": "soon",
        "min_denylist_entries": 0,
        "min_honey_probes": 0,
        "now_unix": 0,
        "bad\nfield": 1,
        "private_key": 2,
    }
    rendered["external_evidence"] = {
        "feed_promotion": [],
        "unknown_kind": ["unknown.json"],
        "controller_runtime": "controller-runtime.json",
        "bad\nkind": ["feed-promotion.json"],
        "observability": ["bad\npath"],
    }
    rendered["evidence_contract"] = {
        "feed_promotion": {
            "schema": "wrong.schema.v1",
            "required_payload_fields": ["schema", "schema", "bad\nfield"],
            "raw_payload": True,
            "bad\nfield": "runtime-only-key-material",
        },
        "unknown_kind": {
            "schema": "sorafs.gateway_compliance.unknown.v1",
            "required_payload_fields": [],
        },
        "controller_runtime": "contract-shaped-entry",
        "bad\nkind": {
            "schema": MODULE.KIND_BY_NAME["feed_promotion"].schema,
            "required_payload_fields": ["schema"],
        },
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "gateway compliance rollout runner plan fields must be canonical strings"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan schema must be canonical"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan verifier schema must be canonical"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan required_kinds must contain canonical strings"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan required_kinds must not contain duplicate kinds"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan required_kinds must use known kind names"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan thresholds keys must be canonical strings"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan thresholds must contain only configured threshold fields"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan thresholds.max_evidence_age_secs must be a non-negative integer"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan thresholds.max_route_latency_ms must be a positive integer"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan thresholds.max_reload_latency_ms must be a positive integer"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan thresholds.min_gateways must be a positive integer"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan thresholds.min_denylist_entries must be a positive integer"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan thresholds.min_honey_probes must be a positive integer"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan thresholds.now_unix must be a positive integer"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan external_evidence keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan external_evidence keys must use known kind names"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan external_evidence must map each kind to non-empty path lists"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan external_evidence paths must be canonical strings"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan evidence_contract keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan evidence_contract keys must use known kind names"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan evidence_contract must map each kind to a contract object"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan evidence_contract fields must be canonical strings"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan evidence_contract fields must be schema and required_payload_fields"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan evidence_contract schemas must match evidence kind"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan evidence_contract required_payload_fields must be non-empty lists"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan evidence_contract required_payload_fields must contain canonical strings"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan evidence_contract required_payload_fields must not contain duplicate fields"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan evidence_contract required_payload_fields must match checker fields"
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
    payload = write_payload(tmp_path / "feed-promotion.json")
    args = MODULE.parse_args(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--require-kind",
            "feed_promotion",
            "--feed-promotion-evidence",
            str(payload),
        ]
    )
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["external_evidence"]["controller_runtime"] = [
        str(tmp_path / "controller-runtime.json")
    ]
    rendered["evidence_contract"]["controller_runtime"] = {
        "schema": MODULE.KIND_BY_NAME["controller_runtime"].schema,
        "required_payload_fields": list(
            MODULE.EVIDENCE_REQUIRED_FIELDS["controller_runtime"]
        ),
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "gateway compliance rollout runner plan external_evidence must contain only required kinds"
        in diagnostics
    )
    assert (
        "gateway compliance rollout runner plan evidence_contract must contain only required kinds"
        in diagnostics
    )
    assert "controller_runtime" not in diagnostics


def test_execution_rejects_plan_validation_drift_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_validate_plan_json(rendered, plan, args):
        return ["gateway compliance rollout runner plan schema must match the contract"]

    def fake_run_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "validate_plan_json", fake_validate_plan_json)
    monkeypatch.setattr(MODULE, "run_plan", fake_run_plan)

    assert MODULE.main(complete_args(tmp_path)) == 2

    assert not ran_plan
    assert (
        "gateway compliance rollout runner plan schema must match the contract"
        in capsys.readouterr().err
    )


def test_response_file_dry_run_prints_complete_plan(tmp_path: Path, capsys) -> None:
    args_file = write_args_file(
        tmp_path / "gateway-compliance-rollout.args",
        complete_args(tmp_path),
    )

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert plan["external_evidence"]["transparency_publication"]
    assert "appeal_override" in plan["evidence_contract"]


def test_non_dry_run_executes_without_printing_collection_plan(
    tmp_path: Path, capsys, monkeypatch
) -> None:
    calls = []

    def fake_run_plan(plan, out_dir):
        calls.append((plan, out_dir))
        return 0

    monkeypatch.setattr(MODULE, "run_plan", fake_run_plan)

    exit_code = MODULE.main(complete_args(tmp_path))

    captured = capsys.readouterr()
    assert exit_code == 0
    assert captured.out == ""
    assert captured.err == ""
    assert len(calls) == 1
    assert calls[0][0][0].label == "rollout_evidence_gate"
    assert calls[0][1] == tmp_path / "evidence"


def test_split_response_file_dry_run_prints_complete_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(
        tmp_path / "split-gateway-compliance-rollout.args",
        complete_args(tmp_path),
    )

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.gateway_compliance.rollout_evidence_collection_plan.v1"
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert "honey_audit" in plan["evidence_contract"]


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--enforcement-probe-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing required rollout evidence input" in captured.err
    assert captured.out == ""


def test_missing_controller_runtime_evidence_fails_before_plan(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--controller-runtime-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing required rollout evidence input" in captured.err
    assert captured.out == ""


def test_missing_moderation_toggle_evidence_fails_before_plan(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--moderation-toggle-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing required rollout evidence input" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-honey-audit.json"
    evidence_index = args.index("--honey-audit-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert "--honey-audit-evidence" not in captured.err
    assert str(missing) not in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "feed-promotion.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--require-kind",
            "feed_promotion",
            "--feed-promotion-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["feed_promotion"]
    assert list(plan["evidence_contract"]) == ["feed_promotion"]
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "feed_promotion" in verifier


def test_unknown_required_kind_fails_before_plan(tmp_path: Path, capsys) -> None:
    assert (
        MODULE.main(
            [
                "--out-dir",
                str(tmp_path / "evidence"),
                "--require-kind",
                "unknown",
                "--dry-run",
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "unknown required evidence kind" in captured.err
    assert "usage:" not in captured.err
    assert captured.out == ""
