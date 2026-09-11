//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::ProofOutcomeSignerPolicyV1>(
        "iroha_data_model::sorafs::proof_ledger::ProofOutcomeSignerPolicyV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProofOutcomeSignerPolicyRecordV1>(
        "iroha_data_model::sorafs::proof_ledger::ProofOutcomeSignerPolicyRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProofOutcomeKindV1>(
        "iroha_data_model::sorafs::proof_ledger::ProofOutcomeKindV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PdpOutcomeStatusV1>(
        "iroha_data_model::sorafs::proof_ledger::PdpOutcomeStatusV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PotrOutcomeStatusV1>(
        "iroha_data_model::sorafs::proof_ledger::PotrOutcomeStatusV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProofOutcomeEd25519AttestationV1>(
        "iroha_data_model::sorafs::proof_ledger::ProofOutcomeEd25519AttestationV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PdpOutcomeProjectionV1>(
        "iroha_data_model::sorafs::proof_ledger::PdpOutcomeProjectionV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PotrOutcomeProjectionV1>(
        "iroha_data_model::sorafs::proof_ledger::PotrOutcomeProjectionV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProofOutcomeProjectionV1>(
        "iroha_data_model::sorafs::proof_ledger::ProofOutcomeProjectionV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProofOutcomeRecordV1>(
        "iroha_data_model::sorafs::proof_ledger::ProofOutcomeRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProofOutcomeFinalizedCursorV1>(
        "iroha_data_model::sorafs::proof_ledger::ProofOutcomeFinalizedCursorV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProofOutcomeFinalizedRecordV1>(
        "iroha_data_model::sorafs::proof_ledger::ProofOutcomeFinalizedRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProofOutcomeFinalizedEventCursorV1>(
        "iroha_data_model::sorafs::proof_ledger::ProofOutcomeFinalizedEventCursorV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProofOutcomeFinalizedEventV1>(
        "iroha_data_model::sorafs::proof_ledger::ProofOutcomeFinalizedEventV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProofOutcomeFinalizedEventPageV1>(
        "iroha_data_model::sorafs::proof_ledger::ProofOutcomeFinalizedEventPageV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
