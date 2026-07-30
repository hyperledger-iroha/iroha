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

from sorafs_rollout_runner_test_support import (  # noqa: E402
    write_topology_qualification,
)

AUTH_ACCOUNT = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"
INTEGRATION_NOW_UNIX = 1_800_400_000
INTEGRATION_GENERATED_AT = INTEGRATION_NOW_UNIX - 120
INTEGRATION_SNAPSHOT_ID = "11" * 16
INTEGRATION_MERKLE_ROOT = "22" * 32
INTEGRATION_WEIGHTS_DIGEST = "55" * 32
INTEGRATION_PROOF_SIBLING = "33" * 32
INTEGRATION_METRICS = (
    "sorafs_reputation_ingest_lag_seconds",
    "sorafs_reputation_snapshot_age_seconds",
    "sorafs_reputation_snapshot_generated_at_unix",
    "sorafs_reputation_provider_count",
    "sorafs_reputation_low_score_providers",
    "sorafs_reputation_score",
    "sorafs_reputation_threshold_crossings_total",
    "sorafs_reputation_iteration_count",
    "sorafs_reputation_penalty_applied_total",
)


def write_payload(path: Path) -> Path:
    path.write_text("{}", encoding="utf-8")
    return path


def write_auth_key(path: Path) -> Path:
    path.write_text("runtime-only-test-key\n", encoding="utf-8")
    path.chmod(0o600)
    return path


def integration_provider_inventory() -> list[dict[str, str]]:
    return [{"name": "provider-a"}, {"name": "provider-b"}]


def integration_common(kind: str) -> dict[str, object]:
    return {
        "schema": MODULE.KIND_BY_NAME[kind].schema,
        "generated_at_unix": INTEGRATION_GENERATED_AT,
        "deployment_id": "sorafs-mainnet-20260701",
        "environment": "production",
        "deployment_context_reviewed": True,
        "snapshot_id_hex": INTEGRATION_SNAPSHOT_ID,
        "merkle_root_hex": INTEGRATION_MERKLE_ROOT,
    }


def integration_publish_evidence() -> dict[str, object]:
    return {
        **integration_common("publish"),
        "status": "accepted",
        "weights_digest_hex": INTEGRATION_WEIGHTS_DIGEST,
        "provider_count": 2,
        "providers": integration_provider_inventory(),
    }


def integration_external_evidence(kind: str) -> dict[str, object]:
    payload = integration_common(kind)
    if kind == "metrics":
        payload.update(
            {
                "status": "passed",
                "metrics_scrape_success": True,
                "provider_count": 2,
                "providers": integration_provider_inventory(),
                "metrics": list(INTEGRATION_METRICS),
                "metric_count": len(INTEGRATION_METRICS),
                "snapshot_age_seconds": 120,
                "ingest_lag_seconds": 60,
                "response_bodies_included": False,
            }
        )
    elif kind == "transport":
        payload.update(
            {
                "status": "passed",
                "sse_connected": True,
                "sse_event_count": 1,
                "sse_events": [{"name": "reputation-sse-event-snapshot-00"}],
                "websocket_connected": True,
                "websocket_event_count": 1,
                "websocket_events": [
                    {"name": "reputation-websocket-event-snapshot-00"}
                ],
                "response_bodies_included": False,
            }
        )
    elif kind == "consumption":
        payload.update(
            {
                "status": "passed",
                "provider_count": 2,
                "providers": integration_provider_inventory(),
                "routing_score_consumed": True,
                "routing_weight_changed": True,
                "incentive_score_consumed": True,
                "raw_provider_records_included": False,
            }
        )
    else:  # pragma: no cover - helper contract
        raise ValueError("unsupported integration external evidence kind")
    return payload


def integration_raw_metrics() -> dict[str, int]:
    return {
        "version": 1,
        "por_success_bps": 9_500,
        "pdp_success_bps": 9_400,
        "potr_success_bps": 9_300,
        "latency_health_bps": 9_200,
        "dispute_rate_bps": 100,
        "token_violation_rate_bps": 50,
        "repair_breach_rate_bps": 25,
    }


def integration_raw_provider(provider_id: str, score_bps: int) -> dict[str, object]:
    return {
        "provider_id": provider_id,
        "score_bps": score_bps,
        "degradation_flags": [],
        "raw_metrics": integration_raw_metrics(),
        "raw_metrics_hash_hex": "66" * 32,
    }


def integration_cli_payloads(*, provider_root: str) -> dict[str, dict[str, object]]:
    return {
        "snapshot": {
            "snapshot_id_hex": INTEGRATION_SNAPSHOT_ID,
            "generated_at_unix": INTEGRATION_GENERATED_AT,
            "previous_snapshot_id_hex": None,
            "merkle_root_hex": INTEGRATION_MERKLE_ROOT,
            "provider_count": 2,
            "returned_provider_count": 2,
            "limit": 100,
            "truncated_providers": False,
            "alpha_bps": 1_500,
            "current_score_weight_bps": 7_500,
            "weights": {
                "version": 1,
                "por_success_bps": 2_200,
                "pdp_success_bps": 2_000,
                "potr_success_bps": 1_800,
                "latency_bps": 1_500,
                "dispute_bps": 1_000,
                "token_violation_bps": 500,
                "repair_breach_bps": 1_000,
            },
            "providers": [
                integration_raw_provider("provider-a", 9_400),
                integration_raw_provider("provider-b", 8_800),
            ],
        },
        "fetch": {
            "snapshot_id_hex": INTEGRATION_SNAPSHOT_ID,
            "generated_at_unix": INTEGRATION_GENERATED_AT,
            "merkle_root_hex": provider_root,
            "provider": integration_raw_provider("provider-a", 9_400),
            "proof": {
                "provider_id": "provider-a",
                "leaf_index": 0,
                "leaf_count": 2,
                "siblings_hex": [INTEGRATION_PROOF_SIBLING],
            },
        },
        "watch": {
            "since": 0,
            "limit": 7,
            "count": 1,
            "next_since": 1,
            "events": [
                {
                    "version": 1,
                    "sequence": 1,
                    "snapshot_id_hex": INTEGRATION_SNAPSHOT_ID,
                    "generated_at_unix": INTEGRATION_GENERATED_AT,
                    "merkle_root_hex": INTEGRATION_MERKLE_ROOT,
                    "provider_count": 2,
                    "previous_snapshot_id_hex": None,
                }
            ],
        },
        "verify": {
            "snapshot_path": "",
            "snapshot_id_hex": INTEGRATION_SNAPSHOT_ID,
            "generated_at_unix": INTEGRATION_GENERATED_AT,
            "provider_count": 2,
            "merkle_root_hex": INTEGRATION_MERKLE_ROOT,
            "alpha_bps": 1_500,
            "current_score_weight_bps": 7_500,
            "valid": True,
            "provider_id": "provider-a",
            "provider_score_bps": 9_400,
            "proof_path": "",
            "proof_leaf_index": 0,
            "proof_sibling_count": 1,
            "proof_verified": True,
        },
    }


def write_fake_sorafs_cli(
    path: Path,
    *,
    provider_root: str = INTEGRATION_MERKLE_ROOT,
) -> Path:
    payloads_json = json.dumps(
        integration_cli_payloads(provider_root=provider_root),
        sort_keys=True,
    )
    script = f"""#!/usr/bin/env python3
import json
import sys
from pathlib import Path

PAYLOADS = json.loads({payloads_json!r})

if len(sys.argv) < 3 or sys.argv[1] != "reputation":
    raise SystemExit(64)
operation = sys.argv[2]
arguments = {{}}
for raw in sys.argv[3:]:
    option, separator, value = raw.partition("=")
    if separator:
        arguments[option.removeprefix("--")] = value
if operation not in PAYLOADS:
    raise SystemExit(64)
payload = dict(PAYLOADS[operation])
if operation == "verify":
    payload["snapshot_path"] = arguments["snapshot"]
    payload["proof_path"] = arguments["proof"]
output = arguments.get("output", arguments.get("summary-out"))
if output is None:
    raise SystemExit(64)
Path(output).write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
"""
    path.write_text(script, encoding="utf-8")
    path.chmod(0o755)
    return path


def complete_args(tmp_path: Path) -> list[str]:
    payload_dir = tmp_path / "payloads"
    payload_dir.mkdir()
    return [
        "--sorafs-cli-bin",
        "/usr/local/bin/sorafs_cli",
        "--torii-url",
        "https://torii.example",
        "--auth-account",
        AUTH_ACCOUNT,
        "--auth-private-key-file",
        str(write_auth_key(payload_dir / "reputation-reader.key")),
        "--snapshot",
        str(write_payload(payload_dir / "reputation-snapshot.to")),
        "--publish-evidence",
        str(write_payload(payload_dir / "publish.json")),
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
        "--now-unix",
        "1800400000",
        "--topology-qualification-summary",
        str(
            write_topology_qualification(
                tmp_path / "topology-qualification.json",
                deployment_id="sorafs-mainnet-20260701",
            )
        ),
        "--max-snapshot-age-secs",
        "691200",
        "--max-ingest-lag-secs",
        "900",
        "--watch-limit",
        "7",
        "--watch-max-polls",
        "1",
        "--watch-poll-interval-ms",
        "0",
    ]


def integration_args(tmp_path: Path, cli: Path) -> list[str]:
    args = complete_args(tmp_path)
    args[args.index("--sorafs-cli-bin") + 1] = str(cli)
    args[args.index("--torii-url") + 1] = "http://127.0.0.1:8080"
    args[args.index("--now-unix") + 1] = str(INTEGRATION_NOW_UNIX)
    for option, payload in (
        ("--publish-evidence", integration_publish_evidence()),
        ("--metrics-evidence", integration_external_evidence("metrics")),
        ("--transport-evidence", integration_external_evidence("transport")),
        ("--consumption-evidence", integration_external_evidence("consumption")),
    ):
        Path(args[args.index(option) + 1]).write_text(
            json.dumps(payload, sort_keys=True),
            encoding="utf-8",
        )
    return args


def write_args_file(path: Path, args: list[str]) -> Path:
    lines = [
        "# comments and blank lines are ignored",
        "",
    ]
    for index in range(0, len(args), 2):
        option = args[index]
        value = args[index + 1]
        lines.append(f"{option} {json.dumps(value, ensure_ascii=False)}")
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
        "publish": args[args.index("--publish-evidence") + 1],
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
        "fetch_latest_snapshot",
        "adapt_latest_snapshot",
        "fetch_provider_provider-a",
        "adapt_provider_provider-a",
        "verify_provider_provider-a",
        "adapt_verify_provider_provider-a",
        "watch_reputation_events",
        "adapt_reputation_events",
        "rollout_evidence_gate",
    ]
    assert all(
        step["command"][:3] != ["/usr/local/bin/sorafs_cli", "reputation", "publish"]
        for step in plan["steps"]
    )
    fetch = plan["steps"][2]["command"]
    assert "--format=json" in fetch
    torii_url = "--torii-url=https://torii.example"
    auth_account = f"--auth-account={AUTH_ACCOUNT}"
    auth_key = MODULE.REDACTED_AUTH_PRIVATE_KEY_ARGUMENT
    private_key_path = args[args.index("--auth-private-key-file") + 1]
    for step_index in (0, 2, 6):
        command = plan["steps"][step_index]["command"]
        assert command.count(torii_url) == 1
        assert command.count(auth_account) == 1
        assert command.count(auth_key) == 1
        assert private_key_path not in " ".join(command)
    for step_index in (1, 3, 4, 5, 7, 8):
        command = plan["steps"][step_index]["command"]
        assert torii_url not in command
        assert auth_account not in command
        assert auth_key not in command
    verify = plan["steps"][4]["command"]
    assert "--provider-id=provider-a" in verify
    assert plan["steps"][0]["artifact"].endswith(".raw")
    assert plan["steps"][2]["artifact"].endswith(".raw")
    assert plan["steps"][4]["artifact"].endswith(".raw")
    assert plan["steps"][6]["artifact"].endswith(".raw")
    verifier = plan["steps"][8]["command"]
    assert "check_sorafs_reputation_rollout_evidence.py" in verifier[1]
    assert "--now-unix" in verifier
    assert "1800400000" in verifier
    assert "--max-snapshot-age-secs" in verifier
    assert "691200" in verifier
    assert "--max-ingest-lag-secs" in verifier
    assert "900" in verifier
    assert "--require-provider" in verifier
    assert "provider-a" in verifier
    assert (
        f"publish={args[args.index('--publish-evidence') + 1]}"
        in verifier
    )
    assert f"latest={Path(args[args.index('--out-dir') + 1]) / 'latest.json'}" in verifier
    assert any(item.startswith("provider=") for item in verifier)
    assert any(item.startswith("verify=") for item in verifier)
    assert f"events={Path(args[args.index('--out-dir') + 1]) / 'events.json'}" in verifier
    executable_plan = MODULE.build_command_plan(MODULE.parse_args(args))
    expected_private_key_argument = f"--auth-private-key-file={private_key_path}"
    for step_index in (0, 2, 6):
        command = executable_plan[step_index].command
        assert command.count(expected_private_key_argument) == 1
        assert MODULE.REDACTED_AUTH_PRIVATE_KEY_ARGUMENT not in command
    out_dir = Path(args[args.index("--out-dir") + 1])
    assert not out_dir.exists()


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
        "latest": "latest.json",
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
    assert plan["steps"][0]["label"] == "fetch_latest_snapshot"
    assert plan["steps"][8]["label"] == "rollout_evidence_gate"
    assert "events" in plan["evidence_contract"]


def test_response_file_rejects_duplicate_auth_options_without_values(
    tmp_path: Path,
    capsys,
) -> None:
    for index, option in enumerate(("--auth-account", "--auth-private-key-file")):
        case_dir = tmp_path / f"duplicate-auth-{index}"
        case_dir.mkdir()
        args_file = write_args_file(
            case_dir / "reputation-rollout.args",
            complete_args(case_dir),
        )
        duplicate_value = (
            "private-key-account-placeholder"
            if option == "--auth-account"
            else str(case_dir / "private-key-material.key")
        )
        args_file.write_text(
            f"{args_file.read_text(encoding='utf-8')}\n"
            f"{option} {json.dumps(duplicate_value)}\n",
            encoding="utf-8",
        )

        assert MODULE.main([f"@{args_file}", "--dry-run"]) == 2

        captured = capsys.readouterr()
        option_label = MODULE.OPTION_DIAGNOSTIC_LABELS.get(option, option)
        assert f"{option_label} must be supplied at most once" in captured.err
        assert duplicate_value not in captured.err
        assert captured.out == ""


def test_auth_option_prefix_abbreviations_fail_without_values(
    tmp_path: Path,
    capsys,
) -> None:
    for index, (option, abbreviated) in enumerate(
        (
            ("--auth-account", "--auth-acc"),
            ("--auth-private-key-file", "--auth-private-key-f"),
        )
    ):
        case_dir = tmp_path / f"abbreviated-auth-{index}"
        case_dir.mkdir()
        args = complete_args(case_dir)
        value = args[args.index(option) + 1]
        args[args.index(option)] = abbreviated

        assert MODULE.main([*args, "--dry-run"]) == 2

        captured = capsys.readouterr()
        assert "unsupported reputation rollout option" in captured.err
        assert value not in captured.err
        assert captured.out == ""


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


def test_provider_id_rejects_values_outside_exact_torii_grammar_before_plan(
    tmp_path: Path,
) -> None:
    parsed_args = MODULE.parse_args(complete_args(tmp_path))

    for provider_id in (
        " provider-a",
        "provider-a ",
        "provider-a\u200d",
        "provider-a\u202e",
        "provider/a",
        "provider%2Fa",
        "provider?query",
        "provider#fragment",
        "provider=alias",
        "provideré",
        ".",
        "..",
        "p" * 257,
    ):
        parsed_args.provider_id = [provider_id]
        errors = MODULE.validate_inputs(parsed_args)
        diagnostics = "\n".join(errors)
        escaped_provider_id = provider_id.encode("unicode_escape").decode("ascii")

        assert MODULE.REPUTATION_PROVIDER_ID_ERROR in diagnostics
        if provider_id not in {".", ".."}:
            assert provider_id not in diagnostics
            assert escaped_provider_id not in diagnostics


def test_provider_ids_preserve_exact_collision_free_sharded_artifact_paths() -> None:
    provider_ids = (
        "a",
        "aa",
        "provider-a",
        "provider_a",
        "provider.a",
        "provider:a",
        "provider..a",
        "A0",
        "p" * 256,
    )

    paths = [
        MODULE.provider_artifact_name(provider_id, "provider")
        for provider_id in provider_ids
    ]
    source_paths = [
        MODULE.provider_source_artifact_name(provider_id, "provider")
        for provider_id in provider_ids
    ]

    assert len(paths) == len(set(paths))
    assert len(source_paths) == len(set(source_paths))
    for provider_id, path, source_path in zip(provider_ids, paths, source_paths):
        assert path.parts[0] == "provider-by-provider-id"
        assert path.parts[-1] == "artifact.json"
        assert source_path.parent == path.parent
        assert source_path.name == "source.raw"
        hex_chunks = path.parts[1:-1]
        assert hex_chunks
        assert all(
            0 < len(chunk) <= MODULE.PROVIDER_ARTIFACT_HEX_CHUNK_CHARS
            and len(chunk) % 2 == 0
            and set(chunk) <= set("0123456789abcdef")
            for chunk in hex_chunks
        )
        assert all(
            len(chunk) == MODULE.PROVIDER_ARTIFACT_HEX_CHUNK_CHARS
            for chunk in hex_chunks[:-1]
        )
        assert bytes.fromhex("".join(hex_chunks)).decode("ascii") == provider_id
    assert all(MODULE.provider_id_is_canonical(provider_id) for provider_id in provider_ids)


def test_provider_artifact_suffix_is_schema_closed() -> None:
    try:
        MODULE.provider_artifact_name("provider-a", "../verify")
    except ValueError as error:
        assert str(error) == "provider artifact suffix must be provider or verify"
    else:  # pragma: no cover - defensive
        raise AssertionError("unsafe provider artifact suffix must be rejected")


def test_prepare_provider_artifact_parent_creates_only_planned_shards(
    tmp_path: Path,
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    step = plan[2]
    args.out_dir.mkdir()

    assert MODULE.prepare_reputation_artifact_parent(step, args.out_dir) == []
    assert step.artifact is not None
    assert step.artifact.parent.is_dir()
    assert not step.artifact.exists()
    assert not (args.out_dir / "verify-by-provider-id").exists()


def test_prepare_provider_artifact_parent_rejects_tampered_layout(
    tmp_path: Path,
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    step = plan[2]
    args.out_dir.mkdir()
    tampered = MODULE.CommandPlan(
        step.label,
        args.out_dir / "provider-by-provider-id" / ".." / "artifact.json",
        step.command,
    )

    assert MODULE.prepare_reputation_artifact_parent(tampered, args.out_dir) == [
        "provider artifact path must match its canonical sharded layout"
    ]
    assert list(args.out_dir.iterdir()) == []


def test_prepare_provider_artifact_parent_rejects_symlinked_namespace(
    tmp_path: Path,
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    step = plan[2]
    outside = tmp_path / "outside"
    outside.mkdir()
    args.out_dir.mkdir()
    namespace = args.out_dir / "provider-by-provider-id"
    namespace.symlink_to(outside, target_is_directory=True)

    errors = MODULE.prepare_reputation_artifact_parent(step, args.out_dir)

    assert any("must not be a symlink" in error for error in errors)
    assert list(outside.iterdir()) == []


def test_run_plan_installs_reputation_artifact_preparation_callback(
    tmp_path: Path,
    monkeypatch,
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    captured: dict[str, object] = {}

    def fake_run_command_plan(
        command_plan,
        out_dir,
        *,
        prepare_step,
        notice_command,
    ):
        captured["plan"] = command_plan
        captured["out_dir"] = out_dir
        captured["prepare_step"] = prepare_step
        captured["notice_command"] = notice_command
        return 17

    monkeypatch.setattr(MODULE, "run_command_plan", fake_run_command_plan)

    assert MODULE.run_plan(plan, args.out_dir) == 17
    assert captured["plan"] is plan
    assert captured["out_dir"] == args.out_dir
    assert callable(captured["prepare_step"])
    assert callable(captured["notice_command"])


def test_source_bound_runner_output_passes_full_reputation_gate(
    tmp_path: Path,
    capsys,
) -> None:
    cli = write_fake_sorafs_cli(tmp_path / "sorafs-cli-fixture")
    args = integration_args(tmp_path, cli)
    private_key_path = args[args.index("--auth-private-key-file") + 1]
    out_dir = Path(args[args.index("--out-dir") + 1])

    assert MODULE.main(args) == 0

    summary = json.loads(
        (out_dir / "rollout-summary.json").read_text(encoding="utf-8")
    )
    assert summary["status"] == "ready"
    assert summary["evidence_file_count"] == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert summary["recognized_artifact_count"] == len(
        MODULE.DEFAULT_REQUIRED_KINDS
    )
    assert summary["provider_ids"] == ["provider-a"]
    assert summary["valid_snapshot_bindings"] == [
        {
            "snapshot_id_hex": INTEGRATION_SNAPSHOT_ID,
            "merkle_root_hex": INTEGRATION_MERKLE_ROOT,
        }
    ]
    assert (out_dir / MODULE.LATEST_SOURCE_ARTIFACT_FILENAME).is_file()
    assert (out_dir / MODULE.EVENTS_SOURCE_ARTIFACT_FILENAME).is_file()
    assert (
        out_dir / MODULE.provider_source_artifact_name("provider-a", "provider")
    ).is_file()
    assert (
        out_dir / MODULE.provider_source_artifact_name("provider-a", "verify")
    ).is_file()
    assert all(path.suffix == ".raw" for path in out_dir.rglob("*.raw"))
    captured = capsys.readouterr()
    assert private_key_path not in captured.err
    assert MODULE.REDACTED_AUTH_PRIVATE_KEY_ARGUMENT in captured.err


def test_source_bound_runner_stops_before_gate_on_provider_anchor_mismatch(
    tmp_path: Path,
) -> None:
    cli = write_fake_sorafs_cli(
        tmp_path / "sorafs-cli-fixture",
        provider_root="77" * 32,
    )
    args = integration_args(tmp_path, cli)
    out_dir = Path(args[args.index("--out-dir") + 1])

    assert MODULE.main(args) == 2

    provider_artifact = out_dir / MODULE.provider_artifact_name(
        "provider-a",
        "provider",
    )
    provider_source = out_dir / MODULE.provider_source_artifact_name(
        "provider-a",
        "provider",
    )
    assert provider_source.is_file()
    assert not provider_artifact.exists()
    assert not (out_dir / "rollout-summary.json").exists()


def test_provider_proof_rejects_provider_id_outside_torii_grammar() -> None:
    for spec in (
        "provider/a=/runtime/proof.to",
        "provider%2Fa=/runtime/proof.to",
        "provideré=/runtime/proof.to",
        f"{'p' * 257}=/runtime/proof.to",
    ):
        try:
            MODULE.split_provider_proof_spec(spec)
        except ValueError as error:
            assert str(error) == "--provider-proof must use PROVIDER_ID=PATH form"
        else:  # pragma: no cover - defensive
            raise AssertionError("invalid provider id must be rejected")


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


def test_missing_auth_signing_file_fails_before_plan_without_leaking(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-reader.key"
    args[args.index("--auth-private-key-file") + 1] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert str(missing) not in captured.err
    assert captured.out == ""


def test_auth_account_rejects_alias_and_sensitive_values_without_leaking(
    tmp_path: Path, capsys
) -> None:
    for index, (account, expected_diagnostic) in enumerate(
        (
            ("merchant@paynet", MODULE.AUTH_ACCOUNT_ERROR),
            (
                "private_key_account_placeholder",
                "SoraFS runner passthrough arguments must not contain",
            ),
        )
    ):
        case_dir = tmp_path / f"auth-account-{index}"
        case_dir.mkdir()
        args = complete_args(case_dir)
        args[args.index("--auth-account") + 1] = account

        assert MODULE.main([*args, "--dry-run"]) == 2

        captured = capsys.readouterr()
        assert expected_diagnostic in captured.err
        assert account not in captured.err
        assert captured.out == ""


def test_auth_account_candidate_shape_tracks_i105_sentinels_and_alphabet() -> None:
    payload = "1234567"
    for account in (
        f"sora{payload}",
        f"test{payload}",
        f"dev{payload}",
        f"n42{payload}",
        AUTH_ACCOUNT,
    ):
        assert MODULE.auth_account_is_i105_candidate(account)

    for account in (
        "",
        "merchant@paynet",
        f"n0{payload}",
        f"n00042{payload}",
        "sora1234560",
        "sora123456",
    ):
        assert not MODULE.auth_account_is_i105_candidate(account)


def test_invalid_freshness_args_fail_before_plan(tmp_path: Path, capsys) -> None:
    cases = (
        ("--now-unix", "0"),
        ("--max-snapshot-age-secs", "0"),
        ("--max-ingest-lag-secs", "0"),
    )

    for option, value in cases:
        case_dir = tmp_path / option.removeprefix("--").replace("-", "_")
        case_dir.mkdir()
        args = complete_args(case_dir)
        args[args.index(option) + 1] = value

        assert MODULE.main([*args, "--dry-run"]) == 2

        captured = capsys.readouterr()
        assert "must be positive" in captured.err
        assert captured.out == ""


def test_watch_bounds_match_cli_and_torii_before_plan(
    tmp_path: Path,
    capsys,
) -> None:
    cases = (
        ("--watch-since", str(1 << 64), "unsigned 64-bit"),
        ("--watch-limit", "501", "within 1..=500"),
        ("--watch-max-polls", str(1 << 64), "unsigned 64-bit"),
        (
            "--watch-max-polls",
            "2",
            "must be 1 so the archived source binds the exact watch request",
        ),
        ("--watch-poll-interval-ms", str(1 << 64), "unsigned 64-bit"),
    )

    for index, (option, value, diagnostic) in enumerate(cases):
        case_dir = tmp_path / f"watch-bound-{index}"
        case_dir.mkdir()
        args = complete_args(case_dir)
        if option in args:
            args[args.index(option) + 1] = value
        else:
            args.extend([option, value])

        assert MODULE.main([*args, "--dry-run"]) == 2

        captured = capsys.readouterr()
        assert diagnostic in captured.err
        assert value not in captured.err
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
    assert (
        "SoraFS runner service origins must be exact canonical bare HTTPS"
        in captured.err
    )
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
        assert (
            "SoraFS runner service origins must be exact canonical bare HTTPS"
            in captured.err
        )
        assert unsafe_url not in captured.err
        assert "C%3A" not in captured.err
        assert "http%3A" not in captured.err
        assert captured.out == ""


def test_torii_url_preflight_matches_reputation_cli_origin_profile(
    tmp_path: Path,
    capsys,
) -> None:
    accepted = (
        "https://torii.example",
        "https://torii.example/",
        "https://torii.example:8443",
        "http://localhost:8080",
        "http://127.0.0.1:8080/",
        "http://[::1]:8080",
    )
    rejected = (
        "http://torii.example",
        "https://torii.example/path",
        "https://torii.example/?query=1",
        "https://torii.example/#fragment",
        "https://user@torii.example",
        "https://torii.example:0",
        "https://%74orii.example",
        "https://127.000.000.001",
        "HTTPS://torii.example",
    )

    for index, torii_url in enumerate(accepted):
        case_dir = tmp_path / f"accepted-origin-{index}"
        case_dir.mkdir()
        args = complete_args(case_dir)
        args[args.index("--torii-url") + 1] = torii_url

        assert MODULE.main([*args, "--dry-run"]) == 0
        assert json.loads(capsys.readouterr().out)["steps"]

    for index, torii_url in enumerate(rejected):
        case_dir = tmp_path / f"rejected-origin-{index}"
        case_dir.mkdir()
        args = complete_args(case_dir)
        args[args.index("--torii-url") + 1] = torii_url

        assert MODULE.main([*args, "--dry-run"]) == 2
        captured = capsys.readouterr()
        assert (
            "SoraFS runner service origins must be exact canonical bare HTTPS"
            in captured.err
        )
        assert torii_url not in captured.err
        assert captured.out == ""
        assert not Path(args[args.index("--out-dir") + 1]).exists()


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
