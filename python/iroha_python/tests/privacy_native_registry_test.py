"""Native compiled-profile catalog registry parity checks."""

from __future__ import annotations

import re
from pathlib import Path

from iroha_python.privacy_catalog import PRIVACY_PROTOCOL_IDS_V1

ROOT = Path(__file__).resolve().parents[3]


def test_native_hosts_use_the_typed_local_catalog_without_synthetic_network_state() -> None:
    for relative in (
        "crates/connect_norito_bridge/src/lib.rs",
        "crates/iroha_js_host/src/lib.rs",
        "python/iroha_python/iroha_python_rs/src/lib.rs",
    ):
        source = (ROOT / relative).read_text(encoding="utf-8")
        assert "PrivacyCompiledProfileCatalogV1" in source
        assert "PrivacyProtocolIdV1::ALL" in source
        assert "compiled_privacy_profile_catalog_v1" in source
        assert "validate_local_privacy_compiled_profile_catalog_archive_v1" in source
        assert "PrivacyConsensusPolicyV1::taira_default()" not in source
        assert "fn privacy_capabilities(" not in source
        assert "pub fn privacy_capabilities_v1(" not in source
        assert 'name = "privacy_capabilities_v1"' not in source
        assert "iroha_privacy_capabilities_v1" not in source
        assert "committed_privacy_capability_snapshot_v1" not in source
        assert "struct PrivacyAlgorithmEntry" not in source
        assert "struct PrivacyCapabilitiesV1" not in source


def test_runtime_readiness_is_only_built_from_a_fresh_committed_torii_view() -> None:
    runtime = (ROOT / "crates/iroha_torii/src/runtime.rs").read_text(
        encoding="utf-8"
    )
    state = (ROOT / "crates/iroha_core/src/state.rs").read_text(encoding="utf-8")
    assert "PrivacyCapabilitySnapshotV1" in runtime
    assert re.search(
        r"state\s*\.view\(\)\s*\.privacy_capability_snapshot_v1\(\)", runtime
    )
    assert "committed_privacy_capability_snapshot_v1(" in state
    assert "world.privacy_consensus_policy()" in state
    assert re.search(r"world\s*\.privacy_activations\(\)", state)


def test_python_ids_match_the_rust_first_release_labels() -> None:
    source = (ROOT / "crates/iroha_data_model/src/privacy.rs").read_text(encoding="utf-8")
    assert len(PRIVACY_PROTOCOL_IDS_V1) == 12
    for protocol_id in PRIVACY_PROTOCOL_IDS_V1:
        assert f'"{protocol_id}"' in source


def test_python_native_action_registry_covers_twelve_protocols_and_thirteen_actions() -> None:
    source = (ROOT / "python/iroha_python/iroha_python_rs/src/privacy_native_actions.rs").read_text(
        encoding="utf-8"
    )
    assert (
        "PRIVACY_NATIVE_ACTION_CAPABILITIES_V1: [PrivacyNativeActionCapabilityV1; 13]"
    ) in source
    for protocol_id in PRIVACY_PROTOCOL_IDS_V1:
        assert f'"{protocol_id}"' in source
    assert 'operation_schema: "zk_x509_identity_presentation_v1"' in source
    assert 'operation_schema: "zk_ams_batch_admission_action_v1"' in source
    assert 'operation_schema: "zk_ams_provision_account_action_v1"' in source
    assert "PrivacyNativeActionRequestV1::ZkX509" in source
    assert "ZK-X509 is intentionally absent" not in source


def test_python_exact12_receipt_bridge_is_sealed_to_typed_query_105() -> None:
    receipt_source = (
        ROOT
        / "python/iroha_python/iroha_python_rs/src/privacy_action_receipt.rs"
    ).read_text(encoding="utf-8")
    bridge_source = (
        ROOT / "python/iroha_python/iroha_python_rs/src/lib.rs"
    ).read_text(encoding="utf-8")
    client_source = (
        ROOT / "python/iroha_python/src/iroha_python/client.py"
    ).read_text(encoding="utf-8")

    for marker in (
        "FindPrivacyActionExecutionReceiptV1::new(",
        "sign_query_request_with_signer(",
        "PrivacyActionExecutionReceiptViewV1(",
        "receipt.validate()",
        "receipt.network_id != *network_id.as_inner()",
        "receipt.transaction_hash != *expected_transaction_hash.as_ref()",
        "receipt.proof_envelope_hash != expected_envelope",
    ):
        assert marker in receipt_source
    assert "mod privacy_action_receipt;" in bridge_source
    assert "privacy_action_receipt::build_query_with_signer" in bridge_source
    assert "privacy_action_receipt::inspect_response" in bridge_source
    assert '"/v1/query"' in client_source
    assert "canonical_auth.signer" in client_source
    assert "get_privacy_action_execution_receipt_v1" in client_source
