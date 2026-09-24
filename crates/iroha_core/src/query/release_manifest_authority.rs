//! Purpose-owned role-13 custody readback from one committed State view.
//!
//! This raw history authenticates retained rows and a selected block hash. It does not prove
//! consensus finality, current signer eligibility, permission, or completed release operations.
use super::signer_custody_history::{self as history, HistoryError, ManifestPurpose};
use crate::state::StateReadOnly;
use iroha_data_model::sorafs::release_manifest_authority::ReleaseManifestCustodyRecordV1;
use sorafs_manifest::signer::{
    custody::{SignerCustodyAnchorV1, SignerCustodyBindingV1},
    custody_control::SignerCustodyControlStateV1,
    protocol::SignerPurposeBindingV1,
};

/// Payload-free raw role-13 custody history failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("release-manifest custody rejected: {self:?}")]
pub enum ReleaseManifestCustodyErrorV1 {
    /// Malformed, oversized or noncanonical input.
    Invalid,
    /// Chain, network, role, purpose or deployment differs.
    BindingMismatch,
    /// Retained custody row or immutable index is inconsistent.
    CorruptHistory,
    /// Selected committed height is unavailable.
    HeightUnavailable,
    /// Exact predecessor or compare-and-swap expectation differs.
    Conflict,
    /// Finite custody revision capacity is exhausted.
    Capacity,
    /// Key or policy generation is reused or rolled back.
    Generation,
    /// Signed enrollment or current custody assertion is ineligible.
    Custody,
}
impl From<HistoryError> for ReleaseManifestCustodyErrorV1 {
    fn from(value: HistoryError) -> Self {
        match value {
            HistoryError::Invalid => Self::Invalid,
            HistoryError::BindingMismatch => Self::BindingMismatch,
            HistoryError::CorruptHistory => Self::CorruptHistory,
            HistoryError::HeightUnavailable => Self::HeightUnavailable,
            HistoryError::Conflict => Self::Conflict,
            HistoryError::Capacity => Self::Capacity,
            HistoryError::Generation => Self::Generation,
            HistoryError::Custody => Self::Custody,
        }
    }
}

/// Coherent role-13 control row and block hash from one committed State view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseManifestCustodySnapshotV1 {
    /// Exact immutable custody transition and admitted enrollment bytes.
    pub control_record: ReleaseManifestCustodyRecordV1,
    /// Governed signer/attester key generations, active enrollment and revocations.
    pub control: SignerCustodyControlStateV1,
    /// Selected committed block hash bound to this custody record digest.
    pub custody_anchor: SignerCustodyAnchorV1,
}

/// Read exact role-13 custody at a selected committed height.
///
/// This is a raw same-State snapshot. A signer must separately authenticate an independently
/// authorized, successfully executed Check and its Kura/QC finality before using the key.
///
/// # Errors
/// Rejects foreign bindings, missing heights and corrupt retained history.
pub fn read_release_manifest_custody_at_v1(
    state: &impl StateReadOnly,
    binding: &SignerCustodyBindingV1,
    height: u64,
) -> Result<Option<ReleaseManifestCustodySnapshotV1>, ReleaseManifestCustodyErrorV1> {
    read_at(state, binding, height).map_err(Into::into)
}

fn read_at(
    state: &impl StateReadOnly,
    binding: &SignerCustodyBindingV1,
    height: u64,
) -> Result<Option<ReleaseManifestCustodySnapshotV1>, HistoryError> {
    let SignerPurposeBindingV1::ReleaseManifest { deployment_id } = &binding.purpose else {
        return Err(HistoryError::BindingMismatch);
    };
    history::validate_binding::<ManifestPurpose>(state, binding, deployment_id)?;
    let offset = height
        .checked_sub(1)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(HistoryError::HeightUnavailable)?;
    let hash = state
        .block_hashes()
        .get(offset)
        .ok_or(HistoryError::HeightUnavailable)?;
    let Some(current) =
        history::read_control_at::<ManifestPurpose>(state.world(), deployment_id, height)?
    else {
        return Ok(None);
    };
    if current.state.policy.binding != *binding {
        return Err(HistoryError::BindingMismatch);
    }
    Ok(Some(ReleaseManifestCustodySnapshotV1 {
        control_record: current.record,
        control: current.state,
        custody_anchor: SignerCustodyAnchorV1 {
            height,
            block_hash: *hash.as_ref(),
            state_digest: current.index.digest,
        },
    }))
}
