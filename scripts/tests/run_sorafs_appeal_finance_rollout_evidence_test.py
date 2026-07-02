"""Tests for scripts/run_sorafs_appeal_finance_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_appeal_finance_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_appeal_finance_rollout_evidence",
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
        "1800008000",
        "--max-canary-age-secs",
        "86400",
        "--max-dashboard-age-secs",
        "604800",
        "--max-route-latency-ms",
        "1500",
        "--max-settlement-lag-secs",
        "900",
        "--min-peers",
        "4",
        "--pricing-config-evidence",
        str(write_payload(payload_dir / "pricing-config.json")),
        "--quote-api-evidence",
        str(write_payload(payload_dir / "quote-api.json")),
        "--deposit-lifecycle-evidence",
        str(write_payload(payload_dir / "deposit-lifecycle.json")),
        "--settlement-execution-evidence",
        str(write_payload(payload_dir / "settlement-execution.json")),
        "--settlement-submitter-evidence",
        str(write_payload(payload_dir / "settlement-submitter.json")),
        "--moderation-worker-evidence",
        str(write_payload(payload_dir / "moderation-worker.json")),
        "--governance-dag-publication-evidence",
        str(write_payload(payload_dir / "governance-dag-publication.json")),
        "--dashboard-metrics-evidence",
        str(write_payload(payload_dir / "dashboard-metrics.json")),
        "--multi-peer-reconciliation-evidence",
        str(write_payload(payload_dir / "multi-peer-reconciliation.json")),
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


def test_dry_run_prints_complete_appeal_finance_rollout_plan(
    tmp_path: Path, capsys
) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.appeal_finance.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.appeal_finance.rollout_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_canary_age_secs": 86400,
        "max_dashboard_age_secs": 604800,
        "max_route_latency_ms": 1500,
        "max_settlement_lag_secs": 900,
        "min_peers": 4,
        "now_unix": 1800008000,
    }
    assert plan["external_evidence"]["multi_peer_reconciliation"] == [
        str(tmp_path / "payloads" / "multi-peer-reconciliation.json")
    ]
    assert plan["evidence_contract"]["pricing_config"]["schema"] == (
        "sorafs.appeal_finance.pricing_config_canary.v1"
    )
    assert (
        "config_source"
        in plan["evidence_contract"]["pricing_config"]["required_payload_fields"]
    )
    assert (
        "policy_digest_hex"
        in plan["evidence_contract"]["pricing_config"]["required_payload_fields"]
    )
    assert (
        "routes"
        in plan["evidence_contract"]["deposit_lifecycle"]["required_payload_fields"]
    )
    assert (
        "deposit_probes"
        in plan["evidence_contract"]["deposit_lifecycle"]["required_payload_fields"]
    )
    assert (
        "settlement_probe_count"
        in plan["evidence_contract"]["settlement_execution"]["required_payload_fields"]
    )
    assert (
        "signers"
        in plan["evidence_contract"]["settlement_submitter"]["required_payload_fields"]
    )
    assert (
        "steps"
        in plan["evidence_contract"]["settlement_submitter"]["required_payload_fields"]
    )
    assert (
        "runtime_signed_dag_verified"
        in plan["evidence_contract"]["governance_dag_publication"][
            "required_payload_fields"
        ]
    )
    assert (
        "reports"
        in plan["evidence_contract"]["governance_dag_publication"][
            "required_payload_fields"
        ]
    )
    assert (
        "weekly_rollups"
        in plan["evidence_contract"]["governance_dag_publication"][
            "required_payload_fields"
        ]
    )
    assert (
        "settlement_receipts"
        in plan["evidence_contract"]["governance_dag_publication"][
            "required_payload_fields"
        ]
    )
    assert (
        "qc_quorum_satisfied"
        in plan["evidence_contract"]["multi_peer_reconciliation"][
            "required_payload_fields"
        ]
    )
    assert (
        "peers"
        in plan["evidence_contract"]["multi_peer_reconciliation"][
            "required_payload_fields"
        ]
    )
    assert (
        "validators"
        in plan["evidence_contract"]["multi_peer_reconciliation"][
            "required_payload_fields"
        ]
    )
    assert (
        "cases"
        in plan["evidence_contract"]["multi_peer_reconciliation"][
            "required_payload_fields"
        ]
    )
    assert (
        "iroha_config_bound"
        in plan["evidence_contract"]["governance_approval"]["required_payload_fields"]
    )
    assert [step["label"] for step in plan["steps"]] == ["rollout_evidence_gate"]
    verifier = plan["steps"][0]["command"]
    assert "check_sorafs_appeal_finance_rollout_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "deposit-lifecycle.json") in verifier
    assert verifier.count("--deposit-lifecycle-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--min-peers" in verifier
    assert "--now-unix" in verifier


def test_plan_json_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    assert MODULE.validate_plan_json(rendered, plan, args) == []

    assert MODULE.validate_plan_json(["step"], plan, args) == [
        "appeal finance rollout runner plan must be an object"
    ]

    rendered["schema"] = "sorafs.appeal_finance.rollout_evidence_collection_plan.v0"
    rendered["unexpected"] = True
    rendered["required_kinds"] = []
    rendered["thresholds"] = {}
    rendered["external_evidence"] = {}
    rendered["evidence_contract"] = {}
    rendered["steps"] = []

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "appeal finance rollout runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan schema must match the contract"
        in diagnostics
    )
    assert "appeal finance rollout runner plan required_kinds must match args" in diagnostics
    assert "appeal finance rollout runner plan thresholds must match args" in diagnostics
    assert (
        "appeal finance rollout runner plan external_evidence must match args"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan evidence_contract must match checker fields"
        in diagnostics
    )
    assert "runner plan steps must match command plan" in diagnostics
    assert "multi-peer-reconciliation.json" not in diagnostics


def test_plan_json_nested_shapes_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["bad\nfield"] = "runtime-only-key-material"
    rendered["schema"] = "sorafs\nappeal"
    rendered["verifier_summary_schema"] = "summary\nschema"
    rendered["required_kinds"] = [
        "pricing_config",
        "pricing_config",
        "unknown_kind",
        "bad\nkind",
    ]
    rendered["thresholds"] = {
        "max_canary_age_secs": -1,
        "max_dashboard_age_secs": False,
        "max_route_latency_ms": 0,
        "max_settlement_lag_secs": "soon",
        "min_peers": 0,
        "now_unix": 0,
        "bad\nfield": 1,
        "private_key": 2,
    }
    rendered["external_evidence"] = {
        "pricing_config": [],
        "unknown_kind": ["unknown.json"],
        "quote_api": "quote-api.json",
        "bad\nkind": ["pricing-config.json"],
        "dashboard_metrics": ["bad\npath"],
    }
    rendered["evidence_contract"] = {
        "pricing_config": {
            "schema": "wrong.schema.v1",
            "required_payload_fields": ["schema", "schema", "bad\nfield"],
            "raw_payload": True,
            "bad\nfield": "runtime-only-key-material",
        },
        "unknown_kind": {
            "schema": "sorafs.appeal_finance.unknown.v1",
            "required_payload_fields": [],
        },
        "quote_api": "contract-shaped-entry",
        "bad\nkind": {
            "schema": MODULE.KIND_BY_NAME["pricing_config"].schema,
            "required_payload_fields": ["schema"],
        },
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "appeal finance rollout runner plan fields must be canonical strings"
        in diagnostics
    )
    assert "appeal finance rollout runner plan schema must be canonical" in diagnostics
    assert (
        "appeal finance rollout runner plan verifier schema must be canonical"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan required_kinds must contain canonical strings"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan required_kinds must not contain duplicate kinds"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan required_kinds must use known kind names"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan thresholds keys must be canonical strings"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan thresholds must contain only configured threshold fields"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan thresholds.max_canary_age_secs must be a non-negative integer"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan thresholds.max_dashboard_age_secs must be a non-negative integer"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan thresholds.max_settlement_lag_secs must be a non-negative integer"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan thresholds.max_route_latency_ms must be a positive integer"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan thresholds.min_peers must be a positive integer"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan thresholds.now_unix must be a positive integer"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan external_evidence keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan external_evidence keys must use known kind names"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan external_evidence must map each kind to non-empty path lists"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan external_evidence paths must be canonical strings"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan evidence_contract keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan evidence_contract keys must use known kind names"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan evidence_contract must map each kind to a contract object"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan evidence_contract fields must be canonical strings"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan evidence_contract fields must be schema and required_payload_fields"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan evidence_contract schemas must match evidence kind"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan evidence_contract required_payload_fields must be non-empty lists"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan evidence_contract required_payload_fields must contain canonical strings"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan evidence_contract required_payload_fields must not contain duplicate fields"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan evidence_contract required_payload_fields must match checker fields"
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
    payload = write_payload(tmp_path / "pricing-config.json")
    args = MODULE.parse_args(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--require-kind",
            "pricing_config",
            "--pricing-config-evidence",
            str(payload),
        ]
    )
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["external_evidence"]["quote_api"] = [str(tmp_path / "quote-api.json")]
    rendered["evidence_contract"]["quote_api"] = {
        "schema": MODULE.KIND_BY_NAME["quote_api"].schema,
        "required_payload_fields": list(MODULE.EVIDENCE_REQUIRED_FIELDS["quote_api"]),
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "appeal finance rollout runner plan external_evidence must contain only required kinds"
        in diagnostics
    )
    assert (
        "appeal finance rollout runner plan evidence_contract must contain only required kinds"
        in diagnostics
    )
    assert "quote_api" not in diagnostics


def test_execution_rejects_plan_validation_drift_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_validate_plan_json(rendered, plan, args):
        return ["appeal finance rollout runner plan schema must match the contract"]

    def fake_run_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "validate_plan_json", fake_validate_plan_json)
    monkeypatch.setattr(MODULE, "run_plan", fake_run_plan)

    assert MODULE.main(complete_args(tmp_path)) == 2

    assert not ran_plan
    assert (
        "appeal finance rollout runner plan schema must match the contract"
        in capsys.readouterr().err
    )


def test_response_file_dry_run_prints_complete_plan(tmp_path: Path, capsys) -> None:
    args_file = write_args_file(tmp_path / "appeal-finance-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert plan["external_evidence"]["settlement_submitter"]
    assert "settlement_submitter" in plan["evidence_contract"]


def test_split_response_file_dry_run_prints_complete_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(
        tmp_path / "split-appeal-finance-rollout.args",
        complete_args(tmp_path),
    )

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.appeal_finance.rollout_evidence_collection_plan.v1"
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert "dashboard_metrics" in plan["evidence_contract"]


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--multi-peer-reconciliation-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing required rollout evidence input" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-dashboard.json"
    evidence_index = args.index("--dashboard-metrics-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert "--dashboard-metrics-evidence" not in captured.err
    assert str(missing) not in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "pricing-config.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--require-kind",
            "pricing_config",
            "--pricing-config-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["pricing_config"]
    assert list(plan["evidence_contract"]) == ["pricing_config"]
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "pricing_config" in verifier


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
