"""Tests for scripts/build_sorafs_potr_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_potr_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_potr_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location("build_sorafs_potr_canary", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_potr_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


NOW_UNIX = 1_800_600_000
GENERATED_AT = NOW_UNIX - 120
DIGEST = "e" * 64
POLICY_DIGEST = "f" * 64
STATS_DIGEST = "c" * 64
VALIDATION_DIGEST = "d" * 64
PQ_KEY_ROSTER_DIGEST = "b" * 64
REPUTATION_POLICY_DIGEST = "a" * 64


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "potr-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(NOW_UNIX),
    ]
    if kind in MODULE.RECEIPT_SUMMARY_DIGEST_KINDS:
        args.extend(["--receipt-summary-digest-hex", DIGEST])
    if kind == "multi_provider_probe":
        for tier in MODULE.REQUIRED_TIERS:
            args.extend(["--tier", tier])
        for index in range(CHECKER.DEFAULT_MIN_PROVIDERS):
            args.extend(["--provider", f"provider-{index:02d}"])
        for index in range(CHECKER.DEFAULT_MIN_RECEIPTS):
            args.extend(["--receipt", f"receipt-{index:02d}"])
    elif kind == "receipt_validation":
        args.extend(["--validation-bundle-digest-hex", VALIDATION_DIGEST])
        args.extend(["--pq-key-roster-digest-hex", PQ_KEY_ROSTER_DIGEST])
    elif kind == "proof_stream":
        for route in MODULE.REQUIRED_ROUTES:
            args.extend(["--route", route])
    elif kind == "reputation_integration":
        args.extend(["--stats-digest-hex", STATS_DIGEST])
        args.extend(
            [
                "--reputation-weight-policy-digest-hex",
                REPUTATION_POLICY_DIGEST,
            ]
        )
    elif kind == "observability":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    elif kind == "governance_approval":
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
        args.extend(["--pq-key-roster-digest-hex", PQ_KEY_ROSTER_DIGEST])
        args.extend(
            [
                "--reputation-weight-policy-digest-hex",
                REPUTATION_POLICY_DIGEST,
            ]
        )
    return args


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=NOW_UNIX,
        max_evidence_age_secs=CHECKER.DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=CHECKER.DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_hot_latency_ms=CHECKER.DEFAULT_MAX_HOT_LATENCY_MS,
        max_warm_latency_ms=CHECKER.DEFAULT_MAX_WARM_LATENCY_MS,
        min_providers=CHECKER.DEFAULT_MIN_PROVIDERS,
        min_receipts=CHECKER.DEFAULT_MIN_RECEIPTS,
    )


def test_builds_payload_free_proof_stream_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("proof_stream", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "proof_stream").read_text("utf-8"))

    assert payload["schema"] == "sorafs.potr.proof_stream_canary.v1"
    assert payload["route_count"] == len(MODULE.REQUIRED_ROUTES)
    assert payload["passed_route_count"] == len(MODULE.REQUIRED_ROUTES)
    assert payload["response_bodies_included"] is False
    assert all(route["norito_verified"] is True for route in payload["routes"])
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "proof_stream"
    assert errors == []


def test_generated_canaries_pass_full_potr_gate(tmp_path: Path) -> None:
    evidence_paths: list[Path] = []
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        evidence_paths.append(canary_path(tmp_path, kind))
    summary = tmp_path / "summary.json"

    command = ["--now-unix", str(NOW_UNIX)]
    for path in evidence_paths:
        command.extend(["--evidence", str(path)])
    command.extend(["--summary-out", str(summary)])

    assert CHECKER.main(command) == 0

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["valid_receipt_summary_digests"] == [DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    assert payload["valid_pq_key_roster_digests"] == [PQ_KEY_ROSTER_DIGEST]
    assert payload["valid_reputation_weight_policy_digests"] == [
        REPUTATION_POLICY_DIGEST
    ]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_response_file_can_build_multi_provider_probe_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "multi-provider-probe.args"
    args_file.write_text(
        "\n".join(args_for("multi_provider_probe", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(
        canary_path(tmp_path, "multi_provider_probe").read_text("utf-8")
    )
    assert payload["tiers_observed"] == list(MODULE.REQUIRED_TIERS)
    assert payload["provider_count"] == CHECKER.DEFAULT_MIN_PROVIDERS
    assert payload["providers"] == [
        {"name": f"provider-{index:02d}"}
        for index in range(CHECKER.DEFAULT_MIN_PROVIDERS)
    ]
    assert payload["receipt_count"] == CHECKER.DEFAULT_MIN_RECEIPTS
    assert payload["receipts"] == [
        {"name": f"receipt-{index:02d}"}
        for index in range(CHECKER.DEFAULT_MIN_RECEIPTS)
    ]
    assert payload["raw_receipts_included"] is False
    assert payload["fetch_transcripts_included"] is False


def test_probe_provider_inventory_must_match_provider_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("multi_provider_probe", tmp_path)
    index = args.index("--provider")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--provider unique values must match --provider-count" in captured.err
    assert not canary_path(tmp_path, "multi_provider_probe").exists()


def test_probe_receipt_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("multi_provider_probe", tmp_path)
    first_receipt_index = args.index("--receipt")
    args[first_receipt_index + 1] = "receipt-01"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--receipt must not contain duplicates" in captured.err
    assert "--receipt unique values must match --receipt-count" in captured.err
    assert not canary_path(tmp_path, "multi_provider_probe").exists()


def test_missing_proof_stream_route_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("proof_stream", tmp_path)
    index = args.index("--route")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--route must include every required value" in captured.err
    assert not canary_path(tmp_path, "proof_stream").exists()


def test_probe_latency_thresholds_fail_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("multi_provider_probe", tmp_path)
    args.extend(["--hot-latency-ms", "90001", "--warm-latency-ms", "300001"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--hot-latency-ms must be <=" in captured.err
    assert "--warm-latency-ms must be <=" in captured.err
    assert not canary_path(tmp_path, "multi_provider_probe").exists()


def test_output_symlink_is_refused(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    link = tmp_path / "link.json"
    link.symlink_to(target)
    args = args_for("multi_provider_probe", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(link)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a symlink" in captured.err
    assert not target.exists()
