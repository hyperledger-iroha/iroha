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
    elif kind == "evidence_viewer":
        args.extend(
            [
                "--session-count",
                "3",
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
    elif kind == "metrics_alerts":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    elif kind == "governance_approval":
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    return args


def test_builds_payload_free_commit_reveal_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("commit_reveal", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "commit_reveal").read_text("utf-8"))

    assert payload["schema"] == "sorafs.moderation_panel.commit_reveal_canary.v1"
    assert payload["case_digest_hex"] == CASE_DIGEST
    assert payload["roster_hash_hex"] == ROSTER_HASH
    assert payload["tally_digest_hex"] == TALLY_DIGEST
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
    assert payload["validator_count"] == CHECKER.DEFAULT_MIN_PEERS


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


def test_under_replicated_e2e_panel_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("e2e_panel", tmp_path)
    args[args.index("--peer-count") + 1] = str(CHECKER.DEFAULT_MIN_PEERS - 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert f"--peer-count must be >= {CHECKER.DEFAULT_MIN_PEERS}" in captured.err
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
