"""Tests for scripts/build_sorafs_evidence_viewer_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_evidence_viewer_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_moderation_panel_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_evidence_viewer_canary",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_moderation_panel_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)
TOPOLOGY = sys.modules["sorafs_topology_qualification"]


DIGEST = "a" * 64
GENERATED_AT = 1_800_100_000


def canary_path(tmp_path: Path) -> Path:
    return tmp_path / "evidence-viewer.json"


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def write_topology_qualification(root: Path) -> Path:
    """Write the exact reviewed topology required by the moderation gate."""

    path = root / "topology-qualification.json"
    return write_json(
        path,
        {
            "schema": TOPOLOGY.SUMMARY_SCHEMA,
            "status": "configuration-qualified",
            "qualification_scope": "pre-deployment-configuration",
            "live_evidence_recognized": False,
            "promotion_eligible": False,
            "manifest_sha256": "11" * 32,
            "canonical_manifest_sha256": "22" * 32,
            "deployment": {
                "deployment_id": "sorafs-panel-prod-20260701",
                "environment": "production",
            },
            "validator_count": 4,
            "storage_provider_count": 2,
            "gateway_count": 2,
            "governance_dag_instance_count": 2,
            "runtime_handle_kinds": ["monitoring", "hsm", "kms", "webauthn"],
            "runtime_material_policy_valid": True,
            "signed_model_artifact_count": 1,
            "required_lane_slots": list(TOPOLOGY.CANONICAL_READINESS_LANES),
            "recognized_lane_slot_count": len(
                TOPOLOGY.CANONICAL_READINESS_LANES
            ),
            "errors": [],
        },
    )


def with_context(payload: dict) -> dict:
    payload["deployment_id"] = "sorafs-panel-prod-20260701"
    payload["environment"] = "production"
    payload["deployment_context_reviewed"] = True
    payload.setdefault("generated_at_unix", GENERATED_AT)
    return payload


def route(name: str) -> dict:
    return {
        "name": name,
        "passed": True,
        "status_code": 200,
        "body_blake3_hex": DIGEST,
        "authz_enforced": True,
        "signature_verified": True,
        "latency_ms": 40,
    }


def appeal_intake() -> dict:
    routes = [
        route(name)
        for name in ("appeal_submit", "case_status", "deposit_quote", "deposit_confirm")
    ]
    return with_context(
        {
            "schema": "sorafs.moderation_panel.appeal_intake_canary.v1",
            "status": "passed",
            "case_digest_hex": DIGEST,
            "route_count": len(routes),
            "passed_route_count": len(routes),
            "routes": routes,
            "case_count": 2,
            "accepted_case_count": 2,
            "cases": [
                {"name": "moderation-appeal-case-00", "accepted": True},
                {"name": "moderation-appeal-case-01", "accepted": True},
            ],
            "appellant_auth_enforced": True,
            "proof_token_verified": True,
            "deposit_confirmation_bound": True,
            "policy_reference_bound": True,
            "duplicate_case_rejected": True,
            "invalid_payload_rejected": True,
            "payloads_included": False,
            "response_bodies_included": False,
        }
    )


def sortition_roster() -> dict:
    return with_context(
        {
            "schema": "sorafs.moderation_panel.sortition_roster_canary.v1",
            "status": "passed",
            "case_digest_hex": DIGEST,
            "pop_snapshot_digest_hex": DIGEST,
            "roster_hash_hex": DIGEST,
            "sortition_seed_hex": DIGEST,
            "panel_size": 7,
            "jurors": [
                {"name": f"moderation-roster-juror-{index:02d}", "eligible": True}
                for index in range(7)
            ],
            "quorum": 5,
            "pop_snapshot_bound": True,
            "juror_eligibility_verified": True,
            "failover_plan_present": True,
            "roster_privacy_preserved": True,
            "juror_private_data_included": False,
        }
    )


def complete_args(tmp_path: Path) -> list[str]:
    args = [
        "--out",
        str(tmp_path / "evidence-viewer.json"),
        "--deployment-id",
        "sorafs-panel-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(GENERATED_AT),
        "--case-digest-hex",
        DIGEST,
        "--roster-hash-hex",
        DIGEST,
        "--session-count",
        "3",
        "--viewer-session",
        "moderation-viewer-session-00",
        "--viewer-session",
        "moderation-viewer-session-01",
        "--viewer-session",
        "moderation-viewer-session-02",
        "--max-url-ttl-secs",
        "300",
        "--session-manifest-digest-hex",
        DIGEST,
        "--watermark-metadata-digest-hex",
        DIGEST,
        "--access-log-digest-hex",
        DIGEST,
        "--legal-hold-receipt-digest-hex",
        DIGEST,
        "--transparency-report-digest-hex",
        DIGEST,
        "--audit-digest-hex",
        DIGEST,
        "--gateway-compliance-denial-status-code",
        "451",
        "--gateway-compliance-denial-code",
        "gateway_compliance_denied",
        "--gateway-compliance-denial-source",
        "baseline",
        "--gateway-compliance-catalog-digest-hex",
        DIGEST,
    ]
    for value in MODULE.REQUIRED_VIEWER_ROLES:
        args.extend(["--role", value])
    for value in MODULE.REQUIRED_VIEWER_SECURITY_CONTROLS:
        args.extend(["--security-control", value])
    for value in MODULE.REQUIRED_VIEWER_EVENT_KINDS:
        args.extend(["--access-event-kind", value])
    for value in MODULE.REQUIRED_VIEWER_EXPORT_TARGETS:
        args.extend(["--export-target", value])
    for value in MODULE.VERIFIED_TRUE_CLAIMS:
        args.extend(["--verified-claim", value])
    return args


def assert_rejected_without_artifact(
    args: list[str],
    *,
    tmp_path: Path,
    capsys,
    expected_error: str,
) -> None:
    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path).exists()


def test_builds_payload_free_evidence_viewer_canary(tmp_path: Path) -> None:
    assert MODULE.main(complete_args(tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path).read_text("utf-8"))

    assert payload["schema"] == "sorafs.moderation_panel.evidence_viewer_canary.v1"
    assert payload["status"] == "passed"
    assert payload["deployment_context_reviewed"] is True
    assert payload["role_count"] == len(MODULE.REQUIRED_VIEWER_ROLES)
    assert payload["roles_tested"] == list(MODULE.REQUIRED_VIEWER_ROLES)
    assert payload["security_control_count"] == len(
        MODULE.REQUIRED_VIEWER_SECURITY_CONTROLS
    )
    assert payload["viewer_security_controls"] == list(
        MODULE.REQUIRED_VIEWER_SECURITY_CONTROLS
    )
    assert payload["access_event_kind_count"] == len(MODULE.REQUIRED_VIEWER_EVENT_KINDS)
    assert payload["access_event_kinds"] == list(MODULE.REQUIRED_VIEWER_EVENT_KINDS)
    assert payload["export_target_count"] == len(MODULE.REQUIRED_VIEWER_EXPORT_TARGETS)
    assert payload["export_targets"] == list(MODULE.REQUIRED_VIEWER_EXPORT_TARGETS)
    assert payload["session_count"] == 3
    assert payload["attested_session_count"] == 3
    assert payload["logged_session_count"] == 3
    assert payload["sessions"] == [
        {"name": "moderation-viewer-session-00", "attested": True, "logged": True},
        {"name": "moderation-viewer-session-01", "attested": True, "logged": True},
        {"name": "moderation-viewer-session-02", "attested": True, "logged": True},
    ]
    for claim in MODULE.VERIFIED_TRUE_CLAIMS:
        assert payload[claim] is True
    assert payload["gateway_compliance_denial_enforced"] == {
        "status_code": 451,
        "code": "gateway_compliance_denied",
        "source": "baseline",
        "catalog_digest_hex": DIGEST,
    }
    assert "denylisted_digest_blocked" not in payload
    assert payload["audit_log_tamper_rejected"] is True
    assert payload["watermark_metadata_mismatch_rejected"] is True
    for claim in MODULE.FORBIDDEN_PAYLOAD_CLAIMS:
        assert payload[claim] is False
    assert MODULE.DIGEST_FIELDS == (
        "session_manifest_digest_hex",
        "watermark_metadata_digest_hex",
        "access_log_digest_hex",
        "legal_hold_receipt_digest_hex",
        "transparency_report_digest_hex",
        "audit_digest_hex",
    )
    for field in MODULE.DIGEST_FIELDS:
        assert payload[field] == DIGEST
    assert "raw_evidence" not in payload
    assert "session_token" not in payload
    assert "signed_url" not in payload
    kind, errors = CHECKER.validate_evidence_payload(
        payload,
        CHECKER.ValidationOptions(
            now_unix=GENERATED_AT,
            max_canary_age_secs=CHECKER.DEFAULT_MAX_CANARY_AGE_SECS,
            max_event_lag_secs=CHECKER.DEFAULT_MAX_EVENT_LAG_SECS,
            max_route_latency_ms=CHECKER.DEFAULT_MAX_ROUTE_LATENCY_MS,
            min_panel_size=CHECKER.DEFAULT_MIN_PANEL_SIZE,
            min_peers=CHECKER.DEFAULT_MIN_PEERS,
        ),
    )
    assert kind == "evidence_viewer"
    assert errors == []


def test_unreviewed_deployment_id_fails_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    args[args.index("--deployment-id") + 1] = "sorafs-panel-dev-20260701"

    assert_rejected_without_artifact(
        args,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--deployment-id must not contain non-reviewed deployment markers "
            "['dev']"
        ),
    )


def test_unreviewed_environment_fails_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    args[args.index("--environment") + 1] = "dev"

    assert_rejected_without_artifact(
        args,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--environment must be one of "
            "['prod', 'production', 'release', 'staging']"
        ),
    )


def test_now_unix_is_required_for_freshness_validation(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    now_index = args.index("--now-unix")
    del args[now_index : now_index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--now-unix" in captured.err
    assert "required" in captured.err
    assert not canary_path(tmp_path).exists()


def test_future_generated_at_unix_fails_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    args[args.index("--generated-at-unix") + 1] = str(GENERATED_AT + 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "generated_at_unix must not be in the future" in captured.err
    assert str(GENERATED_AT + 1) not in captured.err
    assert not canary_path(tmp_path).exists()


def test_stale_generated_at_unix_fails_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    stale_generated_at = GENERATED_AT - CHECKER.DEFAULT_MAX_CANARY_AGE_SECS - 1
    args[args.index("--generated-at-unix") + 1] = str(stale_generated_at)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "generated_at_unix is older than 86400 seconds" in captured.err
    assert str(stale_generated_at) not in captured.err
    assert not canary_path(tmp_path).exists()


def test_generated_canary_passes_existing_evidence_viewer_gate(tmp_path: Path) -> None:
    assert MODULE.main(complete_args(tmp_path)) == 0
    write_json(tmp_path / "appeal-intake.json", appeal_intake())
    write_json(tmp_path / "sortition-roster.json", sortition_roster())
    summary = tmp_path / "summary.json"

    assert (
        CHECKER.main(
            [
                "--topology-qualification-summary",
                str(write_topology_qualification(tmp_path)),
                "--evidence",
                str(tmp_path / "appeal-intake.json"),
                "--evidence",
                str(tmp_path / "sortition-roster.json"),
                "--evidence",
                str(tmp_path / "evidence-viewer.json"),
                "--require-kind",
                "evidence_viewer",
                "--summary-out",
                str(summary),
                "--now-unix",
                str(GENERATED_AT),
            ]
        )
        == 0
    )

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["required"]["evidence_viewer"]["artifact_count"] == 1
    assert payload["required"]["evidence_viewer"]["artifacts"][0]["valid"] is True


def test_response_file_can_build_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "viewer.args"
    args_file.write_text("\n".join(complete_args(tmp_path)), encoding="utf-8")

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path).read_text("utf-8"))
    assert payload["access_log_digest_hex"] == DIGEST


def test_missing_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    index = args.index("--verified-claim")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--verified-claim must include every required value" in captured.err
    assert not canary_path(tmp_path).exists()


def test_missing_access_event_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    index = args.index("--access-event-kind")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--access-event-kind must include every required value" in captured.err
    assert not canary_path(tmp_path).exists()


@pytest.mark.parametrize(
    ("option", "duplicate_value", "unknown_value"),
    (
        (
            "--verified-claim",
            MODULE.VERIFIED_TRUE_CLAIMS[0],
            "unreviewed-viewer-claim",
        ),
        (
            "--role",
            MODULE.REQUIRED_VIEWER_ROLES[0],
            "unreviewed-viewer-role",
        ),
        (
            "--security-control",
            MODULE.REQUIRED_VIEWER_SECURITY_CONTROLS[0],
            "unreviewed-security-control",
        ),
        (
            "--access-event-kind",
            MODULE.REQUIRED_VIEWER_EVENT_KINDS[0],
            "unreviewed-access-event-kind",
        ),
        (
            "--export-target",
            MODULE.REQUIRED_VIEWER_EXPORT_TARGETS[0],
            "unreviewed-export-target",
        ),
    ),
)
def test_closed_set_inputs_reject_duplicate_and_unknown_values_before_write(
    option: str,
    duplicate_value: str,
    unknown_value: str,
    tmp_path: Path,
    capsys,
) -> None:
    duplicate_args = complete_args(tmp_path)
    duplicate_args.extend([option, duplicate_value])
    assert_rejected_without_artifact(
        duplicate_args,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=f"{option} must not contain duplicates",
    )

    unknown_dir = tmp_path / "unknown"
    unknown_dir.mkdir()
    unknown_args = complete_args(unknown_dir)
    unknown_args.extend([option, unknown_value])
    assert_rejected_without_artifact(
        unknown_args,
        tmp_path=unknown_dir,
        capsys=capsys,
        expected_error=f"{option} contains an unknown value",
    )


def test_session_count_drift_fails_before_write(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    args[args.index("--session-count") + 1] = "4"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--viewer-session unique values must match --session-count" in captured.err
    assert not (tmp_path / "evidence-viewer.json").exists()


def test_duplicate_session_inventory_fails_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    first_session = args.index("--viewer-session") + 1
    args.extend(["--viewer-session", args[first_session]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--viewer-session must not contain duplicates" in captured.err
    assert not (tmp_path / "evidence-viewer.json").exists()


def test_viewer_session_label_family_fails_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    first_session = args.index("--viewer-session") + 1
    args[first_session] = "viewer-session-00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--viewer-session must match canonical lowercase "
        "`moderation-viewer-session-*`"
    ) in captured.err
    assert not (tmp_path / "evidence-viewer.json").exists()


def test_viewer_session_placeholder_marker_fails_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    first_session = args.index("--viewer-session") + 1
    args[first_session] = "moderation-viewer-session-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--viewer-session[0] must not contain non-production markers "
        "['placeholder']"
    ) in captured.err
    assert not (tmp_path / "evidence-viewer.json").exists()


def test_unknown_and_duplicate_role_coverage_fails_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    args.extend(["--role", MODULE.REQUIRED_VIEWER_ROLES[0]])
    args.extend(["--role", "debug_operator"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--role must not contain duplicates" in captured.err
    assert "--role contains an unknown value" in captured.err
    assert not (tmp_path / "evidence-viewer.json").exists()


def test_unknown_security_control_fails_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    args.extend(["--security-control", "developer_console"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--security-control contains an unknown value" in captured.err
    assert not (tmp_path / "evidence-viewer.json").exists()


def test_duplicate_export_target_fails_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    args.extend(["--export-target", MODULE.REQUIRED_VIEWER_EXPORT_TARGETS[0]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--export-target must not contain duplicates" in captured.err
    assert not (tmp_path / "evidence-viewer.json").exists()


def test_digest_must_be_exact_lowercase_hex_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    args[args.index("--case-digest-hex") + 1] = "A" * 64

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--case-digest-hex must be exact lowercase 32-byte hex" in captured.err
    assert not (tmp_path / "evidence-viewer.json").exists()


def test_removed_local_denylist_claim_is_rejected_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    args.extend(["--verified-claim", "denylisted_digest_blocked"])

    assert_rejected_without_artifact(
        args,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--verified-claim contains an unknown value",
    )


@pytest.mark.parametrize(
    ("option", "value", "expected_error"),
    (
        (
            "--gateway-compliance-denial-status-code",
            "403",
            "--gateway-compliance-denial-status-code must be exactly 451",
        ),
        (
            "--gateway-compliance-denial-code",
            "denylisted",
            (
                "--gateway-compliance-denial-code must be exactly "
                "gateway_compliance_denied"
            ),
        ),
        (
            "--gateway-compliance-denial-source",
            "accepted_appeal",
            "--gateway-compliance-denial-source must be one of",
        ),
        (
            "--gateway-compliance-catalog-digest-hex",
            "A" * 64,
            (
                "--gateway-compliance-catalog-digest-hex must be exact "
                "lowercase 32-byte hex"
            ),
        ),
    ),
)
def test_gateway_compliance_denial_input_must_be_canonical_before_write(
    option: str,
    value: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    args[args.index(option) + 1] = value

    assert_rejected_without_artifact(
        args,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=expected_error,
    )


def test_long_lived_url_ttl_fails_before_write(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    args[args.index("--max-url-ttl-secs") + 1] = str(
        MODULE.DEFAULT_MAX_VIEWER_URL_TTL_SECS + 1
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        f"--max-url-ttl-secs must be <= {MODULE.DEFAULT_MAX_VIEWER_URL_TTL_SECS}"
        in captured.err
    )
    assert not (tmp_path / "evidence-viewer.json").exists()


def test_output_directory_is_rejected(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    args[args.index("--out") + 1] = str(tmp_path)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a directory" in captured.err


def test_output_symlink_is_rejected(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    symlink = tmp_path / "evidence-viewer.json"
    symlink.symlink_to(target)

    assert MODULE.main(complete_args(tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a symlink" in captured.err
    assert not target.exists()
