"""Tests for scripts/build_sorafs_hedging_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_hedging_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_hedging_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_hedging_canary",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_hedging_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


DECISION_DIGEST = "a" * 64
LINE_ROOT_DIGEST = "b" * 64
STATEMENT_BUNDLE_DIGEST = "c" * 64
RECONCILIATION_DIGEST = "d" * 64
POLICY_DIGEST = "e" * 64
STATEMENT_DIGEST_A = "1" * 64
STATEMENT_DIGEST_B = "2" * 64
ARTIFACT_DIGEST = "f" * 64
GENERATED_AT = 1_800_100_000


def canary_path(tmp_path: Path, kind: str, suffix: str = "") -> Path:
    stem = kind if not suffix else f"{kind}-{suffix}"
    return tmp_path / f"{stem}.json"


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=GENERATED_AT,
        max_feed_lag_secs=CHECKER.DEFAULT_MAX_FEED_LAG_SECS,
        max_cycle_age_secs=CHECKER.DEFAULT_MAX_CYCLE_AGE_SECS,
        max_divergence_bps=CHECKER.DEFAULT_MAX_DIVERGENCE_BPS,
        min_billing_cycles=CHECKER.DEFAULT_MIN_BILLING_CYCLES,
    )


def args_for(kind: str, tmp_path: Path, suffix: str = "") -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind, suffix)),
        "--deployment-id",
        "sorafs-hedging-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(GENERATED_AT),
    ]
    for claim in MODULE.TRUE_CLAIMS[kind]:
        args.extend(["--verified-claim", claim])
    if kind in MODULE.CYCLE_BINDING_KINDS:
        args.extend(
            [
                "--statement-bundle-digest-hex",
                STATEMENT_BUNDLE_DIGEST,
                "--reconciliation-digest-hex",
                RECONCILIATION_DIGEST,
            ]
        )
    if kind in MODULE.POLICY_DIGEST_KINDS:
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    if kind == "feed_collector":
        args.extend(["--feed-count", "3", "--feed-lag-seconds", "60"])
        for feed in ("feed-primary", "feed-secondary", "feed-tertiary"):
            args.extend(["--feed", feed])
    elif kind == "reference_price":
        args.extend(
            [
                "--decision-id-hex",
                DECISION_DIGEST,
                "--reference-price-micro-usd",
                "4200000",
                "--feed-count",
                "3",
                "--divergence-bps",
                "50",
                "--decision-lag-seconds",
                "60",
            ]
        )
        for feed in ("feed-primary", "feed-secondary", "feed-tertiary"):
            args.extend(["--feed", feed])
    elif kind == "billing_cycle":
        cycle_index = 2 if suffix == "b" else 1
        args.extend(
            [
                "--cycle-id",
                f"cycle-{cycle_index}",
                "--cycle-index",
                str(cycle_index),
                "--line-item-count",
                "5",
                "--total-micro-xor",
                "10000",
                "--total-usd-micro",
                "42000",
                "--reference-decision-id-hex",
                DECISION_DIGEST,
                "--line-item-root-hex",
                LINE_ROOT_DIGEST,
                "--statement-digest-hex",
                STATEMENT_DIGEST_A,
                "--statement-digest-hex",
                STATEMENT_DIGEST_B,
                "--statement",
                "statement-00",
                "--statement",
                "statement-01",
            ]
        )
        for index in range(5):
            args.extend(["--line-item", f"line-{index:02d}"])
    elif kind == "statement_publication":
        args.extend(["--acknowledgement-probe-count", "1"])
        for route in MODULE.REQUIRED_PUBLICATION_ROUTES:
            args.extend(["--route", route])
    elif kind == "reconciliation":
        args.extend(["--line-item-count", "5"])
        for index in range(5):
            args.extend(["--line-item", f"line-{index:02d}"])
        for source in MODULE.REQUIRED_RECONCILIATION_SOURCES:
            args.extend(["--source", source])
    elif kind == "metrics_alerts":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    elif kind == "native_bridge_release":
        args.extend(["--bridge-abi-version", "12"])
        args.extend(["--artifact", f"NoritoBridge.xcframework:{ARTIFACT_DIGEST}"])
        args.extend(["--artifact", f"connect-norito-jni-macos-arm64:{ARTIFACT_DIGEST}"])
    elif kind == "governance_approval":
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    return args


def test_builds_payload_free_billing_cycle_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("billing_cycle", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "billing_cycle").read_text("utf-8"))

    assert payload["schema"] == "sorafs.billing.cycle_canary.v1"
    assert payload["statement_bundle_digest_hex"] == STATEMENT_BUNDLE_DIGEST
    assert payload["reconciliation_digest_hex"] == RECONCILIATION_DIGEST
    assert payload["reference_decision_id_hex"] == DECISION_DIGEST
    assert payload["policy_digest_hex"] == POLICY_DIGEST
    assert payload["statement_count"] == 2
    assert [statement["name"] for statement in payload["statements"]] == [
        "statement-00",
        "statement-01",
    ]
    assert [line_item["name"] for line_item in payload["line_items"]] == [
        "line-00",
        "line-01",
        "line-02",
        "line-03",
        "line-04",
    ]
    for claim in MODULE.TRUE_CLAIMS["billing_cycle"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["billing_cycle"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "billing_cycle"
    assert errors == []


def test_generated_canaries_pass_full_hedging_gate(tmp_path: Path) -> None:
    evidence_paths: list[Path] = []
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        evidence_paths.append(canary_path(tmp_path, kind))
        if kind == "billing_cycle":
            assert MODULE.main(args_for(kind, tmp_path, "b")) == 0
            evidence_paths.append(canary_path(tmp_path, kind, "b"))
    summary = tmp_path / "summary.json"

    command = []
    for path in evidence_paths:
        command.extend(["--evidence", str(path)])
    command.extend(["--summary-out", str(summary), "--now-unix", str(GENERATED_AT)])

    assert CHECKER.main(command) == 0

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["valid_reference_decision_ids"] == [DECISION_DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    assert len(payload["valid_billing_cycles"]) == CHECKER.DEFAULT_MIN_BILLING_CYCLES
    assert payload["valid_cycle_bindings"] == [
        {
            "statement_bundle_digest_hex": STATEMENT_BUNDLE_DIGEST,
            "reconciliation_digest_hex": RECONCILIATION_DIGEST,
        }
    ]


def test_response_file_can_build_reference_price_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "reference.args"
    args_file.write_text("\n".join(args_for("reference_price", tmp_path)), encoding="utf-8")

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "reference_price").read_text("utf-8"))
    assert payload["decision_id_hex"] == DECISION_DIGEST
    assert payload["reference_price_micro_usd"] == 4_200_000
    assert [feed["name"] for feed in payload["feeds"]] == [
        "feed-primary",
        "feed-secondary",
        "feed-tertiary",
    ]


def test_duplicate_native_bridge_artifact_id_fails_closed_without_leaking(
    tmp_path: Path, capsys
) -> None:
    args = args_for("native_bridge_release", tmp_path)
    artifact_id = "NoritoBridge-private-key-placeholder"
    first_artifact = args.index("--artifact") + 1
    args[first_artifact] = f"{artifact_id}:{ARTIFACT_DIGEST}"
    args.extend(["--artifact", f"{artifact_id}:{'0' * 64}"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "duplicate --artifact id" in captured.err
    assert artifact_id not in captured.err
    assert not canary_path(tmp_path, "native_bridge_release").exists()


def test_billing_cycle_statement_inventory_must_match_statement_digests(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("billing_cycle", tmp_path)
    args.extend(["--statement", "statement-02"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--statement unique values must match --statement-digest-hex" in captured.err
    assert not canary_path(tmp_path, "billing_cycle").exists()


def test_billing_cycle_statement_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("billing_cycle", tmp_path)
    first_statement = args.index("--statement") + 1
    args.extend(["--statement", args[first_statement]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--statement must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "billing_cycle").exists()


def test_billing_cycle_line_item_inventory_must_match_line_item_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("billing_cycle", tmp_path)
    args[args.index("--line-item-count") + 1] = "6"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--line-item unique values must match --line-item-count" in captured.err
    assert not canary_path(tmp_path, "billing_cycle").exists()


def test_billing_cycle_line_item_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("billing_cycle", tmp_path)
    first_line_item = args.index("--line-item") + 1
    args.extend(["--line-item", args[first_line_item]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--line-item must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "billing_cycle").exists()


def test_reconciliation_line_item_inventory_must_match_line_item_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("reconciliation", tmp_path)
    args[args.index("--line-item-count") + 1] = "6"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--line-item unique values must match --line-item-count" in captured.err
    assert not canary_path(tmp_path, "reconciliation").exists()


def test_reconciliation_line_item_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("reconciliation", tmp_path)
    first_line_item = args.index("--line-item") + 1
    args.extend(["--line-item", args[first_line_item]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--line-item must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "reconciliation").exists()


def test_reference_price_feed_inventory_must_match_feed_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("reference_price", tmp_path)
    args[args.index("--feed-count") + 1] = "4"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--feed unique values must match --feed-count" in captured.err
    assert not canary_path(tmp_path, "reference_price").exists()


def test_reference_price_feed_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("reference_price", tmp_path)
    first_feed = args.index("--feed") + 1
    args.extend(["--feed", args[first_feed]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--feed must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "reference_price").exists()


def test_missing_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("reference_price", tmp_path)
    index = args.index("--verified-claim")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--verified-claim must include every required value" in captured.err
    assert not canary_path(tmp_path, "reference_price").exists()


def test_missing_publication_route_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("statement_publication", tmp_path)
    index = args.index("--route")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--route must include every required value" in captured.err
    assert not canary_path(tmp_path, "statement_publication").exists()


def test_excessive_divergence_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("reference_price", tmp_path)
    args[args.index("--divergence-bps") + 1] = str(
        CHECKER.DEFAULT_MAX_DIVERGENCE_BPS + 1
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--divergence-bps must be <=" in captured.err
    assert not canary_path(tmp_path, "reference_price").exists()


def test_hedge_execution_enabled_requires_governance_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("governance_approval", tmp_path)
    args.append("--hedge-execution-enabled")

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--hedge-execution-governed is required" in captured.err
    assert not canary_path(tmp_path, "governance_approval").exists()


def test_output_symlink_is_rejected(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    symlink = canary_path(tmp_path, "feed_collector")
    symlink.symlink_to(target)

    assert MODULE.main(args_for("feed_collector", tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a symlink" in captured.err
    assert not target.exists()
