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
        test_module = "#[cfg(test)]\nmod tests {"
        assert source.count(test_module) == 1
        test_start = source.index(test_module)
        test_body_start = test_start + len(test_module)
        test_end = re.search(
            r"(?m)^}\n(?=(?:\n|#\[|pub |fn |use |include!|///))",
            source[test_body_start:],
        )
        assert test_end is not None
        production = source[:test_start] + source[test_body_start + test_end.end():]
        assert "PrivacyCompiledProfileCatalogV1" in production
        assert "PrivacyProtocolIdV1::ALL" in production
        assert "compiled_privacy_profile_catalog_v1" in production
        assert "validate_local_privacy_compiled_profile_catalog_archive_v1" in production
        assert "PrivacyConsensusPolicyV1::taira_default()" not in production
        assert "fn privacy_capabilities(" not in production
        assert "pub fn privacy_capabilities_v1(" not in production
        assert 'name = "privacy_capabilities_v1"' not in production
        assert "iroha_privacy_capabilities_v1" not in production
        assert "committed_privacy_capability_snapshot_v1" not in production
        assert "struct PrivacyAlgorithmEntry" not in production
        assert "struct PrivacyCapabilitiesV1" not in production


def test_runtime_readiness_is_only_built_from_a_fresh_committed_torii_view() -> None:
    runtime = (ROOT / "crates/iroha_torii/src/runtime.rs").read_text(
        encoding="utf-8"
    )
    state = (ROOT / "crates/iroha_core/src/state.rs").read_text(encoding="utf-8")
    handler_start = runtime.index("pub async fn handle_privacy_capabilities(")
    handler = runtime[
        handler_start:runtime.index(
            "/// GET /v1/node/query/projection/checkpoint", handler_start
        )
    ]
    assert "PrivacyExact12CapabilityManifestV1" in handler
    assert re.search(
        r"state\s*\.view\(\)\s*\.privacy_capability_snapshot_v1\(\)", handler
    )
    assert "snapshot.exact12_capability_manifest_v1()" in handler
    assert 'code: "privacy_capability_snapshot_invalid"' in handler
    assert 'code: "privacy_exact12_capability_manifest_invalid"' in handler
    assert "PrivacyConsensusPolicyV1::taira_default()" not in handler

    snapshot_start = state.index("fn privacy_capability_snapshot_v1(")
    snapshot = state[
        snapshot_start:state.index("fn qualified_privacy_activation_v1(", snapshot_start)
    ]
    assert "PrivacyCapabilitySnapshotV1" in snapshot
    assert "u64::try_from(self.height())" in snapshot
    assert "crate::privacy_profiles::committed_privacy_capability_snapshot_v1(" in snapshot
    assert "*world.privacy_consensus_policy()" in snapshot
    assert "world.privacy_exact12_qualification().clone()" in snapshot
    assert re.search(r"world\s*\.privacy_activations\(\)", snapshot)
    assert "async fn privacy_capabilities_are_built_from_one_committed_state_view()" in runtime


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
