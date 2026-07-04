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
GENERATED_AT = NOW - 120
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
        "body_blake3_hex": HEX,
        "authz_enforced": True,
        "signature_verified": True,
    }


def complete_payloads() -> dict[str, dict[str, object]]:
    payloads = {
        "issuer_bundle": {
            "schema": "sorafs.pop.issuer_bundle_canary.v1",
            "status": "passed",
            "issuer_id": "pop-issuer-a",
            "bundle_id_hex": HEX,
            "root_digest_hex": HEX,
            "revocation_list_digest_hex": HEX_2,
            "credential_count": 3,
            "signed_credential_count": 3,
            "credentials": [
                {"name": "pop-credential-00"},
                {"name": "pop-credential-01"},
                {"name": "pop-credential-02"},
            ],
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
            "revoked_nonce_refs": [
                {"name": "pop-revoked-nonce-00"},
                {"name": "pop-revoked-nonce-01"},
            ],
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
            "route_count": 3,
            "passed_route_count": 3,
            "probes": [
                {"name": "pop-valid-proof-00", "accepted": True},
                {"name": "pop-invalid-proof-00", "accepted": False},
                {"name": "pop-invalid-proof-01", "accepted": False},
                {"name": "pop-invalid-proof-02", "accepted": False},
            ],
            "expired_proof_rejected": True,
            "revoked_proof_rejected": True,
            "replay_nullifier_rejected": True,
            "root_binding_verified": True,
            "policy_digest_hex": HEX,
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
            "sortition_probes": [
                {"name": "pop-sortition-probe-00"},
                {"name": "pop-sortition-probe-01"},
            ],
            "commit_reveal_probe_count": 2,
            "commit_reveal_probes": [
                {"name": "pop-commit-reveal-probe-00"},
                {"name": "pop-commit-reveal-probe-01"},
            ],
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
            "metric_count": len(MODULE.REQUIRED_METRICS),
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
        payload["generated_at_unix"] = GENERATED_AT
        payload["deployment_id"] = DEPLOYMENT_ID
        payload["environment"] = ENVIRONMENT
        payload["deployment_context_reviewed"] = True
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


def validation_options() -> object:
    return MODULE.ValidationOptions(
        now_unix=NOW,
        max_root_age_secs=MODULE.DEFAULT_MAX_ROOT_AGE_SECS,
        max_revocation_age_secs=MODULE.DEFAULT_MAX_REVOCATION_AGE_SECS,
        max_service_lag_secs=MODULE.DEFAULT_MAX_SERVICE_LAG_SECS,
        max_verify_latency_ms=MODULE.DEFAULT_MAX_VERIFY_LATENCY_MS,
    )


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
    assert payload["valid_juror_sync_bindings"] == [
        {
            "synced_root_digest_hex": HEX,
            "synced_revocation_list_digest_hex": HEX_2,
        }
    ]
    assert payload["valid_pop_snapshot_digests"] == [HEX]
    assert payload["valid_root_digests"] == [HEX]
    assert payload["valid_revocation_list_digests"] == [HEX_2]
    assert payload["valid_policy_digests"] == [HEX]
    assert payload["metrics"] == sorted(MODULE.REQUIRED_METRICS)
    assert payload["metric_count_values"] == [len(MODULE.REQUIRED_METRICS)]
    metrics_alerts_artifact = payload["required"]["metrics_alerts"]["artifacts"][0]
    assert metrics_alerts_artifact["fingerprint"]["metric_count"] == len(
        MODULE.REQUIRED_METRICS
    )
    assert metrics_alerts_artifact["fingerprint"]["metrics"] == list(
        MODULE.REQUIRED_METRICS
    )
    assert payload["required"]["issuer_bundle"]["artifacts"][0]["fingerprint"][
        "deployment_id"
    ] == DEPLOYMENT_ID


def test_payload_safety_flags_are_required(tmp_path: Path) -> None:
    required_false_fields = (
        ("issuer_bundle", "credential_payloads_included"),
        ("issuer_bundle", "holder_identities_included"),
        ("commitment_root", "credential_leaves_included"),
        ("revocation_registry", "rollback_detected"),
        ("revocation_registry", "revoked_nonces_included"),
        ("enrollment_portal", "pii_fields_included"),
        ("enrollment_portal", "attestations_included"),
        ("juror_client", "holder_identity_included"),
        ("juror_client", "proof_payloads_included"),
        ("verifier_service", "raw_proofs_included"),
        ("verifier_service", "holder_identity_disclosed"),
        ("moderation_integration", "identity_payloads_included"),
        ("moderation_integration", "credential_payloads_included"),
        ("metrics_alerts", "critical_alerts_firing"),
        ("metrics_alerts", "response_bodies_included"),
    )

    for kind, field in required_false_fields:
        root = tmp_path / f"{kind}_{field}"
        root.mkdir()
        evidence_dir = write_complete_evidence(root)
        payload = complete_payloads()[kind]
        del payload[field]
        write_json(evidence_dir / f"{kind}.json", payload)
        summary = root / "summary.json"

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

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert artifact["valid"] is False
        assert f"{field} must be false" in artifact["errors"]


def test_response_file_complete_evidence_is_ready(tmp_path: Path, capsys) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    args_file = write_args_file(tmp_path / "rollout.args", evidence_dir, summary)

    exit_code = MODULE.main([f"@{args_file}"])

    assert exit_code == 0
    assert "is ready" in capsys.readouterr().err
    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["status"] == "ready"


def test_issuer_id_must_be_canonical() -> None:
    for issuer_id in (
        "pop-issuer-a",
        "pop-issuer-prod-a",
        "pop-issuer-governance-12",
    ):
        payload = complete_payloads()["issuer_bundle"]
        payload["issuer_id"] = issuer_id
        kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert kind == "issuer_bundle"
        assert errors == []

    for issuer_id in ("pop-issuer", "Pop-issuer-a", "pop-issuer--a", "issuer-a"):
        payload = complete_payloads()["issuer_bundle"]
        payload["issuer_id"] = issuer_id
        kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
        assert kind == "issuer_bundle"
        assert MODULE.ISSUER_ID_ERROR in errors

    payload = complete_payloads()["issuer_bundle"]
    payload["issuer_id"] = "pop-issuer-dev-a"
    _kind, errors = MODULE.validate_evidence_payload(payload, validation_options())
    assert "issuer_id must not contain non-production markers ['dev']" in errors


def test_issuer_id_rejects_generic_issuer_family() -> None:
    payload = complete_payloads()["issuer_bundle"]
    payload["issuer_id"] = "issuer-prod-a"

    kind, errors = MODULE.validate_evidence_payload(payload, validation_options())

    assert kind == "issuer_bundle"
    assert MODULE.ISSUER_ID_ERROR in errors


def test_issuer_credential_count_must_match_unique_credentials(
    tmp_path: Path,
) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    issuer = complete_payloads()["issuer_bundle"]
    issuer["credential_count"] += 1
    issuer["signed_credential_count"] = issuer["credential_count"]
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
    assert "credential_count must match unique credentials count" in artifact["errors"]


def test_issuer_credentials_must_not_duplicate(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    issuer = complete_payloads()["issuer_bundle"]
    issuer["credentials"].append(dict(issuer["credentials"][0]))
    issuer["credential_count"] = len(issuer["credentials"])
    issuer["signed_credential_count"] = len(issuer["credentials"])
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
    assert "credentials must not contain duplicate values" in artifact["errors"]
    assert "credential_count must match unique credentials count" in artifact["errors"]


def test_issuer_credentials_must_use_reviewed_labels(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    issuer = complete_payloads()["issuer_bundle"]
    issuer["credentials"][0]["name"] = "credential_00"
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
    assert MODULE.CREDENTIAL_LABEL_ERROR in artifact["errors"]


def test_issuer_credentials_reject_non_production_markers(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    issuer = complete_payloads()["issuer_bundle"]
    issuer["credentials"][0]["name"] = "pop-credential-placeholder"
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
    assert (
        "credentials[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_issuer_credentials_must_use_pop_family(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    issuer = complete_payloads()["issuer_bundle"]
    issuer["credentials"][0]["name"] = "credential-00"
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
    assert MODULE.CREDENTIAL_LABEL_ERROR in artifact["errors"]


def test_verifier_proof_probe_count_must_match_unique_probes(
    tmp_path: Path,
) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    verifier = complete_payloads()["verifier_service"]
    verifier["proof_probe_count"] += 1
    verifier["rejected_invalid_proof_count"] += 1
    write_json(evidence_dir / "verifier_service.json", verifier)
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
    artifact = payload["required"]["verifier_service"]["artifacts"][0]
    assert "proof_probe_count must match unique probes count" in artifact["errors"]
    assert (
        "rejected_invalid_proof_count must match rejected probes count"
        in artifact["errors"]
    )


def test_verifier_probes_must_not_duplicate(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    verifier = complete_payloads()["verifier_service"]
    verifier["probes"].append(dict(verifier["probes"][0]))
    verifier["proof_probe_count"] = len(verifier["probes"])
    verifier["accepted_valid_proof_count"] += 1
    write_json(evidence_dir / "verifier_service.json", verifier)
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
    artifact = payload["required"]["verifier_service"]["artifacts"][0]
    assert "probes must not contain duplicate values" in artifact["errors"]
    assert "proof_probe_count must match unique probes count" in artifact["errors"]


def test_verifier_probes_must_use_partitioned_reviewed_labels(
    tmp_path: Path,
) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    verifier = complete_payloads()["verifier_service"]
    verifier["probes"][0]["name"] = "pop-invalid-proof-on-accepted-side"
    verifier["probes"][1]["name"] = "pop-valid-proof-on-rejected-side"
    write_json(evidence_dir / "verifier_service.json", verifier)
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
    artifact = payload["required"]["verifier_service"]["artifacts"][0]
    assert MODULE.VALID_PROOF_PROBE_LABEL_ERROR in artifact["errors"]
    assert MODULE.INVALID_PROOF_PROBE_LABEL_ERROR in artifact["errors"]


def test_verifier_probes_reject_non_production_markers(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    verifier = complete_payloads()["verifier_service"]
    verifier["probes"][0]["name"] = "pop-valid-proof-placeholder"
    verifier["probes"][1]["name"] = "pop-invalid-proof-placeholder"
    write_json(evidence_dir / "verifier_service.json", verifier)
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
    artifact = payload["required"]["verifier_service"]["artifacts"][0]
    assert (
        "probes[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_verifier_probes_must_use_pop_families(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    verifier = complete_payloads()["verifier_service"]
    verifier["probes"][0]["name"] = "valid-proof-00"
    verifier["probes"][1]["name"] = "invalid-proof-00"
    write_json(evidence_dir / "verifier_service.json", verifier)
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
    artifact = payload["required"]["verifier_service"]["artifacts"][0]
    assert MODULE.VALID_PROOF_PROBE_LABEL_ERROR in artifact["errors"]
    assert MODULE.INVALID_PROOF_PROBE_LABEL_ERROR in artifact["errors"]


def test_verifier_probe_partition_counts_must_match_probes(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    verifier = complete_payloads()["verifier_service"]
    verifier["probes"][0]["accepted"] = False
    write_json(evidence_dir / "verifier_service.json", verifier)
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
    artifact = payload["required"]["verifier_service"]["artifacts"][0]
    assert (
        "accepted_valid_proof_count must match accepted probes count"
        in artifact["errors"]
    )
    assert (
        "rejected_invalid_proof_count must match rejected probes count"
        in artifact["errors"]
    )


def test_verifier_route_count_must_match_unique_routes(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    verifier = complete_payloads()["verifier_service"]
    verifier["route_count"] += 1
    verifier["passed_route_count"] = verifier["route_count"]
    write_json(evidence_dir / "verifier_service.json", verifier)
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
    artifact = payload["required"]["verifier_service"]["artifacts"][0]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_verifier_routes_must_not_duplicate(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    verifier = complete_payloads()["verifier_service"]
    verifier["routes"].append(dict(verifier["routes"][0]))
    verifier["route_count"] = len(verifier["routes"])
    verifier["passed_route_count"] = len(verifier["routes"])
    write_json(evidence_dir / "verifier_service.json", verifier)
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
    artifact = payload["required"]["verifier_service"]["artifacts"][0]
    assert "routes must not contain duplicate values" in artifact["errors"]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_verifier_routes_must_not_include_unknown_values(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    verifier = complete_payloads()["verifier_service"]
    verifier["routes"].append(route("proof_unknown"))
    verifier["route_count"] = len(verifier["routes"])
    verifier["passed_route_count"] = len(verifier["routes"])
    write_json(evidence_dir / "verifier_service.json", verifier)
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
    artifact = payload["required"]["verifier_service"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "routes must not include unknown values" in artifact["errors"]


def test_verifier_payload_free_flags_are_required(tmp_path: Path) -> None:
    for field in ("raw_proofs_included", "holder_identity_disclosed"):
        root = tmp_path / field
        root.mkdir()
        evidence_dir = write_complete_evidence(root)
        verifier = complete_payloads()["verifier_service"]
        del verifier[field]
        write_json(evidence_dir / "verifier_service.json", verifier)
        summary = root / "summary.json"

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

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"]["verifier_service"]["artifacts"][0]
        assert artifact["valid"] is False
        assert f"{field} must be false" in artifact["errors"]


def test_route_body_hash_is_required_for_route_artifacts(tmp_path: Path) -> None:
    route_artifacts = (
        ("enrollment_portal", "enrollment_portal.json"),
        ("verifier_service", "verifier_service.json"),
    )
    for kind, filename in route_artifacts:
        root = tmp_path / kind
        root.mkdir()
        evidence_dir = write_complete_evidence(root)
        payload = complete_payloads()[kind]
        del payload["routes"][0]["body_blake3_hex"]
        write_json(evidence_dir / filename, payload)
        summary = root / "summary.json"

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

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert artifact["valid"] is False
        assert (
            "routes[0].body_blake3_hex must be a non-empty string"
            in artifact["errors"]
        )


def test_moderation_sortition_probe_count_must_match_unique_probes(
    tmp_path: Path,
) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    moderation = complete_payloads()["moderation_integration"]
    moderation["sortition_probe_count"] += 1
    write_json(evidence_dir / "moderation_integration.json", moderation)
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
    artifact = payload["required"]["moderation_integration"]["artifacts"][0]
    assert (
        "sortition_probe_count must match unique sortition_probes count"
        in artifact["errors"]
    )


def test_moderation_sortition_probes_must_not_duplicate(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    moderation = complete_payloads()["moderation_integration"]
    moderation["sortition_probes"].append(dict(moderation["sortition_probes"][0]))
    moderation["sortition_probe_count"] = len(moderation["sortition_probes"])
    write_json(evidence_dir / "moderation_integration.json", moderation)
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
    artifact = payload["required"]["moderation_integration"]["artifacts"][0]
    assert "sortition_probes must not contain duplicate values" in artifact["errors"]
    assert (
        "sortition_probe_count must match unique sortition_probes count"
        in artifact["errors"]
    )


def test_moderation_sortition_probes_must_use_reviewed_labels(
    tmp_path: Path,
) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    moderation = complete_payloads()["moderation_integration"]
    moderation["sortition_probes"][0]["name"] = "sortition_probe_00"
    write_json(evidence_dir / "moderation_integration.json", moderation)
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
    artifact = payload["required"]["moderation_integration"]["artifacts"][0]
    assert MODULE.SORTITION_PROBE_LABEL_ERROR in artifact["errors"]


def test_moderation_sortition_probes_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    moderation = complete_payloads()["moderation_integration"]
    moderation["sortition_probes"][0]["name"] = "pop-sortition-probe-placeholder"
    write_json(evidence_dir / "moderation_integration.json", moderation)
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
    artifact = payload["required"]["moderation_integration"]["artifacts"][0]
    assert (
        "sortition_probes[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_moderation_probes_must_use_pop_families(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    moderation = complete_payloads()["moderation_integration"]
    moderation["sortition_probes"][0]["name"] = "sortition-probe-00"
    moderation["commit_reveal_probes"][0]["name"] = "commit-reveal-probe-00"
    write_json(evidence_dir / "moderation_integration.json", moderation)
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
    artifact = payload["required"]["moderation_integration"]["artifacts"][0]
    assert MODULE.SORTITION_PROBE_LABEL_ERROR in artifact["errors"]
    assert MODULE.COMMIT_REVEAL_PROBE_LABEL_ERROR in artifact["errors"]


def test_moderation_commit_reveal_probe_count_must_match_unique_probes(
    tmp_path: Path,
) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    moderation = complete_payloads()["moderation_integration"]
    moderation["commit_reveal_probe_count"] += 1
    write_json(evidence_dir / "moderation_integration.json", moderation)
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
    artifact = payload["required"]["moderation_integration"]["artifacts"][0]
    assert (
        "commit_reveal_probe_count must match unique commit_reveal_probes count"
        in artifact["errors"]
    )


def test_moderation_commit_reveal_probes_must_use_reviewed_labels(
    tmp_path: Path,
) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    moderation = complete_payloads()["moderation_integration"]
    moderation["commit_reveal_probes"][0]["name"] = "commit_reveal_probe_00"
    write_json(evidence_dir / "moderation_integration.json", moderation)
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
    artifact = payload["required"]["moderation_integration"]["artifacts"][0]
    assert MODULE.COMMIT_REVEAL_PROBE_LABEL_ERROR in artifact["errors"]


def test_moderation_commit_reveal_probes_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    moderation = complete_payloads()["moderation_integration"]
    moderation["commit_reveal_probes"][0]["name"] = "pop-commit-reveal-probe-placeholder"
    write_json(evidence_dir / "moderation_integration.json", moderation)
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
    artifact = payload["required"]["moderation_integration"]["artifacts"][0]
    assert (
        "commit_reveal_probes[].name must not contain non-production markers "
        "['placeholder']"
        in artifact["errors"]
    )


def test_moderation_commit_reveal_probes_must_not_duplicate(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    moderation = complete_payloads()["moderation_integration"]
    moderation["commit_reveal_probes"].append(
        dict(moderation["commit_reveal_probes"][0])
    )
    moderation["commit_reveal_probe_count"] = len(
        moderation["commit_reveal_probes"]
    )
    write_json(evidence_dir / "moderation_integration.json", moderation)
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
    artifact = payload["required"]["moderation_integration"]["artifacts"][0]
    assert (
        "commit_reveal_probes must not contain duplicate values"
        in artifact["errors"]
    )
    assert (
        "commit_reveal_probe_count must match unique commit_reveal_probes count"
        in artifact["errors"]
    )


def test_enrollment_portal_route_count_must_match_unique_routes(
    tmp_path: Path,
) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    portal = complete_payloads()["enrollment_portal"]
    portal["route_count"] += 1
    portal["passed_route_count"] = portal["route_count"]
    write_json(evidence_dir / "enrollment_portal.json", portal)
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
    artifact = payload["required"]["enrollment_portal"]["artifacts"][0]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_enrollment_portal_routes_must_not_duplicate(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    portal = complete_payloads()["enrollment_portal"]
    portal["routes"].append(dict(portal["routes"][0]))
    portal["route_count"] = len(portal["routes"])
    portal["passed_route_count"] = len(portal["routes"])
    write_json(evidence_dir / "enrollment_portal.json", portal)
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
    artifact = payload["required"]["enrollment_portal"]["artifacts"][0]
    assert "routes must not contain duplicate values" in artifact["errors"]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_enrollment_portal_routes_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    portal = complete_payloads()["enrollment_portal"]
    portal["routes"].append(route("application_unknown"))
    portal["route_count"] = len(portal["routes"])
    portal["passed_route_count"] = len(portal["routes"])
    write_json(evidence_dir / "enrollment_portal.json", portal)
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
    artifact = payload["required"]["enrollment_portal"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "routes must not include unknown values" in artifact["errors"]


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
    assert "missing required rollout evidence" in captured.err
    assert "missing required verifier_service rollout evidence" not in captured.err


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


def test_verifier_service_requires_policy_digest(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    verifier = complete_payloads()["verifier_service"]
    del verifier["policy_digest_hex"]
    write_json(evidence_dir / "verifier_service.json", verifier)
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
    artifact = payload["required"]["verifier_service"]["artifacts"][0]
    assert "policy_digest_hex must be a non-empty string" in artifact["errors"]
    assert payload["valid_policy_digests"] == []


def test_governance_policy_digest_must_match_verifier_service(
    tmp_path: Path,
) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    governance = complete_payloads()["governance_approval"]
    governance["policy_digest_hex"] = HEX_2
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
    artifact = payload["required"]["governance_approval"]["artifacts"][0]
    assert payload["valid_policy_digests"] == [HEX]
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "governance_approval policy_digest_hex must match a valid "
        "verifier_service policy_digest_hex"
    ]


def test_policy_bound_subset_requires_verifier_service_anchor(tmp_path: Path) -> None:
    evidence_dir = tmp_path / "evidence"
    evidence_dir.mkdir()
    write_json(
        evidence_dir / "governance_approval.json",
        complete_payloads()["governance_approval"],
    )
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(evidence_dir),
                "--require-kind",
                "governance_approval",
                "--summary-out",
                str(summary),
                "--now-unix",
                str(NOW),
            ]
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["governance_approval"]["artifacts"][0]
    assert payload["valid_policy_digests"] == []
    assert artifact["valid"] is False
    assert (
        "governance_approval policy_digest_hex must match a valid "
        "verifier_service policy_digest_hex"
    ) in artifact["errors"]


def test_payload_leakage_blocks_rollout(tmp_path: Path, capsys) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    issuer = complete_payloads()["issuer_bundle"]
    issuer["credential_payload"] = "raw credential bytes"
    write_json(evidence_dir / "issuer_bundle.json", issuer)

    assert MODULE.main(["--evidence-dir", str(evidence_dir), "--now-unix", str(NOW)]) == 1

    err = capsys.readouterr().err
    assert "<sensitive-key> must not be present" in err
    assert "credential_payload must not be present" not in err


def test_stale_revocation_registry_blocks_rollout(tmp_path: Path, capsys) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    revocation = complete_payloads()["revocation_registry"]
    revocation["published_at_unix"] = NOW - MODULE.DEFAULT_MAX_REVOCATION_AGE_SECS - 1
    write_json(evidence_dir / "revocation_registry.json", revocation)

    assert MODULE.main(["--evidence-dir", str(evidence_dir), "--now-unix", str(NOW)]) == 1

    assert "published_at_unix is older" in capsys.readouterr().err


def test_revoked_nonce_count_must_match_unique_refs(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    revocation = complete_payloads()["revocation_registry"]
    revocation["revoked_nonce_count"] += 1
    write_json(evidence_dir / "revocation_registry.json", revocation)
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
    artifact = payload["required"]["revocation_registry"]["artifacts"][0]
    assert (
        "revoked_nonce_count must match unique revoked_nonce_refs count"
        in artifact["errors"]
    )


def test_revoked_nonce_refs_must_not_duplicate(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    revocation = complete_payloads()["revocation_registry"]
    revocation["revoked_nonce_refs"].append(dict(revocation["revoked_nonce_refs"][0]))
    revocation["revoked_nonce_count"] = len(revocation["revoked_nonce_refs"])
    write_json(evidence_dir / "revocation_registry.json", revocation)
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
    artifact = payload["required"]["revocation_registry"]["artifacts"][0]
    assert "revoked_nonce_refs must not contain duplicate values" in artifact["errors"]
    assert (
        "revoked_nonce_count must match unique revoked_nonce_refs count"
        in artifact["errors"]
    )


def test_revoked_nonce_refs_must_use_reviewed_labels(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    revocation = complete_payloads()["revocation_registry"]
    revocation["revoked_nonce_refs"][0]["name"] = "revoked-nonce-00"
    write_json(evidence_dir / "revocation_registry.json", revocation)
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
    artifact = payload["required"]["revocation_registry"]["artifacts"][0]
    assert MODULE.REVOKED_NONCE_LABEL_ERROR in artifact["errors"]


def test_revoked_nonce_refs_reject_non_production_markers(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    revocation = complete_payloads()["revocation_registry"]
    revocation["revoked_nonce_refs"][0]["name"] = "pop-revoked-nonce-placeholder"
    write_json(evidence_dir / "revocation_registry.json", revocation)
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
    artifact = payload["required"]["revocation_registry"]["artifacts"][0]
    assert (
        "revoked_nonce_refs[0].name must not contain non-production markers "
        "['placeholder']"
    ) in artifact["errors"]


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
    assert payload["valid_juror_sync_bindings"] == []
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


def test_metrics_must_not_duplicate(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    metrics = complete_payloads()["metrics_alerts"]
    metrics["metrics"].append(metrics["metrics"][0])
    metrics["metric_count"] = len(metrics["metrics"])
    write_json(evidence_dir / "metrics_alerts.json", metrics)
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
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["metrics_alerts"]["artifacts"][0]
    assert "metrics must not contain duplicate values" in artifact["errors"]
    assert "metric_count must match unique metrics count" in artifact["errors"]


def test_metrics_must_not_include_unknown_values(tmp_path: Path) -> None:
    evidence_dir = write_complete_evidence(tmp_path)
    metrics = complete_payloads()["metrics_alerts"]
    metrics["metrics"].append("pop_unknown_metric_total")
    metrics["metric_count"] = len(metrics["metrics"])
    write_json(evidence_dir / "metrics_alerts.json", metrics)
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
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["metrics_alerts"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "metrics must not include unknown values" in artifact["errors"]


def test_metrics_payload_free_flags_are_required(tmp_path: Path) -> None:
    for field in ("critical_alerts_firing", "response_bodies_included"):
        root = tmp_path / field
        root.mkdir()
        evidence_dir = write_complete_evidence(root)
        metrics = complete_payloads()["metrics_alerts"]
        del metrics[field]
        write_json(evidence_dir / "metrics_alerts.json", metrics)
        summary = root / "summary.json"

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
        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"]["metrics_alerts"]["artifacts"][0]
        assert artifact["valid"] is False
        assert f"{field} must be false" in artifact["errors"]


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

    assert "unknown required evidence kind" in capsys.readouterr().err
