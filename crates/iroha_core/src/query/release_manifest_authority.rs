//! Purpose-owned role-13 custody readback from one committed State view.
//!
//! Raw history authenticates retained rows and a selected block hash. The separate block-finality
//! read joins that hash to the same State view's durable Kura/QC evidence. Neither read proves a
//! successful role-13 Check, current signer eligibility, or completed release operations.
use super::signer_custody_history::{self as history, HistoryError, ManifestPurpose};
use super::signer_finality::{VerifiedSignerFinalityV1, verify_signer_finality_v1};
use crate::state::{StateReadOnly, StateView};
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
    /// The requested height is not the current committed State height.
    StaleHeight,
    /// The exact State block lacks matching durable Kura/QC finality.
    FinalityUnavailable,
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

/// Raw role-13 custody paired with finality of its current State block.
///
/// This is only a block-history prerequisite. The record may not have a successful native
/// execution source; this type grants no signer or operation authority and has no wire form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseManifestCustodyBlockFinalityV1 {
    /// Raw role-13 custody retained at the current State height.
    custody: ReleaseManifestCustodySnapshotV1,
    /// Exact same-State durable block and revision-4 Kura/QC finality.
    block_finality: VerifiedSignerFinalityV1,
}
impl ReleaseManifestCustodyBlockFinalityV1 {
    /// Borrow the raw retained custody row; this does not grant signer or operation authority.
    #[must_use]
    pub const fn custody(&self) -> &ReleaseManifestCustodySnapshotV1 {
        &self.custody
    }

    /// Exact block finality paired with this raw row by the same-State reader.
    #[must_use]
    pub const fn block_finality(&self) -> VerifiedSignerFinalityV1 {
        self.block_finality
    }
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

/// Pair a role-13 raw custody row with finality of the current committed State block.
///
/// This validates the deployment binding and retained custody chain from one State view, then
/// authenticates its exact current block through that view's Kura/QC reader. It does not establish
/// that the role-13 transition executed successfully in that block, authenticate a Check, read an
/// operation, or authorize signing. A missing custody row remains `None` only after block finality
/// has been verified.
///
/// # Errors
/// Rejects stale heights, foreign bindings, inconsistent history, or absent/forked finality.
pub fn read_current_release_manifest_custody_block_finality_v1(
    state: &StateView<'_>,
    binding: &SignerCustodyBindingV1,
    height: u64,
) -> Result<Option<ReleaseManifestCustodyBlockFinalityV1>, ReleaseManifestCustodyErrorV1> {
    if height == 0 || usize::try_from(height).ok() != Some(state.block_hashes().len()) {
        return Err(ReleaseManifestCustodyErrorV1::StaleHeight);
    }
    let custody = read_at(state, binding, height)?;
    let block_hash = state
        .block_hashes()
        .get(state.block_hashes().len() - 1)
        .ok_or(ReleaseManifestCustodyErrorV1::HeightUnavailable)?;
    let block_finality = verify_signer_finality_v1(state, height, *block_hash.as_ref())
        .map_err(|_| ReleaseManifestCustodyErrorV1::FinalityUnavailable)?;
    Ok(
        custody.map(|custody| ReleaseManifestCustodyBlockFinalityV1 {
            custody,
            block_finality,
        }),
    )
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
