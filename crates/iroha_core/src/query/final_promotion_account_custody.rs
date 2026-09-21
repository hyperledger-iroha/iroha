//! Native deployment-scoped custody of the independently governed transaction account.
//!
//! These same-State snapshots are raw native history, not hardware or finality capabilities.
//! The observation owner separately authenticates exact Check execution and the current cut.
use super::signer_custody_history::{self as history, AccountPurpose, HistoryError};
use crate::state::StateReadOnly;
use iroha_data_model::sorafs::final_promotion_account_custody::FinalPromotionAccountCustodyRecordV1;
use sorafs_manifest::signer::{
    custody::{SignerCustodyAnchorV1, SignerCustodyBindingV1},
    custody_control::SignerCustodyControlStateV1,
    protocol::SignerPurposeBindingV1,
};

pub(crate) mod check;
pub mod observation;

/// Payload-free native account-custody failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("final-promotion account custody rejected: {self:?}")]
pub enum FinalPromotionAccountCustodyErrorV1 {
    /// Malformed, oversized or noncanonical public input.
    Invalid,
    /// Exact chain, network, purpose, account or permission differs.
    BindingMismatch,
    /// Retained custody rows or immutable indexes are inconsistent.
    CorruptHistory,
    /// An independently required committed height is unavailable.
    HeightUnavailable,
    /// Exact custody predecessor or compare-and-swap expectation differs.
    Conflict,
    /// The finite custody revision capacity is exhausted.
    Capacity,
    /// A key or policy generation reuses retired authority or rolls back.
    Generation,
    /// Current independently attested signer authorization is ineligible.
    Custody,
}
impl From<HistoryError> for FinalPromotionAccountCustodyErrorV1 {
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

/// Coherent native account-custody history from one committed State view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinalPromotionAccountCustodySnapshotV1 {
    /// Exact original transition and admitted enrollment bytes.
    pub control_record: FinalPromotionAccountCustodyRecordV1,
    /// Governed account key, independent attester, enrollment head and revocations.
    pub control: SignerCustodyControlStateV1,
    /// Selected committed block hash bound to this exact custody record digest.
    pub custody_anchor: SignerCustodyAnchorV1,
}

/// Read this purpose's exact custody state at the independently selected committed height.
///
/// # Errors
/// Rejects foreign purposes or bindings, missing blocks and corrupt retained history.
/// This read does not establish current permissions, physical custody or consensus finality.
pub fn read_final_promotion_account_custody_at_v1(
    state: &impl StateReadOnly,
    binding: &SignerCustodyBindingV1,
    height: u64,
) -> Result<Option<FinalPromotionAccountCustodySnapshotV1>, FinalPromotionAccountCustodyErrorV1> {
    read_at(state, binding, height).map_err(Into::into)
}

fn read_at(
    state: &impl StateReadOnly,
    binding: &SignerCustodyBindingV1,
    height: u64,
) -> Result<Option<FinalPromotionAccountCustodySnapshotV1>, HistoryError> {
    let SignerPurposeBindingV1::FinalPromotionAccountTransaction { deployment_id } =
        &binding.purpose
    else {
        return Err(HistoryError::BindingMismatch);
    };
    history::validate_binding::<AccountPurpose>(state, binding, deployment_id)?;
    let offset = height
        .checked_sub(1)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(HistoryError::HeightUnavailable)?;
    let hash = state
        .block_hashes()
        .get(offset)
        .ok_or(HistoryError::HeightUnavailable)?;
    let Some(current) =
        history::read_control_at::<AccountPurpose>(state.world(), deployment_id, height)?
    else {
        return Ok(None);
    };
    if current.state.policy.binding != *binding {
        return Err(HistoryError::BindingMismatch);
    }
    Ok(Some(FinalPromotionAccountCustodySnapshotV1 {
        control_record: current.record,
        control: current.state,
        custody_anchor: SignerCustodyAnchorV1 {
            height,
            block_hash: *hash.as_ref(),
            state_digest: current.index.digest,
        },
    }))
}
