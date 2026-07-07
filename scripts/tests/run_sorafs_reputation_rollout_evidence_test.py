"""Tests for scripts/run_sorafs_reputation_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_reputation_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_reputation_rollout_evidence",
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
        "--sorafs-cli-bin",
        "/usr/local/bin/sorafs_cli",
        "--torii-url",
        "https://torii.example",
        "--snapshot",
        str(write_payload(payload_dir / "reputation-snapshot.to")),
        "--provider-id",
        "provider-a",
        "--provider-proof",
        f"provider-a={write_payload(payload_dir / 'provider-a-proof.to')}",
        "--metrics-evidence",
        str(write_payload(payload_dir / "metrics.json")),
        "--transport-evidence",
        str(write_payload(payload_dir / "transport.json")),
        "--consumption-evidence",
        str(write_payload(payload_dir / "consumption.json")),
        "--out-dir",
        str(tmp_path / "evidence"),
        "--watch-limit",
        "7",
        "--watch-max-polls",
        "2",
        "--watch-poll-interval-ms",
        "0",
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


def test_dry_run_prints_complete_reputation_rollout_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    exit_code = MODULE.main([*args, "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.reputation.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.reputation.rollout_evidence_gate.v1"
    assert plan["external_evidence"] == {
        "metrics": args[args.index("--metrics-evidence") + 1],
        "transport": args[args.index("--transport-evidence") + 1],
        "consumption": args[args.index("--consumption-evidence") + 1],
    }
    assert plan["evidence_contract"]["publish"]["schema"] == (
        "sorafs.reputation.publish_snapshot_summary.v1"
    )
    assert (
        "generated_at_unix"
        in plan["evidence_contract"]["latest"]["required_payload_fields"]
    )
    assert "providers" in plan["evidence_contract"]["publish"][
        "required_payload_fields"
    ]
    assert "providers" in plan["evidence_contract"]["latest"][
        "required_payload_fields"
    ]
    assert (
        "provider"
        in plan["evidence_contract"]["provider"]["required_payload_fields"]
    )
    assert (
        "proof_verified"
        in plan["evidence_contract"]["verify"]["required_payload_fields"]
    )
    assert "providers" in plan["evidence_contract"]["verify"][
        "required_payload_fields"
    ]
    assert plan["evidence_contract"]["metrics"]["schema"] == (
        "sorafs.reputation.metrics_canary.v1"
    )
    assert "providers" in plan["evidence_contract"]["metrics"][
        "required_payload_fields"
    ]
    assert (
        "sse_connected"
        in plan["evidence_contract"]["transport"]["required_payload_fields"]
    )
    assert (
        "routing_score_consumed"
        in plan["evidence_contract"]["consumption"]["required_payload_fields"]
    )
    assert "providers" in plan["evidence_contract"]["consumption"][
        "required_payload_fields"
    ]
    labels = [step["label"] for step in plan["steps"]]
    assert labels == [
        "publish_snapshot",
        "fetch_latest_snapshot",
        "fetch_provider_provider-a",
        "verify_provider_provider-a",
        "watch_reputation_events",
        "rollout_evidence_gate",
    ]
    publish = plan["steps"][0]["command"]
    assert publish[:3] == ["/usr/local/bin/sorafs_cli", "reputation", "publish"]
    assert "--torii-url=https://torii.example" in publish
    assert any(arg.startswith("--summary-out=") for arg in publish)
    fetch = plan["steps"][2]["command"]
    assert "--format=json" in fetch
    verify = plan["steps"][3]["command"]
    assert "--provider-id=provider-a" in verify
    verifier = plan["steps"][5]["command"]
    assert "check_sorafs_reputation_rollout_evidence.py" in verifier[1]
    assert "--require-provider" in verifier
    assert "provider-a" in verifier


def test_plan_json_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    assert MODULE.validate_plan_json(rendered, plan, args) == []

    assert MODULE.validate_plan_json(["step"], plan, args) == [
        "reputation rollout runner plan must be an object"
    ]

    rendered["schema"] = "sorafs.reputation.rollout_evidence_collection_plan.v0"
    rendered["unexpected"] = True
    rendered["external_evidence"] = {"metrics": str(tmp_path / "metrics.json")}
    rendered["evidence_contract"] = {}
    rendered["steps"] = []

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "reputation rollout runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert "reputation rollout runner plan schema must match the contract" in diagnostics
    assert (
        "reputation rollout runner plan external_evidence must match args"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan evidence_contract must match checker fields"
        in diagnostics
    )
    assert "runner plan steps must match command plan" in diagnostics
    assert "provider-a-proof" not in diagnostics


def test_plan_json_nested_shapes_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["bad\nfield"] = "runtime-only-key-material"
    rendered["schema"] = "sorafs\nreputation"
    rendered["verifier_summary_schema"] = "summary\nschema"
    rendered["external_evidence"] = {
        "metrics": "bad\npath",
        "publish": ["publish.json"],
        "unknown_kind": "unknown.json",
        "bad\nkind": "metrics.json",
        "private_key": "runtime-only-key-material",
    }
    rendered["evidence_contract"] = {
        "metrics": {
            "schema": "wrong.schema.v1",
            "required_payload_fields": ["schema", "schema", "bad\nfield"],
            "raw_payload": True,
            "bad\nfield": "runtime-only-key-material",
        },
        "unknown_kind": {
            "schema": "sorafs.reputation.unknown.v1",
            "required_payload_fields": [],
        },
        "bad\nkind": "contract-shaped-entry",
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert "reputation rollout runner plan fields must be canonical strings" in diagnostics
    assert "reputation rollout runner plan schema must be canonical" in diagnostics
    assert (
        "reputation rollout runner plan verifier schema must be canonical"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan external_evidence keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan external_evidence keys must use known kind names"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan external_evidence must contain only configured evidence fields"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan external_evidence values must be canonical strings"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan external_evidence must match configured fields"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan evidence_contract keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan evidence_contract keys must use known kind names"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan evidence_contract must map each kind to a contract object"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan evidence_contract fields must be canonical strings"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan evidence_contract fields must be schema and required_payload_fields"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan evidence_contract schemas must match evidence kind"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan evidence_contract required_payload_fields must be non-empty lists"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan evidence_contract required_payload_fields must contain canonical strings"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan evidence_contract required_payload_fields must not contain duplicate fields"
        in diagnostics
    )
    assert (
        "reputation rollout runner plan evidence_contract required_payload_fields must match checker fields"
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
        return ["reputation rollout runner plan schema must match the contract"]

    def fake_run_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "validate_plan_json", fake_validate_plan_json)
    monkeypatch.setattr(MODULE, "run_plan", fake_run_plan)

    assert MODULE.main(complete_args(tmp_path)) == 2

    assert not ran_plan
    assert (
        "reputation rollout runner plan schema must match the contract"
        in capsys.readouterr().err
    )


def test_response_file_dry_run_prints_complete_reputation_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_args_file(tmp_path / "reputation-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "publish_snapshot"
    assert plan["steps"][5]["label"] == "rollout_evidence_gate"
    assert "events" in plan["evidence_contract"]


def test_missing_provider_proof_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    provider_id = "provider-a-private-key-placeholder"
    provider_index = args.index("--provider-id") + 1
    args[provider_index] = provider_id
    proof_index = args.index("--provider-proof")
    del args[proof_index : proof_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing --provider-proof for requested provider" in captured.err
    assert provider_id not in captured.err
    assert captured.out == ""


def test_duplicate_provider_id_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    provider_id = "provider-a-private-key-placeholder"
    first_provider_index = args.index("--provider-id") + 1
    first_proof_index = args.index("--provider-proof") + 1
    args[first_provider_index] = provider_id
    args[first_proof_index] = f"{provider_id}={tmp_path / 'payloads' / 'provider-a-proof.to'}"
    args.extend(["--provider-id", provider_id])

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "duplicate --provider-id" in captured.err
    assert provider_id not in captured.err
    assert captured.out == ""


def test_extra_provider_proof_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    provider_id = "provider-b-private-key-placeholder"
    proof_path = write_payload(tmp_path / "payloads" / "provider-b-proof.to")
    args.extend(["--provider-proof", f"{provider_id}={proof_path}"])

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "--provider-proof supplied for unrequested provider" in captured.err
    assert provider_id not in captured.err
    assert captured.out == ""


def test_duplicate_provider_proof_does_not_echo_provider_id(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    provider_id = "provider-a-private-key-placeholder"
    provider_index = args.index("--provider-id") + 1
    first_proof_index = args.index("--provider-proof") + 1
    proof_path = tmp_path / "payloads" / "provider-a-proof.to"
    args[provider_index] = provider_id
    args[first_proof_index] = f"{provider_id}={proof_path}"
    args.extend(["--provider-proof", f"{provider_id}={proof_path}"])

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "duplicate --provider-proof" in captured.err
    assert provider_id not in captured.err
    assert captured.out == ""


def test_malformed_provider_proof_sanitizes_exception_text(
    tmp_path: Path, monkeypatch
) -> None:
    bad_message = "provider-a\nshadow-proof"

    def raise_malformed_provider_proof(_spec: str):
        raise ValueError(bad_message)

    monkeypatch.setattr(MODULE, "split_provider_proof_spec", raise_malformed_provider_proof)
    errors = MODULE.validate_inputs(MODULE.parse_args(complete_args(tmp_path)))

    assert "<non-canonical-error>" in errors
    assert bad_message not in "\n".join(errors)


def test_malformed_provider_proof_does_not_echo_spec(tmp_path: Path) -> None:
    args = complete_args(tmp_path)
    bad_spec = "provider-a-private-key-placeholder"
    proof_index = args.index("--provider-proof") + 1
    args[proof_index] = bad_spec

    errors = MODULE.validate_inputs(MODULE.parse_args(args))

    diagnostics = "\n".join(errors)
    assert "--provider-proof must use PROVIDER_ID=PATH form" in diagnostics
    assert bad_spec not in diagnostics


def test_provider_proof_rejects_padded_or_unicode_components_without_trimming(
    tmp_path: Path,
) -> None:
    parsed_args = MODULE.parse_args(complete_args(tmp_path))
    proof_path = tmp_path / "payloads" / "provider-a-proof.to"
    cases = (
        f" provider-a={proof_path}",
        f"provider-a={proof_path} ",
        f"provider-a\u200d={proof_path}",
        f"provider-a={proof_path}\u202e",
    )

    for spec in cases:
        parsed_args.provider_proof = [spec]
        errors = MODULE.validate_inputs(parsed_args)
        diagnostics = "\n".join(errors)
        escaped_spec = spec.encode("unicode_escape").decode("ascii")

        assert "--provider-proof must use PROVIDER_ID=PATH form" in diagnostics
        assert spec not in diagnostics
        assert escaped_spec not in diagnostics


def test_provider_id_rejects_padded_or_unicode_values_before_plan(
    tmp_path: Path,
) -> None:
    parsed_args = MODULE.parse_args(complete_args(tmp_path))

    for provider_id in (" provider-a", "provider-a ", "provider-a\u200d", "provider-a\u202e"):
        parsed_args.provider_id = [provider_id]
        errors = MODULE.validate_inputs(parsed_args)
        diagnostics = "\n".join(errors)
        escaped_provider_id = provider_id.encode("unicode_escape").decode("ascii")

        assert "--provider-id must be canonical" in diagnostics
        assert provider_id not in diagnostics
        assert escaped_provider_id not in diagnostics


def test_missing_external_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-metrics.json"
    metrics_index = args.index("--metrics-evidence") + 1
    args[metrics_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert "--metrics-evidence" not in captured.err
    assert str(missing) not in captured.err
    assert captured.out == ""


def test_torii_url_rejects_secret_bearing_url_without_leaking(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    url = "https://user:private_key@torii.example/path?token=secret"
    url_index = args.index("--torii-url") + 1
    args[url_index] = url

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "SoraFS runner URL arguments must not contain" in captured.err
    assert "private_key" not in captured.err
    assert "token=secret" not in captured.err
    assert captured.out == ""


def test_torii_url_rejects_encoded_host_tokens_without_leaking(
    tmp_path: Path, capsys
) -> None:
    unsafe_urls = (
        "https://C%3A.torii.example/status",
        "https://http%3A.torii.example/status",
    )

    for index, unsafe_url in enumerate(unsafe_urls):
        case_dir = tmp_path / f"url-case-{index}"
        case_dir.mkdir()
        args = complete_args(case_dir)
        args[args.index("--torii-url") + 1] = unsafe_url

        assert MODULE.main([*args, "--dry-run"]) == 2

        captured = capsys.readouterr()
        assert "SoraFS runner URL arguments must not contain" in captured.err
        assert unsafe_url not in captured.err
        assert "C%3A" not in captured.err
        assert "http%3A" not in captured.err
        assert captured.out == ""


def test_sorafs_cli_bin_rejects_secret_bearing_path_without_leaking(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    args[args.index("--sorafs-cli-bin") + 1] = "/runtime/private_key/sorafs_cli"

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "SoraFS runner passthrough arguments must not contain" in captured.err
    assert "private_key" not in captured.err
    assert "/runtime/private_key" not in captured.err
    assert captured.out == ""
