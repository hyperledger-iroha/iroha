"""Tests for scripts/run_sorafs_reserve_rent_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_reserve_rent_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_reserve_rent_rollout_evidence",
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
        "1800007000",
        "--max-ledger-age-secs",
        "604800",
        "--max-lifecycle-lag-secs",
        "900",
        "--max-route-latency-ms",
        "1500",
        "--max-bake-age-secs",
        "1209600",
        "--policy-config-evidence",
        str(write_payload(payload_dir / "policy-config.json")),
        "--quote-matrix-evidence",
        str(write_payload(payload_dir / "quote-matrix.json")),
        "--ledger-digest-evidence",
        str(write_payload(payload_dir / "ledger-digest.json")),
        "--lifecycle-service-evidence",
        str(write_payload(payload_dir / "lifecycle-service.json")),
        "--signed-routes-evidence",
        str(write_payload(payload_dir / "signed-routes.json")),
        "--reserve-movement-evidence",
        str(write_payload(payload_dir / "reserve-movement.json")),
        "--credit-line-evidence",
        str(write_payload(payload_dir / "credit-line.json")),
        "--appeal-policy-evidence",
        str(write_payload(payload_dir / "appeal-policy.json")),
        "--metrics-alerts-evidence",
        str(write_payload(payload_dir / "metrics-alerts.json")),
        "--provider-bake-evidence",
        str(write_payload(payload_dir / "provider-bake.json")),
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


def test_dry_run_prints_complete_reserve_rollout_plan(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.reserve_rent.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.reserve_rent.rollout_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_bake_age_secs": 1209600,
        "max_ledger_age_secs": 604800,
        "max_lifecycle_lag_secs": 900,
        "max_route_latency_ms": 1500,
        "now_unix": 1800007000,
    }
    assert plan["external_evidence"]["signed_routes"] == [
        str(tmp_path / "payloads" / "signed-routes.json")
    ]
    assert plan["evidence_contract"]["provider_bake"]["schema"] == (
        "sorafs.reserve.provider_bake.v1"
    )
    assert (
        "scheduled_lifecycle_canary_passed"
        in plan["evidence_contract"]["provider_bake"]["required_payload_fields"]
    )
    assert (
        "confirmed_balance_projection_verified"
        in plan["evidence_contract"]["reserve_movement"]["required_payload_fields"]
    )
    assert (
        "live_chain_submission_verified"
        in plan["evidence_contract"]["reserve_movement"]["required_payload_fields"]
    )
    assert (
        "automatic_finality_polling_verified"
        in plan["evidence_contract"]["reserve_movement"]["required_payload_fields"]
    )
    assert (
        "live_account_mutation_verified"
        in plan["evidence_contract"]["credit_line"]["required_payload_fields"]
    )
    assert (
        "downstream_compliance_policy_applied"
        in plan["evidence_contract"]["governance_approval"]["required_payload_fields"]
    )
    assert [step["label"] for step in plan["steps"]] == ["rollout_evidence_gate"]
    verifier = plan["steps"][0]["command"]
    assert "check_sorafs_reserve_rent_rollout_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "ledger-digest.json") in verifier
    assert verifier.count("--signed-routes-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--max-route-latency-ms" in verifier
    assert "--now-unix" in verifier


def test_plan_json_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    assert MODULE.validate_plan_json(rendered, plan, args) == []

    assert MODULE.validate_plan_json(["step"], plan, args) == [
        "reserve/rent rollout runner plan must be an object"
    ]

    rendered["schema"] = "sorafs.reserve_rent.rollout_evidence_collection_plan.v0"
    rendered["unexpected"] = True
    rendered["required_kinds"] = []
    rendered["thresholds"] = {}
    rendered["external_evidence"] = {}
    rendered["evidence_contract"] = {}
    rendered["steps"] = []

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "reserve/rent rollout runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert "reserve/rent rollout runner plan schema must match the contract" in diagnostics
    assert "reserve/rent rollout runner plan required_kinds must match args" in diagnostics
    assert "reserve/rent rollout runner plan thresholds must match args" in diagnostics
    assert "reserve/rent rollout runner plan external_evidence must match args" in diagnostics
    assert (
        "reserve/rent rollout runner plan evidence_contract must match checker fields"
        in diagnostics
    )
    assert "runner plan steps must match command plan" in diagnostics
    assert "signed-routes.json" not in diagnostics


def test_plan_json_nested_shapes_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["bad\nfield"] = "runtime-only-key-material"
    rendered["schema"] = "sorafs\nreserve"
    rendered["verifier_summary_schema"] = "summary\nschema"
    rendered["required_kinds"] = [
        "policy_config",
        "policy_config",
        "unknown_kind",
        "bad\nkind",
    ]
    rendered["thresholds"] = {
        "max_ledger_age_secs": -1,
        "max_lifecycle_lag_secs": False,
        "max_route_latency_ms": 0,
        "max_bake_age_secs": "soon",
        "now_unix": 0,
        "bad\nfield": 1,
        "private_key": 2,
    }
    rendered["external_evidence"] = {
        "policy_config": [],
        "unknown_kind": ["unknown.json"],
        "signed_routes": "signed-routes.json",
        "bad\nkind": ["policy-config.json"],
        "provider_bake": ["bad\npath"],
    }
    rendered["evidence_contract"] = {
        "policy_config": {
            "schema": "wrong.schema.v1",
            "required_payload_fields": ["schema", "schema", "bad\nfield"],
            "raw_payload": True,
            "bad\nfield": "runtime-only-key-material",
        },
        "unknown_kind": {
            "schema": "sorafs.reserve.unknown.v1",
            "required_payload_fields": [],
        },
        "signed_routes": "contract-shaped-entry",
        "bad\nkind": {
            "schema": "sorafs.reserve.policy_config_canary.v1",
            "required_payload_fields": ["schema"],
        },
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "reserve/rent rollout runner plan fields must be canonical strings"
        in diagnostics
    )
    assert "reserve/rent rollout runner plan schema must be canonical" in diagnostics
    assert (
        "reserve/rent rollout runner plan verifier schema must be canonical"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan required_kinds must contain canonical strings"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan required_kinds must not contain duplicate kinds"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan required_kinds must use known kind names"
        in diagnostics
    )
    assert "reserve/rent rollout runner plan thresholds keys must be canonical strings" in diagnostics
    assert (
        "reserve/rent rollout runner plan thresholds must contain only configured threshold fields"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan thresholds.max_ledger_age_secs must be a non-negative integer"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan thresholds.max_lifecycle_lag_secs must be a non-negative integer"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan thresholds.max_bake_age_secs must be a non-negative integer"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan thresholds.max_route_latency_ms must be a positive integer"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan thresholds.now_unix must be a positive integer"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan external_evidence keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan external_evidence keys must use known kind names"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan external_evidence must map each kind to non-empty path lists"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan external_evidence paths must be canonical strings"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan evidence_contract keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan evidence_contract keys must use known kind names"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan evidence_contract must map each kind to a contract object"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan evidence_contract fields must be canonical strings"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan evidence_contract fields must be schema and required_payload_fields"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan evidence_contract schemas must match evidence kind"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan evidence_contract required_payload_fields must be non-empty lists"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan evidence_contract required_payload_fields must contain canonical strings"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan evidence_contract required_payload_fields must not contain duplicate fields"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan evidence_contract required_payload_fields must match checker fields"
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
    payload = write_payload(tmp_path / "policy-config.json")
    args = MODULE.parse_args(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--now-unix",
            "1800007000",
            "--require-kind",
            "policy_config",
            "--policy-config-evidence",
            str(payload),
        ]
    )
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["external_evidence"]["provider_bake"] = [
        str(tmp_path / "provider-bake.json")
    ]
    rendered["evidence_contract"]["provider_bake"] = {
        "schema": MODULE.KIND_BY_NAME["provider_bake"].schema,
        "required_payload_fields": list(
            MODULE.EVIDENCE_REQUIRED_FIELDS["provider_bake"]
        ),
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "reserve/rent rollout runner plan external_evidence must contain only required kinds"
        in diagnostics
    )
    assert (
        "reserve/rent rollout runner plan evidence_contract must contain only required kinds"
        in diagnostics
    )
    assert "provider_bake" not in diagnostics


def test_execution_rejects_plan_validation_drift_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_validate_plan_json(rendered, plan, args):
        return ["reserve/rent rollout runner plan schema must match the contract"]

    def fake_run_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "validate_plan_json", fake_validate_plan_json)
    monkeypatch.setattr(MODULE, "run_plan", fake_run_plan)

    assert MODULE.main(complete_args(tmp_path)) == 2

    assert not ran_plan
    assert (
        "reserve/rent rollout runner plan schema must match the contract"
        in capsys.readouterr().err
    )


def test_response_file_dry_run_prints_complete_reserve_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_args_file(tmp_path / "reserve-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert plan["external_evidence"]["provider_bake"]
    assert "provider_bake" in plan["evidence_contract"]


def test_split_response_file_dry_run_prints_complete_reserve_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(
        tmp_path / "split-reserve-rollout.args",
        complete_args(tmp_path),
    )

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.reserve_rent.rollout_evidence_collection_plan.v1"
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--signed-routes-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing required rollout evidence input" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-ledger.json"
    evidence_index = args.index("--ledger-digest-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert "--ledger-digest-evidence" not in captured.err
    assert str(missing) not in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "policy-config.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--now-unix",
            "1800007000",
            "--require-kind",
            "policy_config",
            "--policy-config-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["policy_config"]
    assert list(plan["evidence_contract"]) == ["policy_config"]
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "policy_config" in verifier


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
    assert captured.out == ""
