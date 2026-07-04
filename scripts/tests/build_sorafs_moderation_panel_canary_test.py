"""Tests for scripts/build_sorafs_moderation_panel_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_moderation_panel_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_moderation_panel_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_moderation_panel_canary",
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


CASE_DIGEST = "a" * 64
ROSTER_HASH = "b" * 64
TALLY_DIGEST = "c" * 64
POP_DIGEST = "d" * 64
SORTITION_SEED = "e" * 64
POLICY_DIGEST = "f" * 64
ROUTE_BODY_DIGEST = "9" * 64
GENERATED_AT = 1_800_300_000


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=GENERATED_AT,
        max_canary_age_secs=CHECKER.DEFAULT_MAX_CANARY_AGE_SECS,
        max_event_lag_secs=CHECKER.DEFAULT_MAX_EVENT_LAG_SECS,
        max_route_latency_ms=CHECKER.DEFAULT_MAX_ROUTE_LATENCY_MS,
        min_panel_size=CHECKER.DEFAULT_MIN_PANEL_SIZE,
        min_peers=CHECKER.DEFAULT_MIN_PEERS,
    )


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "moderation-panel-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(GENERATED_AT),
        "--case-digest-hex",
        CASE_DIGEST,
    ]
    if kind in MODULE.ROSTER_DIGEST_KINDS:
        args.extend(["--roster-hash-hex", ROSTER_HASH])
    if kind in MODULE.TALLY_DIGEST_KINDS:
        args.extend(["--tally-digest-hex", TALLY_DIGEST])
    if kind in MODULE.ROUTE_BODY_DIGEST_KINDS:
        args.extend(["--route-body-blake3-hex", ROUTE_BODY_DIGEST])
    for claim in MODULE.TRUE_CLAIMS[kind]:
        args.extend(["--verified-claim", claim])
    if kind == "appeal_intake":
        args.extend(["--case-count", "2"])
        args.extend(["--case", "moderation-appeal-case-00"])
        args.extend(["--case", "moderation-appeal-case-01"])
        for route in MODULE.REQUIRED_INTAKE_ROUTES:
            args.extend(["--intake-route", route])
    elif kind == "sortition_roster":
        args.extend(
            [
                "--pop-snapshot-digest-hex",
                POP_DIGEST,
                "--sortition-seed-hex",
                SORTITION_SEED,
                "--panel-size",
                str(CHECKER.DEFAULT_MIN_PANEL_SIZE),
                "--quorum",
                "5",
            ]
        )
        for index in range(CHECKER.DEFAULT_MIN_PANEL_SIZE):
            args.extend(["--roster-juror", f"moderation-roster-juror-{index:02d}"])
    elif kind == "evidence_viewer":
        args.extend(
            [
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
                CASE_DIGEST,
                "--watermark-metadata-digest-hex",
                CASE_DIGEST,
                "--access-log-digest-hex",
                CASE_DIGEST,
                "--legal-hold-receipt-digest-hex",
                CASE_DIGEST,
                "--transparency-report-digest-hex",
                CASE_DIGEST,
                "--audit-digest-hex",
                CASE_DIGEST,
            ]
        )
        for role in MODULE.REQUIRED_VIEWER_ROLES:
            args.extend(["--viewer-role", role])
        for control in MODULE.REQUIRED_VIEWER_SECURITY_CONTROLS:
            args.extend(["--viewer-security-control", control])
        for event_kind in MODULE.REQUIRED_VIEWER_EVENT_KINDS:
            args.extend(["--viewer-event-kind", event_kind])
        for target in MODULE.REQUIRED_VIEWER_EXPORT_TARGETS:
            args.extend(["--viewer-export-target", target])
    elif kind == "operator_workflow":
        for route in MODULE.REQUIRED_OPERATOR_ROUTES:
            args.extend(["--operator-route", route])
    elif kind == "juror_notifications":
        args.extend(["--notification-count", "7", "--juror-count", "7"])
        for index in range(7):
            args.extend(["--notification", f"moderation-notification-{index:02d}"])
        for index in range(7):
            args.extend(["--juror", f"moderation-juror-{index:02d}"])
    elif kind == "commit_reveal":
        args.extend(
            [
                "--panel-size",
                str(CHECKER.DEFAULT_MIN_PANEL_SIZE),
                "--commit-count",
                "7",
                "--reveal-count",
                "7",
                "--max-event-lag-seconds",
                "60",
            ]
        )
        for index in range(7):
            args.extend(["--commit", f"moderation-commit-{index:02d}"])
        for index in range(7):
            args.extend(["--reveal", f"moderation-reveal-{index:02d}"])
        for route in MODULE.REQUIRED_BALLOT_ROUTES:
            args.extend(["--ballot-route", route])
        for scenario in MODULE.REQUIRED_COMMIT_REVEAL_SCENARIOS:
            args.extend(["--scenario", scenario])
    elif kind == "decision_publication":
        for route in MODULE.REQUIRED_DECISION_ROUTES:
            args.extend(["--decision-route", route])
        for outcome in MODULE.REQUIRED_OUTCOMES:
            args.extend(["--outcome", outcome])
    elif kind == "settlement_integration":
        args.extend(["--settlement-count", "2"])
        args.extend(["--settlement", "moderation-settlement-00"])
        args.extend(["--settlement", "moderation-settlement-01"])
    elif kind == "transparency_reputation":
        for target in MODULE.REQUIRED_PUBLICATION_TARGETS:
            args.extend(["--publication-target", target])
    elif kind == "e2e_panel":
        args.extend(
            [
                "--policy-digest-hex",
                POLICY_DIGEST,
                "--peer-count",
                str(CHECKER.DEFAULT_MIN_PEERS),
                "--validator-count",
                str(CHECKER.DEFAULT_MIN_PEERS),
                "--case-count",
                "2",
            ]
        )
        args.extend(["--panel-case", "moderation-case-00"])
        args.extend(["--panel-case", "moderation-case-01"])
        for index in range(CHECKER.DEFAULT_MIN_PEERS):
            args.extend(["--peer", f"moderation-peer-{index:02d}"])
        for index in range(CHECKER.DEFAULT_MIN_PEERS):
            args.extend(["--validator", f"moderation-validator-{index:02d}"])
    elif kind == "metrics_alerts":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    elif kind == "governance_approval":
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    return args


def assert_rejected_without_artifact(
    args: list[str],
    *,
    kind: str,
    tmp_path: Path,
    capsys,
    expected_error: str,
) -> None:
    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, kind).exists()


def test_builds_payload_free_appeal_intake_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("appeal_intake", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "appeal_intake").read_text("utf-8"))

    assert payload["schema"] == "sorafs.moderation_panel.appeal_intake_canary.v1"
    assert payload["case_digest_hex"] == CASE_DIGEST
    assert payload["case_count"] == 2
    assert payload["accepted_case_count"] == 2
    assert payload["cases"] == [
        {"name": "moderation-appeal-case-00", "accepted": True},
        {"name": "moderation-appeal-case-01", "accepted": True},
    ]
    for claim in MODULE.TRUE_CLAIMS["appeal_intake"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["appeal_intake"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "appeal_intake"
    assert errors == []


def test_builds_payload_free_sortition_roster_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("sortition_roster", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "sortition_roster").read_text("utf-8"))

    assert payload["schema"] == "sorafs.moderation_panel.sortition_roster_canary.v1"
    assert payload["pop_snapshot_digest_hex"] == POP_DIGEST
    assert payload["sortition_seed_hex"] == SORTITION_SEED
    assert payload["panel_size"] == CHECKER.DEFAULT_MIN_PANEL_SIZE
    assert payload["jurors"] == [
        {"name": f"moderation-roster-juror-{index:02d}", "eligible": True}
        for index in range(CHECKER.DEFAULT_MIN_PANEL_SIZE)
    ]
    assert payload["quorum"] == 5
    for claim in MODULE.TRUE_CLAIMS["sortition_roster"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["sortition_roster"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "sortition_roster"
    assert errors == []


def test_builds_payload_free_commit_reveal_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("commit_reveal", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "commit_reveal").read_text("utf-8"))

    assert payload["schema"] == "sorafs.moderation_panel.commit_reveal_canary.v1"
    assert payload["case_digest_hex"] == CASE_DIGEST
    assert payload["roster_hash_hex"] == ROSTER_HASH
    assert payload["tally_digest_hex"] == TALLY_DIGEST
    assert payload["commit_count"] == 7
    assert payload["commits"] == [
        {"name": f"moderation-commit-{index:02d}"} for index in range(7)
    ]
    assert payload["reveal_count"] == 7
    assert payload["reveals"] == [
        {"name": f"moderation-reveal-{index:02d}"} for index in range(7)
    ]
    assert payload["scenario_count"] == len(MODULE.REQUIRED_COMMIT_REVEAL_SCENARIOS)
    assert payload["scenarios_exercised"] == list(
        MODULE.REQUIRED_COMMIT_REVEAL_SCENARIOS
    )
    for claim in MODULE.TRUE_CLAIMS["commit_reveal"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["commit_reveal"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "commit_reveal"
    assert errors == []


def test_builds_payload_free_evidence_viewer_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("evidence_viewer", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "evidence_viewer").read_text("utf-8"))

    assert payload["schema"] == "sorafs.moderation_panel.evidence_viewer_canary.v1"
    assert payload["session_count"] == 3
    assert payload["attested_session_count"] == 3
    assert payload["logged_session_count"] == 3
    assert payload["sessions"] == [
        {
            "name": "moderation-viewer-session-00",
            "attested": True,
            "logged": True,
        },
        {
            "name": "moderation-viewer-session-01",
            "attested": True,
            "logged": True,
        },
        {
            "name": "moderation-viewer-session-02",
            "attested": True,
            "logged": True,
        },
    ]
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
    for claim in MODULE.TRUE_CLAIMS["evidence_viewer"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["evidence_viewer"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "evidence_viewer"
    assert errors == []


def test_builds_payload_free_decision_publication_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("decision_publication", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "decision_publication").read_text("utf-8")
    )

    assert payload["schema"] == (
        "sorafs.moderation_panel.decision_publication_canary.v1"
    )
    assert payload["outcome_count"] == len(MODULE.REQUIRED_OUTCOMES)
    assert payload["outcomes"] == list(MODULE.REQUIRED_OUTCOMES)
    for claim in MODULE.TRUE_CLAIMS["decision_publication"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["decision_publication"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "decision_publication"
    assert errors == []


@pytest.mark.parametrize("kind", MODULE.ROUTE_BODY_DIGEST_KINDS)
def test_route_canaries_record_route_body_digest(kind: str, tmp_path: Path) -> None:
    assert MODULE.main(args_for(kind, tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, kind).read_text("utf-8"))

    assert all(
        route["body_blake3_hex"] == ROUTE_BODY_DIGEST for route in payload["routes"]
    )
    validated_kind, errors = CHECKER.validate_evidence_payload(
        payload, checker_options()
    )
    assert validated_kind == kind
    assert errors == []


def test_builds_payload_free_transparency_reputation_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("transparency_reputation", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "transparency_reputation").read_text("utf-8")
    )

    assert payload["schema"] == (
        "sorafs.moderation_panel.transparency_reputation_canary.v1"
    )
    assert payload["publication_target_count"] == len(
        MODULE.REQUIRED_PUBLICATION_TARGETS
    )
    assert payload["publication_targets"] == list(MODULE.REQUIRED_PUBLICATION_TARGETS)
    for claim in MODULE.TRUE_CLAIMS["transparency_reputation"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["transparency_reputation"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "transparency_reputation"
    assert errors == []


def test_builds_payload_free_juror_notifications_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("juror_notifications", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "juror_notifications").read_text("utf-8")
    )

    assert payload["schema"] == (
        "sorafs.moderation_panel.juror_notifications_canary.v1"
    )
    assert payload["notification_count"] == 7
    assert payload["delivered_notification_count"] == 7
    assert payload["notifications"] == [
        {"name": f"moderation-notification-{index:02d}", "delivered": True}
        for index in range(7)
    ]
    assert payload["juror_count"] == 7
    assert payload["jurors"] == [
        {"name": f"moderation-juror-{index:02d}"}
        for index in range(7)
    ]
    for claim in MODULE.TRUE_CLAIMS["juror_notifications"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["juror_notifications"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "juror_notifications"
    assert errors == []


def test_builds_payload_free_settlement_integration_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("settlement_integration", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "settlement_integration").read_text("utf-8")
    )

    assert payload["schema"] == (
        "sorafs.moderation_panel.settlement_integration_canary.v1"
    )
    assert payload["settlement_count"] == 2
    assert payload["settlements"] == [
        {"name": "moderation-settlement-00"},
        {"name": "moderation-settlement-01"},
    ]
    for claim in MODULE.TRUE_CLAIMS["settlement_integration"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["settlement_integration"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "settlement_integration"
    assert errors == []


def test_generated_canaries_pass_full_moderation_panel_gate(tmp_path: Path) -> None:
    evidence_paths: list[Path] = []
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        evidence_paths.append(canary_path(tmp_path, kind))
    summary = tmp_path / "summary.json"

    command = []
    for path in evidence_paths:
        command.extend(["--evidence", str(path)])
    command.extend(["--summary-out", str(summary), "--now-unix", str(GENERATED_AT)])

    assert CHECKER.main(command) == 0

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["valid_case_digests"] == [CASE_DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    assert payload["valid_roster_bindings"] == [
        {
            "case_digest_hex": CASE_DIGEST,
            "roster_hash_hex": ROSTER_HASH,
        }
    ]
    assert payload["valid_tally_bindings"] == [
        {
            "case_digest_hex": CASE_DIGEST,
            "roster_hash_hex": ROSTER_HASH,
            "tally_digest_hex": TALLY_DIGEST,
        }
    ]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_response_file_can_build_e2e_panel_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "e2e.args"
    args_file.write_text(
        "\n".join(args_for("e2e_panel", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "e2e_panel").read_text("utf-8"))
    assert payload["policy_digest_hex"] == POLICY_DIGEST
    assert payload["peer_count"] == CHECKER.DEFAULT_MIN_PEERS
    assert [peer["name"] for peer in payload["peers"]] == [
        f"moderation-peer-{index:02d}" for index in range(CHECKER.DEFAULT_MIN_PEERS)
    ]
    assert payload["validator_count"] == CHECKER.DEFAULT_MIN_PEERS
    assert [validator["name"] for validator in payload["validators"]] == [
        f"moderation-validator-{index:02d}"
        for index in range(CHECKER.DEFAULT_MIN_PEERS)
    ]
    assert payload["case_count"] == 2
    assert payload["cases"] == [
        {"name": "moderation-case-00", "passed": True},
        {"name": "moderation-case-01", "passed": True},
    ]


def test_missing_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("appeal_intake", tmp_path)
    index = args.index("--verified-claim")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--verified-claim must include every required value" in captured.err
    assert not canary_path(tmp_path, "appeal_intake").exists()


def test_e2e_panel_requires_policy_digest(tmp_path: Path, capsys) -> None:
    args = args_for("e2e_panel", tmp_path)
    index = args.index("--policy-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--policy-digest-hex is required for e2e_panel" in captured.err
    assert not canary_path(tmp_path, "e2e_panel").exists()


@pytest.mark.parametrize("kind", MODULE.ROUTE_BODY_DIGEST_KINDS)
def test_route_canaries_require_route_body_digest(
    kind: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for(kind, tmp_path)
    index = args.index("--route-body-blake3-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--route-body-blake3-hex must be exact lowercase 32-byte hex" in captured.err
    assert not canary_path(tmp_path, kind).exists()


def test_missing_viewer_event_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("evidence_viewer", tmp_path)
    index = args.index("--viewer-event-kind")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--viewer-event-kind must include every required value" in captured.err
    assert not canary_path(tmp_path, "evidence_viewer").exists()


@pytest.mark.parametrize(
    ("kind", "option", "duplicate_value", "unknown_value"),
    (
        (
            "appeal_intake",
            "--verified-claim",
            MODULE.TRUE_CLAIMS["appeal_intake"][0],
            "unreviewed-moderation-panel-claim",
        ),
        (
            "appeal_intake",
            "--intake-route",
            MODULE.REQUIRED_INTAKE_ROUTES[0],
            "unreviewed-intake-route",
        ),
        (
            "evidence_viewer",
            "--viewer-role",
            MODULE.REQUIRED_VIEWER_ROLES[0],
            "unreviewed-viewer-role",
        ),
        (
            "evidence_viewer",
            "--viewer-security-control",
            MODULE.REQUIRED_VIEWER_SECURITY_CONTROLS[0],
            "unreviewed-viewer-security-control",
        ),
        (
            "evidence_viewer",
            "--viewer-event-kind",
            MODULE.REQUIRED_VIEWER_EVENT_KINDS[0],
            "unreviewed-viewer-event-kind",
        ),
        (
            "evidence_viewer",
            "--viewer-export-target",
            MODULE.REQUIRED_VIEWER_EXPORT_TARGETS[0],
            "unreviewed-viewer-export-target",
        ),
        (
            "operator_workflow",
            "--operator-route",
            MODULE.REQUIRED_OPERATOR_ROUTES[0],
            "unreviewed-operator-route",
        ),
        (
            "commit_reveal",
            "--ballot-route",
            MODULE.REQUIRED_BALLOT_ROUTES[0],
            "unreviewed-ballot-route",
        ),
        (
            "commit_reveal",
            "--scenario",
            MODULE.REQUIRED_COMMIT_REVEAL_SCENARIOS[0],
            "unreviewed-commit-reveal-scenario",
        ),
        (
            "decision_publication",
            "--decision-route",
            MODULE.REQUIRED_DECISION_ROUTES[0],
            "unreviewed-decision-route",
        ),
        (
            "decision_publication",
            "--outcome",
            MODULE.REQUIRED_OUTCOMES[0],
            "unreviewed-decision-outcome",
        ),
        (
            "transparency_reputation",
            "--publication-target",
            MODULE.REQUIRED_PUBLICATION_TARGETS[0],
            "unreviewed-publication-target",
        ),
        (
            "metrics_alerts",
            "--metric",
            MODULE.REQUIRED_METRICS[0],
            "unreviewed-moderation-panel-metric",
        ),
    ),
)
def test_closed_set_inputs_reject_duplicate_and_unknown_values_before_write(
    kind: str,
    option: str,
    duplicate_value: str,
    unknown_value: str,
    tmp_path: Path,
    capsys,
) -> None:
    duplicate_args = args_for(kind, tmp_path)
    duplicate_args.extend([option, duplicate_value])
    assert_rejected_without_artifact(
        duplicate_args,
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=f"{option} must not contain duplicates",
    )

    unknown_dir = tmp_path / "unknown"
    unknown_dir.mkdir()
    unknown_args = args_for(kind, unknown_dir)
    unknown_args.extend([option, unknown_value])
    assert_rejected_without_artifact(
        unknown_args,
        kind=kind,
        tmp_path=unknown_dir,
        capsys=capsys,
        expected_error=f"{option} contains an unknown value",
    )


def test_appeal_intake_case_inventory_must_match_case_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("appeal_intake", tmp_path)
    args[args.index("--case-count") + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--case unique values must match --case-count" in captured.err
    assert not canary_path(tmp_path, "appeal_intake").exists()


def test_appeal_intake_case_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("appeal_intake", tmp_path)
    first_case = args.index("--case") + 1
    args.extend(["--case", args[first_case]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--case must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "appeal_intake").exists()


@pytest.mark.parametrize(
    ("invalid_value", "expected_error"),
    (
        (
            "appeal-case-00",
            "--case must match canonical lowercase `moderation-appeal-case-*`",
        ),
        (
            "moderation-appeal-case-placeholder",
            "--case[0] must not contain non-production markers ['placeholder']",
        ),
    ),
)
def test_appeal_intake_case_inventory_must_use_reviewed_labels_before_write(
    invalid_value: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("appeal_intake", tmp_path)
    args[args.index("--case") + 1] = invalid_value

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, "appeal_intake").exists()


def test_sortition_roster_juror_inventory_must_match_panel_size(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("sortition_roster", tmp_path)
    args[args.index("--panel-size") + 1] = str(CHECKER.DEFAULT_MIN_PANEL_SIZE + 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--roster-juror unique values must match --panel-size" in captured.err
    assert not canary_path(tmp_path, "sortition_roster").exists()


def test_sortition_roster_juror_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("sortition_roster", tmp_path)
    first_juror = args.index("--roster-juror") + 1
    args.extend(["--roster-juror", args[first_juror]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--roster-juror must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "sortition_roster").exists()


@pytest.mark.parametrize(
    ("invalid_value", "expected_error"),
    (
        (
            "roster-juror-00",
            "--roster-juror must match canonical lowercase "
            "`moderation-roster-juror-*`",
        ),
        (
            "moderation-roster-juror-placeholder",
            "--roster-juror[0] must not contain non-production markers "
            "['placeholder']",
        ),
    ),
)
def test_sortition_roster_juror_inventory_must_use_reviewed_labels_before_write(
    invalid_value: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("sortition_roster", tmp_path)
    args[args.index("--roster-juror") + 1] = invalid_value

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, "sortition_roster").exists()


def test_evidence_viewer_session_inventory_must_match_session_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("evidence_viewer", tmp_path)
    args[args.index("--session-count") + 1] = "4"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--viewer-session unique values must match --session-count" in captured.err
    assert not canary_path(tmp_path, "evidence_viewer").exists()


def test_evidence_viewer_session_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("evidence_viewer", tmp_path)
    first_session = args.index("--viewer-session") + 1
    args.extend(["--viewer-session", args[first_session]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--viewer-session must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "evidence_viewer").exists()


@pytest.mark.parametrize(
    ("invalid_value", "expected_error"),
    (
        (
            "viewer-session-00",
            "--viewer-session must match canonical lowercase "
            "`moderation-viewer-session-*`",
        ),
        (
            "moderation-viewer-session-placeholder",
            "--viewer-session[0] must not contain non-production markers "
            "['placeholder']",
        ),
    ),
)
def test_evidence_viewer_session_inventory_must_use_reviewed_labels_before_write(
    invalid_value: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("evidence_viewer", tmp_path)
    args[args.index("--viewer-session") + 1] = invalid_value

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, "evidence_viewer").exists()


def test_juror_notification_inventory_must_match_notification_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("juror_notifications", tmp_path)
    args[args.index("--notification-count") + 1] = "8"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--notification unique values must match --notification-count"
        in captured.err
    )
    assert not canary_path(tmp_path, "juror_notifications").exists()


def test_juror_notification_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("juror_notifications", tmp_path)
    first_notification = args.index("--notification") + 1
    args.extend(["--notification", args[first_notification]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--notification must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "juror_notifications").exists()


@pytest.mark.parametrize(
    ("invalid_value", "expected_error"),
    (
        (
            "notification-00",
            "--notification must match canonical lowercase "
            "`moderation-notification-*`",
        ),
        (
            "moderation-notification-placeholder",
            "--notification[0] must not contain non-production markers ['placeholder']",
        ),
    ),
)
def test_juror_notification_inventory_must_use_reviewed_labels_before_write(
    invalid_value: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("juror_notifications", tmp_path)
    args[args.index("--notification") + 1] = invalid_value

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, "juror_notifications").exists()


def test_juror_inventory_must_match_juror_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("juror_notifications", tmp_path)
    args[args.index("--juror-count") + 1] = "6"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--juror unique values must match --juror-count" in captured.err
    assert not canary_path(tmp_path, "juror_notifications").exists()


def test_juror_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("juror_notifications", tmp_path)
    first_juror = args.index("--juror") + 1
    args.extend(["--juror", args[first_juror]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--juror must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "juror_notifications").exists()


@pytest.mark.parametrize(
    ("option", "invalid_value", "expected_error"),
    (
        (
            "--juror",
            "juror-00",
            "--juror must match canonical lowercase `moderation-juror-*`",
        ),
        (
            "--juror",
            "moderation-juror-placeholder",
            "--juror[0] must not contain non-production markers ['placeholder']",
        ),
    ),
)
def test_juror_inventory_must_use_reviewed_labels_before_write(
    option: str,
    invalid_value: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("juror_notifications", tmp_path)
    args[args.index(option) + 1] = invalid_value

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, "juror_notifications").exists()


def test_commit_reveal_commit_inventory_must_match_commit_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("commit_reveal", tmp_path)
    args[args.index("--commit-count") + 1] = "8"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--commit unique values must match --commit-count" in captured.err
    assert not canary_path(tmp_path, "commit_reveal").exists()


def test_commit_reveal_commit_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("commit_reveal", tmp_path)
    first_commit = args.index("--commit") + 1
    args.extend(["--commit", args[first_commit]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--commit must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "commit_reveal").exists()


@pytest.mark.parametrize(
    ("option", "invalid_value", "expected_error"),
    (
        (
            "--commit",
            "commit-00",
            "--commit must match canonical lowercase `moderation-commit-*`",
        ),
        (
            "--commit",
            "moderation-commit-placeholder",
            "--commit[0] must not contain non-production markers ['placeholder']",
        ),
        (
            "--reveal",
            "reveal-00",
            "--reveal must match canonical lowercase `moderation-reveal-*`",
        ),
        (
            "--reveal",
            "moderation-reveal-placeholder",
            "--reveal[0] must not contain non-production markers ['placeholder']",
        ),
    ),
)
def test_commit_reveal_inventory_must_use_reviewed_labels_before_write(
    option: str,
    invalid_value: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("commit_reveal", tmp_path)
    args[args.index(option) + 1] = invalid_value

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, "commit_reveal").exists()


def test_commit_reveal_reveal_inventory_must_match_reveal_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("commit_reveal", tmp_path)
    args[args.index("--reveal-count") + 1] = "8"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--reveal unique values must match --reveal-count" in captured.err
    assert not canary_path(tmp_path, "commit_reveal").exists()


def test_commit_reveal_reveal_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("commit_reveal", tmp_path)
    first_reveal = args.index("--reveal") + 1
    args.extend(["--reveal", args[first_reveal]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--reveal must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "commit_reveal").exists()


def test_commit_reveal_reveal_count_must_not_exceed_commit_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("commit_reveal", tmp_path)
    args[args.index("--reveal-count") + 1] = "8"
    args.extend(["--reveal", "moderation-reveal-07"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--reveal-count must be <= --commit-count" in captured.err
    assert not canary_path(tmp_path, "commit_reveal").exists()


def test_settlement_integration_inventory_must_match_settlement_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("settlement_integration", tmp_path)
    args[args.index("--settlement-count") + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--settlement unique values must match --settlement-count" in captured.err
    assert not canary_path(tmp_path, "settlement_integration").exists()


def test_settlement_integration_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("settlement_integration", tmp_path)
    first_settlement = args.index("--settlement") + 1
    args.extend(["--settlement", args[first_settlement]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--settlement must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "settlement_integration").exists()


@pytest.mark.parametrize(
    ("invalid_value", "expected_error"),
    (
        (
            "appeal-settlement-00",
            "--settlement must match canonical lowercase `moderation-settlement-*`",
        ),
        (
            "moderation-settlement-placeholder",
            "--settlement[0] must not contain non-production markers ['placeholder']",
        ),
    ),
)
def test_settlement_integration_inventory_must_use_reviewed_labels_before_write(
    invalid_value: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("settlement_integration", tmp_path)
    args[args.index("--settlement") + 1] = invalid_value

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, "settlement_integration").exists()


def test_under_replicated_e2e_panel_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("e2e_panel", tmp_path)
    args[args.index("--peer-count") + 1] = str(CHECKER.DEFAULT_MIN_PEERS - 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert f"--peer-count must be >= {CHECKER.DEFAULT_MIN_PEERS}" in captured.err
    assert not canary_path(tmp_path, "e2e_panel").exists()


def test_e2e_panel_peer_inventory_must_match_peer_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("e2e_panel", tmp_path)
    args[args.index("--peer-count") + 1] = str(CHECKER.DEFAULT_MIN_PEERS + 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--peer unique values must match --peer-count" in captured.err
    assert not canary_path(tmp_path, "e2e_panel").exists()


def test_e2e_panel_peer_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("e2e_panel", tmp_path)
    first_peer = args.index("--peer") + 1
    args.extend(["--peer", args[first_peer]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--peer must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "e2e_panel").exists()


@pytest.mark.parametrize(
    ("option", "invalid_value", "expected_error"),
    (
        (
            "--peer",
            "peer-00",
            "--peer must match canonical lowercase `moderation-peer-*`",
        ),
        (
            "--peer",
            "moderation-peer-placeholder",
            "--peer[0] must not contain non-production markers ['placeholder']",
        ),
        (
            "--validator",
            "validator-00",
            "--validator must match canonical lowercase `moderation-validator-*`",
        ),
        (
            "--validator",
            "moderation-validator-placeholder",
            "--validator[0] must not contain non-production markers ['placeholder']",
        ),
        (
            "--panel-case",
            "panel-case-00",
            "--panel-case must match canonical lowercase `moderation-case-*`",
        ),
        (
            "--panel-case",
            "moderation-case-placeholder",
            "--panel-case[0] must not contain non-production markers ['placeholder']",
        ),
    ),
)
def test_e2e_panel_inventory_must_use_reviewed_labels_before_write(
    option: str,
    invalid_value: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("e2e_panel", tmp_path)
    args[args.index(option) + 1] = invalid_value

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, "e2e_panel").exists()


def test_e2e_panel_validator_inventory_must_match_validator_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("e2e_panel", tmp_path)
    args[args.index("--validator-count") + 1] = str(CHECKER.DEFAULT_MIN_PEERS + 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--validator unique values must match --validator-count" in captured.err
    assert not canary_path(tmp_path, "e2e_panel").exists()


def test_e2e_panel_validator_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("e2e_panel", tmp_path)
    first_validator = args.index("--validator") + 1
    args.extend(["--validator", args[first_validator]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--validator must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "e2e_panel").exists()


def test_e2e_panel_case_inventory_must_match_case_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("e2e_panel", tmp_path)
    args[args.index("--case-count") + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--panel-case unique values must match --case-count" in captured.err
    assert not canary_path(tmp_path, "e2e_panel").exists()


def test_e2e_panel_case_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("e2e_panel", tmp_path)
    first_case = args.index("--panel-case") + 1
    args.extend(["--panel-case", args[first_case]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--panel-case must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "e2e_panel").exists()


def test_long_lived_viewer_url_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("evidence_viewer", tmp_path)
    args[args.index("--max-url-ttl-secs") + 1] = str(
        CHECKER.DEFAULT_MAX_VIEWER_URL_TTL_SECS + 1
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--max-url-ttl-secs must be <=" in captured.err
    assert not canary_path(tmp_path, "evidence_viewer").exists()


def test_output_symlink_is_rejected(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    symlink = canary_path(tmp_path, "appeal_intake")
    symlink.symlink_to(target)

    assert MODULE.main(args_for("appeal_intake", tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a symlink" in captured.err
    assert not target.exists()


def test_output_directory_is_rejected(tmp_path: Path, capsys) -> None:
    output_dir = canary_path(tmp_path, "appeal_intake")
    output_dir.mkdir()

    assert MODULE.main(args_for("appeal_intake", tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a directory" in captured.err
    assert output_dir.is_dir()
