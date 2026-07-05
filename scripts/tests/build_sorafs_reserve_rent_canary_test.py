"""Tests for scripts/build_sorafs_reserve_rent_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_reserve_rent_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_reserve_rent_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_reserve_rent_canary",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_reserve_rent_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


POLICY_DIGEST = "a" * 64
MATRIX_DIGEST = "b" * 64
LEDGER_DIGEST = "c" * 64
ROUTE_BODY_DIGEST = "d" * 64
GENERATED_AT = 1_800_100_000


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=GENERATED_AT,
        max_ledger_age_secs=CHECKER.DEFAULT_MAX_LEDGER_AGE_SECS,
        max_lifecycle_lag_secs=CHECKER.DEFAULT_MAX_LIFECYCLE_LAG_SECS,
        max_route_latency_ms=CHECKER.DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_bake_age_secs=CHECKER.DEFAULT_MAX_BAKE_AGE_SECS,
    )


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "sorafs-reserve-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(GENERATED_AT),
        "--policy-digest-hex",
        POLICY_DIGEST,
    ]
    if kind in MODULE.MATRIX_DIGEST_KINDS:
        args.extend(["--matrix-digest-hex", MATRIX_DIGEST])
    if kind in MODULE.LEDGER_DIGEST_KINDS:
        args.extend(["--ledger-digest-hex", LEDGER_DIGEST])
    for claim in MODULE.TRUE_CLAIMS[kind]:
        args.extend(["--verified-claim", claim])
    if kind == "policy_config":
        args.extend(
            [
                "--policy-version",
                "1",
                "--tier-count",
                "3",
                "--storage-class-count",
                "3",
                "--duration-count",
                "3",
            ]
        )
    elif kind == "quote_matrix":
        args.extend(["--scenario-count", "27"])
        for storage_class in MODULE.REQUIRED_STORAGE_CLASSES:
            args.extend(["--storage-class", storage_class])
        for tier in MODULE.REQUIRED_TIERS:
            args.extend(["--tier", tier])
        for duration in MODULE.REQUIRED_DURATIONS:
            args.extend(["--duration", duration])
    elif kind == "ledger_digest":
        args.extend(["--ledger-count", "1", "--instruction-count", "2"])
        args.extend(["--ledger-ref", "reserve-ledger-main"])
        args.extend(["--instruction-ref", "reserve-instruction-rent-settlement"])
        args.extend(["--instruction-ref", "reserve-instruction-reserve-top-up"])
    elif kind == "lifecycle_service":
        args.extend(["--route-body-blake3-hex", ROUTE_BODY_DIGEST])
        args.extend(
            [
                "--max-lifecycle-lag-seconds",
                "60",
                "--persisted-stage-count",
                "4",
            ]
        )
        for route in MODULE.REQUIRED_LIFECYCLE_ROUTES:
            args.extend(["--lifecycle-route", route])
        for stage in (
            "reserve-lifecycle-stage-active",
            "reserve-lifecycle-stage-warning",
            "reserve-lifecycle-stage-defaulted",
            "reserve-lifecycle-stage-suspended",
        ):
            args.extend(["--persisted-stage", stage])
    elif kind == "signed_routes":
        args.extend(["--route-body-blake3-hex", ROUTE_BODY_DIGEST])
        args.extend(["--max-route-latency-ms", "250"])
        for route in MODULE.REQUIRED_SIGNED_ROUTES:
            args.extend(["--signed-route", route])
    elif kind == "reserve_movement":
        args.extend(
            [
                "--movement-count",
                "4",
                "--chain-submission-count",
                "4",
                "--finality-poll-attempt-count",
                "4",
            ]
        )
        for action in MODULE.REQUIRED_RESERVE_MOVEMENT_ACTIONS:
            args.extend(["--movement-action", action])
    elif kind == "credit_line":
        args.extend(
            [
                "--credit-line-mutation-count",
                "2",
                "--accrual-cycle-count",
                "2",
            ]
        )
        for mutation in MODULE.REQUIRED_CREDIT_LINE_MUTATIONS:
            args.extend(["--credit-line-mutation", mutation])
        for cycle in MODULE.REQUIRED_CREDIT_LINE_ACCRUAL_CYCLES:
            args.extend(["--accrual-cycle", cycle])
    elif kind == "appeal_policy":
        args.extend(
            [
                "--appeal-probe-count",
                "2",
                "--approved-appeal-count",
                "1",
                "--rejected-appeal-count",
                "1",
            ]
        )
        for probe in MODULE.REQUIRED_APPEAL_POLICY_PROBES:
            args.extend(["--appeal-probe", probe])
    elif kind == "metrics_alerts":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    elif kind == "provider_bake":
        args.extend(
            [
                "--bake-id",
                "reserve-bake-001",
                "--started-at-unix",
                str(GENERATED_AT - 3_600),
                "--completed-at-unix",
                str(GENERATED_AT),
                "--provider-count",
                "3",
                "--rent-cycle-count",
                "2",
                "--top-up-cycle-count",
                "2",
                "--appeal-cycle-count",
                "1",
                "--scheduled-lifecycle-canary-last-tick-unix",
                str(GENERATED_AT - 60),
                "--scheduled-lifecycle-canary-tick-count",
                "2",
                "--scheduled-lifecycle-canary-defaulted-provider-count",
                "1",
            ]
        )
        for provider in ("provider-alpha", "provider-beta", "provider-gamma"):
            args.extend(["--provider", provider])
        for cycle in ("reserve-rent-cycle-001", "reserve-rent-cycle-002"):
            args.extend(["--rent-cycle", cycle])
        for cycle in ("reserve-top-up-cycle-001", "reserve-top-up-cycle-002"):
            args.extend(["--top-up-cycle", cycle])
        args.extend(["--appeal-cycle", "reserve-appeal-cycle-001"])
        for tick in ("reserve-lifecycle-tick-001", "reserve-lifecycle-tick-002"):
            args.extend(["--scheduled-lifecycle-canary-tick", tick])
    elif kind == "governance_approval":
        args.extend(
            [
                "--bake-id",
                "reserve-bake-001",
                "--downstream-compliance-consumer-count",
                "2",
                "--downstream-compliance-consumer",
                "reserve-compliance-consumer-gateway",
                "--downstream-compliance-consumer",
                "reserve-compliance-consumer-orderbook",
            ]
        )
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


def test_builds_payload_free_provider_bake_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("provider_bake", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "provider_bake").read_text("utf-8"))

    assert payload["schema"] == "sorafs.reserve.provider_bake.v1"
    assert payload["policy_digest_hex"] == POLICY_DIGEST
    assert payload["matrix_digest_hex"] == MATRIX_DIGEST
    assert payload["ledger_digest_hex"] == LEDGER_DIGEST
    assert payload["completed_provider_count"] == payload["provider_count"]
    assert payload["failure_count"] == 0
    assert [record["name"] for record in payload["providers"]] == [
        "provider-alpha",
        "provider-beta",
        "provider-gamma",
    ]
    assert [record["defaulted"] for record in payload["providers"]] == [
        True,
        False,
        False,
    ]
    assert [record["name"] for record in payload["rent_cycles"]] == [
        "reserve-rent-cycle-001",
        "reserve-rent-cycle-002",
    ]
    assert [record["name"] for record in payload["top_up_cycles"]] == [
        "reserve-top-up-cycle-001",
        "reserve-top-up-cycle-002",
    ]
    assert [record["name"] for record in payload["appeal_cycles"]] == [
        "reserve-appeal-cycle-001",
    ]
    assert [record["name"] for record in payload["scheduled_lifecycle_canary_ticks"]] == [
        "reserve-lifecycle-tick-001",
        "reserve-lifecycle-tick-002",
    ]
    for claim in MODULE.TRUE_CLAIMS["provider_bake"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["provider_bake"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "provider_bake"
    assert errors == []


def test_provider_bake_id_must_be_canonical(tmp_path: Path, capsys) -> None:
    args = args_for("provider_bake", tmp_path)
    args[args.index("--bake-id") + 1] = "reserve_bake_001"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--bake-id must match canonical lowercase `reserve-bake-*`" in captured.err
    assert not canary_path(tmp_path, "provider_bake").exists()


def test_provider_bake_id_rejects_non_production_markers(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider_bake", tmp_path)
    args[args.index("--bake-id") + 1] = "reserve-bake-prod-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--bake-id must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "provider_bake").exists()


def test_provider_bake_id_accepts_future_production_label(tmp_path: Path) -> None:
    args = args_for("provider_bake", tmp_path)
    args[args.index("--bake-id") + 1] = "reserve-bake-prod-a-202607"

    assert MODULE.main(args) == 0

    payload = json.loads(canary_path(tmp_path, "provider_bake").read_text("utf-8"))
    assert payload["bake_id"] == "reserve-bake-prod-a-202607"


def test_generated_canaries_pass_full_reserve_rent_gate(tmp_path: Path) -> None:
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
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    assert payload["valid_policy_matrix_ledger_bindings"] == [
        {
            "policy_digest_hex": POLICY_DIGEST,
            "matrix_digest_hex": MATRIX_DIGEST,
            "ledger_digest_hex": LEDGER_DIGEST,
        }
    ]
    assert payload["valid_provider_bakes"] == [
        {
            "bake_id": "reserve-bake-001",
            "deployment_id": "sorafs-reserve-prod-20260701",
            "environment": "production",
            "policy_digest_hex": POLICY_DIGEST,
            "matrix_digest_hex": MATRIX_DIGEST,
            "ledger_digest_hex": LEDGER_DIGEST,
            "started_at_unix": GENERATED_AT - 3_600,
            "completed_at_unix": GENERATED_AT,
            "provider_count": 3,
            "scheduled_lifecycle_canary_last_tick_at_unix": GENERATED_AT - 60,
            "scheduled_lifecycle_canary_tick_count": 2,
            "scheduled_lifecycle_canary_defaulted_provider_count": 1,
        }
    ]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_builds_payload_free_ledger_digest_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("ledger_digest", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "ledger_digest").read_text("utf-8"))

    assert payload["schema"] == "sorafs.reserve.ledger_digest_canary.v1"
    assert payload["ledger_count"] == 1
    assert payload["ledgers"] == [{"name": "reserve-ledger-main"}]
    assert payload["instruction_count"] == 2
    assert payload["instructions"] == [
        {"name": "reserve-instruction-rent-settlement"},
        {"name": "reserve-instruction-reserve-top-up"},
    ]
    assert payload["raw_ledger_included"] is False
    assert payload["raw_transfer_instructions_included"] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "ledger_digest"
    assert errors == []


def test_builds_payload_free_lifecycle_service_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("lifecycle_service", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "lifecycle_service").read_text("utf-8"))

    assert payload["schema"] == "sorafs.reserve.lifecycle_service_canary.v1"
    assert payload["persisted_stage_count"] == 4
    assert payload["persisted_stages"] == [
        {"name": "reserve-lifecycle-stage-active"},
        {"name": "reserve-lifecycle-stage-warning"},
        {"name": "reserve-lifecycle-stage-defaulted"},
        {"name": "reserve-lifecycle-stage-suspended"},
    ]
    assert payload["response_bodies_included"] is False
    assert all(
        route["body_blake3_hex"] == ROUTE_BODY_DIGEST for route in payload["routes"]
    )
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "lifecycle_service"
    assert errors == []


def test_builds_payload_free_signed_routes_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("signed_routes", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "signed_routes").read_text("utf-8"))

    assert payload["schema"] == "sorafs.reserve.signed_route_canary.v1"
    assert payload["max_route_latency_ms"] == 250
    assert payload["response_bodies_included"] is False
    assert all(
        route["body_blake3_hex"] == ROUTE_BODY_DIGEST for route in payload["routes"]
    )
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "signed_routes"
    assert errors == []


def test_builds_payload_free_governance_approval_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("governance_approval", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "governance_approval").read_text("utf-8")
    )

    assert payload["schema"] == "sorafs.reserve.governance_approval.v1"
    assert payload["bake_id"] == "reserve-bake-001"
    assert payload["downstream_compliance_consumer_count"] == 2
    assert payload["downstream_compliance_consumers"] == [
        {"name": "reserve-compliance-consumer-gateway"},
        {"name": "reserve-compliance-consumer-orderbook"},
    ]
    for claim in MODULE.TRUE_CLAIMS["governance_approval"]:
        assert payload[claim] is True
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "governance_approval"
    assert errors == []


def test_governance_approval_requires_bake_id_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_approval", tmp_path)
    index = args.index("--bake-id")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--bake-id is required for governance_approval" in captured.err
    assert not canary_path(tmp_path, "governance_approval").exists()


def test_governance_approval_bake_id_must_be_canonical_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_approval", tmp_path)
    args[args.index("--bake-id") + 1] = "reserve_bake_001"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--bake-id must match canonical lowercase `reserve-bake-*`" in captured.err
    assert not canary_path(tmp_path, "governance_approval").exists()


def test_governance_approval_bake_id_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_approval", tmp_path)
    args[args.index("--bake-id") + 1] = "reserve-bake-prod-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--bake-id must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "governance_approval").exists()


def test_response_file_can_build_reserve_movement_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "reserve-movement.args"
    args_file.write_text(
        "\n".join(args_for("reserve_movement", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "reserve_movement").read_text("utf-8"))
    assert payload["movement_count"] == 4
    assert [record["action"] for record in payload["movements"]] == list(
        MODULE.REQUIRED_RESERVE_MOVEMENT_ACTIONS
    )
    assert payload["chain_submission_count"] == 4
    assert payload["finality_poll_attempt_count"] == 4


def test_quote_matrix_scenario_count_must_match_required_product(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("quote_matrix", tmp_path)
    args[args.index("--scenario-count") + 1] = str(
        MODULE.REQUIRED_QUOTE_MATRIX_SCENARIOS - 1
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--scenario-count must match required quote-matrix product" in captured.err
    assert not canary_path(tmp_path, "quote_matrix").exists()


@pytest.mark.parametrize("kind", ("lifecycle_service", "signed_routes"))
def test_route_canaries_require_route_body_digest(
    kind: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for(kind, tmp_path)
    index = args.index("--route-body-blake3-hex")
    del args[index : index + 2]

    assert_rejected_without_artifact(
        args,
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--route-body-blake3-hex must be exact lowercase 32-byte hex",
    )


@pytest.mark.parametrize(
    ("kind", "option", "duplicate_value", "unknown_value"),
    (
        (
            "reserve_movement",
            "--verified-claim",
            MODULE.TRUE_CLAIMS["reserve_movement"][0],
            "unreviewed_claim",
        ),
        (
            "quote_matrix",
            "--storage-class",
            MODULE.REQUIRED_STORAGE_CLASSES[0],
            "unreviewed-storage-class",
        ),
        ("quote_matrix", "--tier", MODULE.REQUIRED_TIERS[0], "unreviewed-tier"),
        (
            "quote_matrix",
            "--duration",
            MODULE.REQUIRED_DURATIONS[0],
            "unreviewed-duration",
        ),
        (
            "lifecycle_service",
            "--lifecycle-route",
            MODULE.REQUIRED_LIFECYCLE_ROUTES[0],
            "unreviewed-lifecycle-route",
        ),
        (
            "signed_routes",
            "--signed-route",
            MODULE.REQUIRED_SIGNED_ROUTES[0],
            "unreviewed-signed-route",
        ),
        (
            "reserve_movement",
            "--movement-action",
            MODULE.REQUIRED_RESERVE_MOVEMENT_ACTIONS[0],
            "unreviewed-movement-action",
        ),
        (
            "credit_line",
            "--credit-line-mutation",
            MODULE.REQUIRED_CREDIT_LINE_MUTATIONS[0],
            "unreviewed-credit-line-mutation",
        ),
        (
            "credit_line",
            "--accrual-cycle",
            MODULE.REQUIRED_CREDIT_LINE_ACCRUAL_CYCLES[0],
            "unreviewed-accrual-cycle",
        ),
        (
            "appeal_policy",
            "--appeal-probe",
            MODULE.REQUIRED_APPEAL_POLICY_PROBES[0],
            "unreviewed-appeal-probe",
        ),
        (
            "metrics_alerts",
            "--metric",
            MODULE.REQUIRED_METRICS[0],
            "unreviewed-reserve-metric",
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

    unknown_args = args_for(kind, tmp_path)
    unknown_args.extend([option, unknown_value])
    assert_rejected_without_artifact(
        unknown_args,
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=f"{option} contains an unknown value",
    )


def test_response_file_can_build_appeal_policy_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "appeal-policy.args"
    args_file.write_text(
        "\n".join(args_for("appeal_policy", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "appeal_policy").read_text("utf-8"))
    assert payload["appeal_probe_count"] == 2
    assert [record["name"] for record in payload["appeal_probes"]] == list(
        MODULE.REQUIRED_APPEAL_POLICY_PROBES
    )
    assert [record["outcome"] for record in payload["appeal_probes"]] == [
        "approved",
        "rejected",
    ]


def test_response_file_can_build_credit_line_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "credit-line.args"
    args_file.write_text(
        "\n".join(args_for("credit_line", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "credit_line").read_text("utf-8"))
    assert payload["credit_line_mutation_count"] == 2
    assert [record["name"] for record in payload["credit_line_mutations"]] == list(
        MODULE.REQUIRED_CREDIT_LINE_MUTATIONS
    )
    assert payload["accrual_cycle_count"] == 2
    assert [record["name"] for record in payload["accrual_cycles"]] == list(
        MODULE.REQUIRED_CREDIT_LINE_ACCRUAL_CYCLES
    )


def test_missing_movement_action_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("reserve_movement", tmp_path)
    index = args.index("--movement-action")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--movement-action must include every required value" in captured.err
    assert not canary_path(tmp_path, "reserve_movement").exists()


def test_movement_count_must_match_action_inventory(tmp_path: Path, capsys) -> None:
    args = args_for("reserve_movement", tmp_path)
    index = args.index("--movement-count")
    args[index + 1] = "5"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--movement-count must match --movement-action inventory" in captured.err
    assert not canary_path(tmp_path, "reserve_movement").exists()


def test_missing_appeal_probe_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("appeal_policy", tmp_path)
    index = args.index("--appeal-probe")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--appeal-probe must include every required value" in captured.err
    assert not canary_path(tmp_path, "appeal_policy").exists()


def test_appeal_probe_counts_must_match_inventory(tmp_path: Path, capsys) -> None:
    args = args_for("appeal_policy", tmp_path)
    index = args.index("--approved-appeal-count")
    args[index + 1] = "2"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--approved-appeal-count must match approved --appeal-probe inventory"
        in captured.err
    )
    assert not canary_path(tmp_path, "appeal_policy").exists()


def test_missing_credit_line_mutation_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("credit_line", tmp_path)
    index = args.index("--credit-line-mutation")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--credit-line-mutation must include every required value" in captured.err
    assert not canary_path(tmp_path, "credit_line").exists()


def test_credit_line_mutation_count_must_match_inventory(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("credit_line", tmp_path)
    index = args.index("--credit-line-mutation-count")
    args[index + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--credit-line-mutation-count must match --credit-line-mutation inventory"
        in captured.err
    )
    assert not canary_path(tmp_path, "credit_line").exists()


def test_missing_accrual_cycle_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("credit_line", tmp_path)
    index = args.index("--accrual-cycle")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--accrual-cycle must include every required value" in captured.err
    assert not canary_path(tmp_path, "credit_line").exists()


def test_accrual_cycle_count_must_match_inventory(tmp_path: Path, capsys) -> None:
    args = args_for("credit_line", tmp_path)
    index = args.index("--accrual-cycle-count")
    args[index + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--accrual-cycle-count must match --accrual-cycle inventory" in captured.err
    assert not canary_path(tmp_path, "credit_line").exists()


def test_missing_provider_bake_provider_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider_bake", tmp_path)
    index = args.index("--provider")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--provider-count must match --provider inventory" in captured.err
    assert not canary_path(tmp_path, "provider_bake").exists()


def test_duplicate_provider_bake_provider_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider_bake", tmp_path)
    args.extend(["--provider", "provider-alpha"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--provider must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "provider_bake").exists()


def test_provider_bake_provider_must_use_reviewed_label_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider_bake", tmp_path)
    provider_index = args.index("--provider")
    args[provider_index + 1] = "provider_alpha"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--provider must match canonical lowercase `provider-*`" in captured.err
    assert not canary_path(tmp_path, "provider_bake").exists()


def test_provider_bake_provider_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider_bake", tmp_path)
    provider_index = args.index("--provider")
    args[provider_index + 1] = "provider-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--provider must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "provider_bake").exists()


@pytest.mark.parametrize(
    ("option", "duplicate_value"),
    (
        ("--rent-cycle", "reserve-rent-cycle-001"),
        ("--top-up-cycle", "reserve-top-up-cycle-001"),
        ("--appeal-cycle", "reserve-appeal-cycle-001"),
    ),
)
def test_provider_bake_cycle_inputs_must_not_duplicate_before_write(
    option: str,
    duplicate_value: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider_bake", tmp_path)
    args.extend([option, duplicate_value])

    assert_rejected_without_artifact(
        args,
        kind="provider_bake",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=f"{option} must not contain duplicates",
    )


@pytest.mark.parametrize(
    ("option", "replacement", "expected_error"),
    (
        (
            "--rent-cycle",
            "rent-cycle-001",
            "--rent-cycle must match canonical lowercase `reserve-rent-cycle-*`",
        ),
        (
            "--top-up-cycle",
            "top-up-cycle-001",
            "--top-up-cycle must match canonical lowercase `reserve-top-up-cycle-*`",
        ),
        (
            "--appeal-cycle",
            "appeal-cycle-001",
            "--appeal-cycle must match canonical lowercase `reserve-appeal-cycle-*`",
        ),
    ),
)
def test_provider_bake_cycle_inputs_must_use_reviewed_labels_before_write(
    option: str,
    replacement: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider_bake", tmp_path)
    args[args.index(option) + 1] = replacement

    assert_rejected_without_artifact(
        args,
        kind="provider_bake",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=expected_error,
    )


@pytest.mark.parametrize(
    ("option", "replacement", "expected_error"),
    (
        (
            "--rent-cycle",
            "reserve-rent-cycle-placeholder",
            "--rent-cycle must not contain non-production markers ['placeholder']",
        ),
        (
            "--top-up-cycle",
            "reserve-top-up-cycle-sample",
            "--top-up-cycle must not contain non-production markers ['sample']",
        ),
        (
            "--appeal-cycle",
            "reserve-appeal-cycle-test",
            "--appeal-cycle must not contain non-production markers ['test']",
        ),
    ),
)
def test_provider_bake_cycle_inputs_reject_non_production_markers_before_write(
    option: str,
    replacement: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider_bake", tmp_path)
    args[args.index(option) + 1] = replacement

    assert_rejected_without_artifact(
        args,
        kind="provider_bake",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=expected_error,
    )


def test_provider_bake_cycle_count_must_match_inventory(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider_bake", tmp_path)
    args[args.index("--rent-cycle-count") + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--rent-cycle-count must match --rent-cycle inventory" in captured.err
    assert not canary_path(tmp_path, "provider_bake").exists()


def test_provider_bake_tick_inventory_must_match_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider_bake", tmp_path)
    args[args.index("--scheduled-lifecycle-canary-tick-count") + 1] = "3"

    assert_rejected_without_artifact(
        args,
        kind="provider_bake",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--scheduled-lifecycle-canary-tick-count must match "
            "--scheduled-lifecycle-canary-tick inventory"
        ),
    )


def test_provider_bake_tick_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider_bake", tmp_path)
    args.extend(["--scheduled-lifecycle-canary-tick", "reserve-lifecycle-tick-001"])

    assert_rejected_without_artifact(
        args,
        kind="provider_bake",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--scheduled-lifecycle-canary-tick must not contain duplicates",
    )


def test_provider_bake_tick_inventory_must_use_reviewed_labels_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider_bake", tmp_path)
    args[
        args.index("--scheduled-lifecycle-canary-tick") + 1
    ] = "lifecycle-tick-001"

    assert_rejected_without_artifact(
        args,
        kind="provider_bake",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--scheduled-lifecycle-canary-tick must match canonical lowercase "
            "`reserve-lifecycle-tick-*`"
        ),
    )


def test_provider_bake_tick_inventory_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider_bake", tmp_path)
    args[
        args.index("--scheduled-lifecycle-canary-tick") + 1
    ] = "reserve-lifecycle-tick-placeholder"

    assert_rejected_without_artifact(
        args,
        kind="provider_bake",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--scheduled-lifecycle-canary-tick must not contain "
            "non-production markers ['placeholder']"
        ),
    )


def test_provider_bake_defaulted_count_cannot_exceed_providers(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider_bake", tmp_path)
    args[args.index("--scheduled-lifecycle-canary-defaulted-provider-count") + 1] = "4"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--scheduled-lifecycle-canary-defaulted-provider-count must not exceed "
        "--provider inventory"
    ) in captured.err
    assert not canary_path(tmp_path, "provider_bake").exists()


@pytest.mark.parametrize(
    ("count_option", "replacement", "expected_error"),
    (
        (
            "--ledger-count",
            "2",
            "--ledger-count must match --ledger-ref inventory",
        ),
        (
            "--instruction-count",
            "3",
            "--instruction-count must match --instruction-ref inventory",
        ),
    ),
)
def test_ledger_digest_ref_inventory_must_match_count(
    count_option: str,
    replacement: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("ledger_digest", tmp_path)
    args[args.index(count_option) + 1] = replacement

    assert_rejected_without_artifact(
        args,
        kind="ledger_digest",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=expected_error,
    )


@pytest.mark.parametrize(
    ("option", "duplicate", "expected_error"),
    (
        (
            "--ledger-ref",
            "reserve-ledger-main",
            "--ledger-ref must not contain duplicates",
        ),
        (
            "--instruction-ref",
            "reserve-instruction-rent-settlement",
            "--instruction-ref must not contain duplicates",
        ),
    ),
)
def test_ledger_digest_ref_inventory_must_not_duplicate(
    option: str,
    duplicate: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("ledger_digest", tmp_path)
    args.extend([option, duplicate])

    assert_rejected_without_artifact(
        args,
        kind="ledger_digest",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=expected_error,
    )


@pytest.mark.parametrize(
    ("option", "replacement", "expected_error"),
    (
        (
            "--ledger-ref",
            "ledger-main",
            "--ledger-ref must match canonical lowercase `reserve-ledger-*`",
        ),
        (
            "--instruction-ref",
            "instruction-rent-settlement",
            "--instruction-ref must match canonical lowercase `reserve-instruction-*`",
        ),
    ),
)
def test_ledger_digest_ref_inventory_must_use_reviewed_labels_before_write(
    option: str,
    replacement: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("ledger_digest", tmp_path)
    args[args.index(option) + 1] = replacement

    assert_rejected_without_artifact(
        args,
        kind="ledger_digest",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=expected_error,
    )


@pytest.mark.parametrize(
    ("option", "replacement", "expected_error"),
    (
        (
            "--ledger-ref",
            "reserve-ledger-placeholder",
            "--ledger-ref must not contain non-production markers ['placeholder']",
        ),
        (
            "--instruction-ref",
            "reserve-instruction-placeholder",
            "--instruction-ref must not contain non-production markers ['placeholder']",
        ),
    ),
)
def test_ledger_digest_ref_inventory_rejects_non_production_markers_before_write(
    option: str,
    replacement: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("ledger_digest", tmp_path)
    args[args.index(option) + 1] = replacement

    assert_rejected_without_artifact(
        args,
        kind="ledger_digest",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=expected_error,
    )


def test_lifecycle_persisted_stage_inventory_must_match_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("lifecycle_service", tmp_path)
    args[args.index("--persisted-stage-count") + 1] = "5"

    assert_rejected_without_artifact(
        args,
        kind="lifecycle_service",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--persisted-stage-count must match --persisted-stage inventory"
        ),
    )


def test_lifecycle_persisted_stage_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("lifecycle_service", tmp_path)
    args.extend(["--persisted-stage", "reserve-lifecycle-stage-active"])

    assert_rejected_without_artifact(
        args,
        kind="lifecycle_service",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--persisted-stage must not contain duplicates",
    )


def test_lifecycle_persisted_stage_inventory_must_use_reviewed_labels_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("lifecycle_service", tmp_path)
    args[args.index("--persisted-stage") + 1] = "lifecycle-stage-active"

    assert_rejected_without_artifact(
        args,
        kind="lifecycle_service",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--persisted-stage must match canonical lowercase "
            "`reserve-lifecycle-stage-*`"
        ),
    )


def test_lifecycle_persisted_stage_inventory_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("lifecycle_service", tmp_path)
    args[args.index("--persisted-stage") + 1] = "reserve-lifecycle-stage-placeholder"

    assert_rejected_without_artifact(
        args,
        kind="lifecycle_service",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--persisted-stage must not contain "
            "non-production markers ['placeholder']"
        ),
    )


def test_governance_approval_consumer_inventory_must_match_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_approval", tmp_path)
    index = args.index("--downstream-compliance-consumer")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--downstream-compliance-consumer-count must match "
        "--downstream-compliance-consumer inventory"
    ) in captured.err
    assert not canary_path(tmp_path, "governance_approval").exists()


def test_governance_approval_consumer_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_approval", tmp_path)
    args.extend(
        ["--downstream-compliance-consumer", "reserve-compliance-consumer-gateway"]
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--downstream-compliance-consumer must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "governance_approval").exists()


def test_governance_approval_consumer_inventory_must_use_reviewed_labels_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_approval", tmp_path)
    args[args.index("--downstream-compliance-consumer") + 1] = "consumer-gateway"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--downstream-compliance-consumer must match canonical lowercase "
        "`reserve-compliance-consumer-*`"
    ) in captured.err
    assert not canary_path(tmp_path, "governance_approval").exists()


def test_governance_approval_consumer_inventory_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_approval", tmp_path)
    args[
        args.index("--downstream-compliance-consumer") + 1
    ] = "reserve-compliance-consumer-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--downstream-compliance-consumer must not contain "
        "non-production markers ['placeholder']"
    ) in captured.err
    assert not canary_path(tmp_path, "governance_approval").exists()


def test_missing_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("reserve_movement", tmp_path)
    index = args.index("--verified-claim")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--verified-claim must include every required value" in captured.err
    assert not canary_path(tmp_path, "reserve_movement").exists()


def test_missing_metric_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("metrics_alerts", tmp_path)
    index = args.index("--metric")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--metric must include every required value" in captured.err
    assert not canary_path(tmp_path, "metrics_alerts").exists()


def test_stale_scheduler_tick_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("provider_bake", tmp_path)
    args[
        args.index("--scheduled-lifecycle-canary-last-tick-unix") + 1
    ] = str(GENERATED_AT - CHECKER.DEFAULT_MAX_LIFECYCLE_LAG_SECS - 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "scheduled_lifecycle_canary_last_tick_at_unix must be within" in captured.err
    assert not canary_path(tmp_path, "provider_bake").exists()


def test_output_symlink_is_rejected(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    symlink = canary_path(tmp_path, "policy_config")
    symlink.symlink_to(target)

    assert MODULE.main(args_for("policy_config", tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a symlink" in captured.err
    assert not target.exists()


def test_output_directory_is_rejected(tmp_path: Path, capsys) -> None:
    output_dir = canary_path(tmp_path, "policy_config")
    output_dir.mkdir()

    assert MODULE.main(args_for("policy_config", tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a directory" in captured.err
    assert output_dir.is_dir()
