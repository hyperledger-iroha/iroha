"""Tests for scripts/check_sorafs_ai_prescreen_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_ai_prescreen_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_ai_prescreen_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

TEST_DIR = Path(__file__).resolve().parent
if str(TEST_DIR) not in sys.path:
    sys.path.insert(0, str(TEST_DIR))
from sorafs_rollout_runner_test_support import TopologyBoundChecker  # noqa: E402


DIGEST = "ab" * 32
DIGEST_2 = "cd" * 32
MANIFEST_ID = "12" * 16
QUARANTINE_ID = "34" * 16
SUBJECT_REFERENCE = "cid:bafyprodmoderation20260701"
DEPLOYMENT_ID = "ai-prescreen-production-a"
ENVIRONMENT = "production"
GENERATED_AT = 1_800_000_200
NOW_UNIX = GENERATED_AT
CHECKER = TopologyBoundChecker(
    MODULE.main,
    deployment_id=DEPLOYMENT_ID,
    environment=ENVIRONMENT,
    name="ai-prescreen-checker",
)


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def with_context(payload: dict) -> dict:
    payload.setdefault("generated_at_unix", GENERATED_AT)
    payload["deployment_id"] = DEPLOYMENT_ID
    payload["environment"] = ENVIRONMENT
    payload["deployment_context_reviewed"] = True
    return payload


def validation_options() -> object:
    return MODULE.ValidationOptions(
        now_unix=NOW_UNIX,
        max_evidence_age_secs=MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS,
    )


def runner(*, status: str = "verified", subject: str = SUBJECT_REFERENCE) -> dict:
    return with_context({
        "schema": "sorafs.moderation.runner.rollout_evidence.v1",
        "status": status,
        "synthetic": False,
        "source": "sorafs_cli",
        "outbound_network": "model_engine_none_process_policy_required",
        "process_isolation_evidence": {
            "required": True,
            "status": "runtime_verified",
            "enforcement": "systemd_ip_filter",
            "attestation_digest_hex": "".join(f"{index:02x}" for index in range(32)),
            "verified_at_unix": GENERATED_AT,
            "reviewed": True,
            "synthetic": False,
        },
        "runner_url": "https://runner.example",
        "status_url": "https://runner.example/v1/sorafs/moderation/runner/status",
        "screen_url": "https://runner.example/v1/sorafs/moderation/runner/screen",
        "manifest_id_hex": MANIFEST_ID,
        "runner_hash_hex": DIGEST,
        "subject": subject,
        "subject_digest_hex": DIGEST,
        "screened_at_unix": 1_800_000_000,
        "checked_at_unix": GENERATED_AT,
        "combined_score_bps": 7250,
        "verdict": "quarantine",
        "evidence_digest_hex": DIGEST,
        "policy_digest_hex": DIGEST,
        "probe_count": 2,
        "passed_probe_count": 2,
        "probes": [
            {
                "name": "status",
                "method": "GET",
                "url": "https://runner.example/v1/sorafs/moderation/runner/status",
                "status_code": 200,
                "request_bytes": 0,
                "request_body_blake3": MODULE.EMPTY_BLAKE3_HEX,
                "response_bytes": 256,
                "response_body_blake3": DIGEST,
                "passed": True,
                "payload_bytes_included": False,
                "private_payloads_included": False,
            },
            {
                "name": "screen",
                "method": "POST",
                "url": "https://runner.example/v1/sorafs/moderation/runner/screen",
                "status_code": 200,
                "request_bytes": 256,
                "request_body_blake3": DIGEST_2,
                "response_bytes": 384,
                "response_body_blake3": DIGEST_2,
                "passed": True,
                "payload_bytes_included": False,
                "private_payloads_included": False,
            },
        ],
        "runner_status": {
            "schema": "sorafs.moderation.runner.status.v1",
            "status": "ready",
            "manifest_id_hex": MANIFEST_ID,
            "runner_hash_hex": DIGEST,
            "outbound_network": "model_engine_none_process_policy_required",
            "process_isolation": "external_runtime_attestation_required",
            "process_isolation_verified": False,
        },
        "screening_result": {
            "subject": subject,
            "subject_digest_hex": DIGEST,
            "manifest_id_hex": MANIFEST_ID,
            "runner_hash_hex": DIGEST,
            "screened_at_unix": 1_800_000_000,
            "combined_score_bps": 7250,
            "verdict": "quarantine",
            "evidence_digest_hex": DIGEST,
            "policy_digest_hex": DIGEST,
        },
    })


def committee(*, status: str = "verified", subject: str = SUBJECT_REFERENCE) -> dict:
    return with_context({
        "schema": "sorafs.moderation.committee.rollout_evidence.v1",
        "status": status,
        "synthetic": False,
        "source": "sorafs_cli",
        "outbound_network": "network_capable_process_policy_required",
        "process_isolation_evidence": {
            "required": True,
            "status": "runtime_verified",
            "enforcement": "systemd_ip_filter",
            "attestation_digest_hex": "".join(f"{index:02x}" for index in range(32, 64)),
            "verified_at_unix": GENERATED_AT,
            "reviewed": True,
            "synthetic": False,
        },
        "committee_url": "https://committee.example",
        "status_url": "https://committee.example/v1/sorafs/moderation/committee/status",
        "aggregate_url": "https://committee.example/v1/sorafs/moderation/committee/aggregate",
        "manifest_id_hex": MANIFEST_ID,
        "runner_hash_hex": DIGEST,
        "quorum": 2,
        "aggregation": "median_score_bps",
        "result_count": 3,
        "results": [
            {
                "name": "ai-prescreen-committee-result-a",
                "bytes": 256,
                "body_blake3_hex": "01" * 32,
                "payload_bytes_included": False,
                "private_payloads_included": False,
            },
            {
                "name": "ai-prescreen-committee-result-b",
                "bytes": 257,
                "body_blake3_hex": "02" * 32,
                "payload_bytes_included": False,
                "private_payloads_included": False,
            },
            {
                "name": "ai-prescreen-committee-result-c",
                "bytes": 258,
                "body_blake3_hex": "03" * 32,
                "payload_bytes_included": False,
                "private_payloads_included": False,
            },
        ],
        "subject": subject,
        "subject_digest_hex": DIGEST,
        "aggregated_score_bps": 7250,
        "verdict": "quarantine",
        "checked_at_unix": GENERATED_AT,
        "probe_count": 2,
        "passed_probe_count": 2,
        "probes": [
            {
                "name": "status",
                "method": "GET",
                "url": "https://committee.example/v1/sorafs/moderation/committee/status",
                "status_code": 200,
                "request_bytes": 0,
                "request_body_blake3": MODULE.EMPTY_BLAKE3_HEX,
                "response_bytes": 256,
                "response_body_blake3": DIGEST,
                "passed": True,
                "payload_bytes_included": False,
                "private_payloads_included": False,
            },
            {
                "name": "aggregate",
                "method": "POST",
                "url": "https://committee.example/v1/sorafs/moderation/committee/aggregate",
                "status_code": 200,
                "request_bytes": 512,
                "request_body_blake3": DIGEST_2,
                "response_bytes": 512,
                "response_body_blake3": DIGEST_2,
                "passed": True,
                "payload_bytes_included": False,
                "private_payloads_included": False,
            },
        ],
        "committee_status": {
            "schema": "sorafs.moderation.committee.status.v1",
            "status": "ready",
            "manifest_id_hex": MANIFEST_ID,
            "runner_hash_hex": DIGEST,
            "quorum": 2,
            "aggregation": "median_score_bps",
            "outbound_network": "network_capable_process_policy_required",
            "process_isolation": "external_runtime_attestation_required",
            "process_isolation_verified": False,
        },
        "committee_aggregate": {
            "schema": "sorafs.moderation.committee.aggregate.v1",
            "status": "quorum_satisfied",
            "manifest_id_hex": MANIFEST_ID,
            "runner_hash_hex": DIGEST,
            "subject": subject,
            "subject_digest_hex": DIGEST,
            "result_count": 3,
            "quorum": 2,
            "aggregation": "median_score_bps",
            "aggregated_score_bps": 7250,
            "verdict": "quarantine",
        },
    })


def operator_route(name: str) -> dict:
    route_path = MODULE.operator_route_paths(QUARANTINE_ID).get(name, f"/{name}")
    return {
        "name": name,
        "method": "GET",
        "path": route_path,
        "url": f"https://operator.example{route_path}",
        "status_code": 200,
        "content_type": MODULE.REQUIRED_OPERATOR_CONTENT_TYPES.get(
            name,
            "application/json",
        ),
        "schema": MODULE.REQUIRED_OPERATOR_SCHEMAS.get(name),
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
        "passed_route_count": len(routes),
        "payload_bytes_included": False,
        "private_payloads_included": False,
        "routes": routes,
    })


def notification_transport(*, accepted_count: int | None = None) -> dict:
    probes = [
        {
            "delivery_id": "ai-prescreen-notification-delivery-01",
            "dedup_key": (
                "sorafs-moderation-juror:"
                "ai-prescreen-notification-delivery-01"
            ),
            "action": "submit_commit",
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
        },
        {
            "delivery_id": "ai-prescreen-notification-delivery-02",
            "dedup_key": (
                "sorafs-moderation-juror:"
                "ai-prescreen-notification-delivery-02"
            ),
            "action": "submit_reveal",
            "case_id": "case-1",
            "round_id": "round-1",
            "juror_id": "juror-2@moderation",
            "notification_bytes": 256,
            "notification_body_blake3": DIGEST,
            "response_status": 202,
            "response_success": True,
            "response_bytes": 12,
            "response_body_blake3": DIGEST,
            "payload_bytes_included": False,
            "private_payloads_included": False,
        },
    ]
    return with_context({
        "schema": "sorafs.moderation.juror_notifications.transport_canary.v1",
        "source": "juror-notifications",
        "status": "passed",
        "manifest_path": "juror-notifications.json",
        "workflow_digest_hex": DIGEST,
        "manifest_body_blake3_hex": DIGEST,
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
        "execution_summary_digest_hex": DIGEST,
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
    producer_names = (
        "screening_ingest",
        "quarantine_escalation",
        "operator_review",
        "appeal_handoff",
        "appeal_ballot",
        "juror_notifications",
        "commit_reveal_executor",
        "transparency_publication",
    )
    producers = [
        {"name": name}
        for name in producer_names
    ]
    edges = [
        {
            "producer": producer,
            "name": f"ai-prescreen-governance-edge-{producer.replace('_', '-')}",
        }
        for producer in producer_names
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
        "edge_count": len(edges),
        "producers": producers,
        "edges": edges,
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


RUNNER_BOUND_FIXTURES = (
    ("committee", "committee.json", committee),
)

WORKFLOW_BOUND_FIXTURES = (
    ("operator_workflow", "operator-workflow.json", operator_workflow),
    ("notification_transport", "notification-transport.json", notification_transport),
    ("commit_reveal_executor", "commit-reveal-executor.json", commit_reveal_executor),
    ("transparency_publication", "transparency-publication.json", transparency_publication),
    ("governance_dag", "governance-dag.json", governance_dag),
)

POLICY_BOUND_FIXTURES = (
    ("governance_dag", "governance-dag.json", governance_dag),
)


def run_gate(root: Path, *extra: str) -> int:
    return CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(root), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.moderation.ai_prescreen.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["operator_workflow"]["valid"] is True
    assert payload["thresholds"] == {
        "max_evidence_bytes": MODULE.MAX_EVIDENCE_BYTES,
        "max_evidence_age_secs": MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS,
    }
    assert payload["recognized_artifact_count"] == 8
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
    assert payload["valid_policy_digests"] == [DIGEST]
    assert payload["required"]["runner"]["artifacts"][0]["fingerprint"][
        "deployment_id"
    ] == DEPLOYMENT_ID


def test_generated_at_unix_rejects_future_and_stale_artifacts(tmp_path: Path) -> None:
    cases = (
        (
            "future",
            NOW_UNIX + 1,
            "generated_at_unix must not be in the future",
        ),
        (
            "stale",
            NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1,
            f"generated_at_unix is older than {MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS} seconds",
        ),
    )

    for label, generated_at, expected_error in cases:
        root = tmp_path / label
        root.mkdir()
        payload = runner()
        payload["generated_at_unix"] = generated_at
        write_json(root / "runner.json", payload)
        summary = root / "summary.json"

        assert run_gate(root, "--require-kind", "runner", "--summary-out", str(summary)) == 1

        report = json.loads(summary.read_text(encoding="utf-8"))
        assert expected_error in json.dumps(report)


def test_shape_only_runner_evidence_without_live_probes_fails_closed(
    tmp_path: Path,
) -> None:
    payload = runner()
    for field in (
        "probe_count",
        "passed_probe_count",
        "probes",
        "runner_status",
        "screening_result",
    ):
        del payload[field]
    write_json(tmp_path / "runner.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(
        tmp_path,
        "--require-kind",
        "runner",
        "--summary-out",
        str(summary),
    ) == 1

    artifact = json.loads(summary.read_text("utf-8"))["required"]["runner"][
        "artifacts"
    ][0]
    assert artifact["valid"] is False
    assert "probes must be a non-empty array" in artifact["errors"]
    assert "runner_status must be an object" in artifact["errors"]
    assert "screening_result must be an object" in artifact["errors"]


@pytest.mark.parametrize(("kind", "factory"), (("runner", runner), ("committee", committee)))
def test_synthetic_runner_or_committee_evidence_is_never_production_evidence(
    kind: str,
    factory,
    tmp_path: Path,
) -> None:
    payload = factory()
    payload["synthetic"] = True
    write_json(tmp_path / f"{kind}.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(
        tmp_path,
        "--require-kind",
        kind,
        "--summary-out",
        str(summary),
    ) == 1

    artifact = json.loads(summary.read_text("utf-8"))["required"][kind][
        "artifacts"
    ][0]
    assert "synthetic must be false" in artifact["errors"]


def test_runner_live_probe_inventory_rejects_missing_and_duplicate_probes(
    tmp_path: Path,
) -> None:
    missing_dir = tmp_path / "missing"
    missing_dir.mkdir()
    missing = runner()
    missing["probes"].pop()
    write_json(missing_dir / "runner.json", missing)
    missing_summary = missing_dir / "summary.json"

    assert run_gate(
        missing_dir,
        "--require-kind",
        "runner",
        "--summary-out",
        str(missing_summary),
    ) == 1
    missing_errors = json.loads(missing_summary.read_text("utf-8"))["required"][
        "runner"
    ]["artifacts"][0]["errors"]
    assert "probe_count must equal probes length" in missing_errors
    assert "probes must include name `screen`" in missing_errors

    duplicate_dir = tmp_path / "duplicate"
    duplicate_dir.mkdir()
    duplicate = runner()
    duplicate["probes"].append(dict(duplicate["probes"][0]))
    duplicate["probe_count"] = len(duplicate["probes"])
    duplicate["passed_probe_count"] = len(duplicate["probes"])
    write_json(duplicate_dir / "runner.json", duplicate)
    duplicate_summary = duplicate_dir / "summary.json"

    assert run_gate(
        duplicate_dir,
        "--require-kind",
        "runner",
        "--summary-out",
        str(duplicate_summary),
    ) == 1
    duplicate_errors = json.loads(duplicate_summary.read_text("utf-8"))[
        "required"
    ]["runner"]["artifacts"][0]["errors"]
    assert "probes must not contain duplicate values" in duplicate_errors
    assert "probes must not contain duplicate fingerprints" in duplicate_errors


def test_runner_timestamps_are_probe_completion_bound(tmp_path: Path) -> None:
    payload = runner()
    payload["checked_at_unix"] = payload["generated_at_unix"] - 1
    payload["screened_at_unix"] = payload["generated_at_unix"] + 1
    payload["screening_result"]["screened_at_unix"] = payload["screened_at_unix"]
    write_json(tmp_path / "runner.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(
        tmp_path,
        "--require-kind",
        "runner",
        "--summary-out",
        str(summary),
    ) == 1
    errors = json.loads(summary.read_text("utf-8"))["required"]["runner"][
        "artifacts"
    ][0]["errors"]
    assert "checked_at_unix must equal generated_at_unix" in errors
    assert "screened_at_unix must not be after checked_at_unix" in errors


def test_rollout_artifacts_must_share_one_reviewed_deployment_context(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    drifted = committee()
    drifted["deployment_id"] = "ai-prescreen-staging-b"
    write_json(tmp_path / "committee.json", drifted)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    report = json.loads(summary.read_text("utf-8"))
    assert report["deployment_context"] == {}
    assert (
        "valid_deployment_contexts must contain exactly one active binding"
        in report["errors"]
    )


def test_bound_fixture_tables_cover_checker_bound_kind_sets() -> None:
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in RUNNER_BOUND_FIXTURES)
        == MODULE.RUNNER_BOUND_KINDS
    )
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in WORKFLOW_BOUND_FIXTURES)
        == MODULE.WORKFLOW_BOUND_KINDS
    )
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in POLICY_BOUND_FIXTURES)
        == MODULE.POLICY_BOUND_KINDS
    )


def test_fixture_inventories_cover_checker_required_sets() -> None:
    operator_routes = operator_workflow()["routes"]
    assert tuple(route["name"] for route in operator_workflow()["routes"]) == (
        MODULE.REQUIRED_OPERATOR_ROUTES
    )
    assert {
        route["name"]: route["schema"]
        for route in operator_routes
        if route["schema"] is not None
    } == MODULE.REQUIRED_OPERATOR_SCHEMAS
    assert {
        route["name"]: route["content_type"] for route in operator_routes
    } == MODULE.REQUIRED_OPERATOR_CONTENT_TYPES
    assert all(
        probe["action"] in MODULE.ALLOWED_NOTIFICATION_ACTIONS
        for probe in notification_transport()["probes"]
    )
    assert (
        tuple(probe["action"] for probe in notification_transport()["probes"])
        == MODULE.ALLOWED_NOTIFICATION_ACTIONS
    )
    executor_artifacts = commit_reveal_executor()["artifacts"]
    assert tuple(artifact["name"] for artifact in executor_artifacts) == (
        MODULE.REQUIRED_EXECUTOR_ARTIFACTS
    )
    assert {
        artifact["name"]: artifact["kind"] for artifact in executor_artifacts
    } == MODULE.REQUIRED_EXECUTOR_ARTIFACT_KINDS
    assert tuple(
        probe["source_kind"] for probe in transparency_publication()["probes"]
    ) == MODULE.REQUIRED_TRANSPARENCY_SOURCE_KINDS

    governance = governance_dag()
    assert tuple(producer["name"] for producer in governance["producers"]) == (
        MODULE.REQUIRED_GOVERNANCE_PRODUCERS
    )
    assert governance["edge_count"] == MODULE.REQUIRED_GOVERNANCE_EDGE_COUNT
    assert tuple(step["name"] for step in end_to_end_workflow()["steps"]) == (
        MODULE.REQUIRED_E2E_STEPS
    )


def test_expected_operator_route_url_preserves_exact_base_url() -> None:
    assert MODULE.expected_operator_route_url(
        "https://operator.example/",
        "/healthz",
    ) == "https://operator.example//healthz"


def test_all_runner_bound_artifacts_reject_runner_binding_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in RUNNER_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["subject_digest_hex"] = DIGEST_2
        payload["committee_aggregate"]["subject_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert result["valid_runner_bindings"] == [
            {
                "manifest_id_hex": MANIFEST_ID,
                "runner_hash_hex": DIGEST,
                "subject_digest_hex": DIGEST,
            }
        ]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} manifest_id_hex, runner_hash_hex, and "
            "subject_digest_hex must match a valid runner artifact"
        ) in artifact["errors"]


def test_all_workflow_bound_artifacts_reject_e2e_workflow_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in WORKFLOW_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["workflow_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert result["valid_workflow_digests"] == [DIGEST]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} workflow_digest_hex must match a valid "
            "end_to_end_workflow workflow_digest_hex"
        ) in artifact["errors"]


def test_all_policy_bound_artifacts_reject_runner_policy_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in POLICY_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["policy_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert result["valid_policy_digests"] == [DIGEST]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} policy_digest_hex must match a valid "
            "runner policy_digest_hex"
        ) in artifact["errors"]


def test_multiple_valid_runner_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = runner()
    payload["subject_digest_hex"] = DIGEST_2
    payload["screening_result"]["subject_digest_hex"] = DIGEST_2
    write_json(tmp_path / "runner-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_runner_bindings"] == []
    assert (
        "valid_runner_bindings must contain exactly one active binding"
        in result["errors"]
    )


def test_multiple_valid_workflow_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = end_to_end_workflow()
    payload["workflow_digest_hex"] = DIGEST_2
    write_json(tmp_path / "end-to-end-workflow-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_workflow_digests"] == []
    assert (
        "valid_workflow_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_notification_manifest_anchors_fail_closed(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = notification_transport()
    payload["manifest_body_blake3_hex"] = DIGEST_2
    write_json(tmp_path / "notification-transport-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_notification_manifest_digests"] == []
    assert (
        "valid_notification_manifest_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_executor_summary_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = commit_reveal_executor()
    payload["execution_summary_digest_hex"] = DIGEST_2
    payload["execution_summary"]["body_blake3"] = DIGEST_2
    write_json(tmp_path / "commit-reveal-executor-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_executor_summary_digests"] == []
    assert (
        "valid_executor_summary_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_policy_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = runner()
    payload["policy_digest_hex"] = DIGEST_2
    payload["screening_result"]["policy_digest_hex"] = DIGEST_2
    write_json(tmp_path / "runner-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_policy_digests"] == []
    assert (
        "valid_policy_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_payload_safety_flags_are_required(tmp_path: Path) -> None:
    cases = (
        (
            "operator-top-payload-bytes",
            "operator_workflow",
            "operator-workflow.json",
            operator_workflow,
            ("payload_bytes_included",),
        ),
        (
            "operator-top-private-payloads",
            "operator_workflow",
            "operator-workflow.json",
            operator_workflow,
            ("private_payloads_included",),
        ),
        (
            "operator-route-payload-bytes",
            "operator_workflow",
            "operator-workflow.json",
            operator_workflow,
            ("routes", 0, "payload_bytes_included"),
        ),
        (
            "operator-route-private-payloads",
            "operator_workflow",
            "operator-workflow.json",
            operator_workflow,
            ("routes", 0, "private_payloads_included"),
        ),
        (
            "notification-top-payload-bytes",
            "notification_transport",
            "notification-transport.json",
            notification_transport,
            ("payload_bytes_included",),
        ),
        (
            "notification-top-private-payloads",
            "notification_transport",
            "notification-transport.json",
            notification_transport,
            ("private_payloads_included",),
        ),
        (
            "notification-probe-payload-bytes",
            "notification_transport",
            "notification-transport.json",
            notification_transport,
            ("probes", 0, "payload_bytes_included"),
        ),
        (
            "notification-probe-private-payloads",
            "notification_transport",
            "notification-transport.json",
            notification_transport,
            ("probes", 0, "private_payloads_included"),
        ),
        (
            "executor-top-payload-bytes",
            "commit_reveal_executor",
            "commit-reveal-executor.json",
            commit_reveal_executor,
            ("payload_bytes_included",),
        ),
        (
            "executor-top-private-payloads",
            "commit_reveal_executor",
            "commit-reveal-executor.json",
            commit_reveal_executor,
            ("private_payloads_included",),
        ),
        (
            "executor-private-files",
            "commit_reveal_executor",
            "commit-reveal-executor.json",
            commit_reveal_executor,
            ("private_payload_files_copied",),
        ),
        (
            "executor-artifact-payload-bytes",
            "commit_reveal_executor",
            "commit-reveal-executor.json",
            commit_reveal_executor,
            ("artifacts", 0, "payload_bytes_included"),
        ),
        (
            "executor-artifact-private-payloads",
            "commit_reveal_executor",
            "commit-reveal-executor.json",
            commit_reveal_executor,
            ("artifacts", 0, "private_payloads_included"),
        ),
        (
            "executor-summary-payload-bytes",
            "commit_reveal_executor",
            "commit-reveal-executor.json",
            commit_reveal_executor,
            ("execution_summary", "payload_bytes_included"),
        ),
        (
            "executor-summary-private-payloads",
            "commit_reveal_executor",
            "commit-reveal-executor.json",
            commit_reveal_executor,
            ("execution_summary", "private_payloads_included"),
        ),
        (
            "transparency-top-payload-bytes",
            "transparency_publication",
            "transparency-publication.json",
            transparency_publication,
            ("payload_bytes_included",),
        ),
        (
            "transparency-top-private-payloads",
            "transparency_publication",
            "transparency-publication.json",
            transparency_publication,
            ("private_payloads_included",),
        ),
        (
            "transparency-response-bodies",
            "transparency_publication",
            "transparency-publication.json",
            transparency_publication,
            ("response_bodies_included",),
        ),
        (
            "transparency-probe-payload-bytes",
            "transparency_publication",
            "transparency-publication.json",
            transparency_publication,
            ("probes", 0, "payload_bytes_included"),
        ),
        (
            "transparency-probe-private-payloads",
            "transparency_publication",
            "transparency-publication.json",
            transparency_publication,
            ("probes", 0, "private_payloads_included"),
        ),
        (
            "transparency-probe-response-body",
            "transparency_publication",
            "transparency-publication.json",
            transparency_publication,
            ("probes", 0, "response_body_included"),
        ),
        (
            "governance-payload-bytes",
            "governance_dag",
            "governance-dag.json",
            governance_dag,
            ("payload_bytes_included",),
        ),
        (
            "governance-private-payloads",
            "governance_dag",
            "governance-dag.json",
            governance_dag,
            ("private_payloads_included",),
        ),
        (
            "e2e-payload-bytes",
            "end_to_end_workflow",
            "end-to-end-workflow.json",
            end_to_end_workflow,
            ("payload_bytes_included",),
        ),
        (
            "e2e-private-payloads",
            "end_to_end_workflow",
            "end-to-end-workflow.json",
            end_to_end_workflow,
            ("private_payloads_included",),
        ),
    )
    for label, kind, filename, factory, field_path in cases:
        root = tmp_path / label
        root.mkdir()
        write_complete_evidence(root)
        payload = factory()
        target = payload
        for item in field_path[:-1]:
            target = target[item]
        field = field_path[-1]
        del target[field]
        write_json(root / filename, payload)
        summary = root / "summary.json"

        assert run_gate(root, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert artifact["valid"] is False
        assert f"{field} must be false" in artifact["errors"]


def test_url_evidence_fields_must_be_safe_without_leaking(
    tmp_path: Path,
    capsys,
) -> None:
    cases = (
        (
            "runner.json",
            "runner",
            runner(),
            "runner_url",
            "https://user:private_key@runner.example",
        ),
        (
            "runner.json",
            "runner",
            runner(),
            "status_url",
            "https://runner.example/%2e%2e/status",
        ),
        (
            "committee.json",
            "committee",
            committee(),
            "aggregate_url",
            "https://committee.example/C%3A/aggregate",
        ),
        (
            "committee.json",
            "committee",
            committee(),
            "aggregate_url",
            "https://C%3A.committee.example/aggregate",
        ),
        (
            "committee.json",
            "committee",
            committee(),
            "aggregate_url",
            "https://http%3A.committee.example/aggregate",
        ),
        (
            "operator-workflow.json",
            "operator_workflow",
            operator_workflow(),
            "operator_url",
            "https://operator.example/%70rivate_key",
        ),
        (
            "operator-workflow.json",
            "operator_workflow",
            operator_workflow(),
            "routes.0.url",
            "https://operator.example/bad%2Froute",
        ),
        (
            "notification-transport.json",
            "notification_transport",
            notification_transport(),
            "webhook_url",
            "https://notifications.example/hook?token=secret",
        ),
    )

    for index, (file_name, kind, payload, field, unsafe_url) in enumerate(cases):
        case_dir = tmp_path / f"case-{index}"
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        if field == "routes.0.url":
            payload["routes"][0]["url"] = unsafe_url
        else:
            payload[field] = unsafe_url
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        captured = capsys.readouterr()
        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        result_text = json.dumps(result, sort_keys=True)
        assert MODULE.EVIDENCE_URL_FIELD_ERROR in artifact["errors"]
        assert unsafe_url not in captured.err
        assert unsafe_url not in result_text


def test_base_url_evidence_fields_reject_trailing_slash(tmp_path: Path) -> None:
    cases = (
        ("runner", "runner.json", runner(), "runner_url"),
        ("committee", "committee.json", committee(), "committee_url"),
        (
            "operator_workflow",
            "operator-workflow.json",
            operator_workflow(),
            "operator_url",
        ),
    )

    for index, (kind, file_name, payload, field) in enumerate(cases):
        case_dir = tmp_path / f"case-{index}"
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload[field] = f"{payload[field]}/"
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert artifact["valid"] is False
        assert MODULE.AI_PRESCREEN_BASE_URL_ERROR in artifact["errors"]


def test_path_evidence_fields_must_be_archive_portable_without_leaking(
    tmp_path: Path,
    capsys,
) -> None:
    cases = (
        (
            "notification-transport.json",
            "notification_transport",
            notification_transport(),
            "manifest_path",
            "manifests/%2e%2e/private_key.json",
        ),
        (
            "commit-reveal-executor.json",
            "commit_reveal_executor",
            commit_reveal_executor(),
            "artifacts.0.path",
            "bundle/bad%2Fprivate_key.env",
        ),
        (
            "commit-reveal-executor.json",
            "commit_reveal_executor",
            commit_reveal_executor(),
            "execution_summary.path",
            "summaries/C%3A/private_key.json",
        ),
        (
            "transparency-publication.json",
            "transparency_publication",
            transparency_publication(),
            "probes.0.payload_path",
            "payloads/%252e%252e/private_key.json",
        ),
    )

    for index, (file_name, kind, payload, field, unsafe_path) in enumerate(cases):
        case_dir = tmp_path / f"path-case-{index}"
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        if field == "artifacts.0.path":
            payload["artifacts"][0]["path"] = unsafe_path
        elif field == "execution_summary.path":
            payload["execution_summary"]["path"] = unsafe_path
        elif field == "probes.0.payload_path":
            payload["probes"][0]["payload_path"] = unsafe_path
        else:
            payload[field] = unsafe_path
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        captured = capsys.readouterr()
        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        result_text = json.dumps(result, sort_keys=True)
        assert any(MODULE.EVIDENCE_PATH_FIELD_ERROR in error for error in artifact["errors"])
        assert unsafe_path not in captured.err
        assert unsafe_path not in result_text


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
    assert "deployment_id must be a non-empty canonical string" in artifact["errors"]


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


def test_runner_live_status_probe_must_report_ready(tmp_path: Path) -> None:
    payload = runner()
    payload["runner_status"]["status"] = "degraded"
    write_json(tmp_path / "runner.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(
        tmp_path,
        "--require-kind",
        "runner",
        "--summary-out",
        str(summary),
    ) == 1

    errors = json.loads(summary.read_text("utf-8"))["required"]["runner"][
        "artifacts"
    ][0]["errors"]
    assert "runner_status.status must be `ready`" in errors


def test_runner_status_uses_honest_outbound_posture(tmp_path: Path) -> None:
    payload = runner()
    payload["runner_status"]["outbound_network"] = "disabled"
    write_json(tmp_path / "runner.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(
        tmp_path,
        "--require-kind",
        "runner",
        "--summary-out",
        str(summary),
    ) == 1
    errors = json.loads(summary.read_text("utf-8"))["required"]["runner"][
        "artifacts"
    ][0]["errors"]
    assert any("model_engine_none_process_policy_required" in error for error in errors)


def test_direct_runner_canary_without_runtime_isolation_attestation_fails(
    tmp_path: Path,
) -> None:
    payload = runner()
    payload["process_isolation_evidence"] = {
        "required": True,
        "status": "not_verified",
        "reason": "runner cannot self-attest its host runtime network policy",
    }
    write_json(tmp_path / "runner.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(
        tmp_path,
        "--require-kind",
        "runner",
        "--summary-out",
        str(summary),
    ) == 1
    errors = json.loads(summary.read_text("utf-8"))["required"]["runner"][
        "artifacts"
    ][0]["errors"]
    assert any("process_isolation_evidence.status" in error for error in errors)


def test_committee_status_uses_honest_outbound_posture(tmp_path: Path) -> None:
    payload = committee()
    payload["committee_status"]["outbound_network"] = "disabled"
    write_json(tmp_path / "committee.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(
        tmp_path,
        "--require-kind",
        "committee",
        "--summary-out",
        str(summary),
    ) == 1
    errors = json.loads(summary.read_text("utf-8"))["required"]["committee"][
        "artifacts"
    ][0]["errors"]
    assert any("network_capable_process_policy_required" in error for error in errors)


def test_direct_committee_canary_without_runtime_isolation_attestation_fails(
    tmp_path: Path,
) -> None:
    payload = committee()
    payload["process_isolation_evidence"] = {
        "required": True,
        "status": "not_verified",
        "reason": "committee cannot self-attest its host runtime network policy",
    }
    write_json(tmp_path / "committee.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(
        tmp_path,
        "--require-kind",
        "committee",
        "--summary-out",
        str(summary),
    ) == 1
    errors = json.loads(summary.read_text("utf-8"))["required"]["committee"][
        "artifacts"
    ][0]["errors"]
    assert any("process_isolation_evidence.status" in error for error in errors)


@pytest.mark.parametrize(
    ("kind", "filename", "factory"),
    [
        ("runner", "runner.json", runner),
        ("committee", "committee.json", committee),
    ],
)
def test_runtime_isolation_attestation_rejects_forged_metadata(
    tmp_path: Path,
    kind: str,
    filename: str,
    factory,
) -> None:
    mutations = [
        ("unsupported enforcement", "enforcement", "application_claim"),
        ("placeholder digest", "attestation_digest_hex", "ab" * 32),
        ("future timestamp", "verified_at_unix", GENERATED_AT + 1),
        ("unreviewed evidence", "reviewed", False),
        ("synthetic evidence", "synthetic", True),
    ]
    for label, field, value in mutations:
        payload = factory()
        payload["process_isolation_evidence"][field] = value
        write_json(tmp_path / filename, payload)
        summary = tmp_path / f"summary-{field}.json"

        assert run_gate(
            tmp_path,
            "--require-kind",
            kind,
            "--summary-out",
            str(summary),
        ) == 1, label
        errors = json.loads(summary.read_text("utf-8"))["required"][kind][
            "artifacts"
        ][0]["errors"]
        assert any("process_isolation_evidence" in error for error in errors), (
            label,
            errors,
        )


def test_runner_evidence_digest_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = runner()
    del payload["evidence_digest_hex"]
    write_json(tmp_path / "runner.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["runner"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "evidence_digest_hex must be a non-empty string" in artifact["errors"]


def test_operator_route_body_bytes_must_be_positive(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_workflow()
    payload["routes"][0]["body_bytes"] = 0
    write_json(tmp_path / "operator-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["operator_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "body_bytes must be a positive integer" in artifact["errors"]


def test_operator_route_body_hash_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_workflow()
    del payload["routes"][0]["body_blake3_hex"]
    write_json(tmp_path / "operator-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["operator_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "routes[0].body_blake3_hex must be a non-empty string"
        in artifact["errors"]
    )


def test_operator_route_url_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_workflow()
    del payload["routes"][0]["url"]
    write_json(tmp_path / "operator-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["operator_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "url must be a non-empty canonical string" in artifact["errors"]


def test_runner_subject_must_be_canonical(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = runner(subject="CID:prod-canary")
    write_json(tmp_path / "runner.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["runner"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "subject must match canonical lowercase `cid:name`" in artifact["errors"]


def test_runner_subject_rejects_non_production_markers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = runner(subject="cid:example")
    write_json(tmp_path / "runner.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["runner"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "subject must not contain non-production markers ['example']"
        in artifact["errors"]
    )


def test_committee_subject_rejects_non_production_markers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = committee(subject="cid:placeholder")
    write_json(tmp_path / "committee.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["committee"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "subject must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_subject_accepts_future_production_reference(tmp_path: Path) -> None:
    subject = "cid:bafyprodmoderation20260715"
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "runner.json", runner(subject=subject))
    write_json(tmp_path / "committee.json", committee(subject=subject))
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["status"] == "ready"
    assert payload["valid_runner_bindings"][0]["subject_digest_hex"] == DIGEST


def test_runner_and_committee_accept_only_shipped_verdicts() -> None:
    allowed = MODULE.ALLOWED_PRESCREEN_VERDICTS
    expected_error = (
        "verdict must be `pass`, `warn`, `quarantine`, `escalate` or `block`"
    )

    for build_payload in (runner, committee):
        for verdict in allowed:
            payload = build_payload()
            payload["verdict"] = verdict
            response_field = (
                "screening_result" if build_payload is runner else "committee_aggregate"
            )
            payload[response_field]["verdict"] = verdict
            kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
            assert kind in {"runner", "committee"}
            assert errors == []

        payload = build_payload()
        payload["verdict"] = "allow"
        response_field = (
            "screening_result" if build_payload is runner else "committee_aggregate"
        )
        payload[response_field]["verdict"] = "allow"
        _kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert expected_error in errors

        payload = build_payload()
        payload["verdict"] = " Quarantine "
        response_field = (
            "screening_result" if build_payload is runner else "committee_aggregate"
        )
        payload[response_field]["verdict"] = " Quarantine "
        _kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert "validation value must be a non-empty canonical string" in errors


@pytest.mark.parametrize(
    ("filename", "kind", "factory", "field", "value", "expected_error"),
    (
        (
            "runner.json",
            "runner",
            runner,
            "combined_score_bps",
            12.5,
            "combined_score_bps must be a non-negative integer",
        ),
        (
            "runner.json",
            "runner",
            runner,
            "combined_score_bps",
            10_001,
            "combined_score_bps must be <= 10000",
        ),
        (
            "committee.json",
            "committee",
            committee,
            "aggregated_score_bps",
            12.5,
            "aggregated_score_bps must be a non-negative integer",
        ),
        (
            "committee.json",
            "committee",
            committee,
            "aggregated_score_bps",
            10_001,
            "aggregated_score_bps must be <= 10000",
        ),
    ),
)
def test_runner_and_committee_score_bps_must_be_basis_points(
    filename: str,
    kind: str,
    factory,
    field: str,
    value,
    expected_error: str,
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = factory()
    payload[field] = value
    write_json(tmp_path / filename, payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"][kind]["artifacts"][0]
    assert expected_error in artifact["errors"]


def test_committee_must_match_runner_subject_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = committee()
    payload["subject_digest_hex"] = DIGEST_2
    payload["committee_aggregate"]["subject_digest_hex"] = DIGEST_2
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


def test_committee_result_count_must_match_unique_results(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = committee()
    payload["result_count"] += 1
    write_json(tmp_path / "committee.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["committee"]["artifacts"][0]
    assert "result_count must match unique results count" in artifact["errors"]


def test_committee_results_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = committee()
    payload["results"].append(dict(payload["results"][0]))
    payload["result_count"] = len(payload["results"])
    write_json(tmp_path / "committee.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["committee"]["artifacts"][0]
    assert "results must not contain duplicate values" in artifact["errors"]
    assert "result_count must match unique results count" in artifact["errors"]


def test_committee_results_must_use_production_family(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = committee()
    payload["results"][0]["name"] = "runner-result-a"
    write_json(tmp_path / "committee.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["committee"]["artifacts"][0]
    assert artifact["valid"] is False
    assert MODULE.COMMITTEE_RESULT_LABEL_ERROR in artifact["errors"]


def test_committee_results_reject_placeholder_marker(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = committee()
    payload["results"][0]["name"] = "ai-prescreen-committee-result-placeholder"
    write_json(tmp_path / "committee.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["committee"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "results[0].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_committee_results_reject_numbered_placeholder_marker(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = committee()
    payload["results"][0]["name"] = "ai-prescreen-committee-result-placeholder1"
    write_json(tmp_path / "committee.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["committee"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "results[0].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_committee_results_reject_compact_placeholder_marker(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = committee()
    payload["results"][0]["name"] = "ai-prescreen-committee-result-placeholderreview"
    write_json(tmp_path / "committee.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["committee"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "results[0].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_committee_results_reject_sandwiched_compact_placeholder_marker(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = committee()
    payload["results"][0]["name"] = (
        "ai-prescreen-committee-result-prodplaceholderreview"
    )
    write_json(tmp_path / "committee.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["committee"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "results[0].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
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


def test_operator_route_count_must_match_unique_routes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_workflow()
    payload["route_count"] += 1
    payload["passed_route_count"] = payload["route_count"]
    write_json(tmp_path / "operator-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["operator_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "route_count must match unique routes count" in artifact["errors"]


def test_operator_passed_route_count_must_match_route_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_workflow()
    payload["passed_route_count"] -= 1
    write_json(tmp_path / "operator-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["operator_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "passed_route_count must equal route_count" in artifact["errors"]


def test_operator_routes_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_workflow()
    payload["routes"].append(dict(payload["routes"][0]))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "operator-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["operator_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "routes must not contain duplicate values" in artifact["errors"]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_operator_routes_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_workflow()
    payload["routes"].append(operator_route("debug_console"))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "operator-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["operator_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "routes must not include unknown values" in artifact["errors"]


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


def test_operator_route_method_must_be_get(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_workflow()
    payload["routes"][0]["method"] = "POST"
    write_json(tmp_path / "operator-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["operator_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "routes[0].method must be `GET`" in artifact["errors"]


def test_operator_route_path_must_match_reviewed_route(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_workflow()
    payload["routes"][6]["path"] = MODULE.operator_route_paths(QUARANTINE_ID)[
        "juror_plan"
    ]
    write_json(tmp_path / "operator-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["operator_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    expected = MODULE.operator_route_paths(QUARANTINE_ID)["juror_notifications"]
    assert f"routes[6].path must be `{expected}`" in artifact["errors"]


def test_operator_route_url_must_match_reviewed_route(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_workflow()
    route_paths = MODULE.operator_route_paths(QUARANTINE_ID)
    payload["routes"][6]["url"] = MODULE.expected_operator_route_url(
        payload["operator_url"],
        route_paths["juror_plan"],
    )
    write_json(tmp_path / "operator-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["operator_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "routes[6].url must match operator_url and reviewed route path"
        in artifact["errors"]
    )


def test_operator_route_content_type_must_match_reviewed_route(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = operator_workflow()
    payload["routes"][2]["content_type"] = "application/json"
    write_json(tmp_path / "operator-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["operator_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "routes[2].content_type must be `text/html; charset=utf-8`"
        in artifact["errors"]
    )


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
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["notification_transport"]["artifacts"][0]
    assert "accepted_count must be a positive integer" in artifact["errors"]
    assert "accepted_count must be at least 1" in artifact["errors"]
    assert "accepted_count must equal probe_count" in artifact["errors"]


def test_notification_probe_count_must_match_unique_deliveries(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = notification_transport()
    payload["probe_count"] += 1
    payload["accepted_count"] = payload["probe_count"]
    write_json(tmp_path / "notification-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["notification_transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "probe_count must equal probes length" in artifact["errors"]
    assert "probe_count must match unique probes count" in artifact["errors"]
    assert "accepted_count must match unique probes count" in artifact["errors"]


def test_notification_probes_must_not_duplicate_delivery_id(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = notification_transport()
    payload["probes"].append(dict(payload["probes"][0]))
    payload["probe_count"] = len(payload["probes"])
    payload["accepted_count"] = len(payload["probes"])
    write_json(tmp_path / "notification-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["notification_transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "probes must not contain duplicate values" in artifact["errors"]
    assert "probe_count must match unique probes count" in artifact["errors"]
    assert "accepted_count must match unique probes count" in artifact["errors"]


def test_notification_delivery_ids_must_use_production_family(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = notification_transport()
    payload["probes"][0]["delivery_id"] = "notify-1"
    write_json(tmp_path / "notification-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["notification_transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert MODULE.NOTIFICATION_DELIVERY_LABEL_ERROR in artifact["errors"]


def test_notification_delivery_ids_reject_placeholder_marker(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = notification_transport()
    payload["probes"][0]["delivery_id"] = (
        "ai-prescreen-notification-delivery-placeholder"
    )
    write_json(tmp_path / "notification-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["notification_transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "probes[0].delivery_id must not contain non-production markers "
        "['placeholder']"
        in artifact["errors"]
    )


def test_notification_probe_identity_fields_are_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = notification_transport()
    del payload["probes"][0]["dedup_key"]
    payload["probes"][0]["action"] = "notify"
    payload["probes"][0]["case_id"] = ""
    payload["probes"][0]["round_id"] = 17
    payload["probes"][0]["juror_id"] = None
    write_json(tmp_path / "notification-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["notification_transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "probes[0].dedup_key must be a string" in artifact["errors"]
    assert (
        "probes[0].action must be `submit_commit` or `submit_reveal`"
        in artifact["errors"]
    )
    assert (
        "probes[0].case_id must be a non-empty canonical string"
        in artifact["errors"]
    )
    assert "probes[0].round_id must be a string" in artifact["errors"]
    assert "probes[0].juror_id must be a string" in artifact["errors"]


@pytest.mark.parametrize("legacy_action", ("commit", "reveal"))
def test_notification_probe_rejects_legacy_short_action_labels(
    tmp_path: Path, legacy_action: str
) -> None:
    write_complete_evidence(tmp_path)
    payload = notification_transport()
    payload["probes"][0]["action"] = legacy_action
    write_json(tmp_path / "notification-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["notification_transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "probes[0].action must be `submit_commit` or `submit_reveal`"
        in artifact["errors"]
    )


def test_notification_transport_requires_both_shipped_actions(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = notification_transport()
    for probe in payload["probes"]:
        probe["action"] = "submit_commit"
    write_json(tmp_path / "notification-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["notification_transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "probes must include action `submit_reveal`" in artifact["errors"]


def test_notification_dedup_key_must_bind_delivery_id(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = notification_transport()
    payload["probes"][0]["dedup_key"] = (
        "sorafs-moderation-juror:ai-prescreen-notification-delivery-02"
    )
    write_json(tmp_path / "notification-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["notification_transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "probes[0].dedup_key must equal "
        "`sorafs-moderation-juror:<delivery_id>`"
    ) in artifact["errors"]


def test_notification_transport_byte_counts_are_validated(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = notification_transport()
    payload["probes"][0]["notification_bytes"] = 0
    payload["probes"][0]["response_bytes"] = -1
    write_json(tmp_path / "notification-transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["notification_transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "notification_bytes must be a positive integer" in artifact["errors"]
    assert "response_bytes must be a non-negative integer" in artifact["errors"]


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


def test_executor_artifact_count_must_match_unique_artifacts(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = commit_reveal_executor()
    payload["artifact_count"] += 1
    payload["passed_artifact_count"] = payload["artifact_count"]
    write_json(tmp_path / "commit-reveal-executor.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["commit_reveal_executor"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "artifact_count must equal artifacts length" in artifact["errors"]
    assert "artifact_count must match unique artifacts count" in artifact["errors"]


def test_executor_artifacts_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = commit_reveal_executor()
    payload["artifacts"].append(dict(payload["artifacts"][0]))
    payload["artifact_count"] = len(payload["artifacts"])
    payload["passed_artifact_count"] = len(payload["artifacts"])
    write_json(tmp_path / "commit-reveal-executor.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["commit_reveal_executor"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "artifacts must not contain duplicate values" in artifact["errors"]
    assert "artifact_count must match unique artifacts count" in artifact["errors"]


def test_executor_artifacts_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = commit_reveal_executor()
    payload["artifacts"].append({
        "name": "debug.log",
        "kind": "log",
        "path": "debug.log",
        "exists": True,
        "bytes": 16,
        "body_blake3": DIGEST,
        "passed": True,
        "checks": [{"name": "payload-free", "passed": True}],
        "payload_bytes_included": False,
        "private_payloads_included": False,
    })
    payload["artifact_count"] = len(payload["artifacts"])
    payload["passed_artifact_count"] = len(payload["artifacts"])
    write_json(tmp_path / "commit-reveal-executor.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["commit_reveal_executor"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "artifacts must not include unknown values" in artifact["errors"]


def test_executor_artifact_kind_must_match_reviewed_name(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = commit_reveal_executor()
    payload["artifacts"][1]["kind"] = "env"
    write_json(tmp_path / "commit-reveal-executor.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["commit_reveal_executor"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "artifacts[1].kind must be `script`" in artifact["errors"]


def test_executor_artifact_path_must_match_reviewed_name(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = commit_reveal_executor()
    payload["artifacts"][0]["path"] = "run.sh"
    write_json(tmp_path / "commit-reveal-executor.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["commit_reveal_executor"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "artifacts[0].path must be `executor.env`" in artifact["errors"]


def test_executor_artifacts_must_cover_required_bundle_files(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = commit_reveal_executor()
    payload["artifacts"] = [
        artifact for artifact in payload["artifacts"] if artifact["name"] != "run.sh"
    ]
    payload["artifact_count"] = len(payload["artifacts"])
    payload["passed_artifact_count"] = len(payload["artifacts"])
    write_json(tmp_path / "commit-reveal-executor.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["commit_reveal_executor"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "artifact_count must be at least 2" in artifact["errors"]
    assert "artifacts must include name `run.sh`" in artifact["errors"]


def test_executor_service_name_must_match_reviewed_service(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = commit_reveal_executor()
    payload["service_name"] = "sorafs-moderation-ballots-debug"
    write_json(tmp_path / "commit-reveal-executor.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["commit_reveal_executor"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "service_name must be `sorafs-moderation-ballots-executor`"
        in artifact["errors"]
    )


def test_executor_summary_digest_must_match_summary_body(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = commit_reveal_executor()
    payload["execution_summary_digest_hex"] = DIGEST_2
    write_json(tmp_path / "commit-reveal-executor.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["commit_reveal_executor"]["artifacts"][0]
    assert artifact["valid"] is False
    assert payload["valid_executor_summary_digests"] == []
    assert (
        "execution_summary.body_blake3 must match execution_summary_digest_hex"
        in artifact["errors"]
    )


def test_executor_bundle_and_summary_byte_counts_are_validated(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = commit_reveal_executor()
    payload["bundle_metadata_bytes"] = 0
    payload["bundle_metadata_blake3"] = "not-hex"
    payload["interval_secs"] = 0
    payload["artifacts"][0]["bytes"] = 0
    payload["execution_summary"]["bytes"] = 0
    payload["execution_summary"]["commit_action_count"] = -1
    payload["execution_summary"]["reveal_action_count"] = 0
    payload["execution_summary"]["tally_action_count"] = 0
    write_json(tmp_path / "commit-reveal-executor.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["commit_reveal_executor"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "bundle_metadata_bytes must be a positive integer" in artifact["errors"]
    assert "bundle_metadata_blake3 must be 64 lowercase hex characters" in artifact["errors"]
    assert "interval_secs must be a positive integer" in artifact["errors"]
    assert "bytes must be a positive integer" in artifact["errors"]
    assert "commit_action_count must be a non-negative integer" in artifact["errors"]
    assert (
        "execution_summary commit/reveal/tally counts must sum to action_count"
        in artifact["errors"]
    )


def test_transparency_publication_requires_moderation_source_kinds(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "transparency-publication.json",
        transparency_publication(missing_source="moderation-juror-notifications-canary"),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transparency_publication"]["artifacts"][0]
    assert "source_entry_probe_count must be at least 8" in artifact["errors"]
    assert (
        "probes must include source_kind `moderation-juror-notifications-canary`"
        in artifact["errors"]
    )


def test_transparency_probe_count_must_match_unique_source_kinds(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = transparency_publication()
    payload["probe_count"] += 1
    payload["passed_probe_count"] = payload["probe_count"]
    payload["source_entry_probe_count"] = payload["probe_count"]
    write_json(tmp_path / "transparency-publication.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transparency_publication"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "probe_count must equal probes length" in artifact["errors"]
    assert "probe_count must match unique probes count" in artifact["errors"]
    assert (
        "source_entry_probe_count must match unique probes count"
        in artifact["errors"]
    )


def test_transparency_probes_must_not_duplicate_source_kind(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = transparency_publication()
    payload["probes"].append(dict(payload["probes"][0]))
    payload["probe_count"] = len(payload["probes"])
    payload["passed_probe_count"] = len(payload["probes"])
    payload["source_entry_probe_count"] = len(payload["probes"])
    write_json(tmp_path / "transparency-publication.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transparency_publication"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "probes must not contain duplicate values" in artifact["errors"]
    assert "probe_count must match unique probes count" in artifact["errors"]
    assert (
        "source_entry_probe_count must match unique probes count"
        in artifact["errors"]
    )


def test_transparency_source_kinds_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = transparency_publication()
    payload["probes"].append({
        "source_kind": "moderation-debug-source",
        "payload_path": "moderation-debug-source.json",
        "request_bytes": 128,
        "request_body_blake3": DIGEST,
        "response_status": 201,
        "response_success": True,
        "response_bytes": 16,
        "response_body_blake3": DIGEST,
        "payload_bytes_included": False,
        "private_payloads_included": False,
        "response_body_included": False,
    })
    payload["probe_count"] = len(payload["probes"])
    payload["passed_probe_count"] = len(payload["probes"])
    payload["source_entry_probe_count"] = len(payload["probes"])
    write_json(tmp_path / "transparency-publication.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transparency_publication"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "probes must not include unknown values" in artifact["errors"]


def test_transparency_source_entry_probe_count_must_match_probe_count(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = transparency_publication()
    payload["source_entry_probe_count"] -= 1
    write_json(tmp_path / "transparency-publication.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transparency_publication"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "source_entry_probe_count must equal probe_count" in artifact["errors"]


def test_transparency_publication_payload_free_flags_are_required(
    tmp_path: Path,
) -> None:
    for field in (
        "payload_bytes_included",
        "private_payloads_included",
        "response_bodies_included",
    ):
        root = tmp_path / field
        root.mkdir()
        write_complete_evidence(root)
        payload = transparency_publication()
        del payload[field]
        write_json(root / "transparency-publication.json", payload)
        summary = root / "summary.json"

        assert run_gate(root, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"]["transparency_publication"]["artifacts"][0]
        assert artifact["valid"] is False
        assert f"{field} must be false" in artifact["errors"]


def test_transparency_probe_byte_counts_are_validated(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = transparency_publication()
    payload["probes"][0]["request_bytes"] = 0
    payload["probes"][0]["response_bytes"] = -1
    write_json(tmp_path / "transparency-publication.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transparency_publication"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "request_bytes must be a positive integer" in artifact["errors"]
    assert "response_bytes must be a non-negative integer" in artifact["errors"]


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
    assert "policy_digest_hex must be 64 lowercase hex characters" in artifact["errors"]


def test_governance_dag_policy_digest_must_match_runner(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_dag()
    payload["policy_digest_hex"] = DIGEST_2
    write_json(tmp_path / "governance-dag.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["required"]["governance_dag"]["valid"] is False
    assert payload["required"]["governance_dag"]["artifacts"][0]["valid"] is False
    assert payload["valid_policy_digests"] == [DIGEST]
    errors = "\n".join(payload["errors"])
    assert (
        "governance_dag policy_digest_hex must match a valid "
        "runner policy_digest_hex"
        in errors
    )


def test_governance_producer_count_must_match_unique_producers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_dag()
    payload["producer_count"] += 1
    write_json(tmp_path / "governance-dag.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_dag"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "producer_count must equal producers length" in artifact["errors"]
    assert "producer_count must match unique producers count" in artifact["errors"]


def test_governance_producers_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_dag()
    payload["producers"].append(dict(payload["producers"][0]))
    payload["producer_count"] = len(payload["producers"])
    write_json(tmp_path / "governance-dag.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_dag"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "producers must not contain duplicate values" in artifact["errors"]
    assert "producer_count must match unique producers count" in artifact["errors"]


def test_governance_producers_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_dag()
    payload["producers"].append({"name": "debug_producer"})
    payload["producer_count"] = len(payload["producers"])
    write_json(tmp_path / "governance-dag.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_dag"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "producers must not include unknown values" in artifact["errors"]


def test_governance_edge_count_must_match_unique_edges(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_dag()
    payload["edge_count"] += 1
    write_json(tmp_path / "governance-dag.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_dag"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "edge_count must match unique edges count" in artifact["errors"]


def test_governance_edge_count_must_match_required_producer_inventory(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_dag()
    payload["edges"].append(
        {
            "producer": "screening_ingest",
            "name": "ai-prescreen-governance-edge-screening-ingest-extra",
        }
    )
    payload["edge_count"] = len(payload["edges"])
    write_json(tmp_path / "governance-dag.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_dag"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "edge_count must equal required governance producer inventory"
        in artifact["errors"]
    )


def test_governance_edges_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_dag()
    payload["edges"].append(dict(payload["edges"][0]))
    payload["edge_count"] = len(payload["edges"])
    write_json(tmp_path / "governance-dag.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_dag"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "edges must not contain duplicate values" in artifact["errors"]
    assert "edge_count must match unique edges count" in artifact["errors"]


def test_governance_edges_must_use_production_family(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_dag()
    payload["edges"][0]["name"] = "screening-ingest-edge"
    write_json(tmp_path / "governance-dag.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_dag"]["artifacts"][0]
    assert artifact["valid"] is False
    assert MODULE.GOVERNANCE_EDGE_LABEL_ERROR in artifact["errors"]


def test_governance_edges_reject_placeholder_marker(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_dag()
    payload["edges"][0]["name"] = "ai-prescreen-governance-edge-placeholder"
    write_json(tmp_path / "governance-dag.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_dag"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "edges[0].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_governance_edges_must_cover_required_producers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_dag()
    payload["edges"] = payload["edges"][:-1]
    payload["edge_count"] = len(payload["edges"])
    write_json(tmp_path / "governance-dag.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_dag"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "edges must include producer `transparency_publication`"
        in artifact["errors"]
    )


def test_governance_edges_must_use_required_producers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_dag()
    payload["edges"][0]["producer"] = "unknown"
    write_json(tmp_path / "governance-dag.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_dag"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "edges producer must be one of required producers" in artifact["errors"]


def test_e2e_workflow_requires_full_path(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "end-to-end-workflow.json",
        end_to_end_workflow(omit_step="release"),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["end_to_end_workflow"]["artifacts"][0]
    assert "step_count must be at least 9" in artifact["errors"]
    assert "steps must include name `release`" in artifact["errors"]


def test_e2e_workflow_id_must_be_canonical(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = end_to_end_workflow()
    payload["workflow_id"] = "sfm_4a_prod_canary_20260701"
    write_json(tmp_path / "end-to-end-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["end_to_end_workflow"]["artifacts"][0]
    assert MODULE.WORKFLOW_ID_ERROR in artifact["errors"]


def test_e2e_workflow_id_rejects_non_production_markers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = end_to_end_workflow()
    payload["workflow_id"] = "sfm-4a-prod-placeholder"
    write_json(tmp_path / "end-to-end-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["end_to_end_workflow"]["artifacts"][0]
    assert (
        "workflow_id must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_e2e_workflow_id_accepts_future_production_label(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = end_to_end_workflow()
    payload["workflow_id"] = "sfm-4a-prod-canary-20260701"
    write_json(tmp_path / "end-to-end-workflow.json", payload)

    assert run_gate(tmp_path) == 0


def test_e2e_step_count_must_match_unique_steps(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = end_to_end_workflow()
    payload["step_count"] += 1
    payload["passed_step_count"] = payload["step_count"]
    write_json(tmp_path / "end-to-end-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["end_to_end_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "step_count must equal steps length" in artifact["errors"]
    assert "step_count must match unique steps count" in artifact["errors"]


def test_e2e_steps_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = end_to_end_workflow()
    payload["steps"].append(dict(payload["steps"][0]))
    payload["step_count"] = len(payload["steps"])
    payload["passed_step_count"] = len(payload["steps"])
    write_json(tmp_path / "end-to-end-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["end_to_end_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "steps must not contain duplicate values" in artifact["errors"]
    assert "step_count must match unique steps count" in artifact["errors"]


def test_e2e_steps_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = end_to_end_workflow()
    payload["steps"].append({"name": "debug_step", "passed": True})
    payload["step_count"] = len(payload["steps"])
    payload["passed_step_count"] = len(payload["steps"])
    write_json(tmp_path / "end-to-end-workflow.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["end_to_end_workflow"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "steps must not include unknown values" in artifact["errors"]


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

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence", str(path)]) == 1


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

    assert CHECKER(["--now-unix", str(NOW_UNIX), f"@{args_file}"]) == 0


def test_invalid_optional_recognized_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "runner.json", runner())
    write_json(tmp_path / "committee.json", committee(status="failed"))

    assert run_gate(tmp_path, "--require-kind", "runner") == 1
