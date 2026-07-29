//! Production prover facade for the first-release IVM private-note STARK.

use iroha_data_model::privacy::IrohaIvmPrivateNoteStarkStatementV1;
use rand::{TryCryptoRng, rngs::OsRng};
use thiserror::Error;

use super::{
    relation::{
        IvmPrivateNoteRelationErrorV1, IvmPrivateNoteWitnessV1, validate_private_note_relation_v1,
        validate_statement_v1,
    },
    stark::{prove_private_note_stark_v1_with_rng, verify_private_note_stark_v1},
};
use crate::privacy_engines::{
    proof_managed_note_stark::ProofManagedNoteStarkErrorV1,
    prover_randomness::{HealthCheckedTryCryptoRngV1, TryCryptoProverRandomnessErrorV1},
};

/// Failure constructing or checking a complete IVM private-note proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum IvmPrivateNoteProofErrorV1 {
    /// The public statement or private witness failed the canonical relation.
    #[error(transparent)]
    Relation(#[from] IvmPrivateNoteRelationErrorV1),
    /// The injected or operating-system cryptographic source failed.
    #[error("IVM private-note prover entropy is unavailable")]
    RandomnessUnavailable,
    /// The cryptographic source emitted a catastrophic repeated pattern.
    #[error("IVM private-note prover entropy failed its health check")]
    UnhealthyRandomness,
    /// A fixed proof or allocation bound was exceeded.
    #[error("IVM private-note proof resource bound is exceeded")]
    ResourceLimit,
    /// Supplied proof bytes are malformed or invalid for the statement.
    #[error("IVM private-note proof verification failed")]
    InvalidProof,
    /// A prevalidated witness could not be compiled by the fixed prover.
    #[error("IVM private-note prover invariant failed")]
    ProverInvariant,
    /// The independent final check rejected bytes produced by the prover.
    #[error("IVM private-note prover self-verification failed")]
    SelfVerification,
}

fn map_entropy_error_v1(error: TryCryptoProverRandomnessErrorV1) -> IvmPrivateNoteProofErrorV1 {
    match error {
        TryCryptoProverRandomnessErrorV1::Unavailable => {
            IvmPrivateNoteProofErrorV1::RandomnessUnavailable
        }
        TryCryptoProverRandomnessErrorV1::Unhealthy => {
            IvmPrivateNoteProofErrorV1::UnhealthyRandomness
        }
    }
}

fn map_prover_error_v1(error: ProofManagedNoteStarkErrorV1) -> IvmPrivateNoteProofErrorV1 {
    match error {
        ProofManagedNoteStarkErrorV1::Randomness => {
            IvmPrivateNoteProofErrorV1::RandomnessUnavailable
        }
        ProofManagedNoteStarkErrorV1::Resource => IvmPrivateNoteProofErrorV1::ResourceLimit,
        ProofManagedNoteStarkErrorV1::InvalidProfile
        | ProofManagedNoteStarkErrorV1::InvalidTrace
        | ProofManagedNoteStarkErrorV1::Copy
        | ProofManagedNoteStarkErrorV1::Constraint
        | ProofManagedNoteStarkErrorV1::ProofWire
        | ProofManagedNoteStarkErrorV1::TraceOpening
        | ProofManagedNoteStarkErrorV1::Composition
        | ProofManagedNoteStarkErrorV1::Fri
        | ProofManagedNoteStarkErrorV1::Transcript
        | ProofManagedNoteStarkErrorV1::Internal => IvmPrivateNoteProofErrorV1::ProverInvariant,
    }
}

fn map_verifier_error_v1(error: ProofManagedNoteStarkErrorV1) -> IvmPrivateNoteProofErrorV1 {
    match error {
        ProofManagedNoteStarkErrorV1::Resource => IvmPrivateNoteProofErrorV1::ResourceLimit,
        ProofManagedNoteStarkErrorV1::InvalidProfile | ProofManagedNoteStarkErrorV1::Internal => {
            IvmPrivateNoteProofErrorV1::ProverInvariant
        }
        ProofManagedNoteStarkErrorV1::InvalidTrace
        | ProofManagedNoteStarkErrorV1::Copy
        | ProofManagedNoteStarkErrorV1::Constraint
        | ProofManagedNoteStarkErrorV1::ProofWire
        | ProofManagedNoteStarkErrorV1::TraceOpening
        | ProofManagedNoteStarkErrorV1::Composition
        | ProofManagedNoteStarkErrorV1::Fri
        | ProofManagedNoteStarkErrorV1::Transcript
        | ProofManagedNoteStarkErrorV1::Randomness => IvmPrivateNoteProofErrorV1::InvalidProof,
    }
}

/// Construct a complete canonical private-note proof with injected entropy.
///
/// The complete relation and exact one-to-two cardinality bounds are checked
/// before entropy is sampled or the fixed trace is allocated. The returned
/// bytes have passed the independent production verifier.
///
/// # Errors
///
/// Returns a typed relation, entropy, resource, or prover failure. A successful
/// result is never returned without final self-verification.
pub fn prove_ivm_private_note_v1_with_rng<R: TryCryptoRng + ?Sized>(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
    witness: &IvmPrivateNoteWitnessV1,
    randomness: &mut R,
) -> Result<Vec<u8>, IvmPrivateNoteProofErrorV1> {
    validate_private_note_relation_v1(statement, witness)?;
    let mut checked_randomness =
        HealthCheckedTryCryptoRngV1::new(randomness).map_err(map_entropy_error_v1)?;
    let proof = prove_private_note_stark_v1_with_rng(statement, witness, &mut checked_randomness)
        .map_err(map_prover_error_v1)?;
    verify_private_note_stark_v1(statement, &proof)
        .map_err(|_| IvmPrivateNoteProofErrorV1::SelfVerification)?;
    Ok(proof)
}

/// Construct a complete canonical private-note proof with operating-system entropy.
///
/// # Errors
///
/// Returns the same closed typed failures as
/// [`prove_ivm_private_note_v1_with_rng`].
pub fn prove_ivm_private_note_v1(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
    witness: &IvmPrivateNoteWitnessV1,
) -> Result<Vec<u8>, IvmPrivateNoteProofErrorV1> {
    prove_ivm_private_note_v1_with_rng(statement, witness, &mut OsRng)
}

/// Verify one complete first-release IVM private-note proof.
///
/// # Errors
///
/// Rejects malformed, oversized, non-canonical, or statement-substituted proof
/// bytes with a typed failure.
pub fn verify_ivm_private_note_v1(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
    proof: &[u8],
) -> Result<(), IvmPrivateNoteProofErrorV1> {
    validate_statement_v1(statement)?;
    verify_private_note_stark_v1(statement, proof).map_err(map_verifier_error_v1)
}

#[cfg(test)]
mod tests {
    use rand::{TryCryptoRng, TryRngCore};

    use super::*;
    use crate::privacy_engines::ivm_private_note::{
        IvmPrivateNoteInputWitnessV1, IvmPrivateNoteOutputWitnessV1, IvmPrivateNoteWitnessV1,
        PRIVATE_NOTE_MAX_INPUTS_V1, PRIVATE_NOTE_TREE_DEPTH_V1,
        PRIVATE_PROGRAM_INSTRUCTION_COUNT_V1, PrivateInstructionV1, PrivateNotePlaintextV1,
        PrivateOpcodeV1, PrivateProgramV1, derive_note_authority_v1,
    };

    #[derive(Debug)]
    struct InjectedEntropyError;

    impl core::fmt::Display for InjectedEntropyError {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("injected IVM private-note entropy failure")
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

    #[test]
    fn typed_constructors_reject_malformed_material_and_debug_is_redacted() {
        assert_eq!(
            PrivateNotePlaintextV1::new(0, [1; 32], [2; 32], [3; 32], [0; 32]),
            Err(IvmPrivateNoteRelationErrorV1::ZeroWitnessComponent)
        );
        let secret = [0x41; 32];
        let note = PrivateNotePlaintextV1::new(
            9,
            derive_note_authority_v1(&secret).expect("authority"),
            [0x42; 32],
            [0x43; 32],
            [0x44; 32],
        )
        .expect("note");
        assert_eq!(format!("{note:?}"), "PrivateNotePlaintextV1(<redacted>)");
        assert_eq!(
            IvmPrivateNoteInputWitnessV1::new(
                note.clone(),
                [0x55; 32],
                0,
                [[0x66; 32]; PRIVATE_NOTE_TREE_DEPTH_V1],
            ),
            Err(IvmPrivateNoteRelationErrorV1::SpendingAuthorityMismatch)
        );
        assert_eq!(
            IvmPrivateNoteInputWitnessV1::new(
                note.clone(),
                secret,
                0,
                [[0; 32]; PRIVATE_NOTE_TREE_DEPTH_V1],
            ),
            Err(IvmPrivateNoteRelationErrorV1::ZeroWitnessComponent)
        );

        let mut instructions = [PrivateInstructionV1::HALT; PRIVATE_PROGRAM_INSTRUCTION_COUNT_V1];
        instructions[0] = PrivateInstructionV1::new(PrivateOpcodeV1::MoveImmediate, 7, 0, 0, 1)
            .expect("instruction");
        let program = PrivateProgramV1::new(instructions).expect("program");
        assert_eq!(format!("{program:?}"), "PrivateProgramV1(<redacted>)");
        let input = IvmPrivateNoteInputWitnessV1::new(
            note.clone(),
            secret,
            0,
            [[0x66; 32]; PRIVATE_NOTE_TREE_DEPTH_V1],
        )
        .expect("input");
        let output = IvmPrivateNoteOutputWitnessV1::new(note).expect("output");
        assert_eq!(
            IvmPrivateNoteWitnessV1::new(program.clone(), Vec::new(), vec![output.clone()]),
            Err(IvmPrivateNoteRelationErrorV1::WitnessShape)
        );
        let witness =
            IvmPrivateNoteWitnessV1::new(program, vec![input], vec![output]).expect("witness");
        let debug = format!("{witness:?}");
        assert!(debug.contains("input_count"));
        assert!(!debug.contains("41414141"));
    }

    #[test]
    fn facade_rejects_partial_constant_and_repeated_entropy_before_proving() {
        let value = crate::privacy_engines::ivm_private_note::tests::fixture();
        assert_eq!(
            prove_ivm_private_note_v1_with_rng(
                &value.statement,
                &value.witness,
                &mut AdversarialRng(EntropyMode::FailPartial),
            ),
            Err(IvmPrivateNoteProofErrorV1::RandomnessUnavailable)
        );
        for mode in [EntropyMode::Constant, EntropyMode::Repeated] {
            assert_eq!(
                prove_ivm_private_note_v1_with_rng(
                    &value.statement,
                    &value.witness,
                    &mut AdversarialRng(mode),
                ),
                Err(IvmPrivateNoteProofErrorV1::UnhealthyRandomness)
            );
        }
    }

    #[test]
    fn verifier_preflights_statement_bounds_before_proof_parsing() {
        let mut statement = crate::privacy_engines::ivm_private_note::tests::fixture().statement;
        let duplicate = statement.nullifiers[0];
        while statement.nullifiers.len() <= PRIVATE_NOTE_MAX_INPUTS_V1 {
            statement.nullifiers.push(duplicate);
        }
        assert_eq!(
            verify_ivm_private_note_v1(&statement, &[]),
            Err(IvmPrivateNoteProofErrorV1::Relation(
                IvmPrivateNoteRelationErrorV1::InvalidStatement,
            ))
        );
    }
}
