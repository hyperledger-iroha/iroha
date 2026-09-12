//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::EscrowEvent>(
        "iroha_data_model::events::data::escrow::EscrowEvent",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ConditionalEscrowAttested>(
        "iroha_data_model::events::data::escrow::ConditionalEscrowAttested",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AssetEscrowDisputed>(
        "iroha_data_model::events::data::escrow::AssetEscrowDisputed",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AssetEscrowResolved>(
        "iroha_data_model::events::data::escrow::AssetEscrowResolved",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
