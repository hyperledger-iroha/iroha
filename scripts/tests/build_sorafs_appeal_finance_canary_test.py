"""Tests for scripts/build_sorafs_appeal_finance_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_appeal_finance_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_appeal_finance_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_appeal_finance_canary",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_appeal_finance_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


CONFIG_DIGEST = "a" * 64
POLICY_DIGEST = "b" * 64
GENERATED_AT = 1_800_200_000


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=GENERATED_AT,
        max_canary_age_secs=CHECKER.DEFAULT_MAX_CANARY_AGE_SECS,
        max_dashboard_age_secs=CHECKER.DEFAULT_MAX_DASHBOARD_AGE_SECS,
        max_route_latency_ms=CHECKER.DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_settlement_lag_secs=CHECKER.DEFAULT_MAX_SETTLEMENT_LAG_SECS,
        min_peers=CHECKER.DEFAULT_MIN_PEERS,
    )


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "appeal-finance-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(GENERATED_AT),
        "--config-digest-hex",
        CONFIG_DIGEST,
    ]
    for claim in MODULE.TRUE_CLAIMS[kind]:
        args.extend(["--verified-claim", claim])
    if kind == "pricing_config":
        args.extend(
            [
                "--config-version",
                "baseline-v1",
                "--class-count",
                str(len(MODULE.REQUIRED_APPEAL_CLASSES)),
            ]
        )
    elif kind == "quote_api":
        args.extend(["--quote-count", "8", "--max-route-latency-ms", "250"])
        for route in MODULE.REQUIRED_QUOTE_ROUTES:
            args.extend(["--quote-route", route])
        for appeal_class in MODULE.REQUIRED_APPEAL_CLASSES:
            args.extend(["--appeal-class", appeal_class])
        for urgency in MODULE.REQUIRED_URGENCIES:
            args.extend(["--urgency", urgency])
    elif kind == "deposit_lifecycle":
        args.extend(
            [
                "--deposit-probe-count",
                "2",
                "--confirmed-deposit-count",
                "2",
                "--max-route-latency-ms",
                "300",
            ]
        )
        for route in MODULE.REQUIRED_DEPOSIT_ROUTES:
            args.extend(["--deposit-route", route])
    elif kind == "settlement_execution":
        args.extend(["--settlement-probe-count", "7", "--instruction-step-count", "2"])
        for route in MODULE.REQUIRED_SETTLEMENT_ROUTES:
            args.extend(["--settlement-route", route])
        for outcome in MODULE.REQUIRED_OUTCOMES:
            args.extend(["--outcome", outcome])
        for status in MODULE.REQUIRED_RECONCILIATION_STATUSES:
            args.extend(["--reconciliation-status", status])
    elif kind == "settlement_submitter":
        args.extend(
            [
                "--configured-signer-count",
                "2",
                "--queued-step-count",
                "2",
                "--submitted-step-count",
                "2",
                "--max-settlement-lag-seconds",
                "60",
            ]
        )
    elif kind == "moderation_worker":
        args.extend(
            [
                "--ballot-replay-count",
                "3",
                "--max-settlement-lag-seconds",
                "60",
            ]
        )
    elif kind == "governance_dag_publication":
        args.extend(
            [
                "--report-count",
                "2",
                "--weekly-rollup-count",
                "1",
                "--settlement-receipt-count",
                "2",
            ]
        )
        for payload_kind in MODULE.REQUIRED_PAYLOAD_KINDS:
            args.extend(["--payload-kind", payload_kind])
    elif kind == "dashboard_metrics":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
        for payload_kind in MODULE.REQUIRED_PAYLOAD_KINDS:
            args.extend(["--payload-kind", payload_kind])
    elif kind == "multi_peer_reconciliation":
        args.extend(
            [
                "--peer-count",
                str(CHECKER.DEFAULT_MIN_PEERS),
                "--validator-count",
                str(CHECKER.DEFAULT_MIN_PEERS),
                "--case-count",
                "2",
            ]
        )
    elif kind == "governance_approval":
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    if kind in MODULE.POLICY_DIGEST_KINDS and "--policy-digest-hex" not in args:
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    return args


def test_builds_payload_free_dashboard_metrics_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("dashboard_metrics", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "dashboard_metrics").read_text("utf-8"))

    assert payload["schema"] == "sorafs.appeal_finance.dashboard_metrics_canary.v1"
    assert payload["config_digest_hex"] == CONFIG_DIGEST
    assert payload["metrics"] == list(MODULE.REQUIRED_METRICS)
    assert payload["payload_kinds"] == list(MODULE.REQUIRED_PAYLOAD_KINDS)
    for claim in MODULE.TRUE_CLAIMS["dashboard_metrics"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["dashboard_metrics"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "dashboard_metrics"
    assert errors == []


def test_generated_canaries_pass_full_appeal_finance_gate(tmp_path: Path) -> None:
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
    assert payload["valid_config_digests"] == [CONFIG_DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    assert payload["valid_multi_peer_runs"] == [
        {
            "deployment_id": "appeal-finance-prod-20260701",
            "environment": "production",
            "generated_at_unix": GENERATED_AT,
            "peer_count": CHECKER.DEFAULT_MIN_PEERS,
            "validator_count": CHECKER.DEFAULT_MIN_PEERS,
            "case_count": 2,
            "config_digest_hex": CONFIG_DIGEST,
        }
    ]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_response_file_can_build_pricing_config_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "pricing.args"
    args_file.write_text(
        "\n".join(args_for("pricing_config", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "pricing_config").read_text("utf-8"))
    assert payload["config_version"] == "baseline-v1"
    assert payload["policy_digest_hex"] == POLICY_DIGEST
    assert payload["class_count"] == len(MODULE.REQUIRED_APPEAL_CLASSES)


def test_missing_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("settlement_submitter", tmp_path)
    index = args.index("--verified-claim")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--verified-claim must include every required value" in captured.err
    assert not canary_path(tmp_path, "settlement_submitter").exists()


def test_missing_quote_route_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("quote_api", tmp_path)
    index = args.index("--quote-route")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--quote-route must include every required value" in captured.err
    assert not canary_path(tmp_path, "quote_api").exists()


def test_pricing_config_requires_policy_digest_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("pricing_config", tmp_path)
    index = args.index("--policy-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--policy-digest-hex is required for pricing_config" in captured.err
    assert not canary_path(tmp_path, "pricing_config").exists()


def test_under_replicated_multi_peer_run_fails_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("multi_peer_reconciliation", tmp_path)
    args[args.index("--peer-count") + 1] = str(CHECKER.DEFAULT_MIN_PEERS - 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert f"--peer-count must be >= {CHECKER.DEFAULT_MIN_PEERS}" in captured.err
    assert not canary_path(tmp_path, "multi_peer_reconciliation").exists()


def test_excessive_settlement_lag_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("moderation_worker", tmp_path)
    args[args.index("--max-settlement-lag-seconds") + 1] = str(
        CHECKER.DEFAULT_MAX_SETTLEMENT_LAG_SECS + 1
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--max-settlement-lag-seconds must be <=" in captured.err
    assert not canary_path(tmp_path, "moderation_worker").exists()


def test_output_symlink_is_rejected(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    symlink = canary_path(tmp_path, "pricing_config")
    symlink.symlink_to(target)

    assert MODULE.main(args_for("pricing_config", tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a symlink" in captured.err
    assert not target.exists()
