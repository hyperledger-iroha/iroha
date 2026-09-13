//! Local committed-history authority for hardware stream-token observations.

use super::{StreamTokenHardwarePinsV1, StreamTokenIssuerError};
use iroha_core::query::stream_token_custody::read_stream_token_custody_control_at_v1;
use iroha_core::state::{State, StateReadOnly, WorldReadOnly};
use iroha_data_model::sorafs::capacity::ProviderId;
use mv::storage::StorageReadOnly;
use sorafs_manifest::signer::custody::SignerCustodyAnchorV1;
use sorafs_manifest::signer::{
    protocol::SignerPurposeBindingV1,
    stream_token_custody_control::StreamTokenCustodyControlStateV1,
    stream_token_evidence::SignerStreamTokenStateObservationBodyV1,
};
use std::{num::NonZeroUsize, sync::Arc};

#[derive(Clone, Copy)]
pub(super) struct FinalityFloorV1 {
    pub(super) height: u64,
    pub(super) block_hash: [u8; 32],
}

#[derive(Clone, Copy)]
pub(super) enum HistoricalFinalityV1 {
    Custody(SignerCustodyAnchorV1),
    Block(FinalityFloorV1),
}
#[cfg(test)]
impl HistoricalFinalityV1 {
    pub(super) fn coordinates(self) -> FinalityFloorV1 {
        match self {
            Self::Custody(anchor) => FinalityFloorV1 {
                height: anchor.height,
                block_hash: anchor.block_hash,
            },
            Self::Block(anchor) => anchor,
        }
    }
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
        historical: &[HistoricalFinalityV1],
        observation: &SignerStreamTokenStateObservationBodyV1,
    ) -> Result<(), StreamTokenIssuerError>;
}

pub(super) struct CoreFinalityV1 {
    state: Arc<State>,
    pins: StreamTokenHardwarePinsV1,
}
impl CoreFinalityV1 {
    pub(super) fn new(state: Arc<State>, pins: StreamTokenHardwarePinsV1) -> Self {
        Self { state, pins }
    }
}
impl HardwareFinalityV1 for CoreFinalityV1 {
    fn capture(
        &self,
        minimum: SignerCustodyAnchorV1,
    ) -> Result<FinalityFloorV1, StreamTokenIssuerError> {
        let view = self.state.view();
        check_registered_provider(&view, &self.pins)?;
        check_anchor(&view, &self.pins, minimum)?;
        let height = u64::try_from(view.block_hashes().len()).map_err(|_| unavailable())?;
        let block_hash = view
            .block_hashes()
            .last()
            .map(|hash| *hash.as_ref())
            .ok_or_else(unavailable)?;
        let current = read_stream_token_custody_control_at_v1(&view, self.pins.binding(), height)
            .map_err(|_| unavailable())?
            .ok_or_else(unavailable)?;
        check_block(&view, current.anchor.height, current.anchor.block_hash)?;
        check_control_pins(&current.state, &self.pins)?;
        if current.state.active_head.is_none()
            || current.state.signer_revoked
            || current.state.attester_revoked
        {
            return Err(unavailable());
        }
        Ok(FinalityFloorV1 { height, block_hash })
    }
    fn validate(
        &self,
        minimum: SignerCustodyAnchorV1,
        candidate: SignerCustodyAnchorV1,
        floor: FinalityFloorV1,
        historical: &[HistoricalFinalityV1],
        observation: &SignerStreamTokenStateObservationBodyV1,
    ) -> Result<(), StreamTokenIssuerError> {
        let view = self.state.view();
        check_registered_provider(&view, &self.pins)?;
        // All endpoints belong to one immutable committed history; a larger unsigned height or a
        // block-hash cache entry alone cannot establish ancestry or revision-4 finality.
        check_anchor(&view, &self.pins, minimum)?;
        check_block(&view, floor.height, floor.block_hash)?;
        let current = check_anchor(&view, &self.pins, candidate)?;
        check_observed_control(&current, candidate, observation)?;
        if historical.is_empty()
            || historical.len() > 3
            || !matches!(historical[0], HistoricalFinalityV1::Custody(anchor)
                if anchor == observation.active_head.approved_anchor)
        {
            return Err(unavailable());
        }
        for anchor in historical {
            match anchor {
                HistoricalFinalityV1::Custody(anchor) => {
                    check_anchor(&view, &self.pins, *anchor)?;
                }
                HistoricalFinalityV1::Block(anchor) => {
                    check_block(&view, anchor.height, anchor.block_hash)?;
                }
            }
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
pub(super) fn check_registered_provider(
    view: &impl StateReadOnly,
    pins: &StreamTokenHardwarePinsV1,
) -> Result<(), StreamTokenIssuerError> {
    let SignerPurposeBindingV1::StreamToken { provider_id } = pins.binding().purpose else {
        return Err(unavailable());
    };
    if view
        .world()
        .provider_owners()
        .get(&ProviderId::new(provider_id))
        .is_none()
    {
        return Err(unavailable());
    }
    Ok(())
}
fn check_anchor(
    view: &impl StateReadOnly,
    pins: &StreamTokenHardwarePinsV1,
    anchor: SignerCustodyAnchorV1,
) -> Result<StreamTokenCustodyControlStateV1, StreamTokenIssuerError> {
    if anchor.state_digest == [0; 32] {
        return Err(unavailable());
    }
    check_block(view, anchor.height, anchor.block_hash)?;
    let native = read_stream_token_custody_control_at_v1(view, pins.binding(), anchor.height)
        .map_err(|_| unavailable())?
        .ok_or_else(unavailable)?;
    if native.anchor != anchor {
        return Err(unavailable());
    }
    check_control_pins(&native.state, pins)?;
    Ok(native.state)
}
pub(super) fn check_control_pins(
    native: &StreamTokenCustodyControlStateV1,
    pins: &StreamTokenHardwarePinsV1,
) -> Result<(), StreamTokenIssuerError> {
    let trust = native.policy.custody_trust();
    let expected = pins.custody_trust();
    if &native.policy.binding != pins.binding()
        || trust.authority != expected.authority
        || trust.public_key != expected.public_key
        || trust.active_from_unix_ms != expected.active_from_unix_ms
        || trust.active_until_unix_ms != expected.active_until_unix_ms
        || trust.max_validity_ms != expected.max_validity_ms
        || trust.max_anchor_age_ms != expected.max_anchor_age_ms
    {
        return Err(unavailable());
    }
    Ok(())
}
pub(super) fn check_observed_control(
    native: &StreamTokenCustodyControlStateV1,
    candidate: SignerCustodyAnchorV1,
    observation: &SignerStreamTokenStateObservationBodyV1,
) -> Result<(), StreamTokenIssuerError> {
    if observation.current_anchor != candidate
        || native.active_head != Some(observation.active_head)
        || native.signer_revoked != observation.signer_revoked
        || native.attester_revoked != observation.attester_revoked
        || native.signer_revoked
        || native.attester_revoked
    {
        return Err(unavailable());
    }
    Ok(())
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
