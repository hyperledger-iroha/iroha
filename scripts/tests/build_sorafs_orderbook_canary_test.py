"""Tests for scripts/build_sorafs_orderbook_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


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

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--verified-claim must include every required value" in captured.err
    assert not canary_path(tmp_path, "contract_surface").exists()


def test_missing_api_route_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("api_gateway", tmp_path)
    index = args.index("--route")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--route must include every required value" in captured.err
    assert not canary_path(tmp_path, "api_gateway").exists()


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
