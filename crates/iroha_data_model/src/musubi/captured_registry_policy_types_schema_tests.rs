//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::MusubiRegistryAdmissionModeV1>(
        "iroha_data_model::musubi::MusubiRegistryAdmissionModeV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::MusubiRegistryPolicyV1>(
        "iroha_data_model::musubi::MusubiRegistryPolicyV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
