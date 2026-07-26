"""Tests for scripts/build_sorafs_appeal_finance_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


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
ROUTE_BODY_DIGEST = "c" * 64
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
    if kind in ("quote_api", "deposit_lifecycle", "settlement_execution"):
        args.extend(["--route-body-blake3-hex", ROUTE_BODY_DIGEST])
    if kind == "pricing_config":
        args.extend(
            [
                "--config-version",
                "appeal-finance-config-baseline-v1",
                "--class-count",
                str(len(MODULE.REQUIRED_APPEAL_CLASSES)),
            ]
        )
        for appeal_class in MODULE.REQUIRED_APPEAL_CLASSES:
            args.extend(["--appeal-class", appeal_class])
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
                "--confirmed-deposit-probe",
                "appeal-finance-deposit-probe-00",
                "--confirmed-deposit-probe",
                "appeal-finance-deposit-probe-01",
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
        for instruction_step in MODULE.REQUIRED_SETTLEMENT_INSTRUCTION_STEPS:
            args.extend(["--instruction-step", instruction_step])
        for outcome in MODULE.REQUIRED_OUTCOMES:
            args.extend(["--outcome", outcome])
        for status in MODULE.REQUIRED_RECONCILIATION_STATUSES:
            args.extend(["--reconciliation-status", status])
    elif kind == "settlement_submitter":
        args.extend(
            [
                "--configured-signer-count",
                "2",
                "--signer",
                "appeal-finance-submitter-signer-00",
                "--signer",
                "appeal-finance-submitter-signer-01",
                "--queued-step-count",
                "2",
                "--submitted-step",
                "appeal-finance-submitter-step-00",
                "--submitted-step",
                "appeal-finance-submitter-step-01",
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
                "--replayed-ballot",
                "appeal-finance-worker-ballot-00",
                "--replayed-ballot",
                "appeal-finance-worker-ballot-01",
                "--replayed-ballot",
                "appeal-finance-worker-ballot-02",
                "--max-settlement-lag-seconds",
                "60",
            ]
        )
    elif kind == "governance_dag_publication":
        args.extend(
            [
                "--report-count",
                "2",
                "--report",
                "appeal-finance-report-00",
                "--report",
                "appeal-finance-report-01",
                "--weekly-rollup-count",
                "1",
                "--weekly-rollup",
                "appeal-finance-weekly-rollup-00",
                "--settlement-receipt-count",
                "2",
                "--settlement-receipt",
                "appeal-finance-settlement-receipt-00",
                "--settlement-receipt",
                "appeal-finance-settlement-receipt-01",
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
        for index in range(CHECKER.DEFAULT_MIN_PEERS):
            args.extend(["--peer", f"appeal-finance-peer-{index:02d}"])
        for index in range(CHECKER.DEFAULT_MIN_PEERS):
            args.extend(["--validator", f"appeal-finance-validator-{index:02d}"])
        args.extend(
            [
                "--reconciliation-case",
                "appeal-finance-case-00",
                "--reconciliation-case",
                "appeal-finance-case-01",
            ]
        )
    elif kind == "governance_approval":
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    if kind in MODULE.POLICY_DIGEST_KINDS and "--policy-digest-hex" not in args:
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


def replace_first_option_value(args: list[str], option: str, value: str) -> None:
    """Replace the first value for a repeated CLI option."""

    args[args.index(option) + 1] = value


def test_builds_payload_free_dashboard_metrics_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("dashboard_metrics", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "dashboard_metrics").read_text("utf-8"))

    assert payload["schema"] == "sorafs.appeal_finance.dashboard_metrics_canary.v1"
    assert payload["config_digest_hex"] == CONFIG_DIGEST
    assert payload["metrics"] == list(MODULE.REQUIRED_METRICS)
    assert payload["payload_kind_count"] == len(MODULE.REQUIRED_PAYLOAD_KINDS)
    assert payload["payload_kinds"] == list(MODULE.REQUIRED_PAYLOAD_KINDS)
    for claim in MODULE.TRUE_CLAIMS["dashboard_metrics"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["dashboard_metrics"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "dashboard_metrics"
    assert errors == []


def test_builds_payload_free_deposit_lifecycle_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("deposit_lifecycle", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "deposit_lifecycle").read_text("utf-8")
    )

    assert payload["schema"] == "sorafs.appeal_finance.deposit_lifecycle_canary.v1"
    assert payload["deposit_probe_count"] == 2
    assert payload["confirmed_deposit_count"] == 2
    assert payload["deposit_probes"] == [
        {"name": "appeal-finance-deposit-probe-00", "confirmed": True},
        {"name": "appeal-finance-deposit-probe-01", "confirmed": True},
    ]
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "deposit_lifecycle"
    assert errors == []


@pytest.mark.parametrize(
    "kind",
    ("quote_api", "deposit_lifecycle", "settlement_execution"),
)
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


def test_builds_payload_free_settlement_submitter_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("settlement_submitter", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "settlement_submitter").read_text("utf-8")
    )

    assert payload["schema"] == "sorafs.appeal_finance.settlement_submitter_canary.v1"
    assert payload["configured_signer_count"] == 2
    assert payload["signers"] == [
        {"name": "appeal-finance-submitter-signer-00"},
        {"name": "appeal-finance-submitter-signer-01"},
    ]
    assert payload["queued_step_count"] == 2
    assert payload["submitted_step_count"] == 2
    assert payload["steps"] == [
        {"name": "appeal-finance-submitter-step-00", "submitted": True},
        {"name": "appeal-finance-submitter-step-01", "submitted": True},
    ]
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "settlement_submitter"
    assert errors == []


def test_builds_payload_free_moderation_worker_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("moderation_worker", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "moderation_worker").read_text("utf-8")
    )

    assert payload["schema"] == "sorafs.appeal_finance.moderation_worker_canary.v1"
    assert payload["ballot_replay_count"] == 3
    assert payload["ballots"] == [
        {"name": "appeal-finance-worker-ballot-00"},
        {"name": "appeal-finance-worker-ballot-01"},
        {"name": "appeal-finance-worker-ballot-02"},
    ]
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "moderation_worker"
    assert errors == []


def test_builds_payload_free_governance_dag_publication_canary(
    tmp_path: Path,
) -> None:
    assert MODULE.main(args_for("governance_dag_publication", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "governance_dag_publication").read_text("utf-8")
    )

    assert payload["schema"] == (
        "sorafs.appeal_finance.governance_dag_publication_canary.v1"
    )
    assert payload["report_count"] == 2
    assert payload["reports"] == [
        {"name": "appeal-finance-report-00"},
        {"name": "appeal-finance-report-01"},
    ]
    assert payload["weekly_rollup_count"] == 1
    assert payload["weekly_rollups"] == [
        {"name": "appeal-finance-weekly-rollup-00"},
    ]
    assert payload["settlement_receipt_count"] == 2
    assert payload["settlement_receipts"] == [
        {"name": "appeal-finance-settlement-receipt-00"},
        {"name": "appeal-finance-settlement-receipt-01"},
    ]
    assert payload["payload_kind_count"] == len(MODULE.REQUIRED_PAYLOAD_KINDS)
    assert payload["payload_kinds"] == list(MODULE.REQUIRED_PAYLOAD_KINDS)
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "governance_dag_publication"
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
    reconciliation_payload = json.loads(
        canary_path(tmp_path, "multi_peer_reconciliation").read_text("utf-8")
    )
    assert [peer["name"] for peer in reconciliation_payload["peers"]] == [
        f"appeal-finance-peer-{index:02d}"
        for index in range(CHECKER.DEFAULT_MIN_PEERS)
    ]
    assert [validator["name"] for validator in reconciliation_payload["validators"]] == [
        f"appeal-finance-validator-{index:02d}"
        for index in range(CHECKER.DEFAULT_MIN_PEERS)
    ]
    assert reconciliation_payload["cases"] == [
        {"name": "appeal-finance-case-00", "reconciled": True},
        {"name": "appeal-finance-case-01", "reconciled": True},
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
    assert payload["config_version"] == "appeal-finance-config-baseline-v1"
    assert payload["policy_digest_hex"] == POLICY_DIGEST
    assert payload["class_count"] == len(MODULE.REQUIRED_APPEAL_CLASSES)
    assert payload["classes"] == list(MODULE.REQUIRED_APPEAL_CLASSES)


def test_config_version_rejects_malformed_value_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("pricing_config", tmp_path)
    version_index = args.index("--config-version")
    args[version_index + 1] = "latest"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        CHECKER.CONFIG_VERSION_ERROR.replace("config_version", "--config-version")
        in captured.err
    )
    assert not canary_path(tmp_path, "pricing_config").exists()


def test_config_version_rejects_non_production_marker_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("pricing_config", tmp_path)
    version_index = args.index("--config-version")
    args[version_index + 1] = "appeal-finance-config-dev-baseline-v1"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--config-version must not contain non-production markers ['dev']"
        in captured.err
    )
    assert not canary_path(tmp_path, "pricing_config").exists()


def test_config_version_rejects_generic_name_family_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("pricing_config", tmp_path)
    version_index = args.index("--config-version")
    args[version_index + 1] = "baseline-v1"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        CHECKER.CONFIG_VERSION_ERROR.replace("config_version", "--config-version")
        in captured.err
    )
    assert not canary_path(tmp_path, "pricing_config").exists()


def test_config_version_accepts_reviewed_future_label(tmp_path: Path) -> None:
    args = args_for("pricing_config", tmp_path)
    version_index = args.index("--config-version")
    args[version_index + 1] = "appeal-finance-config-governance-baseline-v12"

    assert MODULE.main(args) == 0

    payload = json.loads(canary_path(tmp_path, "pricing_config").read_text("utf-8"))
    assert payload["config_version"] == "appeal-finance-config-governance-baseline-v12"
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "pricing_config"
    assert errors == []


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


@pytest.mark.parametrize(
    "kind",
    ("quote_api", "deposit_lifecycle", "settlement_execution"),
)
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


def test_missing_pricing_class_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("pricing_config", tmp_path)
    index = args.index("--appeal-class")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--appeal-class must include every required value" in captured.err
    assert not canary_path(tmp_path, "pricing_config").exists()


@pytest.mark.parametrize(
    ("kind", "option", "duplicate_value", "unknown_value"),
    (
        (
            "settlement_submitter",
            "--verified-claim",
            MODULE.TRUE_CLAIMS["settlement_submitter"][0],
            "unreviewed-appeal-finance-claim",
        ),
        (
            "pricing_config",
            "--appeal-class",
            MODULE.REQUIRED_APPEAL_CLASSES[0],
            "unreviewed-appeal-class",
        ),
        (
            "quote_api",
            "--quote-route",
            MODULE.REQUIRED_QUOTE_ROUTES[0],
            "unreviewed-quote-route",
        ),
        (
            "quote_api",
            "--urgency",
            MODULE.REQUIRED_URGENCIES[0],
            "unreviewed-urgency",
        ),
        (
            "deposit_lifecycle",
            "--deposit-route",
            MODULE.REQUIRED_DEPOSIT_ROUTES[0],
            "unreviewed-deposit-route",
        ),
        (
            "settlement_execution",
            "--settlement-route",
            MODULE.REQUIRED_SETTLEMENT_ROUTES[0],
            "unreviewed-settlement-route",
        ),
        (
            "settlement_execution",
            "--outcome",
            MODULE.REQUIRED_OUTCOMES[0],
            "unreviewed-settlement-outcome",
        ),
        (
            "settlement_execution",
            "--instruction-step",
            MODULE.REQUIRED_SETTLEMENT_INSTRUCTION_STEPS[0],
            "unreviewed-instruction-step",
        ),
        (
            "settlement_execution",
            "--reconciliation-status",
            MODULE.REQUIRED_RECONCILIATION_STATUSES[0],
            "unreviewed-reconciliation-status",
        ),
        (
            "governance_dag_publication",
            "--payload-kind",
            MODULE.REQUIRED_PAYLOAD_KINDS[0],
            "unreviewed-payload-kind",
        ),
        (
            "dashboard_metrics",
            "--metric",
            MODULE.REQUIRED_METRICS[0],
            "unreviewed-appeal-finance-metric",
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


@pytest.mark.parametrize(
    ("kind", "option", "legacy_label", "expected_error"),
    (
        (
            "deposit_lifecycle",
            "--confirmed-deposit-probe",
            "deposit-probe-00",
            "--confirmed-deposit-probe must match canonical lowercase "
            "`appeal-finance-deposit-probe-name`",
        ),
        (
            "settlement_submitter",
            "--signer",
            "submitter-signer-00",
            "--signer must match canonical lowercase "
            "`appeal-finance-submitter-signer-name`",
        ),
        (
            "settlement_submitter",
            "--submitted-step",
            "settlement-step-00",
            "--submitted-step must match canonical lowercase "
            "`appeal-finance-submitter-step-name`",
        ),
        (
            "governance_dag_publication",
            "--report",
            "report-00",
            "--report must match canonical lowercase `appeal-finance-report-*`",
        ),
        (
            "governance_dag_publication",
            "--weekly-rollup",
            "weekly-rollup-00",
            "--weekly-rollup must match canonical lowercase "
            "`appeal-finance-weekly-rollup-name`",
        ),
        (
            "governance_dag_publication",
            "--settlement-receipt",
            "settlement-receipt-00",
            "--settlement-receipt must match canonical lowercase "
            "`appeal-finance-settlement-receipt-name`",
        ),
    ),
)
def test_reviewed_inventory_labels_reject_legacy_families_before_write(
    kind: str,
    option: str,
    legacy_label: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for(kind, tmp_path)
    replace_first_option_value(args, option, legacy_label)

    assert_rejected_without_artifact(
        args,
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=expected_error,
    )


@pytest.mark.parametrize(
    ("kind", "option", "placeholder_label"),
    (
        (
            "deposit_lifecycle",
            "--confirmed-deposit-probe",
            "appeal-finance-deposit-probe-placeholder",
        ),
        (
            "settlement_submitter",
            "--signer",
            "appeal-finance-submitter-signer-placeholder",
        ),
        (
            "settlement_submitter",
            "--submitted-step",
            "appeal-finance-submitter-step-placeholder",
        ),
        (
            "governance_dag_publication",
            "--report",
            "appeal-finance-report-placeholder",
        ),
        (
            "governance_dag_publication",
            "--weekly-rollup",
            "appeal-finance-weekly-rollup-placeholder",
        ),
        (
            "governance_dag_publication",
            "--settlement-receipt",
            "appeal-finance-settlement-receipt-placeholder",
        ),
    ),
)
def test_reviewed_inventory_labels_reject_placeholder_markers_before_write(
    kind: str,
    option: str,
    placeholder_label: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for(kind, tmp_path)
    replace_first_option_value(args, option, placeholder_label)

    assert_rejected_without_artifact(
        args,
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=f"{option} must not contain non-production markers ['placeholder']",
    )


def test_unconfirmed_deposit_probe_label_family_is_validated_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("deposit_lifecycle", tmp_path)
    replace_first_option_value(args, "--deposit-probe-count", "3")
    args.extend(["--unconfirmed-deposit-probe", "deposit-probe-02"])

    assert_rejected_without_artifact(
        args,
        kind="deposit_lifecycle",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--unconfirmed-deposit-probe must match canonical lowercase "
            "`appeal-finance-deposit-probe-name`"
        ),
    )


def test_queued_only_submitter_step_label_family_is_validated_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("settlement_submitter", tmp_path)
    replace_first_option_value(args, "--queued-step-count", "3")
    args.extend(["--queued-only-step", "settlement-step-02"])

    assert_rejected_without_artifact(
        args,
        kind="settlement_submitter",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--queued-only-step must match canonical lowercase "
            "`appeal-finance-submitter-step-name`"
        ),
    )


def test_pricing_class_count_must_match_inventory(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("pricing_config", tmp_path)
    args[args.index("--class-count") + 1] = str(len(MODULE.REQUIRED_APPEAL_CLASSES) + 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--class-count must match --appeal-class inventory" in captured.err
    assert not canary_path(tmp_path, "pricing_config").exists()


def test_quote_count_must_match_required_class_urgency_product(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("quote_api", tmp_path)
    args[args.index("--quote-count") + 1] = str(MODULE.REQUIRED_QUOTE_API_QUOTES - 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--quote-count must match required class/urgency product" in captured.err
    assert not canary_path(tmp_path, "quote_api").exists()


def test_missing_settlement_instruction_step_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("settlement_execution", tmp_path)
    index = args.index("--instruction-step")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--instruction-step must include every required value" in captured.err
    assert not canary_path(tmp_path, "settlement_execution").exists()


def test_settlement_instruction_step_count_must_match_inventory(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("settlement_execution", tmp_path)
    args[args.index("--instruction-step-count") + 1] = str(
        len(MODULE.REQUIRED_SETTLEMENT_INSTRUCTION_STEPS) + 1
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "instruction_step_count must match unique instruction_steps count" in captured.err
    assert not canary_path(tmp_path, "settlement_execution").exists()


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


def test_multi_peer_peer_inventory_must_match_peer_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("multi_peer_reconciliation", tmp_path)
    args[args.index("--peer-count") + 1] = str(CHECKER.DEFAULT_MIN_PEERS + 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--peer unique values must match --peer-count" in captured.err
    assert not canary_path(tmp_path, "multi_peer_reconciliation").exists()


def test_multi_peer_peer_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("multi_peer_reconciliation", tmp_path)
    first_peer = args.index("--peer") + 1
    args.extend(["--peer", args[first_peer]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--peer must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "multi_peer_reconciliation").exists()


def test_multi_peer_peer_inventory_must_use_production_family(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("multi_peer_reconciliation", tmp_path)
    first_peer = args.index("--peer") + 1
    args[first_peer] = "peer-00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--peer must match canonical lowercase `appeal-finance-peer-*`"
        in captured.err
    )
    assert not canary_path(tmp_path, "multi_peer_reconciliation").exists()


def test_multi_peer_peer_inventory_rejects_placeholder_marker(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("multi_peer_reconciliation", tmp_path)
    first_peer = args.index("--peer") + 1
    args[first_peer] = "appeal-finance-peer-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--peer must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "multi_peer_reconciliation").exists()


def test_multi_peer_validator_inventory_must_match_validator_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("multi_peer_reconciliation", tmp_path)
    args[args.index("--validator-count") + 1] = str(CHECKER.DEFAULT_MIN_PEERS + 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--validator unique values must match --validator-count" in captured.err
    assert not canary_path(tmp_path, "multi_peer_reconciliation").exists()


def test_multi_peer_validator_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("multi_peer_reconciliation", tmp_path)
    first_validator = args.index("--validator") + 1
    args.extend(["--validator", args[first_validator]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--validator must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "multi_peer_reconciliation").exists()


def test_multi_peer_validator_inventory_rejects_peer_family(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("multi_peer_reconciliation", tmp_path)
    first_validator = args.index("--validator") + 1
    args[first_validator] = "appeal-finance-peer-00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--validator must match canonical lowercase "
        "`appeal-finance-validator-name`"
        in captured.err
    )
    assert not canary_path(tmp_path, "multi_peer_reconciliation").exists()


def test_multi_peer_case_inventory_must_match_case_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("multi_peer_reconciliation", tmp_path)
    args[args.index("--case-count") + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--reconciliation-case unique values must match --case-count" in captured.err
    assert not canary_path(tmp_path, "multi_peer_reconciliation").exists()


def test_multi_peer_case_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("multi_peer_reconciliation", tmp_path)
    first_case = args.index("--reconciliation-case") + 1
    args.extend(["--reconciliation-case", args[first_case]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--reconciliation-case must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "multi_peer_reconciliation").exists()


def test_multi_peer_case_inventory_must_be_canonical(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("multi_peer_reconciliation", tmp_path)
    first_case = args.index("--reconciliation-case") + 1
    args[first_case] = "appeal_finance_case_00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--reconciliation-case must match canonical lowercase "
        "`appeal-finance-case-*`"
        in captured.err
    )
    assert not canary_path(tmp_path, "multi_peer_reconciliation").exists()


def test_deposit_confirmed_probe_inventory_must_match_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("deposit_lifecycle", tmp_path)
    args[args.index("--confirmed-deposit-count") + 1] = "1"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--confirmed-deposit-probe unique values must match "
        "--confirmed-deposit-count"
        in captured.err
    )
    assert not canary_path(tmp_path, "deposit_lifecycle").exists()


def test_deposit_unconfirmed_probe_inventory_is_required_for_unconfirmed_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("deposit_lifecycle", tmp_path)
    args[args.index("--deposit-probe-count") + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--unconfirmed-deposit-probe is required for deposit_lifecycle" in captured.err
    assert not canary_path(tmp_path, "deposit_lifecycle").exists()


def test_deposit_probe_inventories_must_not_overlap(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("deposit_lifecycle", tmp_path)
    args[args.index("--deposit-probe-count") + 1] = "3"
    args.extend(["--unconfirmed-deposit-probe", "appeal-finance-deposit-probe-00"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--confirmed-deposit-probe and --unconfirmed-deposit-probe must not overlap"
        in captured.err
    )
    assert not canary_path(tmp_path, "deposit_lifecycle").exists()


def test_submitter_signer_inventory_must_match_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("settlement_submitter", tmp_path)
    args[args.index("--configured-signer-count") + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--signer unique values must match --configured-signer-count" in captured.err
    assert not canary_path(tmp_path, "settlement_submitter").exists()


def test_submitter_submitted_step_inventory_must_match_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("settlement_submitter", tmp_path)
    args[args.index("--submitted-step-count") + 1] = "1"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--submitted-step unique values must match --submitted-step-count"
        in captured.err
    )
    assert not canary_path(tmp_path, "settlement_submitter").exists()


def test_submitter_queued_only_step_inventory_is_required_for_pending_steps(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("settlement_submitter", tmp_path)
    args[args.index("--queued-step-count") + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--queued-only-step is required for settlement_submitter" in captured.err
    assert not canary_path(tmp_path, "settlement_submitter").exists()


def test_submitter_step_inventories_must_not_overlap(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("settlement_submitter", tmp_path)
    args[args.index("--queued-step-count") + 1] = "3"
    args.extend(["--queued-only-step", "appeal-finance-submitter-step-00"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--submitted-step and --queued-only-step must not overlap" in captured.err
    assert not canary_path(tmp_path, "settlement_submitter").exists()


def test_governance_dag_report_inventory_must_match_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_dag_publication", tmp_path)
    args[args.index("--report-count") + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--report unique values must match --report-count" in captured.err
    assert not canary_path(tmp_path, "governance_dag_publication").exists()


def test_governance_dag_weekly_rollup_inventory_is_required(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_dag_publication", tmp_path)
    index = args.index("--weekly-rollup")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--weekly-rollup is required for governance_dag_publication" in captured.err
    assert not canary_path(tmp_path, "governance_dag_publication").exists()


def test_governance_dag_settlement_receipt_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_dag_publication", tmp_path)
    first_receipt = args.index("--settlement-receipt") + 1
    args.extend(["--settlement-receipt", args[first_receipt]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--settlement-receipt must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "governance_dag_publication").exists()


def test_moderation_worker_ballot_inventory_must_match_replay_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("moderation_worker", tmp_path)
    first_ballot = args.index("--replayed-ballot")
    del args[first_ballot : first_ballot + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--replayed-ballot unique values must match --ballot-replay-count"
        in captured.err
    )
    assert not canary_path(tmp_path, "moderation_worker").exists()


def test_moderation_worker_ballot_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("moderation_worker", tmp_path)
    first_ballot_value = args[args.index("--replayed-ballot") + 1]
    args.extend(["--replayed-ballot", first_ballot_value])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--replayed-ballot must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "moderation_worker").exists()


@pytest.mark.parametrize(
    ("label", "expected_error"),
    (
        (
            "worker-ballot-00",
            "--replayed-ballot must match canonical lowercase "
            "`appeal-finance-worker-ballot-name`",
        ),
        (
            "appeal-finance-worker-ballot-placeholder",
            "--replayed-ballot must not contain non-production markers ['placeholder']",
        ),
    ),
)
def test_moderation_worker_ballot_inventory_must_use_reviewed_labels_before_write(
    label: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("moderation_worker", tmp_path)
    args[args.index("--replayed-ballot") + 1] = label

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, "moderation_worker").exists()


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


def test_output_directory_is_rejected(tmp_path: Path, capsys) -> None:
    output_dir = canary_path(tmp_path, "pricing_config")
    output_dir.mkdir()

    assert MODULE.main(args_for("pricing_config", tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a directory" in captured.err
    assert output_dir.is_dir()
