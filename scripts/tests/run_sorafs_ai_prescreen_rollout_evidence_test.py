"""Tests for scripts/run_sorafs_ai_prescreen_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_ai_prescreen_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_ai_prescreen_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def write_payload(path: Path) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("{}", encoding="utf-8")
    return path


def complete_args(tmp_path: Path) -> list[str]:
    payload_dir = tmp_path / "payloads"
    bundle_dir = tmp_path / "executor-bundle"
    bundle_dir.mkdir()
    args = [
        "--sorafs-cli-bin",
        "/usr/local/bin/sorafs_cli",
        "--iroha-bin",
        "/usr/local/bin/iroha",
        "--iroha-arg",
        "--config",
        "--iroha-arg",
        "/runtime/client.toml",
        "--out-dir",
        str(tmp_path / "evidence"),
        "--manifest",
        str(write_payload(payload_dir / "manifest.json")),
        "--manifest-format",
        "json",
        "--runner-url",
        "https://runner.example",
        "--runner-payload",
        str(write_payload(payload_dir / "runner-payload.bin")),
        "--runner-subject",
        "cid:bafy-runner",
        "--screened-at",
        "1800004000",
        "--runner-checked-at",
        "1800004999",
        "--runner-timeout-ms",
        "5000",
        "--committee-url",
        "https://committee.example",
        "--quorum",
        "2",
        "--committee-result",
        str(write_payload(payload_dir / "result-a.json")),
        "--committee-result",
        str(write_payload(payload_dir / "result-b.json")),
        "--committee-checked-at",
        "1800005999",
        "--committee-timeout-ms",
        "5000",
        "--operator-url",
        "https://operator.example",
        "--quarantine-id",
        "12" * 16,
        "--limit",
        "4",
        "--operator-timeout-secs",
        "5",
        "--juror-notifications-manifest",
        str(write_payload(payload_dir / "juror-notifications.json")),
        "--notification-webhook-url",
        "https://notifications.example/hook",
        "--notification-timeout-secs",
        "5",
        "--executor-bundle",
        str(bundle_dir),
        "--executor-execution-summary",
        str(write_payload(payload_dir / "execution-summary.json")),
        "--governance-dag-evidence",
        str(write_payload(payload_dir / "governance-dag.json")),
        "--e2e-evidence",
        str(write_payload(payload_dir / "e2e.json")),
    ]
    for source_kind in MODULE.REQUIRED_TRANSPARENCY_SOURCE_KINDS:
        path = write_payload(payload_dir / f"{source_kind}.json")
        args.extend(["--source-entry", f"{source_kind}={path}"])
    return args


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


def test_dry_run_prints_complete_collection_plan(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.moderation.ai_prescreen.rollout_evidence_collection_plan.v1"
    assert (
        plan["verifier_summary_schema"]
        == "sorafs.moderation.ai_prescreen.rollout_evidence_gate.v1"
    )
    labels = [step["label"] for step in plan["steps"]]
    assert labels == [
        "runner_canary",
        "committee_canary",
        "operator_workflow_canary",
        "notification_transport_canary",
        "commit_reveal_executor_canary",
        "transparency_source_entry_canary",
        "rollout_evidence_gate",
    ]
    runner = plan["steps"][0]["command"]
    assert runner[:3] == ["/usr/local/bin/sorafs_cli", "moderation", "runner-canary"]
    assert "--format=json" in runner
    operator = plan["steps"][2]["command"]
    assert operator[:4] == ["/usr/local/bin/iroha", "--config", "/runtime/client.toml", "sorafs"]
    assert "operator-canary" in operator
    verifier = plan["steps"][6]["command"]
    assert "check_sorafs_ai_prescreen_rollout_evidence.py" in verifier[1]
    assert str(plan["external_evidence"]["governance_dag"]).endswith("governance-dag.json")
    assert plan["evidence_contract"]["runner"]["schema"] == (
        "sorafs.moderation.runner.rollout_evidence.v1"
    )
    assert "manifest_id_hex" in plan["evidence_contract"]["runner"]["required_payload_fields"]
    assert (
        "workflow_digest_hex"
        in plan["evidence_contract"]["operator_workflow"]["required_payload_fields"]
    )
    assert "routes" in plan["evidence_contract"]["operator_workflow"]["required_payload_fields"]
    assert (
        "execution_summary"
        in plan["evidence_contract"]["commit_reveal_executor"][
            "required_payload_fields"
        ]
    )
    assert "config_source" in plan["evidence_contract"]["governance_dag"]["required_payload_fields"]


def test_response_file_dry_run_prints_complete_collection_plan(tmp_path: Path, capsys) -> None:
    args_file = write_args_file(tmp_path / "rollout.args", complete_args(tmp_path))

    assert MODULE.main([f"@{args_file}", "--dry-run"]) == 0

    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "runner_canary"
    assert plan["steps"][6]["label"] == "rollout_evidence_gate"
    assert "end_to_end_workflow" in plan["evidence_contract"]


def test_split_response_file_dry_run_prints_complete_collection_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(tmp_path / "split.args", complete_args(tmp_path))

    assert MODULE.main([f"@{args_file}", "--dry-run"]) == 0

    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][3]["label"] == "notification_transport_canary"
    assert "notification_transport" in plan["evidence_contract"]


def test_missing_transparency_source_kind_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    source_index = args.index("--source-entry")
    del args[source_index : source_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing required source-entry coverage" in captured.err
    assert "dataset_manifest" not in captured.err
    assert captured.out == ""


def test_malformed_source_entry_sanitizes_exception_text(
    tmp_path: Path, monkeypatch
) -> None:
    bad_message = "transparency\nsource"

    def raise_malformed_source_entry(_spec: str):
        raise ValueError(bad_message)

    monkeypatch.setattr(MODULE, "split_source_entry_spec", raise_malformed_source_entry)
    errors = MODULE.validate_inputs(MODULE.parse_args(complete_args(tmp_path)))

    assert "<non-canonical-error>" in errors
    assert bad_message not in "\n".join(errors)


def test_malformed_source_entry_does_not_echo_spec(tmp_path: Path) -> None:
    args = complete_args(tmp_path)
    bad_spec = "source-entry-private-key-placeholder"
    source_index = args.index("--source-entry") + 1
    args[source_index] = bad_spec

    errors = MODULE.validate_inputs(MODULE.parse_args(args))

    diagnostics = "\n".join(errors)
    assert "--source-entry must use KIND=PATH form" in diagnostics
    assert bad_spec not in diagnostics


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-execution-summary.json"
    summary_index = args.index("--executor-execution-summary") + 1
    args[summary_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert "--executor-execution-summary" not in captured.err
    assert str(missing) not in captured.err


def test_service_url_rejects_secret_bearing_url_without_leaking(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    url = "https://notifications.example/private_key/hook?token=secret"
    args[args.index("--notification-webhook-url") + 1] = url

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "SoraFS runner URL arguments must not contain" in captured.err
    assert "private_key" not in captured.err
    assert "token=secret" not in captured.err
    assert captured.out == ""


def test_iroha_arg_rejects_secret_bearing_value_without_leaking(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    args.extend(["--iroha-arg", "--bearer-token=runtime-secret"])

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "SoraFS runner passthrough arguments must not contain" in captured.err
    assert "bearer-token" not in captured.err
    assert "runtime-secret" not in captured.err
    assert captured.out == ""


def test_committee_results_must_satisfy_quorum(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    quorum_index = args.index("--quorum") + 1
    args[quorum_index] = "3"

    assert MODULE.main([*args, "--dry-run"]) == 2

    assert "--committee-result count must be at least --quorum" in capsys.readouterr().err
