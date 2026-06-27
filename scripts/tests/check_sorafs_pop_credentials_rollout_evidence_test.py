"""Tests for scripts/check_sorafs_pop_credentials_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_pop_credentials_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_pop_credentials_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


NOW = 1_800_006_000
HEX = "ab" * 32
HEX_2 = "cd" * 32
DEPLOYMENT_ID = "pop-staging-a"
ENVIRONMENT = "staging"


def write_json(path: Path, payload: dict[str, object]) -> Path:
    path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
    return path


def route(name: str) -> dict[str, object]:
    return {
        "name": name,
        "passed": True,
        "status_code": 200,
        "authz_enforced": True,
        "signature_verified": True,
    }


def complete_payloads() -> dict[str, dict[str, object]]:
    payloads = {
        "issuer_bundle": {
            "schema": "sorafs.pop.issuer_bundle_canary.v1",
            "status": "passed",
            "issuer_id": "issuer-a",
            "bundle_id_hex": HEX,
            "root_digest_hex": HEX,
            "revocation_list_digest_hex": HEX_2,
            "credential_count": 3,
            "signed_credential_count": 3,
            "canonical_norito_verified": True,
            "issuer_signature_verified": True,
            "issuer_key_policy_verified": True,
            "credential_payloads_included": False,
            "holder_identities_included": False,
        },
        "commitment_root": {
            "schema": "sorafs.pop.commitment_root_publication_canary.v1",
            "status": "passed",
            "root_digest_hex": HEX,
            "tree_version": 7,
            "published_at_unix": NOW - 60,
            "publisher_signature_verified": True,
            "monotonic_tree_version": True,
            "anchor_published": True,
            "credential_leaves_included": False,
        },
        "revocation_registry": {
            "schema": "sorafs.pop.revocation_registry_canary.v1",
            "status": "passed",
            "revocation_list_digest_hex": HEX_2,
            "revocation_list_version": 8,
            "published_at_unix": NOW - 30,
            "publisher_signature_verified": True,
            "test_revocation_probe_passed": True,
            "rollback_detected": False,
            "revoked_nonces_included": False,
            "revoked_nonce_count": 2,
        },
        "enrollment_portal": {
            "schema": "sorafs.pop.enrollment_portal_canary.v1",
            "status": "passed",
            "route_count": 4,
            "passed_route_count": 4,
            "issuer_approval_required": True,
            "renewal_flow_verified": True,
            "rate_limit_configured": True,
            "pii_fields_included": False,
            "attestations_included": False,
            "routes": [
                route("application_submit"),
                route("application_status"),
                route("issuer_approval"),
                route("renewal_request"),
            ],
        },
        "juror_client": {
            "schema": "sorafs.pop.juror_client_canary.v1",
            "status": "passed",
            "synced_root_digest_hex": HEX,
            "synced_revocation_list_digest_hex": HEX_2,
            "credential_store_encrypted": True,
            "revocation_sync_success": True,
            "proof_generation_success": True,
            "credential_rotation_dry_run_success": True,
            "offline_export_encrypted": True,
            "holder_identity_included": False,
            "proof_payloads_included": False,
        },
        "verifier_service": {
            "schema": "sorafs.pop.verifier_service_canary.v1",
            "status": "passed",
            "root_digest_hex": HEX,
            "revocation_list_digest_hex": HEX_2,
            "proof_probe_count": 4,
            "accepted_valid_proof_count": 1,
            "rejected_invalid_proof_count": 3,
            "expired_proof_rejected": True,
            "revoked_proof_rejected": True,
            "replay_nullifier_rejected": True,
            "root_binding_verified": True,
            "max_verify_latency_ms": 250,
            "max_service_lag_seconds": 20,
            "raw_proofs_included": False,
            "holder_identity_disclosed": False,
            "routes": [
                route("proof_verify"),
                route("proof_status"),
                route("health"),
            ],
        },
        "moderation_integration": {
            "schema": "sorafs.pop.moderation_integration_canary.v1",
            "status": "passed",
            "root_digest_hex": HEX,
            "revocation_list_digest_hex": HEX_2,
            "pop_snapshot_digest_hex": HEX,
            "sortition_probe_count": 2,
            "commit_reveal_probe_count": 2,
            "juror_pool_bound": True,
            "moderation_case_binding_verified": True,
            "duplicate_nullifier_rejected": True,
            "observer_credentials_excluded": True,
            "identity_payloads_included": False,
            "credential_payloads_included": False,
        },
        "metrics_alerts": {
            "schema": "sorafs.pop.metrics_alert_canary.v1",
            "status": "passed",
            "root_digest_hex": HEX,
            "revocation_list_digest_hex": HEX_2,
            "metrics_scrape_success": True,
            "dashboard_provisioned": True,
            "alert_rules_installed": True,
            "critical_alerts_firing": False,
            "metrics": list(MODULE.REQUIRED_METRICS),
            "response_bodies_included": False,
        },
        "governance_approval": {
            "schema": "sorafs.pop.governance_approval.v1",
            "status": "passed",
            "root_digest_hex": HEX,
            "revocation_list_digest_hex": HEX_2,
            "approved": True,
            "governance_vote_recorded": True,
            "iroha_config_bound": True,
            "issuer_key_policy_present": True,
            "revocation_policy_present": True,
            "retention_policy_present": True,
            "manual_override_policy_present": True,
            "zk_verifier_audit_passed": True,
            "config_source": "iroha_config",
            "privacy_proof_system": "groth16_membership_v1",
            "policy_digest_hex": HEX,
        },
    }
    for payload in payloads.values():
        payload["deployment_id"] = DEPLOYMENT_ID
        payload["environment"] = ENVIRONMENT
    return payloads


def write_complete_evidence(tmp_path: Path) -> Path:
    evidence_dir = tmp_path / "evidence"
    evidence_dir.mkdir()
    for name, payload in complete_payloads().items():
        write_json(evidence_dir / f"{name}.json", payload)
    return evidence_dir


def write_args_file(path: Path, evidence_dir: Path, summary: Path) -> Path:
    path.write_text(
        "\n".join(
            [
                "# comments and blank lines are ignored",
                "",
                "--evidence-dir",
                json.dumps(str(evidence_dir)),
                "--summary-out",
                json.dumps(str(summary)),
                "--now-unix",
                str(NOW),
            ]
        ),
        encoding="utf-8",
    )
    return path


def test_complete_evidence_is_ready(tmp_path: Path, capsys) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    exit_code = MODULE.main(
        [
            "--evidence-dir",
            str(evidence_dir),
            "--summary-out",
            str(summary),
            "--now-unix",
            str(NOW),
        ]
    )

    assert exit_code == 0
    captured = capsys.readouterr()
    assert "is ready" in captured.err
    assert captured.out == ""
    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.pop_credentials.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert payload["recognized_artifact_count"] == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert payload["valid_root_digests"] == [HEX]
    assert payload["valid_revocation_list_digests"] == [HEX_2]
    assert payload["required"]["issuer_bundle"]["artifacts"][0]["fingerprint"][
        "deployment_id"
    ] == DEPLOYMENT_ID


def test_response_file_complete_evidence_is_ready(tmp_path: Path, capsys) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    args_file = write_args_file(tmp_path / "rollout.args", evidence_dir, summary)

    exit_code = MODULE.main([f"@{args_file}"])

    assert exit_code == 0
    assert "is ready" in capsys.readouterr().err
    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["status"] == "ready"


def test_deployment_context_is_required(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    issuer = complete_payloads()["issuer_bundle"]
    del issuer["deployment_id"]
    write_json(evidence_dir / "issuer_bundle.json", issuer)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(evidence_dir),
                "--summary-out",
                str(summary),
                "--now-unix",
                str(NOW),
            ]
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["issuer_bundle"]["artifacts"][0]
    assert "deployment_id must be a non-empty string" in artifact["errors"]


def test_unreviewed_deployment_context_fails(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    governance = complete_payloads()["governance_approval"]
    governance["deployment_id"] = "pop-dev-a"
    governance["environment"] = "dev"
    write_json(evidence_dir / "governance_approval.json", governance)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(evidence_dir),
                "--summary-out",
                str(summary),
                "--now-unix",
                str(NOW),
            ]
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact_errors = payload["required"]["governance_approval"]["artifacts"][0][
        "errors"
    ]
    assert (
        "deployment_id must not contain non-reviewed deployment markers ['dev']"
        in artifact_errors
    )
    assert "environment must be one of" in "\n".join(artifact_errors)


def test_missing_verifier_service_blocks_rollout(tmp_path: Path, capsys) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    (evidence_dir / "verifier_service.json").unlink()

    assert MODULE.main(["--evidence-dir", str(evidence_dir), "--now-unix", str(NOW)]) == 1

    captured = capsys.readouterr()
    assert "missing required verifier_service rollout evidence" in captured.err


def test_transcript_digest_backend_cannot_pass_governance(tmp_path: Path, capsys) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    governance = complete_payloads()["governance_approval"]
    governance["privacy_proof_system"] = "transcript_digest_v1"
    write_json(evidence_dir / "governance_approval.json", governance)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(evidence_dir),
                "--now-unix",
                str(NOW),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    assert "production privacy-preserving proof backend" in capsys.readouterr().err
    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_approval"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "privacy_proof_system must be production privacy-preserving proof backend"
        in artifact["errors"]
    )


def test_payload_leakage_blocks_rollout(tmp_path: Path, capsys) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    issuer = complete_payloads()["issuer_bundle"]
    issuer["credential_payload"] = "raw credential bytes"
    write_json(evidence_dir / "issuer_bundle.json", issuer)

    assert MODULE.main(["--evidence-dir", str(evidence_dir), "--now-unix", str(NOW)]) == 1

    assert "credential_payload must not be present" in capsys.readouterr().err


def test_stale_revocation_registry_blocks_rollout(tmp_path: Path, capsys) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    revocation = complete_payloads()["revocation_registry"]
    revocation["published_at_unix"] = NOW - MODULE.DEFAULT_MAX_REVOCATION_AGE_SECS - 1
    write_json(evidence_dir / "revocation_registry.json", revocation)

    assert MODULE.main(["--evidence-dir", str(evidence_dir), "--now-unix", str(NOW)]) == 1

    assert "published_at_unix is older" in capsys.readouterr().err


def test_verifier_service_requires_root_revocation_binding(tmp_path: Path, capsys) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    verifier = complete_payloads()["verifier_service"]
    verifier.pop("root_digest_hex")
    write_json(evidence_dir / "verifier_service.json", verifier)

    assert MODULE.main(["--evidence-dir", str(evidence_dir), "--now-unix", str(NOW)]) == 1

    assert "root_digest_hex must be a non-empty string" in capsys.readouterr().err


def test_commitment_root_must_match_issuer_bundle(tmp_path: Path, capsys) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    commitment = complete_payloads()["commitment_root"]
    commitment["root_digest_hex"] = HEX_2
    write_json(evidence_dir / "commitment_root.json", commitment)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(evidence_dir),
                "--summary-out",
                str(summary),
                "--now-unix",
                str(NOW),
            ]
        )
        == 1
    )

    assert "issuer_bundle root_digest_hex must match" in capsys.readouterr().err
    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["valid_root_digests"] == []
    assert payload["valid_revocation_list_digests"] == []
    assert payload["required"]["issuer_bundle"]["valid"] is False
    assert payload["required"]["commitment_root"]["valid"] is False
    assert payload["required"]["juror_client"]["valid"] is False


def test_juror_root_binding_must_match_published_root(tmp_path: Path, capsys) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    juror = complete_payloads()["juror_client"]
    juror["synced_root_digest_hex"] = HEX_2
    write_json(evidence_dir / "juror_client.json", juror)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(evidence_dir),
                "--summary-out",
                str(summary),
                "--now-unix",
                str(NOW),
            ]
        )
        == 1
    )

    assert "juror_client root binding must match" in capsys.readouterr().err
    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["valid_root_digests"] == [HEX]
    assert payload["required"]["juror_client"]["valid"] is False


def test_governance_revocation_binding_must_match_published_registry(
    tmp_path: Path, capsys
) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    governance = complete_payloads()["governance_approval"]
    governance["revocation_list_digest_hex"] = HEX
    write_json(evidence_dir / "governance_approval.json", governance)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(evidence_dir),
                "--summary-out",
                str(summary),
                "--now-unix",
                str(NOW),
            ]
        )
        == 1
    )

    assert "governance_approval revocation binding must match" in capsys.readouterr().err
    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["valid_revocation_list_digests"] == [HEX_2]
    assert payload["required"]["governance_approval"]["valid"] is False


def test_missing_required_metric_blocks_rollout(tmp_path: Path, capsys) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    metrics = complete_payloads()["metrics_alerts"]
    metrics["metrics"] = ["pop_credential_issuance_total"]
    write_json(evidence_dir / "metrics_alerts.json", metrics)

    assert MODULE.main(["--evidence-dir", str(evidence_dir), "--now-unix", str(NOW)]) == 1

    assert "metrics must include value `pop_revocation_publication_total`" in capsys.readouterr().err


def test_explicit_unknown_schema_fails(tmp_path: Path, capsys) -> None:
    path = write_json(
        tmp_path / "unknown.json",
        {"schema": "sorafs.pop.unknown.v1", "status": "passed"},
    )

    assert MODULE.main(["--evidence", str(path), "--require-kind", "issuer_bundle"]) == 1

    assert "not a recognized SoraFS PoP rollout artifact" in capsys.readouterr().err


def test_unknown_directory_artifact_is_ignored_for_subset_gate(tmp_path: Path, capsys) -> None:
    evidence_dir = tmp_path / "evidence"
    evidence_dir.mkdir()
    write_json(evidence_dir / "unknown.json", {"schema": "other.v1"})
    write_json(evidence_dir / "issuer_bundle.json", complete_payloads()["issuer_bundle"])

    exit_code = MODULE.main(
        [
            "--evidence-dir",
            str(evidence_dir),
            "--require-kind",
            "issuer_bundle",
        ]
    )

    assert exit_code == 0
    captured = capsys.readouterr()
    assert "is ready" in captured.err
    summary = json.loads(captured.out)
    assert summary["status"] == "ready"
    assert summary["required_kinds"] == ["issuer_bundle"]


def test_invalid_optional_artifact_blocks_subset_gate(tmp_path: Path, capsys) -> None:
    evidence_dir = tmp_path / "evidence"
    evidence_dir.mkdir()
    write_json(evidence_dir / "issuer_bundle.json", complete_payloads()["issuer_bundle"])
    verifier = complete_payloads()["verifier_service"]
    verifier["max_verify_latency_ms"] = MODULE.DEFAULT_MAX_VERIFY_LATENCY_MS + 1
    write_json(evidence_dir / "verifier_service.json", verifier)

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(evidence_dir),
                "--require-kind",
                "issuer_bundle",
            ]
        )
        == 1
    )

    assert "max_verify_latency_ms must be <=" in capsys.readouterr().err


def test_unknown_required_kind_fails_before_validation(capsys) -> None:
    assert MODULE.main(["--evidence", "missing.json", "--require-kind", "unknown"]) == 2

    assert "unknown required evidence kind `unknown`" in capsys.readouterr().err
