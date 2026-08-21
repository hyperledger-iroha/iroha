impl VerifiedHeightContext {
    /// Verify one complete consensus envelope against this frozen height.
    ///
    /// Lifecycle cold recovery uses this fixed oracle after LedgerV1 has decoded a
    /// signed Broadcast but before the original recovered WAL Sign may reconstruct
    /// a live carrier. This covers the individual signature and every embedded
    /// proposal/timeout justification under the exact retained predecessor and
    /// proof-of-possession authority.
    pub(in crate::sumeragi) fn verify_consensus_message(
        &self,
        message: &wire::ConsensusMessageV2,
    ) -> Result<(), AdapterError> {
        verify_authenticated_message(
            &self.context,
            self.parent_verification.as_ref(),
            message,
            &self.proofs_of_possession,
        )
    }

    /// Re-authenticate one canonical persisted equivocation pair after restart.
    ///
    /// Ledger replay bytes are inert until both signed statements independently
    /// pass the same frozen-roster oracle as live ingress. Only then is the private
    /// process-local evidence carrier reminted. Canonical ordering is checked again
    /// so a reordered or merely structural pair cannot acquire report authority
    /// during cold open.
    pub(in crate::sumeragi) fn authenticate_recovered_equivocation(
        &self,
        persisted: &wire::SumeragiV2Equivocation,
    ) -> Result<AdapterEquivocationEvidence, AdapterError> {
        if crate::sumeragi::evidence::canonicalize_v2_conflict(persisted) != *persisted {
            return Err(AdapterError::EquivocationArtifactMismatch);
        }
        let evidence = match persisted {
            wire::SumeragiV2Equivocation::Proposal { first, second } => {
                self.verify_consensus_message(&wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Proposal(first.clone()),
                ))?;
                self.verify_consensus_message(&wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Proposal(second.clone()),
                ))?;
                AdapterEquivocationEvidence::proposal(first.clone(), second.clone())
            }
            wire::SumeragiV2Equivocation::PhaseVote { first, second } => {
                self.verify_consensus_message(&wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Vote(first.clone()),
                ))?;
                self.verify_consensus_message(&wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Vote(second.clone()),
                ))?;
                AdapterEquivocationEvidence::vote(first.clone(), second.clone())
            }
            wire::SumeragiV2Equivocation::TimeoutVote { first, second } => {
                self.verify_consensus_message(&wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::TimeoutVote(first.clone()),
                ))?;
                self.verify_consensus_message(&wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::TimeoutVote(second.clone()),
                ))?;
                AdapterEquivocationEvidence::timeout_vote(first.clone(), second.clone())
            }
        };
        if evidence.to_wire() != *persisted || evidence.validate_structure(&self.context).is_err() {
            return Err(AdapterError::EquivocationArtifactMismatch);
        }
        Ok(evidence)
    }
}
