//! First-release NPoS schedule and candidate guards.
//!
//! Consensus randomness is supplied exclusively by finalized global
//! threshold-beacon pulses. The pre-release commit/reveal VRF lifecycle and
//! its peer messages are not part of the first-release protocol.

use crate::state::WorldReadOnly;
use iroha_data_model::block::consensus_v2 as wire;
use thiserror::Error;

/// Failure while resolving or validating first-release NPoS consensus state.
#[derive(Debug, Error)]
pub(crate) enum V2NposError {
    /// Frozen context itself is malformed.
    #[error("invalid frozen NPoS height context: {0}")]
    Context(#[from] wire::ValidationError),
    /// Authoritative v2 requires the signed genesis/on-chain NPoS parameter snapshot.
    #[error("authoritative v2 NPoS requires committed sumeragi_npos_parameters")]
    MissingCommittedParameters,
}

/// Resolve the committed epoch length used by the first-release NPoS schedule.
pub(crate) fn committed_epoch_length_blocks(
    world: &impl WorldReadOnly,
) -> Result<u64, V2NposError> {
    world
        .sumeragi_npos_parameters()
        .map(|params| params.epoch_length_blocks().get())
        .ok_or(V2NposError::MissingCommittedParameters)
}

/// Validate the frozen context of one authoritative v2 NPoS candidate.
pub(crate) fn validate_candidate_context(context: &wire::HeightContext) -> Result<(), V2NposError> {
    context.validate()?;
    Ok(())
}
