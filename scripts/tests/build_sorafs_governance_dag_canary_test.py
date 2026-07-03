"""Tests for scripts/build_sorafs_governance_dag_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


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


def block_refs(count: int) -> list[str]:
    return [f"governance-dag-block-{index:02d}" for index in range(count)]


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
        args.extend(["--source-count", str(len(MODULE.REQUIRED_PAYLOAD_KINDS))])
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
        for block_ref in block_refs(8):
            args.extend(["--block-ref", block_ref])
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
        for block_ref in block_refs(8):
            args.extend(["--block-ref", block_ref])
        for payload_kind in MODULE.REQUIRED_PAYLOAD_KINDS:
            args.extend(["--payload-kind", payload_kind])
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


def test_builds_payload_free_publisher_service_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("publisher_service", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "publisher_service").read_text("utf-8"))

    assert payload["schema"] == "sorafs.governance_dag.publisher_service_canary.v1"
    assert payload["public_head_cid_hex"] == PUBLIC_HEAD
    assert payload["policy_digest_hex"] == POLICY
    assert payload["payload_kind_count"] == len(MODULE.REQUIRED_PAYLOAD_KINDS)
    assert payload["payload_kinds"] == list(MODULE.REQUIRED_PAYLOAD_KINDS)
    assert payload["block_count"] == 8
    assert payload["block_refs"] == block_refs(8)
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


def test_ingest_source_count_must_match_payload_kinds_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("ingest_service", tmp_path)
    args[args.index("--source-count") + 1] = str(
        len(MODULE.REQUIRED_PAYLOAD_KINDS) + 1
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--source-count must match unique --payload-kind count" in captured.err
    assert not canary_path(tmp_path, "ingest_service").exists()


def test_publisher_block_ref_inventory_must_match_block_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("publisher_service", tmp_path)
    index = args.index("--block-ref")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--block-ref unique values must match --block-count" in captured.err
    assert not canary_path(tmp_path, "publisher_service").exists()


def test_publisher_block_ref_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("publisher_service", tmp_path)
    args.extend(["--block-ref", block_refs(8)[0]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--block-ref must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "publisher_service").exists()


def test_publisher_block_ref_inventory_must_use_production_family(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("publisher_service", tmp_path)
    args[args.index("--block-ref") + 1] = "governance-block-00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--block-ref must match canonical lowercase `governance-dag-block-name`"
        in captured.err
    )
    assert not canary_path(tmp_path, "publisher_service").exists()


def test_publisher_block_ref_inventory_rejects_placeholder_marker(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("publisher_service", tmp_path)
    args[args.index("--block-ref") + 1] = "governance-dag-block-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--block-ref must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "publisher_service").exists()


def test_ipfs_block_ref_inventory_must_match_block_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("ipfs_ipns_e2e", tmp_path)
    index = args.index("--block-ref")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--block-ref unique values must match --block-count" in captured.err
    assert not canary_path(tmp_path, "ipfs_ipns_e2e").exists()


def test_ipfs_block_ref_inventory_must_use_production_family(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("ipfs_ipns_e2e", tmp_path)
    args[args.index("--block-ref") + 1] = "governance-block-00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--block-ref must match canonical lowercase `governance-dag-block-name`"
        in captured.err
    )
    assert not canary_path(tmp_path, "ipfs_ipns_e2e").exists()


def test_missing_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("publisher_service", tmp_path)
    index = args.index("--verified-claim")
    del args[index : index + 2]

    assert_rejected_without_artifact(
        args,
        kind="publisher_service",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--verified-claim must include every required value",
    )


def test_unknown_verified_claim_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("publisher_service", tmp_path)
    args.extend(["--verified-claim", "unreviewed_publication_claim"])

    assert_rejected_without_artifact(
        args,
        kind="publisher_service",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--verified-claim contains an unknown value",
    )


def test_duplicate_verified_claim_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("publisher_service", tmp_path)
    first_claim = args.index("--verified-claim") + 1
    args.extend(["--verified-claim", args[first_claim]])

    assert_rejected_without_artifact(
        args,
        kind="publisher_service",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--verified-claim must not contain duplicates",
    )


def test_unknown_payload_kind_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("ingest_service", tmp_path)
    args.extend(["--payload-kind", "debug-raw-dag-block"])

    assert_rejected_without_artifact(
        args,
        kind="ingest_service",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--payload-kind contains an unknown value",
    )


def test_duplicate_payload_kind_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("ingest_service", tmp_path)
    first_payload_kind = args.index("--payload-kind") + 1
    args.extend(["--payload-kind", args[first_payload_kind]])

    assert_rejected_without_artifact(
        args,
        kind="ingest_service",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--payload-kind must not contain duplicates",
    )


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

    assert_rejected_without_artifact(
        args,
        kind="dashboard_api",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--route must include every required value",
    )


def test_unknown_dashboard_route_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("dashboard_api", tmp_path)
    args.extend(["--route", "debug_raw_head"])

    assert_rejected_without_artifact(
        args,
        kind="dashboard_api",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--route contains an unknown value",
    )


def test_duplicate_dashboard_route_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("dashboard_api", tmp_path)
    first_route = args.index("--route") + 1
    args.extend(["--route", args[first_route]])

    assert_rejected_without_artifact(
        args,
        kind="dashboard_api",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--route must not contain duplicates",
    )


def test_unknown_metric_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("observability", tmp_path)
    args.extend(["--metric", "sorafs_governance_dag_debug_payload_bytes"])

    assert_rejected_without_artifact(
        args,
        kind="observability",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--metric contains an unknown value",
    )


def test_duplicate_metric_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("observability", tmp_path)
    first_metric = args.index("--metric") + 1
    args.extend(["--metric", args[first_metric]])

    assert_rejected_without_artifact(
        args,
        kind="observability",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--metric must not contain duplicates",
    )


@pytest.mark.parametrize(
    ("kind", "option", "duplicate_value", "unknown_value"),
    (
        (
            "publisher_service",
            "--verified-claim",
            MODULE.TRUE_CLAIMS["publisher_service"][0],
            "unreviewed_publication_claim",
        ),
        (
            "ingest_service",
            "--payload-kind",
            MODULE.REQUIRED_PAYLOAD_KINDS[0],
            "debug-raw-dag-block",
        ),
        (
            "dashboard_api",
            "--route",
            MODULE.REQUIRED_DASHBOARD_ROUTES[0],
            "debug_raw_head",
        ),
        (
            "observability",
            "--metric",
            MODULE.REQUIRED_METRICS[0],
            "sorafs_governance_dag_debug_payload_bytes",
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


def test_output_directory_is_rejected(tmp_path: Path, capsys) -> None:
    output_dir = canary_path(tmp_path, "publisher_service")
    output_dir.mkdir()

    assert MODULE.main(args_for("publisher_service", tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a directory" in captured.err
    assert output_dir.is_dir()
