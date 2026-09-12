//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] =
    &[crate::captured_schema_tests::Case::bidirectional::<
        super::CompoundPredicateWire,
    >("iroha_data_model::query::dsl::CompoundPredicateWire")];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
