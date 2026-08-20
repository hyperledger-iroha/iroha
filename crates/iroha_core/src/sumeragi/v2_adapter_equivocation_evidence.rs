/// Exact process-local pair of authenticated artifacts proving equivocation.
///
/// The variants are deliberately closed over the three signed consensus
/// message classes which can equivocate. Offender, round, and kind are derived
/// from the pair and cannot be supplied independently.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AdapterEquivocationEvidence {
    /// Two different proposals signed by one round leader.
    Proposal(SealedEquivocationPair<wire::Proposal>),
    /// Two different vote statements signed in one phase and round.
    Vote(SealedEquivocationPair<wire::Vote>),
    /// Two different high-QC claims signed for one timeout round.
    TimeoutVote(SealedEquivocationPair<wire::TimeoutVote>),
}
/// An authenticated same-class conflict whose constructor is sealed inside
/// the adapter module.
///
/// Sibling production modules may inspect or clone an already-minted carrier,
/// but cannot replace either signed artifact or manufacture a new pair from
/// structurally valid, unauthenticated wire values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SealedEquivocationPair<T> {
    first: T,
    second: T,
}
impl<T> SealedEquivocationPair<T> {
    fn new(first: T, second: T) -> Self {
        Self { first, second }
    }
}
impl AdapterEquivocationEvidence {
    fn proposal(first: wire::Proposal, second: wire::Proposal) -> Self {
        Self::Proposal(SealedEquivocationPair::new(first, second))
    }
    fn vote(first: wire::Vote, second: wire::Vote) -> Self {
        Self::Vote(SealedEquivocationPair::new(first, second))
    }
    fn timeout_vote(first: wire::TimeoutVote, second: wire::TimeoutVote) -> Self {
        Self::TimeoutVote(SealedEquivocationPair::new(first, second))
    }
    /// Return the conflicting message class derived from the pair variant.
    pub(crate) const fn kind(&self) -> reducer::EquivocationKind {
        match self {
            Self::Proposal(_) => reducer::EquivocationKind::Proposal,
            Self::Vote(_) => reducer::EquivocationKind::Vote,
            Self::TimeoutVote(_) => reducer::EquivocationKind::Timeout,
        }
    }
    /// Return the offending validator index derived from the first artifact.
    pub(crate) const fn offender_index(&self) -> wire::ValidatorIndex {
        match self {
            Self::Proposal(pair) => pair.first.proposer,
            Self::Vote(pair) => pair.first.signer,
            Self::TimeoutVote(pair) => pair.first.signer,
        }
    }
    /// Return the common conflict round derived from the first artifact.
    pub(crate) const fn round(&self) -> wire::ConsensusRound {
        match self {
            Self::Proposal(pair) => pair.first.round,
            Self::Vote(pair) => pair.first.round,
            Self::TimeoutVote(pair) => pair.first.round,
        }
    }
    /// Return the complete signed artifacts in observation order.
    pub(crate) fn signed_artifact_pair(&self) -> (Vec<u8>, Vec<u8>) {
        match self {
            Self::Proposal(pair) => (pair.first.encode(), pair.second.encode()),
            Self::Vote(pair) => (pair.first.encode(), pair.second.encode()),
            Self::TimeoutVote(pair) => (pair.first.encode(), pair.second.encode()),
        }
    }
    /// Return the unsigned conflicting statements in canonical pair order.
    pub(crate) fn canonical_unsigned_statement_pair(&self) -> (Vec<u8>, Vec<u8>) {
        let (mut first, mut second) = match self {
            Self::Proposal(pair) => (
                pair.first.signature_preimage(),
                pair.second.signature_preimage(),
            ),
            Self::Vote(pair) => (
                pair.first.signature_preimage(),
                pair.second.signature_preimage(),
            ),
            Self::TimeoutVote(pair) => (
                pair.first.signature_preimage(),
                pair.second.signature_preimage(),
            ),
        };
        if second < first {
            core::mem::swap(&mut first, &mut second);
        }
        (first, second)
    }
    /// Project the sealed authenticated pair into the canonical persisted wire form.
    pub(crate) fn to_wire(&self) -> wire::SumeragiV2Equivocation {
        let conflict = match self {
            Self::Proposal(pair) => wire::SumeragiV2Equivocation::Proposal {
                first: pair.first.clone(),
                second: pair.second.clone(),
            },
            Self::Vote(pair) => wire::SumeragiV2Equivocation::PhaseVote {
                first: pair.first.clone(),
                second: pair.second.clone(),
            },
            Self::TimeoutVote(pair) => wire::SumeragiV2Equivocation::TimeoutVote {
                first: pair.first.clone(),
                second: pair.second.clone(),
            },
        };
        super::evidence::canonicalize_v2_conflict(&conflict)
    }
    /// Recheck the sealed pair's structural contract against one frozen height
    /// context.
    ///
    /// Cryptographic authentication is the minting precondition enforced by
    /// [`SumeragiV2Adapter::authenticate`]. This defense-in-depth check cannot
    /// be used as a substitute for that boundary.
    pub(crate) fn validate_structure(&self, context: &wire::HeightContext) -> Result<(), String> {
        let conflict = match self {
            Self::Proposal(pair) => {
                let first = &pair.first;
                let second = &pair.second;
                first
                    .validate(context)
                    .map_err(|error| format!("first proposal is invalid: {error}"))?;
                second
                    .validate(context)
                    .map_err(|error| format!("second proposal is invalid: {error}"))?;
                first.round == second.round
                    && first.proposer == second.proposer
                    && first.signature_preimage() != second.signature_preimage()
            }
            Self::Vote(pair) => {
                let first = &pair.first;
                let second = &pair.second;
                first
                    .validate(context)
                    .map_err(|error| format!("first vote is invalid: {error}"))?;
                second
                    .validate(context)
                    .map_err(|error| format!("second vote is invalid: {error}"))?;
                first.round == second.round
                    && first.phase == second.phase
                    && first.signer == second.signer
                    && first.signature_preimage() != second.signature_preimage()
            }
            Self::TimeoutVote(pair) => {
                let first = &pair.first;
                let second = &pair.second;
                first
                    .validate(context)
                    .map_err(|error| format!("first timeout vote is invalid: {error}"))?;
                second
                    .validate(context)
                    .map_err(|error| format!("second timeout vote is invalid: {error}"))?;
                first.round == second.round
                    && first.signer == second.signer
                    && first.signature_preimage() != second.signature_preimage()
            }
        };
        conflict
            .then_some(())
            .ok_or_else(|| "authenticated equivocation artifacts do not form one conflict".into())
    }
    #[cfg(all(test, feature = "bls"))]
    /// Construct a proposal pair for sibling-module tests only.
    pub(crate) fn proposal_for_test(first: wire::Proposal, second: wire::Proposal) -> Self {
        Self::proposal(first, second)
    }
    #[cfg(test)]
    /// Construct a vote pair for sibling-module tests only.
    pub(crate) fn vote_for_test(first: wire::Vote, second: wire::Vote) -> Self {
        Self::vote(first, second)
    }
    #[cfg(all(test, feature = "bls"))]
    /// Construct a timeout-vote pair for sibling-module tests only.
    pub(crate) fn timeout_vote_for_test(
        first: wire::TimeoutVote,
        second: wire::TimeoutVote,
    ) -> Self {
        Self::timeout_vote(first, second)
    }
    #[cfg(test)]
    /// Consume a vote pair in sibling-module tests only.
    pub(crate) fn into_vote_pair_for_test(self) -> Option<(wire::Vote, wire::Vote)> {
        let Self::Vote(pair) = self else {
            return None;
        };
        Some((pair.first, pair.second))
    }
    #[cfg(test)]
    fn proposal_pair(&self) -> Option<(&wire::Proposal, &wire::Proposal)> {
        let Self::Proposal(pair) = self else {
            return None;
        };
        Some((&pair.first, &pair.second))
    }
    #[cfg(test)]
    fn vote_pair(&self) -> Option<(&wire::Vote, &wire::Vote)> {
        let Self::Vote(pair) = self else {
            return None;
        };
        Some((&pair.first, &pair.second))
    }
}
