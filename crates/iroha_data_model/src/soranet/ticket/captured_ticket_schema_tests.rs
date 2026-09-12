//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] =
    &[crate::captured_schema_tests::Case::serialize::<
        super::TicketSignaturePayloadV1,
    >(
        "iroha_data_model::soranet::ticket::TicketSignaturePayloadV1",
    )];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
