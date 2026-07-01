"""Tests for scripts/build_sorafs_evidence_viewer_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


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


DIGEST = "a" * 64
GENERATED_AT = 1_800_100_000


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


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
        "--attested-session-count",
        "3",
        "--logged-session-count",
        "3",
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


def test_builds_payload_free_evidence_viewer_canary(tmp_path: Path) -> None:
    assert MODULE.main(complete_args(tmp_path)) == 0

    payload = json.loads((tmp_path / "evidence-viewer.json").read_text("utf-8"))

    assert payload["schema"] == "sorafs.moderation_panel.evidence_viewer_canary.v1"
    assert payload["status"] == "passed"
    assert payload["deployment_context_reviewed"] is True
    assert payload["roles_tested"] == list(MODULE.REQUIRED_VIEWER_ROLES)
    assert payload["viewer_security_controls"] == list(
        MODULE.REQUIRED_VIEWER_SECURITY_CONTROLS
    )
    assert payload["access_event_kinds"] == list(MODULE.REQUIRED_VIEWER_EVENT_KINDS)
    assert payload["export_targets"] == list(MODULE.REQUIRED_VIEWER_EXPORT_TARGETS)
    for claim in MODULE.VERIFIED_TRUE_CLAIMS:
        assert payload[claim] is True
    for claim in MODULE.FORBIDDEN_PAYLOAD_CLAIMS:
        assert payload[claim] is False
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


def test_generated_canary_passes_existing_evidence_viewer_gate(tmp_path: Path) -> None:
    assert MODULE.main(complete_args(tmp_path)) == 0
    write_json(tmp_path / "appeal-intake.json", appeal_intake())
    write_json(tmp_path / "sortition-roster.json", sortition_roster())
    summary = tmp_path / "summary.json"

    assert (
        CHECKER.main(
            [
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

    payload = json.loads((tmp_path / "evidence-viewer.json").read_text("utf-8"))
    assert payload["access_log_digest_hex"] == DIGEST


def test_missing_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    index = args.index("--verified-claim")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--verified-claim must include every required value" in captured.err
    assert not (tmp_path / "evidence-viewer.json").exists()


def test_missing_access_event_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    index = args.index("--access-event-kind")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--access-event-kind must include every required value" in captured.err
    assert not (tmp_path / "evidence-viewer.json").exists()


def test_session_count_drift_fails_before_write(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    args[args.index("--logged-session-count") + 1] = "2"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--logged-session-count must equal --session-count" in captured.err
    assert not (tmp_path / "evidence-viewer.json").exists()


def test_output_symlink_is_rejected(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    symlink = tmp_path / "evidence-viewer.json"
    symlink.symlink_to(target)

    assert MODULE.main(complete_args(tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a symlink" in captured.err
    assert not target.exists()
