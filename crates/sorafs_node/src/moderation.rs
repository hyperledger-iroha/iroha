//! Local SoraFS moderation ballot lifecycle runtime.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::sorafs::moderation::{
    SoraFsModerationBallotCommitV1, SoraFsModerationBallotContextV1, SoraFsModerationBallotError,
    SoraFsModerationBallotRevealV1, SoraFsModerationVoteChoice,
};
use sorafs_manifest::{
    SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
    SoraFsModerationBallotGovernanceEventKindV1, SoraFsModerationBallotGovernanceEventV1,
    SoraFsModerationBallotGovernanceTallyV1, SoraFsModerationVoteChoiceV1,
    SoraFsModerationVoteCountsV1,
};
use thiserror::Error;

const MODERATION_ROSTER_HASH_DOMAIN_V1: &[u8] = b"sorafs.moderation.local.panel-roster-hash.v1";

/// Derive the local moderation panel roster hash for an ordered juror roster.
///
/// The local runtime includes the quorum and ordered juror identifiers so
/// replayed announcements cannot silently swap failover order or quorum policy.
#[must_use]
pub fn local_moderation_panel_roster_hash(juror_ids: &[String], quorum: u16) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_ROSTER_HASH_DOMAIN_V1);
    hasher.update(&quorum.to_le_bytes());
    hasher.update(&(juror_ids.len() as u64).to_le_bytes());
    for juror_id in juror_ids {
        hasher.update(&(juror_id.len() as u64).to_le_bytes());
        hasher.update(juror_id.as_bytes());
    }
    *hasher.finalize().as_bytes()
}

/// Local moderation ballot announcement accepted by the lifecycle runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotAnnouncement {
    /// Immutable case context bound by every commit and reveal.
    pub context: SoraFsModerationBallotContextV1,
    /// Confirmed appeal deposit asset-lock id that admitted this ballot.
    pub appeal_deposit_escrow_id_hex: Option<String>,
    /// Confirmed appeal deposit metadata used to publish tally finance reports.
    pub appeal_deposit: Option<ModerationAppealDeposit>,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Ordered juror identifiers eligible to participate in this ballot.
    pub juror_ids: Vec<String>,
    /// Minimum number of valid reveals required before a tally can finalize.
    pub quorum: u16,
    /// UTC timestamp (milliseconds) when the ballot was announced.
    pub announced_at_unix_ms: u64,
    /// Last UTC timestamp (milliseconds) at which commits are accepted.
    pub commit_deadline_unix_ms: u64,
    /// Last UTC timestamp (milliseconds) for the post-commit challenge buffer.
    pub challenge_deadline_unix_ms: u64,
    /// Last UTC timestamp (milliseconds) at which reveals are accepted.
    pub reveal_deadline_unix_ms: u64,
}

/// Confirmed appeal deposit metadata captured at moderation intake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationAppealDeposit {
    /// Confirmed native asset-lock escrow id.
    pub escrow_id_hex: String,
    /// Canonical account that funded the deposit lock.
    pub payer_account: String,
    /// Canonical account receiving non-refunded deposit drawdowns.
    pub destination_account: String,
    /// Optional canonical account required to approve drawdowns.
    pub release_authority_account: Option<String>,
    /// Canonical asset definition held by the lock.
    pub asset_definition_id: String,
    /// Deterministic native asset-lock custody account.
    pub custody_account: String,
    /// Exact non-negative deposited XOR decimal amount.
    pub deposit_xor: String,
    /// Optional lock expiry timestamp in Unix milliseconds.
    pub expires_at_ms: Option<u64>,
    /// Client-supplied idempotency key used to derive the escrow id.
    pub idempotency_key: String,
}

/// Sequenced local moderation ballot event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationBallotEventKind {
    /// A ballot was announced.
    BallotAnnounced,
    /// A juror commitment was accepted.
    CommitAccepted,
    /// A juror reveal was accepted.
    RevealAccepted,
    /// The ballot was tallied.
    BallotTallied,
}

impl From<ModerationBallotEventKind> for SoraFsModerationBallotGovernanceEventKindV1 {
    fn from(kind: ModerationBallotEventKind) -> Self {
        match kind {
            ModerationBallotEventKind::BallotAnnounced => Self::BallotAnnounced,
            ModerationBallotEventKind::CommitAccepted => Self::CommitAccepted,
            ModerationBallotEventKind::RevealAccepted => Self::RevealAccepted,
            ModerationBallotEventKind::BallotTallied => Self::BallotTallied,
        }
    }
}

/// Local moderation ballot vote counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModerationVoteCounts {
    /// Number of `uphold` reveals.
    pub uphold: u32,
    /// Number of `overturn` reveals.
    pub overturn: u32,
    /// Number of `modify` reveals.
    pub modify: u32,
    /// Number of `escalate` reveals.
    pub escalate: u32,
}

impl ModerationVoteCounts {
    fn increment(&mut self, choice: SoraFsModerationVoteChoice) {
        match choice {
            SoraFsModerationVoteChoice::Uphold => self.uphold = self.uphold.saturating_add(1),
            SoraFsModerationVoteChoice::Overturn => {
                self.overturn = self.overturn.saturating_add(1);
            }
            SoraFsModerationVoteChoice::Modify => self.modify = self.modify.saturating_add(1),
            SoraFsModerationVoteChoice::Escalate => {
                self.escalate = self.escalate.saturating_add(1);
            }
        }
    }

    fn winning_choice(self) -> Option<SoraFsModerationVoteChoice> {
        let choices = [
            (SoraFsModerationVoteChoice::Uphold, self.uphold),
            (SoraFsModerationVoteChoice::Overturn, self.overturn),
            (SoraFsModerationVoteChoice::Modify, self.modify),
            (SoraFsModerationVoteChoice::Escalate, self.escalate),
        ];
        let max_votes = choices.iter().map(|(_, count)| *count).max().unwrap_or(0);
        if max_votes == 0
            || choices
                .iter()
                .filter(|(_, count)| *count == max_votes)
                .count()
                != 1
        {
            return None;
        }
        choices
            .into_iter()
            .find_map(|(choice, count)| (count == max_votes).then_some(choice))
    }
}

impl From<ModerationVoteCounts> for SoraFsModerationVoteCountsV1 {
    fn from(counts: ModerationVoteCounts) -> Self {
        Self {
            uphold: counts.uphold,
            overturn: counts.overturn,
            modify: counts.modify,
            escalate: counts.escalate,
        }
    }
}

/// Final local tally for one moderation ballot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotTally {
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Vote counts by moderation choice.
    pub counts: ModerationVoteCounts,
    /// Number of valid reveals included in the tally.
    pub votes_total: u32,
    /// Required reveal quorum.
    pub quorum: u16,
    /// Winning choice when the tally has exactly one highest vote count.
    pub winning_choice: Option<SoraFsModerationVoteChoice>,
    /// True when the vote reached quorum but no unique winner exists.
    pub contested: bool,
    /// UTC timestamp (milliseconds) when the tally was finalized locally.
    pub tallied_at_unix_ms: u64,
}

impl From<&ModerationBallotTally> for SoraFsModerationBallotGovernanceTallyV1 {
    fn from(tally: &ModerationBallotTally) -> Self {
        Self {
            case_id: tally.case_id.clone(),
            round_id: tally.round_id.clone(),
            counts: tally.counts.into(),
            votes_total: tally.votes_total,
            quorum: tally.quorum,
            winning_choice: tally.winning_choice.map(governance_vote_choice),
            contested: tally.contested,
            tallied_at_unix_ms: tally.tallied_at_unix_ms,
        }
    }
}

/// Snapshot of one local moderation ballot lifecycle record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotRecord {
    /// Ballot announcement.
    pub announcement: ModerationBallotAnnouncement,
    /// Accepted juror commitments sorted by juror id.
    pub commits: Vec<SoraFsModerationBallotCommitV1>,
    /// Accepted juror reveals sorted by juror id.
    pub reveals: Vec<SoraFsModerationBallotRevealV1>,
    /// Final tally when the ballot has been finalized.
    pub tally: Option<ModerationBallotTally>,
}

/// Result of accepting a moderation ballot commitment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotCommitOutcome {
    /// Accepted commitment payload.
    pub accepted_commit: SoraFsModerationBallotCommitV1,
    /// Number of commitments accepted for the ballot.
    pub committed_count: usize,
    /// Number of reveals accepted for the ballot.
    pub revealed_count: usize,
}

/// Result of accepting a moderation ballot reveal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotRevealOutcome {
    /// Accepted reveal payload.
    pub accepted_reveal: SoraFsModerationBallotRevealV1,
    /// Number of commitments accepted for the ballot.
    pub committed_count: usize,
    /// Number of reveals accepted for the ballot.
    pub revealed_count: usize,
}

/// Sequenced local moderation ballot event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotEvent {
    /// Monotonic local event sequence.
    pub sequence: u64,
    /// Event kind.
    pub kind: ModerationBallotEventKind,
    /// UTC timestamp (milliseconds) when the event was generated.
    pub generated_at_unix_ms: u64,
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Juror associated with the event, when any.
    pub juror_id: Option<String>,
    /// Accepted commitment count after the event.
    pub committed_count: u64,
    /// Accepted reveal count after the event.
    pub revealed_count: u64,
    /// Tally associated with the event, when finalized.
    pub tally: Option<ModerationBallotTally>,
}

impl ModerationBallotEvent {
    pub(crate) fn to_governance_event_v1(&self) -> SoraFsModerationBallotGovernanceEventV1 {
        SoraFsModerationBallotGovernanceEventV1 {
            version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
            sequence: self.sequence,
            kind: self.kind.into(),
            generated_at_unix_ms: self.generated_at_unix_ms,
            case_id: self.case_id.clone(),
            round_id: self.round_id.clone(),
            juror_id: self.juror_id.clone(),
            committed_count: self.committed_count,
            revealed_count: self.revealed_count,
            tally: self.tally.as_ref().map(Into::into),
        }
    }
}

fn governance_vote_choice(choice: SoraFsModerationVoteChoice) -> SoraFsModerationVoteChoiceV1 {
    match choice {
        SoraFsModerationVoteChoice::Uphold => SoraFsModerationVoteChoiceV1::Uphold,
        SoraFsModerationVoteChoice::Overturn => SoraFsModerationVoteChoiceV1::Overturn,
        SoraFsModerationVoteChoice::Modify => SoraFsModerationVoteChoiceV1::Modify,
        SoraFsModerationVoteChoice::Escalate => SoraFsModerationVoteChoiceV1::Escalate,
    }
}

/// Error raised by the local moderation ballot runtime.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModerationBallotRuntimeError {
    /// Canonical moderation ballot payload validation failed.
    #[error(transparent)]
    Validation(#[from] SoraFsModerationBallotError),
    /// Ballot already exists for the case and round.
    #[error("moderation ballot `{case_id}` round `{round_id}` already exists")]
    DuplicateBallot {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
    },
    /// Ballot is unknown for the case and round.
    #[error("moderation ballot `{case_id}` round `{round_id}` is unknown")]
    UnknownBallot {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
    },
    /// Ballot round id is blank.
    #[error("moderation ballot round id is required")]
    MissingRoundId,
    /// Ballot roster is empty.
    #[error("moderation ballot juror roster is required")]
    MissingJurors,
    /// Ballot roster contains a blank juror id.
    #[error("moderation ballot juror id is required")]
    BlankJurorId,
    /// Ballot roster contains a duplicate juror id.
    #[error("moderation ballot juror `{juror_id}` is duplicated")]
    DuplicateJuror {
        /// Duplicated juror identifier.
        juror_id: String,
    },
    /// Ballot quorum is zero or larger than the roster.
    #[error("moderation ballot quorum `{quorum}` is invalid for roster size `{roster_size}`")]
    InvalidQuorum {
        /// Requested quorum.
        quorum: u16,
        /// Roster length.
        roster_size: usize,
    },
    /// Announcement roster hash does not match the local canonical roster hash.
    #[error("moderation ballot roster hash does not match juror roster and quorum")]
    RosterHashMismatch,
    /// Stored appeal deposit metadata is missing its escrow id.
    #[error("moderation ballot appeal deposit escrow id is required")]
    MissingAppealDepositEscrowId,
    /// Stored appeal deposit metadata conflicts with the compatibility escrow id field.
    #[error("moderation ballot appeal deposit escrow id metadata mismatch")]
    AppealDepositEscrowIdMismatch,
    /// Ballot window timestamps are inconsistent.
    #[error("moderation ballot windows must satisfy announced < commit < challenge < reveal")]
    InvalidWindows,
    /// Commit/reveal payload is bound to a different context.
    #[error("moderation ballot payload context mismatch")]
    PayloadContextMismatch,
    /// Commit/reveal payload is bound to a different round.
    #[error("moderation ballot payload round mismatch: expected `{expected}`, found `{found}`")]
    PayloadRoundMismatch {
        /// Expected round identifier.
        expected: String,
        /// Payload round identifier.
        found: String,
    },
    /// Juror is not part of the announced roster.
    #[error(
        "juror `{juror_id}` is not eligible for moderation ballot `{case_id}` round `{round_id}`"
    )]
    IneligibleJuror {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Juror identifier.
        juror_id: String,
    },
    /// Commit submission arrived outside the commit window.
    #[error("moderation ballot commit window is closed at `{now_unix_ms}`")]
    CommitWindowClosed {
        /// Acceptance timestamp.
        now_unix_ms: u64,
    },
    /// A commitment already exists for the juror.
    #[error(
        "juror `{juror_id}` already committed for moderation ballot `{case_id}` round `{round_id}`"
    )]
    DuplicateCommit {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Juror identifier.
        juror_id: String,
    },
    /// Reveal submission arrived before the reveal window opened.
    #[error("moderation ballot reveal window is not open at `{now_unix_ms}`")]
    RevealWindowNotOpen {
        /// Acceptance timestamp.
        now_unix_ms: u64,
    },
    /// Reveal submission arrived after the reveal window closed.
    #[error("moderation ballot reveal window is closed at `{now_unix_ms}`")]
    RevealWindowClosed {
        /// Acceptance timestamp.
        now_unix_ms: u64,
    },
    /// The juror has no accepted commitment to reveal.
    #[error(
        "juror `{juror_id}` has no commitment for moderation ballot `{case_id}` round `{round_id}`"
    )]
    MissingCommit {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Juror identifier.
        juror_id: String,
    },
    /// A reveal already exists for the juror.
    #[error(
        "juror `{juror_id}` already revealed for moderation ballot `{case_id}` round `{round_id}`"
    )]
    DuplicateReveal {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Juror identifier.
        juror_id: String,
    },
    /// Tally was requested before the reveal window closed and before all jurors revealed.
    #[error("moderation ballot reveal window is still open until `{reveal_deadline_unix_ms}`")]
    TallyWindowOpen {
        /// Reveal deadline.
        reveal_deadline_unix_ms: u64,
    },
    /// Tally was already finalized.
    #[error("moderation ballot `{case_id}` round `{round_id}` was already tallied")]
    AlreadyTallied {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
    },
    /// Accepted reveals did not satisfy quorum.
    #[error("moderation ballot quorum `{quorum}` not met by `{reveals}` reveals")]
    QuorumNotMet {
        /// Required quorum.
        quorum: u16,
        /// Accepted reveal count.
        reveals: usize,
    },
    /// The local moderation runtime lock was poisoned.
    #[error("moderation ballot state lock poisoned")]
    StateLockPoisoned,
}

/// Local in-memory moderation ballot lifecycle runtime.
#[derive(Debug, Default)]
pub(crate) struct ModerationBallotRuntime {
    ballots: BTreeMap<ModerationBallotKey, ModerationBallotState>,
}

impl ModerationBallotRuntime {
    pub(crate) fn announce_ballot(
        &mut self,
        announcement: ModerationBallotAnnouncement,
    ) -> Result<ModerationBallotRecord, ModerationBallotRuntimeError> {
        validate_announcement(&announcement)?;
        let key = ModerationBallotKey::from_announcement(&announcement);
        if self.ballots.contains_key(&key) {
            return Err(ModerationBallotRuntimeError::DuplicateBallot {
                case_id: key.case_id,
                round_id: key.round_id,
            });
        }
        self.ballots.insert(
            key.clone(),
            ModerationBallotState {
                announcement,
                commits: BTreeMap::new(),
                reveals: BTreeMap::new(),
                tally: None,
            },
        );
        Ok(self
            .ballots
            .get(&key)
            .expect("inserted moderation ballot exists")
            .to_record())
    }

    pub(crate) fn submit_commit(
        &mut self,
        commit: SoraFsModerationBallotCommitV1,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotCommitOutcome, ModerationBallotRuntimeError> {
        commit.validate()?;
        let key = ModerationBallotKey::from_context_and_round(&commit.context, &commit.round_id);
        let state = self.ballots.get_mut(&key).ok_or_else(|| {
            ModerationBallotRuntimeError::UnknownBallot {
                case_id: key.case_id.clone(),
                round_id: key.round_id.clone(),
            }
        })?;
        validate_payload_scope(state, &commit.context, &commit.round_id)?;
        if now_unix_ms < state.announcement.announced_at_unix_ms
            || now_unix_ms > state.announcement.commit_deadline_unix_ms
        {
            return Err(ModerationBallotRuntimeError::CommitWindowClosed { now_unix_ms });
        }
        ensure_eligible_juror(state, &commit.juror_id)?;
        if state.commits.contains_key(&commit.juror_id) {
            return Err(ModerationBallotRuntimeError::DuplicateCommit {
                case_id: key.case_id,
                round_id: key.round_id,
                juror_id: commit.juror_id,
            });
        }
        state
            .commits
            .insert(commit.juror_id.clone(), commit.clone());
        Ok(ModerationBallotCommitOutcome {
            accepted_commit: commit,
            committed_count: state.commits.len(),
            revealed_count: state.reveals.len(),
        })
    }

    pub(crate) fn submit_reveal(
        &mut self,
        reveal: SoraFsModerationBallotRevealV1,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotRevealOutcome, ModerationBallotRuntimeError> {
        reveal.validate()?;
        let key = ModerationBallotKey::from_context_and_round(&reveal.context, &reveal.round_id);
        let state = self.ballots.get_mut(&key).ok_or_else(|| {
            ModerationBallotRuntimeError::UnknownBallot {
                case_id: key.case_id.clone(),
                round_id: key.round_id.clone(),
            }
        })?;
        validate_payload_scope(state, &reveal.context, &reveal.round_id)?;
        if now_unix_ms <= state.announcement.challenge_deadline_unix_ms {
            return Err(ModerationBallotRuntimeError::RevealWindowNotOpen { now_unix_ms });
        }
        if now_unix_ms > state.announcement.reveal_deadline_unix_ms {
            return Err(ModerationBallotRuntimeError::RevealWindowClosed { now_unix_ms });
        }
        ensure_eligible_juror(state, &reveal.juror_id)?;
        let commit = state.commits.get(&reveal.juror_id).ok_or_else(|| {
            ModerationBallotRuntimeError::MissingCommit {
                case_id: key.case_id.clone(),
                round_id: key.round_id.clone(),
                juror_id: reveal.juror_id.clone(),
            }
        })?;
        if state.reveals.contains_key(&reveal.juror_id) {
            return Err(ModerationBallotRuntimeError::DuplicateReveal {
                case_id: key.case_id,
                round_id: key.round_id,
                juror_id: reveal.juror_id,
            });
        }
        commit.verify_reveal(&reveal)?;
        state
            .reveals
            .insert(reveal.juror_id.clone(), reveal.clone());
        Ok(ModerationBallotRevealOutcome {
            accepted_reveal: reveal,
            committed_count: state.commits.len(),
            revealed_count: state.reveals.len(),
        })
    }

    pub(crate) fn tally_ballot(
        &mut self,
        case_id: &str,
        round_id: &str,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotTally, ModerationBallotRuntimeError> {
        let key = ModerationBallotKey::new(case_id, round_id);
        let state = self.ballots.get_mut(&key).ok_or_else(|| {
            ModerationBallotRuntimeError::UnknownBallot {
                case_id: case_id.to_owned(),
                round_id: round_id.to_owned(),
            }
        })?;
        if state.tally.is_some() {
            return Err(ModerationBallotRuntimeError::AlreadyTallied {
                case_id: case_id.to_owned(),
                round_id: round_id.to_owned(),
            });
        }
        let all_jurors_revealed = state.reveals.len() == state.announcement.juror_ids.len();
        if now_unix_ms < state.announcement.reveal_deadline_unix_ms && !all_jurors_revealed {
            return Err(ModerationBallotRuntimeError::TallyWindowOpen {
                reveal_deadline_unix_ms: state.announcement.reveal_deadline_unix_ms,
            });
        }
        if state.reveals.len() < usize::from(state.announcement.quorum) {
            return Err(ModerationBallotRuntimeError::QuorumNotMet {
                quorum: state.announcement.quorum,
                reveals: state.reveals.len(),
            });
        }

        let mut counts = ModerationVoteCounts::default();
        for reveal in state.reveals.values() {
            counts.increment(reveal.choice);
        }
        let winning_choice = counts.winning_choice();
        let tally = ModerationBallotTally {
            case_id: case_id.to_owned(),
            round_id: round_id.to_owned(),
            counts,
            votes_total: state.reveals.len() as u32,
            quorum: state.announcement.quorum,
            winning_choice,
            contested: winning_choice.is_none(),
            tallied_at_unix_ms: now_unix_ms,
        };
        state.tally = Some(tally.clone());
        Ok(tally)
    }

    pub(crate) fn ballot(&self, case_id: &str, round_id: &str) -> Option<ModerationBallotRecord> {
        self.ballots
            .get(&ModerationBallotKey::new(case_id, round_id))
            .map(ModerationBallotState::to_record)
    }

    pub(crate) fn ballots(&self) -> Vec<ModerationBallotRecord> {
        self.ballots
            .values()
            .map(ModerationBallotState::to_record)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ModerationBallotKey {
    case_id: String,
    round_id: String,
}

impl ModerationBallotKey {
    fn new(case_id: &str, round_id: &str) -> Self {
        Self {
            case_id: case_id.to_owned(),
            round_id: round_id.to_owned(),
        }
    }

    fn from_announcement(announcement: &ModerationBallotAnnouncement) -> Self {
        Self::new(&announcement.context.case_id, &announcement.round_id)
    }

    fn from_context_and_round(context: &SoraFsModerationBallotContextV1, round_id: &str) -> Self {
        Self::new(&context.case_id, round_id)
    }
}

#[derive(Debug, Clone)]
struct ModerationBallotState {
    announcement: ModerationBallotAnnouncement,
    commits: BTreeMap<String, SoraFsModerationBallotCommitV1>,
    reveals: BTreeMap<String, SoraFsModerationBallotRevealV1>,
    tally: Option<ModerationBallotTally>,
}

impl ModerationBallotState {
    fn to_record(&self) -> ModerationBallotRecord {
        ModerationBallotRecord {
            announcement: self.announcement.clone(),
            commits: self.commits.values().cloned().collect(),
            reveals: self.reveals.values().cloned().collect(),
            tally: self.tally.clone(),
        }
    }
}

fn validate_announcement(
    announcement: &ModerationBallotAnnouncement,
) -> Result<(), ModerationBallotRuntimeError> {
    announcement.context.validate()?;
    if announcement.round_id.trim().is_empty() {
        return Err(ModerationBallotRuntimeError::MissingRoundId);
    }
    if announcement.juror_ids.is_empty() {
        return Err(ModerationBallotRuntimeError::MissingJurors);
    }
    let mut seen = BTreeSet::new();
    for juror_id in &announcement.juror_ids {
        if juror_id.trim().is_empty() {
            return Err(ModerationBallotRuntimeError::BlankJurorId);
        }
        if !seen.insert(juror_id.clone()) {
            return Err(ModerationBallotRuntimeError::DuplicateJuror {
                juror_id: juror_id.clone(),
            });
        }
    }
    if announcement.quorum == 0 || usize::from(announcement.quorum) > announcement.juror_ids.len() {
        return Err(ModerationBallotRuntimeError::InvalidQuorum {
            quorum: announcement.quorum,
            roster_size: announcement.juror_ids.len(),
        });
    }
    let expected_hash =
        local_moderation_panel_roster_hash(&announcement.juror_ids, announcement.quorum);
    if announcement.context.panel_roster_hash != expected_hash {
        return Err(ModerationBallotRuntimeError::RosterHashMismatch);
    }
    if let Some(deposit) = &announcement.appeal_deposit {
        if deposit.escrow_id_hex.trim().is_empty() {
            return Err(ModerationBallotRuntimeError::MissingAppealDepositEscrowId);
        }
        if announcement
            .appeal_deposit_escrow_id_hex
            .as_deref()
            .is_some_and(|escrow_id| escrow_id != deposit.escrow_id_hex)
        {
            return Err(ModerationBallotRuntimeError::AppealDepositEscrowIdMismatch);
        }
    }
    if announcement.announced_at_unix_ms >= announcement.commit_deadline_unix_ms
        || announcement.commit_deadline_unix_ms >= announcement.challenge_deadline_unix_ms
        || announcement.challenge_deadline_unix_ms >= announcement.reveal_deadline_unix_ms
    {
        return Err(ModerationBallotRuntimeError::InvalidWindows);
    }
    Ok(())
}

fn validate_payload_scope(
    state: &ModerationBallotState,
    context: &SoraFsModerationBallotContextV1,
    round_id: &str,
) -> Result<(), ModerationBallotRuntimeError> {
    if state.announcement.context != *context {
        return Err(ModerationBallotRuntimeError::PayloadContextMismatch);
    }
    if state.announcement.round_id != round_id {
        return Err(ModerationBallotRuntimeError::PayloadRoundMismatch {
            expected: state.announcement.round_id.clone(),
            found: round_id.to_owned(),
        });
    }
    Ok(())
}

fn ensure_eligible_juror(
    state: &ModerationBallotState,
    juror_id: &str,
) -> Result<(), ModerationBallotRuntimeError> {
    if state
        .announcement
        .juror_ids
        .iter()
        .any(|eligible| eligible == juror_id)
    {
        return Ok(());
    }
    Err(ModerationBallotRuntimeError::IneligibleJuror {
        case_id: state.announcement.context.case_id.clone(),
        round_id: state.announcement.round_id.clone(),
        juror_id: juror_id.to_owned(),
    })
}
