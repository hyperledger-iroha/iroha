"""Tests for scripts/run_sorafs_governance_dag_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = (
    Path(__file__).resolve().parents[1] / "run_sorafs_governance_dag_rollout_evidence.py"
)
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_governance_dag_rollout_evidence",
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
                deployment_id="governance-dag-production-a",
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
        "1800300000",
        *topology_args(tmp_path),
        "--max-evidence-age-secs",
        "86400",
        "--max-route-latency-ms",
        "1200",
        "--max-pin-lag-secs",
        "1200",
        "--max-head-age-secs",
        "1500",
        "--min-blocks",
        "4",
        "--min-payload-kinds",
        "6",
        "--ingest-service-evidence",
        str(write_payload(payload_dir / "ingest-service.json")),
        "--publisher-service-evidence",
        str(write_payload(payload_dir / "publisher-service.json")),
        "--mirror-datastore-evidence",
        str(write_payload(payload_dir / "mirror-datastore.json")),
        "--operator-recovery-evidence",
        str(write_payload(payload_dir / "operator-recovery.json")),
        "--dashboard-api-evidence",
        str(write_payload(payload_dir / "dashboard-api.json")),
        "--observability-evidence",
        str(write_payload(payload_dir / "observability.json")),
        "--publication-e2e-evidence",
        str(write_payload(payload_dir / "publication-e2e.json")),
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


def test_dry_run_prints_complete_governance_dag_rollout_plan(
    tmp_path: Path, capsys
) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.governance_dag.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.governance_dag.rollout_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_evidence_age_secs": 86400,
        "max_head_age_secs": 1500,
        "max_pin_lag_secs": 1200,
        "max_route_latency_ms": 1200,
        "min_blocks": 4,
        "min_payload_kinds": 6,
        "now_unix": 1800300000,
    }
    assert plan["external_evidence"]["publisher_service"] == [
        str(tmp_path / "payloads" / "publisher-service.json")
    ]
    assert plan["evidence_contract"]["publisher_service"]["schema"] == (
        "sorafs.governance_dag.publisher_service_canary.v1"
    )
    publisher_fields = plan["evidence_contract"]["publisher_service"][
        "required_payload_fields"
    ]
    assert "kubo_unixfs_profile" in publisher_fields
    assert "strong_single_etag_verified" in publisher_fields
    assert "ingress_enforcement" in publisher_fields
    assert "replay_posture" in publisher_fields
    assert (
        "policy_digest_hex"
        in plan["evidence_contract"]["publisher_service"]["required_payload_fields"]
    )
    assert (
        "sealed_typed_store_enabled"
        in plan["evidence_contract"]["mirror_datastore"]["required_payload_fields"]
    )
    assert (
        "retention_max_entries"
        in plan["evidence_contract"]["mirror_datastore"]["required_payload_fields"]
    )
    assert (
        "fresh_checkpoint_coherent_reads_verified"
        in plan["evidence_contract"]["dashboard_api"]["required_payload_fields"]
    )
    assert (
        "audit_max_bytes_per_poll"
        in plan["evidence_contract"]["observability"]["required_payload_fields"]
    )
    assert (
        "signed_http_head_resolved"
        in plan["evidence_contract"]["publication_e2e"]["required_payload_fields"]
    )
    assert (
        "iroha_config_bound"
        in plan["evidence_contract"]["governance_approval"]["required_payload_fields"]
    )
    assert (
        "replay_namespace_digest_hex"
        in plan["evidence_contract"]["governance_approval"]["required_payload_fields"]
    )
    assert [step["label"] for step in plan["steps"]] == ["rollout_evidence_gate"]
    verifier = plan["steps"][0]["command"]
    assert "check_sorafs_governance_dag_rollout_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "dashboard-api.json") in verifier
    assert verifier.count("--publisher-service-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--max-pin-lag-secs" in verifier
    assert "--now-unix" in verifier


def test_retired_ipns_kind_and_flag_are_rejected(tmp_path: Path, capsys) -> None:
    assert (
        MODULE.main(
            [
                "--out-dir",
                str(tmp_path / "evidence-kind"),
                *topology_args(tmp_path),
                "--require-kind",
                "ipfs_ipns_e2e",
                "--dry-run",
            ]
        )
        == 2
    )
    assert "unknown required evidence kind" in capsys.readouterr().err

    retired = write_payload(tmp_path / "retired-ipns.json")
    assert (
        MODULE.main(
            [
                "--out-dir",
                str(tmp_path / "evidence-flag"),
                *topology_args(tmp_path),
                "--require-kind",
                "publication_e2e",
                "--ipfs-ipns-e2e-evidence",
                str(retired),
                "--dry-run",
            ]
        )
        == 2
    )
    assert "unrecognized arguments" in capsys.readouterr().err


def test_plan_json_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    assert MODULE.validate_plan_json(rendered, plan, args) == []

    assert MODULE.validate_plan_json(["step"], plan, args) == [
        "Governance DAG rollout runner plan must be an object"
    ]

    rendered["schema"] = "sorafs.governance_dag.rollout_evidence_collection_plan.v0"
    rendered["unexpected"] = True
    rendered["required_kinds"] = []
    rendered["thresholds"] = {}
    rendered["external_evidence"] = {}
    rendered["evidence_contract"] = {}
    rendered["steps"] = []

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "Governance DAG rollout runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan schema must match the contract"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan required_kinds must match args"
        in diagnostics
    )
    assert "Governance DAG rollout runner plan thresholds must match args" in diagnostics
    assert (
        "Governance DAG rollout runner plan external_evidence must match args"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan evidence_contract must match checker fields"
        in diagnostics
    )
    assert "runner plan steps must match command plan" in diagnostics
    assert "dashboard-api.json" not in diagnostics


def test_plan_json_nested_shapes_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["bad\nfield"] = "runtime-only-key-material"
    rendered["schema"] = "sorafs\ngovernance"
    rendered["verifier_summary_schema"] = "summary\nschema"
    rendered["required_kinds"] = [
        "publisher_service",
        "publisher_service",
        "unknown_kind",
        "bad\nkind",
    ]
    rendered["thresholds"] = {
        "max_evidence_age_secs": -1,
        "max_route_latency_ms": 0,
        "max_pin_lag_secs": False,
        "max_head_age_secs": "soon",
        "min_blocks": 0,
        "min_payload_kinds": "many",
        "now_unix": 0,
        "bad\nfield": 1,
        "private_key": 2,
    }
    rendered["external_evidence"] = {
        "publisher_service": [],
        "unknown_kind": ["unknown.json"],
        "ingest_service": "ingest-service.json",
        "bad\nkind": ["publisher-service.json"],
        "dashboard_api": ["bad\npath"],
    }
    rendered["evidence_contract"] = {
        "publisher_service": {
            "schema": "wrong.schema.v1",
            "required_payload_fields": ["schema", "schema", "bad\nfield"],
            "raw_payload": True,
            "bad\nfield": "runtime-only-key-material",
        },
        "unknown_kind": {
            "schema": "sorafs.governance_dag.unknown.v1",
            "required_payload_fields": [],
        },
        "ingest_service": "contract-shaped-entry",
        "bad\nkind": {
            "schema": MODULE.KIND_BY_NAME["publisher_service"].schema,
            "required_payload_fields": ["schema"],
        },
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "Governance DAG rollout runner plan fields must be canonical strings"
        in diagnostics
    )
    assert "Governance DAG rollout runner plan schema must be canonical" in diagnostics
    assert (
        "Governance DAG rollout runner plan verifier schema must be canonical"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan required_kinds must contain canonical strings"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan required_kinds must not contain duplicate kinds"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan required_kinds must use known kind names"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan thresholds keys must be canonical strings"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan thresholds must contain only configured threshold fields"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan thresholds.max_evidence_age_secs must be a non-negative integer"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan thresholds.max_route_latency_ms must be a positive integer"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan thresholds.max_pin_lag_secs must be a positive integer"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan thresholds.max_head_age_secs must be a positive integer"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan thresholds.min_blocks must be a positive integer"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan thresholds.min_payload_kinds must be a positive integer"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan thresholds.now_unix must be a positive integer"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan external_evidence keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan external_evidence keys must use known kind names"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan external_evidence must map each kind to non-empty path lists"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan external_evidence paths must be canonical strings"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan evidence_contract keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan evidence_contract keys must use known kind names"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan evidence_contract must map each kind to a contract object"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan evidence_contract fields must be canonical strings"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan evidence_contract fields must be schema and required_payload_fields"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan evidence_contract schemas must match evidence kind"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan evidence_contract required_payload_fields must be non-empty lists"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan evidence_contract required_payload_fields must contain canonical strings"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan evidence_contract required_payload_fields must not contain duplicate fields"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan evidence_contract required_payload_fields must match checker fields"
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
    payload = write_payload(tmp_path / "ingest-service.json")
    args = MODULE.parse_args(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--now-unix",
            "1800300000",
            *topology_args(tmp_path),
            "--require-kind",
            "ingest_service",
            "--ingest-service-evidence",
            str(payload),
        ]
    )
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["external_evidence"]["publisher_service"] = [
        str(tmp_path / "publisher-service.json")
    ]
    rendered["evidence_contract"]["publisher_service"] = {
        "schema": MODULE.KIND_BY_NAME["publisher_service"].schema,
        "required_payload_fields": list(
            MODULE.EVIDENCE_REQUIRED_FIELDS["publisher_service"]
        ),
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "Governance DAG rollout runner plan external_evidence must contain only required kinds"
        in diagnostics
    )
    assert (
        "Governance DAG rollout runner plan evidence_contract must contain only required kinds"
        in diagnostics
    )
    assert "publisher_service" not in diagnostics


def test_execution_rejects_plan_validation_drift_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_validate_plan_json(rendered, plan, args):
        return ["Governance DAG rollout runner plan schema must match the contract"]

    def fake_run_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "validate_plan_json", fake_validate_plan_json)
    monkeypatch.setattr(MODULE, "run_plan", fake_run_plan)

    assert MODULE.main(complete_args(tmp_path)) == 2

    assert not ran_plan
    assert (
        "Governance DAG rollout runner plan schema must match the contract"
        in capsys.readouterr().err
    )


def test_response_file_dry_run_prints_complete_governance_dag_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_args_file(tmp_path / "governance-dag-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert plan["external_evidence"]["ingest_service"]
    assert "ingest_service" in plan["evidence_contract"]


def test_split_response_file_dry_run_prints_complete_governance_dag_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(
        tmp_path / "split-governance-dag-rollout.args",
        complete_args(tmp_path),
    )

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.governance_dag.rollout_evidence_collection_plan.v1"
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--publisher-service-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing required rollout evidence input" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-dashboard.json"
    evidence_index = args.index("--dashboard-api-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert "--dashboard-api-evidence" not in captured.err
    assert str(missing) not in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "ingest-service.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--now-unix",
            "1800300000",
            *topology_args(tmp_path),
            "--require-kind",
            "ingest_service",
            "--ingest-service-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["ingest_service"]
    assert list(plan["evidence_contract"]) == ["ingest_service"]
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "ingest_service" in verifier


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
