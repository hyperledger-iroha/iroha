//! Exact native block finality shared by signer state consumers.
//!
//! This boundary checks one read-only State view against its own durable Kura history and
//! Kura's cryptographically authenticated revision-4 finality for that State's exact network.
//! It does not establish custody, the freshness of a selected head, or an independently certified
//! application-state root.

use crate::state::StateReadOnly;
use std::num::NonZeroUsize;

/// Runtime evidence that an exact block belonged to the supplied native State view and its
/// durable, cryptographically verified Kura history for that State's network when it was read.
///
/// Only [`verify_signer_finality_v1`] constructs this value. It has no wire representation;
/// callers must check their own purpose-specific state and freshness against the same view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedSignerFinalityV1 {
    height: u64,
    block_hash: [u8; 32],
}

impl VerifiedSignerFinalityV1 {
    /// Exact one-based height authenticated by the native read.
    #[must_use]
    pub const fn height(&self) -> u64 {
        self.height
    }

    /// Exact canonical block-header hash authenticated by the native read.
    #[must_use]
    pub const fn block_hash(&self) -> [u8; 32] {
        self.block_hash
    }
}

/// The requested exact native block lacks matching, durable, authenticated finality.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("signer finality is unavailable for the exact native block")]
pub struct SignerFinalityErrorV1;

/// Verify an exact block using the same native view that supplies the caller's state claim.
///
/// Kura owns canonical block/header and complete-wire association, proof-of-possession and
/// CommitQC verification. This helper additionally binds that authority to the exact hash and
/// network in the supplied State view. The signed height context's network ID is the
/// genesis-derived replay identity; an operator-selected chain label cannot replace it.
/// A height/hash supplied by a remote observer is not authority.
///
/// # Errors
///
/// Returns [`SignerFinalityErrorV1`] for invalid coordinates, mismatched State/block/network,
/// missing finality, or any canonical-association or cryptographic verification failure.
/// Storage errors are deliberately not exposed in signer-facing diagnostics.
pub fn verify_signer_finality_v1(
    view: &impl StateReadOnly,
    height: u64,
    block_hash: [u8; 32],
) -> Result<VerifiedSignerFinalityV1, SignerFinalityErrorV1> {
    let height_index = usize::try_from(height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or(SignerFinalityErrorV1)?;
    if block_hash == [0; 32]
        || view
            .block_hashes()
            .get(height_index.get() - 1)
            .map(|value| *value.as_ref())
            != Some(block_hash)
        || view
            .kura()
            .get_durable_block_hash(height_index)
            .map(|value| *value.as_ref())
            != Some(block_hash)
    {
        return Err(SignerFinalityErrorV1);
    }
    let proof = view
        .kura()
        .v2_finality_artifact(height)
        .map_err(|_| SignerFinalityErrorV1)?
        .ok_or(SignerFinalityErrorV1)?;
    if proof.height != height
        || *proof.block_hash.as_ref() != block_hash
        || proof.height_context.network_id != *view.network_id()
    {
        return Err(SignerFinalityErrorV1);
    }
    Ok(VerifiedSignerFinalityV1 { height, block_hash })
}

#[cfg(test)]
mod tests;
