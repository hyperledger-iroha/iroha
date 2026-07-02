"""Tests for scripts/build_sorafs_ai_prescreen_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_ai_prescreen_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_ai_prescreen_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_ai_prescreen_canary",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_ai_prescreen_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


DIGEST = "a" * 64
POLICY_DIGEST = "b" * 64
MANIFEST_ID = "c" * 32
QUARANTINE_ID = "d" * 32
GENERATED_AT = 1_800_400_000


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "ai-prescreen-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--body-digest-hex",
        DIGEST,
    ]
    if kind in MODULE.RUNNER_BINDING_KINDS:
        args.extend(
            [
                "--manifest-id-hex",
                MANIFEST_ID,
                "--runner-hash-hex",
                DIGEST,
                "--subject",
                "cid:example",
                "--subject-digest-hex",
                DIGEST,
            ]
        )
    if kind in MODULE.WORKFLOW_DIGEST_KINDS:
        args.extend(["--workflow-digest-hex", DIGEST])
    if kind == "runner":
        args.extend(
            [
                "--runner-url",
                "https://runner.example",
                "--evidence-digest-hex",
                DIGEST,
                "--policy-digest-hex",
                POLICY_DIGEST,
            ]
        )
    elif kind == "committee":
        args.extend(["--committee-url", "https://committee.example"])
    elif kind == "operator_workflow":
        args.extend(
            [
                "--operator-url",
                "https://operator.example",
                "--quarantine-id-hex",
                QUARANTINE_ID,
            ]
        )
        for route in MODULE.REQUIRED_OPERATOR_ROUTES:
            args.extend(["--operator-route", route])
    elif kind == "notification_transport":
        args.extend(
            [
                "--webhook-url",
                "https://notifications.example/hook",
                "--probe-count",
                "2",
            ]
        )
    elif kind == "transparency_publication":
        for source_kind in MODULE.REQUIRED_TRANSPARENCY_SOURCE_KINDS:
            args.extend(["--transparency-source-kind", source_kind])
    elif kind == "governance_dag":
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
        for producer in MODULE.REQUIRED_GOVERNANCE_PRODUCERS:
            args.extend(["--governance-producer", producer])
    elif kind == "end_to_end_workflow":
        args.extend(["--workflow-id", "sfm-4a-prod-canary-20260701"])
        for step in MODULE.REQUIRED_E2E_STEPS:
            args.extend(["--workflow-step", step])
    return args


def test_builds_payload_free_notification_transport_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("notification_transport", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "notification_transport").read_text("utf-8")
    )

    assert payload["schema"] == (
        "sorafs.moderation.juror_notifications.transport_canary.v1"
    )
    assert payload["workflow_digest_hex"] == DIGEST
    assert payload["probe_count"] == 2
    assert payload["accepted_count"] == 2
    assert payload["payload_bytes_included"] is False
    assert payload["private_payloads_included"] is False
    kind, errors = CHECKER.validate_evidence_payload(payload)
    assert kind == "notification_transport"
    assert errors == []


def test_generated_canaries_pass_full_ai_prescreen_gate(tmp_path: Path) -> None:
    evidence_paths: list[Path] = []
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        evidence_paths.append(canary_path(tmp_path, kind))
    summary = tmp_path / "summary.json"

    command = []
    for path in evidence_paths:
        command.extend(["--evidence", str(path)])
    command.extend(["--summary-out", str(summary)])

    assert CHECKER.main(command) == 0

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["recognized_artifact_count"] == len(MODULE.CANARY_KINDS)
    assert payload["valid_runner_bindings"] == [
        {
            "manifest_id_hex": MANIFEST_ID,
            "runner_hash_hex": DIGEST,
            "subject_digest_hex": DIGEST,
        }
    ]
    assert payload["valid_workflow_digests"] == [DIGEST]
    assert payload["valid_notification_manifest_digests"] == [DIGEST]
    assert payload["valid_executor_summary_digests"] == [DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_response_file_can_build_end_to_end_workflow_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "e2e.args"
    args_file.write_text(
        "\n".join(args_for("end_to_end_workflow", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(
        canary_path(tmp_path, "end_to_end_workflow").read_text("utf-8")
    )
    assert payload["step_count"] == len(MODULE.REQUIRED_E2E_STEPS)
    assert payload["passed_step_count"] == len(MODULE.REQUIRED_E2E_STEPS)


def test_missing_operator_route_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("operator_workflow", tmp_path)
    index = args.index("--operator-route")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--operator-route must include every required value" in captured.err
    assert not canary_path(tmp_path, "operator_workflow").exists()


def test_operator_workflow_canary_records_passed_route_count(tmp_path: Path) -> None:
    assert MODULE.main(args_for("operator_workflow", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "operator_workflow").read_text("utf-8")
    )
    assert payload["route_count"] == len(MODULE.REQUIRED_OPERATOR_ROUTES)
    assert payload["passed_route_count"] == payload["route_count"]


def test_committee_result_count_must_cover_quorum(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("committee", tmp_path)
    args.extend(["--quorum", "4", "--result-count", "3"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--result-count must be >= --quorum" in captured.err
    assert not canary_path(tmp_path, "committee").exists()


def test_output_symlink_is_refused(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    link = tmp_path / "link.json"
    link.symlink_to(target)
    args = args_for("runner", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(link)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a symlink" in captured.err
    assert not target.exists()
