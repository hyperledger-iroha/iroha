"""Tests for scripts/build_sorafs_governance_dag_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_governance_dag_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_governance_dag_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_governance_dag_canary",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_governance_dag_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


PUBLIC_HEAD = "a" * 64
CHECKPOINT = "b" * 64
POLICY = "c" * 64
GENERATED_AT = 1_800_100_000


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=GENERATED_AT,
        max_evidence_age_secs=CHECKER.DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=CHECKER.DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_pin_lag_secs=CHECKER.DEFAULT_MAX_PIN_LAG_SECS,
        max_head_age_secs=CHECKER.DEFAULT_MAX_HEAD_AGE_SECS,
        min_blocks=CHECKER.DEFAULT_MIN_BLOCKS,
        min_payload_kinds=CHECKER.DEFAULT_MIN_PAYLOAD_KINDS,
    )


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "sorafs-governance-dag-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(GENERATED_AT),
    ]
    if kind in MODULE.PUBLIC_HEAD_KINDS:
        args.extend(["--public-head-cid-hex", PUBLIC_HEAD])
    if kind in MODULE.POLICY_DIGEST_KINDS:
        args.extend(["--policy-digest-hex", POLICY])
    for claim in MODULE.TRUE_CLAIMS[kind]:
        args.extend(["--verified-claim", claim])
    if kind == "ingest_service":
        args.extend(["--source-count", "3"])
        for payload_kind in MODULE.REQUIRED_PAYLOAD_KINDS:
            args.extend(["--payload-kind", payload_kind])
    elif kind == "publisher_service":
        args.extend(
            [
                "--pin-lag-seconds",
                "60",
                "--head-age-seconds",
                "120",
                "--block-count",
                "8",
            ]
        )
        for payload_kind in MODULE.REQUIRED_PAYLOAD_KINDS:
            args.extend(["--payload-kind", payload_kind])
    elif kind == "operator_recovery":
        args.extend(["--checkpoint-digest-hex", CHECKPOINT])
    elif kind == "dashboard_api":
        for route in MODULE.REQUIRED_DASHBOARD_ROUTES:
            args.extend(["--route", route])
    elif kind == "observability":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    elif kind == "ipfs_ipns_e2e":
        args.extend(["--block-count", "8"])
        for payload_kind in MODULE.REQUIRED_PAYLOAD_KINDS:
            args.extend(["--payload-kind", payload_kind])
    return args


def test_builds_payload_free_publisher_service_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("publisher_service", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "publisher_service").read_text("utf-8"))

    assert payload["schema"] == "sorafs.governance_dag.publisher_service_canary.v1"
    assert payload["public_head_cid_hex"] == PUBLIC_HEAD
    assert payload["policy_digest_hex"] == POLICY
    assert payload["payload_kind_count"] == len(MODULE.REQUIRED_PAYLOAD_KINDS)
    assert payload["block_count"] == 8
    for claim in MODULE.TRUE_CLAIMS["publisher_service"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["publisher_service"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "publisher_service"
    assert errors == []


def test_generated_canaries_pass_full_governance_dag_gate(tmp_path: Path) -> None:
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
    assert payload["valid_checkpoint_digests"] == [CHECKPOINT]
    assert payload["valid_public_head_cids"] == [PUBLIC_HEAD]
    assert payload["valid_policy_digests"] == [POLICY]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_response_file_can_build_dashboard_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "dashboard.args"
    args_file.write_text("\n".join(args_for("dashboard_api", tmp_path)), encoding="utf-8")

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "dashboard_api").read_text("utf-8"))
    assert [route["name"] for route in payload["routes"]] == list(
        MODULE.REQUIRED_DASHBOARD_ROUTES
    )


def test_missing_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("publisher_service", tmp_path)
    index = args.index("--verified-claim")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--verified-claim must include every required value" in captured.err
    assert not canary_path(tmp_path, "publisher_service").exists()


def test_publisher_policy_digest_is_required(tmp_path: Path, capsys) -> None:
    args = args_for("publisher_service", tmp_path)
    index = args.index("--policy-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--policy-digest-hex must be exact lowercase 32-byte hex" in captured.err
    assert not canary_path(tmp_path, "publisher_service").exists()


def test_missing_dashboard_route_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("dashboard_api", tmp_path)
    index = args.index("--route")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--route must include every required value" in captured.err
    assert not canary_path(tmp_path, "dashboard_api").exists()


def test_stale_public_head_age_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("publisher_service", tmp_path)
    args[args.index("--head-age-seconds") + 1] = str(
        CHECKER.DEFAULT_MAX_HEAD_AGE_SECS + 1
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--head-age-seconds must be <=" in captured.err
    assert not canary_path(tmp_path, "publisher_service").exists()


def test_output_symlink_is_rejected(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    symlink = canary_path(tmp_path, "publisher_service")
    symlink.symlink_to(target)

    assert MODULE.main(args_for("publisher_service", tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a symlink" in captured.err
    assert not target.exists()
