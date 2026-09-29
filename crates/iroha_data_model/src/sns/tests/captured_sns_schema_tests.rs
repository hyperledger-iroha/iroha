//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    #[cfg(test)]
    crate::captured_schema_tests::Case::serialize::<super::ForgedTokenValue>(
        "iroha_data_model::sns::tests::ForgedTokenValue",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

crate::captured_schema_tests::native_capture::owner_printer!(CASES);
