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
        production_source = source
        if relative == "crates/iroha_js_host/src/lib.rs":
            # Synthetic snapshots are legitimate only in this EOF test module.
            production_source = source.split(
                "\n#[cfg(test)]\nmod tests {", maxsplit=1
            )[0]
        assert "PrivacyCompiledProfileCatalogV1" in production_source
        assert "PrivacyProtocolIdV1::ALL" in production_source
        assert "compiled_privacy_profile_catalog_v1" in production_source
        assert "validate_local_privacy_compiled_profile_catalog_archive_v1" in production_source
        assert "PrivacyConsensusPolicyV1::taira_default()" not in production_source
        assert "fn privacy_capabilities(" not in production_source
        assert "pub fn privacy_capabilities_v1(" not in production_source
        assert 'name = "privacy_capabilities_v1"' not in production_source
        assert "iroha_privacy_capabilities_v1" not in production_source
        assert "committed_privacy_capability_snapshot_v1" not in production_source
        assert "struct PrivacyAlgorithmEntry" not in production_source
        assert "struct PrivacyCapabilitiesV1" not in production_source


def test_runtime_readiness_is_only_built_from_a_fresh_committed_torii_view() -> None:
    runtime = (ROOT / "crates/iroha_torii/src/runtime.rs").read_text(
        encoding="utf-8"
    )
    state = (ROOT / "crates/iroha_core/src/state.rs").read_text(encoding="utf-8")
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


def test_python_native_action_registry_covers_all_exact_twelve_protocols() -> None:
    source = (ROOT / "python/iroha_python/iroha_python_rs/src/privacy_native_actions.rs").read_text(
        encoding="utf-8"
    )
    assert (
        "PRIVACY_NATIVE_ACTION_CAPABILITIES_V1: [PrivacyNativeActionCapabilityV1; 12]"
    ) in source
    for protocol_id in PRIVACY_PROTOCOL_IDS_V1:
        assert f'"{protocol_id}"' in source
    assert 'operation_schema: "zk_x509_identity_presentation_v1"' in source
    assert "PrivacyNativeActionRequestV1::ZkX509" in source
    assert "ZK-X509 is intentionally absent" not in source
