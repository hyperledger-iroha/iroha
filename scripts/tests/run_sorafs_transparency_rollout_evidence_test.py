"""Tests for scripts/run_sorafs_transparency_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_transparency_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_transparency_rollout_evidence", MODULE_PATH
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
    args = [
        "--iroha-bin",
        "/usr/local/bin/iroha",
        "--iroha-arg",
        "--config",
        "--iroha-arg",
        "/runtime/client.toml",
        "--torii-url",
        "https://torii.example",
        "--deployment-id",
        "transparency-staging-a",
        "--environment",
        "staging",
        "--out-dir",
        str(tmp_path / "evidence"),
        "--cycle-id",
        "11" * 16,
        "--limit",
        "7",
        "--timeout-secs",
        "5",
        "--privacy-source-event",
        str(write_payload(payload_dir / "privacy-source-event.json")),
        "--privacy-publish-due",
        str(write_payload(payload_dir / "privacy-publish-due.json")),
        "--proof-token-issuance",
        str(write_payload(payload_dir / "proof-token-issuance.json")),
    ]
    for source_kind in MODULE.DEFAULT_REQUIRED_SOURCE_KINDS:
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
    lines = [
        "# one token per line also works for long reviewed inputs",
        *args,
    ]
    path.write_text("\n".join(lines), encoding="utf-8")
    return path


def test_dry_run_prints_complete_rollout_plan(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.transparency.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.transparency.rollout_evidence_gate.v1"
    assert plan["deployment_context"] == {
        "deployment_id": "transparency-staging-a",
        "environment": "staging",
        "deployment_context_reviewed": True,
    }
    assert plan["evidence_contract"]["source_entry"]["schema"] == (
        "sorafs.transparency.source_entry.canary.v1"
    )
    assert (
        "source_entry_probe_count"
        in plan["evidence_contract"]["source_entry"]["required_payload_fields"]
    )
    assert (
        "publisher_identity_required"
        in plan["evidence_contract"]["publication"]["required_payload_fields"]
    )
    assert (
        "publish_due_probe_count"
        in plan["evidence_contract"]["privacy_aggregate"]["required_payload_fields"]
    )
    assert (
        "proof_token_frames_included"
        in plan["evidence_contract"]["proof_token_issuance"]["required_payload_fields"]
    )
    assert (
        "routes"
        in plan["evidence_contract"]["explorer"]["required_payload_fields"]
    )
    labels = [step["label"] for step in plan["steps"]]
    assert labels == [
        "source_entry_canary",
        "privacy_aggregate_canary",
        "proof_token_issuance_canary",
        "publication_canary",
        "explorer_canary",
        "rollout_evidence_gate",
    ]
    publication = plan["steps"][3]["command"]
    assert publication[:4] == ["/usr/local/bin/iroha", "--config", "/runtime/client.toml", "sorafs"]
    assert "publication-canary" in publication
    assert "--cycle-id" in publication
    assert "--torii-url" in publication
    verifier = plan["steps"][5]["command"]
    assert "check_sorafs_transparency_rollout_evidence.py" in verifier[1]


def test_response_file_dry_run_prints_complete_rollout_plan(tmp_path: Path, capsys) -> None:
    args_file = write_args_file(tmp_path / "rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    publication = plan["steps"][3]["command"]
    assert publication[:4] == ["/usr/local/bin/iroha", "--config", "/runtime/client.toml", "sorafs"]
    assert "publication-canary" in publication
    assert "source_entry" in plan["evidence_contract"]


def test_split_response_file_dry_run_prints_complete_rollout_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(tmp_path / "split-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "source_entry_canary"
    assert plan["steps"][5]["label"] == "rollout_evidence_gate"


def test_missing_source_kind_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    source_index = args.index("--source-entry")
    del args[source_index : source_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing --source-entry coverage" in captured.err
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


def test_generated_artifact_read_error_is_sanitized(
    tmp_path: Path,
    monkeypatch,
) -> None:
    artifact = write_payload(tmp_path / "artifact.json")
    bad_message = "json\nsecret"

    def load_raises(_path: Path, _max_bytes: int):
        raise ValueError(bad_message)

    monkeypatch.setattr(MODULE, "load_evidence_json", load_raises)

    errors = MODULE.annotate_evidence_artifact(
        artifact,
        deployment_id="transparency-staging-a",
        environment="staging",
    )

    assert errors == [
        f"failed to read generated evidence artifact `{artifact}`: "
        "<non-canonical-error>"
    ]
    assert bad_message not in "\n".join(errors)


def test_generated_artifact_annotation_marks_reviewed_context(tmp_path: Path) -> None:
    artifact = write_payload(tmp_path / "artifact.json")

    errors = MODULE.annotate_evidence_artifact(
        artifact,
        deployment_id="transparency-staging-a",
        environment="staging",
    )

    assert errors == []
    payload = json.loads(artifact.read_text(encoding="utf-8"))
    assert payload["deployment_id"] == "transparency-staging-a"
    assert payload["environment"] == "staging"
    assert payload["deployment_context_reviewed"] is True


def test_deployment_context_write_error_is_sanitized(
    tmp_path: Path,
    monkeypatch,
) -> None:
    artifact = write_payload(tmp_path / "artifact.json")
    original_write_text = Path.write_text
    bad_message = "write denied\nsecret"

    def write_text_raises(path: Path, *args, **kwargs):
        if path == artifact:
            raise OSError(bad_message)
        return original_write_text(path, *args, **kwargs)

    monkeypatch.setattr(Path, "write_text", write_text_raises)

    errors = MODULE.annotate_evidence_artifact(
        artifact,
        deployment_id="transparency-staging-a",
        environment="staging",
    )

    assert errors == [
        f"failed to write deployment context into `{artifact}`: "
        "<non-canonical-error>"
    ]
    assert bad_message not in "\n".join(errors)


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing.json"
    proof_index = args.index("--proof-token-issuance") + 1
    args[proof_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "--proof-token-issuance" in captured.err
    assert "must exist and be a file" in captured.err
    assert str(missing) in captured.err


def test_unreviewed_deployment_context_fails_before_plan(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    args[args.index("--deployment-id") + 1] = "transparency-dev-a"
    args[args.index("--environment") + 1] = "dev"

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert (
        "deployment_id must not contain non-reviewed deployment markers ['dev']"
        in captured.err
    )
    assert "environment must be one of" in captured.err


def test_cycle_id_is_required_for_publication_detail(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    cycle_index = args.index("--cycle-id")
    del args[cycle_index : cycle_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    assert "at least one --cycle-id" in capsys.readouterr().err
