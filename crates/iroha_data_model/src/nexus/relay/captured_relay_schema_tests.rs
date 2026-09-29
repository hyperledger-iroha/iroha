//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::VerifiedFeeSponsorVaultAllocation>(
        "iroha_data_model::nexus::relay::VerifiedFeeSponsorVaultAllocation",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::FeeSponsorVaultAllocationClaim>(
        "iroha_data_model::nexus::relay::FeeSponsorVaultAllocationClaim",
    ),
    crate::captured_schema_tests::Case::serialize::<super::FeeSponsorVaultSourceStateCommitment>(
        "iroha_data_model::nexus::relay::FeeSponsorVaultSourceStateCommitment",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

crate::captured_schema_tests::native_capture::owner_printer!(CASES);
