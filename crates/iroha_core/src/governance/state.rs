//! Governance selection state (parliament members + alternates per epoch/term).

use iroha_data_model::{account::AccountId, isi::governance::CouncilDerivationKind};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    derive::{JsonDeserialize, JsonSerialize},
};

/// Parliament membership for a term/epoch.
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
    /// Count of candidates whose VRF proofs verified for this term.
    #[norito(default)]
    pub verified: u32,
    /// Total candidates considered (verified + rejected).
    #[norito(default)]
    pub candidate_count: u32,
    /// Derivation method used to compute the roster.
    #[norito(default)]
    pub derived_by: CouncilDerivationKind,
}

impl ParliamentTerm {
    /// Replace a missing member with the next alternate (if present).
    #[must_use]
    pub fn replace_member(&mut self, missing: &AccountId) -> bool {
        super::draw::replace_with_alternate(&mut self.members, &mut self.alternates, missing)
    }
}
