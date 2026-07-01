"""Tests for scripts/run_sorafs_pop_credentials_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_pop_credentials_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_pop_credentials_rollout_evidence",
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
        "1800006000",
        "--max-root-age-secs",
        "604800",
        "--max-revocation-age-secs",
        "86400",
        "--max-service-lag-secs",
        "900",
        "--max-verify-latency-ms",
        "1000",
        "--issuer-bundle-evidence",
        str(write_payload(payload_dir / "issuer-bundle.json")),
        "--commitment-root-evidence",
        str(write_payload(payload_dir / "commitment-root.json")),
        "--revocation-registry-evidence",
        str(write_payload(payload_dir / "revocation-registry.json")),
        "--enrollment-portal-evidence",
        str(write_payload(payload_dir / "enrollment-portal.json")),
        "--juror-client-evidence",
        str(write_payload(payload_dir / "juror-client.json")),
        "--verifier-service-evidence",
        str(write_payload(payload_dir / "verifier-service.json")),
        "--moderation-integration-evidence",
        str(write_payload(payload_dir / "moderation-integration.json")),
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


def test_dry_run_prints_complete_pop_rollout_plan(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.pop_credentials.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.pop_credentials.rollout_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_revocation_age_secs": 86400,
        "max_root_age_secs": 604800,
        "max_service_lag_secs": 900,
        "max_verify_latency_ms": 1000,
        "now_unix": 1800006000,
    }
    assert plan["external_evidence"]["issuer_bundle"] == [
        str(tmp_path / "payloads" / "issuer-bundle.json")
    ]
    assert plan["evidence_contract"]["issuer_bundle"]["schema"] == (
        "sorafs.pop.issuer_bundle_canary.v1"
    )
    assert (
        "root_digest_hex"
        in plan["evidence_contract"]["issuer_bundle"]["required_payload_fields"]
    )
    assert (
        "published_at_unix"
        in plan["evidence_contract"]["commitment_root"]["required_payload_fields"]
    )
    assert (
        "revoked_nonce_count"
        in plan["evidence_contract"]["revocation_registry"][
            "required_payload_fields"
        ]
    )
    assert (
        "synced_root_digest_hex"
        in plan["evidence_contract"]["juror_client"]["required_payload_fields"]
    )
    assert (
        "proof_probe_count"
        in plan["evidence_contract"]["verifier_service"]["required_payload_fields"]
    )
    assert (
        "policy_digest_hex"
        in plan["evidence_contract"]["verifier_service"]["required_payload_fields"]
    )
    assert (
        "privacy_proof_system"
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
    assert "check_sorafs_pop_credentials_rollout_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "verifier-service.json") in verifier
    assert verifier.count("--issuer-bundle-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--max-verify-latency-ms" in verifier
    assert "--now-unix" in verifier


def test_plan_json_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    assert MODULE.validate_plan_json(rendered, plan, args) == []

    assert MODULE.validate_plan_json(["step"], plan, args) == [
        "PoP credential rollout runner plan must be an object"
    ]

    rendered["schema"] = "sorafs.pop_credentials.rollout_evidence_collection_plan.v0"
    rendered["unexpected"] = True
    rendered["required_kinds"] = []
    rendered["thresholds"] = {}
    rendered["external_evidence"] = {}
    rendered["evidence_contract"] = {}
    rendered["steps"] = []

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "PoP credential rollout runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert "PoP credential rollout runner plan schema must match the contract" in diagnostics
    assert "PoP credential rollout runner plan required_kinds must match args" in diagnostics
    assert "PoP credential rollout runner plan thresholds must match args" in diagnostics
    assert (
        "PoP credential rollout runner plan external_evidence must match args"
        in diagnostics
    )
    assert (
        "PoP credential rollout runner plan evidence_contract must match checker fields"
        in diagnostics
    )
    assert "runner plan steps must match command plan" in diagnostics
    assert "issuer-bundle.json" not in diagnostics


def test_execution_rejects_plan_validation_drift_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_validate_plan_json(rendered, plan, args):
        return ["PoP credential rollout runner plan schema must match the contract"]

    def fake_run_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "validate_plan_json", fake_validate_plan_json)
    monkeypatch.setattr(MODULE, "run_plan", fake_run_plan)

    assert MODULE.main(complete_args(tmp_path)) == 2

    assert not ran_plan
    assert (
        "PoP credential rollout runner plan schema must match the contract"
        in capsys.readouterr().err
    )


def test_response_file_dry_run_prints_complete_pop_plan(tmp_path: Path, capsys) -> None:
    args_file = write_args_file(tmp_path / "pop-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert plan["external_evidence"]["juror_client"]
    assert "moderation_integration" in plan["evidence_contract"]


def test_split_response_file_dry_run_prints_complete_pop_plan(tmp_path: Path, capsys) -> None:
    args_file = write_split_args_file(tmp_path / "split-pop-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.pop_credentials.rollout_evidence_collection_plan.v1"
    assert "metrics_alerts" in plan["evidence_contract"]


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--verifier-service-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing required rollout evidence input" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-governance.json"
    evidence_index = args.index("--governance-approval-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert "--governance-approval-evidence" not in captured.err
    assert str(missing) not in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "issuer-bundle.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--require-kind",
            "issuer_bundle",
            "--issuer-bundle-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["issuer_bundle"]
    assert list(plan["evidence_contract"]) == ["issuer_bundle"]
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "issuer_bundle" in verifier


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
