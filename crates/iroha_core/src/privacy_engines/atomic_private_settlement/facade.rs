//! Production-facing prover and verifier for one private settlement leg.

use super::{
    relation::{
        AtomicPrivateSettlementProverWitnessV1, AtomicPrivateSettlementRelationErrorV1,
        compile_witness_v1, validate_public_binding_v1,
    },
    stark::{
        prove_atomic_private_settlement_stark_v1_with_rng,
        verify_atomic_private_settlement_stark_v1,
    },
};
use crate::privacy_engines::{
    proof_managed_note_stark::ProofManagedNoteStarkErrorV1,
    prover_randomness::{HealthCheckedTryCryptoRngV1, TryCryptoProverRandomnessErrorV1},
};
use iroha_data_model::nexus::{AtomicPrivateSettlementV1, PrivateSettlementProofStatementV1};
use rand::{TryCryptoRng, rngs::OsRng};
use thiserror::Error;

/// Failure constructing or verifying one settlement-only private-note proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum AtomicPrivateSettlementProofErrorV1 {
    /// Public or private relation material failed its fixed binding.
    #[error(transparent)]
    Relation(#[from] AtomicPrivateSettlementRelationErrorV1),
    /// The injected or operating-system cryptographic source failed.
    #[error("atomic private settlement prover entropy is unavailable")]
    RandomnessUnavailable,
    /// The cryptographic source emitted a catastrophic repeated pattern.
    #[error("atomic private settlement prover entropy failed its health check")]
    UnhealthyRandomness,
    /// A fixed proof or allocation bound was exceeded.
    #[error("atomic private settlement proof resource bound is exceeded")]
    ResourceLimit,
    /// Supplied proof bytes are malformed or invalid for the public bundle and leg.
    #[error("atomic private settlement proof verification failed")]
    InvalidProof,
    /// The fixed prover profile is internally inconsistent.
    #[error("atomic private settlement prover invariant failed")]
    ProverInvariant,
    /// The independent final verifier rejected bytes emitted by the prover.
    #[error("atomic private settlement prover self-verification failed")]
    SelfVerification,
}

fn map_entropy_error_v1(
    error: TryCryptoProverRandomnessErrorV1,
) -> AtomicPrivateSettlementProofErrorV1 {
    match error {
        TryCryptoProverRandomnessErrorV1::Unavailable => {
            AtomicPrivateSettlementProofErrorV1::RandomnessUnavailable
        }
        TryCryptoProverRandomnessErrorV1::Unhealthy => {
            AtomicPrivateSettlementProofErrorV1::UnhealthyRandomness
        }
    }
}

fn map_prover_error_v1(error: ProofManagedNoteStarkErrorV1) -> AtomicPrivateSettlementProofErrorV1 {
    match error {
        ProofManagedNoteStarkErrorV1::Randomness => {
            AtomicPrivateSettlementProofErrorV1::RandomnessUnavailable
        }
        ProofManagedNoteStarkErrorV1::Resource => {
            AtomicPrivateSettlementProofErrorV1::ResourceLimit
        }
        ProofManagedNoteStarkErrorV1::InvalidProfile
        | ProofManagedNoteStarkErrorV1::InvalidTrace
        | ProofManagedNoteStarkErrorV1::Copy
        | ProofManagedNoteStarkErrorV1::Constraint
        | ProofManagedNoteStarkErrorV1::ProofWire
        | ProofManagedNoteStarkErrorV1::TraceOpening
        | ProofManagedNoteStarkErrorV1::Composition
        | ProofManagedNoteStarkErrorV1::Fri
        | ProofManagedNoteStarkErrorV1::Transcript
        | ProofManagedNoteStarkErrorV1::Internal => {
            AtomicPrivateSettlementProofErrorV1::ProverInvariant
        }
    }
}

fn map_verifier_error_v1(
    error: ProofManagedNoteStarkErrorV1,
) -> AtomicPrivateSettlementProofErrorV1 {
    match error {
        ProofManagedNoteStarkErrorV1::Resource => {
            AtomicPrivateSettlementProofErrorV1::ResourceLimit
        }
        ProofManagedNoteStarkErrorV1::InvalidProfile | ProofManagedNoteStarkErrorV1::Internal => {
            AtomicPrivateSettlementProofErrorV1::ProverInvariant
        }
        ProofManagedNoteStarkErrorV1::InvalidTrace
        | ProofManagedNoteStarkErrorV1::Copy
        | ProofManagedNoteStarkErrorV1::Constraint
        | ProofManagedNoteStarkErrorV1::ProofWire
        | ProofManagedNoteStarkErrorV1::TraceOpening
        | ProofManagedNoteStarkErrorV1::Composition
        | ProofManagedNoteStarkErrorV1::Fri
        | ProofManagedNoteStarkErrorV1::Transcript
        | ProofManagedNoteStarkErrorV1::Randomness => {
            AtomicPrivateSettlementProofErrorV1::InvalidProof
        }
    }
}

/// Construct one canonical settlement proof with injected masking entropy.
///
/// The exact public manifest, fixed leg statement, trusted genesis hash,
/// current height, auditor plaintext, two membership witnesses, and three
/// fixed outputs are checked before the full trace is allocated.
///
/// # Errors
///
/// Returns a redacted relation, entropy, resource, or prover failure.
pub fn prove_atomic_private_settlement_v1_with_rng<R: TryCryptoRng + ?Sized>(
    manifest: &AtomicPrivateSettlementV1,
    statement: &PrivateSettlementProofStatementV1,
    canonical_genesis_hash: [u8; 32],
    current_height: u64,
    witness: &AtomicPrivateSettlementProverWitnessV1,
    randomness: &mut R,
) -> Result<Vec<u8>, AtomicPrivateSettlementProofErrorV1> {
    validate_public_binding_v1(manifest, statement, canonical_genesis_hash, current_height)?;
    let compiled = compile_witness_v1(manifest, statement, witness)?;
    let mut checked_randomness =
        HealthCheckedTryCryptoRngV1::new(randomness).map_err(map_entropy_error_v1)?;
    let proof = prove_atomic_private_settlement_stark_v1_with_rng(
        manifest,
        statement,
        canonical_genesis_hash,
        current_height,
        &compiled,
        &mut checked_randomness,
    )
    .map_err(map_prover_error_v1)?;
    verify_atomic_private_settlement_stark_v1(
        manifest,
        statement,
        canonical_genesis_hash,
        current_height,
        &proof,
    )
    .map_err(|_| AtomicPrivateSettlementProofErrorV1::SelfVerification)?;
    Ok(proof)
}

/// Construct one canonical settlement proof with operating-system entropy.
///
/// # Errors
///
/// Returns the same closed failures as
/// [`prove_atomic_private_settlement_v1_with_rng`].
pub fn prove_atomic_private_settlement_v1(
    manifest: &AtomicPrivateSettlementV1,
    statement: &PrivateSettlementProofStatementV1,
    canonical_genesis_hash: [u8; 32],
    current_height: u64,
    witness: &AtomicPrivateSettlementProverWitnessV1,
) -> Result<Vec<u8>, AtomicPrivateSettlementProofErrorV1> {
    prove_atomic_private_settlement_v1_with_rng(
        manifest,
        statement,
        canonical_genesis_hash,
        current_height,
        witness,
        &mut OsRng,
    )
}

/// Verify one canonical settlement proof against the complete public bundle.
///
/// # Errors
///
/// Rejects expired, cross-network, manifest-substituted, statement-substituted,
/// malformed, oversized, non-canonical, or cryptographically invalid proofs.
pub fn verify_atomic_private_settlement_v1(
    manifest: &AtomicPrivateSettlementV1,
    statement: &PrivateSettlementProofStatementV1,
    canonical_genesis_hash: [u8; 32],
    current_height: u64,
    proof: &[u8],
) -> Result<(), AtomicPrivateSettlementProofErrorV1> {
    validate_public_binding_v1(manifest, statement, canonical_genesis_hash, current_height)?;
    verify_atomic_private_settlement_stark_v1(
        manifest,
        statement,
        canonical_genesis_hash,
        current_height,
        proof,
    )
    .map_err(map_verifier_error_v1)
}
