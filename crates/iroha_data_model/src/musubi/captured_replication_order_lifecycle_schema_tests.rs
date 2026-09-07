//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<
        super::MusubiReplicationOrderArchiveBindingV1,
    >("iroha_data_model::musubi::MusubiReplicationOrderArchiveBindingV1");
    crate::captured_schema_tests::assert_bidirectional::<
        super::MusubiRetiredReplicationOrderLocationV1,
    >("iroha_data_model::musubi::MusubiRetiredReplicationOrderLocationV1");
    crate::captured_schema_tests::assert_bidirectional::<
        super::MusubiReplicationOrderLocationLifecycleV1,
    >("iroha_data_model::musubi::MusubiReplicationOrderLocationLifecycleV1");
    crate::captured_schema_tests::assert_bidirectional::<
        super::MusubiReplicationOrderLocationReferenceV1,
    >("iroha_data_model::musubi::MusubiReplicationOrderLocationReferenceV1");
}
