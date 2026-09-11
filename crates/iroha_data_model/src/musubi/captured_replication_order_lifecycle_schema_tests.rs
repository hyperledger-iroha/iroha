//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<
        super::MusubiReplicationOrderArchiveBindingV1,
    >("iroha_data_model::musubi::MusubiReplicationOrderArchiveBindingV1"),
    crate::captured_schema_tests::Case::bidirectional::<
        super::MusubiRetiredReplicationOrderLocationV1,
    >("iroha_data_model::musubi::MusubiRetiredReplicationOrderLocationV1"),
    crate::captured_schema_tests::Case::bidirectional::<
        super::MusubiReplicationOrderLocationLifecycleV1,
    >("iroha_data_model::musubi::MusubiReplicationOrderLocationLifecycleV1"),
    crate::captured_schema_tests::Case::bidirectional::<
        super::MusubiReplicationOrderLocationReferenceV1,
    >("iroha_data_model::musubi::MusubiReplicationOrderLocationReferenceV1"),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
