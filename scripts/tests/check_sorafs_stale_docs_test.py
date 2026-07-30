"""Focused guards for canonical SoraFS V1 documentation claims."""

from __future__ import annotations

import re
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]


def read(relative: str) -> str:
    """Read one checked-in UTF-8 source file."""

    return (REPO_ROOT / relative).read_text(encoding="utf-8")


def test_hedging_plan_uses_the_current_bridge_abi() -> None:
    plan = read("specs/sorafs_hedging_plan.md")
    bridge = read("crates/connect_norito_bridge/src/lib.rs")
    header = read("crates/connect_norito_bridge/include/connect_norito_bridge.h")
    privacy = read("crates/iroha_data_model/src/privacy/protocol.rs")

    source_match = re.search(
        r"CONNECT_NORITO_BRIDGE_ABI_VERSION:\s*u32\s*=\s*"
        r"PRIVACY_BRIDGE_ABI_VERSION_V1",
        bridge,
    )
    canonical_match = re.search(
        r"PRIVACY_BRIDGE_ABI_VERSION_V1:\s*u32\s*=\s*(\d+)",
        privacy,
    )
    header_match = re.search(
        r"#define\s+CONNECT_NORITO_BRIDGE_ABI_VERSION\s+(\d+)",
        header,
    )
    assert source_match is not None
    assert canonical_match is not None
    assert header_match is not None
    assert canonical_match.group(1) == header_match.group(1) == "21"
    assert "bridge source ABI is now 12" not in plan
    assert "sole first-release ABI, version 21" in plan


def test_reference_sdk_plan_does_not_reopen_native_orderbook_work() -> None:
    plan = read("specs/sorafs_reference_sdk_plan.md")
    normalized = " ".join(plan.split())

    assert (
        "runtime matcher service wiring, durable escrow mutation, and signature "
        "authorization remain"
    ) not in normalized
    assert "release-wide signed fixture inventory" not in normalized
    assert "The published and sealed cross-domain fixture inventory" not in normalized
    for marker in (
        "authoritative native ledger and supervised worker now own bounded "
        "price-time matching",
        "atomic custody mutation",
        "authority/signature enforcement",
        "The checked-in, test-only signed and sealed cross-domain fixture "
        "inventory binds 82 payload artifacts",
        "82 payload artifacts",
        "32 `ValidationOutcomeV1` outcomes",
        "38 negative payload vectors",
        "twelve exact parity profiles",
        "All eight generated `CancelAssetLock` positive/negative files are "
        "checked in",
        "The 85-byte canonical frame is byte-exact",
        "redundant 86-byte `0x21 0x20 <hash>` representation",
        "JavaScript/TypeScript, Python, Swift, Kotlin/JVM, mirrored Java "
        "Android, and C#",
        "published per-target archives and binding packages",
        "genuine downstream install/smoke evidence",
    ):
        assert marker in normalized


def test_gateway_tls_docs_require_withdrawal_and_runtime_adapter_recovery() -> None:
    handbook = read("specs/sorafs_gateway_deployment_handbook.md")
    automation = read("specs/sorafs_gateway_tls_automation.md")
    combined = f"{handbook}\n{automation}"

    assert "sorafs-gateway tls renew" not in combined
    assert "fall back to stored cert" not in combined
    assert "Withdraw the affected gateway" in handbook
    assert "audited runtime ACME adapter and controller boundary" in handbook
    assert "There is no production repository renewal" in automation
    assert (
        "Repository tooling neither issues nor installs production certificates"
        in automation
    )


def test_stream_token_docs_use_the_runtime_signer_hard_cut() -> None:
    protocol = read("specs/sorafs_node_client_protocol.md")
    chunk_range = read("specs/sorafs_gateway_chunk_range.md")
    handbook = read("specs/sorafs_gateway_deployment_handbook.md")
    playbook = read("specs/sorafs_gateway_operator_playbook.md")
    roadmap = read("roadmap.md")
    status = read("status.md")
    active = "\n".join((protocol, chunk_range, handbook, playbook, roadmap))
    normalized_active = " ".join(active.split())
    normalized_status = " ".join(status.split())

    for stale in (
        "SORAFS_STREAM_TOKENS_ENABLED",
        "token_signing_sk",
        "signing_key_path",
        "when the signing key is not configured",
        "sign_with_seed",
    ):
        assert stale not in active
    for marker in (
        "only when issuance is disabled in node TOML",
        "No signing-seed file, key path, or environment enablement is accepted",
        "There is no environment-variable enablement or signing-seed path",
        "The former file-seed loader, environment enablement, standard-launcher "
        "node-key derivation, and internal seed-signing API are deleted",
    ):
        assert marker in normalized_active
    for marker in (
        "Historical record: the Torii stream-token key-file parser",
        "removed by the V1 hard cut",
        "it has no file-key or environment fallback",
    ):
        assert marker in normalized_status


def test_roadmap_does_not_preserve_retired_gateway_policy_tooling() -> None:
    roadmap = read("roadmap.md")

    for stale in (
        "SoraFS gateway denylist",
        "gateway-denylist",
        "local denylist",
        "denylist_entry_count",
        "denylist_entries",
        "denylist-entry",
        "--allow-missing-removals",
        "feed-promotion",
        "valid_bundle_digests",
        "appeal-override",
    ):
        assert stale not in roadmap
    for marker in (
        "`catalog_promotion`",
        "`catalog_digest_hex`",
        "`valid_catalog_digests`",
        "`min_catalog_entries`",
        "`min_catalog_changes`",
        "`gateway_compliance_denied`",
    ):
        assert marker in roadmap


def test_future_dated_seaglass_reports_are_not_readiness_evidence() -> None:
    reports_dir = REPO_ROOT / "specs/ministry/reports"
    status = read("specs/ministry/red_team_status.md")
    normalized_status = " ".join(status.split())

    assert not list(
        reports_dir.glob("2026-08-mod-red-team-operation-seaglass*.md")
    )
    for stale in (
        "SeaGlass",
        "Operation SeaGlass",
        "MINFO-RT-17",
        "MINFO-RT-18",
        "2026-08-18",
    ):
        assert stale not in status
    for marker in (
        "no completed genuine moderation red-team drill",
        "Only genuine runtime evidence may populate a completed-drill row",
        "Passing that structural guard does not verify referenced artefacts",
    ):
        assert marker in normalized_status
