"""Tests for scripts/check_sorafs_ai_prescreen_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_ai_prescreen_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_ai_prescreen_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


DIGEST = "ab" * 32
DIGEST_2 = "cd" * 32
MANIFEST_ID = "12" * 16
QUARANTINE_ID = "34" * 16
DEPLOYMENT_ID = "ai-prescreen-staging-a"
ENVIRONMENT = "staging"


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def with_context(payload: dict) -> dict:
    payload["deployment_id"] = DEPLOYMENT_ID
    payload["environment"] = ENVIRONMENT
    return payload


def runner(*, status: str = "verified") -> dict:
    return with_context({
        "schema": "sorafs.moderation.runner.rollout_evidence.v1",
        "status": status,
        "source": "sorafs_cli",
        "runner_url": "https://runner.example",
        "status_url": "https://runner.example/v1/sorafs/moderation/runner/status",
        "screen_url": "https://runner.example/v1/sorafs/moderation/runner/screen",
        "manifest_id_hex": MANIFEST_ID,
        "runner_hash_hex": DIGEST,
        "subject": "cid:example",
        "subject_digest_hex": DIGEST,
        "screened_at_unix": 1_800_000_000,
        "checked_at_unix": 1_800_000_120,
        "combined_score_bps": 7250,
        "verdict": "quarantine",
        "evidence_digest_hex": DIGEST,
        "policy_digest_hex": DIGEST,
    })


def committee(*, status: str = "verified") -> dict:
    return with_context({
        "schema": "sorafs.moderation.committee.rollout_evidence.v1",
        "status": status,
        "source": "sorafs_cli",
        "committee_url": "https://committee.example",
        "status_url": "https://committee.example/v1/sorafs/moderation/committee/status",
        "aggregate_url": "https://committee.example/v1/sorafs/moderation/committee/aggregate",
        "manifest_id_hex": MANIFEST_ID,
        "runner_hash_hex": DIGEST,
        "quorum": 2,
        "aggregation": "median_score_bps",
        "result_count": 3,
        "subject": "cid:example",
        "subject_digest_hex": DIGEST,
        "aggregated_score_bps": 7250,
        "verdict": "quarantine",
        "checked_at_unix": 1_800_000_180,
    })


def operator_route(name: str) -> dict:
    schemas = {
        "healthz": "sorafs.moderation.quarantine.operator_service.status.v1",
        "status": "sorafs.moderation.quarantine.operator_service.status.v1",
        "operator_panel": "sorafs.moderation.quarantine.operator_panel.v1",
        "bridge_plan": "sorafs.moderation.quarantine.bridge_plan.v1",
        "juror_plan": "sorafs.moderation.quarantine.juror_plan.v1",
        "juror_notifications": "sorafs.moderation.quarantine.juror_notifications.v1",
        "commit_reveal_status": "sorafs.moderation.quarantine.commit_reveal_status.v1",
    }
    return {
        "name": name,
        "method": "GET",
        "path": f"/{name}",
        "url": f"https://operator.example/{name}",
        "status_code": 200,
        "schema": schemas.get(name),
        "body_blake3_hex": DIGEST,
        "body_bytes": 128,
        "payload_bytes_included": False,
        "private_payloads_included": False,
    }


def operator_workflow(*, omit_route: str | None = None) -> dict:
    routes = [
        operator_route(name)
        for name in (
            "healthz",
            "status",
            "browser_ui",
            "operator_panel",
            "bridge_plan",
            "juror_plan",
            "juror_notifications",
            "commit_reveal_status",
        )
        if name != omit_route
    ]
    return with_context({
        "schema": "sorafs.moderation.quarantine.operator_canary.v1",
        "status": "passed",
        "source": "iroha_cli",
        "operator_url": "https://operator.example",
        "workflow_digest_hex": DIGEST,
        "quarantine_id_hex": QUARANTINE_ID,
        "generated_at_unix": 1_800_000_200,
        "route_count": len(routes),
        "payload_bytes_included": False,
        "private_payloads_included": False,
        "routes": routes,
    })


def notification_transport(*, accepted_count: int | None = None) -> dict:
    probes = [
        {
            "delivery_id": "notify-1",
            "dedup_key": "sorafs-moderation-juror:notify-1",
            "action": "commit",
            "case_id": "case-1",
            "round_id": "round-1",
            "juror_id": "juror-1@moderation",
            "notification_bytes": 256,
            "notification_body_blake3": DIGEST,
            "response_status": 202,
            "response_success": True,
            "response_bytes": 12,
            "response_body_blake3": DIGEST,
            "payload_bytes_included": False,
            "private_payloads_included": False,
        }
    ]
    return with_context({
        "schema": "sorafs.moderation.juror_notifications.transport_canary.v1",
        "source": "juror-notifications",
        "status": "passed",
        "manifest_path": "juror-notifications.json",
        "workflow_digest_hex": DIGEST,
        "manifest_body_blake3": DIGEST,
        "webhook_url": "https://notifications.example/hook",
        "probe_count": len(probes),
        "accepted_count": len(probes) if accepted_count is None else accepted_count,
        "payload_bytes_included": False,
        "private_payloads_included": False,
        "probes": probes,
    })


def commit_reveal_executor(*, execution_summary_present: bool = True) -> dict:
    artifacts = [
        {
            "name": "executor.env",
            "kind": "env",
            "path": "executor.env",
            "exists": True,
            "bytes": 64,
            "body_blake3": DIGEST,
            "passed": True,
            "checks": [{"name": "payload-free", "passed": True}],
            "payload_bytes_included": False,
            "private_payloads_included": False,
        },
        {
            "name": "run.sh",
            "kind": "script",
            "path": "run.sh",
            "exists": True,
            "bytes": 128,
            "body_blake3": DIGEST,
            "passed": True,
            "checks": [{"name": "executable", "passed": True}],
            "payload_bytes_included": False,
            "private_payloads_included": False,
        },
    ]
    return with_context({
        "schema": "sorafs.moderation.ballots.executor_canary.v1",
        "source": "executor-bundle",
        "status": "passed",
        "bundle_dir": "/tmp/executor",
        "workflow_digest_hex": DIGEST,
        "bundle_metadata_bytes": 128,
        "bundle_metadata_blake3": DIGEST,
        "service_name": "sorafs-moderation-ballots-executor",
        "interval_secs": 60,
        "artifact_count": len(artifacts),
        "passed_artifact_count": len(artifacts),
        "execution_summary_present": execution_summary_present,
        "execution_summary": {
            "passed": True,
            "path": "execution.json",
            "bytes": 512,
            "body_blake3": DIGEST,
            "action_count": 3,
            "commit_action_count": 1,
            "reveal_action_count": 1,
            "tally_action_count": 1,
            "payload_bytes_included": False,
            "private_payloads_included": False,
        }
        if execution_summary_present
        else None,
        "payload_bytes_included": False,
        "private_payloads_included": False,
        "private_payload_files_copied": False,
        "artifacts": artifacts,
    })


def transparency_publication(*, missing_source: str | None = None) -> dict:
    source_kinds = [
        "moderation-reviewed-quarantine",
        "moderation-appeal-handoff",
        "moderation-appeal-ballot",
        "moderation-juror-plan",
        "moderation-juror-notifications-delivery",
        "moderation-juror-notifications-canary",
        "moderation-commit-reveal-status",
        "moderation-ballots-executor",
    ]
    probes = [
        {
            "source_kind": source_kind,
            "payload_path": f"{source_kind}.json",
            "request_bytes": 128,
            "request_body_blake3": DIGEST,
            "response_status": 201,
            "response_success": True,
            "response_bytes": 16,
            "response_body_blake3": DIGEST,
            "payload_bytes_included": False,
            "private_payloads_included": False,
            "response_body_included": False,
        }
        for source_kind in source_kinds
        if source_kind != missing_source
    ]
    return with_context({
        "schema": "sorafs.transparency.source_entry.canary.v1",
        "source": "iroha_cli",
        "status": "passed",
        "workflow_digest_hex": DIGEST,
        "probe_count": len(probes),
        "passed_probe_count": len(probes),
        "source_entry_probe_count": len(probes),
        "payload_bytes_included": False,
        "private_payloads_included": False,
        "response_bodies_included": False,
        "probes": probes,
    })


def governance_dag(*, config_source: str = "iroha_config") -> dict:
    producers = [
        {"name": name}
        for name in (
            "screening_ingest",
            "quarantine_escalation",
            "operator_review",
            "appeal_handoff",
            "appeal_ballot",
            "juror_notifications",
            "commit_reveal_executor",
            "transparency_publication",
        )
    ]
    return with_context({
        "schema": "sorafs.moderation.governance_dag_rollout.v1",
        "status": "passed",
        "workflow_digest_hex": DIGEST,
        "governance_dag_bound": True,
        "live_producers_bound": True,
        "transparency_source_entries_bound": True,
        "screening_ingest_bound": True,
        "quarantine_escalation_bound": True,
        "role_provisioning_recorded": True,
        "config_source": config_source,
        "policy_digest_hex": DIGEST,
        "producer_count": len(producers),
        "edge_count": 12,
        "producers": producers,
        "payload_bytes_included": False,
        "private_payloads_included": False,
    })


def end_to_end_workflow(*, omit_step: str | None = None) -> dict:
    steps = [
        {"name": name, "passed": True}
        for name in (
            "ingest",
            "quarantine",
            "operator_review",
            "release",
            "appeal_handoff",
            "appeal_ballot",
            "juror_notifications",
            "commit_reveal_executor",
            "transparency_publication",
        )
        if name != omit_step
    ]
    return with_context({
        "schema": "sorafs.moderation.end_to_end_rollout.v1",
        "status": "passed",
        "workflow_id": "sfm-4a-prod-canary-20260625",
        "workflow_digest_hex": DIGEST,
        "deployed_services": True,
        "runner_committee_live": True,
        "ingest_quarantine_release_path_passed": True,
        "appeal_path_passed": True,
        "transparency_publication_passed": True,
        "role_gate_checks_passed": True,
        "encrypted_object_api_checks_passed": True,
        "step_count": len(steps),
        "passed_step_count": len(steps),
        "steps": steps,
        "payload_bytes_included": False,
        "private_payloads_included": False,
    })


def write_complete_evidence(root: Path) -> None:
    write_json(root / "runner.json", runner())
    write_json(root / "committee.json", committee())
    write_json(root / "operator-workflow.json", operator_workflow())
    write_json(root / "notification-transport.json", notification_transport())
    write_json(root / "commit-reveal-executor.json", commit_reveal_executor())
    write_json(root / "transparency-publication.json", transparency_publication())
    write_json(root / "governance-dag.json", governance_dag())
    write_json(root / "end-to-end-workflow.json", end_to_end_workflow())


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.moderation.ai_prescreen.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["operator_workflow"]["valid"] is True
    assert payload["recognized_artifact_count"] == 8
    assert payload["valid_runner_bindings"] == [
        {
            "manifest_id_hex": MANIFEST_ID,
            "runner_hash_hex": DIGEST,
            "subject_digest_hex": DIGEST,
        }
    ]
    assert payload["valid_workflow_digests"] == [DIGEST]
    assert payload["required"]["runner"]["artifacts"][0]["fingerprint"][
        "deployment_id"
    ] == DEPLOYMENT_ID


def test_missing_commit_reveal_executor_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "commit-reveal-executor.json").unlink()

    assert run_gate(tmp_path) == 1


def test_deployment_context_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = runner()
    del payload["deployment_id"]
    write_json(tmp_path / "runner.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["runner"]["artifacts"][0]
    assert "deployment_id must be a non-empty string" in artifact["errors"]


def test_unreviewed_deployment_context_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_dag()
    payload["deployment_id"] = "ai-prescreen-dev-a"
    payload["environment"] = "dev"
    write_json(tmp_path / "governance-dag.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact_errors = result["required"]["governance_dag"]["artifacts"][0][
        "errors"
    ]
    assert (
        "deployment_id must not contain non-reviewed deployment markers ['dev']"
        in artifact_errors
    )
    assert "environment must be one of" in "\n".join(artifact_errors)


def test_runner_status_must_be_verified(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "runner.json", runner(status="passed"))

    assert run_gate(tmp_path) == 1


def test_committee_must_match_runner_subject_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = committee()
    payload["subject_digest_hex"] = DIGEST_2
    write_json(tmp_path / "committee.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["required"]["committee"]["valid"] is False
    assert payload["required"]["committee"]["artifacts"][0]["valid"] is False
    errors = "\n".join(payload["errors"])
    assert (
        "committee manifest_id_hex, runner_hash_hex, and subject_digest_hex "
        "must match a valid runner artifact"
        in errors
    )


def test_operator_canary_must_cover_juror_notifications(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "operator-workflow.json",
        operator_workflow(omit_route="juror_notifications"),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["operator_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "routes must include name `juror_notifications`" in artifact["errors"]


def test_operator_route_schema_must_match_expected_route(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_workflow()
    payload["routes"][3]["schema"] = "sorafs.moderation.quarantine.wrong.v1"
    write_json(tmp_path / "operator-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["operator_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "routes[3].schema must be "
        "`sorafs.moderation.quarantine.operator_panel.v1`"
    ) in artifact["errors"]


def test_operator_workflow_requires_workflow_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_workflow()
    payload.pop("workflow_digest_hex")
    write_json(tmp_path / "operator-workflow.json", payload)

    assert run_gate(tmp_path) == 1


def test_transparency_workflow_binding_must_match_e2e(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = transparency_publication()
    payload["workflow_digest_hex"] = DIGEST_2
    write_json(tmp_path / "transparency-publication.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["required"]["transparency_publication"]["valid"] is False
    assert (
        payload["required"]["transparency_publication"]["artifacts"][0]["valid"]
        is False
    )
    errors = "\n".join(payload["errors"])
    assert (
        "transparency_publication workflow_digest_hex must match a valid "
        "end_to_end_workflow workflow_digest_hex"
        in errors
    )


def test_notification_transport_acceptance_must_equal_probe_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "notification-transport.json",
        notification_transport(accepted_count=0),
    )

    assert run_gate(tmp_path) == 1


def test_executor_requires_execution_summary(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "commit-reveal-executor.json",
        commit_reveal_executor(execution_summary_present=False),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["commit_reveal_executor"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "execution_summary must be an object" in artifact["errors"]


def test_transparency_publication_requires_moderation_source_kinds(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "transparency-publication.json",
        transparency_publication(missing_source="moderation-juror-notifications-canary"),
    )

    assert run_gate(tmp_path) == 1


def test_governance_dag_must_be_config_bound(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "governance-dag.json", governance_dag(config_source="env"))

    assert run_gate(tmp_path) == 1


def test_governance_dag_requires_policy_digest(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_dag()
    payload["policy_digest_hex"] = "not-hex"
    write_json(tmp_path / "governance-dag.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_dag"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "policy_digest_hex must be 64 hex characters" in artifact["errors"]


def test_e2e_workflow_requires_full_path(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "end-to-end-workflow.json",
        end_to_end_workflow(omit_step="release"),
    )

    assert run_gate(tmp_path) == 1


def test_sensitive_payload_field_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_workflow()
    payload["payload_b64"] = "secret"
    write_json(tmp_path / "operator-workflow.json", payload)

    assert run_gate(tmp_path) == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(
        tmp_path / "unknown.json",
        {"schema": "sorafs.moderation.unexpected.v1", "status": "passed"},
    )

    assert MODULE.main(["--evidence", str(path)]) == 1


def test_unknown_schema_in_directory_is_ignored_for_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "runner.json", runner())
    write_json(
        tmp_path / "unknown.json",
        {"schema": "sorafs.moderation.unexpected.v1", "status": "passed"},
    )

    assert run_gate(tmp_path, "--require-kind", "runner") == 0


def test_response_file_arguments_are_supported(tmp_path: Path) -> None:
    write_json(tmp_path / "runner.json", runner())
    args_file = tmp_path / "gate.args"
    args_file.write_text(
        "\n".join(
            [
                "# payload-free rollout gate arguments",
                "--evidence-dir",
                str(tmp_path),
                "--require-kind runner",
            ]
        ),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0


def test_invalid_optional_recognized_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "runner.json", runner())
    write_json(tmp_path / "committee.json", committee(status="failed"))

    assert run_gate(tmp_path, "--require-kind", "runner") == 1
