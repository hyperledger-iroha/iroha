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

from sorafs_rollout_runner_test_support import (  # noqa: E402
    write_topology_qualification,
)

TRANSPARENCY_SOURCE_KINDS = (
    "moderation-reviewed-quarantine",
    "moderation-appeal-handoff",
    "moderation-appeal-ballot",
    "moderation-juror-plan",
    "moderation-juror-notifications-delivery",
    "moderation-juror-notifications-canary",
    "moderation-commit-reveal-status",
    "moderation-ballots-executor",
)


def write_payload(path: Path) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("{}", encoding="utf-8")
    return path


def write_transparency_producer_evidence(path: Path) -> Path:
    producers = [
        {
            "source_kind": source_kind,
            "producer_id": f"moderation-{index}",
            "producer_route": f"internal:moderation/{index}",
            "provenance_digest_hex": f"{index + 1:x}" * 64,
            "durable_checkpoint_verified": True,
        }
        for index, source_kind in enumerate(TRANSPARENCY_SOURCE_KINDS)
    ]
    path.write_text(
        json.dumps(
            {
                "schema": "sorafs.moderation.transparency_source.producer_evidence.v1",
                "status": "passed",
                "generated_at_unix": 1_800_400_000,
                "deployment_id": "ai-prescreen-production-20260701",
                "environment": "production",
                "deployment_context_reviewed": True,
                "workflow_digest_hex": "a" * 64,
                "producer_count": len(producers),
                "generic_public_ingress_absent": True,
                "payload_bytes_included": False,
                "private_payloads_included": False,
                "producers": producers,
            }
        ),
        encoding="utf-8",
    )
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
        "--iroha-arg=--config",
        "--iroha-arg=/runtime/client.toml",
        "--out-dir",
        str(tmp_path / "evidence"),
        "--deployment-id",
        "ai-prescreen-production-20260701",
        "--environment",
        "production",
        "--deployment-context-reviewed",
        "true",
        "--generated-at-unix",
        "1800400000",
        "--now-unix",
        "1800400000",
        "--topology-qualification-summary",
        str(
            write_topology_qualification(
                tmp_path / "topology-qualification.json",
                deployment_id="ai-prescreen-production-20260701",
            )
        ),
        "--max-evidence-age-secs",
        "604800",
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
        "1800400000",
        "--runner-process-isolation-enforcement",
        "systemd_ip_filter",
        "--runner-process-isolation-attestation-digest",
        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        "--runner-process-isolation-verified-at",
        "1800399999",
        "--runner-process-isolation-reviewed",
        "true",
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
        "1800400000",
        "--committee-process-isolation-enforcement",
        "systemd_ip_filter",
        "--committee-process-isolation-attestation-digest",
        "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f",
        "--committee-process-isolation-verified-at",
        "1800399999",
        "--committee-process-isolation-reviewed",
        "true",
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
        "--transparency-producer-evidence",
        str(
            write_transparency_producer_evidence(
                payload_dir / "transparency-producers.json"
            )
        ),
        "--governance-dag-evidence",
        str(write_payload(payload_dir / "governance-dag.json")),
        "--e2e-evidence",
        str(write_payload(payload_dir / "e2e.json")),
    ]
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
        "rollout_evidence_gate",
    ]
    runner = plan["steps"][0]["command"]
    assert runner[:3] == ["/usr/local/bin/sorafs_cli", "moderation", "runner-canary"]
    assert "--format=json" in runner
    for reviewed_argument in (
        "--deployment-id=ai-prescreen-production-20260701",
        "--environment=production",
        "--deployment-context-reviewed=true",
        "--generated-at-unix=1800400000",
        "--checked-at=1800400000",
        "--process-isolation-enforcement=systemd_ip_filter",
        "--process-isolation-attestation-digest=000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        "--process-isolation-verified-at=1800399999",
        "--process-isolation-reviewed=true",
    ):
        assert reviewed_argument in runner
    committee = plan["steps"][1]["command"]
    for reviewed_argument in (
        "--deployment-id=ai-prescreen-production-20260701",
        "--environment=production",
        "--deployment-context-reviewed=true",
        "--generated-at-unix=1800400000",
        "--checked-at=1800400000",
        "--process-isolation-enforcement=systemd_ip_filter",
        "--process-isolation-attestation-digest=202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f",
        "--process-isolation-verified-at=1800399999",
        "--process-isolation-reviewed=true",
    ):
        assert reviewed_argument in committee
    operator = plan["steps"][2]["command"]
    assert operator[:4] == ["/usr/local/bin/iroha", "--config", "/runtime/client.toml", "sorafs"]
    assert "operator-canary" in operator
    verifier = plan["steps"][5]["command"]
    assert "check_sorafs_ai_prescreen_rollout_evidence.py" in verifier[1]
    assert "--now-unix" in verifier
    assert "1800400000" in verifier
    assert "--max-evidence-age-secs" in verifier
    assert "604800" in verifier
    assert str(plan["external_evidence"]["governance_dag"]).endswith("governance-dag.json")
    assert str(plan["external_evidence"]["transparency_publication"]).endswith(
        "transparency-producers.json"
    )
    assert plan["evidence_contract"]["runner"]["schema"] == (
        "sorafs.moderation.runner.rollout_evidence.v1"
    )
    assert "manifest_id_hex" in plan["evidence_contract"]["runner"]["required_payload_fields"]
    assert "policy_digest_hex" in plan["evidence_contract"]["runner"]["required_payload_fields"]
    assert "probes" in plan["evidence_contract"]["runner"]["required_payload_fields"]
    assert "synthetic" in plan["evidence_contract"]["runner"]["required_payload_fields"]
    assert "results" in plan["evidence_contract"]["committee"]["required_payload_fields"]
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
    assert (
        "manifest_path"
        in plan["evidence_contract"]["notification_transport"][
            "required_payload_fields"
        ]
    )
    assert (
        "bundle_metadata_bytes"
        in plan["evidence_contract"]["commit_reveal_executor"][
            "required_payload_fields"
        ]
    )
    assert (
        "bundle_metadata_blake3"
        in plan["evidence_contract"]["commit_reveal_executor"][
            "required_payload_fields"
        ]
    )
    assert "config_source" in plan["evidence_contract"]["governance_dag"]["required_payload_fields"]


def test_plan_json_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    assert MODULE.validate_plan_json(rendered, plan, args) == []

    assert MODULE.validate_plan_json(["step"], plan, args) == [
        "AI pre-screen rollout runner plan must be an object"
    ]

    rendered["schema"] = "sorafs.moderation.ai_prescreen.rollout_evidence_collection_plan.v0"
    rendered["unexpected"] = True
    rendered["external_evidence"] = {}
    rendered["evidence_contract"] = {}
    rendered["steps"] = []

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "AI pre-screen rollout runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert "AI pre-screen rollout runner plan schema must match the contract" in diagnostics
    assert (
        "AI pre-screen rollout runner plan external_evidence must match args"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan evidence_contract must match checker fields"
        in diagnostics
    )
    assert "runner plan steps must match command plan" in diagnostics
    assert "runner-payload.bin" not in diagnostics


def test_plan_json_nested_shapes_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["bad\nfield"] = "runtime-only-key-material"
    rendered["schema"] = "sorafs\nai_prescreen"
    rendered["verifier_summary_schema"] = "summary\nschema"
    rendered["external_evidence"] = {
        "governance_dag": "bad\npath",
        "runner": ["runner.json"],
        "unknown_kind": "unknown.json",
        "bad\nkind": "governance-dag.json",
        "private_key": "runtime-only-key-material",
    }
    rendered["evidence_contract"] = {
        "runner": {
            "schema": "wrong.schema.v1",
            "required_payload_fields": ["schema", "schema", "bad\nfield"],
            "raw_payload": True,
            "bad\nfield": "runtime-only-key-material",
        },
        "unknown_kind": {
            "schema": "sorafs.moderation.unknown.v1",
            "required_payload_fields": [],
        },
        "bad\nkind": "contract-shaped-entry",
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert "AI pre-screen rollout runner plan fields must be canonical strings" in diagnostics
    assert "AI pre-screen rollout runner plan schema must be canonical" in diagnostics
    assert (
        "AI pre-screen rollout runner plan verifier schema must be canonical"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan external_evidence keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan external_evidence keys must use known kind names"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan external_evidence must contain only configured evidence fields"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan external_evidence values must be canonical strings"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan external_evidence must match configured fields"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan evidence_contract keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan evidence_contract keys must use known kind names"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan evidence_contract must map each kind to a contract object"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan evidence_contract fields must be canonical strings"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan evidence_contract fields must be schema and required_payload_fields"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan evidence_contract schemas must match evidence kind"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan evidence_contract required_payload_fields must be non-empty lists"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan evidence_contract required_payload_fields must contain canonical strings"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan evidence_contract required_payload_fields must not contain duplicate fields"
        in diagnostics
    )
    assert (
        "AI pre-screen rollout runner plan evidence_contract required_payload_fields must match checker fields"
        in diagnostics
    )
    assert "unknown_kind" not in diagnostics
    assert "bad\nkind" not in diagnostics
    assert "bad\nfield" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "private_key" not in diagnostics
    assert "wrong.schema.v1" not in diagnostics


def test_execution_rejects_plan_validation_drift_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_validate_plan_json(rendered, plan, args):
        return ["AI pre-screen rollout runner plan schema must match the contract"]

    def fake_run_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "validate_plan_json", fake_validate_plan_json)
    monkeypatch.setattr(MODULE, "run_plan", fake_run_plan)

    assert MODULE.main(complete_args(tmp_path)) == 2

    assert not ran_plan
    assert (
        "AI pre-screen rollout runner plan schema must match the contract"
        in capsys.readouterr().err
    )


def test_response_file_dry_run_prints_complete_collection_plan(tmp_path: Path, capsys) -> None:
    args_file = write_args_file(tmp_path / "rollout.args", complete_args(tmp_path))

    assert MODULE.main([f"@{args_file}", "--dry-run"]) == 0

    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "runner_canary"
    assert plan["steps"][5]["label"] == "rollout_evidence_gate"
    assert "end_to_end_workflow" in plan["evidence_contract"]


def test_split_response_file_dry_run_prints_complete_collection_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(tmp_path / "split.args", complete_args(tmp_path))

    assert MODULE.main([f"@{args_file}", "--dry-run"]) == 0

    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][3]["label"] == "notification_transport_canary"
    assert "notification_transport" in plan["evidence_contract"]


def test_missing_transparency_producer_evidence_fails_before_plan(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    source_index = args.index("--transparency-producer-evidence") + 1
    args[source_index] = str(tmp_path / "missing-producer-evidence.json")

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert "missing-producer-evidence.json" not in captured.err
    assert captured.out == ""


def test_invalid_transparency_producer_evidence_fails_without_leaking(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    path = Path(args[args.index("--transparency-producer-evidence") + 1])
    path.write_text(
        json.dumps({"schema": "private-key-placeholder"}),
        encoding="utf-8",
    )

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert MODULE.TRANSPARENCY_PRODUCER_EVIDENCE_INVALID_DIAGNOSTIC in captured.err
    assert "private-key-placeholder" not in captured.err
    assert str(path) not in captured.err
    assert captured.out == ""


def test_transparency_producer_evidence_must_match_rollout_context(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    path = Path(args[args.index("--transparency-producer-evidence") + 1])
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["deployment_id"] = "different-production"
    path.write_text(json.dumps(payload), encoding="utf-8")

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert MODULE.TRANSPARENCY_PRODUCER_EVIDENCE_CONTEXT_DIAGNOSTIC in captured.err
    assert "different-production" not in captured.err
    assert str(path) not in captured.err
    assert captured.out == ""


def test_transparency_producer_evidence_read_error_is_sanitized(
    tmp_path: Path, monkeypatch
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    bad_message = "transparency\nprivate-key-placeholder"

    def load_raises(_path: Path, _max_bytes: int):
        raise ValueError(bad_message)

    monkeypatch.setattr(MODULE, "load_evidence_json", load_raises)
    errors = MODULE.validate_transparency_producer_evidence(args)

    assert errors == [MODULE.TRANSPARENCY_PRODUCER_EVIDENCE_READ_DIAGNOSTIC]
    assert bad_message not in "\n".join(errors)


def test_invalid_freshness_args_fail_before_plan(tmp_path: Path, capsys) -> None:
    cases = (
        ("--now-unix", "0", "must be positive"),
        ("--max-evidence-age-secs", "-1", "must be non-negative"),
    )

    for option, value, diagnostic in cases:
        case_dir = tmp_path / option.removeprefix("--").replace("-", "_")
        case_dir.mkdir()
        args = complete_args(case_dir)
        value_index = args.index(option) + 1
        args[value_index] = value

        assert MODULE.main([*args, "--dry-run"]) == 2

        captured = capsys.readouterr()
        assert diagnostic in captured.err
        assert captured.out == ""


def test_transparency_producer_evidence_rejects_generic_public_ingress(
    tmp_path: Path,
) -> None:
    args = complete_args(tmp_path)
    path = Path(args[args.index("--transparency-producer-evidence") + 1])
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["generic_public_ingress_absent"] = False
    payload["producers"][0]["producer_route"] = (
        "/v1/sorafs/transparency/source-entries/moderation-reviewed-quarantine"
    )
    path.write_text(json.dumps(payload), encoding="utf-8")

    errors = MODULE.validate_inputs(MODULE.parse_args(args))

    assert errors == [MODULE.TRANSPARENCY_PRODUCER_EVIDENCE_INVALID_DIAGNOSTIC]


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


def test_service_url_rejects_encoded_host_tokens_without_leaking(
    tmp_path: Path, capsys
) -> None:
    unsafe_urls = (
        "https://C%3A.notifications.example/hook",
        "https://http%3A.notifications.example/hook",
    )

    for index, unsafe_url in enumerate(unsafe_urls):
        case_dir = tmp_path / f"url-case-{index}"
        case_dir.mkdir()
        args = complete_args(case_dir)
        args[args.index("--notification-webhook-url") + 1] = unsafe_url

        assert MODULE.main([*args, "--dry-run"]) == 2

        captured = capsys.readouterr()
        assert "SoraFS runner URL arguments must not contain" in captured.err
        assert unsafe_url not in captured.err
        assert "C%3A" not in captured.err
        assert "http%3A" not in captured.err
        assert captured.out == ""


def test_iroha_arg_rejects_secret_bearing_value_without_leaking(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    args.append("--iroha-arg=--bearer-token=runtime-secret")

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "SoraFS runner passthrough arguments must not contain" in captured.err
    assert "bearer-token" not in captured.err
    assert "runtime-secret" not in captured.err
    assert captured.out == ""


def test_split_iroha_arg_form_fails_without_leaking_value(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    args.extend(["--iroha-arg", "--bearer-token=runtime-secret"])

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert MODULE.IROHA_ARG_EQUALS_FORM_DIAGNOSTIC in captured.err
    assert "bearer-token" not in captured.err
    assert "runtime-secret" not in captured.err
    assert captured.out == ""


def test_committee_results_must_satisfy_quorum(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    quorum_index = args.index("--quorum") + 1
    args[quorum_index] = "3"

    assert MODULE.main([*args, "--dry-run"]) == 2

    assert "--committee-result count must be at least --quorum" in capsys.readouterr().err


def test_collection_requires_explicit_reviewed_deployment_context(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    index = args.index("--deployment-context-reviewed")
    del args[index : index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    assert "--deployment-context-reviewed" in capsys.readouterr().err


def test_collection_requires_runner_process_isolation_attestation(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    index = args.index("--runner-process-isolation-reviewed")
    del args[index : index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2
    assert "--runner-process-isolation-reviewed" in capsys.readouterr().err


def test_collection_rejects_placeholder_or_future_isolation_attestation(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    digest_index = args.index("--runner-process-isolation-attestation-digest") + 1
    args[digest_index] = "00" * 32
    verified_index = args.index("--runner-process-isolation-verified-at") + 1
    args[verified_index] = "1800400001"

    assert MODULE.main([*args, "--dry-run"]) == 2
    errors = capsys.readouterr().err
    assert "placeholder digest" in errors
    assert "must not be after --generated-at-unix" in errors


def test_collection_requires_committee_process_isolation_attestation(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    index = args.index("--committee-process-isolation-reviewed")
    del args[index : index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2
    assert "--committee-process-isolation-reviewed" in capsys.readouterr().err


def test_collection_rejects_placeholder_or_future_committee_isolation_attestation(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    digest_index = args.index("--committee-process-isolation-attestation-digest") + 1
    args[digest_index] = "ab" * 32
    verified_index = args.index("--committee-process-isolation-verified-at") + 1
    args[verified_index] = "1800400001"

    assert MODULE.main([*args, "--dry-run"]) == 2
    errors = capsys.readouterr().err
    assert "placeholder digest" in errors
    assert "must not be after --generated-at-unix" in errors


def test_collection_rejects_context_drift_markers_before_plan(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    args[args.index("--deployment-id") + 1] = "ai-prescreen-dev-placeholder"
    args[args.index("--environment") + 1] = "staging"

    assert MODULE.main([*args, "--dry-run"]) == 2

    diagnostics = capsys.readouterr().err
    assert "deployment_id must not contain non-reviewed deployment markers" in diagnostics


def test_collection_rejects_unbound_and_unfresh_timestamps(
    tmp_path: Path,
    capsys,
) -> None:
    cases = (
        (
            "runner-mismatch",
            "--runner-checked-at",
            "1800399999",
            "--runner-checked-at must equal --generated-at-unix",
        ),
        (
            "committee-mismatch",
            "--committee-checked-at",
            "1800399999",
            "--committee-checked-at must equal --generated-at-unix",
        ),
        (
            "future",
            "--generated-at-unix",
            "1800400001",
            "--generated-at-unix must not be after --now-unix",
        ),
        (
            "stale",
            "--generated-at-unix",
            "1799000000",
            "--generated-at-unix exceeds --max-evidence-age-secs at --now-unix",
        ),
    )
    for label, option, value, expected in cases:
        case_dir = tmp_path / label
        case_dir.mkdir()
        args = complete_args(case_dir)
        args[args.index(option) + 1] = value
        if option == "--generated-at-unix":
            args[args.index("--runner-checked-at") + 1] = value
            args[args.index("--committee-checked-at") + 1] = value

        assert MODULE.main([*args, "--dry-run"]) == 2
        assert expected in capsys.readouterr().err
