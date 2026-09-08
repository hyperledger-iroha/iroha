"""Focused guards for canonical SoraFS V1 documentation claims."""

from __future__ import annotations

import re
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 uses the existing test dependency
    import tomli as tomllib

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
    checker = release_module("check_sorafs_reference_sdk_fixtures")
    payload_count = len(checker.EXPECTED_PAYLOADS)
    outcome_count = len(checker.EXPECTED_OUTCOMES)
    negative_count = sum(
        expectation != "valid"
        for _, _, _, expectation in checker.EXPECTED_PAYLOADS.values()
    )
    # Every repeated inventory claim must match the executable signed-inventory owner.
    count_claims = (
        (r"(\d+) payload artifacts", payload_count, 4),
        (r"(\d+) `ValidationOutcomeV1` (?:outcomes|files)", outcome_count, 2),
        (r"(\d+) exact outcome files", outcome_count, 1),
        (r"(\d+) outcomes", outcome_count, 1),
        (r"(\d+) negative payload vectors", negative_count, 4),
    )
    for pattern, expected, occurrences in count_claims:
        claims = re.findall(pattern, normalized)
        assert len(claims) == occurrences, pattern
        assert all(int(claim) == expected for claim in claims), (pattern, claims)

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
        f"inventory binds {payload_count} payload artifacts",
        f"{payload_count} payload artifacts",
        f"{outcome_count} `ValidationOutcomeV1` outcomes",
        f"{outcome_count} `ValidationOutcomeV1` files",
        f"{negative_count} negative payload vectors",
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


def _rust_struct_fields(relative: str, name: str) -> dict[str, str]:
    source = read(relative)
    structure = re.search(rf"pub struct {name} \{{(.*?)\n\}}", source, re.DOTALL)
    assert structure is not None, name
    return dict(re.findall(r"pub ([a-z_]+): ([^,\n]+)", structure.group(1)))


def test_stream_token_docs_use_the_runtime_signer_hard_cut() -> None:
    paths = (
        "specs/sorafs_node_client_protocol.md",
        "specs/sorafs_gateway_chunk_range.md",
        "specs/sorafs_gateway_deployment_handbook.md",
        "specs/sorafs_gateway_operator_playbook.md",
    )
    documents = [read(path) for path in paths]
    active = "\n".join(documents)
    normalized_active = " ".join(active.split())
    for stale in (
        "SORAFS_STREAM_TOKENS_ENABLED", "token_signing_sk", "signing_key_path",
        "when the signing key is not configured", "sign_with_seed",
        "signer_handle =", "signer_public_key_hex =", "signer_revision =",
        "signer_policy_digest_hex =", "key_version =", "StreamTokenRuntimeSigner",
        "BareRuntimeSigner", "external software signer probes", "both exact startup qualification probes",
    ):
        assert stale not in active
    for marker in (
        "only when issuance is disabled in node TOML",
        "No signing-seed file, key path, or environment enablement is accepted",
        "There is no environment-variable enablement or signing-seed path",
    ):
        assert marker in normalized_active
    for document in documents:
        assert "sorafs/stream_token_hardware_custody.md" in document
    actual = "crates/iroha_config/src/parameters/actual.rs"
    fields = _rust_struct_fields(actual, "SorafsTokenConfig")
    assert fields["hardware"] == "Option<SorafsStreamTokenHardwareConfig>"
    assert fields.keys().isdisjoint({
        "signer_handle", "signer_public_key", "signer_revision", "signer_policy_digest",
        "key_version", "signing_key_path", "token_signing_sk", "signing_seed",
    })
    hardware = _rust_struct_fields(
        "crates/iroha_config/src/parameters/actual/stream_token_hardware.rs",
        "SorafsStreamTokenHardwareConfig",
    )
    assert set(hardware) == {
        "runtime_handle", "key_handle", "service_id", "administrator_id", "public_key",
        "key_revision", "policy_revision", "policy_digest", "attester", "observer",
    }
    assert hardware["attester"] == "SorafsStreamTokenAttesterConfig"
    assert hardware["observer"] == "SorafsStreamTokenObserverConfig"
    user = _rust_struct_fields("crates/iroha_config/src/parameters/user.rs", "SorafsStreamTokenConfig")
    assert user["hardware"] == "SorafsStreamTokenHardwareConfig"
    assert "key_version" not in user


def test_stream_token_template_names_every_required_independent_hardware_pin() -> None:
    path = "specs/sorafs/snippets/stream_token_hardware_binding.toml"
    template = tomllib.loads(read(path))
    storage = template["sorafs"]["storage"]
    tokens = storage["stream_tokens"]
    hardware = tokens["hardware"]
    child = "crates/iroha_config/src/parameters/user/stream_token_hardware.rs"
    names = {"hardware": "SorafsStreamTokenHardwareConfig", "attester": "SorafsStreamTokenAttesterConfig",
             "observer": "SorafsStreamTokenObserverConfig"}
    assert set(hardware) == set(_rust_struct_fields(child, names["hardware"]))
    for role in ("attester", "observer"):
        assert set(hardware[role]) == set(_rust_struct_fields(child, names[role]))
    assert len(hardware) - 2 + len(hardware["attester"]) + len(hardware["observer"]) == 28
    assert "provider_id_hex" in storage and "provider_id_hex" not in hardware
    assert "key_version" not in tokens
    identities = [role[field] for role in (hardware, hardware["attester"], hardware["observer"])
                  for field in ("service_id", "administrator_id")]
    assert len(set(identities)) == 6
    assert len({role["public_key_hex"] for role in (hardware, hardware["attester"], hardware["observer"])}) == 3
    # The template is intentionally not ready-to-run trust or fabricated qualification.
    assert all(role["key_revision"] == role["policy_revision"] == 0
               for role in (hardware, hardware["attester"], hardware["observer"]))
    assert hardware["attester"]["active_from_unix_ms"] == hardware["attester"]["active_until_unix_ms"] == 0
    assert hardware["observer"]["active_from_unix_ms"] == hardware["observer"]["active_until_unix_ms"] == 0
    contract = " ".join(read("specs/sorafs/stream_token_hardware_custody.md").split())
    for marker in ("deliberately invalid", "all six service/administrator", "all three public keys",
                   "86,400,000 ms", "300,000 ms", "independent approved full anchor",
                   "real device provider", "genuinely current finalized-state observer",
                   "coherent native", "do not complete those qualification outcomes"):
        assert marker.lower() in contract.lower()


def test_stream_token_runtime_has_separate_trust_and_one_body_recovery_owners() -> None:
    prefix = "crates/iroha_torii/src/sorafs/token/"
    transport = read(prefix + "hardware_transport.rs")
    lifecycle = read(prefix + "hardware_lifecycle.rs")
    finality = read(prefix + "hardware_finality.rs")
    pins = read(prefix + "hardware_pins.rs")
    issuer = read("crates/iroha_torii/src/sorafs/token.rs")
    exports = read("crates/iroha_torii/src/sorafs/mod.rs")
    builders = read("crates/iroha_torii/src/sorafs/stream_token_runtime.rs")
    for retired in ("trait StreamTokenRuntimeSigner", "StreamTokenRuntimeSignerQualificationV1", "BareRuntimeSigner"):
        assert retired not in transport + issuer + exports
    for owner in ("StreamTokenHardwareClientV1", "StreamTokenStateObserverClientV1",
                  "StreamTokenApprovedCustodyAnchorV1", "StreamTokenHardwareReceiptV1"):
        assert owner in transport and owner in exports
    for builder in ("with_sorafs_stream_token_hardware_client", "with_sorafs_stream_token_state_observer",
                    "with_sorafs_stream_token_approved_anchor"):
        assert f"pub fn {builder}" in builders
    assert "Option<Arc<dyn StreamTokenHardwareClientV1>>" in issuer
    assert "Option<Arc<dyn StreamTokenStateObserverClientV1>>" in issuer
    assert "Option<StreamTokenApprovedCustodyAnchorV1>" in issuer
    for phase in ("Startup", "BeforeProvider", "AfterCommit", "BeforeRelease"):
        assert f"Phase::{phase}" in lifecycle
    assert re.sub(r"\s+", "", lifecycle).count("self.client.sign(") == 1
    assert re.sub(r"\s+", "", lifecycle).count("self.client.recover(") == 1
    assert "StreamTokenHardwareCallErrorV1::AmbiguousCompletion" in lifecycle
    for verifier in ("verify_stream_token_signer_current_evidence_v1", "verify_stream_token_signer_completed_observation_v1",
                     "verify_stream_token_signer_evidence_v1"):
        assert verifier in lifecycle
    assert "continues_active_state" in lifecycle and "historical_block(signing_anchor)" in lifecycle
    assert "v2_finality_artifact(height)" in finality and "proof.block_hash.as_ref()" in finality
    assert "get_durable_block_hash" in finality and "view.block_hashes()" in finality
    assert "stream_token_binding_digest_v1(&binding)" in pins
    assert "u32::try_from(hardware.key_revision)" in pins
    assert "norito::encode_canonical(token)" in issuer
    assert "norito::decode_canonical_with_limits" in issuer
    assert "norito::to_bytes(token)" not in issuer
    assert "fn validate_token_body" not in issuer
    shared = read("crates/sorafs_manifest/src/token.rs")
    assert "pub fn validate_token_body" in shared
    assert re.search(r"STREAM_TOKEN_MAX_WIRE_BYTES_V1:\s*usize\s*=\s*2_048", shared)
    producer = read("crates/irohad/src/signer_operation/stream_token.rs")
    assert "prepare_stream_token_signing_payload_v1" in producer
    assert "SignerReceiptPurposeV1::StreamToken" in producer
    assert "pub fn recover(" in producer
    native = read("crates/iroha_config/tests/sorafs_stream_token_runtime_signer/hardware_config_tests.rs")
    for witness in ("rejects_incomplete_disabled_and_non_production_forms", "rejects_noncanonical_or_invalid_ed25519_keys",
                    "provider_is_required_once_and_obsolete_flat_signer_fields_have_no_aliases", "independent"):
        assert witness in native
    config_regressions = read("crates/iroha_config/tests/sorafs_stream_token_runtime_signer.rs")
    assert "signing_key_path" in config_regressions
    assert "SORAFS_STREAM_TOKENS_ENABLED" in config_regressions
    assert "sorafs_configuration_has_no_production_environment_bindings" in config_regressions


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
