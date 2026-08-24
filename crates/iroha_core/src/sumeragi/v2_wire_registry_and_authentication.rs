impl WireRegistry {
    fn new(context: &wire::HeightContext) -> Result<Self, AdapterError> {
        let mut registry = Self {
            wire_context: Some(context.clone()),
            context_id: Some(context.id()),
            peers: context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect(),
            ..Self::default()
        };
        for index in 0..context.roster.len() {
            let index = u32::try_from(index).map_err(|_| wire::ValidationError::RosterTooLarge)?;
            registry.validators.insert(validator_token(index), index);
        }
        Ok(registry)
    }
    fn core_context(
        &mut self,
        context: &wire::HeightContext,
    ) -> Result<reducer::HeightContext, AdapterError> {
        if self.context_id != Some(context.id()) {
            return Err(wire::ValidationError::WrongHeightContext.into());
        }
        let parent_commit = context
            .parent_commit_qc
            .as_ref()
            .map(|certificate| self.register_parent_qc(certificate))
            .transpose()?;
        let roster = context
            .roster
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let index =
                    u32::try_from(index).map_err(|_| wire::ValidationError::RosterTooLarge)?;
                Ok(reducer::Validator::new(
                    validator_token(index),
                    reducer::VotingPower::new(entry.power),
                ))
            })
            .collect::<Result<Vec<_>, AdapterError>>()?;
        let mode = match context.mode {
            wire::ConsensusMode::Permissioned => reducer::VotingMode::Permissioned,
            wire::ConsensusMode::Npos => reducer::VotingMode::Npos,
        };
        let leader_height_seed = Hash::new((context.leader_seed, context.height).encode());
        let context_id = context_id(
            self.context_id
                .expect("registry is constructed with a height context"),
        );
        let network_id = reducer::NetworkId::new(*context.network_id.as_bytes());
        let nexus_hash = reducer::Digest::new(*context.nexus_amx_context_hash.as_ref());
        let execution_policy_hash = reducer::Digest::new(*context.execution_policy_hash.as_ref());
        let da_hash = reducer::Digest::new(Hash::new(context.da_layout.encode()).into());
        let leader_seed = reducer::Digest::new(leader_height_seed.into());
        if context.snapshot_bootstrap.is_some() {
            reducer::HeightContext::new_snapshot_bootstrap(
                context_id,
                network_id,
                context.height,
                context.epoch,
                roster,
                mode,
                nexus_hash,
                execution_policy_hash,
                da_hash,
                leader_seed,
            )
        } else {
            reducer::HeightContext::new(
                context_id,
                network_id,
                context.height,
                parent_commit,
                context.epoch,
                roster,
                mode,
                nexus_hash,
                execution_policy_hash,
                da_hash,
                leader_seed,
            )
        }
        .map_err(Into::into)
    }
    fn validator_id(
        &self,
        index: wire::ValidatorIndex,
    ) -> Result<reducer::ValidatorId, AdapterError> {
        if usize::try_from(index)
            .ok()
            .is_some_and(|index| index < self.peers.len())
        {
            Ok(validator_token(index))
        } else {
            Err(AdapterError::ValidatorIndexOutOfRange(index))
        }
    }
    fn validator_index(
        &self,
        validator: reducer::ValidatorId,
    ) -> Result<wire::ValidatorIndex, AdapterError> {
        self.validators
            .get(&validator)
            .copied()
            .ok_or(AdapterError::UnknownValidator(validator))
    }
    fn peer(&self, validator: reducer::ValidatorId) -> Result<PeerId, AdapterError> {
        let index = self.validator_index(validator)?;
        self.peers
            .get(usize::try_from(index).unwrap_or(usize::MAX))
            .cloned()
            .ok_or(AdapterError::ValidatorIndexOutOfRange(index))
    }
    fn register_subject(
        &mut self,
        subject: wire::BlockSubject,
    ) -> Result<reducer::Subject, AdapterError> {
        let digest = reducer::Subject::new(Hash::new(subject.encode()).into());
        match self.subjects.get(&digest) {
            Some(existing) if *existing != subject => Err(AdapterError::SubjectCollision),
            Some(_) => Ok(digest),
            None => {
                self.subjects.insert(digest, subject);
                Ok(digest)
            }
        }
    }
    fn manifest_conflicts(&self, incoming: &wire::PayloadManifest) -> bool {
        let round = reducer::Round::new(incoming.round.height, incoming.round.view);
        let subject = reducer::Subject::new(Hash::new(incoming.subject.encode()).into());
        self.manifests
            .get(&(round, subject))
            .is_some_and(|registered| registered != incoming)
    }
    fn subject(&self, subject: reducer::Subject) -> Result<wire::BlockSubject, AdapterError> {
        self.subjects
            .get(&subject)
            .copied()
            .ok_or(AdapterError::UnknownSubject(subject))
    }
    fn register_execution_commitment(
        &mut self,
        round: reducer::Round,
        subject: reducer::Subject,
        commitment: wire::ExecutionCommitment,
    ) -> Result<(), AdapterError> {
        commitment.validate()?;
        if self
            .execution_commitments
            .iter()
            .any(|((_, registered_subject), registered)| {
                *registered_subject == subject && *registered != commitment
            })
        {
            return Err(AdapterError::ConflictingExecutionCommitment);
        }
        match self.execution_commitments.get(&(round, subject)) {
            Some(_) => Ok(()),
            None => {
                self.execution_commitments
                    .insert((round, subject), commitment);
                Ok(())
            }
        }
    }
    fn execution_commitment(
        &self,
        round: reducer::Round,
        subject: reducer::Subject,
    ) -> Result<wire::ExecutionCommitment, AdapterError> {
        self.execution_commitments
            .get(&(round, subject))
            .copied()
            .ok_or(AdapterError::MissingExecutionCommitment)
    }
    fn round_to_core(
        &self,
        round: wire::ConsensusRound,
        context: &wire::HeightContext,
    ) -> Result<reducer::Round, AdapterError> {
        if Some(round.context_id) != self.context_id || round.height != context.height {
            return Err(wire::ValidationError::WrongHeightContext.into());
        }
        Ok(reducer::Round::new(round.height, round.view))
    }
    fn round_to_wire(&self, round: reducer::Round) -> wire::ConsensusRound {
        wire::ConsensusRound {
            context_id: self
                .context_id
                .expect("registry is constructed with a height context"),
            height: round.height(),
            view: round.view(),
        }
    }
    fn phase_to_core(phase: wire::GlobalPhase) -> reducer::Phase {
        match phase {
            wire::GlobalPhase::Prepare => reducer::Phase::Prepare,
            wire::GlobalPhase::Commit => reducer::Phase::Commit,
        }
    }
    fn phase_to_wire(phase: reducer::Phase) -> wire::GlobalPhase {
        match phase {
            reducer::Phase::Prepare => wire::GlobalPhase::Prepare,
            reducer::Phase::Commit => wire::GlobalPhase::Commit,
        }
    }
    fn vote_to_core(
        &mut self,
        vote: &wire::Vote,
        context: &wire::HeightContext,
    ) -> Result<reducer::SignedVote, AdapterError> {
        if vote.round != vote.proposal_round {
            return Err(AdapterError::WireValidation(
                wire::ValidationError::InvalidProposalRound,
            ));
        }
        let round = self.round_to_core(vote.round, context)?;
        let proposal_round = self.round_to_core(vote.proposal_round, context)?;
        let subject = self.register_subject(vote.subject)?;
        self.register_execution_commitment(proposal_round, subject, vote.execution_commitment)?;
        let signer = self.validator_id(vote.signer)?;
        Ok(reducer::SignedVote::new(
            reducer::Vote::new_with_proposal_round(
                context_id(vote.round.context_id),
                round,
                proposal_round,
                Self::phase_to_core(vote.phase),
                subject,
                signer,
            ),
            reducer::OpaqueSignature::new(vote.signature.clone()),
        ))
    }
    fn unsigned_vote_to_wire(&self, vote: reducer::Vote) -> Result<wire::Vote, AdapterError> {
        Ok(wire::Vote {
            round: self.round_to_wire(vote.round()),
            proposal_round: self.round_to_wire(vote.proposal_round()),
            phase: Self::phase_to_wire(vote.phase()),
            subject: self.subject(vote.subject())?,
            execution_commitment: self
                .execution_commitment(vote.proposal_round(), vote.subject())?,
            signer: self.validator_index(vote.signer())?,
            signature: Vec::new(),
        })
    }
    fn signed_vote_to_wire(&self, vote: &reducer::SignedVote) -> Result<wire::Vote, AdapterError> {
        let mut wire = self.unsigned_vote_to_wire(vote.vote())?;
        wire.signature = vote.signature().as_bytes().to_vec();
        Ok(wire)
    }
    fn qc_reference_to_core(
        &mut self,
        reference: &wire::QuorumCertificateRef,
    ) -> Result<reducer::CertificateRef, AdapterError> {
        let Some(wire_context_id) = self.context_id else {
            return Err(AdapterError::WireValidation(
                wire::ValidationError::WrongHeightContext,
            ));
        };
        self.qc_reference_to_core_for_context(reference, wire_context_id)
    }
    fn qc_reference_to_core_for_context(
        &mut self,
        reference: &wire::QuorumCertificateRef,
        expected_context_id: wire::HeightContextId,
    ) -> Result<reducer::CertificateRef, AdapterError> {
        if reference.round.context_id != expected_context_id
            || reference.proposal_round.context_id != expected_context_id
            || reference.proposal_round.height != reference.round.height
        {
            return Err(AdapterError::WireValidation(
                wire::ValidationError::WrongHeightContext,
            ));
        }
        if reference.proposal_round != reference.round {
            return Err(AdapterError::WireValidation(
                wire::ValidationError::InvalidProposalRound,
            ));
        }
        let round = reducer::Round::new(reference.round.height, reference.round.view);
        let proposal_round = reducer::Round::new(
            reference.proposal_round.height,
            reference.proposal_round.view,
        );
        let subject = self.register_subject(reference.subject)?;
        self.register_execution_commitment(
            proposal_round,
            subject,
            reference.execution_commitment,
        )?;
        Ok(reducer::CertificateRef::new_with_proposal_round(
            context_id(reference.round.context_id),
            round,
            proposal_round,
            Self::phase_to_core(reference.phase),
            subject,
        ))
    }
    /// Register the predecessor CommitQC frozen into a successor context.
    ///
    /// This is the sole certificate conversion that does not target the active
    /// registry context. The certificate remains bound to the predecessor and
    /// must name the same committed decision as the successor's immutable
    /// parent anchor; ordinary QCs continue through [`Self::qc_reference_to_core`].
    fn register_parent_qc(
        &mut self,
        certificate: &wire::QuorumCertificate,
    ) -> Result<reducer::CertificateRef, AdapterError> {
        let frozen = self
            .wire_context
            .as_ref()
            .and_then(|context| context.parent_commit_qc.as_ref())
            .map(wire::QuorumCertificate::as_ref)
            .ok_or(AdapterError::ParentContextMismatch)?;
        let reference = certificate.as_ref();
        if !reference.same_commit_decision(frozen) {
            return Err(AdapterError::ParentContextMismatch);
        }
        let core = self.qc_reference_to_core_for_context(&reference, frozen.round.context_id)?;
        self.certificates.insert(core, certificate.clone());
        Ok(core)
    }
    fn qc_to_core(
        &mut self,
        certificate: &wire::QuorumCertificate,
        context: &wire::HeightContext,
    ) -> Result<reducer::QuorumCertificate, AdapterError> {
        certificate.validate(context)?;
        let reference = self.qc_reference_to_core(&certificate.as_ref())?;
        let aggregate = aggregate_token(&certificate.aggregate_signature);
        let signatures = certificate
            .signers
            .iter()
            .map(|index| {
                Ok(reducer::SignatureShare::new(
                    self.validator_id(*index)?,
                    aggregate.clone(),
                ))
            })
            .collect::<Result<Vec<_>, AdapterError>>()?;
        let core = reducer::QuorumCertificate::new(reference, signatures);
        self.certificates.insert(reference, certificate.clone());
        Ok(core)
    }
    fn qc_to_wire(
        &mut self,
        certificate: &reducer::QuorumCertificate,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::QuorumCertificate, AdapterError> {
        let signers = certificate
            .signatures()
            .iter()
            .map(|share| self.validator_index(share.signer()))
            .collect::<Result<Vec<_>, _>>()?;
        let aggregate_signature = aggregate_core_shares(certificate.signatures(), aggregator)?;
        let wire = wire::QuorumCertificate {
            round: self.round_to_wire(certificate.round()),
            proposal_round: self.round_to_wire(certificate.proposal_round()),
            phase: Self::phase_to_wire(certificate.phase()),
            subject: self.subject(certificate.subject())?,
            execution_commitment: self
                .execution_commitment(certificate.proposal_round(), certificate.subject())?,
            signers,
            aggregate_signature,
        };
        self.certificates
            .insert(certificate.reference(), wire.clone());
        Ok(wire)
    }
    /// Return whether a reducer QC retains this exact authenticated wire QC.
    ///
    /// Network QCs store the aggregate signature as the same opaque token on
    /// every reducer signature share. Comparing that token as well as the
    /// canonical signer order prevents a different certificate for the same
    /// round, phase, and subject from borrowing an existing deferred owner.
    fn reducer_qc_matches_wire(
        &self,
        queued: &reducer::QuorumCertificate,
        candidate: &wire::QuorumCertificate,
    ) -> bool {
        if self.round_to_wire(queued.round()) != candidate.round
            || self.round_to_wire(queued.proposal_round()) != candidate.proposal_round
            || Self::phase_to_wire(queued.phase()) != candidate.phase
            || !self
                .subject(queued.subject())
                .is_ok_and(|subject| subject == candidate.subject)
            || !self
                .execution_commitment(queued.proposal_round(), queued.subject())
                .is_ok_and(|commitment| commitment == candidate.execution_commitment)
            || queued.signatures().len() != candidate.signers.len()
        {
            return false;
        }
        let aggregate = aggregate_token(&candidate.aggregate_signature);
        queued
            .signatures()
            .iter()
            .zip(&candidate.signers)
            .all(|(share, signer)| {
                self.validator_index(share.signer())
                    .is_ok_and(|index| index == *signer)
                    && share.signature() == &aggregate
            })
    }
    fn timeout_vote_to_core(
        &mut self,
        vote: &wire::TimeoutVote,
        context: &wire::HeightContext,
    ) -> Result<reducer::SignedTimeoutVote, AdapterError> {
        vote.validate(context)?;
        let round = self.round_to_core(vote.round, context)?;
        let highest = vote
            .highest_prepare_qc
            .as_ref()
            .map(|certificate| self.qc_to_core(certificate, context))
            .transpose()?;
        Ok(reducer::SignedTimeoutVote::new(
            reducer::TimeoutVote::new(
                context_id(vote.round.context_id),
                round,
                self.validator_id(vote.signer)?,
                highest,
            ),
            reducer::OpaqueSignature::new(vote.signature.clone()),
        ))
    }
    fn unsigned_timeout_vote_to_wire(
        &mut self,
        vote: &reducer::TimeoutVote,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::TimeoutVote, AdapterError> {
        let highest_prepare_qc = vote
            .highest_prepare()
            .map(|certificate| self.qc_to_wire(certificate, aggregator))
            .transpose()?;
        Ok(wire::TimeoutVote {
            round: self.round_to_wire(vote.round()),
            highest_prepare_qc,
            signer: self.validator_index(vote.signer())?,
            signature: Vec::new(),
        })
    }
    fn signed_timeout_vote_to_wire(
        &mut self,
        vote: &reducer::SignedTimeoutVote,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::TimeoutVote, AdapterError> {
        let mut wire = self.unsigned_timeout_vote_to_wire(&vote.vote(), aggregator)?;
        wire.signature = vote.signature().as_bytes().to_vec();
        Ok(wire)
    }
    fn tc_to_core(
        &mut self,
        certificate: &wire::TimeoutCertificate,
        context: &wire::HeightContext,
    ) -> Result<reducer::TimeoutCertificate, AdapterError> {
        certificate.validate(context)?;
        let round = self.round_to_core(certificate.round, context)?;
        let groups = certificate
            .groups
            .iter()
            .map(|group| {
                let high = group
                    .highest_prepare_qc
                    .as_ref()
                    .map(|certificate| self.qc_to_core(certificate, context))
                    .transpose()?;
                let aggregate = aggregate_token(&group.aggregate_signature);
                let signatures = group
                    .signers
                    .iter()
                    .map(|index| {
                        Ok(reducer::SignatureShare::new(
                            self.validator_id(*index)?,
                            aggregate.clone(),
                        ))
                    })
                    .collect::<Result<Vec<_>, AdapterError>>()?;
                Ok(reducer::TimeoutSignatureGroup::new(high, signatures))
            })
            .collect::<Result<Vec<_>, AdapterError>>()?;
        let core = reducer::TimeoutCertificate::new(
            context_id(certificate.round.context_id),
            round,
            groups,
        );
        Ok(core)
    }
    fn tc_to_wire(
        &mut self,
        certificate: &reducer::TimeoutCertificate,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::TimeoutCertificate, AdapterError> {
        let groups = certificate
            .groups()
            .iter()
            .map(|group| {
                let highest_prepare_qc = group
                    .highest_prepare()
                    .map(|certificate| self.qc_to_wire(certificate, aggregator))
                    .transpose()?;
                let signers = group
                    .signatures()
                    .iter()
                    .map(|share| self.validator_index(share.signer()))
                    .collect::<Result<Vec<_>, AdapterError>>()?;
                let aggregate_signature = aggregate_core_shares(group.signatures(), aggregator)?;
                Ok(wire::TimeoutVoteGroup {
                    highest_prepare_qc,
                    signers,
                    aggregate_signature,
                })
            })
            .collect::<Result<Vec<_>, AdapterError>>()?;
        let wire = wire::TimeoutCertificate {
            round: self.round_to_wire(certificate.round()),
            groups,
        };
        Ok(wire)
    }
    fn manifest_to_core(
        &mut self,
        manifest: &wire::PayloadManifest,
        context: &wire::HeightContext,
    ) -> Result<reducer::PayloadManifest, AdapterError> {
        manifest.validate(context)?;
        let round = self.round_to_core(manifest.round, context)?;
        let subject = self.register_subject(manifest.subject)?;
        let chunk_count = u32::try_from(manifest.chunk_hashes.len())
            .map_err(|_| wire::ValidationError::ChunkCountTooLarge)?;
        if self
            .manifests
            .get(&(round, subject))
            .is_some_and(|existing| existing != manifest)
        {
            return Err(AdapterError::ConflictingManifest);
        }
        self.manifests.insert((round, subject), manifest.clone());
        Ok(reducer::PayloadManifest::new(
            subject,
            reducer::Digest::new(*manifest.subject.payload_hash.as_ref()),
            reducer::Digest::new(*manifest.chunk_root.as_ref()),
            manifest.payload_size_bytes,
            chunk_count,
        ))
    }
    fn manifest_to_wire(
        &self,
        round: reducer::Round,
        manifest: &reducer::PayloadManifest,
    ) -> Result<wire::PayloadManifest, AdapterError> {
        self.manifests
            .get(&(round, manifest.subject()))
            .cloned()
            .ok_or(AdapterError::MissingManifest)
    }
    fn proposal_to_core(
        &mut self,
        proposal: &wire::Proposal,
        context: &wire::HeightContext,
    ) -> Result<reducer::SignedProposal, AdapterError> {
        let core_proposal = self.proposal_body_to_core(proposal, context)?;
        Ok(reducer::SignedProposal::new(
            core_proposal,
            reducer::OpaqueSignature::new(proposal.signature.clone()),
        ))
    }
    fn proposal_body_to_core(
        &mut self,
        proposal: &wire::Proposal,
        context: &wire::HeightContext,
    ) -> Result<reducer::Proposal, AdapterError> {
        let round = self.round_to_core(proposal.round, context)?;
        if proposal.manifest.round != proposal.round
            || proposal.manifest.subject != proposal.subject
        {
            return Err(AdapterError::InvalidProposalJustification);
        }
        let manifest = self.manifest_to_core(&proposal.manifest, context)?;
        let justification = self.justification_to_core(&proposal.justification, context)?;
        let core_proposal = reducer::Proposal::new(
            context_id(proposal.round.context_id),
            round,
            self.validator_id(proposal.proposer)?,
            manifest,
            justification,
        );
        let subject = core_proposal.manifest().subject();
        // Replay may contain the same semantic proposal intent more than once.
        // The reducer identity intentionally reduces full certificates to
        // stable references, while the leader signature covers the exact wire
        // justification. Preserve the first durable envelope so a later
        // same-reference certificate variant cannot retarget re-signing.
        self.proposals
            .entry((round, subject))
            .or_insert_with(|| proposal.clone());
        Ok(core_proposal)
    }
    fn justification_to_core(
        &mut self,
        justification: &wire::ProposalJustification,
        context: &wire::HeightContext,
    ) -> Result<reducer::ProposalJustification, AdapterError> {
        match justification {
            wire::ProposalJustification::ParentCommit(parent) => {
                let reference = parent
                    .certificate
                    .as_ref()
                    .map(|certificate| self.register_parent_qc(certificate))
                    .transpose()?;
                Ok(reducer::ProposalJustification::ParentCommit(reference))
            }
            wire::ProposalJustification::Timeout(timeout) => {
                let selected = timeout.timeout_certificate.highest_prepare_qc();
                if selected != timeout.highest_prepare_qc.as_ref() {
                    return Err(AdapterError::InvalidProposalJustification);
                }
                let certificate = self.tc_to_core(&timeout.timeout_certificate, context)?;
                Ok(reducer::ProposalJustification::Timeout(certificate))
            }
        }
    }
    fn justification_to_wire(
        &mut self,
        justification: &reducer::ProposalJustification,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::ProposalJustification, AdapterError> {
        match justification {
            reducer::ProposalJustification::ParentCommit(reference) => {
                let certificate = reference
                    .map(|reference| {
                        self.certificates
                            .get(&reference)
                            .cloned()
                            .ok_or(AdapterError::MissingCertificate)
                    })
                    .transpose()?;
                Ok(wire::ProposalJustification::ParentCommit(
                    wire::ParentCommitJustification { certificate },
                ))
            }
            reducer::ProposalJustification::Timeout(certificate) => {
                let timeout_certificate = self.tc_to_wire(certificate, aggregator)?;
                let highest_prepare_qc = certificate
                    .highest_prepare()
                    .map(|certificate| self.qc_to_wire(certificate, aggregator))
                    .transpose()?;
                Ok(wire::ProposalJustification::Timeout(
                    wire::TimeoutJustification {
                        timeout_certificate,
                        highest_prepare_qc,
                    },
                ))
            }
        }
    }
    fn unsigned_proposal_to_wire(
        &mut self,
        proposal: &reducer::Proposal,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::Proposal, AdapterError> {
        let key = (proposal.round(), proposal.manifest().subject());
        if let Some(cached) = self.proposals.get(&key) {
            let mut cached = cached.clone();
            cached.signature.clear();
            return Ok(cached);
        }
        let manifest = self.manifest_to_wire(proposal.round(), proposal.manifest())?;
        let wire = wire::Proposal {
            round: self.round_to_wire(proposal.round()),
            proposer: self.validator_index(proposal.proposer())?,
            subject: self.subject(proposal.manifest().subject())?,
            manifest,
            justification: self.justification_to_wire(proposal.justification(), aggregator)?,
            signature: Vec::new(),
        };
        self.proposals.insert(key, wire.clone());
        Ok(wire)
    }
    fn signed_proposal_to_wire(
        &mut self,
        proposal: &reducer::SignedProposal,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::Proposal, AdapterError> {
        let mut wire = self.unsigned_proposal_to_wire(proposal.proposal(), aggregator)?;
        wire.signature = proposal.signature().as_bytes().to_vec();
        self.proposals.insert(
            (
                proposal.proposal().round(),
                proposal.proposal().manifest().subject(),
            ),
            wire.clone(),
        );
        Ok(wire)
    }
    fn message_to_wire(
        &mut self,
        message: reducer::ConsensusMessageV2,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::ConsensusMessageV2, AdapterError> {
        let payload = match message {
            reducer::ConsensusMessageV2::Proposal(proposal) => {
                wire::ConsensusMessageV2Payload::Proposal(
                    self.signed_proposal_to_wire(&proposal, aggregator)?,
                )
            }
            reducer::ConsensusMessageV2::Vote(vote) => {
                wire::ConsensusMessageV2Payload::Vote(self.signed_vote_to_wire(&vote)?)
            }
            reducer::ConsensusMessageV2::QuorumCertificate(certificate) => {
                wire::ConsensusMessageV2Payload::QuorumCertificate(
                    self.qc_to_wire(&certificate, aggregator)?,
                )
            }
            reducer::ConsensusMessageV2::TimeoutVote(vote) => {
                wire::ConsensusMessageV2Payload::TimeoutVote(
                    self.signed_timeout_vote_to_wire(&vote, aggregator)?,
                )
            }
            reducer::ConsensusMessageV2::TimeoutCertificate(certificate) => {
                wire::ConsensusMessageV2Payload::TimeoutCertificate(
                    self.tc_to_wire(&certificate, aggregator)?,
                )
            }
            reducer::ConsensusMessageV2::BodyRequest(_)
            | reducer::ConsensusMessageV2::BodyChunk(_) => {
                return Err(AdapterError::TransportPayload);
            }
        };
        Ok(wire::ConsensusMessageV2::new(payload))
    }
    fn encode_wal_entry(
        &mut self,
        entry: &reducer::WalEntry,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<Vec<u8>, AdapterError> {
        let record = match entry.record() {
            reducer::WalRecord::ProposalIntent(proposal) => {
                WalRecordV2::ProposalIntent(self.unsigned_proposal_to_wire(proposal, aggregator)?)
            }
            reducer::WalRecord::PrepareIntent(vote) => {
                WalRecordV2::PrepareIntent(self.unsigned_vote_to_wire(*vote)?)
            }
            reducer::WalRecord::ObservePrepare(certificate) => {
                WalRecordV2::ObservePrepare(self.qc_to_wire(certificate, aggregator)?)
            }
            reducer::WalRecord::LockAndCommit { prepare, vote } => WalRecordV2::LockAndCommit {
                prepare: self.qc_to_wire(prepare, aggregator)?,
                vote: self.unsigned_vote_to_wire(*vote)?,
            },
            reducer::WalRecord::TimeoutIntent(vote) => {
                WalRecordV2::TimeoutIntent(self.unsigned_timeout_vote_to_wire(vote, aggregator)?)
            }
            reducer::WalRecord::InstallTimeout(certificate) => {
                WalRecordV2::InstallTimeout(self.tc_to_wire(certificate, aggregator)?)
            }
            reducer::WalRecord::Decision(certificate) => {
                WalRecordV2::Decision(self.qc_to_wire(certificate, aggregator)?)
            }
        };
        Ok(WalEnvelopeV2 {
            protocol_version: wire::PROTOCOL_VERSION,
            persistence_id: entry.id().get(),
            record,
        }
        .encode())
    }
    fn decode_wal_entry(
        &mut self,
        frame: &RecoveredRecord,
        parent_verification: Option<&ParentVerificationContext>,
        proofs_of_possession: &[Vec<u8>],
    ) -> Result<reducer::WalEntry, AdapterError> {
        let mut input = frame.payload();
        let envelope = WalEnvelopeV2::decode(&mut input)
            .map_err(|error| AdapterError::WalDecode(error.to_string()))?;
        if !input.is_empty() {
            return Err(AdapterError::WalDecode(
                "trailing bytes after complete record".to_owned(),
            ));
        }
        if envelope.protocol_version != wire::PROTOCOL_VERSION {
            return Err(AdapterError::WalDecode(format!(
                "unsupported protocol version {}",
                envelope.protocol_version
            )));
        }
        if frame.sequence().checked_add(1) != Some(envelope.persistence_id) {
            return Err(AdapterError::WalFrameIdentityMismatch {
                frame_sequence: frame.sequence(),
                persistence_id: envelope.persistence_id,
                frame_hash: frame.frame_hash(),
            });
        }
        let wire_context = self
            .wire_context
            .as_ref()
            .expect("registry is constructed with a height context");
        verify_wal_record_authority(
            wire_context,
            parent_verification,
            &envelope.record,
            proofs_of_possession,
        )?;
        // The registry is already bound to this immutable context identifier;
        // reducer replay performs the remaining height and safety checks.
        let wire_context_id = self
            .context_id
            .expect("registry is constructed with a height context");
        let context_height = match &envelope.record {
            WalRecordV2::ProposalIntent(proposal) => proposal.round.height,
            WalRecordV2::PrepareIntent(vote) | WalRecordV2::LockAndCommit { vote, .. } => {
                vote.round.height
            }
            WalRecordV2::ObservePrepare(certificate) | WalRecordV2::Decision(certificate) => {
                certificate.round.height
            }
            WalRecordV2::TimeoutIntent(vote) => vote.round.height,
            WalRecordV2::InstallTimeout(certificate) => certificate.round.height,
        };
        if context_height == 0 {
            return Err(AdapterError::WalDecode("zero consensus height".to_owned()));
        }
        let round = |wire_round: wire::ConsensusRound| {
            if wire_round.context_id != wire_context_id || wire_round.height != context_height {
                Err(AdapterError::WireValidation(
                    wire::ValidationError::WrongHeightContext,
                ))
            } else {
                Ok(reducer::Round::new(wire_round.height, wire_round.view))
            }
        };
        let record = match envelope.record {
            WalRecordV2::ProposalIntent(proposal) => {
                let context = self
                    .wire_context
                    .clone()
                    .expect("registry is constructed with a height context");
                reducer::WalRecord::ProposalIntent(self.proposal_body_to_core(&proposal, &context)?)
            }
            WalRecordV2::PrepareIntent(vote) => {
                vote.execution_commitment.validate()?;
                let core_round = round(vote.round)?;
                let proposal_round = round(vote.proposal_round)?;
                let subject = self.register_subject(vote.subject)?;
                self.register_execution_commitment(
                    proposal_round,
                    subject,
                    vote.execution_commitment,
                )?;
                reducer::WalRecord::PrepareIntent(reducer::Vote::new_with_proposal_round(
                    context_id(wire_context_id),
                    core_round,
                    proposal_round,
                    Self::phase_to_core(vote.phase),
                    subject,
                    self.validator_id(vote.signer)?,
                ))
            }
            WalRecordV2::ObservePrepare(certificate) => {
                reducer::WalRecord::ObservePrepare(self.qc_to_core_unchecked(&certificate)?)
            }
            WalRecordV2::LockAndCommit { prepare, vote } => {
                vote.execution_commitment.validate()?;
                let core_round = round(vote.round)?;
                let proposal_round = round(vote.proposal_round)?;
                let subject = self.register_subject(vote.subject)?;
                self.register_execution_commitment(
                    proposal_round,
                    subject,
                    vote.execution_commitment,
                )?;
                reducer::WalRecord::LockAndCommit {
                    prepare: self.qc_to_core_unchecked(&prepare)?,
                    vote: reducer::Vote::new_with_proposal_round(
                        context_id(wire_context_id),
                        core_round,
                        proposal_round,
                        Self::phase_to_core(vote.phase),
                        subject,
                        self.validator_id(vote.signer)?,
                    ),
                }
            }
            WalRecordV2::TimeoutIntent(vote) => {
                let core_round = round(vote.round)?;
                let high = vote
                    .highest_prepare_qc
                    .as_ref()
                    .map(|certificate| self.qc_to_core_unchecked(certificate))
                    .transpose()?;
                reducer::WalRecord::TimeoutIntent(reducer::TimeoutVote::new(
                    context_id(wire_context_id),
                    core_round,
                    self.validator_id(vote.signer)?,
                    high,
                ))
            }
            WalRecordV2::InstallTimeout(certificate) => {
                reducer::WalRecord::InstallTimeout(self.tc_to_core_unchecked(&certificate)?)
            }
            WalRecordV2::Decision(certificate) => {
                reducer::WalRecord::Decision(self.qc_to_core_unchecked(&certificate)?)
            }
        };
        Ok(reducer::WalEntry::new(
            reducer::PersistenceId::new(envelope.persistence_id),
            record,
        ))
    }
    fn qc_to_core_unchecked(
        &mut self,
        certificate: &wire::QuorumCertificate,
    ) -> Result<reducer::QuorumCertificate, AdapterError> {
        let reference = self.qc_reference_to_core(&certificate.as_ref())?;
        let aggregate = aggregate_token(&certificate.aggregate_signature);
        let signatures = certificate
            .signers
            .iter()
            .map(|index| {
                Ok(reducer::SignatureShare::new(
                    self.validator_id(*index)?,
                    aggregate.clone(),
                ))
            })
            .collect::<Result<Vec<_>, AdapterError>>()?;
        let core = reducer::QuorumCertificate::new(reference, signatures);
        self.certificates.insert(reference, certificate.clone());
        Ok(core)
    }
    fn tc_to_core_unchecked(
        &mut self,
        certificate: &wire::TimeoutCertificate,
    ) -> Result<reducer::TimeoutCertificate, AdapterError> {
        let round = reducer::Round::new(certificate.round.height, certificate.round.view);
        let groups = certificate
            .groups
            .iter()
            .map(|group| {
                let highest = group
                    .highest_prepare_qc
                    .as_ref()
                    .map(|certificate| self.qc_to_core_unchecked(certificate))
                    .transpose()?;
                let aggregate = aggregate_token(&group.aggregate_signature);
                let signatures = group
                    .signers
                    .iter()
                    .map(|index| {
                        Ok(reducer::SignatureShare::new(
                            self.validator_id(*index)?,
                            aggregate.clone(),
                        ))
                    })
                    .collect::<Result<Vec<_>, AdapterError>>()?;
                Ok(reducer::TimeoutSignatureGroup::new(highest, signatures))
            })
            .collect::<Result<Vec<_>, AdapterError>>()?;
        Ok(reducer::TimeoutCertificate::new(
            context_id(certificate.round.context_id),
            round,
            groups,
        ))
    }
}
fn verify_proposal_justification_authority(
    context: &wire::HeightContext,
    parent_verification: Option<&ParentVerificationContext>,
    justification: &wire::ProposalJustification,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    match justification {
        wire::ProposalJustification::ParentCommit(parent) => {
            match (&parent.certificate, parent_verification) {
                (None, None) if context.height == 1 || context.snapshot_bootstrap.is_some() => {
                    Ok(())
                }
                (Some(certificate), Some(parent_verification)) => verify_quorum_certificate(
                    &parent_verification.context,
                    certificate,
                    &parent_verification.proofs_of_possession,
                ),
                (None, None) | (None, Some(_)) | (Some(_), None) => {
                    Err(AdapterError::ParentContextMismatch)
                }
            }
        }
        wire::ProposalJustification::Timeout(timeout) => {
            verify_timeout_certificate(
                context,
                &timeout.timeout_certificate,
                proofs_of_possession,
            )?;
            if let Some(highest) = &timeout.highest_prepare_qc {
                verify_quorum_certificate(context, highest, proofs_of_possession)?;
            }
            Ok(())
        }
    }
}
/// Reauthenticate every external authority proof embedded in one durable WAL
/// record before reducer replay may consume it.
///
/// WAL frame checksums and identity hashes detect corruption and accidental
/// key/configuration drift; they are not signatures by a remote quorum. Local
/// unsigned intents therefore remain inside the trusted-storage boundary, but
/// every carried QC and TC must still verify under its frozen roster. Requiring
/// empty intent-signature fields also prevents ignored wire bytes from aliasing
/// the same durable core intent.
fn verify_wal_record_authority(
    context: &wire::HeightContext,
    parent_verification: Option<&ParentVerificationContext>,
    record: &WalRecordV2,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    let require_unsigned = |signature: &[u8], kind: &str| {
        if signature.is_empty() {
            Ok(())
        } else {
            Err(AdapterError::WalDecode(format!(
                "{kind} must not carry signature bytes"
            )))
        }
    };
    match record {
        WalRecordV2::ProposalIntent(proposal) => {
            require_unsigned(&proposal.signature, "ProposalIntent")?;
            verify_proposal_justification_authority(
                context,
                parent_verification,
                &proposal.justification,
                proofs_of_possession,
            )
        }
        WalRecordV2::PrepareIntent(vote) => require_unsigned(&vote.signature, "PrepareIntent"),
        WalRecordV2::ObservePrepare(certificate) => {
            verify_quorum_certificate(context, certificate, proofs_of_possession)
        }
        WalRecordV2::LockAndCommit { prepare, vote } => {
            require_unsigned(&vote.signature, "LockAndCommit vote")?;
            verify_quorum_certificate(context, prepare, proofs_of_possession)
        }
        WalRecordV2::TimeoutIntent(vote) => {
            require_unsigned(&vote.signature, "TimeoutIntent")?;
            if let Some(highest) = &vote.highest_prepare_qc {
                verify_quorum_certificate(context, highest, proofs_of_possession)?;
            }
            Ok(())
        }
        WalRecordV2::InstallTimeout(certificate) => {
            verify_timeout_certificate(context, certificate, proofs_of_possession)
        }
        WalRecordV2::Decision(certificate) => {
            verify_quorum_certificate(context, certificate, proofs_of_possession)
        }
    }
}
fn verify_authenticated_message(
    context: &wire::HeightContext,
    parent_verification: Option<&ParentVerificationContext>,
    message: &wire::ConsensusMessageV2,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    message.validate_version()?;
    // `SumeragiV2Adapter` can only be built from `VerifiedHeightContext`,
    // which has already validated the immutable context, every BLS key, and
    // the complete aligned PoP vector. Do not rescan the boundary snapshot for
    // every hostile ingress message.
    match &message.payload {
        wire::ConsensusMessageV2Payload::Proposal(proposal) => {
            proposal.validate(context)?;
            verify_individual_signature(
                context,
                proposal.proposer,
                &proposal.signature,
                &proposal.signature_preimage(),
            )?;
            verify_proposal_justification_authority(
                context,
                parent_verification,
                &proposal.justification,
                proofs_of_possession,
            )
        }
        wire::ConsensusMessageV2Payload::Vote(vote) => {
            vote.validate(context)?;
            verify_individual_signature(
                context,
                vote.signer,
                &vote.signature,
                &vote.signature_preimage(),
            )
        }
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
            verify_quorum_certificate(context, certificate, proofs_of_possession)
        }
        wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
            vote.validate(context)?;
            if let Some(highest) = &vote.highest_prepare_qc {
                verify_quorum_certificate(context, highest, proofs_of_possession)?;
            }
            verify_individual_signature(
                context,
                vote.signer,
                &vote.signature,
                &vote.signature_preimage(),
            )
        }
        wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
            verify_timeout_certificate(context, certificate, proofs_of_possession)
        }
        wire::ConsensusMessageV2Payload::PayloadManifest(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | wire::ConsensusMessageV2Payload::VrfCommit(_)
        | wire::ConsensusMessageV2Payload::VrfReveal(_)
        | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => {
            Err(AdapterError::TransportPayload)
        }
    }
}
fn verify_roster_proofs(
    context: &wire::HeightContext,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    wire::finality::verify_validator_roster_pops(context, proofs_of_possession).map_err(|error| {
        match error {
            wire::finality::V2QuorumCertificateVerificationError::ProofOfPossessionCount {
                expected,
                actual,
            } => AdapterError::ProofOfPossessionCount { expected, actual },
            other => AdapterError::Cryptography(other.to_string()),
        }
    })
}
fn verify_next_epoch_snapshot_proofs(context: &wire::HeightContext) -> Result<(), AdapterError> {
    let Some(snapshot) = &context.next_epoch_snapshot else {
        return Ok(());
    };
    wire::finality::verify_validator_power_roster_pops(
        &snapshot.roster,
        &snapshot.validator_set_pops,
    )
    .map_err(|error| AdapterError::Cryptography(error.to_string()))
}
fn verify_individual_signature(
    context: &wire::HeightContext,
    signer: wire::ValidatorIndex,
    signature: &[u8],
    preimage: &[u8],
) -> Result<(), AdapterError> {
    let index = usize::try_from(signer)
        .ok()
        .filter(|index| *index < context.roster.len())
        .ok_or(AdapterError::ValidatorIndexOutOfRange(signer))?;
    let signature = Signature::try_from_bytes(signature)
        .map_err(|error| AdapterError::Cryptography(error.to_string()))?;
    signature
        .verify(context.roster[index].validator.public_key(), preimage)
        .map_err(|error| AdapterError::Cryptography(error.to_string()))
}
fn verify_quorum_certificate(
    context: &wire::HeightContext,
    certificate: &wire::QuorumCertificate,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    wire::finality::verify_quorum_certificate_with_validator_pops(
        context,
        certificate,
        proofs_of_possession,
    )
    .map_err(|error| match error {
        wire::finality::V2QuorumCertificateVerificationError::InvalidCertificate(error) => {
            AdapterError::WireValidation(error)
        }
        wire::finality::V2QuorumCertificateVerificationError::ProofOfPossessionCount {
            expected,
            actual,
        } => AdapterError::ProofOfPossessionCount { expected, actual },
        other => AdapterError::Cryptography(other.to_string()),
    })
}
/// Verify one certificate against immutable historical context authority.
///
/// This deliberately reuses the exact production roster-PoP and aggregate
/// verifier used by live reducer ingress; block sync does not maintain a
/// second certificate-validation implementation.
pub(crate) fn verify_historical_quorum_certificate(
    context: &wire::HeightContext,
    proofs_of_possession: &[Vec<u8>],
    certificate: &wire::QuorumCertificate,
) -> Result<(), AdapterError> {
    context.validate()?;
    verify_roster_proofs(context, proofs_of_possession)?;
    verify_quorum_certificate(context, certificate, proofs_of_possession)
}
