//! Production proof and wallet facades for first-release PQ-MASP.

use iroha_data_model::privacy::{
    PqMaspStarkStatementV1, PrivacyCommitmentV1, PrivacyEncryptedOutputV1, PrivacyStatementV1,
};
use rand::{TryCryptoRng, rngs::OsRng};
use soranet_pq::HedgedRngSeed;
use thiserror::Error;

use super::{
    relation::{
        PqMaspNotePlaintextV1, PqMaspRelationErrorV1, PqMaspWitnessV1,
        derive_pq_masp_note_commitment_v1, validate_pq_masp_relation_v1, validate_statement_v1,
    },
    stark::{prove_pq_masp_stark_v1_with_rng, verify_pq_masp_stark_v1},
    wire::{
        PqMaspWireErrorV1, authorize_pq_masp_stark_proof_v1, derive_pq_masp_recipient_id_v1,
        encrypt_pq_masp_note_v1 as encrypt_pq_masp_note_from_seed_v1,
        validate_pq_masp_authorization_secret_key_v1, verify_pq_masp_authorization_v1,
    },
};
use crate::privacy_engines::{
    proof_managed_note_stark::ProofManagedNoteStarkErrorV1,
    prover_randomness::{
        HealthCheckedTryCryptoRngV1, TryCryptoProverRandomnessErrorV1,
        derive_healthy_try_crypto_seed_v1,
    },
};

const PQ_MASP_AUTHORIZATION_HEDGE_PURPOSE_V1: &[u8] =
    b"iroha:privacy:pq-masp:authorization-hedge:v1";
const PQ_MASP_NOTE_ENCRYPTION_PURPOSE_V1: &[u8] = b"iroha:privacy:pq-masp:note-encryption-seed:v1";

/// Failure constructing or checking a complete authorized PQ-MASP proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PqMaspProofErrorV1 {
    /// The public statement or private witness failed the canonical relation.
    #[error(transparent)]
    Relation(#[from] PqMaspRelationErrorV1),
    /// ML-DSA authorization material or its complete outer wire was invalid.
    #[error(transparent)]
    Authorization(#[from] PqMaspWireErrorV1),
    /// Canonical statement encoding failed before authorization.
    #[error("PQ-MASP statement digest encoding failed")]
    StatementEncoding,
    /// The injected or operating-system cryptographic source failed.
    #[error("PQ-MASP prover entropy is unavailable")]
    RandomnessUnavailable,
    /// The cryptographic source emitted a catastrophic repeated pattern.
    #[error("PQ-MASP prover entropy failed its health check")]
    UnhealthyRandomness,
    /// A fixed proof or allocation bound was exceeded.
    #[error("PQ-MASP proof resource bound is exceeded")]
    ResourceLimit,
    /// Supplied proof bytes are malformed or invalid for the statement.
    #[error("PQ-MASP proof verification failed")]
    InvalidProof,
    /// A prevalidated witness could not be compiled by the fixed prover.
    #[error("PQ-MASP prover invariant failed")]
    ProverInvariant,
    /// The independent final check rejected bytes produced by the prover.
    #[error("PQ-MASP prover self-verification failed")]
    SelfVerification,
}

fn map_entropy_error_v1(error: TryCryptoProverRandomnessErrorV1) -> PqMaspProofErrorV1 {
    match error {
        TryCryptoProverRandomnessErrorV1::Unavailable => PqMaspProofErrorV1::RandomnessUnavailable,
        TryCryptoProverRandomnessErrorV1::Unhealthy => PqMaspProofErrorV1::UnhealthyRandomness,
    }
}

fn map_wallet_entropy_error_v1(error: TryCryptoProverRandomnessErrorV1) -> PqMaspWireErrorV1 {
    match error {
        TryCryptoProverRandomnessErrorV1::Unavailable => PqMaspWireErrorV1::RandomnessUnavailable,
        TryCryptoProverRandomnessErrorV1::Unhealthy => PqMaspWireErrorV1::UnhealthyRandomness,
    }
}

fn map_prover_error_v1(error: ProofManagedNoteStarkErrorV1) -> PqMaspProofErrorV1 {
    match error {
        ProofManagedNoteStarkErrorV1::Randomness => PqMaspProofErrorV1::RandomnessUnavailable,
        ProofManagedNoteStarkErrorV1::Resource => PqMaspProofErrorV1::ResourceLimit,
        ProofManagedNoteStarkErrorV1::InvalidProfile
        | ProofManagedNoteStarkErrorV1::InvalidTrace
        | ProofManagedNoteStarkErrorV1::Copy
        | ProofManagedNoteStarkErrorV1::Constraint
        | ProofManagedNoteStarkErrorV1::ProofWire
        | ProofManagedNoteStarkErrorV1::TraceOpening
        | ProofManagedNoteStarkErrorV1::Composition
        | ProofManagedNoteStarkErrorV1::Fri
        | ProofManagedNoteStarkErrorV1::Transcript
        | ProofManagedNoteStarkErrorV1::Internal => PqMaspProofErrorV1::ProverInvariant,
    }
}

fn map_verifier_error_v1(error: ProofManagedNoteStarkErrorV1) -> PqMaspProofErrorV1 {
    match error {
        ProofManagedNoteStarkErrorV1::Resource => PqMaspProofErrorV1::ResourceLimit,
        ProofManagedNoteStarkErrorV1::InvalidProfile | ProofManagedNoteStarkErrorV1::Internal => {
            PqMaspProofErrorV1::ProverInvariant
        }
        ProofManagedNoteStarkErrorV1::InvalidTrace
        | ProofManagedNoteStarkErrorV1::Copy
        | ProofManagedNoteStarkErrorV1::Constraint
        | ProofManagedNoteStarkErrorV1::ProofWire
        | ProofManagedNoteStarkErrorV1::TraceOpening
        | ProofManagedNoteStarkErrorV1::Composition
        | ProofManagedNoteStarkErrorV1::Fri
        | ProofManagedNoteStarkErrorV1::Transcript
        | ProofManagedNoteStarkErrorV1::Randomness => PqMaspProofErrorV1::InvalidProof,
    }
}

fn statement_digest_v1(
    statement: &PqMaspStarkStatementV1,
) -> Result<iroha_data_model::privacy::PrivacyStatementDigestV1, PqMaspProofErrorV1> {
    PrivacyStatementV1::PqMaspStarkV0(statement.clone())
        .digest()
        .map_err(|_| PqMaspProofErrorV1::StatementEncoding)
}

/// Construct a complete STARK-plus-ML-DSA PQ-MASP proof with injected entropy.
///
/// The function preflights the exact one-to-two relation and ML-DSA secret-key
/// binding before proof allocation. It builds the inner STARK, independently
/// derives a health-checked authorization hedge from fresh source bytes, signs
/// the canonical protocol-tagged statement digest and exact inner-proof digest,
/// then verifies the complete outer wire before returning it.
///
/// # Errors
///
/// Returns a typed relation, key, encoding, entropy, resource, or prover
/// failure. A successful result is never returned without final verification.
pub fn prove_pq_masp_v1_with_rng<R: TryCryptoRng + ?Sized>(
    statement: &PqMaspStarkStatementV1,
    witness: &PqMaspWitnessV1,
    authorization_secret_key: &[u8],
    randomness: &mut R,
) -> Result<Vec<u8>, PqMaspProofErrorV1> {
    validate_pq_masp_relation_v1(statement, witness)?;
    validate_pq_masp_authorization_secret_key_v1(
        statement.authorization_key_digest,
        authorization_secret_key,
    )?;
    let statement_digest = statement_digest_v1(statement)?;
    let mut checked_randomness =
        HealthCheckedTryCryptoRngV1::new(randomness).map_err(map_entropy_error_v1)?;
    let stark_proof = prove_pq_masp_stark_v1_with_rng(statement, witness, &mut checked_randomness)
        .map_err(map_prover_error_v1)?;
    let authorization_seed = checked_randomness
        .derive_independent_seed_v1(PQ_MASP_AUTHORIZATION_HEDGE_PURPOSE_V1)
        .map_err(map_entropy_error_v1)?;
    let proof = authorize_pq_masp_stark_proof_v1(
        statement_digest,
        statement.authorization_key_digest,
        authorization_secret_key,
        &stark_proof,
        HedgedRngSeed::from_entropy(*authorization_seed),
    )?;
    verify_pq_masp_v1(statement, &proof).map_err(|_| PqMaspProofErrorV1::SelfVerification)?;
    Ok(proof)
}

/// Construct a complete STARK-plus-ML-DSA PQ-MASP proof with OS entropy.
///
/// # Errors
///
/// Returns the same closed typed failures as [`prove_pq_masp_v1_with_rng`].
pub fn prove_pq_masp_v1(
    statement: &PqMaspStarkStatementV1,
    witness: &PqMaspWitnessV1,
    authorization_secret_key: &[u8],
) -> Result<Vec<u8>, PqMaspProofErrorV1> {
    prove_pq_masp_v1_with_rng(statement, witness, authorization_secret_key, &mut OsRng)
}

/// Verify one complete first-release PQ-MASP authorization and inner STARK.
///
/// # Errors
///
/// Rejects malformed, oversized, non-canonical, wrong-key,
/// statement-substituted, signature-invalid, or inner-proof-invalid bytes.
pub fn verify_pq_masp_v1(
    statement: &PqMaspStarkStatementV1,
    proof: &[u8],
) -> Result<(), PqMaspProofErrorV1> {
    validate_statement_v1(statement)?;
    let statement_digest = statement_digest_v1(statement)?;
    let authorization = verify_pq_masp_authorization_v1(
        statement_digest,
        statement.authorization_key_digest,
        proof,
    )?;
    verify_pq_masp_stark_v1(statement, authorization.stark_proof).map_err(map_verifier_error_v1)
}

/// Encrypt one PQ-MASP output note with injected, health-checked entropy.
///
/// The recipient binding and note commitment are checked before entropy is
/// consumed. All 64 sampled bytes influence the internal ML-KEM hedge seed.
///
/// # Errors
///
/// Rejects malformed keys or note material, recipient mismatch, entropy
/// failure or repetition, allocation failure, or authenticated-encryption
/// failure.
pub fn encrypt_pq_masp_note_v1_with_rng<R: TryCryptoRng + ?Sized>(
    statement: &PqMaspStarkStatementV1,
    note: &PqMaspNotePlaintextV1,
    recipient_public_key: &[u8],
    randomness: &mut R,
) -> Result<(PrivacyCommitmentV1, PrivacyEncryptedOutputV1), PqMaspWireErrorV1> {
    let recipient = derive_pq_masp_recipient_id_v1(recipient_public_key)?;
    if recipient != note.recipient_key_digest() {
        return Err(PqMaspWireErrorV1::EncryptedOutputBinding);
    }
    derive_pq_masp_note_commitment_v1(statement, note)
        .map_err(|_| PqMaspWireErrorV1::NoteCommitmentMismatch)?;
    let seed = derive_healthy_try_crypto_seed_v1(randomness, PQ_MASP_NOTE_ENCRYPTION_PURPOSE_V1)
        .map_err(map_wallet_entropy_error_v1)?;
    encrypt_pq_masp_note_from_seed_v1(
        statement,
        note,
        recipient_public_key,
        HedgedRngSeed::from_entropy(*seed),
    )
}

/// Encrypt one PQ-MASP output note with operating-system entropy.
///
/// # Errors
///
/// Returns the same closed typed failures as
/// [`encrypt_pq_masp_note_v1_with_rng`].
pub fn encrypt_pq_masp_note_v1(
    statement: &PqMaspStarkStatementV1,
    note: &PqMaspNotePlaintextV1,
    recipient_public_key: &[u8],
) -> Result<(PrivacyCommitmentV1, PrivacyEncryptedOutputV1), PqMaspWireErrorV1> {
    encrypt_pq_masp_note_v1_with_rng(statement, note, recipient_public_key, &mut OsRng)
}

#[cfg(test)]
mod tests {
    use rand::{TryCryptoRng, TryRngCore};

    use super::*;
    use crate::privacy_engines::pq_masp::{
        PQ_MASP_TREE_DEPTH_V1, PqMaspInputWitnessV1, PqMaspOutputWitnessV1, PqMaspWitnessV1,
        derive_pq_masp_nullifier_key_digest_v1,
    };

    #[derive(Debug)]
    struct InjectedEntropyError;

    impl core::fmt::Display for InjectedEntropyError {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("injected PQ-MASP entropy failure")
        }
    }

    enum EntropyMode {
        FailPartial,
        Constant,
        Repeated,
    }

    struct AdversarialRng(EntropyMode);

    impl TryRngCore for AdversarialRng {
        type Error = InjectedEntropyError;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Err(InjectedEntropyError)
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Err(InjectedEntropyError)
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
            match self.0 {
                EntropyMode::FailPartial => {
                    let midpoint = destination.len() / 2;
                    destination[..midpoint].fill(0x51);
                    Err(InjectedEntropyError)
                }
                EntropyMode::Constant => {
                    destination.fill(0x61);
                    Ok(())
                }
                EntropyMode::Repeated => {
                    for (index, byte) in destination.iter_mut().enumerate() {
                        *byte = [1, 3, 7, 9][index % 4];
                    }
                    Ok(())
                }
            }
        }
    }

    impl TryCryptoRng for AdversarialRng {}

    fn note() -> PqMaspNotePlaintextV1 {
        let secret = [0x31; 32];
        PqMaspNotePlaintextV1::new(
            11,
            iroha_data_model::privacy::PrivacyAuthorizationKeyDigestV1::new([0x32; 32]),
            iroha_data_model::privacy::PrivacyRecipientIdV1::new([0x33; 32]),
            derive_pq_masp_nullifier_key_digest_v1(&secret).expect("nullifier digest"),
            [0x34; 32],
            [0x35; 32],
            [0x36; 32],
        )
        .expect("note")
    }

    #[test]
    fn typed_witnesses_reject_malformed_material_and_redact_debug() {
        let note = note();
        assert_eq!(format!("{note:?}"), "PqMaspNotePlaintextV1(<redacted>)");
        assert_eq!(
            PqMaspInputWitnessV1::new(
                note.clone(),
                [0x41; 32],
                0,
                [[0x42; 32]; PQ_MASP_TREE_DEPTH_V1],
            ),
            Err(PqMaspRelationErrorV1::NullifierKeyMismatch)
        );
        assert_eq!(
            PqMaspInputWitnessV1::new(
                note.clone(),
                [0x31; 32],
                0,
                [[0; 32]; PQ_MASP_TREE_DEPTH_V1],
            ),
            Err(PqMaspRelationErrorV1::ZeroWitnessComponent)
        );
        let input = PqMaspInputWitnessV1::new(
            note.clone(),
            [0x31; 32],
            0,
            [[0x42; 32]; PQ_MASP_TREE_DEPTH_V1],
        )
        .expect("input");
        let output = PqMaspOutputWitnessV1::new(note).expect("output");
        assert_eq!(
            PqMaspWitnessV1::new(Vec::new(), vec![output.clone()]),
            Err(PqMaspRelationErrorV1::WitnessShape)
        );
        let witness = PqMaspWitnessV1::new(vec![input], vec![output]).expect("witness");
        let debug = format!("{witness:?}");
        assert!(debug.contains("input_count"));
        assert!(!debug.contains("31313131"));
    }

    #[test]
    fn authorization_seed_rejects_failure_constant_and_repeated_patterns() {
        assert_eq!(
            derive_healthy_try_crypto_seed_v1(
                &mut AdversarialRng(EntropyMode::FailPartial),
                PQ_MASP_AUTHORIZATION_HEDGE_PURPOSE_V1,
            )
            .map(|_| ()),
            Err(TryCryptoProverRandomnessErrorV1::Unavailable)
        );
        assert_eq!(
            derive_healthy_try_crypto_seed_v1(
                &mut AdversarialRng(EntropyMode::Constant),
                PQ_MASP_AUTHORIZATION_HEDGE_PURPOSE_V1,
            )
            .map(|_| ()),
            Err(TryCryptoProverRandomnessErrorV1::Unhealthy)
        );
        assert_eq!(
            derive_healthy_try_crypto_seed_v1(
                &mut AdversarialRng(EntropyMode::Repeated),
                PQ_MASP_AUTHORIZATION_HEDGE_PURPOSE_V1,
            )
            .map(|_| ()),
            Err(TryCryptoProverRandomnessErrorV1::Unhealthy)
        );
    }

    #[test]
    fn verifier_preflights_statement_bounds_before_outer_proof_parsing() {
        let (mut statement, _) = crate::privacy_engines::pq_masp::relation::tests::valid_fixture();
        let duplicate = statement.nullifiers[0];
        while statement.nullifiers.len() <= crate::privacy_engines::pq_masp::PQ_MASP_INPUT_BOUND_V1
        {
            statement.nullifiers.push(duplicate);
        }
        assert_eq!(
            verify_pq_masp_v1(&statement, &[]),
            Err(PqMaspProofErrorV1::Relation(
                PqMaspRelationErrorV1::InvalidStatement,
            ))
        );
    }
}
