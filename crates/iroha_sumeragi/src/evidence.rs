//! Independent attribution of native signed evidence against authenticated chain inputs.
//!
//! The application supplies original authenticated configurations, the parent tip and the
//! complete demotion window. Decoding an evidence record supplies none of that authority.
//! A verified report identifies accountable signers; committing a monetary penalty remains
//! an application decision. Conflicting Commit certificates across views establish a safety
//! violation without attributing their intersection (§3.6).

use crate::{
    api::CommittedTip,
    crypto::{
        AttestationVerifier, CertError, Crypto, verify_proposal_signature, verify_qc,
        verify_qc_signatures, verify_tc, verify_timeout, verify_vote,
    },
    message::{BlockHeader, Defect, Evidence, Proposal, VoteKind},
    topology::{Topology, committee_permutation, demoted_set, demotion_window},
    types::{Bitmap, Hash32, HeightConfig},
};

/// Independently authenticated inputs for the subject height of one native evidence report.
///
/// These borrowed inputs must come from the original committed history owner. An embedded
/// evidence context or a node-local record is not an authentication source.
pub struct EvidenceContext<'a> {
    /// Application-derived exact native instance, including its network and lane identity.
    pub instance: Hash32,
    /// Height whose authority is being supplied.
    pub height: u64,
    /// Complete authenticated epoch, generation, committee and lagged chain parameters.
    pub config: &'a HeightConfig,
    /// Genesis height of this exact instance.
    pub genesis_height: u64,
    /// Parent tip authenticated by the same original chain history.
    pub parent: &'a CommittedTip,
    /// Parent's exact configuration, required after the genesis parent.
    pub parent_config: Option<&'a HeightConfig>,
    /// Demotion window fixed by signed genesis for this instance.
    pub demotion_window: u64,
    /// Every committed header of the exact demotion interval, in ascending height order.
    pub demotion_headers: &'a [BlockHeader],
}

/// Verified attribution, without conferring authorization to change stake or balances.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceAttribution {
    /// Subject height, checked against the independently authenticated configuration.
    height: u64,
    /// Exact committee bitmap of signers directly proven accountable by this report.
    offenders: Bitmap,
    /// The report proves two conflicting Commit certificates and requires a safety halt.
    safety_violation: bool,
}

impl EvidenceAttribution {
    /// Subject height under the independently supplied native authority.
    pub const fn height(&self) -> u64 {
        self.height
    }
    /// Exact directly accountable signer set; empty for cross-view safety violations.
    pub fn offenders(&self) -> &Bitmap {
        &self.offenders
    }
    /// Whether two conflicting Commit certificates establish a safety violation.
    pub const fn safety_violation(&self) -> bool {
        self.safety_violation
    }
}

/// A report or its independent authority is insufficient for attribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceError {
    /// Missing or inconsistent authenticated height, parent or exact committee geometry.
    Context,
    /// The original complete demotion window was not supplied.
    DemotionHistory,
    /// Signed artifacts do not describe the same native subject or do not conflict.
    NotConflicting,
    /// The declared signed-content defect does not reproduce from committed chain inputs.
    DefectMismatch,
    /// A native signature or certificate did not verify.
    Signature(CertError),
}

impl core::fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Context => f.write_str("invalid native evidence authority context"),
            Self::DemotionHistory => f.write_str("incomplete native evidence demotion history"),
            Self::NotConflicting => f.write_str("native evidence does not prove a signed conflict"),
            Self::DefectMismatch => f.write_str("native proposal defect does not reproduce"),
            Self::Signature(error) => write!(f, "invalid native evidence signature: {error:?}"),
        }
    }
}
impl std::error::Error for EvidenceError {}
impl From<CertError> for EvidenceError {
    fn from(error: CertError) -> Self {
        Self::Signature(error)
    }
}

impl EvidenceContext<'_> {
    fn validate(&self) -> Result<(), EvidenceError> {
        let committee = &self.config.committee;
        if self.height <= self.genesis_height
            || self.parent.height.checked_add(1) != Some(self.height)
            || !self.config.epoch.contains(self.height)
            || committee.n() < 4
            || committee.n() != committee.f() * 3 + 1
            || committee.q() != committee.f() * 2 + 1
        {
            return Err(EvidenceError::Context);
        }
        if self.parent.height > self.genesis_height {
            let parent = self.parent_config.ok_or(EvidenceError::Context)?;
            if !parent.epoch.contains(self.parent.height) {
                return Err(EvidenceError::Context);
            }
        }
        Ok(())
    }

    fn topology(&self, crypto: &dyn Crypto) -> Result<Topology, EvidenceError> {
        let expected = demotion_window(self.height, self.genesis_height, self.demotion_window);
        match expected {
            None if !self.demotion_headers.is_empty() => {
                return Err(EvidenceError::DemotionHistory);
            }
            Some((first, last)) => {
                let count = last.checked_sub(first).and_then(|n| n.checked_add(1));
                if count != u64::try_from(self.demotion_headers.len()).ok()
                    || self
                        .demotion_headers
                        .iter()
                        .enumerate()
                        .any(|(index, header)| {
                            u64::try_from(index)
                                .ok()
                                .and_then(|offset| first.checked_add(offset))
                                != Some(header.height)
                                || header.instance != self.instance
                        })
                {
                    return Err(EvidenceError::DemotionHistory);
                }
            }
            None => {}
        }
        let demoted = demoted_set(
            &self.config.committee,
            self.height,
            self.genesis_height,
            self.demotion_window,
            self.demotion_headers,
        );
        Topology::from_parts(
            committee_permutation(
                crypto,
                &self.instance,
                &self.config.epoch,
                &self.config.committee,
            ),
            &demoted,
            self.height,
        )
        .ok_or(EvidenceError::Context)
    }
}

/// Reverify and attribute all five native evidence variants using original signed artifacts.
///
/// Same-view certificate conflicts attribute only the exact signer intersection. Different-view
/// conflicting Commit certificates return an empty offender set and a safety violation. A signed
/// proposal with poison execution, an unsigned payload substitution, or a relay's altered
/// signature cannot implicate an honest validator.
///
/// # Errors
/// Rejects incomplete authority, wrong epoch/generation context or instance, absent conflicts,
/// incorrect defect labels, and invalid signatures. Caller-owned decode/admission byte limits
/// must be enforced before constructing the input graph.
pub fn verify_evidence(
    crypto: &dyn Crypto,
    attestation: &dyn AttestationVerifier,
    context: &EvidenceContext<'_>,
    evidence: &Evidence,
) -> Result<EvidenceAttribution, EvidenceError> {
    context.validate()?;
    let config = context.config;
    let epoch = &config.epoch.id;
    let committee = &config.committee;
    let mut offenders = Vec::new();
    let mut safety_violation = false;
    match evidence {
        Evidence::ProposalEquivocation(first, second) => {
            if first.height != context.height
                || second.height != context.height
                || first.view != second.view
            {
                return Err(EvidenceError::NotConflicting);
            }
            let topology = context.topology(crypto)?;
            let leader = topology.leader(first.view);
            let first_value = verify_proposal_signature(
                crypto,
                &context.instance,
                epoch,
                committee,
                leader,
                first,
            )?;
            let second_value = verify_proposal_signature(
                crypto,
                &context.instance,
                epoch,
                committee,
                leader,
                second,
            )?;
            if first_value == second_value {
                return Err(EvidenceError::NotConflicting);
            }
            offenders.push(leader);
        }
        Evidence::VoteEquivocation(first, second) => {
            if first.height != context.height
                || second.height != context.height
                || first.view != second.view
                || first.kind != second.kind
                || first.signer != second.signer
                || (first.block_hash, first.result, first.attest)
                    == (second.block_hash, second.result, second.attest)
            {
                return Err(EvidenceError::NotConflicting);
            }
            // A signature on each distinct vote value proves equivocation independently
            // of unsigned attestation attachments; those attachments cannot create a conflict.
            verify_vote(crypto, &context.instance, epoch, committee, first)?;
            verify_vote(crypto, &context.instance, epoch, committee, second)?;
            offenders.push(first.signer);
        }
        Evidence::TimeoutEquivocation(first, second) => {
            if first.height != context.height
                || second.height != context.height
                || first.view != second.view
                || first.signer != second.signer
                || first.hq() == second.hq()
            {
                return Err(EvidenceError::NotConflicting);
            }
            verify_timeout(crypto, &context.instance, epoch, committee, first)?;
            verify_timeout(crypto, &context.instance, epoch, committee, second)?;
            offenders.push(first.signer);
        }
        Evidence::InvalidProposal { proposal, defect } => {
            if proposal.height != context.height {
                return Err(EvidenceError::Context);
            }
            let topology = context.topology(crypto)?;
            let leader = topology.leader(proposal.view);
            let (hash, _) = verify_proposal_signature(
                crypto,
                &context.instance,
                epoch,
                committee,
                leader,
                proposal,
            )?;
            if proposal_defect(crypto, attestation, context, &topology, proposal, hash)
                != Some(*defect)
            {
                return Err(EvidenceError::DefectMismatch);
            }
            offenders.push(leader);
        }
        Evidence::ConflictingCertificates(first, second) => {
            if first.kind != VoteKind::Commit
                || second.kind != VoteKind::Commit
                || first.height != context.height
                || second.height != context.height
                || first.value() == second.value()
            {
                return Err(EvidenceError::NotConflicting);
            }
            // As in the native safety monitor (§7.6), exact Commit signatures alone
            // establish this safety violation; an absent Pasta attachment does not erase it.
            verify_qc_signatures(crypto, &context.instance, epoch, committee, first)?;
            verify_qc_signatures(crypto, &context.instance, epoch, committee, second)?;
            safety_violation = true;
            if first.view == second.view {
                offenders.extend(
                    first
                        .signers
                        .ones()
                        .filter(|index| second.signers.ones().any(|other| other == *index)),
                );
            }
        }
    }
    Ok(EvidenceAttribution {
        height: context.height,
        offenders: Bitmap::from_indices(committee.n(), offenders).ok_or(EvidenceError::Context)?,
        safety_violation,
    })
}

fn proposal_defect(
    crypto: &dyn Crypto,
    attestation: &dyn AttestationVerifier,
    context: &EvidenceContext<'_>,
    topology: &Topology,
    proposal: &Proposal,
    block_hash: Hash32,
) -> Option<Defect> {
    let config = context.config;
    match (proposal.view, &proposal.justify) {
        (0, Some(_)) => return Some(Defect::UnexpectedJustify),
        (0, None) => {}
        (_, None) => return Some(Defect::MissingJustify),
        (view, Some(tc)) => {
            if tc.height != context.height
                || Some(tc.view) != view.checked_sub(1)
                || verify_tc(
                    crypto,
                    &context.instance,
                    &config.epoch.id,
                    &config.committee,
                    tc,
                )
                .is_err()
            {
                return Some(Defect::InvalidJustify);
            }
        }
    }
    if context.parent.height == context.genesis_height {
        if proposal.parent_qc.is_some() {
            return Some(Defect::UnexpectedParentQc);
        }
    } else {
        let Some(qc) = &proposal.parent_qc else {
            return Some(Defect::MissingParentQc);
        };
        let parent_config = context.parent_config?;
        if qc.kind != VoteKind::Commit
            || qc.height != context.parent.height
            || qc.value() != (context.parent.block_hash, context.parent.result)
            || verify_qc(
                crypto,
                attestation,
                &context.instance,
                &parent_config.epoch.id,
                &parent_config.committee,
                qc,
            )
            .is_err()
        {
            return Some(Defect::InvalidParentQc);
        }
    }
    let header = &proposal.header;
    for (bad, defect) in [
        (header.instance != context.instance, Defect::HeaderInstance),
        (header.height != context.height, Defect::HeaderHeight),
        (header.epoch != config.epoch.id, Defect::EpochContext),
        (
            context.height == config.epoch.last_height && !header.attest,
            Defect::BoundaryAttestation,
        ),
        (
            header.parent_hash != context.parent.block_hash,
            Defect::ParentHash,
        ),
        (
            header.parent_result != context.parent.result,
            Defect::ParentResult,
        ),
        (
            header.payload_len > config.params.max_block_bytes,
            Defect::PayloadTooLarge,
        ),
        (header.payload_len == 0, Defect::EmptyPayload),
    ] {
        if bad {
            return Some(defect);
        }
    }
    if let Some(qc) = proposal
        .justify
        .as_ref()
        .and_then(|tc| tc.high_pqc.as_ref())
    {
        return (block_hash != qc.block_hash).then_some(Defect::TcRule);
    }
    for (bad, defect) in [
        (header.origin_view != proposal.view, Defect::OriginView),
        (
            header.proposer != topology.leader(proposal.view),
            Defect::Proposer,
        ),
        (
            header.skipped_leaders
                != topology.skipped_leader_keys(&config.committee, proposal.view),
            Defect::SkippedLeaders,
        ),
    ] {
        if bad {
            return Some(defect);
        }
    }
    None
}

#[cfg(test)]
mod tests;
