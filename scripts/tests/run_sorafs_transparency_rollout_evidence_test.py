"""Tests for scripts/run_sorafs_transparency_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import os
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

from sorafs_rollout_runner_test_support import (  # noqa: E402
    write_topology_qualification,
)

SOURCE_KINDS = (
    "gar-enforcement-receipt",
    "moderation-ballot-governance-event",
    "appeal-finance-report",
    "appeal-finance-settlement-receipt",
    "legal-hold-notice",
    "redaction-notice",
    "evidence-access-summary",
)


def write_payload(path: Path) -> Path:
    path.write_text("{}", encoding="utf-8")
    return path


def write_source_producer_evidence(path: Path) -> Path:
    producers = [
        {
            "source_kind": source_kind,
            "producer_id": f"transparency-{index}",
            "producer_route": f"internal:transparency/{index}",
            "provenance_digest_hex": f"{index + 1:x}" * 64,
            "durable_checkpoint_verified": True,
        }
        for index, source_kind in enumerate(SOURCE_KINDS)
    ]
    path.write_text(
        json.dumps(
            {
                "schema": "sorafs.transparency.source_entry.producer_evidence.v1",
                "status": "passed",
                "generated_at_unix": 1_800_400_000,
                "deployment_id": "transparency-production-a",
                "environment": "production",
                "deployment_context_reviewed": True,
                "source_batch_digest_hex": "a" * 64,
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
    payload_dir.mkdir()
    args = [
        "--iroha-bin",
        "/usr/local/bin/iroha",
        "--iroha-arg=--config",
        "--iroha-arg=/runtime/client.toml",
        "--torii-url",
        "https://torii.example",
        "--deployment-id",
        "transparency-production-a",
        "--environment",
        "production",
        "--out-dir",
        str(tmp_path / "evidence"),
        "--now-unix",
        "1800400000",
        "--topology-qualification-summary",
        str(
            write_topology_qualification(
                tmp_path / "topology-qualification.json",
                deployment_id="transparency-production-a",
            )
        ),
        "--max-evidence-age-secs",
        "604800",
        "--cycle-id",
        "11" * 16,
        "--limit",
        "7",
        "--timeout-secs",
        "5",
        "--source-entry-producer-evidence",
        str(write_source_producer_evidence(payload_dir / "source-producers.json")),
        "--privacy-source-event",
        str(write_payload(payload_dir / "privacy-source-event.json")),
        "--privacy-publish-due",
        str(write_payload(payload_dir / "privacy-publish-due.json")),
        "--proof-token-issuance",
        str(write_payload(payload_dir / "proof-token-issuance.json")),
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
        "deployment_id": "transparency-production-a",
        "environment": "production",
        "deployment_context_reviewed": True,
    }
    assert plan["evidence_contract"]["source_entry"]["schema"] == (
        "sorafs.transparency.source_entry.producer_evidence.v1"
    )
    assert (
        "generic_public_ingress_absent"
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
        "privacy_aggregate_canary",
        "proof_token_issuance_canary",
        "publication_canary",
        "explorer_canary",
        "rollout_evidence_gate",
    ]
    publication = plan["steps"][2]["command"]
    assert publication[:4] == ["/usr/local/bin/iroha", "--config", "/runtime/client.toml", "sorafs"]
    assert "publication-canary" in publication
    assert "--cycle-id" in publication
    assert "--torii-url" in publication
    verifier = plan["steps"][4]["command"]
    assert "check_sorafs_transparency_rollout_evidence.py" in verifier[1]
    assert "--now-unix" in verifier
    assert "1800400000" in verifier
    assert "--max-evidence-age-secs" in verifier
    assert "604800" in verifier
    assert "--evidence" in verifier
    assert any(value.endswith("source-producers.json") for value in verifier)


def test_plan_json_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    assert MODULE.validate_plan_json(rendered, plan, args) == []

    assert MODULE.validate_plan_json(["step"], plan, args) == [
        "transparency rollout runner plan must be an object"
    ]

    rendered["schema"] = "sorafs.transparency.rollout_evidence_collection_plan.v0"
    rendered["unexpected"] = True
    rendered["evidence_contract"] = {}
    rendered["steps"] = []

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "transparency rollout runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan schema must match the contract"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan evidence_contract must match checker fields"
        in diagnostics
    )
    assert "runner plan steps must match command plan" in diagnostics


def test_plan_json_nested_shapes_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["bad\nfield"] = "runtime-only-key-material"
    rendered["schema"] = "sorafs\ntransparency"
    rendered["verifier_summary_schema"] = "summary\nschema"
    rendered["deployment_context"] = {
        "deployment_id": "bad\ndeployment",
        "environment": False,
        "deployment_context_reviewed": False,
        "bad\nfield": "runtime-only-key-material",
        "private_key": "runtime-only-key-material",
    }
    rendered["evidence_contract"] = {
        "source_entry": {
            "schema": "wrong.schema.v1",
            "required_payload_fields": ["schema", "schema", "bad\nfield"],
            "raw_payload": True,
            "bad\nfield": "runtime-only-key-material",
        },
        "unknown_kind": {
            "schema": "sorafs.transparency.unknown.v1",
            "required_payload_fields": [],
        },
        "bad\nkind": "contract-shaped-entry",
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert "transparency rollout runner plan fields must be canonical strings" in diagnostics
    assert "transparency rollout runner plan schema must be canonical" in diagnostics
    assert (
        "transparency rollout runner plan verifier schema must be canonical"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan deployment_context keys must be canonical strings"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan deployment_context fields must match configured fields"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan deployment_context values must be canonical strings"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan deployment_context must be reviewed"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan deployment_context must match args"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan evidence_contract keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan evidence_contract keys must use known kind names"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan evidence_contract must map each kind to a contract object"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan evidence_contract fields must be canonical strings"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan evidence_contract fields must be schema and required_payload_fields"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan evidence_contract schemas must match evidence kind"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan evidence_contract required_payload_fields must be non-empty lists"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan evidence_contract required_payload_fields must contain canonical strings"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan evidence_contract required_payload_fields must not contain duplicate fields"
        in diagnostics
    )
    assert (
        "transparency rollout runner plan evidence_contract required_payload_fields must match checker fields"
        in diagnostics
    )
    assert "unknown_kind" not in diagnostics
    assert "bad\ndeployment" not in diagnostics
    assert "bad\nkind" not in diagnostics
    assert "bad\nfield" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "private_key" not in diagnostics
    assert "wrong.schema.v1" not in diagnostics


def test_plan_json_deployment_context_must_stay_reviewed(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["deployment_context"] = {
        "deployment_id": "transparency-dev-a",
        "environment": "dev",
        "deployment_context_reviewed": True,
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert (
        "transparency rollout runner plan deployment_context must match args"
        in errors
    )
    assert "transparency-dev-a" not in "\n".join(errors)

    case_dir = tmp_path / "invalid-context"
    case_dir.mkdir()
    args = MODULE.parse_args(complete_args(case_dir))
    args.deployment_id = "transparency-dev-a"
    args.environment = "dev"
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "deployment_id must not contain non-reviewed deployment markers ['dev']"
        in diagnostics
    )
    assert "environment must be one of" in diagnostics
    assert "transparency-dev-a" not in diagnostics


def test_execution_rejects_plan_validation_drift_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_validate_plan_json(rendered, plan, args):
        return ["transparency rollout runner plan schema must match the contract"]

    def fake_run_plan(plan, out_dir, args):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "validate_plan_json", fake_validate_plan_json)
    monkeypatch.setattr(MODULE, "run_plan", fake_run_plan)

    assert MODULE.main(complete_args(tmp_path)) == 2

    assert not ran_plan
    assert (
        "transparency rollout runner plan schema must match the contract"
        in capsys.readouterr().err
    )


def test_response_file_dry_run_prints_complete_rollout_plan(tmp_path: Path, capsys) -> None:
    args_file = write_args_file(tmp_path / "rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    publication = plan["steps"][2]["command"]
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
    assert plan["steps"][0]["label"] == "privacy_aggregate_canary"
    assert plan["steps"][4]["label"] == "rollout_evidence_gate"


def test_missing_source_producer_evidence_fails_before_plan(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    source_index = args.index("--source-entry-producer-evidence") + 1
    args[source_index] = str(tmp_path / "missing-producer-evidence.json")

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert "missing-producer-evidence.json" not in captured.err
    assert captured.out == ""


def test_invalid_source_producer_evidence_fails_before_plan_without_leaking(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    path = Path(args[args.index("--source-entry-producer-evidence") + 1])
    path.write_text(
        json.dumps({"schema": "private-key-placeholder"}),
        encoding="utf-8",
    )

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert MODULE.SOURCE_PRODUCER_EVIDENCE_INVALID_DIAGNOSTIC in captured.err
    assert "private-key-placeholder" not in captured.err
    assert str(path) not in captured.err
    assert captured.out == ""


def test_source_producer_evidence_must_match_rollout_context(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    path = Path(args[args.index("--source-entry-producer-evidence") + 1])
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["deployment_id"] = "different-production"
    path.write_text(json.dumps(payload), encoding="utf-8")

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert MODULE.SOURCE_PRODUCER_EVIDENCE_CONTEXT_DIAGNOSTIC in captured.err
    assert "different-production" not in captured.err
    assert str(path) not in captured.err
    assert captured.out == ""


def test_source_producer_evidence_read_error_is_sanitized(
    tmp_path: Path, monkeypatch
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    bad_message = "transparency\nprivate-key-placeholder"

    def load_raises(_path: Path, _max_bytes: int):
        raise ValueError(bad_message)

    monkeypatch.setattr(MODULE, "load_evidence_json", load_raises)
    errors = MODULE.validate_source_entry_producer_evidence(args)

    assert errors == [MODULE.SOURCE_PRODUCER_EVIDENCE_READ_DIAGNOSTIC]
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


def test_source_producer_evidence_rejects_generic_public_ingress(
    tmp_path: Path,
) -> None:
    args = complete_args(tmp_path)
    path = Path(args[args.index("--source-entry-producer-evidence") + 1])
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["generic_public_ingress_absent"] = False
    payload["producers"][0]["producer_route"] = (
        "/v1/sorafs/transparency/source-entries/gar-enforcement-receipt"
    )
    path.write_text(json.dumps(payload), encoding="utf-8")

    errors = MODULE.validate_inputs(MODULE.parse_args(args))

    assert errors == [MODULE.SOURCE_PRODUCER_EVIDENCE_INVALID_DIAGNOSTIC]


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

    assert errors == ["generated evidence artifact cannot be read"]
    assert str(artifact) not in "\n".join(errors)
    assert bad_message not in "\n".join(errors)


def test_generated_artifact_context_conflict_does_not_echo_existing_value(
    tmp_path: Path,
) -> None:
    artifact = tmp_path / "artifact.json"
    existing_value = "deployment-private-key-placeholder"
    artifact.write_text(
        json.dumps({"deployment_id": existing_value}),
        encoding="utf-8",
    )

    errors = MODULE.annotate_evidence_artifact(
        artifact,
        deployment_id="transparency-staging-a",
        environment="staging",
    )

    diagnostics = "\n".join(errors)
    assert "has conflicting deployment context" in diagnostics
    assert existing_value not in diagnostics
    assert str(artifact) not in diagnostics


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


def test_deployment_context_write_uses_no_follow_descriptor_open(
    tmp_path: Path,
    monkeypatch,
) -> None:
    artifact = write_payload(tmp_path / "artifact.json")
    original_open = os.open
    opened: dict[str, int] = {}

    def open_path(path: Path, flags: int, *args, **kwargs):
        if path == artifact:
            opened["flags"] = flags
        return original_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(MODULE.os, "open", open_path)

    errors = MODULE.annotate_evidence_artifact(
        artifact,
        deployment_id="transparency-staging-a",
        environment="staging",
    )

    assert errors == []
    assert opened["flags"] & os.O_WRONLY
    assert opened["flags"] & os.O_TRUNC
    assert not opened["flags"] & os.O_CREAT
    if hasattr(os, "O_NOFOLLOW"):
        assert opened["flags"] & os.O_NOFOLLOW
    assert MODULE.deployment_context_write_open_flags() == opened["flags"]


def test_deployment_context_write_retries_short_os_write(
    tmp_path: Path,
    monkeypatch,
) -> None:
    artifact = write_payload(tmp_path / "artifact.json")
    original_write = os.write
    write_lengths: list[int] = []

    def short_write(fd: int, payload) -> int:
        chunk = bytes(payload[: max(1, min(5, len(payload)))])
        written = original_write(fd, chunk)
        write_lengths.append(written)
        return written

    monkeypatch.setattr(MODULE.os, "write", short_write)

    errors = MODULE.annotate_evidence_artifact(
        artifact,
        deployment_id="transparency-staging-a",
        environment="staging",
    )

    assert errors == []
    assert len(write_lengths) > 1
    payload = json.loads(artifact.read_text(encoding="utf-8"))
    assert payload["deployment_id"] == "transparency-staging-a"
    assert payload["environment"] == "staging"
    assert payload["deployment_context_reviewed"] is True


def test_deployment_context_write_fsyncs_descriptor_before_close(
    tmp_path: Path,
    monkeypatch,
) -> None:
    artifact = write_payload(tmp_path / "artifact.json")
    original_fsync = os.fsync
    fsynced: list[int] = []

    def fsync(fd: int) -> None:
        fsynced.append(fd)
        original_fsync(fd)

    monkeypatch.setattr(MODULE.os, "fsync", fsync)

    errors = MODULE.annotate_evidence_artifact(
        artifact,
        deployment_id="transparency-staging-a",
        environment="staging",
    )

    assert errors == []
    assert len(fsynced) == 2
    payload = json.loads(artifact.read_text(encoding="utf-8"))
    assert payload["deployment_context_reviewed"] is True


def test_deployment_context_parent_fsync_error_is_sanitized(
    tmp_path: Path,
    monkeypatch,
) -> None:
    artifact = write_payload(tmp_path / "bad\nartifact.json")
    bad_message = "parent fsync denied\nsecret"

    def fail_parent_sync(_path: Path, *, label: str) -> list[str]:
        assert label == "deployment-context artifact"
        return [bad_message]

    monkeypatch.setattr(MODULE, "load_evidence_json", lambda _path, _max_bytes: {})
    monkeypatch.setattr(MODULE, "fsync_checker_output_parent", fail_parent_sync)

    errors = MODULE.annotate_evidence_artifact(
        artifact,
        deployment_id="transparency-staging-a",
        environment="staging",
    )

    assert errors == ["deployment context cannot be written into generated artifact"]
    assert str(artifact) not in "\n".join(errors)
    assert bad_message not in "\n".join(errors)


def test_deployment_context_write_error_is_sanitized(
    tmp_path: Path,
    monkeypatch,
) -> None:
    artifact = write_payload(tmp_path / "artifact.json")
    original_open = os.open
    bad_message = "write denied\nsecret"

    def open_raises(path: Path, flags: int, *args, **kwargs):
        if path == artifact:
            raise OSError(bad_message)
        return original_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(MODULE, "load_evidence_json", lambda _path, _max_bytes: {})
    monkeypatch.setattr(MODULE.os, "open", open_raises)

    errors = MODULE.annotate_evidence_artifact(
        artifact,
        deployment_id="transparency-staging-a",
        environment="staging",
    )

    assert errors == ["deployment context cannot be written into generated artifact"]
    assert str(artifact) not in "\n".join(errors)
    assert bad_message not in "\n".join(errors)


def test_deployment_context_fsync_error_is_sanitized(
    tmp_path: Path,
    monkeypatch,
) -> None:
    artifact = write_payload(tmp_path / "artifact.json")
    bad_message = "fsync denied\nsecret"

    def fsync(_fd: int) -> None:
        raise OSError(bad_message)

    monkeypatch.setattr(MODULE, "load_evidence_json", lambda _path, _max_bytes: {})
    monkeypatch.setattr(MODULE.os, "fsync", fsync)

    errors = MODULE.annotate_evidence_artifact(
        artifact,
        deployment_id="transparency-staging-a",
        environment="staging",
    )

    assert errors == ["deployment context cannot be written into generated artifact"]
    assert str(artifact) not in "\n".join(errors)
    assert bad_message not in "\n".join(errors)


def test_deployment_context_write_rejects_symlink_swap_before_open(
    tmp_path: Path,
    monkeypatch,
) -> None:
    artifact = write_payload(tmp_path / "artifact.json")
    target = tmp_path / "target.json"
    target.write_text("old", encoding="utf-8")
    original_open = os.open
    bad_message = "symlink write denied\nsecret"

    def load_and_swap(path: Path, _max_bytes: int) -> dict:
        path.unlink()
        path.symlink_to(target)
        return {}

    def open_path(path: Path, flags: int, *args, **kwargs):
        if path == artifact:
            if hasattr(os, "O_NOFOLLOW"):
                assert flags & os.O_NOFOLLOW
            raise OSError(bad_message)
        return original_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(MODULE, "load_evidence_json", load_and_swap)
    monkeypatch.setattr(MODULE.os, "open", open_path)

    errors = MODULE.annotate_evidence_artifact(
        artifact,
        deployment_id="transparency-staging-a",
        environment="staging",
    )

    assert errors == ["deployment context cannot be written into generated artifact"]
    assert str(artifact) not in "\n".join(errors)
    assert target.read_text(encoding="utf-8") == "old"
    assert bad_message not in "\n".join(errors)


def test_deployment_context_write_rejects_parent_symlink_swap_before_open(
    tmp_path: Path,
    monkeypatch,
) -> None:
    parent = tmp_path / "artifact-parent"
    parent.mkdir()
    artifact = write_payload(parent / "artifact.json")
    old_parent = tmp_path / "old-parent"
    target_parent = tmp_path / "target-parent"
    target_parent.mkdir()
    (target_parent / "artifact.json").write_text("{}", encoding="utf-8")
    original_open = os.open

    def load_and_swap(_path: Path, _max_bytes: int) -> dict:
        parent.rename(old_parent)
        parent.symlink_to(target_parent, target_is_directory=True)
        return {}

    def open_path(path: Path, flags: int, *args, **kwargs):
        if path == artifact:
            raise AssertionError("parent-symlinked artifact must not be opened")
        return original_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(MODULE, "load_evidence_json", load_and_swap)
    monkeypatch.setattr(MODULE.os, "open", open_path)

    errors = MODULE.annotate_evidence_artifact(
        artifact,
        deployment_id="transparency-staging-a",
        environment="staging",
    )

    assert errors == ["deployment-context artifact parent is invalid"]
    assert str(parent) not in "\n".join(errors)
    assert (target_parent / "artifact.json").read_text(encoding="utf-8") == "{}"


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing.json"
    proof_index = args.index("--proof-token-issuance") + 1
    args[proof_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert "--proof-token-issuance" not in captured.err
    assert str(missing) not in captured.err


def test_torii_url_rejects_secret_bearing_url_without_leaking(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    url = "https://torii.example/path?bearer_token=secret"
    args[args.index("--torii-url") + 1] = url

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "SoraFS runner URL arguments must not contain" in captured.err
    assert "bearer_token" not in captured.err
    assert "bearer_token=secret" not in captured.err
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


def test_iroha_arg_rejects_secret_bearing_value_without_leaking(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    args.append("--iroha-arg=--private-key=/runtime/signing.key")

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "SoraFS runner passthrough arguments must not contain" in captured.err
    assert "private-key" not in captured.err
    assert "signing.key" not in captured.err
    assert captured.out == ""


def test_split_iroha_arg_form_fails_without_leaking_value(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    args.extend(["--iroha-arg", "--private-key=/runtime/signing.key"])

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert MODULE.IROHA_ARG_EQUALS_FORM_DIAGNOSTIC in captured.err
    assert "private-key" not in captured.err
    assert "signing.key" not in captured.err
    assert captured.out == ""


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


def test_environment_must_be_exact_reviewed_label_before_plan(
    tmp_path: Path, capsys
) -> None:
    args = complete_args(tmp_path)
    args[args.index("--environment") + 1] = "STAGING"

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "environment must be one of" in captured.err
    assert "STAGING" not in captured.err
    assert captured.out == ""


def test_cycle_id_is_required_for_publication_detail(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    cycle_index = args.index("--cycle-id")
    del args[cycle_index : cycle_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    assert "at least one --cycle-id" in capsys.readouterr().err


def test_cycle_id_must_be_lowercase_16_byte_hex(tmp_path: Path, capsys) -> None:
    bad_cycle_ids = [
        "AA" * 16,
        "11" * 15,
        "g" * 32,
        "private-key-placeholder",
    ]
    for index, bad_cycle_id in enumerate(bad_cycle_ids):
        case_dir = tmp_path / f"case-{index}"
        case_dir.mkdir()
        args = complete_args(case_dir)
        args[args.index("--cycle-id") + 1] = bad_cycle_id

        assert MODULE.main([*args, "--dry-run"]) == 2

        captured = capsys.readouterr()
        assert "--cycle-id must be a 16-byte lowercase hex string" in captured.err
        assert bad_cycle_id not in captured.err
        assert captured.out == ""
