use std::{collections::BTreeMap, error::Error, fmt};

use crate::{
    ContextId, HeightContext, Phase, Proposal, ProposalJustification, QuorumCertificate, Round,
    TimeoutCertificate, TimeoutVote, ValidatorId, Vote,
};

/// Monotonic identifier of a requested append to the safety WAL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PersistenceId(u64);

impl PersistenceId {
    /// Constructs a persistence identifier.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric identifier.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Safety-relevant transition stored in the append-only WAL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WalRecord {
    /// Persist one locally validated proposal before signing it.
    ProposalIntent(Proposal),
    /// Persist a validated Prepare intent before signing the vote.
    PrepareIntent(Vote),
    /// Persist a newly observed highest `PrepareQC` before reporting it.
    ObservePrepare(QuorumCertificate),
    /// Atomically persist the lock and Commit intent before signing Commit.
    LockAndCommit {
        /// `PrepareQC` that establishes the new lock.
        prepare: QuorumCertificate,
        /// Commit vote authorized by that lock.
        vote: Vote,
    },
    /// Persist a timeout intent before signing it.
    TimeoutIntent(TimeoutVote),
    /// Persist a timeout certificate before entering its successor view.
    InstallTimeout(TimeoutCertificate),
    /// Persist a `CommitQC` decision before applying the block.
    Decision(QuorumCertificate),
}

impl WalRecord {
    pub(crate) fn context_id(&self) -> ContextId {
        match self {
            Self::ProposalIntent(proposal) => proposal.context_id(),
            Self::PrepareIntent(vote) | Self::LockAndCommit { vote, .. } => vote.context_id(),
            Self::ObservePrepare(certificate) | Self::Decision(certificate) => {
                certificate.reference().context_id()
            }
            Self::TimeoutIntent(vote) => vote.context_id(),
            Self::InstallTimeout(certificate) => certificate.context_id(),
        }
    }
}

/// One complete append-only WAL frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalEntry {
    id: PersistenceId,
    record: WalRecord,
}

impl WalEntry {
    /// Constructs a WAL entry.
    #[must_use]
    pub const fn new(id: PersistenceId, record: WalRecord) -> Self {
        Self { id, record }
    }

    /// Returns the monotonic entry identifier.
    #[must_use]
    pub const fn id(&self) -> PersistenceId {
        self.id
    }

    /// Returns the stored safety transition.
    #[must_use]
    pub const fn record(&self) -> &WalRecord {
        &self.record
    }
}

/// Consensus state reconstructed exclusively from acknowledged WAL entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableState {
    context_id: ContextId,
    height: u64,
    current_view: u64,
    last_id: PersistenceId,
    proposal_intents: BTreeMap<Round, Proposal>,
    prepare_intents: BTreeMap<Round, Vote>,
    commit_intents: BTreeMap<Round, Vote>,
    timeout_intents: BTreeMap<Round, TimeoutVote>,
    highest_prepare: Option<QuorumCertificate>,
    locked: Option<QuorumCertificate>,
    last_timeout: Option<TimeoutCertificate>,
    decision: Option<QuorumCertificate>,
}

impl DurableState {
    /// Creates the initial durable state for a height.
    #[must_use]
    pub fn new(context: &HeightContext) -> Self {
        Self {
            context_id: context.id(),
            height: context.height(),
            current_view: 0,
            last_id: PersistenceId::default(),
            proposal_intents: BTreeMap::new(),
            prepare_intents: BTreeMap::new(),
            commit_intents: BTreeMap::new(),
            timeout_intents: BTreeMap::new(),
            highest_prepare: None,
            locked: None,
            last_timeout: None,
            decision: None,
        }
    }

    /// Replays complete WAL entries in order and returns the reconstructed state.
    ///
    /// # Errors
    ///
    /// Returns an error if any frame is missing, reordered, malformed, or
    /// violates a durable consensus invariant.
    pub fn replay(
        context: &HeightContext,
        local_validator: Option<ValidatorId>,
        entries: impl IntoIterator<Item = WalEntry>,
    ) -> Result<Self, ReplayError> {
        let mut state = Self::new(context);
        for entry in entries {
            state.apply(context, local_validator, &entry)?;
        }
        Ok(state)
    }

    /// Applies one complete WAL frame.
    ///
    /// # Errors
    ///
    /// Returns an error if the frame is out of sequence, targets another
    /// context, or violates a durable vote, lock, timeout, or decision rule.
    #[allow(clippy::too_many_lines)]
    pub fn apply(
        &mut self,
        context: &HeightContext,
        local_validator: Option<ValidatorId>,
        entry: &WalEntry,
    ) -> Result<(), ReplayError> {
        let mut next = self.clone();
        next.apply_in_place(context, local_validator, entry)?;
        *self = next;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn apply_in_place(
        &mut self,
        context: &HeightContext,
        local_validator: Option<ValidatorId>,
        entry: &WalEntry,
    ) -> Result<(), ReplayError> {
        let expected = self
            .last_id
            .0
            .checked_add(1)
            .ok_or(ReplayError::SequenceOverflow)?;
        if entry.id.0 != expected {
            return Err(ReplayError::NonContiguousSequence {
                expected: PersistenceId::new(expected),
                actual: entry.id,
            });
        }
        if entry.record.context_id() != self.context_id {
            return Err(ReplayError::ContextMismatch);
        }
        match &entry.record {
            WalRecord::ProposalIntent(proposal) => {
                if Some(proposal.proposer()) != local_validator
                    || proposal.proposer() != context.leader(self.current_view)
                    || proposal.context_id() != self.context_id
                    || proposal.round() != Round::new(self.height, self.current_view)
                    || self.timeout_intents.contains_key(&proposal.round())
                {
                    return Err(ReplayError::InvalidProposalIntent);
                }
                let proposal_high = match proposal.justification() {
                    ProposalJustification::ParentCommit(parent)
                        if proposal.round().view() == 0
                            && match (*parent, context.parent_commit()) {
                                (None, None) => true,
                                (Some(carried), Some(frozen)) => {
                                    carried.same_commit_decision(frozen)
                                }
                                (None, Some(_)) | (Some(_), None) => false,
                            } =>
                    {
                        None
                    }
                    ProposalJustification::Timeout(certificate)
                        if proposal.round().view() > 0
                            && certificate.validate(context).is_ok()
                            && certificate.round().view().checked_add(1)
                                == Some(proposal.round().view())
                            && certificate.highest_prepare().is_none_or(|highest| {
                                highest.subject() == proposal.manifest().subject()
                            }) =>
                    {
                        certificate.highest_prepare()
                    }
                    _ => return Err(ReplayError::InvalidProposalIntent),
                };
                if let Some(locked) = &self.locked
                    && locked.subject() != proposal.manifest().subject()
                    && proposal_high.is_none_or(|highest| {
                        highest.phase() != Phase::Prepare
                            || highest.subject() != proposal.manifest().subject()
                            || highest.round().view() <= locked.round().view()
                    })
                {
                    return Err(ReplayError::InvalidProposalIntent);
                }
                if let Some(existing) = self.proposal_intents.get(&proposal.round()) {
                    if existing != proposal {
                        return Err(ReplayError::ConflictingProposalIntent(proposal.round()));
                    }
                } else {
                    self.proposal_intents
                        .insert(proposal.round(), proposal.clone());
                }
            }
            WalRecord::PrepareIntent(vote) => {
                Self::validate_local_vote(context, local_validator, *vote, Phase::Prepare)?;
                if vote.round().view() != self.current_view {
                    return Err(ReplayError::InvalidLocalVote);
                }
                if self.timeout_intents.contains_key(&vote.round()) {
                    return Err(ReplayError::ViewClosed(vote.round()));
                }
                insert_unique_vote(&mut self.prepare_intents, *vote)?;
            }
            WalRecord::ObservePrepare(certificate) => {
                validate_qc(context, certificate, Phase::Prepare)?;
                if certificate.round().view() > self.current_view {
                    return Err(ReplayError::InvalidCertificate);
                }
                update_highest(&mut self.highest_prepare, certificate.clone())?;
            }
            WalRecord::LockAndCommit { prepare, vote } => {
                validate_qc(context, prepare, Phase::Prepare)?;
                Self::validate_local_vote(context, local_validator, *vote, Phase::Commit)?;
                if vote.round().view() != self.current_view {
                    return Err(ReplayError::InvalidLocalVote);
                }
                if vote.round() != prepare.round() || vote.subject() != prepare.subject() {
                    return Err(ReplayError::CommitDoesNotMatchPrepare);
                }
                if self.timeout_intents.contains_key(&vote.round()) {
                    return Err(ReplayError::ViewClosed(vote.round()));
                }
                if let Some(locked) = &self.locked
                    && (prepare.round().view() < locked.round().view()
                        || (prepare.round().view() == locked.round().view()
                            && prepare.subject() != locked.subject()))
                {
                    return Err(ReplayError::LockRegression);
                }
                insert_unique_vote(&mut self.commit_intents, *vote)?;
                update_highest(&mut self.highest_prepare, prepare.clone())?;
                self.locked = Some(prepare.clone());
            }
            WalRecord::TimeoutIntent(vote) => {
                if vote.context_id() != self.context_id
                    || vote.round().height() != self.height
                    || vote.round().view() != self.current_view
                    || Some(vote.signer()) != local_validator
                    || context.validator(&vote.signer()).is_none()
                {
                    return Err(ReplayError::InvalidLocalVote);
                }
                if vote.highest_prepare() != self.highest_prepare.as_ref() {
                    return Err(ReplayError::TimeoutHighQcMismatch);
                }
                if let Some(existing) = self.timeout_intents.get(&vote.round()) {
                    if existing != vote {
                        return Err(ReplayError::ConflictingVoteIntent(vote.round()));
                    }
                } else {
                    self.timeout_intents.insert(vote.round(), vote.clone());
                }
            }
            WalRecord::InstallTimeout(certificate) => {
                certificate
                    .validate(context)
                    .map_err(|_| ReplayError::InvalidCertificate)?;
                if certificate.round().view() < self.current_view {
                    return Err(ReplayError::ViewRegression);
                }
                let selected = certificate.highest_prepare().cloned();
                if let Some(highest) = &selected {
                    match &self.highest_prepare {
                        None => self.highest_prepare = Some(highest.clone()),
                        Some(existing) if highest.round().view() > existing.round().view() => {
                            self.highest_prepare = Some(highest.clone());
                        }
                        Some(existing)
                            if highest.round().view() == existing.round().view()
                                && highest.subject() != existing.subject() =>
                        {
                            return Err(ReplayError::ConflictingHighestPrepare);
                        }
                        Some(_) => {}
                    }

                    match &self.locked {
                        None => self.locked = Some(highest.clone()),
                        Some(locked) if highest.round().view() > locked.round().view() => {
                            self.locked = Some(highest.clone());
                        }
                        Some(locked)
                            if highest.round().view() == locked.round().view()
                                && highest.subject() != locked.subject() =>
                        {
                            return Err(ReplayError::LockRegression);
                        }
                        Some(_) => {}
                    }
                }
                // Installing a TC never lowers or clears a lock. A different
                // subject is safe only when the TC carries a strictly higher
                // PrepareQC; timeout votes transport that full certificate so
                // an omitted local lock becomes known to the next TC quorum.
                self.current_view = certificate
                    .round()
                    .view()
                    .checked_add(1)
                    .ok_or(ReplayError::ViewOverflow)?;
                self.last_timeout = Some(certificate.clone());
            }
            WalRecord::Decision(certificate) => {
                validate_qc(context, certificate, Phase::Commit)?;
                if let Some(existing) = &self.decision {
                    if existing.reference() != certificate.reference() {
                        return Err(ReplayError::ConflictingDecision);
                    }
                } else {
                    self.decision = Some(certificate.clone());
                }
            }
        }
        self.last_id = entry.id;
        Ok(())
    }

    fn validate_local_vote(
        context: &HeightContext,
        local_validator: Option<ValidatorId>,
        vote: Vote,
        phase: Phase,
    ) -> Result<(), ReplayError> {
        if vote.context_id() != context.id()
            || vote.round().height() != context.height()
            || vote.phase() != phase
            || Some(vote.signer()) != local_validator
            || context.validator(&vote.signer()).is_none()
        {
            return Err(ReplayError::InvalidLocalVote);
        }
        Ok(())
    }

    /// Returns the height context identifier.
    #[must_use]
    pub const fn context_id(&self) -> ContextId {
        self.context_id
    }

    /// Returns the height represented by this state.
    #[must_use]
    pub const fn height(&self) -> u64 {
        self.height
    }

    /// Returns the current persisted view.
    #[must_use]
    pub const fn current_view(&self) -> u64 {
        self.current_view
    }

    /// Returns the last applied WAL identifier.
    #[must_use]
    pub const fn last_id(&self) -> PersistenceId {
        self.last_id
    }

    /// Returns the next required WAL identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the monotonic identifier is exhausted.
    pub fn next_id(&self) -> Result<PersistenceId, ReplayError> {
        self.last_id
            .0
            .checked_add(1)
            .map(PersistenceId::new)
            .ok_or(ReplayError::SequenceOverflow)
    }

    /// Returns the highest durable `PrepareQC`.
    #[must_use]
    pub const fn highest_prepare(&self) -> Option<&QuorumCertificate> {
        self.highest_prepare.as_ref()
    }

    /// Returns the current durable lock.
    #[must_use]
    pub const fn locked(&self) -> Option<&QuorumCertificate> {
        self.locked.as_ref()
    }

    /// Returns the last installed timeout certificate.
    #[must_use]
    pub const fn last_timeout(&self) -> Option<&TimeoutCertificate> {
        self.last_timeout.as_ref()
    }

    /// Returns the durable decision, if any.
    #[must_use]
    pub const fn decision(&self) -> Option<&QuorumCertificate> {
        self.decision.as_ref()
    }

    /// Returns the local Prepare intent for a round.
    #[must_use]
    pub fn prepare_intent(&self, round: Round) -> Option<Vote> {
        self.prepare_intents.get(&round).copied()
    }

    /// Returns the local proposal intent for a round.
    #[must_use]
    pub fn proposal_intent(&self, round: Round) -> Option<&Proposal> {
        self.proposal_intents.get(&round)
    }

    /// Returns the local Commit intent for a round.
    #[must_use]
    pub fn commit_intent(&self, round: Round) -> Option<Vote> {
        self.commit_intents.get(&round).copied()
    }

    pub(crate) fn prepare_intents(&self) -> impl Iterator<Item = Vote> + '_ {
        self.prepare_intents.values().copied()
    }

    pub(crate) fn commit_intents(&self) -> impl Iterator<Item = Vote> + '_ {
        self.commit_intents.values().copied()
    }

    /// Returns the local timeout intent for a round.
    #[must_use]
    pub fn timeout_intent(&self, round: Round) -> Option<TimeoutVote> {
        self.timeout_intents.get(&round).cloned()
    }
}

fn validate_qc(
    context: &HeightContext,
    certificate: &QuorumCertificate,
    expected_phase: Phase,
) -> Result<(), ReplayError> {
    if certificate.phase() != expected_phase {
        return Err(ReplayError::InvalidCertificate);
    }
    certificate
        .validate(context)
        .map(|_| ())
        .map_err(|_| ReplayError::InvalidCertificate)
}

fn insert_unique_vote(intents: &mut BTreeMap<Round, Vote>, vote: Vote) -> Result<(), ReplayError> {
    if let Some(existing) = intents.get(&vote.round()) {
        if existing != &vote {
            return Err(ReplayError::ConflictingVoteIntent(vote.round()));
        }
    } else {
        intents.insert(vote.round(), vote);
    }
    Ok(())
}

fn update_highest(
    highest: &mut Option<QuorumCertificate>,
    candidate: QuorumCertificate,
) -> Result<(), ReplayError> {
    if let Some(existing) = highest {
        if candidate.round().view() < existing.round().view() {
            return Err(ReplayError::HighestQcRegression);
        }
        if candidate.round().view() == existing.round().view()
            && candidate.subject() != existing.subject()
        {
            return Err(ReplayError::ConflictingHighestPrepare);
        }
        if candidate.round().view() == existing.round().view() {
            return Ok(());
        }
    }
    *highest = Some(candidate);
    Ok(())
}

/// Failure while applying or replaying complete WAL records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayError {
    /// WAL identifiers cannot be incremented further.
    SequenceOverflow,
    /// A WAL frame was missing, duplicated, or reordered.
    NonContiguousSequence {
        /// Required next identifier.
        expected: PersistenceId,
        /// Identifier found in the WAL.
        actual: PersistenceId,
    },
    /// A WAL record belongs to another height context.
    ContextMismatch,
    /// A local vote record has the wrong signer, context, height, or phase.
    InvalidLocalVote,
    /// A local proposal has an invalid leader, round, justification, or lock.
    InvalidProposalIntent,
    /// A second local proposal conflicts with an already durable proposal.
    ConflictingProposalIntent(Round),
    /// A certificate failed structural, membership, or quorum validation.
    InvalidCertificate,
    /// A second local intent conflicts with an already durable vote.
    ConflictingVoteIntent(Round),
    /// A Commit intent does not match its authorizing `PrepareQC`.
    CommitDoesNotMatchPrepare,
    /// A replayed lock moves backwards or conflicts at the same view.
    LockRegression,
    /// A highest `PrepareQC` moves backwards.
    HighestQcRegression,
    /// Equal-view highest `PrepareQC`s certify different subjects.
    ConflictingHighestPrepare,
    /// A timeout does not report the highest durable `PrepareQC`.
    TimeoutHighQcMismatch,
    /// A Prepare or Commit intent was appended after the view was durably closed.
    ViewClosed(Round),
    /// A timeout certificate would move the persisted view backwards.
    ViewRegression,
    /// The successor view cannot be represented.
    ViewOverflow,
    /// Two durable `CommitQC`s decide different subjects.
    ConflictingDecision,
}

impl fmt::Display for ReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SequenceOverflow => formatter.write_str("WAL sequence overflow"),
            Self::NonContiguousSequence { expected, actual } => write!(
                formatter,
                "non-contiguous WAL sequence: expected {}, got {}",
                expected.get(),
                actual.get()
            ),
            Self::ContextMismatch => formatter.write_str("WAL context mismatch"),
            Self::InvalidLocalVote => formatter.write_str("invalid local vote in WAL"),
            Self::InvalidProposalIntent => formatter.write_str("invalid local proposal in WAL"),
            Self::ConflictingProposalIntent(round) => write!(
                formatter,
                "conflicting proposal intent at height {}, view {}",
                round.height(),
                round.view()
            ),
            Self::InvalidCertificate => formatter.write_str("invalid certificate in WAL"),
            Self::ConflictingVoteIntent(round) => write!(
                formatter,
                "conflicting vote intent at height {}, view {}",
                round.height(),
                round.view()
            ),
            Self::CommitDoesNotMatchPrepare => {
                formatter.write_str("Commit intent does not match PrepareQC")
            }
            Self::LockRegression => formatter.write_str("durable lock regression"),
            Self::HighestQcRegression => formatter.write_str("highest PrepareQC regression"),
            Self::ConflictingHighestPrepare => {
                formatter.write_str("conflicting highest PrepareQCs")
            }
            Self::TimeoutHighQcMismatch => formatter.write_str("timeout high-QC mismatch"),
            Self::ViewClosed(round) => write!(
                formatter,
                "height {}, view {} is durably closed",
                round.height(),
                round.view()
            ),
            Self::ViewRegression => formatter.write_str("durable view regression"),
            Self::ViewOverflow => formatter.write_str("view overflow"),
            Self::ConflictingDecision => formatter.write_str("conflicting durable decisions"),
        }
    }
}

impl Error for ReplayError {}
