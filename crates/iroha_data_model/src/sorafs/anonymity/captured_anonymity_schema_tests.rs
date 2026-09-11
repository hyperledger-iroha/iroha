//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::SorafsCitizenBondExitPendingV1>(
        "iroha_data_model::sorafs::anonymity::SorafsCitizenBondExitPendingV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SorafsCitizenBondStateV1>(
        "iroha_data_model::sorafs::anonymity::SorafsCitizenBondStateV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SorafsCitizenBondV1>(
        "iroha_data_model::sorafs::anonymity::SorafsCitizenBondV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SorafsCitizenBondSnapshotV1>(
        "iroha_data_model::sorafs::anonymity::SorafsCitizenBondSnapshotV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
