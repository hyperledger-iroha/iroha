"""Tests for scripts/build_sorafs_moderation_panel_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


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
    for claim in MODULE.TRUE_CLAIMS[kind]:
        args.extend(["--verified-claim", claim])
    if kind == "appeal_intake":
        args.extend(["--case-count", "2"])
        args.extend(["--case", "appeal-case-00"])
        args.extend(["--case", "appeal-case-01"])
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
            args.extend(["--roster-juror", f"roster-juror-{index:02d}"])
    elif kind == "evidence_viewer":
        args.extend(
            [
                "--session-count",
                "3",
                "--viewer-session",
                "viewer-session-00",
                "--viewer-session",
                "viewer-session-01",
                "--viewer-session",
                "viewer-session-02",
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
            args.extend(["--notification", f"notification-{index:02d}"])
        for index in range(7):
            args.extend(["--juror", f"juror-{index:02d}"])
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
            args.extend(["--commit", f"commit-{index:02d}"])
        for index in range(7):
            args.extend(["--reveal", f"reveal-{index:02d}"])
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
        args.extend(["--settlement", "appeal-settlement-00"])
        args.extend(["--settlement", "appeal-settlement-01"])
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
        args.extend(["--panel-case", "panel-case-00"])
        args.extend(["--panel-case", "panel-case-01"])
        for index in range(CHECKER.DEFAULT_MIN_PEERS):
            args.extend(["--peer", f"peer-{index:02d}"])
        for index in range(CHECKER.DEFAULT_MIN_PEERS):
            args.extend(["--validator", f"validator-{index:02d}"])
    elif kind == "metrics_alerts":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    elif kind == "governance_approval":
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    return args


def test_builds_payload_free_appeal_intake_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("appeal_intake", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "appeal_intake").read_text("utf-8"))

    assert payload["schema"] == "sorafs.moderation_panel.appeal_intake_canary.v1"
    assert payload["case_digest_hex"] == CASE_DIGEST
    assert payload["case_count"] == 2
    assert payload["accepted_case_count"] == 2
    assert payload["cases"] == [
        {"name": "appeal-case-00", "accepted": True},
        {"name": "appeal-case-01", "accepted": True},
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
        {"name": f"roster-juror-{index:02d}", "eligible": True}
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
        {"name": f"commit-{index:02d}"} for index in range(7)
    ]
    assert payload["reveal_count"] == 7
    assert payload["reveals"] == [
        {"name": f"reveal-{index:02d}"} for index in range(7)
    ]
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
        {"name": "viewer-session-00", "attested": True, "logged": True},
        {"name": "viewer-session-01", "attested": True, "logged": True},
        {"name": "viewer-session-02", "attested": True, "logged": True},
    ]
    for claim in MODULE.TRUE_CLAIMS["evidence_viewer"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["evidence_viewer"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "evidence_viewer"
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
        {"name": f"notification-{index:02d}", "delivered": True}
        for index in range(7)
    ]
    assert payload["juror_count"] == 7
    assert payload["jurors"] == [
        {"name": f"juror-{index:02d}"}
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
        {"name": "appeal-settlement-00"},
        {"name": "appeal-settlement-01"},
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
        f"peer-{index:02d}" for index in range(CHECKER.DEFAULT_MIN_PEERS)
    ]
    assert payload["validator_count"] == CHECKER.DEFAULT_MIN_PEERS
    assert [validator["name"] for validator in payload["validators"]] == [
        f"validator-{index:02d}" for index in range(CHECKER.DEFAULT_MIN_PEERS)
    ]
    assert payload["case_count"] == 2
    assert payload["cases"] == [
        {"name": "panel-case-00", "passed": True},
        {"name": "panel-case-01", "passed": True},
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


def test_missing_viewer_event_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("evidence_viewer", tmp_path)
    index = args.index("--viewer-event-kind")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--viewer-event-kind must include every required value" in captured.err
    assert not canary_path(tmp_path, "evidence_viewer").exists()


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
    args.extend(["--reveal", "reveal-07"])

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
