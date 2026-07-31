"""Native capability registry parity checks."""

from __future__ import annotations

from pathlib import Path

from iroha_python.privacy_catalog import PRIVACY_PROTOCOL_IDS_V1

ROOT = Path(__file__).resolve().parents[3]


def test_native_hosts_use_the_typed_canonical_snapshot() -> None:
    for relative in (
        "crates/connect_norito_bridge/src/lib.rs",
        "crates/iroha_js_host/src/lib.rs",
        "python/iroha_python/iroha_python_rs/src/lib.rs",
    ):
        source = (ROOT / relative).read_text(encoding="utf-8")
        assert "PrivacyCapabilitySnapshotV1" in source
        assert "PrivacyProtocolIdV1::ALL" in source
        assert "struct PrivacyAlgorithmEntry" not in source
        assert "struct PrivacyCapabilitiesV1" not in source


def test_python_ids_match_the_rust_first_release_labels() -> None:
    source = (ROOT / "crates/iroha_data_model/src/privacy.rs").read_text(
        encoding="utf-8"
    )
    assert len(PRIVACY_PROTOCOL_IDS_V1) == 12
    for protocol_id in PRIVACY_PROTOCOL_IDS_V1:
        assert f'"{protocol_id}"' in source


def test_python_native_action_registry_covers_all_exact_twelve_protocols() -> None:
    source = (
        ROOT
        / "python/iroha_python/iroha_python_rs/src/privacy_native_actions.rs"
    ).read_text(encoding="utf-8")
    assert (
        "PRIVACY_NATIVE_ACTION_CAPABILITIES_V1: "
        "[PrivacyNativeActionCapabilityV1; 12]"
    ) in source
    for protocol_id in PRIVACY_PROTOCOL_IDS_V1:
        assert f'"{protocol_id}"' in source
    assert 'operation_schema: "zk_x509_identity_presentation_v1"' in source
    assert "PrivacyNativeActionRequestV1::ZkX509" in source
    assert "ZK-X509 is intentionally absent" not in source
