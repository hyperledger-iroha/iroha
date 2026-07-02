"""Tests for scripts/build_sorafs_gateway_compliance_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_gateway_compliance_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_gateway_compliance_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_gateway_compliance_canary",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_gateway_compliance_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


DIGEST = "a" * 64
TOGGLE_DIGEST = "b" * 64
POLICY_DIGEST = "c" * 64
GENERATED_AT = 1_800_100_000


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=GENERATED_AT,
        max_evidence_age_secs=CHECKER.DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=CHECKER.DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_reload_latency_ms=CHECKER.DEFAULT_MAX_RELOAD_LATENCY_MS,
        min_gateways=CHECKER.DEFAULT_MIN_GATEWAYS,
        min_denylist_entries=CHECKER.DEFAULT_MIN_DENYLIST_ENTRIES,
        min_honey_probes=CHECKER.DEFAULT_MIN_HONEY_PROBES,
    )


def feed_promotion() -> dict:
    return {
        "schema": "sorafs.gateway_compliance.feed_promotion_canary.v1",
        "status": "passed",
        "deployment_id": "sorafs-gateway-prod-20260701",
        "environment": "production",
        "deployment_context_reviewed": True,
        "generated_at_unix": GENERATED_AT,
        "external_feeds_normalized": True,
        "feed_signature_verified": True,
        "bundle_pack_verified": True,
        "bundle_diff_reviewed": True,
        "merkle_root_bound": True,
        "update_history_persisted": True,
        "gateway_ack_count": CHECKER.DEFAULT_MIN_GATEWAYS,
        "gateways": [
            {"name": f"gateway-{index}"}
            for index in range(CHECKER.DEFAULT_MIN_GATEWAYS)
        ],
        "denylist_entry_count": CHECKER.DEFAULT_MIN_DENYLIST_ENTRIES,
        "denylist_entries": [
            {"name": f"denylist-entry-{index}"}
            for index in range(CHECKER.DEFAULT_MIN_DENYLIST_ENTRIES)
        ],
        "bundle_digest_hex": DIGEST,
        "policy_digest_hex": POLICY_DIGEST,
        "raw_feeds_included": False,
        "feed_payloads_included": False,
    }


def controller_args(tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        "controller_runtime",
        "--out",
        str(tmp_path / "controller-runtime.json"),
        "--deployment-id",
        "sorafs-gateway-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(GENERATED_AT),
        "--bundle-digest-hex",
        DIGEST,
        "--controller-instance-id",
        "compliance-controller-prod-a",
        "--feed-count",
        "7",
    ]
    for name in (
        "ofac",
        "eu-sanctions",
        "malware",
        "csam-hash",
        "legal-hold",
        "regional-blocklist",
        "appeal-overrides",
    ):
        args.extend(["--feed", name])
    for value in MODULE.CONTROLLER_TRUE_CLAIMS:
        args.extend(["--verified-claim", value])
    return args


def moderation_args(tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        "moderation_toggle",
        "--out",
        str(tmp_path / "moderation-toggle.json"),
        "--deployment-id",
        "sorafs-gateway-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(GENERATED_AT),
        "--bundle-digest-hex",
        DIGEST,
        "--toggle-api-url",
        "https://gateway-compliance.internal/toggles",
        "--toggle-count",
        "4",
        "--toggle-digest-hex",
        TOGGLE_DIGEST,
    ]
    for name in (
        "provider-deny",
        "appeal-override",
        "legal-hold",
        "regional-emergency",
    ):
        args.extend(["--toggle", name])
    for value in MODULE.MODERATION_TRUE_CLAIMS:
        args.extend(["--verified-claim", value])
    return args


def test_builds_payload_free_controller_runtime_canary(tmp_path: Path) -> None:
    assert MODULE.main(controller_args(tmp_path)) == 0

    payload = json.loads((tmp_path / "controller-runtime.json").read_text("utf-8"))

    assert payload["schema"] == "sorafs.gateway_compliance.controller_runtime_canary.v1"
    assert payload["config_source"] == "iroha_config"
    assert payload["external_feed_count"] == 7
    assert payload["fetched_feed_count"] == 7
    assert payload["normalized_feed_count"] == 7
    assert payload["signed_feed_count"] == 7
    assert payload["feeds"] == [
        {"name": "ofac"},
        {"name": "eu-sanctions"},
        {"name": "malware"},
        {"name": "csam-hash"},
        {"name": "legal-hold"},
        {"name": "regional-blocklist"},
        {"name": "appeal-overrides"},
    ]
    for claim in MODULE.CONTROLLER_TRUE_CLAIMS:
        assert payload[claim] is True
    for claim in MODULE.FORBIDDEN_PAYLOAD_CLAIMS["controller_runtime"]:
        assert payload[claim] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "controller_runtime"
    assert errors == []


def test_builds_payload_free_moderation_toggle_canary(tmp_path: Path) -> None:
    assert MODULE.main(moderation_args(tmp_path)) == 0

    payload = json.loads((tmp_path / "moderation-toggle.json").read_text("utf-8"))

    assert payload["schema"] == "sorafs.gateway_compliance.moderation_toggle_canary.v1"
    assert payload["config_source"] == "iroha_config"
    assert payload["toggle_count"] == 4
    assert payload["approved_toggle_count"] == 4
    assert payload["toggles"] == [
        {"name": "provider-deny"},
        {"name": "appeal-override"},
        {"name": "legal-hold"},
        {"name": "regional-emergency"},
    ]
    assert payload["toggle_digest_hex"] == TOGGLE_DIGEST
    for claim in MODULE.MODERATION_TRUE_CLAIMS:
        assert payload[claim] is True
    for claim in MODULE.FORBIDDEN_PAYLOAD_CLAIMS["moderation_toggle"]:
        assert payload[claim] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "moderation_toggle"
    assert errors == []


def test_generated_canaries_pass_gateway_gate_with_feed_promotion_anchor(
    tmp_path: Path,
) -> None:
    assert MODULE.main(controller_args(tmp_path)) == 0
    assert MODULE.main(moderation_args(tmp_path)) == 0
    write_json(tmp_path / "feed-promotion.json", feed_promotion())
    summary = tmp_path / "summary.json"

    assert (
        CHECKER.main(
            [
                "--evidence",
                str(tmp_path / "feed-promotion.json"),
                "--evidence",
                str(tmp_path / "controller-runtime.json"),
                "--evidence",
                str(tmp_path / "moderation-toggle.json"),
                "--require-kind",
                "feed_promotion",
                "--require-kind",
                "controller_runtime",
                "--require-kind",
                "moderation_toggle",
                "--summary-out",
                str(summary),
                "--now-unix",
                str(GENERATED_AT),
            ]
        )
        == 0
    )

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["valid_bundle_digests"] == [DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    assert payload["required"]["controller_runtime"]["artifact_count"] == 1
    assert payload["required"]["controller_runtime"]["artifacts"][0]["valid"] is True
    assert payload["required"]["moderation_toggle"]["artifact_count"] == 1
    assert payload["required"]["moderation_toggle"]["artifacts"][0]["valid"] is True


def test_response_file_can_build_controller_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "controller.args"
    args_file.write_text("\n".join(controller_args(tmp_path)), encoding="utf-8")

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads((tmp_path / "controller-runtime.json").read_text("utf-8"))
    assert payload["controller_instance_id"] == "compliance-controller-prod-a"


def test_missing_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = controller_args(tmp_path)
    index = args.index("--verified-claim")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--verified-claim must include every required value" in captured.err
    assert not (tmp_path / "controller-runtime.json").exists()


def test_missing_controller_feed_inventory_fails_closed(tmp_path: Path, capsys) -> None:
    args = controller_args(tmp_path)
    while "--feed" in args:
        index = args.index("--feed")
        del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--feed is required for controller_runtime" in captured.err
    assert not (tmp_path / "controller-runtime.json").exists()


def test_duplicate_controller_feed_inventory_fails_closed(tmp_path: Path, capsys) -> None:
    args = controller_args(tmp_path)
    args[args.index("--feed-count") + 1] = "8"
    args.extend(["--feed", "ofac"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--feed must not contain duplicates" in captured.err
    assert "--feed-count must match the number of unique --feed values" in captured.err
    assert not (tmp_path / "controller-runtime.json").exists()


def test_missing_moderation_toggle_digest_fails_closed(tmp_path: Path, capsys) -> None:
    args = moderation_args(tmp_path)
    index = args.index("--toggle-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--toggle-digest-hex must be exact lowercase 32-byte hex" in captured.err
    assert not (tmp_path / "moderation-toggle.json").exists()


def test_missing_moderation_toggle_inventory_fails_closed(tmp_path: Path, capsys) -> None:
    args = moderation_args(tmp_path)
    while "--toggle" in args:
        index = args.index("--toggle")
        del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--toggle is required for moderation_toggle" in captured.err
    assert not (tmp_path / "moderation-toggle.json").exists()


def test_duplicate_moderation_toggle_inventory_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = moderation_args(tmp_path)
    args[args.index("--toggle-count") + 1] = "5"
    args.extend(["--toggle", "provider-deny"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--toggle must not contain duplicates" in captured.err
    assert "--toggle-count must match the number of unique --toggle values" in captured.err
    assert not (tmp_path / "moderation-toggle.json").exists()


def test_output_symlink_is_rejected(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    symlink = tmp_path / "controller-runtime.json"
    symlink.symlink_to(target)

    assert MODULE.main(controller_args(tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a symlink" in captured.err
    assert not target.exists()
