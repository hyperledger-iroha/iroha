#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct BodyFrameBindingV1 {
    context: [u8; 32],
    round_height: u64,
    round_view: u64,
    subject: [u8; 32],
    manifest: [u8; 32],
    frame: [u8; 32],
}
impl BodyFrameBindingV1 {
    const fn durable_reference(self) -> DurableBodyFrameReference {
        DurableBodyFrameReference::new(
            LifecycleDigest::new(self.context),
            LifecycleRound::new(self.round_height, self.round_view),
            LifecycleDigest::new(self.subject),
            LifecycleDigest::new(self.manifest),
            LifecycleDigest::new(self.frame),
        )
    }
    fn matches_origin(
        self,
        context: LifecycleContext,
        proposal_round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> bool {
        self.context == *context.id().as_bytes()
            && self.round_height == proposal_round.height
            && self.round_view == proposal_round.view
            && self.subject == *block_subject(subject).as_bytes()
    }
}
#[derive(Clone, Copy)]
struct ReplayShape {
    key: LifecycleKey,
    work_class: LifecycleWorkClass,
    stage_kind: LifecycleStageKind,
}
impl ReplayShape {
    const fn new(
        key: LifecycleKey,
        work_class: LifecycleWorkClass,
        stage_kind: LifecycleStageKind,
    ) -> Self {
        Self {
            key,
            work_class,
            stage_kind,
        }
    }
}
fn project_broadcast(
    context: LifecycleContext,
    message: &wire::ConsensusMessageV2,
    requested_stage: LifecycleStageKind,
    payload: &ReplayPayloadBindingV1,
) -> Result<ReplayShape, ReplayAuthorityValidationError> {
    if message.validate_version().is_err() || !payload.is_none() {
        return Err(ReplayAuthorityValidationError::InvalidSource);
    }
    let shape = match &message.payload {
        wire::ConsensusMessageV2Payload::Proposal(proposal) => {
            if !proposal_shape(context, proposal, true) {
                return Err(ReplayAuthorityValidationError::InvalidSource);
            }
            ReplayShape::new(
                lifecycle_key(
                    context,
                    proposal.round,
                    Some(proposal.round),
                    Some(block_subject(proposal.subject)),
                    LifecyclePhase::BroadcastProposal,
                    None,
                ),
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastProposal,
            )
        }
        wire::ConsensusMessageV2Payload::Vote(vote) => {
            if !vote_shape(context, vote, true) {
                return Err(ReplayAuthorityValidationError::InvalidSource);
            }
            let (phase, stage_kind) = match vote.phase {
                wire::GlobalPhase::Prepare => (
                    LifecyclePhase::BroadcastPrepareVote,
                    LifecycleStageKind::BroadcastPrepareVote,
                ),
                wire::GlobalPhase::Commit => (
                    LifecyclePhase::BroadcastCommitVote,
                    LifecycleStageKind::BroadcastCommitVote,
                ),
            };
            ReplayShape::new(
                lifecycle_key(
                    context,
                    vote.round,
                    Some(vote.proposal_round),
                    Some(block_subject(vote.subject)),
                    phase,
                    Some(execution_commitment(vote.execution_commitment)),
                ),
                LifecycleWorkClass::Broadcast,
                stage_kind,
            )
        }
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
            if !qc_shape(context, certificate) {
                return Err(ReplayAuthorityValidationError::InvalidSource);
            }
            let (phase, stage_kind) = match certificate.phase {
                wire::GlobalPhase::Prepare => (
                    LifecyclePhase::BroadcastPrepareQc,
                    LifecycleStageKind::BroadcastPrepareQc,
                ),
                wire::GlobalPhase::Commit => (
                    LifecyclePhase::BroadcastCommitQc,
                    LifecycleStageKind::BroadcastCommitQc,
                ),
            };
            ReplayShape::new(
                lifecycle_key(
                    context,
                    certificate.round,
                    Some(certificate.proposal_round),
                    Some(block_subject(certificate.subject)),
                    phase,
                    Some(execution_commitment(certificate.execution_commitment)),
                ),
                LifecycleWorkClass::Broadcast,
                stage_kind,
            )
        }
        wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
            if !timeout_vote_shape(context, vote, true) {
                return Err(ReplayAuthorityValidationError::InvalidSource);
            }
            let highest = vote.highest_prepare_qc.as_ref();
            ReplayShape::new(
                lifecycle_key(
                    context,
                    vote.round,
                    highest.map(|qc| qc.proposal_round),
                    highest.map(|qc| block_subject(qc.subject)),
                    LifecyclePhase::BroadcastTimeoutVote,
                    highest.map(|qc| execution_commitment(qc.execution_commitment)),
                ),
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastTimeoutVote,
            )
        }
        wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
            if !timeout_certificate_shape(context, certificate) {
                return Err(ReplayAuthorityValidationError::InvalidSource);
            }
            let highest = certificate.highest_prepare_qc();
            ReplayShape::new(
                lifecycle_key(
                    context,
                    certificate.round,
                    highest.map(|qc| qc.proposal_round),
                    Some(timeout_certificate_envelope_subject(certificate)),
                    LifecyclePhase::BroadcastTc,
                    highest.map(|qc| execution_commitment(qc.execution_commitment)),
                ),
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastTc,
            )
        }
        wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => {
            return Err(ReplayAuthorityValidationError::InvalidSource);
        }
    };
    (shape.stage_kind == requested_stage)
        .then_some(shape)
        .ok_or(ReplayAuthorityValidationError::RecordMismatch)
}
fn project_equivocation(
    context: LifecycleContext,
    evidence: &wire::SumeragiV2Equivocation,
    requested_stage: LifecycleStageKind,
    payload: &ReplayPayloadBindingV1,
) -> Result<ReplayShape, ReplayAuthorityValidationError> {
    if !payload.is_none()
        || crate::sumeragi::evidence::canonicalize_v2_conflict(evidence) != *evidence
    {
        return Err(ReplayAuthorityValidationError::InvalidSource);
    }
    let (round, phase, stage_kind, valid) = match evidence {
        wire::SumeragiV2Equivocation::Proposal { first, second } => (
            first.round,
            LifecyclePhase::DiagnosticProposalEquivocation,
            LifecycleStageKind::ReportProposalEquivocation,
            proposal_shape(context, first, true)
                && proposal_shape(context, second, true)
                && first.round == second.round
                && first.proposer == second.proposer
                && first.signature_preimage() != second.signature_preimage(),
        ),
        wire::SumeragiV2Equivocation::PhaseVote { first, second } => (
            first.round,
            LifecyclePhase::DiagnosticVoteEquivocation,
            LifecycleStageKind::ReportVoteEquivocation,
            vote_shape(context, first, true)
                && vote_shape(context, second, true)
                && first.round == second.round
                && first.phase == second.phase
                && first.signer == second.signer
                && first.signature_preimage() != second.signature_preimage(),
        ),
        wire::SumeragiV2Equivocation::TimeoutVote { first, second } => (
            first.round,
            LifecyclePhase::DiagnosticTimeoutEquivocation,
            LifecycleStageKind::ReportTimeoutEquivocation,
            timeout_vote_shape(context, first, true)
                && timeout_vote_shape(context, second, true)
                && first.round == second.round
                && first.signer == second.signer
                && first.signature_preimage() != second.signature_preimage(),
        ),
    };
    if !valid || requested_stage != stage_kind {
        return Err(ReplayAuthorityValidationError::InvalidSource);
    }
    Ok(ReplayShape::new(
        lifecycle_key(
            context,
            round,
            None,
            Some(equivocation_subject(evidence)),
            phase,
            None,
        ),
        LifecycleWorkClass::EquivocationReport,
        stage_kind,
    ))
}
fn proposal_shape(context: LifecycleContext, proposal: &wire::Proposal, signed: bool) -> bool {
    round_matches_context(context, proposal.round)
        && proposal.manifest.round == proposal.round
        && proposal.manifest.subject == proposal.subject
        && signature_presence_matches(&proposal.signature, signed)
}
fn vote_shape(context: LifecycleContext, vote: &wire::Vote, signed: bool) -> bool {
    round_matches_context(context, vote.round)
        && round_matches_context(context, vote.proposal_round)
        && vote.proposal_round == vote.round
        && vote.execution_commitment.validate().is_ok()
        && signature_presence_matches(&vote.signature, signed)
}
fn qc_shape(context: LifecycleContext, certificate: &wire::QuorumCertificate) -> bool {
    round_matches_context(context, certificate.round)
        && round_matches_context(context, certificate.proposal_round)
        && certificate.proposal_round == certificate.round
        && certificate.execution_commitment.validate().is_ok()
        && !certificate.signers.is_empty()
        && certificate.signers.len() <= wire::MAX_VALIDATORS_PER_HEIGHT
        && certificate.signers.windows(2).all(|pair| pair[0] < pair[1])
        && signature_present(&certificate.aggregate_signature)
}
fn timeout_vote_shape(context: LifecycleContext, vote: &wire::TimeoutVote, signed: bool) -> bool {
    round_matches_context(context, vote.round)
        && signature_presence_matches(&vote.signature, signed)
        && vote.highest_prepare_qc.as_ref().is_none_or(|highest| {
            qc_shape(context, highest)
                && highest.phase == wire::GlobalPhase::Prepare
                && highest.round.view <= vote.round.view
        })
}
fn timeout_certificate_shape(
    context: LifecycleContext,
    certificate: &wire::TimeoutCertificate,
) -> bool {
    round_matches_context(context, certificate.round)
        && !certificate.groups.is_empty()
        && certificate.groups.iter().all(|group| {
            !group.signers.is_empty()
                && group.signers.windows(2).all(|pair| pair[0] < pair[1])
                && signature_present(&group.aggregate_signature)
                && group.highest_prepare_qc.as_ref().is_none_or(|highest| {
                    qc_shape(context, highest)
                        && highest.phase == wire::GlobalPhase::Prepare
                        && highest.round.view <= certificate.round.view
                })
        })
}
fn enter_view_shape(
    context: LifecycleContext,
    tag: ReplayEventTagV1,
    certificate: &wire::TimeoutCertificate,
    protected_lock: Option<&wire::QuorumCertificate>,
) -> bool {
    tag.height == context.height()
        && certificate.round.view.checked_add(1) == Some(tag.view)
        && protected_lock.is_none_or(|lock| {
            qc_shape(context, lock)
                && lock.phase == wire::GlobalPhase::Prepare
                && lock.proposal_round.view < tag.view
        })
        && match certificate.highest_prepare_qc() {
            None => true,
            Some(highest) => protected_lock.is_some_and(|protected| {
                protected.round.view > highest.round.view
                    || (protected.round.view == highest.round.view
                        && protected.round == highest.round
                        && protected.proposal_round == highest.proposal_round
                        && protected.phase == highest.phase
                        && protected.subject == highest.subject
                        && protected.execution_commitment == highest.execution_commitment)
            }),
        }
}
fn manifest_matches_origin(
    context: LifecycleContext,
    manifest: &wire::PayloadManifest,
    proposal_round: wire::ConsensusRound,
    subject: wire::BlockSubject,
) -> bool {
    round_matches_context(context, manifest.round)
        && manifest.round == proposal_round
        && manifest.subject == subject
}
fn signature_presence_matches(signature: &[u8], signed: bool) -> bool {
    if signed {
        signature_present(signature)
    } else {
        signature.is_empty()
    }
}
fn signature_present(signature: &[u8]) -> bool {
    !signature.is_empty() && signature.len() <= wire::MAX_CONSENSUS_SIGNATURE_BYTES
}
fn round_matches_context(context: LifecycleContext, round: wire::ConsensusRound) -> bool {
    round.height == context.height()
        && digest_from_bytes(round.context_id.0.as_ref()) == context.id()
}
fn lifecycle_key(
    context: LifecycleContext,
    round: wire::ConsensusRound,
    proposal_round: Option<wire::ConsensusRound>,
    subject: Option<LifecycleDigest>,
    phase: LifecyclePhase,
    commitment: Option<LifecycleDigest>,
) -> LifecycleKey {
    LifecycleKey::new(
        context.id(),
        LifecycleRound::new(round.height, round.view),
        proposal_round.map(|round| LifecycleRound::new(round.height, round.view)),
        subject,
        phase,
        commitment,
    )
}
fn equivocation_subject(evidence: &wire::SumeragiV2Equivocation) -> LifecycleDigest {
    let (kind, offender, mut first, mut second) = match evidence {
        wire::SumeragiV2Equivocation::Proposal { first, second } => (
            1,
            first.proposer,
            first.signature_preimage(),
            second.signature_preimage(),
        ),
        wire::SumeragiV2Equivocation::PhaseVote { first, second } => (
            2,
            first.signer,
            first.signature_preimage(),
            second.signature_preimage(),
        ),
        wire::SumeragiV2Equivocation::TimeoutVote { first, second } => (
            3,
            first.signer,
            first.signature_preimage(),
            second.signature_preimage(),
        ),
    };
    if second < first {
        core::mem::swap(&mut first, &mut second);
    }
    let mut projection = Vec::new();
    projection.extend_from_slice(EQUIVOCATION_SUBJECT_DOMAIN);
    projection.push(kind);
    projection.extend_from_slice(&offender.to_le_bytes());
    append_field(&mut projection, &first);
    append_field(&mut projection, &second);
    digest_from_hash(&Hash::new(projection))
}
fn append_field(projection: &mut Vec<u8>, field: &[u8]) {
    projection.extend_from_slice(
        &u64::try_from(field.len())
            .expect("bounded replay-authority projection field fits u64")
            .to_le_bytes(),
    );
    projection.extend_from_slice(field);
}
fn digest_from_hash(hash: &Hash) -> LifecycleDigest {
    digest_from_bytes(hash.as_ref())
}
fn digest_from_bytes(bytes: &[u8]) -> LifecycleDigest {
    let mut digest = [0; 32];
    digest.copy_from_slice(bytes);
    LifecycleDigest::new(digest)
}
// Decoded envelopes remain inert persisted evidence. Origin-specific startup
// joins reauthenticate each retained source against its owning durable store
// before the registry reconstructs executable replay work.
