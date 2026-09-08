//! Local committed-history authority for hardware stream-token observations.

use super::StreamTokenIssuerError;
use iroha_core::state::{State, StateReadOnly};
use sorafs_manifest::signer::custody::SignerCustodyAnchorV1;
use std::{num::NonZeroUsize, sync::Arc};

#[derive(Clone, Copy)]
pub(super) struct FinalityFloorV1 {
    pub(super) height: u64,
    pub(super) block_hash: [u8; 32],
}

// This trait is private to the caller. Only tests may inject a simulated history; the public
// constructor always derives its implementation from the same Core State used by Torii.
pub(super) trait HardwareFinalityV1: Send + Sync {
    fn capture(
        &self,
        minimum: SignerCustodyAnchorV1,
    ) -> Result<FinalityFloorV1, StreamTokenIssuerError>;
    fn validate(
        &self,
        minimum: SignerCustodyAnchorV1,
        candidate: SignerCustodyAnchorV1,
        floor: FinalityFloorV1,
        historical: &[FinalityFloorV1],
    ) -> Result<(), StreamTokenIssuerError>;
}

pub(super) struct CoreFinalityV1(Arc<State>);
impl CoreFinalityV1 {
    pub(super) fn new(state: Arc<State>) -> Self {
        Self(state)
    }
}
impl HardwareFinalityV1 for CoreFinalityV1 {
    fn capture(
        &self,
        minimum: SignerCustodyAnchorV1,
    ) -> Result<FinalityFloorV1, StreamTokenIssuerError> {
        let view = self.0.view();
        check_anchor(&view, minimum)?;
        let height = u64::try_from(view.block_hashes().len()).map_err(|_| unavailable())?;
        let block_hash = view
            .block_hashes()
            .last()
            .map(|hash| *hash.as_ref())
            .ok_or_else(unavailable)?;
        check_block(&view, height, block_hash)?;
        Ok(FinalityFloorV1 { height, block_hash })
    }
    fn validate(
        &self,
        minimum: SignerCustodyAnchorV1,
        candidate: SignerCustodyAnchorV1,
        floor: FinalityFloorV1,
        historical: &[FinalityFloorV1],
    ) -> Result<(), StreamTokenIssuerError> {
        let view = self.0.view();
        // All endpoints belong to one immutable committed history; a larger unsigned height or a
        // block-hash cache entry alone cannot establish ancestry or revision-4 finality.
        check_anchor(&view, minimum)?;
        check_block(&view, floor.height, floor.block_hash)?;
        check_anchor(&view, candidate)?;
        if historical.len() > 3 {
            return Err(unavailable());
        }
        for anchor in historical {
            check_block(&view, anchor.height, anchor.block_hash)?;
        }
        let latest = u64::try_from(view.block_hashes().len()).map_err(|_| unavailable())?;
        if candidate.height < floor.height
            || candidate.height < latest
            || candidate.height < minimum.height
            || (candidate.height == minimum.height && candidate != minimum)
        {
            return Err(unavailable());
        }
        Ok(())
    }
}
fn check_anchor(
    view: &impl StateReadOnly,
    anchor: SignerCustodyAnchorV1,
) -> Result<(), StreamTokenIssuerError> {
    if anchor.state_digest == [0; 32] {
        return Err(unavailable());
    }
    // TODO: Replace the independent approved full role-state floor with a genuine Core custody
    // control-state reader once that owner exists. Never derive its digest from an observer claim.
    check_block(view, anchor.height, anchor.block_hash)
}
fn check_block(
    view: &impl StateReadOnly,
    height: u64,
    hash: [u8; 32],
) -> Result<(), StreamTokenIssuerError> {
    let height_index = usize::try_from(height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(unavailable)?;
    if hash == [0; 32]
        || view
            .block_hashes()
            .get(height_index.get() - 1)
            .map(|value| *value.as_ref())
            != Some(hash)
        || view
            .kura()
            .get_durable_block_hash(height_index)
            .map(|value| *value.as_ref())
            != Some(hash)
    {
        return Err(unavailable());
    }
    let proof = view
        .kura()
        .v2_finality_artifact(height)
        .map_err(|_| unavailable())?
        .ok_or_else(unavailable)?;
    if proof.height != height || *proof.block_hash.as_ref() != hash {
        return Err(unavailable());
    }
    Ok(())
}
const fn unavailable() -> StreamTokenIssuerError {
    StreamTokenIssuerError::HardwareFinalityUnavailable
}
