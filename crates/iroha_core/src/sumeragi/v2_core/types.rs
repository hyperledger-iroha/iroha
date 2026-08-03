use std::{error::Error, fmt};

use super::{Quorum, QuorumError};

/// Wire protocol version implemented by this crate.
pub const PROTOCOL_VERSION_V4: u16 = 4;
/// Minimum Byzantine fault tolerance supported by a production committee.
pub const MIN_FAULT_TOLERANCE: usize = 1;
/// Minimum voting validators accepted by a frozen v2 height context.
pub const MIN_VOTING_ROSTER_LEN: usize = 3 * MIN_FAULT_TOLERANCE + 1;
/// Maximum Byzantine validators tolerated by one production committee.
pub const MAX_FAULT_TOLERANCE: usize = 10;
/// Maximum voting validators accepted by a frozen v2 height context.
pub const MAX_VOTING_ROSTER_LEN: usize = 3 * MAX_FAULT_TOLERANCE + 1;
/// Adjacent future timeout rounds retained for bounded pacemaker catch-up.
pub(crate) const FUTURE_TIMEOUT_VOTE_LOOKAHEAD: u64 = 1;

/// Whether a timeout-vote view belongs to the bounded current/future window.
pub(crate) const fn timeout_vote_view_is_admissible(current_view: u64, vote_view: u64) -> bool {
    vote_view >= current_view
        && vote_view <= current_view.saturating_add(FUTURE_TIMEOUT_VOTE_LOOKAHEAD)
}

macro_rules! fixed_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Constructs the value from its canonical 32-byte representation.
            #[must_use]
            pub const fn new(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Returns the canonical byte representation.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            /// Creates a deterministic value useful in fixtures and model traces.
            #[must_use]
            pub const fn repeat(byte: u8) -> Self {
                Self([byte; 32])
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}(", stringify!($name))?;
                for byte in &self.0[..4] {
                    write!(formatter, "{byte:02x}")?;
                }
                formatter.write_str("…)")
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                for byte in &self.0[..8] {
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
        }
    };
}

fixed_id!(Digest, "An opaque, canonical 32-byte digest.");
fixed_id!(
    ChainId,
    "The chain identifier bound into consensus messages."
);
fixed_id!(ContextId, "Digest identifying a frozen height context.");
fixed_id!(ValidatorId, "Canonical identifier of a voting validator.");
fixed_id!(Subject, "Digest identifying a proposed block body.");

/// Voting power assigned to one validator or represented by a signer set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VotingPower(u64);

impl VotingPower {
    /// Constructs a voting-power value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the integer voting power.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A voting validator frozen into a height context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Validator {
    id: ValidatorId,
    power: VotingPower,
}

impl Validator {
    /// Constructs a validator entry.
    #[must_use]
    pub const fn new(id: ValidatorId, power: VotingPower) -> Self {
        Self { id, power }
    }

    /// Returns the validator identifier.
    #[must_use]
    pub const fn id(self) -> ValidatorId {
        self.id
    }

    /// Returns the validator voting power.
    #[must_use]
    pub const fn power(self) -> VotingPower {
        self.power
    }
}

/// Voting-power interpretation selected by genesis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VotingMode {
    /// Every voting validator has power one.
    Permissioned,
    /// Stake selects the epoch-frozen committee; every member has one vote.
    Npos,
}

/// Height and view identifying a protocol round.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Round {
    height: u64,
    view: u64,
}

impl Round {
    /// Constructs a round.
    #[must_use]
    pub const fn new(height: u64, view: u64) -> Self {
        Self { height, view }
    }

    /// Returns the block height.
    #[must_use]
    pub const fn height(self) -> u64 {
        self.height
    }

    /// Returns the view within the height.
    #[must_use]
    pub const fn view(self) -> u64 {
        self.view
    }
}

/// Local incarnation counter used to reject stale asynchronous completions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(u64);

impl Generation {
    /// Initial generation for a fresh process/view ownership episode.
    ///
    /// The view in [`EventTag`] separates ordinary timeout-certificate view
    /// changes.  Only a strict lock upgrade for the already-installed timeout
    /// round increments this local counter.
    pub(crate) const INITIAL: Self = Self(0);

    /// Constructs a generation.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric generation.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(crate) fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// Tag attached to every adapter completion and authenticated input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventTag {
    height: u64,
    view: u64,
    generation: Generation,
}

impl EventTag {
    /// Constructs an event tag.
    #[must_use]
    pub const fn new(height: u64, view: u64, generation: Generation) -> Self {
        Self {
            height,
            view,
            generation,
        }
    }

    /// Returns the tagged height.
    #[must_use]
    pub const fn height(self) -> u64 {
        self.height
    }

    /// Returns the tagged view.
    #[must_use]
    pub const fn view(self) -> u64 {
        self.view
    }

    /// Returns the tagged local generation.
    #[must_use]
    pub const fn generation(self) -> Generation {
        self.generation
    }

    /// Whether this tag is a strictly later local incarnation at the same
    /// height.
    ///
    /// A second timeout certificate for the same timed-out round may install
    /// a strictly higher PrepareQC without changing the resulting view.  The
    /// generation therefore participates in lifecycle ownership independently
    /// when the view is unchanged.  An ordinary view advance starts a fresh
    /// generation-zero episode; the `(view, generation)` pair is ordered
    /// lexicographically so that reset cannot alias prior-view work.
    #[must_use]
    pub(crate) const fn strictly_advances(self, previous: Self) -> bool {
        self.height == previous.height
            && (self.view > previous.view
                || (self.view == previous.view && self.generation.0 > previous.generation.0))
    }
}

/// The two voting phases of the global Sumeragi v2 protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    /// Certifies that an exact body is durably available and valid.
    Prepare,
    /// Certifies finality for a prepared subject.
    Commit,
}

/// Stable identity of a quorum certificate, independent of its signature set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CertificateRef {
    context_id: ContextId,
    round: Round,
    proposal_round: Round,
    phase: Phase,
    subject: Subject,
}

impl CertificateRef {
    /// Constructs a certificate reference.
    #[must_use]
    pub const fn new(context_id: ContextId, round: Round, phase: Phase, subject: Subject) -> Self {
        Self::new_with_proposal_round(context_id, round, round, phase, subject)
    }

    /// Constructs a certificate reference with an explicit proposal round.
    /// Validation requires it to equal `round`; the explicit form is retained
    /// for canonical decode and adversarial fixtures.
    #[must_use]
    pub const fn new_with_proposal_round(
        context_id: ContextId,
        round: Round,
        proposal_round: Round,
        phase: Phase,
        subject: Subject,
    ) -> Self {
        Self {
            context_id,
            round,
            proposal_round,
            phase,
            subject,
        }
    }

    /// Returns the referenced height context.
    #[must_use]
    pub const fn context_id(self) -> ContextId {
        self.context_id
    }

    /// Returns the referenced round.
    #[must_use]
    pub const fn round(self) -> Round {
        self.round
    }

    /// Returns the proposal round, which equals the certified round when valid.
    #[must_use]
    pub const fn proposal_round(self) -> Round {
        self.proposal_round
    }

    /// Returns the referenced phase.
    #[must_use]
    pub const fn phase(self) -> Phase {
        self.phase
    }

    /// Returns the referenced subject.
    #[must_use]
    pub const fn subject(self) -> Subject {
        self.subject
    }

    /// Return whether two certificates concern one immutable body at one height.
    ///
    /// The view and phase are intentionally excluded so a Prepare lock and a
    /// same-body CommitQC from an earlier or later unchanged re-proposal can
    /// be related without weakening context, height, or subject identity.
    #[must_use]
    pub fn same_height_subject(self, other: Self) -> bool {
        certificate_height_subject_identity_equal_body!(
            self.context_id,
            self.round.height,
            self.subject,
            other.context_id,
            other.round.height,
            other.subject,
        )
    }

    /// Returns whether both references certify the same committed decision.
    ///
    /// An immutable body may acquire a CommitQC before or after unchanged
    /// re-proposal. The stable decision identity deliberately excludes the
    /// round while retaining its height-context, height, and subject.
    #[must_use]
    pub fn same_commit_decision(self, other: Self) -> bool {
        self.phase == Phase::Commit
            && other.phase == Phase::Commit
            && self.same_height_subject(other)
    }
}

/// Immutable consensus inputs for one block height.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeightContext {
    protocol_version: u16,
    id: ContextId,
    chain_id: ChainId,
    height: u64,
    parent_commit: Option<CertificateRef>,
    snapshot_bootstrap: bool,
    epoch: u64,
    roster: Vec<Validator>,
    total_voting_power: VotingPower,
    mode: VotingMode,
    nexus_amx_context_hash: Digest,
    execution_policy_hash: Digest,
    da_layout_hash: Digest,
    leader_seed: Digest,
}

impl HeightContext {
    /// Constructs and validates a frozen height context.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, oversized, unordered, zero-powered,
    /// overflowing, or mode-inconsistent roster, or for an invalid parent
    /// certificate.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: ContextId,
        chain_id: ChainId,
        height: u64,
        parent_commit: Option<CertificateRef>,
        epoch: u64,
        roster: Vec<Validator>,
        mode: VotingMode,
        nexus_amx_context_hash: Digest,
        execution_policy_hash: Digest,
        da_layout_hash: Digest,
        leader_seed: Digest,
    ) -> Result<Self, HeightContextError> {
        Self::new_inner(
            id,
            chain_id,
            height,
            parent_commit,
            false,
            epoch,
            roster,
            mode,
            nexus_amx_context_hash,
            execution_policy_hash,
            da_layout_hash,
            leader_seed,
        )
    }

    /// Construct the first context after an authenticated hash-only snapshot boundary.
    ///
    /// The wire adapter admits this constructor only after verifying the complete typed bootstrap
    /// record covered by the audited snapshot payload. No parent certificate is invented.
    ///
    /// # Errors
    ///
    /// Returns an error unless `height` is greater than one and the ordinary frozen-context
    /// invariants hold.
    #[allow(clippy::too_many_arguments)]
    pub fn new_snapshot_bootstrap(
        id: ContextId,
        chain_id: ChainId,
        height: u64,
        epoch: u64,
        roster: Vec<Validator>,
        mode: VotingMode,
        nexus_amx_context_hash: Digest,
        execution_policy_hash: Digest,
        da_layout_hash: Digest,
        leader_seed: Digest,
    ) -> Result<Self, HeightContextError> {
        Self::new_inner(
            id,
            chain_id,
            height,
            None,
            true,
            epoch,
            roster,
            mode,
            nexus_amx_context_hash,
            execution_policy_hash,
            da_layout_hash,
            leader_seed,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_inner(
        id: ContextId,
        chain_id: ChainId,
        height: u64,
        parent_commit: Option<CertificateRef>,
        snapshot_bootstrap: bool,
        epoch: u64,
        roster: Vec<Validator>,
        mode: VotingMode,
        nexus_amx_context_hash: Digest,
        execution_policy_hash: Digest,
        da_layout_hash: Digest,
        leader_seed: Digest,
    ) -> Result<Self, HeightContextError> {
        if roster.is_empty() {
            return Err(HeightContextError::EmptyRoster);
        }
        if roster.len() < MIN_VOTING_ROSTER_LEN {
            return Err(HeightContextError::RosterTooSmall);
        }
        if roster.len() > MAX_VOTING_ROSTER_LEN {
            return Err(HeightContextError::RosterTooLarge);
        }
        if (roster.len() - 1) % 3 != 0 {
            return Err(HeightContextError::InvalidCommitteeGeometry);
        }
        let mut total = 0_u64;
        let mut previous = None;
        for validator in &roster {
            if previous.is_some_and(|id| id >= validator.id) {
                return Err(HeightContextError::RosterNotStrictlyOrdered);
            }
            previous = Some(validator.id);
            if validator.power.get() == 0 {
                return Err(HeightContextError::ZeroVotingPower(validator.id));
            }
            if validator.power.get() != 1 {
                return Err(HeightContextError::VotingPowerNotOne(validator.id));
            }
            total = total
                .checked_add(validator.power.get())
                .ok_or(HeightContextError::VotingPowerOverflow)?;
        }
        match (height, parent_commit, snapshot_bootstrap) {
            (1, None, false) => {}
            (height, None, true) if height > 1 => {}
            (1, Some(_), _) | (0, _, _) | (_, None, false) | (_, Some(_), true) => {
                return Err(HeightContextError::InvalidParentCommit);
            }
            (_, None, true) => return Err(HeightContextError::InvalidParentCommit),
            (_, Some(parent), false)
                if parent.phase != Phase::Commit
                    || parent.round.height.checked_add(1) != Some(height)
                    || parent.proposal_round != parent.round =>
            {
                return Err(HeightContextError::InvalidParentCommit);
            }
            (_, Some(_), false) => {}
        }
        Ok(Self {
            protocol_version: PROTOCOL_VERSION_V4,
            id,
            chain_id,
            height,
            parent_commit,
            snapshot_bootstrap,
            epoch,
            roster,
            total_voting_power: VotingPower::new(total),
            mode,
            nexus_amx_context_hash,
            execution_policy_hash,
            da_layout_hash,
            leader_seed,
        })
    }

    /// Returns the protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    /// Returns the frozen context identifier.
    #[must_use]
    pub const fn id(&self) -> ContextId {
        self.id
    }

    /// Returns the chain identifier.
    #[must_use]
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    /// Returns the block height.
    #[must_use]
    pub const fn height(&self) -> u64 {
        self.height
    }

    /// Returns the parent `CommitQC` reference, if the height is not genesis.
    #[must_use]
    pub const fn parent_commit(&self) -> Option<CertificateRef> {
        self.parent_commit
    }

    /// Return whether an audited snapshot, rather than a parent CommitQC, anchors this height.
    #[must_use]
    pub const fn is_snapshot_bootstrap(&self) -> bool {
        self.snapshot_bootstrap
    }

    /// Returns the epoch containing this height.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Returns the canonically ordered voting roster.
    #[must_use]
    pub fn roster(&self) -> &[Validator] {
        &self.roster
    }

    /// Returns the selected voting mode.
    #[must_use]
    pub const fn mode(&self) -> VotingMode {
        self.mode
    }

    /// Returns the total voting power.
    #[must_use]
    pub const fn total_voting_power(&self) -> VotingPower {
        self.total_voting_power
    }

    /// Returns the strict two-thirds count threshold.
    #[must_use]
    pub const fn minimum_signer_count(&self) -> usize {
        self.roster.len() - (self.roster.len() - 1) / 3
    }

    /// Returns the frozen Nexus/AMX consensus-context commitment.
    #[must_use]
    pub const fn nexus_amx_context_hash(&self) -> Digest {
        self.nexus_amx_context_hash
    }

    /// Returns the frozen V1 boot execution-policy identity.
    #[must_use]
    pub const fn execution_policy_hash(&self) -> Digest {
        self.execution_policy_hash
    }

    /// Returns the deterministic data-availability layout commitment.
    #[must_use]
    pub const fn da_layout_hash(&self) -> Digest {
        self.da_layout_hash
    }

    /// Returns a roster entry by identifier.
    #[must_use]
    pub fn validator(&self, id: &ValidatorId) -> Option<Validator> {
        self.roster
            .binary_search_by_key(id, |validator| validator.id)
            .ok()
            .map(|index| self.roster[index])
    }

    /// Returns the expected leader for a view.
    ///
    /// `leader_seed` is the adapter-supplied
    /// `H(epoch_seed, height)` digest. Interpreting all 256 bits as a
    /// big-endian integer and reducing it modulo the roster length implements
    /// the protocol's exact deterministic rotation without depending on host
    /// integer width.
    #[must_use]
    pub fn leader(&self, view: u64) -> ValidatorId {
        let roster_len = self.roster.len() as u128;
        let start = self
            .leader_seed
            .as_bytes()
            .iter()
            .fold(0_u128, |remainder, byte| {
                (remainder * 256 + u128::from(*byte)) % roster_len
            });
        let start = usize::try_from(start).unwrap_or(0);
        let view_offset = usize::try_from(view % self.roster.len() as u64).unwrap_or(0);
        self.roster[(start + view_offset) % self.roster.len()].id
    }
}

/// Failure while constructing a frozen height context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeightContextError {
    /// A height context cannot operate without voting validators.
    EmptyRoster,
    /// A height context must tolerate at least one Byzantine validator.
    RosterTooSmall,
    /// The voting roster exceeds the first-release protocol bound.
    RosterTooLarge,
    /// The voting roster does not have exact `3f + 1` geometry.
    InvalidCommitteeGeometry,
    /// The roster contains duplicates or is not canonically ordered.
    RosterNotStrictlyOrdered,
    /// A voting validator has zero power.
    ZeroVotingPower(ValidatorId),
    /// Every consensus validator must have exactly one vote.
    VotingPowerNotOne(ValidatorId),
    /// The total voting power overflowed `u64`.
    VotingPowerOverflow,
    /// The parent reference is not a `CommitQC` for the preceding height.
    InvalidParentCommit,
}

impl fmt::Display for HeightContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRoster => formatter.write_str("height context has an empty roster"),
            Self::RosterTooSmall => {
                formatter.write_str("height context must contain at least four validators")
            }
            Self::RosterTooLarge => write!(
                formatter,
                "height context voting roster exceeds the protocol limit of {MAX_VOTING_ROSTER_LEN}"
            ),
            Self::InvalidCommitteeGeometry => {
                formatter.write_str("height context roster must contain exactly 3f + 1 validators")
            }
            Self::RosterNotStrictlyOrdered => {
                formatter.write_str("validator roster is not strictly ordered")
            }
            Self::ZeroVotingPower(id) => write!(formatter, "validator {id} has zero power"),
            Self::VotingPowerNotOne(id) => {
                write!(
                    formatter,
                    "consensus validator {id} does not have power one"
                )
            }
            Self::VotingPowerOverflow => formatter.write_str("total voting power overflow"),
            Self::InvalidParentCommit => formatter.write_str("invalid parent CommitQC reference"),
        }
    }
}

impl Error for HeightContextError {}

/// Opaque signature bytes produced and checked by the cryptographic adapter.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OpaqueSignature(Vec<u8>);

impl OpaqueSignature {
    /// Constructs opaque signature bytes.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Borrows the opaque bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// One validator signature included in a QC or TC.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignatureShare {
    signer: ValidatorId,
    signature: OpaqueSignature,
}

impl SignatureShare {
    /// Constructs a signature share.
    #[must_use]
    pub fn new(signer: ValidatorId, signature: OpaqueSignature) -> Self {
        Self { signer, signature }
    }

    /// Returns the signer.
    #[must_use]
    pub const fn signer(&self) -> ValidatorId {
        self.signer
    }

    /// Returns the opaque signature.
    #[must_use]
    pub const fn signature(&self) -> &OpaqueSignature {
        &self.signature
    }
}

/// Vote over one subject in one phase and round.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Vote {
    context_id: ContextId,
    round: Round,
    proposal_round: Round,
    phase: Phase,
    subject: Subject,
    signer: ValidatorId,
}

impl Vote {
    /// Constructs a vote.
    #[must_use]
    pub const fn new(
        context_id: ContextId,
        round: Round,
        phase: Phase,
        subject: Subject,
        signer: ValidatorId,
    ) -> Self {
        Self::new_with_proposal_round(context_id, round, round, phase, subject, signer)
    }

    /// Constructs a vote with an explicit proposal round. Validation requires
    /// it to equal `round`; the explicit form is retained for decode adapters
    /// and adversarial fixtures.
    #[must_use]
    pub const fn new_with_proposal_round(
        context_id: ContextId,
        round: Round,
        proposal_round: Round,
        phase: Phase,
        subject: Subject,
        signer: ValidatorId,
    ) -> Self {
        Self {
            context_id,
            round,
            proposal_round,
            phase,
            subject,
            signer,
        }
    }

    /// Returns the height context identifier.
    #[must_use]
    pub const fn context_id(self) -> ContextId {
        self.context_id
    }

    /// Returns the vote round.
    #[must_use]
    pub const fn round(self) -> Round {
        self.round
    }

    /// Returns the proposal round, which equals the vote round when valid.
    #[must_use]
    pub const fn proposal_round(self) -> Round {
        self.proposal_round
    }

    /// Returns the vote phase.
    #[must_use]
    pub const fn phase(self) -> Phase {
        self.phase
    }

    /// Returns the voted subject.
    #[must_use]
    pub const fn subject(self) -> Subject {
        self.subject
    }

    /// Returns the signer.
    #[must_use]
    pub const fn signer(self) -> ValidatorId {
        self.signer
    }

    /// Return whether two validators voted for the exact same signable statement.
    ///
    /// The authenticated signer is intentionally excluded: a quorum is formed
    /// from distinct validators signing one shared context, round, phase, and
    /// subject. This predicate does not validate either signer against a
    /// roster; authenticated ingress must do that before reducer admission.
    #[must_use]
    pub fn same_statement(self, other: Self) -> bool {
        vote_statement_identity_equal_body!(
            self.context_id,
            self.round.height,
            self.round.view,
            self.proposal_round.height,
            self.proposal_round.view,
            self.phase,
            self.subject,
            other.context_id,
            other.round.height,
            other.round.view,
            other.proposal_round.height,
            other.proposal_round.view,
            other.phase,
            other.subject,
        )
    }
}

/// Authenticated vote whose signature has already been checked by an adapter.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignedVote {
    vote: Vote,
    signature: OpaqueSignature,
}

impl SignedVote {
    /// Constructs an authenticated vote.
    #[must_use]
    pub fn new(vote: Vote, signature: OpaqueSignature) -> Self {
        Self { vote, signature }
    }

    /// Returns the signed vote body.
    #[must_use]
    pub const fn vote(&self) -> Vote {
        self.vote
    }

    /// Returns its opaque signature.
    #[must_use]
    pub const fn signature(&self) -> &OpaqueSignature {
        &self.signature
    }
}

/// A quorum certificate for a Prepare or Commit vote set.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuorumCertificate {
    reference: CertificateRef,
    signatures: Vec<SignatureShare>,
}

impl QuorumCertificate {
    /// Constructs a quorum certificate from canonically ordered signatures.
    #[must_use]
    pub fn new(reference: CertificateRef, signatures: Vec<SignatureShare>) -> Self {
        Self {
            reference,
            signatures,
        }
    }

    /// Returns the stable certificate reference.
    #[must_use]
    pub const fn reference(&self) -> CertificateRef {
        self.reference
    }

    /// Returns the certificate round.
    #[must_use]
    pub const fn round(&self) -> Round {
        self.reference.round
    }

    /// Returns the exact proposal round, equal to the certificate round.
    #[must_use]
    pub const fn proposal_round(&self) -> Round {
        self.reference.proposal_round
    }

    /// Returns the certificate phase.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.reference.phase
    }

    /// Returns the certified subject.
    #[must_use]
    pub const fn subject(&self) -> Subject {
        self.reference.subject
    }

    /// Returns the canonical signature shares.
    #[must_use]
    pub fn signatures(&self) -> &[SignatureShare] {
        &self.signatures
    }

    /// Validates the context, height, signer order, and both quorum thresholds.
    ///
    /// # Errors
    ///
    /// Returns an error if the certificate targets another context or height,
    /// has malformed signers, or fails either quorum threshold.
    pub fn validate(&self, context: &HeightContext) -> Result<Quorum, QuorumError> {
        if self.reference.context_id != context.id {
            return Err(QuorumError::ContextMismatch);
        }
        if self.reference.round.height != context.height {
            return Err(QuorumError::HeightMismatch);
        }
        if self.reference.proposal_round != self.reference.round {
            return Err(QuorumError::InvalidProposalRound);
        }
        let signers: Vec<_> = self.signatures.iter().map(SignatureShare::signer).collect();
        Quorum::require(context, &signers)
    }
}

/// Payload identity and deterministic data-availability layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PayloadManifest {
    subject: Subject,
    payload_hash: Digest,
    chunk_root: Digest,
    byte_len: u64,
    chunk_count: u32,
}

impl PayloadManifest {
    /// Constructs a payload manifest.
    #[must_use]
    pub const fn new(
        subject: Subject,
        payload_hash: Digest,
        chunk_root: Digest,
        byte_len: u64,
        chunk_count: u32,
    ) -> Self {
        Self {
            subject,
            payload_hash,
            chunk_root,
            byte_len,
            chunk_count,
        }
    }

    /// Returns the subject derived from the exact block body.
    #[must_use]
    pub const fn subject(&self) -> Subject {
        self.subject
    }

    /// Returns the full payload hash.
    #[must_use]
    pub const fn payload_hash(&self) -> Digest {
        self.payload_hash
    }

    /// Returns the chunk-layout Merkle root.
    #[must_use]
    pub const fn chunk_root(&self) -> Digest {
        self.chunk_root
    }

    /// Returns the exact encoded byte length.
    #[must_use]
    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    /// Returns the number of chunks.
    #[must_use]
    pub const fn chunk_count(&self) -> u32 {
        self.chunk_count
    }
}

/// One payload chunk transported outside the reducer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PayloadChunk {
    subject: Subject,
    index: u32,
    bytes: Vec<u8>,
    proof: Vec<Digest>,
}

impl PayloadChunk {
    /// Constructs a payload chunk and its Merkle proof.
    #[must_use]
    pub fn new(subject: Subject, index: u32, bytes: Vec<u8>, proof: Vec<Digest>) -> Self {
        Self {
            subject,
            index,
            bytes,
            proof,
        }
    }

    /// Returns the payload subject.
    #[must_use]
    pub const fn subject(&self) -> Subject {
        self.subject
    }

    /// Returns the zero-based chunk index.
    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// Returns the chunk bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the Merkle proof supplied by the transport adapter.
    #[must_use]
    pub fn proof(&self) -> &[Digest] {
        &self.proof
    }
}

/// Justification required by a proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalJustification {
    /// View zero is justified by the finalized parent certificate reference.
    ParentCommit(Option<CertificateRef>),
    /// A later view is justified by a timeout certificate for the prior view.
    Timeout(TimeoutCertificate),
}

/// A proposal from the deterministic leader.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    context_id: ContextId,
    round: Round,
    proposer: ValidatorId,
    manifest: PayloadManifest,
    justification: ProposalJustification,
}

impl Proposal {
    /// Constructs a proposal.
    #[must_use]
    pub fn new(
        context_id: ContextId,
        round: Round,
        proposer: ValidatorId,
        manifest: PayloadManifest,
        justification: ProposalJustification,
    ) -> Self {
        Self {
            context_id,
            round,
            proposer,
            manifest,
            justification,
        }
    }

    /// Returns the height context identifier.
    #[must_use]
    pub const fn context_id(&self) -> ContextId {
        self.context_id
    }

    /// Returns the proposal round.
    #[must_use]
    pub const fn round(&self) -> Round {
        self.round
    }

    /// Returns the proposer.
    #[must_use]
    pub const fn proposer(&self) -> ValidatorId {
        self.proposer
    }

    /// Returns the payload manifest.
    #[must_use]
    pub const fn manifest(&self) -> &PayloadManifest {
        &self.manifest
    }

    /// Returns the proposal justification.
    #[must_use]
    pub const fn justification(&self) -> &ProposalJustification {
        &self.justification
    }
}

/// Authenticated proposal received from the network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedProposal {
    proposal: Proposal,
    signature: OpaqueSignature,
}

impl SignedProposal {
    /// Constructs an authenticated proposal.
    #[must_use]
    pub fn new(proposal: Proposal, signature: OpaqueSignature) -> Self {
        Self {
            proposal,
            signature,
        }
    }

    /// Returns the proposal.
    #[must_use]
    pub const fn proposal(&self) -> &Proposal {
        &self.proposal
    }

    /// Returns the opaque signature.
    #[must_use]
    pub const fn signature(&self) -> &OpaqueSignature {
        &self.signature
    }
}

/// A durable timeout intent for one view.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeoutVote {
    context_id: ContextId,
    round: Round,
    signer: ValidatorId,
    highest_prepare: Option<QuorumCertificate>,
}

impl TimeoutVote {
    /// Constructs a timeout vote.
    #[must_use]
    pub fn new(
        context_id: ContextId,
        round: Round,
        signer: ValidatorId,
        highest_prepare: Option<QuorumCertificate>,
    ) -> Self {
        Self {
            context_id,
            round,
            signer,
            highest_prepare,
        }
    }

    /// Returns the height context identifier.
    #[must_use]
    pub const fn context_id(&self) -> ContextId {
        self.context_id
    }

    /// Returns the timed-out round.
    #[must_use]
    pub const fn round(&self) -> Round {
        self.round
    }

    /// Returns the signer.
    #[must_use]
    pub const fn signer(&self) -> ValidatorId {
        self.signer
    }

    /// Returns the highest durable `PrepareQC` observed by the signer.
    #[must_use]
    pub const fn highest_prepare(&self) -> Option<&QuorumCertificate> {
        self.highest_prepare.as_ref()
    }

    /// Returns the stable reference authenticated by this vote.
    #[must_use]
    pub fn highest_prepare_ref(&self) -> Option<CertificateRef> {
        self.highest_prepare
            .as_ref()
            .map(QuorumCertificate::reference)
    }
}

/// Authenticated timeout vote.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignedTimeoutVote {
    vote: TimeoutVote,
    signature: OpaqueSignature,
}

impl SignedTimeoutVote {
    /// Constructs an authenticated timeout vote.
    #[must_use]
    pub fn new(vote: TimeoutVote, signature: OpaqueSignature) -> Self {
        Self { vote, signature }
    }

    /// Returns the timeout vote.
    #[must_use]
    pub fn vote(&self) -> TimeoutVote {
        self.vote.clone()
    }

    /// Returns the opaque signature.
    #[must_use]
    pub const fn signature(&self) -> &OpaqueSignature {
        &self.signature
    }
}

/// Timeout signatures grouped by the stable identity of the reported high QC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeoutSignatureGroup {
    highest_prepare: Option<QuorumCertificate>,
    signatures: Vec<SignatureShare>,
}

impl TimeoutSignatureGroup {
    /// Constructs a timeout-signature group.
    #[must_use]
    pub fn new(
        highest_prepare: Option<QuorumCertificate>,
        signatures: Vec<SignatureShare>,
    ) -> Self {
        Self {
            highest_prepare,
            signatures,
        }
    }

    /// Returns the full `PrepareQC` reported by this group, if any.
    #[must_use]
    pub const fn highest_prepare(&self) -> Option<&QuorumCertificate> {
        self.highest_prepare.as_ref()
    }

    /// Returns the stable identity signed by members of this group.
    #[must_use]
    pub fn highest_prepare_ref(&self) -> Option<CertificateRef> {
        self.highest_prepare
            .as_ref()
            .map(QuorumCertificate::reference)
    }

    /// Returns the canonical timeout signature shares.
    #[must_use]
    pub fn signatures(&self) -> &[SignatureShare] {
        &self.signatures
    }
}

/// Certificate proving that an equal-vote quorum timed out one view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeoutCertificate {
    context_id: ContextId,
    round: Round,
    groups: Vec<TimeoutSignatureGroup>,
}

impl TimeoutCertificate {
    /// Constructs a timeout certificate.
    #[must_use]
    pub fn new(context_id: ContextId, round: Round, groups: Vec<TimeoutSignatureGroup>) -> Self {
        Self {
            context_id,
            round,
            groups,
        }
    }

    /// Returns the height context identifier.
    #[must_use]
    pub const fn context_id(&self) -> ContextId {
        self.context_id
    }

    /// Returns the timed-out round.
    #[must_use]
    pub const fn round(&self) -> Round {
        self.round
    }

    /// Returns all high-QC signature groups.
    #[must_use]
    pub fn groups(&self) -> &[TimeoutSignatureGroup] {
        &self.groups
    }

    /// Returns the deterministically highest `PrepareQC` carried by the groups.
    #[must_use]
    pub fn highest_prepare(&self) -> Option<&QuorumCertificate> {
        self.groups
            .iter()
            .filter_map(TimeoutSignatureGroup::highest_prepare)
            .max_by_key(|certificate| (certificate.round().view(), certificate.subject()))
    }

    /// Validates nested `PrepareQC`s, canonical group ordering, signer
    /// disjointness, and the union's equal-vote quorum.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid nested certificates, unordered or
    /// overlapping groups, conflicting maxima, or an insufficient signer union.
    pub fn validate(&self, context: &HeightContext) -> Result<Quorum, QuorumError> {
        use std::collections::BTreeSet;

        if self.context_id != context.id() {
            return Err(QuorumError::ContextMismatch);
        }
        if self.round.height() != context.height() {
            return Err(QuorumError::HeightMismatch);
        }
        let mut previous_group = None;
        let mut all_signers = BTreeSet::new();
        let mut highest_at_view: Option<(u64, Subject)> = None;
        for group in &self.groups {
            let group_ref = group.highest_prepare_ref();
            if previous_group.is_some_and(|previous| previous >= group_ref) {
                return Err(QuorumError::TimeoutGroupsNotStrictlyOrdered);
            }
            previous_group = Some(group_ref);
            if let Some(certificate) = group.highest_prepare() {
                if certificate.phase() != Phase::Prepare {
                    return Err(QuorumError::InvalidPhase);
                }
                certificate.validate(context)?;
                if certificate.round().view() > self.round.view() {
                    return Err(QuorumError::HighestPrepareFromFuture);
                }
                let candidate = (certificate.round().view(), certificate.subject());
                if let Some((highest_view, highest_subject)) = highest_at_view {
                    if candidate.0 == highest_view && candidate.1 != highest_subject {
                        return Err(QuorumError::ConflictingHighestPrepare);
                    }
                    if candidate.0 > highest_view {
                        highest_at_view = Some(candidate);
                    }
                } else {
                    highest_at_view = Some(candidate);
                }
            }
            let signers: Vec<_> = group
                .signatures
                .iter()
                .map(SignatureShare::signer)
                .collect();
            Quorum::calculate(context, &signers)?;
            for signer in signers {
                if !all_signers.insert(signer) {
                    return Err(QuorumError::OverlappingTimeoutSigner(signer));
                }
            }
        }
        let ordered: Vec<_> = all_signers.into_iter().collect();
        Quorum::require(context, &ordered)
    }
}

/// Versioned Sumeragi v2 network message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsensusMessageV2 {
    /// Signed proposal.
    Proposal(SignedProposal),
    /// Signed Prepare or Commit vote.
    Vote(SignedVote),
    /// `PrepareQC` or `CommitQC`.
    QuorumCertificate(QuorumCertificate),
    /// Signed timeout vote.
    TimeoutVote(SignedTimeoutVote),
    /// Timeout certificate.
    TimeoutCertificate(TimeoutCertificate),
    /// Certified body request addressed by subject.
    BodyRequest(Subject),
    /// One body chunk returned by the transport adapter.
    BodyChunk(PayloadChunk),
}

#[cfg(test)]
mod tests {
    use super::{EventTag, Generation};

    #[test]
    fn event_tag_advance_is_lexicographic_within_one_height() {
        let previous = EventTag::new(7, 3, Generation::new(u64::MAX));
        let later_view = EventTag::new(7, 4, Generation::INITIAL);
        assert!(later_view.strictly_advances(previous));

        let same_view_previous = EventTag::new(7, 4, Generation::new(8));
        let same_view_upgrade = EventTag::new(7, 4, Generation::new(9));
        assert!(same_view_upgrade.strictly_advances(same_view_previous));

        assert!(!same_view_previous.strictly_advances(same_view_previous));
        assert!(
            !EventTag::new(7, 3, Generation::new(u64::MAX)).strictly_advances(same_view_previous)
        );
        assert!(!EventTag::new(8, 5, Generation::new(10)).strictly_advances(same_view_previous));
    }
}
