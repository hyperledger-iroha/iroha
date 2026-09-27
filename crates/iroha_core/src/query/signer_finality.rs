//! Exact native block finality shared by signer state consumers.
//!
//! This boundary reads one committed block of a read-only State view through the certified-chain
//! reader ([`crate::sumeragi::certified_chain`]): the block must be the one the view committed at
//! the height, stored in Kura with a commit certificate whose core header and result preimage
//! certify it, and whose `CommitQC` verifies under the committee of its height. It does not
//! establish custody, the freshness of a selected head, or an independently certified
//! application-state root.
//!
//! Instruction execution must not use this read: a block's `CommitQC` is node-local. Deterministic
//! code reads [`committed_block`](crate::sumeragi::certified_chain::committed_block) instead.

use iroha_data_model::block::consensus_v2::HeightContextId;

use crate::{
    state::StateReadOnly,
    sumeragi::certified_chain::{CertifiedBlock, CertifiedChain, ChainReadError},
};

/// Runtime evidence that an exact block belonged to the supplied native State view and its
/// durable, certified Kura history for that State's network when it was read.
///
/// Only [`verify_signer_finality_v1`] constructs this value. It has no wire representation;
/// callers must check their own purpose-specific state and freshness against the same view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedSignerFinalityV1 {
    height: u64,
    block_hash: [u8; 32],
    context_id: HeightContextId,
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

    /// The certified block id of the height (what signer floors pin as their `context_id`).
    #[must_use]
    pub const fn context_id(&self) -> HeightContextId {
        self.context_id
    }
}

/// The requested exact native block lacks matching, durable, certified finality.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("signer finality is unavailable for the exact native block")]
pub struct SignerFinalityErrorV1;

/// Verify an exact block using the same native view that supplies the caller's state claim.
///
/// The view's genesis must be its network's genesis (the network id is the genesis hash), the
/// block at `height` must be the one the view committed with hash `block_hash`, and its Kura
/// frame must carry a commit certificate that certifies it and verifies under the committee of
/// its height (or, for a height whose historical committee the retained chain no longer holds,
/// that the driver verified before storing it; see the reader's trust model). A height/hash
/// supplied by a remote observer is not authority.
///
/// # Errors
///
/// Returns [`SignerFinalityErrorV1`] for invalid coordinates, mismatched State/block/network,
/// missing finality, or any association or cryptographic verification failure. Storage errors
/// are deliberately not exposed in signer-facing diagnostics.
pub fn verify_signer_finality_v1(
    view: &(impl StateReadOnly + ?Sized),
    height: u64,
    block_hash: [u8; 32],
) -> Result<VerifiedSignerFinalityV1, SignerFinalityErrorV1> {
    let chain = CertifiedChain::new(view).map_err(|_| SignerFinalityErrorV1)?;
    certified_block_v1(&chain, height, block_hash).map(|block| VerifiedSignerFinalityV1 {
        height,
        block_hash,
        context_id: block.id(),
    })
}

/// [`verify_signer_finality_v1`] through a caller's reader, returning the certified block
/// (callers that read many heights keep one reader).
///
/// # Errors
/// As [`verify_signer_finality_v1`].
pub fn certified_block_v1<V: StateReadOnly + ?Sized>(
    chain: &CertifiedChain<'_, V>,
    height: u64,
    block_hash: [u8; 32],
) -> Result<CertifiedBlock, SignerFinalityErrorV1> {
    if block_hash == [0; 32] {
        return Err(SignerFinalityErrorV1);
    }
    let block = chain
        .certified(height)
        .map_err(|_: ChainReadError| SignerFinalityErrorV1)?;
    if *block.block_hash().as_ref() != block_hash {
        return Err(SignerFinalityErrorV1);
    }
    Ok(block)
}

#[cfg(test)]
mod tests;
