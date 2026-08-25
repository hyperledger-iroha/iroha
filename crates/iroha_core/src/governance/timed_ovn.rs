//! Replayable, public-only evidence for folded timed Parliament ballots.
//!
//! Authoritative persistence stores the exact canonical registration and
//! ballot wire records, the ordered frozen survivor identities, the complete
//! future-release identity, and replay-derived aggregate transcript. Restore
//! reparses every point/scalar, re-verifies every proof, rebuilds both roster
//! roots, and re-aggregates the exact ballot corpus. Cached roots, aggregates,
//! release terms, and tallies are comparisons only; callers cannot assert them.
//!
//! This module has no secret fields, private-share codec, individual opening,
//! plaintext fallback, or post-freeze recovery path. The only opening API
//! consumes a verified final threshold release and invokes the crypto crate's
//! aggregate-only tally operation.

use iroha_crypto::{
    timed_ovn::{
        G2Bytes, GtBytes, TIMED_OVN_CHOICE_COUNT_V1, TIMED_OVN_MAX_PARTICIPANTS_V1,
        TimedOvnAggregateV1, TimedOvnError, TimedOvnMaskedBallotV1, TimedOvnRegistrationV1,
        TimedOvnRosterV1, TimedOvnSessionV1, TimedOvnSurvivorRosterV1, TimedOvnTallyV1,
        aggregate_timed_ovn_ballots_v1,
    },
    tle::{TleError, TleMasterPublicKey, TleReleaseIdentityV1},
};
use iroha_data_model::{
    governance::types::TleKeySessionId,
    parliament_casting::ParliamentTimedOvnRegistrationCorpusCommitmentV1,
};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    derive::{JsonDeserialize, JsonSerialize},
};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::tle_release::{
    TleFinalReleaseSignatureV1, TleReleaseAdapterError, ValidatedTleKeySessionV1,
};

/// Fixed version of the public timed-OVN evidence format.
pub const TIMED_OVN_EVIDENCE_VERSION_V1: u16 = 1;
/// Exact canonical byte length of one timed-OVN registration record.
pub const TIMED_OVN_REGISTRATION_RECORD_BYTES_V1: usize = 3_624;
/// Exact canonical byte length of one timed-OVN masked-ballot record.
pub const TIMED_OVN_BALLOT_RECORD_BYTES_V1: usize = 2_858;

/// Derive the canonical fixed-suite parameter digest for every timed-OVN v1 attempt.
///
/// Core constructors use this local value; lifecycle wire payloads never get
/// to select or override the active cryptographic parameter profile.
#[must_use]
pub fn timed_ovn_parameter_hash_v1() -> [u8; 32] {
    iroha_crypto::timed_ovn::timed_ovn_parameter_hash_v1()
}

const AGGREGATE_TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.parliament.timed-ovn.aggregate-transcript.v1\0";
const BALLOT_CORPUS_DOMAIN_V1: &[u8] = b"iroha.parliament.timed-ovn.ballot-corpus.v1\0";
const DROPOUT_DECISIONS_DOMAIN_V1: &[u8] =
    b"iroha.parliament.timed-ovn.pre-ballot-dropout-decisions.v1\0";
const OPENING_TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.parliament.timed-ovn.aggregate-opening-transcript.v1\0";
const NO_RECOVERY_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.parliament.timed-ovn.no-post-freeze-recovery.v1\0";

/// Public bindings needed to reconstruct one timed-OVN cryptographic session.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
pub struct TimedOvnSessionPublicV1 {
    /// Canonical network/genesis binding.
    pub network_id: [u8; 32],
    /// Content identifier of the proposal being decided.
    pub proposal_content_id: [u8; 32],
    /// Governance lifecycle-attempt binding.
    pub governance_attempt_id: [u8; 32],
    /// Governed-body instance binding.
    pub body_instance_id: [u8; 32],
    /// Retryable ballot-attempt binding.
    pub ballot_attempt_id: [u8; 32],
    /// Commitment to the exact ballot/proof parameter profile.
    pub parameter_hash: [u8; 32],
    /// Long-lived, purpose-distinct TLE threshold key session.
    pub tle_key_session_id: TleKeySessionId,
    /// Complete verified adaptive TLE DKG transcript binding.
    pub tle_key_transcript_hash: [u8; 32],
    /// Canonical compressed TLE threshold group public key in G2.
    pub tle_master_public_key: [u8; 96],
}

impl TimedOvnSessionPublicV1 {
    /// Reconstruct the exact cryptographic timed-OVN session after checking
    /// the complete public TLE key and transcript binding.
    pub(crate) fn rebuild(
        &self,
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<TimedOvnSessionV1, TimedOvnEvidenceError> {
        if is_zero(self.tle_key_session_id.as_bytes())
            || self.tle_key_session_id != tle_key_session.public_state().key_session_id
            || self.tle_key_transcript_hash != tle_key_session.public_state().transcript_hash
            || self.tle_master_public_key != *tle_key_session.master_public_key().as_bytes()
        {
            return Err(TimedOvnEvidenceError::TleKeySessionMismatch);
        }
        let master_public_key = TleMasterPublicKey::from_bytes(
            self.tle_key_session_id.into_bytes(),
            &self.tle_master_public_key,
        )?;
        Ok(TimedOvnSessionV1::new(
            self.network_id,
            self.proposal_content_id,
            self.governance_attempt_id,
            self.body_instance_id,
            self.ballot_attempt_id,
            self.parameter_hash,
            master_public_key,
        )?)
    }
}

/// Public future-release identity used by the folded ballot relation.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
pub struct TimedOvnReleaseIdentityPublicV1 {
    /// Long-lived TLE threshold key session.
    pub tle_key_session_id: TleKeySessionId,
    /// Governance lifecycle-attempt binding.
    pub governance_attempt_id: [u8; 32],
    /// Governed-body instance binding.
    pub body_instance_id: [u8; 32],
    /// Retryable ballot-attempt binding.
    pub ballot_attempt_id: [u8; 32],
    /// Replay-derived root of the exact frozen survivor corpus.
    pub survivor_corpus_root: [u8; 32],
    /// Replay-derived sentinel committing to the absence of post-freeze recovery.
    pub no_recovery_root: [u8; 32],
    /// Finalized height before which partial releases are rejected.
    pub target_finalized_height: u64,
    /// Commitment to the exact ballot/proof parameter profile.
    pub parameter_hash: [u8; 32],
}

impl TimedOvnReleaseIdentityPublicV1 {
    fn rebuild(
        &self,
        session: &TimedOvnSessionPublicV1,
        survivor_root: &[u8; 32],
        expected_no_recovery_root: &[u8; 32],
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<TleReleaseIdentityV1, TimedOvnEvidenceError> {
        if self.tle_key_session_id != session.tle_key_session_id
            || self.tle_key_session_id != tle_key_session.public_state().key_session_id
            || self.governance_attempt_id != session.governance_attempt_id
            || self.body_instance_id != session.body_instance_id
            || self.ballot_attempt_id != session.ballot_attempt_id
            || self.survivor_corpus_root != *survivor_root
            || self.no_recovery_root != *expected_no_recovery_root
            || self.parameter_hash != session.parameter_hash
        {
            return Err(TimedOvnEvidenceError::ReleaseIdentityMismatch);
        }
        Ok(TleReleaseIdentityV1::new(
            *tle_key_session.transcript().session(),
            self.governance_attempt_id,
            self.body_instance_id,
            self.ballot_attempt_id,
            self.survivor_corpus_root,
            // `TleReleaseIdentityV1` retains a generic compatibility slot for
            // protocols that have a recovery corpus. Timed OVN has none: the
            // only value admitted here is the replay-derived no-recovery sentinel.
            self.no_recovery_root,
            self.target_finalized_height,
            self.parameter_hash,
        )?)
    }
}

/// Replay-derived roots available before the release identity is frozen.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
pub struct TimedOvnProspectiveRootsV1 {
    /// Root of the complete canonical registration roster.
    pub registration_roster_root: [u8; 32],
    /// Root of one keep/drop decision for every canonical registration.
    pub dropout_root: [u8; 32],
    /// Root of the exact canonical survivor subsequence.
    pub survivor_corpus_root: [u8; 32],
    /// Sentinel binding that proves this suite has no post-freeze recovery corpus.
    pub no_recovery_root: [u8; 32],
}

/// Public registration-open state for one timed-OVN ballot attempt.
///
/// Registrations are accumulated one authenticated seated member at a time.
/// The two heights freeze the release schedule so a later transition cannot
/// move the release earlier than the schedule admitted when registration
/// opened.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct TimedOvnRegistrationOpenStateV1 {
    version: u16,
    session: TimedOvnSessionPublicV1,
    registration_opened_at_finalized_height: u64,
    target_finalized_height: u64,
    registration_records: Vec<Vec<u8>>,
    registration_corpus_commitment: ParliamentTimedOvnRegistrationCorpusCommitmentV1,
}

impl TimedOvnRegistrationOpenStateV1 {
    /// Borrow the immutable timed-ballot session binding.
    #[must_use]
    pub const fn session(&self) -> &TimedOvnSessionPublicV1 {
        &self.session
    }

    /// Return the finalized height at which registration opened.
    #[must_use]
    pub const fn registration_opened_at_finalized_height(&self) -> u64 {
        self.registration_opened_at_finalized_height
    }

    /// Return the first finalized height at which threshold release is valid.
    #[must_use]
    pub const fn target_finalized_height(&self) -> u64 {
        self.target_finalized_height
    }

    /// Borrow the proof-validated records accumulated in participant order.
    #[must_use]
    pub fn registration_records(&self) -> &[Vec<u8>] {
        &self.registration_records
    }

    /// Return the transition-maintained commitment to the exact registration bytes.
    #[must_use]
    pub const fn registration_corpus_commitment(
        &self,
    ) -> &ParliamentTimedOvnRegistrationCorpusCommitmentV1 {
        &self.registration_corpus_commitment
    }

    /// Rebuild and validate the immutable session and release schedule.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for a wrong version, non-future
    /// release schedule, or mismatched TLE key/transcript.
    pub fn validate(
        &self,
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<TimedOvnSessionV1, TimedOvnEvidenceError> {
        if self.version != TIMED_OVN_EVIDENCE_VERSION_V1 {
            return Err(TimedOvnEvidenceError::UnsupportedVersion);
        }
        if self.target_finalized_height <= self.registration_opened_at_finalized_height {
            return Err(TimedOvnEvidenceError::InvalidReleaseSchedule);
        }
        let session = self.session.rebuild(tle_key_session)?;
        if !self
            .registration_corpus_commitment
            .matches_records(&self.registration_records)
        {
            return Err(TimedOvnEvidenceError::ReplayMismatch);
        }
        if !self.registration_records.is_empty() {
            rebuild_roster(&self.session, &self.registration_records, tle_key_session)?;
        }
        Ok(session)
    }
}

/// Public registration-closed state retaining the exact canonical roster wires.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct TimedOvnRegistrationClosedStateV1 {
    registration: TimedOvnRegistrationOpenStateV1,
    dropout_participant_hashes: Vec<[u8; 32]>,
}

impl TimedOvnRegistrationClosedStateV1 {
    /// Borrow the registration-open schedule and immutable session bindings.
    #[must_use]
    pub const fn registration(&self) -> &TimedOvnRegistrationOpenStateV1 {
        &self.registration
    }

    /// Borrow the exact canonical registration wire corpus.
    #[must_use]
    pub fn registration_records(&self) -> &[Vec<u8>] {
        &self.registration.registration_records
    }

    /// Borrow the ordered authenticated pre-ballot dropout decisions.
    #[must_use]
    pub fn dropout_participant_hashes(&self) -> &[[u8; 32]] {
        &self.dropout_participant_hashes
    }

    /// Reparse every exact registration and rebuild the canonical roster.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for an invalid schedule, malformed
    /// registration, duplicate participant, or reordered/oversized corpus.
    pub fn validate(
        &self,
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<(TimedOvnSessionV1, TimedOvnRosterV1), TimedOvnEvidenceError> {
        self.registration.validate(tle_key_session)?;
        let rebuilt = rebuild_roster(
            self.registration.session(),
            &self.registration.registration_records,
            tle_key_session,
        )?;
        validate_dropout_participant_hashes(&rebuilt.1, &self.dropout_participant_hashes)?;
        Ok(rebuilt)
    }
}

/// Public survivor-frozen state with its replay-derived future release identity.
///
/// The release identity is constructed only by
/// [`TimedOvnLifecycleStateV1::freeze_survivors`]. Its survivor root and
/// no-recovery sentinel are comparisons against a fresh replay during restore.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct TimedOvnSurvivorsFrozenStateV1 {
    registration: TimedOvnRegistrationClosedStateV1,
    survivor_participant_hashes: Vec<[u8; 32]>,
    dropout_root: [u8; 32],
    release_identity: TimedOvnReleaseIdentityPublicV1,
}

impl TimedOvnSurvivorsFrozenStateV1 {
    /// Borrow the registration-closed state and its exact roster wires.
    #[must_use]
    pub const fn registration(&self) -> &TimedOvnRegistrationClosedStateV1 {
        &self.registration
    }

    /// Borrow the exact ordered survivor subsequence.
    #[must_use]
    pub fn survivor_participant_hashes(&self) -> &[[u8; 32]] {
        &self.survivor_participant_hashes
    }

    /// Return the replay-derived pre-ballot dropout-decision root.
    #[must_use]
    pub const fn dropout_root(&self) -> &[u8; 32] {
        &self.dropout_root
    }

    /// Borrow the replay-derived future release identity.
    #[must_use]
    pub const fn release_identity(&self) -> &TimedOvnReleaseIdentityPublicV1 {
        &self.release_identity
    }

    /// Rebuild the roster, survivor subsequence, and future release identity.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for any invalid registration,
    /// survivor, TLE-key, release-root, or no-recovery binding.
    pub fn validate(
        &self,
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<PreparedTimedOvnAttemptV1, TimedOvnEvidenceError> {
        let (_, roster) = self.registration.validate(tle_key_session)?;
        let expected_survivors = roster
            .registrations()
            .iter()
            .filter_map(|record| {
                self.registration
                    .dropout_participant_hashes
                    .binary_search(record.participant_hash())
                    .is_err()
                    .then_some(*record.participant_hash())
            })
            .collect::<Vec<_>>();
        if expected_survivors != self.survivor_participant_hashes {
            return Err(TimedOvnEvidenceError::ReplayMismatch);
        }
        let prepared = PreparedTimedOvnAttemptV1::from_records(
            *self.registration.registration.session(),
            &self.registration.registration.registration_records,
            &self.survivor_participant_hashes,
            self.release_identity,
            tle_key_session,
        )?;
        if dropout_decisions_root(
            &prepared.session,
            &prepared.roster,
            &self.survivor_participant_hashes,
        ) != self.dropout_root
        {
            return Err(TimedOvnEvidenceError::ReplayMismatch);
        }
        Ok(prepared)
    }
}

/// Replay-derived public aggregate transcript for one exact ballot corpus.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct TimedOvnAggregateTranscriptV1 {
    /// Digest of every immutable timed-OVN session binding.
    pub session_digest: [u8; 32],
    /// Root of the complete canonical registration roster.
    pub registration_roster_root: [u8; 32],
    /// Root of one keep/drop decision for every canonical registration.
    pub dropout_root: [u8; 32],
    /// Root of the exact frozen survivor corpus.
    pub survivor_corpus_root: [u8; 32],
    /// SHA-256 of the exact typed future release message.
    pub identity_digest: [u8; 32],
    /// Intrinsic target-group release term `e(H(identity), P_TLE)`.
    pub release_term: GtBytes,
    /// Commitment to the exact ordered canonical ballot wire corpus.
    pub ballot_corpus_hash: [u8; 32],
    /// Exact number of accepted survivor ballots.
    pub accepted_ballots: u16,
    /// Three aggregate G2 ephemerals; canonical identity values are permitted.
    #[norito(with = "timed_ovn_g2_choice_array_json")]
    pub aggregate_ephemerals: [G2Bytes; TIMED_OVN_CHOICE_COUNT_V1],
    /// Three sealed aggregate GT commitments; canonical identity values are permitted.
    #[norito(with = "timed_ovn_gt_choice_array_json")]
    pub aggregate_commitments: [GtBytes; TIMED_OVN_CHOICE_COUNT_V1],
    /// Domain-separated commitment to every preceding aggregate field.
    pub transcript_hash: [u8; 32],
}

#[cfg(feature = "json")]
mod timed_ovn_g2_choice_array_json {
    use super::*;
    use norito::json::{self, JsonDeserialize as _, JsonSerialize as _, Parser};

    pub fn serialize(value: &[G2Bytes; TIMED_OVN_CHOICE_COUNT_V1], out: &mut String) {
        value.to_vec().json_serialize(out);
    }

    pub fn deserialize(
        parser: &mut Parser<'_>,
    ) -> Result<[G2Bytes; TIMED_OVN_CHOICE_COUNT_V1], json::Error> {
        Vec::<G2Bytes>::json_deserialize(parser)?
            .try_into()
            .map_err(|values: Vec<G2Bytes>| {
                json::Error::Message(format!(
                    "expected exactly {TIMED_OVN_CHOICE_COUNT_V1} timed-OVN G2 values, got {}",
                    values.len()
                ))
            })
    }
}

#[cfg(feature = "json")]
mod timed_ovn_gt_choice_array_json {
    use super::*;
    use norito::json::{self, JsonDeserialize as _, JsonSerialize as _, Parser};

    pub fn serialize(value: &[GtBytes; TIMED_OVN_CHOICE_COUNT_V1], out: &mut String) {
        value.to_vec().json_serialize(out);
    }

    pub fn deserialize(
        parser: &mut Parser<'_>,
    ) -> Result<[GtBytes; TIMED_OVN_CHOICE_COUNT_V1], json::Error> {
        Vec::<GtBytes>::json_deserialize(parser)?
            .try_into()
            .map_err(|values: Vec<GtBytes>| {
                json::Error::Message(format!(
                    "expected exactly {TIMED_OVN_CHOICE_COUNT_V1} timed-OVN GT values, got {}",
                    values.len()
                ))
            })
    }
}

impl TimedOvnAggregateTranscriptV1 {
    fn replay(
        session: &TimedOvnSessionV1,
        survivors: &TimedOvnSurvivorRosterV1,
        identity: &TleReleaseIdentityV1,
        ballots: &[Vec<u8>],
        aggregate: &TimedOvnAggregateV1,
        dropout_root: [u8; 32],
    ) -> Result<Self, TimedOvnEvidenceError> {
        let identity_digest: [u8; 32] = Sha256::digest(identity.release_message()?).into();
        let ballot_corpus_hash = ballot_corpus_hash(ballots)?;
        let mut replay = Self {
            session_digest: session.digest(),
            registration_roster_root: *survivors.roster_root(),
            dropout_root,
            survivor_corpus_root: *survivors.survivor_root(),
            identity_digest,
            release_term: *survivors.release_term(),
            ballot_corpus_hash,
            accepted_ballots: aggregate.accepted_ballots(),
            aggregate_ephemerals: *aggregate.aggregate_ephemerals(),
            aggregate_commitments: *aggregate.aggregate_commitments(),
            transcript_hash: [0; 32],
        };
        replay.transcript_hash = replay.compute_hash();
        Ok(replay)
    }

    fn compute_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(AGGREGATE_TRANSCRIPT_DOMAIN_V1);
        hasher.update(TIMED_OVN_EVIDENCE_VERSION_V1.to_be_bytes());
        hasher.update(self.session_digest);
        hasher.update(self.registration_roster_root);
        hasher.update(self.dropout_root);
        hasher.update(self.survivor_corpus_root);
        hasher.update(self.identity_digest);
        hasher.update(self.release_term);
        hasher.update(self.ballot_corpus_hash);
        hasher.update(self.accepted_ballots.to_be_bytes());
        for ephemeral in self.aggregate_ephemerals {
            hasher.update(ephemeral);
        }
        for commitment in self.aggregate_commitments {
            hasher.update(commitment);
        }
        hasher.finalize().into()
    }
}

/// Public-only persisted evidence for a complete sealed timed-OVN ballot corpus.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct TimedOvnEvidenceStateV1 {
    /// Fixed evidence format version.
    pub version: u16,
    /// Complete immutable timed-ballot session bindings.
    pub session: TimedOvnSessionPublicV1,
    /// Exact canonical registration records, already in participant order.
    pub registration_records: Vec<Vec<u8>>,
    /// Nonempty canonical subsequence of registered participant hashes.
    pub survivor_participant_hashes: Vec<[u8; 32]>,
    /// Exact future threshold-release identity.
    pub release_identity: TimedOvnReleaseIdentityPublicV1,
    /// Exact canonical ballot records, one per survivor in the same order.
    pub ballot_records: Vec<Vec<u8>>,
    /// Replay-derived roots and aggregate transcript.
    pub aggregate: TimedOvnAggregateTranscriptV1,
}

impl TimedOvnEvidenceStateV1 {
    /// Verify the bounded public release statement before replaying either corpus.
    ///
    /// This pregate intentionally reads only fixed-size session, release-identity,
    /// TLE transcript, height, and final-signature fields. It prevents an invalid
    /// permissionless final release from forcing proof verification across the
    /// registration and masked-ballot corpora. Full replay and a second release
    /// verification remain mandatory after this check succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for a wrong evidence version, public
    /// key/session/identity binding, early release, digest, or threshold signature.
    fn verify_final_release_pregate(
        &self,
        tle_key_session: &ValidatedTleKeySessionV1,
        finalized_height: u64,
        final_release: &TleFinalReleaseSignatureV1,
    ) -> Result<(), TimedOvnEvidenceError> {
        if self.version != TIMED_OVN_EVIDENCE_VERSION_V1 {
            return Err(TimedOvnEvidenceError::UnsupportedVersion);
        }
        let _ = self.session.rebuild(tle_key_session)?;
        let identity = self.release_identity.rebuild(
            &self.session,
            &self.release_identity.survivor_corpus_root,
            &self.release_identity.no_recovery_root,
            tle_key_session,
        )?;
        tle_key_session.verify_final_release(&identity, finalized_height, final_release)?;
        Ok(())
    }

    /// Rebuild and reverify every public cryptographic object from exact wires.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for an oversized corpus, malformed or
    /// reordered record, failed proof, wrong TLE session, or cached replay mismatch.
    pub fn validate(
        self,
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<ValidatedTimedOvnEvidenceV1, TimedOvnEvidenceError> {
        if self.version != TIMED_OVN_EVIDENCE_VERSION_V1 {
            return Err(TimedOvnEvidenceError::UnsupportedVersion);
        }
        let prepared = PreparedTimedOvnAttemptV1::from_records(
            self.session,
            &self.registration_records,
            &self.survivor_participant_hashes,
            self.release_identity,
            tle_key_session,
        )?;
        let replayed = prepared.admit_ballot_corpus(&self.ballot_records)?;
        if replayed.state != self {
            return Err(TimedOvnEvidenceError::ReplayMismatch);
        }
        Ok(replayed)
    }
}

/// Public tally derived only after a valid threshold release opens the aggregate.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
pub struct TimedOvnPublicTallyV1 {
    /// Number of Aye ballots.
    pub aye: u16,
    /// Number of Nay ballots.
    pub nay: u16,
    /// Number of Abstain ballots.
    pub abstain: u16,
}

impl From<TimedOvnTallyV1> for TimedOvnPublicTallyV1 {
    fn from(value: TimedOvnTallyV1) -> Self {
        Self {
            aye: value.aye,
            nay: value.nay,
            abstain: value.abstain,
        }
    }
}

/// Public-only persisted evidence after the exact threshold release is finalized.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct TimedOvnReleasedEvidenceV1 {
    /// Fixed evidence format version.
    pub version: u16,
    /// Exact sealed ballot evidence and replay-derived aggregate.
    pub sealed: TimedOvnEvidenceStateV1,
    /// Unique standard BLS final release signature; no signer bitmap.
    pub final_release: TleFinalReleaseSignatureV1,
    /// Aggregate-only replay-derived tally.
    pub tally: TimedOvnPublicTallyV1,
    /// Root of the final release, sealed aggregate, and aggregate-only tally.
    pub opening_root: [u8; 32],
}

impl TimedOvnReleasedEvidenceV1 {
    /// Return the replay-derived aggregate-only opening transcript root.
    #[must_use]
    pub const fn opening_root(&self) -> &[u8; 32] {
        &self.opening_root
    }

    /// Replay all sealed evidence, verify the release, and recompute the tally.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for any proof, release, aggregate, or
    /// cached-tally mismatch.
    pub fn validate(
        self,
        tle_key_session: &ValidatedTleKeySessionV1,
        finalized_height: u64,
    ) -> Result<ValidatedTimedOvnReleasedEvidenceV1, TimedOvnEvidenceError> {
        if self.version != TIMED_OVN_EVIDENCE_VERSION_V1 {
            return Err(TimedOvnEvidenceError::UnsupportedVersion);
        }
        self.sealed.verify_final_release_pregate(
            tle_key_session,
            finalized_height,
            &self.final_release,
        )?;
        let sealed = self.sealed.clone().validate(tle_key_session)?;
        let replayed =
            sealed.finalize_release(tle_key_session, finalized_height, self.final_release)?;
        if replayed.state != self {
            return Err(TimedOvnEvidenceError::ReplayMismatch);
        }
        Ok(replayed)
    }
}

/// Bounded public phase of one timed-OVN lifecycle.
///
/// This projection deliberately carries no registration corpus,
/// masked ballot, release share, or aggregate-opening secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimedOvnLifecyclePhaseV1 {
    /// Participant registration is open.
    Registered,
    /// Registration has closed with an authenticated public roster.
    RegistrationClosed,
    /// The survivor subsequence and release identity are frozen.
    SurvivorsFrozen,
    /// The complete aggregate-only encrypted ballot corpus is sealed.
    Sealed,
    /// A unique threshold release opened the aggregate tally.
    Released,
}

/// Single authoritative public lifecycle for one timed-OVN ballot attempt.
///
/// Each transition consumes the preceding state. Consequently, persistence
/// cannot contain simultaneous sealed/released records or skip the
/// registration and survivor-freeze checks. Decoded states remain untrusted
/// until [`Self::validate`] replays the phase's complete public evidence.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
#[norito(tag = "phase", content = "state", deny_unknown_fields)]
pub enum TimedOvnLifecycleStateV1 {
    /// Registration is open; no participant corpus is frozen yet.
    Registered(TimedOvnRegistrationOpenStateV1),
    /// Registration is closed with an exact canonical public roster corpus.
    RegistrationClosed(TimedOvnRegistrationClosedStateV1),
    /// The exact survivor subsequence and future release identity are frozen.
    SurvivorsFrozen(TimedOvnSurvivorsFrozenStateV1),
    /// The complete one-ballot-per-survivor corpus is proof-verified and sealed.
    Sealed(TimedOvnEvidenceStateV1),
    /// A unique threshold release has opened only the aggregate public tally.
    Released(TimedOvnReleasedEvidenceV1),
}

impl TimedOvnLifecycleStateV1 {
    /// Return the bounded public lifecycle phase without exposing its corpus.
    #[must_use]
    pub const fn phase(&self) -> TimedOvnLifecyclePhaseV1 {
        match self {
            Self::Registered(_) => TimedOvnLifecyclePhaseV1::Registered,
            Self::RegistrationClosed(_) => TimedOvnLifecyclePhaseV1::RegistrationClosed,
            Self::SurvivorsFrozen(_) => TimedOvnLifecyclePhaseV1::SurvivorsFrozen,
            Self::Sealed(_) => TimedOvnLifecyclePhaseV1::Sealed,
            Self::Released(_) => TimedOvnLifecyclePhaseV1::Released,
        }
    }

    /// Corrupt one persisted registration byte for negative restore tests.
    #[cfg(test)]
    #[doc(hidden)]
    pub(crate) fn corrupt_first_registration_record_for_testing(&mut self) {
        let records = match self {
            Self::Registered(state) => &mut state.registration_records,
            Self::RegistrationClosed(state) => &mut state.registration.registration_records,
            Self::SurvivorsFrozen(state) => {
                &mut state.registration.registration.registration_records
            }
            Self::Sealed(state) => &mut state.registration_records,
            Self::Released(state) => &mut state.sealed.registration_records,
        };
        records
            .first_mut()
            .expect("test fixture must carry a registration")[0] ^= 1;
    }

    /// Borrow the frozen public release identity once the survivor set exists.
    ///
    /// Callers must still replay-validate the lifecycle and enforce the
    /// Parliament opening phase before requesting any threshold share.
    #[must_use]
    pub const fn release_identity_public(&self) -> Option<&TimedOvnReleaseIdentityPublicV1> {
        match self {
            Self::Registered(_) | Self::RegistrationClosed(_) => None,
            Self::SurvivorsFrozen(state) => Some(&state.release_identity),
            Self::Sealed(state) => Some(&state.release_identity),
            Self::Released(state) => Some(&state.sealed.release_identity),
        }
    }

    /// Open registration with immutable session, TLE-key, and future-height bindings.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] if the TLE key/transcript binding is
    /// invalid or the release height is not strictly after the opening height.
    pub fn open_registration(
        session: TimedOvnSessionPublicV1,
        registration_opened_at_finalized_height: u64,
        target_finalized_height: u64,
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<Self, TimedOvnEvidenceError> {
        let state = TimedOvnRegistrationOpenStateV1 {
            version: TIMED_OVN_EVIDENCE_VERSION_V1,
            session,
            registration_opened_at_finalized_height,
            target_finalized_height,
            registration_records: Vec::new(),
            registration_corpus_commitment:
                ParliamentTimedOvnRegistrationCorpusCommitmentV1::from_records(&[])
                    .ok_or(TimedOvnEvidenceError::InvalidEvidenceSize)?,
        };
        state.validate(tle_key_session)?;
        Ok(Self::Registered(state))
    }

    /// Accumulate one proof-valid registration bound to an authenticated member.
    ///
    /// Records are inserted into canonical participant-hash order so consensus
    /// state is independent of transaction ordering. The caller supplies only
    /// the participant hash already derived from the transaction authority.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for an out-of-order phase, malformed
    /// or duplicate registration/key, wrong session, oversized corpus, or a
    /// record whose participant hash differs from `expected_participant_hash`.
    pub fn register_participant(
        self,
        expected_participant_hash: [u8; 32],
        registration_record: Vec<u8>,
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<Self, TimedOvnEvidenceError> {
        let Self::Registered(mut registration) = self else {
            return Err(TimedOvnEvidenceError::InvalidLifecycleTransition);
        };
        let session = registration.validate(tle_key_session)?;
        if registration_record.len() != TIMED_OVN_REGISTRATION_RECORD_BYTES_V1
            || registration.registration_records.len() >= TIMED_OVN_MAX_PARTICIPANTS_V1
        {
            return Err(TimedOvnEvidenceError::InvalidEvidenceSize);
        }
        let decoded = TimedOvnRegistrationV1::from_bytes(&session, &registration_record)?;
        if decoded.to_bytes() != registration_record
            || decoded.participant_hash() != &expected_participant_hash
        {
            return Err(TimedOvnEvidenceError::ParticipantBindingMismatch);
        }
        let mut position = 0;
        while position < registration.registration_records.len() {
            let existing = TimedOvnRegistrationV1::from_bytes(
                &session,
                &registration.registration_records[position],
            )?;
            match existing.participant_hash().cmp(&expected_participant_hash) {
                core::cmp::Ordering::Less => position += 1,
                core::cmp::Ordering::Equal => {
                    return Err(TimedOvnEvidenceError::InvalidParticipantDecision);
                }
                core::cmp::Ordering::Greater => break,
            }
        }
        registration
            .registration_records
            .insert(position, registration_record);
        rebuild_roster(
            registration.session(),
            &registration.registration_records,
            tle_key_session,
        )?;
        registration.registration_corpus_commitment =
            ParliamentTimedOvnRegistrationCorpusCommitmentV1::from_records(
                &registration.registration_records,
            )
            .ok_or(TimedOvnEvidenceError::InvalidEvidenceSize)?;
        Ok(Self::Registered(registration))
    }

    /// Close registration over the accumulated authenticated canonical records.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for an out-of-order phase, malformed
    /// or duplicate registration, oversized corpus, or wrong session binding.
    pub fn close_registration(
        self,
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<Self, TimedOvnEvidenceError> {
        let Self::Registered(registration) = self else {
            return Err(TimedOvnEvidenceError::InvalidLifecycleTransition);
        };
        let state = TimedOvnRegistrationClosedStateV1 {
            registration,
            dropout_participant_hashes: Vec::new(),
        };
        state.validate(tle_key_session)?;
        Ok(Self::RegistrationClosed(state))
    }

    /// Record one authenticated registered member's pre-ballot dropout.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for an out-of-order phase or when the
    /// participant is unknown, already dropped out, or otherwise noncanonical.
    pub fn record_dropout(
        self,
        expected_participant_hash: [u8; 32],
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<Self, TimedOvnEvidenceError> {
        let Self::RegistrationClosed(mut registration) = self else {
            return Err(TimedOvnEvidenceError::InvalidLifecycleTransition);
        };
        let (_, roster) = registration.validate(tle_key_session)?;
        if roster
            .registrations()
            .binary_search_by_key(&expected_participant_hash, |record| {
                *record.participant_hash()
            })
            .is_err()
        {
            return Err(TimedOvnEvidenceError::InvalidParticipantDecision);
        }
        match registration
            .dropout_participant_hashes
            .binary_search(&expected_participant_hash)
        {
            Ok(_) => return Err(TimedOvnEvidenceError::InvalidParticipantDecision),
            Err(position) => registration
                .dropout_participant_hashes
                .insert(position, expected_participant_hash),
        }
        registration.validate(tle_key_session)?;
        Ok(Self::RegistrationClosed(registration))
    }

    /// Freeze the exact ordered survivor subsequence and derive its release identity.
    ///
    /// The survivor root and no-recovery sentinel are computed internally; no
    /// caller-supplied root or recovery corpus is accepted.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for an out-of-order phase or a
    /// malformed, empty, duplicate, or reordered survivor subsequence.
    pub fn freeze_survivors(
        self,
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<Self, TimedOvnEvidenceError> {
        let Self::RegistrationClosed(registration) = self else {
            return Err(TimedOvnEvidenceError::InvalidLifecycleTransition);
        };
        let (_, roster) = registration.validate(tle_key_session)?;
        let survivor_participant_hashes = roster
            .registrations()
            .iter()
            .filter_map(|record| {
                registration
                    .dropout_participant_hashes
                    .binary_search(record.participant_hash())
                    .is_err()
                    .then_some(*record.participant_hash())
            })
            .collect::<Vec<_>>();
        let roots = derive_timed_ovn_roots_v1(
            registration.registration.session(),
            &registration.registration.registration_records,
            &survivor_participant_hashes,
            tle_key_session,
        )?;
        let session = registration.registration.session;
        let release_identity = TimedOvnReleaseIdentityPublicV1 {
            tle_key_session_id: session.tle_key_session_id,
            governance_attempt_id: session.governance_attempt_id,
            body_instance_id: session.body_instance_id,
            ballot_attempt_id: session.ballot_attempt_id,
            survivor_corpus_root: roots.survivor_corpus_root,
            no_recovery_root: roots.no_recovery_root,
            target_finalized_height: registration.registration.target_finalized_height,
            parameter_hash: session.parameter_hash,
        };
        let state = TimedOvnSurvivorsFrozenStateV1 {
            registration,
            survivor_participant_hashes,
            dropout_root: roots.dropout_root,
            release_identity,
        };
        state.validate(tle_key_session)?;
        Ok(Self::SurvivorsFrozen(state))
    }

    /// Verify and seal exactly one canonical ballot for every frozen survivor.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for an out-of-order phase, malformed
    /// proof, wrong ballot order/count, duplicate ephemeral, or aggregate failure.
    pub fn seal_ballots(
        self,
        ballot_records: Vec<Vec<u8>>,
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<Self, TimedOvnEvidenceError> {
        let Self::SurvivorsFrozen(frozen) = self else {
            return Err(TimedOvnEvidenceError::InvalidLifecycleTransition);
        };
        let prepared = frozen.validate(tle_key_session)?;
        let sealed = prepared.admit_ballot_corpus(&ballot_records)?;
        Ok(Self::Sealed(sealed.public_state().clone()))
    }

    /// Verify the unique threshold release and persist the aggregate-only tally.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for an out-of-order phase, an early or
    /// wrong release, failed aggregate cancellation, or out-of-range tally.
    pub fn finalize_release(
        self,
        tle_key_session: &ValidatedTleKeySessionV1,
        finalized_height: u64,
        final_release: TleFinalReleaseSignatureV1,
    ) -> Result<Self, TimedOvnEvidenceError> {
        let Self::Sealed(sealed) = self else {
            return Err(TimedOvnEvidenceError::InvalidLifecycleTransition);
        };
        sealed.verify_final_release_pregate(tle_key_session, finalized_height, &final_release)?;
        let sealed = sealed.validate(tle_key_session)?;
        let released = sealed.finalize_release(tle_key_session, finalized_height, final_release)?;
        Ok(Self::Released(released.public_state().clone()))
    }

    /// Replay and validate all public evidence required by the current phase.
    ///
    /// Released states verify their signature at the immutable target height;
    /// admission at or after that height is enforced by [`Self::finalize_release`].
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for any malformed, cross-session,
    /// noncanonical, or replay-inconsistent phase state.
    pub fn validate(
        &self,
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<(), TimedOvnEvidenceError> {
        match self {
            Self::Registered(state) => {
                state.validate(tle_key_session)?;
            }
            Self::RegistrationClosed(state) => {
                state.validate(tle_key_session)?;
            }
            Self::SurvivorsFrozen(state) => {
                state.validate(tle_key_session)?;
            }
            Self::Sealed(state) => {
                state.clone().validate(tle_key_session)?;
            }
            Self::Released(state) => {
                state.clone().validate(
                    tle_key_session,
                    state.sealed.release_identity.target_finalized_height,
                )?;
            }
        }
        Ok(())
    }

    /// Rebuild the canonical roster and return its ordered participant hashes.
    ///
    /// Registration-open state may still be empty. Every nonempty corpus is
    /// fully reparsed and proof-validated before hashes are returned.
    ///
    /// # Errors
    /// Returns [`TimedOvnEvidenceError`] for malformed, reordered, duplicate,
    /// oversized, or cross-session registration evidence.
    pub fn validated_registration_participant_hashes(
        &self,
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<Vec<[u8; 32]>, TimedOvnEvidenceError> {
        self.validate(tle_key_session)?;
        if self.registration_records().is_empty() {
            return Ok(Vec::new());
        }
        let (_, roster) =
            rebuild_roster(self.session(), self.registration_records(), tle_key_session)?;
        Ok(roster
            .registrations()
            .iter()
            .map(|record| *record.participant_hash())
            .collect())
    }

    /// Borrow the exact canonical registration corpus in every phase.
    #[must_use]
    pub fn registration_records(&self) -> &[Vec<u8>] {
        match self {
            Self::Registered(state) => &state.registration_records,
            Self::RegistrationClosed(state) => &state.registration.registration_records,
            Self::SurvivorsFrozen(state) => &state.registration.registration.registration_records,
            Self::Sealed(state) => &state.registration_records,
            Self::Released(state) => &state.sealed.registration_records,
        }
    }

    /// Borrow the immutable session shared by every lifecycle phase.
    #[must_use]
    pub const fn session(&self) -> &TimedOvnSessionPublicV1 {
        match self {
            Self::Registered(state) => &state.session,
            Self::RegistrationClosed(state) => &state.registration.session,
            Self::SurvivorsFrozen(state) => &state.registration.registration.session,
            Self::Sealed(state) => &state.session,
            Self::Released(state) => &state.sealed.session,
        }
    }

    /// Return the exact ballot-attempt storage key binding.
    #[must_use]
    pub fn ballot_attempt_id(&self) -> [u8; 32] {
        self.session().ballot_attempt_id
    }

    /// Return the exact long-lived TLE key-session binding.
    #[must_use]
    pub fn tle_key_session_id(&self) -> TleKeySessionId {
        self.session().tle_key_session_id
    }

    /// Return the immutable first finalized height permitting threshold release.
    #[must_use]
    pub const fn target_finalized_height(&self) -> u64 {
        match self {
            Self::Registered(state) => state.target_finalized_height,
            Self::RegistrationClosed(state) => state.registration.target_finalized_height,
            Self::SurvivorsFrozen(state) => state.release_identity.target_finalized_height,
            Self::Sealed(state) => state.release_identity.target_finalized_height,
            Self::Released(state) => state.sealed.release_identity.target_finalized_height,
        }
    }

    /// Return the immutable registration-open height while the pre-seal
    /// casting lifecycle still retains that schedule field.
    #[must_use]
    pub const fn registration_opened_at_finalized_height(&self) -> Option<u64> {
        match self {
            Self::Registered(state) => Some(state.registration_opened_at_finalized_height),
            Self::RegistrationClosed(state) => {
                Some(state.registration.registration_opened_at_finalized_height)
            }
            Self::SurvivorsFrozen(state) => Some(
                state
                    .registration
                    .registration
                    .registration_opened_at_finalized_height,
            ),
            Self::Sealed(_) | Self::Released(_) => None,
        }
    }

    /// Borrow the cached exact registration-corpus commitment in cast-capable phases.
    #[must_use]
    pub const fn castable_registration_corpus_commitment(
        &self,
    ) -> Option<&ParliamentTimedOvnRegistrationCorpusCommitmentV1> {
        match self {
            Self::Registered(state) => Some(&state.registration_corpus_commitment),
            Self::RegistrationClosed(state) => {
                Some(&state.registration.registration_corpus_commitment)
            }
            Self::SurvivorsFrozen(state) => Some(
                &state
                    .registration
                    .registration
                    .registration_corpus_commitment,
            ),
            Self::Sealed(_) | Self::Released(_) => None,
        }
    }

    /// Return whether `self` is the exact direct successor of `previous`.
    ///
    /// Successor checks compare all prior public evidence byte-for-byte. This
    /// makes phase replacement monotonic and prevents a caller from changing a
    /// schedule, roster, survivor set, release identity, or sealed corpus while
    /// advancing the enum discriminant.
    #[must_use]
    pub fn is_direct_successor_of(&self, previous: &Self) -> bool {
        match (previous, self) {
            (Self::Registered(before), Self::Registered(after)) => {
                before.version == after.version
                    && before.session == after.session
                    && before.registration_opened_at_finalized_height
                        == after.registration_opened_at_finalized_height
                    && before.target_finalized_height == after.target_finalized_height
                    && before.registration_corpus_commitment != after.registration_corpus_commitment
                    && is_single_ordered_insertion(
                        &before.registration_records,
                        &after.registration_records,
                    )
            }
            (Self::Registered(before), Self::RegistrationClosed(after)) => {
                &after.registration == before && after.dropout_participant_hashes.is_empty()
            }
            (Self::RegistrationClosed(before), Self::RegistrationClosed(after)) => {
                before.registration == after.registration
                    && is_single_ordered_insertion(
                        &before.dropout_participant_hashes,
                        &after.dropout_participant_hashes,
                    )
            }
            (Self::RegistrationClosed(before), Self::SurvivorsFrozen(after)) => {
                &after.registration == before
            }
            (Self::SurvivorsFrozen(before), Self::Sealed(after)) => {
                after.session == *before.registration.registration.session()
                    && after.registration_records
                        == before.registration.registration.registration_records
                    && after.survivor_participant_hashes == before.survivor_participant_hashes
                    && after.release_identity == before.release_identity
                    && after.aggregate.dropout_root == before.dropout_root
            }
            (Self::Sealed(before), Self::Released(after)) => &after.sealed == before,
            _ => false,
        }
    }
}

/// Prepared public survivor context used to cast and admit folded ballots.
///
/// This runtime object is nonserializable and contains no secret material.
#[derive(Debug, Clone)]
pub struct PreparedTimedOvnAttemptV1 {
    session_record: TimedOvnSessionPublicV1,
    release_record: TimedOvnReleaseIdentityPublicV1,
    registration_records: Vec<Vec<u8>>,
    survivor_ids: Vec<[u8; 32]>,
    session: TimedOvnSessionV1,
    roster: TimedOvnRosterV1,
    survivors: TimedOvnSurvivorRosterV1,
    release_identity: TleReleaseIdentityV1,
}

impl PreparedTimedOvnAttemptV1 {
    /// Rebuild the public roster and freeze the exact future-release identity.
    ///
    /// Call [`derive_timed_ovn_roots_v1`] first when constructing a new release
    /// record so its survivor root is replay-derived rather than caller asserted.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for malformed/cross-session records,
    /// noncanonical survivors, or an invalid future release identity.
    pub fn from_records(
        session_record: TimedOvnSessionPublicV1,
        registration_records: &[Vec<u8>],
        survivor_ids: &[[u8; 32]],
        release_record: TimedOvnReleaseIdentityPublicV1,
        tle_key_session: &ValidatedTleKeySessionV1,
    ) -> Result<Self, TimedOvnEvidenceError> {
        let (session, roster) =
            rebuild_roster(&session_record, registration_records, tle_key_session)?;
        validate_survivor_count(survivor_ids, registration_records.len())?;
        let survivor_root = roster.prospective_survivor_root(survivor_ids)?;
        let no_recovery_root = no_recovery_root(&session, roster.roster_root(), &survivor_root);
        let release_identity = release_record.rebuild(
            &session_record,
            &survivor_root,
            &no_recovery_root,
            tle_key_session,
        )?;
        let survivors = TimedOvnSurvivorRosterV1::new(&roster, survivor_ids, release_identity)?;
        Ok(Self {
            session_record,
            release_record,
            registration_records: registration_records.to_vec(),
            survivor_ids: survivor_ids.to_vec(),
            session,
            roster,
            survivors,
            release_identity,
        })
    }

    /// Borrow the verified survivor roster needed by secret ballot owners.
    #[must_use]
    pub const fn survivor_roster(&self) -> &TimedOvnSurvivorRosterV1 {
        &self.survivors
    }

    /// Borrow the exact typed future release identity.
    #[must_use]
    pub const fn release_identity(&self) -> &TleReleaseIdentityV1 {
        &self.release_identity
    }

    /// Borrow the replay-validated complete registration roster.
    #[must_use]
    pub const fn registration_roster(&self) -> &TimedOvnRosterV1 {
        &self.roster
    }

    /// Admit exactly one canonical ballot record per frozen survivor.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for a wrong count/width/order, failed
    /// ballot proof, duplicate ephemeral, or aggregate failure.
    pub fn admit_ballot_corpus(
        &self,
        ballot_records: &[Vec<u8>],
    ) -> Result<ValidatedTimedOvnEvidenceV1, TimedOvnEvidenceError> {
        validate_ballot_records(ballot_records, self.survivor_ids.len())?;
        let ballots = ballot_records
            .iter()
            .map(|bytes| {
                let ballot = TimedOvnMaskedBallotV1::from_bytes(&self.survivors, bytes)?;
                if ballot.to_bytes() != *bytes {
                    return Err(TimedOvnError::InvalidEncoding);
                }
                Ok(ballot)
            })
            .collect::<Result<Vec<_>, TimedOvnError>>()?;
        let aggregate = aggregate_timed_ovn_ballots_v1(&self.survivors, &ballots)?;
        let dropout_root = dropout_decisions_root(&self.session, &self.roster, &self.survivor_ids);
        let aggregate_transcript = TimedOvnAggregateTranscriptV1::replay(
            &self.session,
            &self.survivors,
            &self.release_identity,
            ballot_records,
            &aggregate,
            dropout_root,
        )?;
        let state = TimedOvnEvidenceStateV1 {
            version: TIMED_OVN_EVIDENCE_VERSION_V1,
            session: self.session_record,
            registration_records: self.registration_records.clone(),
            survivor_participant_hashes: self.survivor_ids.clone(),
            release_identity: self.release_record,
            ballot_records: ballot_records.to_vec(),
            aggregate: aggregate_transcript,
        };
        Ok(ValidatedTimedOvnEvidenceV1 {
            state,
            session: self.session,
            roster: self.roster.clone(),
            survivors: self.survivors.clone(),
            release_identity: self.release_identity,
            ballots,
            aggregate,
        })
    }
}

/// Constructor-authenticated runtime evidence for one complete sealed corpus.
///
/// The runtime crypto objects are rebuilt from the exact public wires and are
/// deliberately absent from the serialized state.
#[derive(Debug, Clone)]
pub struct ValidatedTimedOvnEvidenceV1 {
    state: TimedOvnEvidenceStateV1,
    session: TimedOvnSessionV1,
    roster: TimedOvnRosterV1,
    survivors: TimedOvnSurvivorRosterV1,
    release_identity: TleReleaseIdentityV1,
    ballots: Vec<TimedOvnMaskedBallotV1>,
    aggregate: TimedOvnAggregateV1,
}

impl ValidatedTimedOvnEvidenceV1 {
    /// Borrow the canonical public-only persistence state.
    #[must_use]
    pub const fn public_state(&self) -> &TimedOvnEvidenceStateV1 {
        &self.state
    }

    /// Borrow the reconstructed immutable timed-OVN session.
    #[must_use]
    pub const fn session(&self) -> &TimedOvnSessionV1 {
        &self.session
    }

    /// Borrow the exact future release identity.
    #[must_use]
    pub const fn release_identity(&self) -> &TleReleaseIdentityV1 {
        &self.release_identity
    }

    /// Borrow the verified public aggregate.
    #[must_use]
    pub const fn aggregate(&self) -> &TimedOvnAggregateV1 {
        &self.aggregate
    }

    /// Borrow the replay-validated complete registration roster.
    #[must_use]
    pub const fn registration_roster(&self) -> &TimedOvnRosterV1 {
        &self.roster
    }

    /// Borrow the replay-validated survivor roster.
    #[must_use]
    pub const fn survivor_roster(&self) -> &TimedOvnSurvivorRosterV1 {
        &self.survivors
    }

    /// Borrow the replay-validated exact public ballot corpus.
    #[must_use]
    pub fn ballots(&self) -> &[TimedOvnMaskedBallotV1] {
        &self.ballots
    }

    /// Verify the final release and open only the exact aggregate.
    ///
    /// # Errors
    ///
    /// Returns [`TimedOvnEvidenceError`] for an early/wrong release, failed
    /// threshold signature, aggregate cancellation failure, or invalid tally.
    pub fn finalize_release(
        &self,
        tle_key_session: &ValidatedTleKeySessionV1,
        finalized_height: u64,
        final_release: TleFinalReleaseSignatureV1,
    ) -> Result<ValidatedTimedOvnReleasedEvidenceV1, TimedOvnEvidenceError> {
        if self.state.session.tle_key_session_id != tle_key_session.public_state().key_session_id
            || self.state.session.tle_key_transcript_hash
                != tle_key_session.public_state().transcript_hash
        {
            return Err(TimedOvnEvidenceError::TleKeySessionMismatch);
        }
        let release_key = tle_key_session.release_key_for_opening(
            &self.release_identity,
            finalized_height,
            &final_release,
        )?;
        let tally = self
            .aggregate
            .open_and_tally(&self.survivors, &release_key)?;
        let tally = TimedOvnPublicTallyV1::from(tally);
        let opening_root = opening_transcript_root(&self.state.aggregate, &final_release, tally);
        let state = TimedOvnReleasedEvidenceV1 {
            version: TIMED_OVN_EVIDENCE_VERSION_V1,
            sealed: self.state.clone(),
            final_release,
            tally,
            opening_root,
        };
        Ok(ValidatedTimedOvnReleasedEvidenceV1 {
            state,
            sealed: self.clone(),
        })
    }
}

/// Constructor-authenticated released evidence with an aggregate-only tally.
#[derive(Debug, Clone)]
pub struct ValidatedTimedOvnReleasedEvidenceV1 {
    state: TimedOvnReleasedEvidenceV1,
    sealed: ValidatedTimedOvnEvidenceV1,
}

impl ValidatedTimedOvnReleasedEvidenceV1 {
    /// Borrow the canonical public-only released evidence.
    #[must_use]
    pub const fn public_state(&self) -> &TimedOvnReleasedEvidenceV1 {
        &self.state
    }

    /// Borrow the constructor-authenticated sealed evidence.
    #[must_use]
    pub const fn sealed(&self) -> &ValidatedTimedOvnEvidenceV1 {
        &self.sealed
    }

    /// Return the aggregate-only public tally.
    #[must_use]
    pub const fn tally(&self) -> TimedOvnPublicTallyV1 {
        self.state.tally
    }
}

/// Verify registrations and derive roots before constructing the release identity.
///
/// # Errors
///
/// Returns [`TimedOvnEvidenceError`] for an oversized/malformed registration
/// corpus, wrong key session, or noncanonical survivor subsequence.
pub fn derive_timed_ovn_roots_v1(
    session_record: &TimedOvnSessionPublicV1,
    registration_records: &[Vec<u8>],
    survivor_ids: &[[u8; 32]],
    tle_key_session: &ValidatedTleKeySessionV1,
) -> Result<TimedOvnProspectiveRootsV1, TimedOvnEvidenceError> {
    let (session, roster) = rebuild_roster(session_record, registration_records, tle_key_session)?;
    validate_survivor_count(survivor_ids, registration_records.len())?;
    let survivor_corpus_root = roster.prospective_survivor_root(survivor_ids)?;
    Ok(TimedOvnProspectiveRootsV1 {
        registration_roster_root: *roster.roster_root(),
        dropout_root: dropout_decisions_root(&session, &roster, survivor_ids),
        survivor_corpus_root,
        no_recovery_root: no_recovery_root(&session, roster.roster_root(), &survivor_corpus_root),
    })
}

/// Errors returned by timed-OVN evidence construction, restore, and release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TimedOvnEvidenceError {
    /// A decoded state advertised another evidence version.
    #[error("unsupported timed-OVN evidence version")]
    UnsupportedVersion,
    /// The release target did not strictly follow registration opening.
    #[error("timed-OVN release height must be after registration opens")]
    InvalidReleaseSchedule,
    /// A lifecycle transition skipped, repeated, or rewound a secure phase.
    #[error("invalid timed-OVN lifecycle transition")]
    InvalidLifecycleTransition,
    /// A registration, survivor, or ballot corpus exceeded its exact v1 bounds.
    #[error("timed-OVN evidence corpus is empty, oversized, or has a wrong wire width")]
    InvalidEvidenceSize,
    /// Opening another cast-capable lifecycle would exceed the protocol maximum.
    #[error("too many concurrent cast-capable timed-OVN contexts")]
    TooManyConcurrentCastingContexts,
    /// A registration record did not bind the authenticated seated member.
    #[error("timed-OVN registration participant differs from the authenticated member")]
    ParticipantBindingMismatch,
    /// A participant registration or dropout was unknown, duplicated, or reordered.
    #[error("invalid timed-OVN participant registration or dropout decision")]
    InvalidParticipantDecision,
    /// The evidence named another adaptive TLE key or transcript.
    #[error("timed-OVN evidence is bound to another TLE key session")]
    TleKeySessionMismatch,
    /// Future release fields did not match the exact timed-ballot session and survivor root.
    #[error("timed-OVN future release identity does not match replayed evidence")]
    ReleaseIdentityMismatch,
    /// Cached roots, aggregates, release terms, or tallies differed from replay.
    #[error("timed-OVN persisted evidence differs from deterministic replay")]
    ReplayMismatch,
    /// Folded timed-OVN cryptographic validation failed.
    #[error(transparent)]
    TimedOvn(#[from] TimedOvnError),
    /// Timelock identity or master-key validation failed.
    #[error(transparent)]
    Tle(#[from] TleError),
    /// Adaptive threshold-release verification failed.
    #[error(transparent)]
    Release(#[from] TleReleaseAdapterError),
}

fn is_single_ordered_insertion<T: PartialEq>(before: &[T], after: &[T]) -> bool {
    if after.len() != before.len().saturating_add(1) {
        return false;
    }
    let mut before_index = 0;
    let mut skipped = false;
    for item in after {
        if before_index < before.len() && item == &before[before_index] {
            before_index += 1;
        } else if skipped {
            return false;
        } else {
            skipped = true;
        }
    }
    skipped && before_index == before.len()
}

/// Replay the exact public session and optional nonempty registration roster
/// carried by a wallet casting-context archive.
///
/// # Errors
/// Returns a timed-OVN evidence error for a malformed session, registration
/// record, proof, ordering, duplicate participant, or TLE transcript binding.
pub(crate) fn rebuild_casting_registration_context_v1(
    session_record: &TimedOvnSessionPublicV1,
    registration_records: &[Vec<u8>],
    tle_key_session: &ValidatedTleKeySessionV1,
) -> Result<(TimedOvnSessionV1, Option<TimedOvnRosterV1>), TimedOvnEvidenceError> {
    if registration_records.is_empty() {
        return Ok((session_record.rebuild(tle_key_session)?, None));
    }
    let (session, roster) = rebuild_roster(session_record, registration_records, tle_key_session)?;
    Ok((session, Some(roster)))
}

fn rebuild_roster(
    session_record: &TimedOvnSessionPublicV1,
    registration_records: &[Vec<u8>],
    tle_key_session: &ValidatedTleKeySessionV1,
) -> Result<(TimedOvnSessionV1, TimedOvnRosterV1), TimedOvnEvidenceError> {
    validate_registration_records(registration_records)?;
    let session = session_record.rebuild(tle_key_session)?;
    let registrations = registration_records
        .iter()
        .map(|bytes| {
            let registration = TimedOvnRegistrationV1::from_bytes(&session, bytes)?;
            if registration.to_bytes() != *bytes {
                return Err(TimedOvnError::InvalidEncoding);
            }
            Ok(registration)
        })
        .collect::<Result<Vec<_>, TimedOvnError>>()?;
    let roster = TimedOvnRosterV1::new(session, registrations)?;
    Ok((session, roster))
}

fn validate_registration_records(records: &[Vec<u8>]) -> Result<(), TimedOvnEvidenceError> {
    if records.is_empty()
        || records.len() > TIMED_OVN_MAX_PARTICIPANTS_V1
        || records
            .iter()
            .any(|record| record.len() != TIMED_OVN_REGISTRATION_RECORD_BYTES_V1)
    {
        return Err(TimedOvnEvidenceError::InvalidEvidenceSize);
    }
    Ok(())
}

fn validate_dropout_participant_hashes(
    roster: &TimedOvnRosterV1,
    dropout_participant_hashes: &[[u8; 32]],
) -> Result<(), TimedOvnEvidenceError> {
    let mut previous = None;
    for participant_hash in dropout_participant_hashes {
        if previous.is_some_and(|value| value >= *participant_hash)
            || roster
                .registrations()
                .binary_search_by_key(participant_hash, |record| *record.participant_hash())
                .is_err()
        {
            return Err(TimedOvnEvidenceError::InvalidParticipantDecision);
        }
        previous = Some(*participant_hash);
    }
    Ok(())
}

fn validate_survivor_count(
    survivor_ids: &[[u8; 32]],
    registration_count: usize,
) -> Result<(), TimedOvnEvidenceError> {
    if survivor_ids.is_empty()
        || survivor_ids.len() > registration_count
        || survivor_ids.len() > TIMED_OVN_MAX_PARTICIPANTS_V1
    {
        return Err(TimedOvnEvidenceError::InvalidEvidenceSize);
    }
    Ok(())
}

fn validate_ballot_records(
    records: &[Vec<u8>],
    survivor_count: usize,
) -> Result<(), TimedOvnEvidenceError> {
    if records.len() != survivor_count
        || records.is_empty()
        || records.len() > TIMED_OVN_MAX_PARTICIPANTS_V1
        || records
            .iter()
            .any(|record| record.len() != TIMED_OVN_BALLOT_RECORD_BYTES_V1)
    {
        return Err(TimedOvnEvidenceError::InvalidEvidenceSize);
    }
    Ok(())
}

fn ballot_corpus_hash(ballots: &[Vec<u8>]) -> Result<[u8; 32], TimedOvnEvidenceError> {
    let count =
        u32::try_from(ballots.len()).map_err(|_| TimedOvnEvidenceError::InvalidEvidenceSize)?;
    let mut hasher = Sha256::new();
    hasher.update(BALLOT_CORPUS_DOMAIN_V1);
    hasher.update(TIMED_OVN_EVIDENCE_VERSION_V1.to_be_bytes());
    hasher.update(count.to_be_bytes());
    for ballot in ballots {
        let length =
            u32::try_from(ballot.len()).map_err(|_| TimedOvnEvidenceError::InvalidEvidenceSize)?;
        hasher.update(length.to_be_bytes());
        hasher.update(ballot);
    }
    Ok(hasher.finalize().into())
}

fn dropout_decisions_root(
    session: &TimedOvnSessionV1,
    roster: &TimedOvnRosterV1,
    survivor_ids: &[[u8; 32]],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(DROPOUT_DECISIONS_DOMAIN_V1);
    hasher.update(TIMED_OVN_EVIDENCE_VERSION_V1.to_be_bytes());
    hasher.update(session.digest());
    hasher.update(roster.roster_root());
    hasher.update((roster.registrations().len() as u32).to_be_bytes());
    let mut survivor_cursor = 0_usize;
    for registration in roster.registrations() {
        let survives = survivor_ids.get(survivor_cursor) == Some(registration.participant_hash());
        hasher.update(registration.participant_hash());
        hasher.update([u8::from(survives)]);
        if survives {
            survivor_cursor += 1;
        }
    }
    debug_assert_eq!(survivor_cursor, survivor_ids.len());
    hasher.finalize().into()
}

fn opening_transcript_root(
    aggregate: &TimedOvnAggregateTranscriptV1,
    final_release: &TleFinalReleaseSignatureV1,
    tally: TimedOvnPublicTallyV1,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(OPENING_TRANSCRIPT_DOMAIN_V1);
    hasher.update(TIMED_OVN_EVIDENCE_VERSION_V1.to_be_bytes());
    hasher.update(aggregate.transcript_hash);
    hasher.update(aggregate.identity_digest);
    hasher.update(final_release.key_session_id.as_bytes());
    hasher.update(final_release.identity_digest);
    hasher.update(final_release.signature);
    hasher.update(tally.aye.to_be_bytes());
    hasher.update(tally.nay.to_be_bytes());
    hasher.update(tally.abstain.to_be_bytes());
    hasher.finalize().into()
}

fn no_recovery_root(
    session: &TimedOvnSessionV1,
    roster_root: &[u8; 32],
    survivor_root: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(NO_RECOVERY_ROOT_DOMAIN_V1);
    hasher.update(TIMED_OVN_EVIDENCE_VERSION_V1.to_be_bytes());
    hasher.update(session.digest());
    hasher.update(roster_root);
    hasher.update(survivor_root);
    hasher.finalize().into()
}

fn is_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{
        threshold_bls::{
            AdaptiveThresholdBlsParameters, AdaptiveThresholdBlsSecretShare, DasRenDealerSecret,
            TleReleasePurpose, ValidatedDealerCommitment,
        },
        timed_ovn::{TimedOvnChoiceV1, TimedOvnRegistrationSecretV1},
    };
    use norito::codec::{DecodeAll as _, Encode as _};
    use rand::{SeedableRng as _, rngs::StdRng};

    use super::*;
    use crate::tle_release::{
        PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_ARCHIVE_MAX_BYTES_V1,
        ParliamentTimedOvnCastingContextArchiveV1, ParliamentTimedOvnCastingPhaseV1,
        TimedOvnCastingArchiveValidationErrorV1, ValidatedTleKeySessionV1,
    };

    fn binding(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    struct TleFixture {
        session: iroha_crypto::threshold_bls::ThresholdBlsSession<TleReleasePurpose>,
        key: ValidatedTleKeySessionV1,
        dealer_secrets: Vec<DasRenDealerSecret<TleReleasePurpose>>,
        dealers: Vec<ValidatedDealerCommitment<TleReleasePurpose>>,
    }

    fn tle_fixture() -> TleFixture {
        let session = iroha_crypto::threshold_bls::ThresholdBlsSession::<TleReleasePurpose>::new(
            binding(1),
            binding(2),
            binding(3),
            4,
            2,
        )
        .expect("TLE threshold session");
        let parameters = AdaptiveThresholdBlsParameters::derive(&session).expect("parameters");
        let mut rng = StdRng::from_seed([31; 32]);
        let mut dealer_secrets = Vec::new();
        let mut dealers = Vec::new();
        for index in 1_u16..=3 {
            let (secret, dealer) =
                DasRenDealerSecret::generate_with_rng(&parameters, index, &mut rng)
                    .expect("dealer");
            dealer_secrets.push(secret);
            dealers.push(dealer);
        }
        let key = ValidatedTleKeySessionV1::from_qualified_dealers(
            session,
            &dealers,
            &[1, 2, 3],
            binding(4),
        )
        .expect("TLE key session");
        TleFixture {
            session,
            key,
            dealer_secrets,
            dealers,
        }
    }

    #[test]
    fn exact_public_evidence_replays_and_opens_only_after_threshold_release() {
        let tle = tle_fixture();
        let session_record = TimedOvnSessionPublicV1 {
            network_id: binding(1),
            proposal_content_id: binding(10),
            governance_attempt_id: binding(11),
            body_instance_id: binding(12),
            ballot_attempt_id: binding(13),
            parameter_hash: timed_ovn_parameter_hash_v1(),
            tle_key_session_id: tle.key.public_state().key_session_id,
            tle_key_transcript_hash: tle.key.public_state().transcript_hash,
            tle_master_public_key: *tle.key.master_public_key().as_bytes(),
        };
        let crypto_session = session_record.rebuild(&tle.key).expect("timed session");
        let mut rng = StdRng::from_seed([32; 32]);
        let participant_ids = [binding(20), binding(21), binding(22)];
        let mut registration_secrets = Vec::new();
        let mut registration_records = Vec::new();
        for participant in participant_ids {
            let (secret, registration) = TimedOvnRegistrationSecretV1::generate_with_rng(
                &crypto_session,
                participant,
                &mut rng,
            )
            .expect("registration");
            assert_eq!(
                registration.to_bytes().len(),
                TIMED_OVN_REGISTRATION_RECORD_BYTES_V1
            );
            registration_secrets.push(secret);
            registration_records.push(registration.to_bytes());
        }
        let roots = derive_timed_ovn_roots_v1(
            &session_record,
            &registration_records,
            &participant_ids,
            &tle.key,
        )
        .expect("roots");
        let release_record = TimedOvnReleaseIdentityPublicV1 {
            tle_key_session_id: tle.key.public_state().key_session_id,
            governance_attempt_id: session_record.governance_attempt_id,
            body_instance_id: session_record.body_instance_id,
            ballot_attempt_id: session_record.ballot_attempt_id,
            survivor_corpus_root: roots.survivor_corpus_root,
            no_recovery_root: roots.no_recovery_root,
            target_finalized_height: 100,
            parameter_hash: session_record.parameter_hash,
        };
        for phase in [
            ParliamentTimedOvnCastingPhaseV1::Registered,
            ParliamentTimedOvnCastingPhaseV1::RegistrationClosed,
        ] {
            let archive = ParliamentTimedOvnCastingContextArchiveV1::try_from_parts_v1(
                20,
                phase,
                session_record,
                10,
                100,
                tle.key.public_state().clone(),
                registration_records.clone(),
                None,
                None,
            )
            .expect("pre-freeze casting archive replay");
            let validated = archive.validate_v1().expect("validated archive");
            assert!(validated.prepared_attempt().is_none());
            let (registration_close, survivor_freeze, commitment_close) = match phase {
                ParliamentTimedOvnCastingPhaseV1::Registered => (21, 22, 23),
                ParliamentTimedOvnCastingPhaseV1::RegistrationClosed => (20, 21, 22),
                ParliamentTimedOvnCastingPhaseV1::SurvivorsFrozen => unreachable!(),
            };
            let compact = validated
                .compact_binding_v1(registration_close, survivor_freeze, commitment_close)
                .expect("archive-derived compact binding");
            assert!(validated.matches_compact_binding_v1(&compact));
            let mut tampered = compact;
            tampered.tle_key_transcript_hash[0] ^= 1;
            assert!(!validated.matches_compact_binding_v1(&tampered));
        }
        let frozen_archive = ParliamentTimedOvnCastingContextArchiveV1::try_from_parts_v1(
            20,
            ParliamentTimedOvnCastingPhaseV1::SurvivorsFrozen,
            session_record,
            10,
            100,
            tle.key.public_state().clone(),
            registration_records.clone(),
            Some(participant_ids.to_vec()),
            Some(release_record),
        )
        .expect("survivor-frozen casting archive replay");
        let validated_frozen = frozen_archive
            .validate_v1()
            .expect("validated frozen archive");
        assert!(validated_frozen.prepared_attempt().is_some());
        let frozen_compact = validated_frozen
            .compact_binding_v1(19, 20, 21)
            .expect("archive-derived frozen compact binding");
        assert!(validated_frozen.matches_compact_binding_v1(&frozen_compact));
        assert_eq!(
            frozen_compact.survivor_count,
            u32::try_from(participant_ids.len()).ok()
        );
        assert_eq!(frozen_compact.dropout_root, Some(roots.dropout_root));
        assert_eq!(
            frozen_compact
                .release_identity
                .expect("frozen release binding")
                .survivor_corpus_root,
            roots.survivor_corpus_root
        );
        let framed = frozen_archive
            .to_canonical_bytes_v1()
            .expect("bounded header-framed casting archive");
        assert!(framed.len() <= PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_ARCHIVE_MAX_BYTES_V1);
        let decoded: ParliamentTimedOvnCastingContextArchiveV1 =
            norito::decode_from_bytes(&framed).expect("decode canonical casting archive");
        assert_eq!(decoded, frozen_archive);
        assert!(decoded.validate_v1().is_ok());
        assert!(matches!(
            ParliamentTimedOvnCastingContextArchiveV1::try_from_parts_v1(
                20,
                ParliamentTimedOvnCastingPhaseV1::RegistrationClosed,
                session_record,
                10,
                100,
                tle.key.public_state().clone(),
                registration_records.clone(),
                Some(participant_ids.to_vec()),
                Some(release_record),
            ),
            Err(TimedOvnCastingArchiveValidationErrorV1::PhaseFieldMismatch)
        ));
        let mut wrong_target = release_record;
        wrong_target.target_finalized_height = 101;
        assert!(matches!(
            ParliamentTimedOvnCastingContextArchiveV1::try_from_parts_v1(
                20,
                ParliamentTimedOvnCastingPhaseV1::SurvivorsFrozen,
                session_record,
                10,
                100,
                tle.key.public_state().clone(),
                registration_records.clone(),
                Some(participant_ids.to_vec()),
                Some(wrong_target),
            ),
            Err(TimedOvnCastingArchiveValidationErrorV1::SessionBindingMismatch)
                | Err(TimedOvnCastingArchiveValidationErrorV1::TimedOvn(_))
        ));
        let mut forged_recovery = release_record;
        forged_recovery.no_recovery_root = binding(23);
        assert_eq!(
            PreparedTimedOvnAttemptV1::from_records(
                session_record,
                &registration_records,
                &participant_ids,
                forged_recovery,
                &tle.key,
            )
            .err(),
            Some(TimedOvnEvidenceError::ReleaseIdentityMismatch)
        );
        assert_eq!(
            TimedOvnLifecycleStateV1::open_registration(session_record, 100, 100, &tle.key).err(),
            Some(TimedOvnEvidenceError::InvalidReleaseSchedule)
        );
        let lifecycle =
            TimedOvnLifecycleStateV1::open_registration(session_record, 10, 100, &tle.key)
                .expect("registration open");
        assert_eq!(
            lifecycle.clone().freeze_survivors(&tle.key).err(),
            Some(TimedOvnEvidenceError::InvalidLifecycleTransition)
        );
        assert_eq!(
            lifecycle
                .clone()
                .register_participant(binding(99), registration_records[0].clone(), &tle.key)
                .err(),
            Some(TimedOvnEvidenceError::ParticipantBindingMismatch)
        );
        let mut lifecycle = lifecycle;
        for (participant_hash, registration_record) in participant_ids
            .into_iter()
            .zip(registration_records.iter().cloned())
            .rev()
        {
            lifecycle = lifecycle
                .register_participant(participant_hash, registration_record, &tle.key)
                .expect("authenticated registration");
        }
        assert_eq!(lifecycle.registration_records(), registration_records);
        let expected_registration_commitment =
            ParliamentTimedOvnRegistrationCorpusCommitmentV1::from_records(&registration_records)
                .expect("bounded registration corpus commitment");
        assert_eq!(
            lifecycle.castable_registration_corpus_commitment(),
            Some(&expected_registration_commitment)
        );
        let mut corrupted_cache = lifecycle.clone();
        let TimedOvnLifecycleStateV1::Registered(corrupted_registration) = &mut corrupted_cache
        else {
            panic!("expected registration-open lifecycle");
        };
        corrupted_registration.registration_corpus_commitment.digest =
            iroha_crypto::Hash::new(b"corrupt cached registration corpus");
        assert_eq!(
            corrupted_cache.validate(&tle.key),
            Err(TimedOvnEvidenceError::ReplayMismatch)
        );
        assert_eq!(
            lifecycle
                .clone()
                .register_participant(
                    participant_ids[0],
                    registration_records[0].clone(),
                    &tle.key,
                )
                .err(),
            Some(TimedOvnEvidenceError::InvalidParticipantDecision)
        );
        let registration_closed = lifecycle
            .close_registration(&tle.key)
            .expect("registration closed");
        assert_eq!(
            registration_closed
                .clone()
                .record_dropout(binding(99), &tle.key)
                .err(),
            Some(TimedOvnEvidenceError::InvalidParticipantDecision)
        );
        let one_dropout = registration_closed
            .clone()
            .record_dropout(participant_ids[0], &tle.key)
            .expect("authenticated dropout");
        assert_eq!(
            one_dropout
                .clone()
                .record_dropout(participant_ids[0], &tle.key)
                .err(),
            Some(TimedOvnEvidenceError::InvalidParticipantDecision)
        );
        let TimedOvnLifecycleStateV1::SurvivorsFrozen(dropout_frozen) = one_dropout
            .freeze_survivors(&tle.key)
            .expect("dropout survivor freeze")
        else {
            panic!("expected survivor-frozen state");
        };
        assert_eq!(
            dropout_frozen.survivor_participant_hashes(),
            &participant_ids[1..]
        );
        let lifecycle = registration_closed
            .freeze_survivors(&tle.key)
            .expect("survivors frozen");
        lifecycle.validate(&tle.key).expect("frozen replay");
        let TimedOvnLifecycleStateV1::SurvivorsFrozen(frozen) = &lifecycle else {
            panic!("expected survivor-frozen state");
        };
        assert_eq!(frozen.release_identity(), &release_record);
        let prepared = frozen.validate(&tle.key).expect("prepared attempt");
        let choices = [
            TimedOvnChoiceV1::Aye,
            TimedOvnChoiceV1::Nay,
            TimedOvnChoiceV1::Abstain,
        ];
        let ballot_records = registration_secrets
            .iter()
            .zip(choices)
            .map(|(secret, choice)| {
                let ballot = secret
                    .cast_ballot_with_rng(prepared.survivor_roster(), choice, &mut rng)
                    .expect("ballot");
                let bytes = ballot.to_bytes();
                assert_eq!(bytes.len(), TIMED_OVN_BALLOT_RECORD_BYTES_V1);
                bytes
            })
            .collect::<Vec<_>>();
        let lifecycle = lifecycle
            .seal_ballots(ballot_records.clone(), &tle.key)
            .expect("sealed lifecycle");
        lifecycle
            .validate(&tle.key)
            .expect("sealed lifecycle replay");
        let TimedOvnLifecycleStateV1::Sealed(sealed_state) = &lifecycle else {
            panic!("expected sealed state");
        };
        let sealed = sealed_state
            .clone()
            .validate(&tle.key)
            .expect("sealed evidence");
        let encoded = sealed.public_state().encode();
        let restored = TimedOvnEvidenceStateV1::decode_all(&mut encoded.as_slice())
            .expect("decode")
            .validate(&tle.key)
            .expect("replay");
        assert_eq!(restored.public_state(), sealed.public_state());

        let identity = sealed.release_identity();
        let parameters = *tle.key.transcript().parameters();
        let mut partial_records = Vec::new();
        for recipient in 1_u16..=2 {
            let contributions = tle
                .dealer_secrets
                .iter()
                .zip(&tle.dealers)
                .map(|(secret, dealer)| {
                    secret
                        .private_share(&parameters, dealer, recipient)
                        .expect("private contribution")
                })
                .collect::<Vec<_>>();
            let signing_share = AdaptiveThresholdBlsSecretShare::from_dealer_shares(
                tle.key.transcript(),
                &contributions,
            )
            .expect("signing share");
            let partial = signing_share
                .sign_payload_with_rng(tle.key.transcript(), &identity.payload_bytes(), &mut rng)
                .expect("partial");
            partial_records.push(
                tle.key
                    .encode_partial_release(identity, 100, &partial)
                    .expect("partial record"),
            );
        }
        let final_release = tle
            .key
            .combine_partial_releases(identity, 100, &partial_records)
            .expect("final release");
        let released = sealed
            .finalize_release(&tle.key, 100, final_release)
            .expect("released evidence");
        assert_eq!(
            released.tally(),
            TimedOvnPublicTallyV1 {
                aye: 1,
                nay: 1,
                abstain: 1,
            }
        );
        let released_bytes = released.public_state().encode();
        let restored_release =
            TimedOvnReleasedEvidenceV1::decode_all(&mut released_bytes.as_slice())
                .expect("decode release")
                .validate(&tle.key, 100)
                .expect("replay release");
        assert_eq!(restored_release.public_state(), released.public_state());
        let released_lifecycle = lifecycle
            .finalize_release(&tle.key, 100, final_release)
            .expect("released lifecycle");
        released_lifecycle
            .validate(&tle.key)
            .expect("released lifecycle replay");
        assert!(matches!(
            released_lifecycle,
            TimedOvnLifecycleStateV1::Released(_)
        ));

        let mut tampered = sealed.public_state().clone();
        tampered.ballot_records[0][TIMED_OVN_BALLOT_RECORD_BYTES_V1 - 1] ^= 1;
        assert!(tampered.clone().validate(&tle.key).is_err());
        let mut invalid_release = final_release;
        invalid_release.signature[0] ^= 1;
        let invalid_signature_first = TimedOvnLifecycleStateV1::Sealed(tampered.clone())
            .finalize_release(&tle.key, 100, invalid_release)
            .expect_err("invalid final signature must fail before corpus replay");
        assert!(matches!(
            invalid_signature_first,
            TimedOvnEvidenceError::Release(_)
        ));
        let corpus_after_valid_signature = TimedOvnLifecycleStateV1::Sealed(tampered)
            .finalize_release(&tle.key, 100, final_release)
            .expect_err("valid final signature must not bypass full corpus replay");
        assert!(!matches!(
            corpus_after_valid_signature,
            TimedOvnEvidenceError::Release(_)
        ));
        assert_eq!(
            sealed.finalize_release(&tle.key, 99, final_release).err(),
            Some(TimedOvnEvidenceError::Release(
                TleReleaseAdapterError::ReleaseHeightNotReached
            ))
        );
        assert_eq!(
            tle.session.session_id(),
            session_record.tle_key_session_id.as_bytes()
        );
    }
}
