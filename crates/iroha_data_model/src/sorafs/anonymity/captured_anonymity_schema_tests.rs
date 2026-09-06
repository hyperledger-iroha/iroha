//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::SorafsCitizenBondExitPendingV1>(
        "iroha_data_model::sorafs::anonymity::SorafsCitizenBondExitPendingV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SorafsCitizenBondStateV1>(
        "iroha_data_model::sorafs::anonymity::SorafsCitizenBondStateV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SorafsCitizenBondV1>(
        "iroha_data_model::sorafs::anonymity::SorafsCitizenBondV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SorafsCitizenBondSnapshotV1>(
        "iroha_data_model::sorafs::anonymity::SorafsCitizenBondSnapshotV1",
    );
}
