"""Tests for scripts/check_sorafs_moderation_panel_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_moderation_panel_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_moderation_panel_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


NOW_UNIX = 1_800_300_000
GENERATED_AT = NOW_UNIX - 120
DIGEST = "ab" * 32
DIGEST_2 = "cd" * 32
DEPLOYMENT_ID = "moderation-panel-staging-a"
ENVIRONMENT = "staging"


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def with_context(payload: dict) -> dict:
    payload["deployment_id"] = DEPLOYMENT_ID
    payload["environment"] = ENVIRONMENT
    payload["deployment_context_reviewed"] = True
    return payload


def route(name: str) -> dict:
    return {
        "name": name,
        "passed": True,
        "status_code": 200,
        "authz_enforced": True,
        "signature_verified": True,
        "latency_ms": 40,
    }


def appeal_intake() -> dict:
    routes = [route(name) for name in ("appeal_submit", "case_status", "deposit_quote", "deposit_confirm")]
    return with_context({
        "schema": "sorafs.moderation_panel.appeal_intake_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "case_digest_hex": DIGEST,
        "route_count": len(routes),
        "passed_route_count": len(routes),
        "routes": routes,
        "case_count": 2,
        "accepted_case_count": 2,
        "appellant_auth_enforced": True,
        "proof_token_verified": True,
        "deposit_confirmation_bound": True,
        "policy_reference_bound": True,
        "duplicate_case_rejected": True,
        "invalid_payload_rejected": True,
        "payloads_included": False,
        "response_bodies_included": False,
    })


def sortition_roster(*, panel_size: int = 7) -> dict:
    return with_context({
        "schema": "sorafs.moderation_panel.sortition_roster_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "case_digest_hex": DIGEST,
        "pop_snapshot_digest_hex": DIGEST,
        "roster_hash_hex": DIGEST,
        "sortition_seed_hex": DIGEST,
        "panel_size": panel_size,
        "quorum": 5,
        "pop_snapshot_bound": True,
        "juror_eligibility_verified": True,
        "failover_plan_present": True,
        "roster_privacy_preserved": True,
        "juror_private_data_included": False,
    })


def evidence_viewer() -> dict:
    return with_context({
        "schema": "sorafs.moderation_panel.evidence_viewer_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "case_digest_hex": DIGEST,
        "roster_hash_hex": DIGEST,
        "session_count": 3,
        "attested_session_count": 3,
        "logged_session_count": 3,
        "attested_viewer_enabled": True,
        "role_scoped_manifest_verified": True,
        "short_lived_urls_verified": True,
        "session_key_workflow_verified": True,
        "strict_csp_enforced": True,
        "offline_mode_disabled": True,
        "per_session_access_logged": True,
        "append_only_log_verified": True,
        "anomaly_events_recorded": True,
        "watermark_overlay_rendered": True,
        "watermark_metadata_hashed": True,
        "audit_digest_exported": True,
        "transparency_report_exported": True,
        "daily_digest_published": True,
        "payload_redaction_verified": True,
        "denylisted_digest_blocked": True,
        "unauthorized_access_rejected": True,
        "stale_url_rejected": True,
        "session_replay_rejected": True,
        "legal_hold_policy_bound": True,
        "max_url_ttl_secs": 300,
        "roles_tested": ["juror", "auditor", "legal_reviewer"],
        "viewer_security_controls": [
            "strict_csp",
            "offline_mode_disabled",
            "short_lived_urls",
            "role_scoped_manifest",
            "watermark_overlay",
        ],
        "access_event_kinds": [
            "view",
            "seek",
            "pause",
            "screenshot_attempt",
            "download_attempt",
            "annotation",
        ],
        "export_targets": ["governance_dag", "transparency_ledger"],
        "session_manifest_digest_hex": DIGEST,
        "watermark_metadata_digest_hex": DIGEST,
        "access_log_digest_hex": DIGEST,
        "legal_hold_receipt_digest_hex": DIGEST,
        "transparency_report_digest_hex": DIGEST,
        "audit_digest_hex": DIGEST,
        "raw_evidence_included": False,
        "session_tokens_included": False,
        "signed_urls_included": False,
        "watermark_secrets_included": False,
        "response_bodies_included": False,
    })


def operator_workflow() -> dict:
    routes = [route(name) for name in ("operator_panel", "bridge_plan", "juror_plan", "commit_reveal_status")]
    return with_context({
        "schema": "sorafs.moderation_panel.operator_workflow_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "case_digest_hex": DIGEST,
        "roster_hash_hex": DIGEST,
        "route_count": len(routes),
        "passed_route_count": len(routes),
        "routes": routes,
        "operator_role_enforced": True,
        "bridge_plan_generated": True,
        "juror_plan_generated": True,
        "mutation_forwarding_signed": True,
        "payload_bytes_rejected": True,
        "response_bodies_included": False,
    })


def juror_notifications() -> dict:
    return with_context({
        "schema": "sorafs.moderation_panel.juror_notifications_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "case_digest_hex": DIGEST,
        "roster_hash_hex": DIGEST,
        "notification_count": 7,
        "delivered_notification_count": 7,
        "juror_count": 7,
        "dedup_keys_verified": True,
        "transport_canary_passed": True,
        "retry_policy_verified": True,
        "private_payloads_rejected": True,
        "message_bodies_included": False,
        "response_bodies_included": False,
    })


def commit_reveal(*, lag: int = 60) -> dict:
    routes = [
        route("ballot_announce"),
        route("ballot_commit"),
        route("ballot_reveal"),
        route("ballot_tally"),
        route("ballot_events"),
    ]
    return with_context({
        "schema": "sorafs.moderation_panel.commit_reveal_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "case_digest_hex": DIGEST,
        "roster_hash_hex": DIGEST,
        "route_count": len(routes),
        "passed_route_count": len(routes),
        "routes": routes,
        "panel_size": 7,
        "commit_count": 7,
        "reveal_count": 7,
        "commit_auth_bound_to_juror": True,
        "reveal_auth_bound_to_juror": True,
        "quorum_satisfied": True,
        "challenge_buffer_enforced": True,
        "contested_tie_detected": True,
        "commit_digest_recomputed": True,
        "duplicate_commit_rejected": True,
        "mismatched_reveal_rejected": True,
        "late_commit_rejected": True,
        "late_reveal_rejected": True,
        "missed_quorum_detected": True,
        "no_show_failover_exercised": True,
        "juror_penalty_plan_emitted": True,
        "tally_deterministic_replay_verified": True,
        "governance_event_digest_bound": True,
        "executor_canary_passed": True,
        "max_event_lag_seconds": lag,
        "scenarios_exercised": [
            "happy_path",
            "duplicate_commit",
            "mismatched_reveal",
            "late_commit",
            "late_reveal",
            "missed_quorum",
            "no_show_failover",
            "contested_challenge",
        ],
        "tally_digest_hex": DIGEST,
        "commit_payloads_included": False,
        "reveal_payloads_included": False,
    })


def decision_publication() -> dict:
    routes = [route(name) for name in ("decision_publish", "decision_status", "challenge_status")]
    return with_context({
        "schema": "sorafs.moderation_panel.decision_publication_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "case_digest_hex": DIGEST,
        "roster_hash_hex": DIGEST,
        "tally_digest_hex": DIGEST,
        "route_count": len(routes),
        "passed_route_count": len(routes),
        "routes": routes,
        "outcomes": ["uphold", "overturn", "modify", "escalate"],
        "decision_signature_verified": True,
        "governance_dag_event_published": True,
        "public_decision_trail_published": True,
        "challenge_dag_bound": True,
        "raw_decision_included": False,
    })


def settlement_integration() -> dict:
    return with_context({
        "schema": "sorafs.moderation_panel.settlement_integration_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "case_digest_hex": DIGEST,
        "roster_hash_hex": DIGEST,
        "tally_digest_hex": DIGEST,
        "settlement_count": 2,
        "appeal_finance_report_published": True,
        "settlement_receipt_published": True,
        "treasury_reconciliation_passed": True,
        "no_show_penalties_applied": True,
        "reputation_penalty_handoff_present": True,
        "signed_transaction_included": False,
        "raw_ledger_included": False,
    })


def transparency_reputation() -> dict:
    return with_context({
        "schema": "sorafs.moderation_panel.transparency_reputation_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "case_digest_hex": DIGEST,
        "roster_hash_hex": DIGEST,
        "tally_digest_hex": DIGEST,
        "publication_targets": [
            "governance_dag",
            "transparency_ledger",
            "moderation_cache",
            "appeal_finance",
            "reputation",
        ],
        "moderation_cache_updated": True,
        "transparency_source_entry_published": True,
        "privacy_aggregate_updated": True,
        "reputation_delta_applied": True,
        "gateway_compliance_cache_updated": True,
        "payloads_included": False,
    })


def e2e_panel(*, peer_count: int = 4) -> dict:
    return with_context({
        "schema": "sorafs.moderation_panel.e2e_panel_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "case_digest_hex": DIGEST,
        "roster_hash_hex": DIGEST,
        "tally_digest_hex": DIGEST,
        "peer_count": peer_count,
        "validator_count": peer_count,
        "case_count": 2,
        "appeal_submission_verified": True,
        "juror_selection_verified": True,
        "evidence_access_verified": True,
        "commit_reveal_verified": True,
        "decision_publication_verified": True,
        "settlement_verified": True,
        "all_peers_reconciled": True,
        "unexpected_failure_count": 0,
        "raw_evidence_included": False,
    })


def metrics_alerts() -> dict:
    return with_context({
        "schema": "sorafs.moderation_panel.metrics_alert_canary.v1",
        "status": "passed",
        "case_digest_hex": DIGEST,
        "roster_hash_hex": DIGEST,
        "tally_digest_hex": DIGEST,
        "metrics_scrape_success": True,
        "dashboard_provisioned": True,
        "alert_rules_installed": True,
        "critical_alerts_firing": False,
        "metrics": [
            "sorafs_moderation_panel_case_total",
            "sorafs_moderation_panel_commit_total",
            "sorafs_moderation_panel_reveal_total",
            "sorafs_moderation_panel_tally_total",
            "sorafs_moderation_panel_decision_lag_seconds",
            "sorafs_moderation_panel_no_show_total",
        ],
        "response_bodies_included": False,
    })


def governance_approval() -> dict:
    return with_context({
        "schema": "sorafs.moderation_panel.governance_approval.v1",
        "status": "passed",
        "case_digest_hex": DIGEST,
        "roster_hash_hex": DIGEST,
        "tally_digest_hex": DIGEST,
        "approved": True,
        "governance_vote_recorded": True,
        "iroha_config_bound": True,
        "appeal_intake_policy_present": True,
        "sortition_policy_present": True,
        "evidence_access_policy_present": True,
        "commit_reveal_policy_present": True,
        "settlement_policy_present": True,
        "public_decision_policy_present": True,
        "e2e_panel_evidence_accepted": True,
        "config_source": "iroha_config",
        "policy_digest_hex": DIGEST,
    })


def write_complete_evidence(root: Path) -> None:
    write_json(root / "appeal-intake.json", appeal_intake())
    write_json(root / "sortition-roster.json", sortition_roster())
    write_json(root / "evidence-viewer.json", evidence_viewer())
    write_json(root / "operator-workflow.json", operator_workflow())
    write_json(root / "juror-notifications.json", juror_notifications())
    write_json(root / "commit-reveal.json", commit_reveal())
    write_json(root / "decision-publication.json", decision_publication())
    write_json(root / "settlement-integration.json", settlement_integration())
    write_json(root / "transparency-reputation.json", transparency_reputation())
    write_json(root / "e2e-panel.json", e2e_panel())
    write_json(root / "metrics-alerts.json", metrics_alerts())
    write_json(root / "governance-approval.json", governance_approval())


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.moderation_panel.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["e2e_panel"]["valid"] is True
    assert payload["valid_case_digests"] == [DIGEST]
    assert payload["valid_roster_bindings"] == [
        {
            "case_digest_hex": DIGEST,
            "roster_hash_hex": DIGEST,
        }
    ]
    assert payload["valid_tally_bindings"] == [
        {
            "case_digest_hex": DIGEST,
            "roster_hash_hex": DIGEST,
            "tally_digest_hex": DIGEST,
        }
    ]
    assert payload["required"]["appeal_intake"]["artifacts"][0]["fingerprint"][
        "deployment_id"
    ] == DEPLOYMENT_ID
    assert payload["deployment_context"] == {
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
    }
    assert len(payload["valid_e2e_runs"]) == 1


def test_deployment_context_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = appeal_intake()
    del payload["deployment_id"]
    write_json(tmp_path / "appeal-intake.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["appeal_intake"]["artifacts"][0]
    assert "deployment_id must be a non-empty string" in artifact["errors"]


def test_unreviewed_deployment_context_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_approval()
    payload["deployment_id"] = "moderation-panel-dev-a"
    payload["environment"] = "dev"
    write_json(tmp_path / "governance-approval.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact_errors = result["required"]["governance_approval"]["artifacts"][0][
        "errors"
    ]
    assert (
        "deployment_id must not contain non-reviewed deployment markers ['dev']"
        in artifact_errors
    )
    assert "environment must be one of" in "\n".join(artifact_errors)


def test_mixed_reviewed_deployment_context_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = e2e_panel()
    payload["deployment_id"] = "moderation-panel-staging-b"
    write_json(tmp_path / "e2e-panel.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["deployment_context"] == {
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
    }
    assert (
        "e2e_panel.deployment_id `moderation-panel-staging-b` does not match "
        f"`{DEPLOYMENT_ID}`"
        in result["errors"]
    )


def test_missing_e2e_panel_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "e2e-panel.json").unlink()

    assert run_gate(tmp_path) == 1


def test_stale_appeal_intake_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = appeal_intake()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_CANARY_AGE_SECS - 1
    write_json(tmp_path / "appeal-intake.json", payload)

    assert run_gate(tmp_path) == 1


def test_payload_leakage_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = commit_reveal()
    payload["reveal_payload"] = {"vote": "overturn"}
    write_json(tmp_path / "commit-reveal.json", payload)

    assert run_gate(tmp_path) == 1


def test_evidence_viewer_rejects_long_lived_urls(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = evidence_viewer()
    payload["max_url_ttl_secs"] = MODULE.DEFAULT_MAX_VIEWER_URL_TTL_SECS + 1
    write_json(tmp_path / "evidence-viewer.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["evidence_viewer"]["artifacts"][0]
    assert (
        f"max_url_ttl_secs must be <= {MODULE.DEFAULT_MAX_VIEWER_URL_TTL_SECS}"
        in artifact["errors"]
    )


def test_evidence_viewer_logged_session_count_must_match(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = evidence_viewer()
    payload["logged_session_count"] = 2
    write_json(tmp_path / "evidence-viewer.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["evidence_viewer"]["artifacts"][0]
    assert "logged_session_count must equal session_count" in artifact["errors"]


def test_evidence_viewer_requires_access_event_coverage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = evidence_viewer()
    payload["access_event_kinds"] = ["view", "seek"]
    write_json(tmp_path / "evidence-viewer.json", payload)

    assert run_gate(tmp_path) == 1


def test_evidence_viewer_requires_auditable_digest_coverage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = evidence_viewer()
    del payload["access_log_digest_hex"]
    write_json(tmp_path / "evidence-viewer.json", payload)

    assert run_gate(tmp_path) == 1


def test_evidence_viewer_requires_transparency_export_targets(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = evidence_viewer()
    payload["export_targets"] = ["governance_dag"]
    write_json(tmp_path / "evidence-viewer.json", payload)

    assert run_gate(tmp_path) == 1


def test_evidence_viewer_rejects_raw_access_log_leakage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = evidence_viewer()
    payload["raw_access_log"] = [{"event": "view", "subject": "juror-a"}]
    write_json(tmp_path / "evidence-viewer.json", payload)

    assert run_gate(tmp_path) == 1


def test_evidence_viewer_rejects_signed_url_leakage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = evidence_viewer()
    payload["signed_url"] = "https://provider.example/evidence?sig=runtime-secret"
    write_json(tmp_path / "evidence-viewer.json", payload)

    assert run_gate(tmp_path) == 1


def test_low_panel_size_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "sortition-roster.json", sortition_roster(panel_size=5))

    assert run_gate(tmp_path) == 1


def test_sortition_roster_quorum_must_not_exceed_panel_size(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = sortition_roster(panel_size=7)
    payload["quorum"] = 8
    write_json(tmp_path / "sortition-roster.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["sortition_roster"]["artifacts"][0]
    assert "quorum must be <= panel_size" in artifact["errors"]


def test_sortition_roster_requires_case_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = sortition_roster()
    payload.pop("case_digest_hex")
    write_json(tmp_path / "sortition-roster.json", payload)

    assert run_gate(tmp_path) == 1


def test_invalid_sortition_roster_does_not_anchor_downstream_evidence(
    tmp_path: Path,
) -> None:
    write_json(tmp_path / "appeal-intake.json", appeal_intake())
    roster = sortition_roster()
    roster["case_digest_hex"] = DIGEST_2
    write_json(tmp_path / "sortition-roster.json", roster)
    voting = commit_reveal()
    voting["case_digest_hex"] = DIGEST_2
    write_json(tmp_path / "commit-reveal.json", voting)
    decision = decision_publication()
    decision["case_digest_hex"] = DIGEST_2
    write_json(tmp_path / "decision-publication.json", decision)
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-kind",
            "appeal_intake,sortition_roster,commit_reveal,decision_publication",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["valid_case_digests"] == [DIGEST]
    assert payload["valid_roster_bindings"] == []
    assert payload["valid_tally_bindings"] == []
    assert payload["required"]["sortition_roster"]["valid"] is False
    assert payload["required"]["commit_reveal"]["valid"] is False
    assert payload["required"]["decision_publication"]["valid"] is False
    errors = "\n".join(payload["errors"])
    assert "sortition_roster case_digest_hex must match a valid appeal_intake" in errors
    assert (
        "commit_reveal case_digest_hex and roster_hash_hex must match a valid "
        "case-bound sortition_roster artifact"
        in errors
    )
    assert (
        "decision_publication case_digest_hex, roster_hash_hex, and tally_digest_hex "
        "must match a valid roster-bound commit_reveal artifact"
        in errors
    )


def test_commit_reveal_roster_binding_must_match_sortition(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = commit_reveal()
    payload["roster_hash_hex"] = DIGEST_2
    write_json(tmp_path / "commit-reveal.json", payload)

    assert run_gate(tmp_path) == 1


def test_decision_publication_tally_binding_must_match_commit_reveal(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = decision_publication()
    payload["tally_digest_hex"] = DIGEST_2
    write_json(tmp_path / "decision-publication.json", payload)

    assert run_gate(tmp_path) == 1


def test_high_event_lag_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "commit-reveal.json", commit_reveal(lag=2_000))

    assert run_gate(tmp_path) == 1


def test_commit_reveal_requires_mismatched_reveal_rejection(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = commit_reveal()
    payload["mismatched_reveal_rejected"] = False
    write_json(tmp_path / "commit-reveal.json", payload)

    assert run_gate(tmp_path) == 1


def test_commit_reveal_requires_negative_scenario_coverage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = commit_reveal()
    payload["scenarios_exercised"] = ["happy_path", "duplicate_commit"]
    write_json(tmp_path / "commit-reveal.json", payload)

    assert run_gate(tmp_path) == 1


def test_e2e_peer_count_below_minimum_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "e2e-panel.json", e2e_panel(peer_count=3))

    assert run_gate(tmp_path) == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(
        tmp_path / "unknown.json",
        {"schema": "sorafs.moderation_panel.unknown.v1"},
    )

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_unknown_directory_artifact_is_ignored_for_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "appeal-intake.json", appeal_intake())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.moderation_panel.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "appeal_intake") == 0


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "appeal-intake.json", appeal_intake())
    invalid = metrics_alerts()
    invalid["critical_alerts_firing"] = True
    write_json(tmp_path / "metrics-alerts.json", invalid)

    assert run_gate(tmp_path, "--require-kind", "appeal_intake") == 1


def test_response_file_complete_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    args = tmp_path / "args.txt"
    args.write_text(
        "\n".join(
            [
                "# response file for the moderation panel gate",
                "--evidence-dir",
                str(tmp_path),
                "--summary-out",
                str(summary),
                "--now-unix",
                str(NOW_UNIX),
            ]
        ),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args}"]) == 0
    assert json.loads(summary.read_text(encoding="utf-8"))["status"] == "ready"


def test_unknown_required_kind_returns_usage_error(tmp_path: Path) -> None:
    write_json(tmp_path / "appeal-intake.json", appeal_intake())

    assert run_gate(tmp_path, "--require-kind", "unknown_kind") == 2
