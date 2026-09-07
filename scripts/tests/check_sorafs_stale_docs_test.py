"""Focused guards for canonical SoraFS V1 documentation claims."""

from __future__ import annotations

import re
from pathlib import Path

from scripts.tests.sorafs_release_contract_support import release_module, required_release_kinds


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
    assert canonical_match.group(1) == header_match.group(1) == "23"
    assert "bridge source ABI is now 12" not in plan
    assert "sole first-release ABI, version 23" in plan


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


def test_fixture_readmes_do_not_claim_native_or_provider_qualification() -> None:
    provider = " ".join(
        read("fixtures/sorafs_manifest/provider_admission/README.md").split()
    )
    cookbook = " ".join(
        read("fixtures/documentation/sorafs_reference_sdk/README.md").split()
    )

    assert "exercise chunk scheduling end-to-end" not in provider
    assert (
        "These fixtures do not provide or qualify authenticated multi-provider "
        "transport, a governance-aware external software completion signer, a sealed-CAS "
        "retention backend, or four-validator deployment evidence."
        in provider
    )
    assert (
        "It is not evidence of clean ABI-23 builds for all five native release "
        "targets, skip-free SDK parity, published packages, external software signing, "
        "a qualified provider deployment, or L1/L2 promotion."
        in cookbook
    )


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
    active = "\n".join((protocol, chunk_range, handbook, playbook))
    normalized_active = " ".join(active.split())

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
    ):
        assert marker in normalized_active
    configuration = read("crates/iroha_config/src/parameters/actual.rs")
    token_config = re.search(r"pub struct SorafsTokenConfig \{(.*?)\n\}", configuration, re.DOTALL)
    assert token_config is not None
    fields = set(re.findall(r"pub ([a-z_]+):", token_config.group(1)))
    assert {"signer_handle", "signer_public_key", "signer_revision", "signer_policy_digest"} <= fields
    assert fields.isdisjoint({"signing_key_path", "token_signing_sk", "signing_seed"})


def test_gateway_release_contract_uses_catalog_authority() -> None:
    checker = release_module("check_sorafs_gateway_compliance_rollout_evidence")
    contract = release_module("sorafs_production_readiness_contract")
    assert "catalog_promotion" in required_release_kinds("gateway_compliance")
    assert {"catalog_entries", "catalog_changes", "catalog_signatures_verified"} <= set(
        checker.EVIDENCE_REQUIRED_FIELDS["catalog_promotion"]
    )
    assert (
        contract.PAYLOAD_FREE_SUMMARY_FINGERPRINT_HEX_LIST_BINDINGS["valid_catalog_digests"]
        == "catalog_digest_hex"
    )
    assert {"min_catalog_entries", "min_catalog_changes"} <= checker.ValidationOptions.__dataclass_fields__.keys()
    for kind, fields in checker.EVIDENCE_REQUIRED_FIELDS.items():
        assert kind not in {"feed-promotion", "appeal-override"}
        assert set(fields).isdisjoint({"denylist_entry_count", "denylist_entries", "valid_bundle_digests"})


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
