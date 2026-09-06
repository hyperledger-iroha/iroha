//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_serialize::<super::TicketSignaturePayloadV1>(
        "iroha_data_model::soranet::ticket::TicketSignaturePayloadV1",
    );
}
