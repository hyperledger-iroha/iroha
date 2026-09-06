//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::RuntimeUpgradeId>(
        "iroha_data_model::runtime::RuntimeUpgradeId",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RuntimeUpgradeSbomDigest>(
        "iroha_data_model::runtime::RuntimeUpgradeSbomDigest",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RuntimeUpgradeManifest>(
        "iroha_data_model::runtime::RuntimeUpgradeManifest",
    );
    crate::captured_schema_tests::assert_bidirectional::<
        super::RuntimeUpgradeManifestSignaturePayload,
    >("iroha_data_model::runtime::RuntimeUpgradeManifestSignaturePayload");
    crate::captured_schema_tests::assert_bidirectional::<super::RuntimeUpgradeRecord>(
        "iroha_data_model::runtime::RuntimeUpgradeRecord",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RuntimeUpgradeStatus>(
        "iroha_data_model::runtime::RuntimeUpgradeStatus",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RuntimeUpgradeProvenanceError>(
        "iroha_data_model::runtime::RuntimeUpgradeProvenanceError",
    );
}
