//! Compiler-captured identities for this module’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::BfvRamEncryptedInputMode>(
        "iroha_crypto::ram_lfe::BfvRamEncryptedInputMode",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::BfvRamProgramProfile>(
        "iroha_crypto::ram_lfe::BfvRamProgramProfile",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RamLfeVerificationMode>(
        "iroha_crypto::ram_lfe::RamLfeVerificationMode",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RamLfeProofVerifierMetadata>(
        "iroha_crypto::ram_lfe::RamLfeProofVerifierMetadata",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::HiddenRamFheInstruction>(
        "iroha_crypto::ram_lfe::HiddenRamFheInstruction",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::HiddenRamFheProgram>(
        "iroha_crypto::ram_lfe::HiddenRamFheProgram",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::BfvProgrammedPublicParameters>(
        "iroha_crypto::ram_lfe::BfvProgrammedPublicParameters",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RamLfeBackend>(
        "iroha_crypto::ram_lfe::RamLfeBackend",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PolicyCommitment>(
        "iroha_crypto::ram_lfe::PolicyCommitment",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ClientRequest>(
        "iroha_crypto::ram_lfe::ClientRequest",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::EvalResponse>(
        "iroha_crypto::ram_lfe::EvalResponse",
    );
}
