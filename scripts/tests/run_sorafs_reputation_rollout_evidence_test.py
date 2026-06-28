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
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.reputation.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.reputation.rollout_evidence_gate.v1"
    assert plan["evidence_contract"]["publish"]["schema"] is None
    assert (
        "generated_at_unix"
        in plan["evidence_contract"]["latest"]["required_payload_fields"]
    )
    assert (
        "provider"
        in plan["evidence_contract"]["provider"]["required_payload_fields"]
    )
    assert (
        "proof_verified"
        in plan["evidence_contract"]["verify"]["required_payload_fields"]
    )
    assert plan["evidence_contract"]["metrics"]["schema"] == (
        "sorafs.reputation.metrics_canary.v1"
    )
    assert (
        "sse_connected"
        in plan["evidence_contract"]["transport"]["required_payload_fields"]
    )
    assert (
        "routing_score_consumed"
        in plan["evidence_contract"]["consumption"]["required_payload_fields"]
    )
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
    proof_index = args.index("--provider-proof")
    del args[proof_index : proof_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing --provider-proof" in captured.err
    assert captured.out == ""


def test_duplicate_provider_id_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    args.extend(["--provider-id", "provider-a"])

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "duplicate --provider-id `provider-a`" in captured.err
    assert captured.out == ""


def test_extra_provider_proof_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    proof_path = write_payload(tmp_path / "payloads" / "provider-b-proof.to")
    args.extend(["--provider-proof", f"provider-b={proof_path}"])

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "unrequested provider `provider-b`" in captured.err
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


def test_missing_external_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-metrics.json"
    metrics_index = args.index("--metrics-evidence") + 1
    args[metrics_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "--metrics-evidence" in captured.err
    assert str(missing) in captured.err
    assert captured.out == ""
