//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::Case::bidirectional::<super::PrivacyProofBytesV1>(
        "iroha_data_model::privacy::PrivacyProofBytesV1",
    )
    .check();
    crate::captured_schema_tests::Case::bidirectional::<super::IrohaZkAmsProofV1>(
        "iroha_data_model::privacy::IrohaZkAmsProofV1",
    )
    .check();
    crate::captured_schema_tests::Case::bidirectional::<super::PrivacyProofV1>(
        "iroha_data_model::privacy::PrivacyProofV1",
    )
    .check();
    crate::captured_schema_tests::Case::bidirectional::<super::PrivacyProofEnvelopeV1>(
        "iroha_data_model::privacy::PrivacyProofEnvelopeV1",
    )
    .check();
}

#[test]
fn captured_exact12_conformance_frame_identities() {
    fn check<
        T: norito::NoritoSchema + norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
    >(
        nominal: &str,
        frame: &str,
        hash: &str,
    ) {
        assert_eq!(T::nominal_name(), nominal);
        assert_eq!(T::frame_name(), frame);
        assert_eq!(
            hex::encode(norito::schema::identity::frame_hash::<T>()),
            hash
        );
    }
    check::<super::exact12_fixture::PrivacyExact12TypedFixtureRowV1>(
        "iroha_data_model::privacy::exact12_fixture::PrivacyExact12TypedFixtureRowV1",
        "iroha.privacy.exact12-typed-fixture-row.v1",
        "05c3c7a70da64e2dde1b7821150043e7",
    );
    check::<super::exact12_fixture::PrivacyExact12FixtureBundleV1>(
        "iroha_data_model::privacy::exact12_fixture::PrivacyExact12FixtureBundleV1",
        "iroha.privacy.exact12-typed-fixture-bundle.v1",
        "48c8d56dfc59c50888aef4db2279c3b7",
    );
}
