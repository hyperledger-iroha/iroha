//! Governance selection state (parliament members + alternates per epoch/term).
use iroha_data_model::{account::AccountId, isi::governance::CouncilDerivationKind};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    derive::{JsonDeserialize, JsonSerialize},
};
/// Compatibility state for the retired independent epoch-council subsystem.
///
/// It remains decodable for existing world snapshots. Canonical first-release Parliament uses
/// attempt-local reducer state and never reads or writes this roster.
#[derive(
    Clone,
    Debug,
    Default,
    JsonSerialize,
    JsonDeserialize,
    NoritoSerialize,
    NoritoDeserialize,
    PartialEq,
    Eq,
)]
pub struct ParliamentTerm {
    /// Epoch/term index this draw corresponds to.
    pub epoch: u64,
    /// Members selected for this term (ordered).
    pub members: Vec<AccountId>,
    /// Alternates (ordered) to replace members who decline/are ineligible.
    #[norito(default)]
    pub alternates: Vec<AccountId>,
    /// Total eligible candidates considered by the historical draw.
    #[norito(default)]
    pub candidate_count: u32,
    /// Derivation method used to compute the roster.
    #[norito(default)]
    pub derived_by: CouncilDerivationKind,
}
