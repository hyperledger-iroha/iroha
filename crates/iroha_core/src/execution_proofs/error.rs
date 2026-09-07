//! Shared native execution proof failure categories.

use super::stark::proof_managed_note_stark::ProofManagedNoteStarkErrorV1;
use thiserror::Error;

/// A proof fails closed at its exact validation layer.
#[derive(Debug, Error)]
pub enum ExecutionProofErrorV1 {
    /// Wrong envelope, resource limit, or canonical payload.
    #[error("invalid or oversized native execution proof envelope")]
    Envelope,
    /// Public claim differs from the exact compiled profile or replay.
    #[error("native race public statement mismatch")]
    Statement,
    /// Transcript or terminal-state structure is invalid.
    #[error("invalid native race replay structure")]
    Replay,
    /// The replay does not extend the exact authenticated consensus history.
    #[error("native race proof does not bind the retained chain history")]
    History,
    /// Native cryptographic proof verification failed.
    #[error("native race STARK failed: {0}")]
    Cryptography(String),
}
impl From<ProofManagedNoteStarkErrorV1> for ExecutionProofErrorV1 {
    fn from(error: ProofManagedNoteStarkErrorV1) -> Self {
        Self::Cryptography(error.to_string())
    }
}
