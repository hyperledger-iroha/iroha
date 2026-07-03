"""Tests for scripts/build_sorafs_orderbook_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_orderbook_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_orderbook_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_orderbook_canary",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_orderbook_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


CONTRACT_DIGEST = "a" * 64
POLICY_DIGEST = "b" * 64
ARTIFACT_DIGEST = "c" * 64
GENERATED_AT = 1_800_100_000


def order_refs(count: int) -> list[str]:
    return [f"order-{index:02d}" for index in range(count)]


def channel_refs(count: int) -> list[str]:
    return [f"channel-{index:02d}" for index in range(count)]


def receipt_refs(count: int) -> list[str]:
    return [f"receipt-{index:02d}" for index in range(count)]


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=GENERATED_AT,
        max_evidence_age_secs=CHECKER.DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=CHECKER.DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_stream_lag_ms=CHECKER.DEFAULT_MAX_STREAM_LAG_MS,
        max_matcher_lag_ms=CHECKER.DEFAULT_MAX_MATCHER_LAG_MS,
        min_reconciliation_peers=CHECKER.DEFAULT_MIN_RECONCILIATION_PEERS,
    )


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "sorafs-orderbook-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(GENERATED_AT),
    ]
    if kind in MODULE.CONTRACT_DIGEST_KINDS:
        args.extend(["--contract-digest-hex", CONTRACT_DIGEST])
    if kind in MODULE.POLICY_DIGEST_KINDS:
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    for claim in MODULE.TRUE_CLAIMS[kind]:
        args.extend(["--verified-claim", claim])
    if kind == "matcher_service":
        args.extend(
            [
                "--matcher-lag-ms",
                "100",
                "--accepted-order-count",
                "12",
                "--matched-order-count",
                "8",
                "--rejected-invalid-order-count",
                "2",
            ]
        )
        for order in order_refs(12):
            args.extend(["--accepted-order", order])
        for order in order_refs(8):
            args.extend(["--matched-order", order])
    elif kind == "settlement_service":
        args.extend(
            [
                "--open-channel-count",
                "5",
                "--settled-receipt-count",
                "9",
                "--settlement-backlog-count",
                "0",
            ]
        )
        for channel in channel_refs(5):
            args.extend(["--open-channel", channel])
        for receipt in receipt_refs(9):
            args.extend(["--settled-receipt", receipt])
    elif kind == "api_gateway":
        for route in MODULE.REQUIRED_API_ROUTES:
            args.extend(["--route", route])
    elif kind == "event_streams":
        for stream in MODULE.REQUIRED_STREAMS:
            args.extend(["--stream", stream])
    elif kind == "sdk_release":
        for language in MODULE.REQUIRED_SDK_LANGUAGES:
            args.extend(["--language", language])
            args.extend(["--artifact", f"{language}-orderbook:{ARTIFACT_DIGEST}"])
    elif kind == "observability":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    elif kind == "reconciliation":
        args.extend(["--peer-count", str(CHECKER.DEFAULT_MIN_RECONCILIATION_PEERS)])
        for index in range(CHECKER.DEFAULT_MIN_RECONCILIATION_PEERS):
            args.extend(["--peer", f"peer-{index:02d}"])
        for source in MODULE.REQUIRED_RECONCILIATION_SOURCES:
            args.extend(["--source", source])
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


def test_builds_payload_free_api_gateway_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("api_gateway", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "api_gateway").read_text("utf-8"))

    assert payload["schema"] == "sorafs.orderbook.api_gateway_canary.v1"
    assert payload["contract_digest_hex"] == CONTRACT_DIGEST
    assert payload["route_count"] == len(MODULE.REQUIRED_API_ROUTES)
    assert payload["passed_route_count"] == len(MODULE.REQUIRED_API_ROUTES)
    assert [route["name"] for route in payload["routes"]] == list(
        MODULE.REQUIRED_API_ROUTES
    )
    for claim in MODULE.TRUE_CLAIMS["api_gateway"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["api_gateway"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "api_gateway"
    assert errors == []


def test_builds_payload_free_event_streams_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("event_streams", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "event_streams").read_text("utf-8"))

    assert payload["schema"] == "sorafs.orderbook.event_streams_canary.v1"
    assert payload["contract_digest_hex"] == CONTRACT_DIGEST
    assert payload["stream_count"] == len(MODULE.REQUIRED_STREAMS)
    assert [stream["name"] for stream in payload["streams"]] == list(
        MODULE.REQUIRED_STREAMS
    )
    for claim in MODULE.TRUE_CLAIMS["event_streams"]:
        assert all(stream[claim] is True for stream in payload["streams"])
    for field in MODULE.FORCED_FALSE_FIELDS["event_streams"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "event_streams"
    assert errors == []


def test_generated_canaries_pass_full_orderbook_gate(tmp_path: Path) -> None:
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
    assert payload["valid_contract_digests"] == [CONTRACT_DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True
    governance_payload = json.loads(
        canary_path(tmp_path, "governance_approval").read_text("utf-8")
    )
    assert governance_payload["contract_digest_hex"] == CONTRACT_DIGEST
    contract_payload = json.loads(
        canary_path(tmp_path, "contract_surface").read_text("utf-8")
    )
    assert contract_payload["policy_digest_hex"] == POLICY_DIGEST


def test_response_file_can_build_sdk_release_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "sdk.args"
    args_file.write_text("\n".join(args_for("sdk_release", tmp_path)), encoding="utf-8")

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "sdk_release").read_text("utf-8"))
    assert payload["language_count"] == len(MODULE.REQUIRED_SDK_LANGUAGES)
    assert [language["name"] for language in payload["languages"]] == list(
        MODULE.REQUIRED_SDK_LANGUAGES
    )
    assert payload["artifact_count"] == len(MODULE.REQUIRED_SDK_LANGUAGES)


def test_response_file_can_build_reconciliation_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "reconciliation.args"
    args_file.write_text(
        "\n".join(args_for("reconciliation", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "reconciliation").read_text("utf-8"))
    assert payload["peer_count"] == CHECKER.DEFAULT_MIN_RECONCILIATION_PEERS
    assert [peer["name"] for peer in payload["peers"]] == [
        f"peer-{index:02d}" for index in range(CHECKER.DEFAULT_MIN_RECONCILIATION_PEERS)
    ]
    assert [source["name"] for source in payload["sources"]] == list(
        MODULE.REQUIRED_RECONCILIATION_SOURCES
    )


def test_reconciliation_peer_inventory_must_match_peer_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("reconciliation", tmp_path)
    peer_count_index = args.index("--peer-count")
    args[peer_count_index + 1] = str(CHECKER.DEFAULT_MIN_RECONCILIATION_PEERS + 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--peer unique values must match --peer-count" in captured.err
    assert not canary_path(tmp_path, "reconciliation").exists()


def test_matcher_accepted_order_inventory_must_match_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("matcher_service", tmp_path)
    args[args.index("--accepted-order-count") + 1] = "13"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--accepted-order unique values must match --accepted-order-count" in captured.err
    assert not canary_path(tmp_path, "matcher_service").exists()


def test_matcher_accepted_order_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("matcher_service", tmp_path)
    first_order = args.index("--accepted-order") + 1
    args.extend(["--accepted-order", args[first_order]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--accepted-order must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "matcher_service").exists()


def test_matcher_matched_orders_must_be_accepted_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("matcher_service", tmp_path)
    matched_order_index = args.index("--matched-order") + 1
    args[matched_order_index] = "order-not-accepted"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--matched-order values must also be present in --accepted-order" in captured.err
    assert not canary_path(tmp_path, "matcher_service").exists()


def test_matcher_order_inventory_must_use_reviewed_labels_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("matcher_service", tmp_path)
    accepted_order_index = args.index("--accepted-order") + 1
    args[accepted_order_index] = "order_alpha"

    assert_rejected_without_artifact(
        args,
        kind="matcher_service",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--accepted-order must match canonical lowercase `order-name`",
    )


def test_matcher_order_inventory_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("matcher_service", tmp_path)
    accepted_order_index = args.index("--accepted-order") + 1
    args[accepted_order_index] = "order-placeholder"

    assert_rejected_without_artifact(
        args,
        kind="matcher_service",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--accepted-order must not contain non-production markers ['placeholder']"
        ),
    )


def test_settlement_open_channel_inventory_must_match_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("settlement_service", tmp_path)
    args[args.index("--open-channel-count") + 1] = "6"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--open-channel unique values must match --open-channel-count" in captured.err
    assert not canary_path(tmp_path, "settlement_service").exists()


def test_settlement_settled_receipt_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("settlement_service", tmp_path)
    first_receipt = args.index("--settled-receipt") + 1
    args.extend(["--settled-receipt", args[first_receipt]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--settled-receipt must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "settlement_service").exists()


def test_settlement_inventory_must_use_reviewed_labels_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("settlement_service", tmp_path)
    channel_index = args.index("--open-channel") + 1
    receipt_index = args.index("--settled-receipt") + 1
    args[channel_index] = "channel_alpha"
    args[receipt_index] = "receipt_alpha"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--open-channel must match canonical lowercase `channel-name`" in captured.err
    assert "--settled-receipt must match canonical lowercase `receipt-name`" in captured.err
    assert not canary_path(tmp_path, "settlement_service").exists()


def test_settlement_inventory_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("settlement_service", tmp_path)
    channel_index = args.index("--open-channel") + 1
    receipt_index = args.index("--settled-receipt") + 1
    args[channel_index] = "channel-placeholder"
    args[receipt_index] = "receipt-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--open-channel must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert (
        "--settled-receipt must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "settlement_service").exists()


def test_reconciliation_peer_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("reconciliation", tmp_path)
    first_peer = args.index("--peer") + 1
    args.extend(["--peer", args[first_peer]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--peer must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "reconciliation").exists()


def test_reconciliation_peer_inventory_must_use_reviewed_labels_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("reconciliation", tmp_path)
    peer_index = args.index("--peer") + 1
    args[peer_index] = "peer_alpha"

    assert_rejected_without_artifact(
        args,
        kind="reconciliation",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--peer must match canonical lowercase `peer-name`",
    )


def test_reconciliation_peer_inventory_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("reconciliation", tmp_path)
    peer_index = args.index("--peer") + 1
    args[peer_index] = "peer-placeholder"

    assert_rejected_without_artifact(
        args,
        kind="reconciliation",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--peer must not contain non-production markers ['placeholder']",
    )


def test_duplicate_sdk_artifact_id_fails_closed_without_leaking(
    tmp_path: Path, capsys
) -> None:
    args = args_for("sdk_release", tmp_path)
    artifact_id = "java-orderbook-private-key-placeholder"
    first_artifact = args.index("--artifact") + 1
    args[first_artifact] = f"{artifact_id}:{ARTIFACT_DIGEST}"
    args.extend(["--artifact", f"{artifact_id}:{'d' * 64}"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "duplicate --artifact id" in captured.err
    assert artifact_id not in captured.err
    assert not canary_path(tmp_path, "sdk_release").exists()


def test_missing_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("contract_surface", tmp_path)
    index = args.index("--verified-claim")
    del args[index : index + 2]

    assert_rejected_without_artifact(
        args,
        kind="contract_surface",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--verified-claim must include every required value",
    )


def test_unknown_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("contract_surface", tmp_path)
    args.extend(["--verified-claim", "unreviewed_claim"])

    assert_rejected_without_artifact(
        args,
        kind="contract_surface",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--verified-claim contains an unknown value",
    )


def test_duplicate_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("contract_surface", tmp_path)
    first_claim = args.index("--verified-claim") + 1
    args.extend(["--verified-claim", args[first_claim]])

    assert_rejected_without_artifact(
        args,
        kind="contract_surface",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--verified-claim must not contain duplicates",
    )


def test_missing_api_route_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("api_gateway", tmp_path)
    index = args.index("--route")
    del args[index : index + 2]

    assert_rejected_without_artifact(
        args,
        kind="api_gateway",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--route must include every required value",
    )


def test_unknown_api_route_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("api_gateway", tmp_path)
    args.extend(["--route", "debug_contract_dump"])

    assert_rejected_without_artifact(
        args,
        kind="api_gateway",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--route contains an unknown value",
    )


def test_duplicate_api_route_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("api_gateway", tmp_path)
    first_route = args.index("--route") + 1
    args.extend(["--route", args[first_route]])

    assert_rejected_without_artifact(
        args,
        kind="api_gateway",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--route must not contain duplicates",
    )


def test_unknown_event_stream_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("event_streams", tmp_path)
    args.extend(["--stream", "debug_depth_stream"])

    assert_rejected_without_artifact(
        args,
        kind="event_streams",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--stream contains an unknown value",
    )


def test_duplicate_event_stream_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("event_streams", tmp_path)
    first_stream = args.index("--stream") + 1
    args.extend(["--stream", args[first_stream]])

    assert_rejected_without_artifact(
        args,
        kind="event_streams",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--stream must not contain duplicates",
    )


def test_unknown_sdk_language_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("sdk_release", tmp_path)
    args.extend(["--language", "debug-shell"])

    assert_rejected_without_artifact(
        args,
        kind="sdk_release",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--language contains an unknown value",
    )


def test_duplicate_sdk_language_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("sdk_release", tmp_path)
    first_language = args.index("--language") + 1
    args.extend(["--language", args[first_language]])

    assert_rejected_without_artifact(
        args,
        kind="sdk_release",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--language must not contain duplicates",
    )


def test_observability_metrics_must_not_duplicate(tmp_path: Path, capsys) -> None:
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


def test_unknown_observability_metric_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("observability", tmp_path)
    args.extend(["--metric", "torii_sorafs_orderbook_debug_payload_bytes"])

    assert_rejected_without_artifact(
        args,
        kind="observability",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--metric contains an unknown value",
    )


def test_unknown_reconciliation_source_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("reconciliation", tmp_path)
    args.extend(["--source", "manual-spreadsheet"])

    assert_rejected_without_artifact(
        args,
        kind="reconciliation",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--source contains an unknown value",
    )


def test_duplicate_reconciliation_source_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("reconciliation", tmp_path)
    first_source = args.index("--source") + 1
    args.extend(["--source", args[first_source]])

    assert_rejected_without_artifact(
        args,
        kind="reconciliation",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--source must not contain duplicates",
    )


@pytest.mark.parametrize(
    ("kind", "option", "duplicate_value", "unknown_value"),
    (
        (
            "contract_surface",
            "--verified-claim",
            MODULE.TRUE_CLAIMS["contract_surface"][0],
            "unreviewed_claim",
        ),
        (
            "api_gateway",
            "--route",
            MODULE.REQUIRED_API_ROUTES[0],
            "debug_contract_dump",
        ),
        (
            "event_streams",
            "--stream",
            MODULE.REQUIRED_STREAMS[0],
            "debug_depth_stream",
        ),
        (
            "sdk_release",
            "--language",
            MODULE.REQUIRED_SDK_LANGUAGES[0],
            "debug-shell",
        ),
        (
            "observability",
            "--metric",
            MODULE.REQUIRED_METRICS[0],
            "torii_sorafs_orderbook_debug_payload_bytes",
        ),
        (
            "reconciliation",
            "--source",
            MODULE.REQUIRED_RECONCILIATION_SOURCES[0],
            "manual-spreadsheet",
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


def test_governance_approval_requires_contract_digest(tmp_path: Path, capsys) -> None:
    args = args_for("governance_approval", tmp_path)
    index = args.index("--contract-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--contract-digest-hex must be exact lowercase 32-byte hex" in captured.err
    assert not canary_path(tmp_path, "governance_approval").exists()


def test_stale_matcher_lag_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("matcher_service", tmp_path)
    args[args.index("--matcher-lag-ms") + 1] = str(
        CHECKER.DEFAULT_MAX_MATCHER_LAG_MS + 1
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--matcher-lag-ms must be <=" in captured.err
    assert not canary_path(tmp_path, "matcher_service").exists()


def test_output_symlink_is_rejected(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    symlink = canary_path(tmp_path, "contract_surface")
    symlink.symlink_to(target)

    assert MODULE.main(args_for("contract_surface", tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a symlink" in captured.err
    assert not target.exists()


def test_output_directory_is_rejected(tmp_path: Path, capsys) -> None:
    output_dir = canary_path(tmp_path, "contract_surface")
    output_dir.mkdir()

    assert MODULE.main(args_for("contract_surface", tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a directory" in captured.err
    assert output_dir.is_dir()
