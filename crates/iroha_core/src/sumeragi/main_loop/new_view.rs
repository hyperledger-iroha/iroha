//! `NewView` validation and freshness helpers.

use iroha_crypto::Signature;
use iroha_data_model::{ChainId, peer::PeerId};

use super::locked_qc::{LockedQcRejection, ensure_locked_qc_allows};
use crate::sumeragi::consensus::{NewView, Phase, Qc, QcHeaderRef, new_view_preimage};

fn highest_qc_is_precommit(qc: &Qc) -> bool {
    matches!(qc.phase, Phase::Precommit)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NewViewValidationError {
    /// The claimed sender index is outside the commit topology.
    SenderOutOfBounds,
    /// Highest QC must be a precommit QC.
    WrongPhase,
    /// The frame is missing a signature.
    MissingSignature,
    /// Signature does not verify against the sender’s public key.
    InvalidSignature,
}

pub(super) fn validate_new_view_signature(
    chain_id: &ChainId,
    mode_tag: &str,
    topology: &[PeerId],
    frame: &NewView,
) -> Result<(), NewViewValidationError> {
    let sender_usize =
        usize::try_from(frame.sender).map_err(|_| NewViewValidationError::SenderOutOfBounds)?;
    let Some(peer) = topology.get(sender_usize) else {
        return Err(NewViewValidationError::SenderOutOfBounds);
    };
    if !highest_qc_is_precommit(&frame.highest_qc) {
        return Err(NewViewValidationError::WrongPhase);
    }
    if frame.signature.is_empty() {
        return Err(NewViewValidationError::MissingSignature);
    }
    let preimage = new_view_preimage(chain_id, mode_tag, frame);
    let signature = Signature::from_bytes(&frame.signature);
    signature
        .verify(peer.public_key(), &preimage)
        .map_err(|_| NewViewValidationError::InvalidSignature)
}

#[allow(variant_size_differences)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum NewViewFreshnessError {
    /// Local pacemaker is already on a newer view for this height.
    StaleView {
        /// Current local view index.
        local_view: u64,
    },
    /// Incoming `HighestQC` conflicts with the locked QC at the same height.
    LockedQcConflict(Box<LockedQcRejection>),
}

pub(super) fn check_new_view_freshness(
    local_view: Option<u64>,
    locked_qc: Option<QcHeaderRef>,
    frame_view: u64,
    highest_qc: QcHeaderRef,
) -> Result<(), NewViewFreshnessError> {
    if let Some(current) = local_view {
        if frame_view < current {
            return Err(NewViewFreshnessError::StaleView {
                local_view: current,
            });
        }
    }
    match ensure_locked_qc_allows(locked_qc, highest_qc) {
        Ok(()) | Err(LockedQcRejection::HeightRegressed { .. }) => Ok(()),
        Err(reason) => Err(NewViewFreshnessError::LockedQcConflict(Box::new(reason))),
    }
}
