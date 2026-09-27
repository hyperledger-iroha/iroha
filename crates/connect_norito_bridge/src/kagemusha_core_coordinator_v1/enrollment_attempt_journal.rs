//! Bounded native enrollment attempt journal; no decoded snapshot restores Core authority.
//!
//! The selected backend supplies authenticated release/policy pins and a storage implementation
//! whose compare-and-swap is durable, authenticated and externally rollback-checked. This module
//! retains phase intent before verifier or device I/O and one exact response afterward. Reopening
//! any snapshot stops its tickets: neither an opaque Core enrollment owner nor its continuous
//! deadline can be reconstructed from bytes. There is intentionally no file-only store, software
//! authority, backend installer or transition from this journal to monetary admission.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use iroha_data_model::account::AccountId;
use norito::{
    DecodeLimits,
    codec::{Decode, Encode},
};
use rand::{TryRngCore as _, rngs::OsRng};
use sha2::{Digest, Sha256};

use super::{
    KAGEMUSHA_CORE_COORDINATOR_MAX_REQUEST_BYTES_V1, KagemushaCoreCoordinatorMethodV1,
    archive_boundary, kagemusha_core_coordinator_decode_request_v1,
    kagemusha_core_coordinator_encode_response_v1, native_deadline::NativeDeadlineV1,
};

const VERSION: u16 = 1;
const MAX_ATTEMPTS: usize = 64;
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_CHALLENGE_RESPONSE_BYTES: usize = 4 * 1024;
const MAX_PROOF_RESPONSE_BYTES: usize = 96 * 1024;
const MAX_FINISH_RESPONSE_BYTES: usize = 4 * 1024;
const DEADLINE: Duration = Duration::from_secs(120);
const REQUEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:native-enrollment-phase-request\0";

/// Closed attempt-journal failures. None constructs a verified Core capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaEnrollmentJournalErrorV1 {
    /// Invalid archive, selection, phase, or native scope.
    Invalid,
    /// Another attempt, account selection, or request already owns the slot.
    Conflict,
    /// An uncertain transition, restart, or explicit revocation froze the attempt.
    Frozen,
    /// The original suspend-inclusive native deadline expired.
    Expired,
    /// The bounded registry has no safe capacity.
    Capacity,
    /// Authenticated, monotonic, durable storage failed or returned corrupt bytes.
    Store,
    /// OS entropy or the continuous clock could not be obtained.
    Unavailable,
}

/// Result for journal operations and qualified storage implementations.
pub type KagemushaEnrollmentJournalResultV1<T> =
    std::result::Result<T, KagemushaEnrollmentJournalErrorV1>;
type Result<T> = KagemushaEnrollmentJournalResultV1<T>;

/// Atomic authenticated storage for the one canonical Norito journal snapshot.
///
/// An implementation must reject an older snapshot even if its bytes and MAC were once valid,
/// pin its hardware/external monotonic generation, fsync before returning success, and make CAS
/// linearizable across processes. No ordinary filesystem-only implementation qualifies. An
/// uncertain CAS result must return an error; the journal then permanently stops in this process.
pub trait KagemushaEnrollmentJournalStoreV1: Send + Sync + 'static {
    /// Load and independently authenticate the latest rollback-checked snapshot.
    fn load_checked(&self) -> Result<Option<Vec<u8>>>;
    /// Atomically replace only the specified canonical revision (`None` means absent).
    fn compare_and_swap(&self, previous_revision: Option<u64>, next: &[u8]) -> Result<()>;
}

/// Immutable backend-selected pins. Caller frames cannot supply these values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaEnrollmentJournalPinsV1 {
    /// Threshold-authenticated release identity.
    pub release_id: [u8; 32],
    /// Governed hardware profile identity.
    pub hardware_profile_id: [u8; 32],
    /// Independently pinned issuer policy identity.
    pub issuer_policy_id: [u8; 32],
    /// Digest of the independently pinned app verifier policy.
    pub app_policy_digest: [u8; 32],
}

impl KagemushaEnrollmentJournalPinsV1 {
    fn validate(self) -> Result<()> {
        if [
            self.release_id,
            self.hardware_profile_id,
            self.issuer_policy_id,
            self.app_policy_digest,
        ]
        .contains(&[0; 32])
        {
            Err(KagemushaEnrollmentJournalErrorV1::Invalid)
        } else {
            Ok(())
        }
    }
}

/// Original native selection. The ticket selects retained state; it carries no authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaEnrollmentJournalSelectionV1 {
    /// Canonical account identity selected from phase 1.
    pub account_i105: String,
    /// Native random nonzero process ticket.
    pub ticket: u64,
    /// Native random client nonce.
    pub client_nonce: [u8; 32],
    /// Pinned governed release.
    pub release_id: [u8; 32],
    /// Pinned governed hardware profile.
    pub hardware_profile_id: [u8; 32],
    /// Native random lane identity.
    pub lane_id: [u8; 32],
}

/// Process-local reference to the original journal ticket and suspend-inclusive deadline.
///
/// This cannot be decoded from an app cache or transferred to a different process. It carries
/// no monetary authority and cannot renew the selected nonce, ticket, lane, or deadline.
pub struct KagemushaEnrollmentLiveSelectionV1 {
    journal: Arc<KagemushaEnrollmentAttemptJournalV1>,
    selection: KagemushaEnrollmentJournalSelectionV1,
    pins: KagemushaEnrollmentJournalPinsV1,
}

impl KagemushaEnrollmentLiveSelectionV1 {
    /// Original independently persisted pins; caller frames cannot replace them.
    #[must_use]
    pub fn pins(&self) -> KagemushaEnrollmentJournalPinsV1 {
        self.pins
    }

    /// Check the exact retained journal record and original continuous deadline.
    pub fn require_live(&self) -> Result<&KagemushaEnrollmentJournalSelectionV1> {
        self.deadline()?;
        Ok(&self.selection)
    }

    /// Clone the original live deadline for the consuming initial-possession ceremony.
    /// A revoked, expired, or replaced journal record cannot hand out its clock.
    pub(super) fn deadline(&self) -> Result<NativeDeadlineV1> {
        let mut state = self.journal.lock()?;
        self.journal.require_fresh_snapshot(&mut state)?;
        let index = self
            .journal
            .active_index(&mut state, self.selection.ticket)?;
        let record = &state.image.records[index];
        if !record.matches_owner(&self.selection)
            || record.release_id != self.pins.release_id
            || record.hardware_profile_id != self.pins.hardware_profile_id
            || record.issuer_policy_id != self.pins.issuer_policy_id
            || record.app_policy_digest != self.pins.app_policy_digest
        {
            return Err(KagemushaEnrollmentJournalErrorV1::Invalid);
        }
        state
            .deadlines
            .get(&self.selection.ticket)
            .cloned()
            .ok_or(KagemushaEnrollmentJournalErrorV1::Frozen)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "connect_norito_bridge::kagemusha_core_coordinator_v1::enrollment_attempt_journal::PhaseV1",
    frame = "iroha.kagemusha.core.v1.enrollment-journal-phase"
)]
enum PhaseV1 {
    Selected,
    ChallengeInFlight,
    ChallengeReady,
    ProofInFlight,
    ProofReady,
    FinishInFlight,
    Finished,
    RecoverOnly,
    Frozen,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "connect_norito_bridge::kagemusha_core_coordinator_v1::enrollment_attempt_journal::RecordV1",
    frame = "iroha.kagemusha.core.v1.enrollment-journal-record"
)]
struct RecordV1 {
    account_i105: String,
    ticket: u64,
    client_nonce: [u8; 32],
    release_id: [u8; 32],
    hardware_profile_id: [u8; 32],
    issuer_policy_id: [u8; 32],
    app_policy_digest: [u8; 32],
    lane_id: [u8; 32],
    phase: PhaseV1,
    challenge_request_digest: [u8; 32],
    challenge_response: Vec<u8>,
    proof_request_digest: [u8; 32],
    proof_response: Vec<u8>,
    finish_request_digest: [u8; 32],
    finish_response: Vec<u8>,
}

impl RecordV1 {
    fn matches_owner(&self, owner: &KagemushaEnrollmentJournalSelectionV1) -> bool {
        self.account_i105 == owner.account_i105
            && self.ticket == owner.ticket
            && self.client_nonce == owner.client_nonce
            && self.release_id == owner.release_id
            && self.hardware_profile_id == owner.hardware_profile_id
            && self.lane_id == owner.lane_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "connect_norito_bridge::kagemusha_core_coordinator_v1::enrollment_attempt_journal::ImageV1",
    frame = "iroha.kagemusha.core.v1.enrollment-journal-image"
)]
struct ImageV1 {
    version: u16,
    revision: u64,
    records: Vec<RecordV1>,
}

impl ImageV1 {
    fn empty() -> Self {
        Self {
            version: VERSION,
            revision: 0,
            records: Vec::new(),
        }
    }

    fn decode_checked(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
            return Err(KagemushaEnrollmentJournalErrorV1::Store);
        }
        let image: Self = norito::decode_canonical_with_limits(
            bytes,
            DecodeLimits::new(
                MAX_IMAGE_BYTES,
                MAX_IMAGE_BYTES,
                MAX_IMAGE_BYTES * 2,
                MAX_IMAGE_BYTES * 4,
                32,
            ),
        )
        .map_err(|_| KagemushaEnrollmentJournalErrorV1::Store)?;
        image.validate()?;
        Ok(image)
    }

    fn encode_checked(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes =
            norito::encode_canonical(self).map_err(|_| KagemushaEnrollmentJournalErrorV1::Store)?;
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(KagemushaEnrollmentJournalErrorV1::Capacity);
        }
        Ok(bytes)
    }

    fn validate(&self) -> Result<()> {
        if self.version != VERSION || self.records.len() > MAX_ATTEMPTS {
            return Err(KagemushaEnrollmentJournalErrorV1::Store);
        }
        let mut accounts = BTreeSet::new();
        let mut tickets = BTreeSet::new();
        let mut lanes = BTreeSet::new();
        for record in &self.records {
            let account = AccountId::parse_encoded(&record.account_i105)
                .map_err(|_| KagemushaEnrollmentJournalErrorV1::Store)?;
            if account
                .canonical_i105()
                .map_err(|_| KagemushaEnrollmentJournalErrorV1::Store)?
                != record.account_i105
                || record.ticket == 0
                || [
                    record.client_nonce,
                    record.release_id,
                    record.hardware_profile_id,
                    record.issuer_policy_id,
                    record.app_policy_digest,
                    record.lane_id,
                ]
                .contains(&[0; 32])
                || !accounts.insert(&record.account_i105)
                || !tickets.insert(record.ticket)
                || !lanes.insert(record.lane_id)
                || record.challenge_response.len() > MAX_CHALLENGE_RESPONSE_BYTES
                || record.proof_response.len() > MAX_PROOF_RESPONSE_BYTES
                || record.finish_response.len() > MAX_FINISH_RESPONSE_BYTES
            {
                return Err(KagemushaEnrollmentJournalErrorV1::Store);
            }
            if (!record.challenge_response.is_empty() && record.challenge_request_digest == [0; 32])
                || (!record.proof_response.is_empty() && record.proof_request_digest == [0; 32])
                || (!record.finish_response.is_empty() && record.finish_request_digest == [0; 32])
                || (record.proof_request_digest != [0; 32] && record.challenge_response.is_empty())
                || (record.finish_request_digest != [0; 32] && record.proof_response.is_empty())
                || (record.challenge_response.is_empty()
                    && matches!(
                        record.phase,
                        PhaseV1::ChallengeReady
                            | PhaseV1::ProofInFlight
                            | PhaseV1::ProofReady
                            | PhaseV1::FinishInFlight
                            | PhaseV1::Finished
                    ))
                || (record.proof_response.is_empty()
                    && matches!(
                        record.phase,
                        PhaseV1::ProofReady | PhaseV1::FinishInFlight | PhaseV1::Finished
                    ))
                || (record.finish_response.is_empty() && record.phase == PhaseV1::Finished)
            {
                return Err(KagemushaEnrollmentJournalErrorV1::Store);
            }
        }
        Ok(())
    }
}

struct StateV1 {
    image: ImageV1,
    persisted: bool,
    deadlines: BTreeMap<u64, NativeDeadlineV1>,
    poisoned: bool,
}

/// The sole process-local owner of a durable enrollment journal snapshot.
///
/// It does not manufacture a `PendingIssuerEnrollmentV1` or an admitted wallet. A qualified
/// backend must own opaque Core state separately and call these methods only after independent
/// release, issuer, app verifier, credential and hardware checks. No backend is installed by this
/// constructor, and the stock typed enrollment hook remains `Unavailable`.
pub struct KagemushaEnrollmentAttemptJournalV1 {
    store: Arc<dyn KagemushaEnrollmentJournalStoreV1>,
    state: Mutex<StateV1>,
}

/// One-shot phase reservation. It cannot be decoded, cloned or built from an app ticket.
#[must_use]
pub struct KagemushaEnrollmentJournalReservationV1 {
    ticket: u64,
    phase: u32,
    request_digest: [u8; 32],
    request_frame: Vec<u8>,
}

/// A completed exact request recovers bytes; otherwise the caller gets one reservation for I/O.
pub enum KagemushaEnrollmentJournalDispatchV1 {
    /// Backend may verify/sign once; it must publish or leave the slot frozen.
    Execute(KagemushaEnrollmentJournalReservationV1),
    /// Original durable response to exactly the same request bytes.
    Retained(Vec<u8>),
}

impl KagemushaEnrollmentAttemptJournalV1 {
    /// Open authenticated storage. Every existing ticket becomes recover-only on reopen.
    ///
    /// This deliberately prevents host-reconstructed pending state after a process crash. A
    /// later replacement lane needs a distinct governed issuer-signed protocol, not a retry of
    /// this constructor.
    pub fn open(store: Arc<dyn KagemushaEnrollmentJournalStoreV1>) -> Result<Self> {
        let stored = store.load_checked()?;
        let (mut image, persisted) = match stored {
            Some(bytes) => (ImageV1::decode_checked(&bytes)?, true),
            None => (ImageV1::empty(), false),
        };
        if image
            .records
            .iter()
            .any(|record| !matches!(record.phase, PhaseV1::Frozen | PhaseV1::RecoverOnly))
        {
            let previous = image.revision;
            image.revision = image
                .revision
                .checked_add(1)
                .ok_or(KagemushaEnrollmentJournalErrorV1::Store)?;
            for record in &mut image.records {
                if record.phase != PhaseV1::Frozen {
                    record.phase = PhaseV1::RecoverOnly;
                }
            }
            store.compare_and_swap(Some(previous), &image.encode_checked()?)?;
        }
        Ok(Self {
            store,
            state: Mutex::new(StateV1 {
                image,
                persisted,
                deadlines: BTreeMap::new(),
                poisoned: false,
            }),
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, StateV1>> {
        let state = self
            .state
            .lock()
            .map_err(|_| KagemushaEnrollmentJournalErrorV1::Store)?;
        if state.poisoned {
            Err(KagemushaEnrollmentJournalErrorV1::Store)
        } else {
            Ok(state)
        }
    }

    fn require_fresh_snapshot(&self, state: &mut StateV1) -> Result<()> {
        let loaded = self
            .store
            .load_checked()
            .and_then(|bytes| bytes.ok_or(KagemushaEnrollmentJournalErrorV1::Store))
            .and_then(|bytes| ImageV1::decode_checked(&bytes));
        match loaded {
            Ok(image) if state.persisted && image == state.image => Ok(()),
            _ => {
                // A read failure or another process' CAS invalidates all cached answers.
                state.poisoned = true;
                state.deadlines.clear();
                Err(KagemushaEnrollmentJournalErrorV1::Store)
            }
        }
    }

    fn persist(&self, state: &mut StateV1, mut next: ImageV1) -> Result<()> {
        next.revision = state
            .image
            .revision
            .checked_add(1)
            .ok_or(KagemushaEnrollmentJournalErrorV1::Store)?;
        let bytes = next.encode_checked()?;
        let prior = state.persisted.then_some(state.image.revision);
        if self.store.compare_and_swap(prior, &bytes).is_err() {
            state.poisoned = true;
            state.deadlines.clear();
            return Err(KagemushaEnrollmentJournalErrorV1::Store);
        }
        state.image = next;
        state.persisted = true;
        Ok(())
    }

    fn active_index(&self, state: &mut StateV1, ticket: u64) -> Result<usize> {
        let index = state
            .image
            .records
            .iter()
            .position(|record| record.ticket == ticket)
            .ok_or(KagemushaEnrollmentJournalErrorV1::Invalid)?;
        if matches!(
            state.image.records[index].phase,
            PhaseV1::Frozen | PhaseV1::RecoverOnly
        ) {
            return Err(KagemushaEnrollmentJournalErrorV1::Frozen);
        }
        if !state
            .deadlines
            .get(&ticket)
            .is_some_and(|deadline| deadline.check().is_ok())
        {
            let mut next = state.image.clone();
            next.records[index].phase = PhaseV1::Frozen;
            self.persist(state, next)?;
            state.deadlines.remove(&ticket);
            return Err(KagemushaEnrollmentJournalErrorV1::Expired);
        }
        Ok(index)
    }

    /// Persist a native random ticket, nonce and lane under independently authenticated pins.
    /// Phase-1 account bytes must already have passed the ABI's strict frame checks.
    pub fn select(
        &self,
        request_frame: &[u8],
        pins: KagemushaEnrollmentJournalPinsV1,
    ) -> Result<(KagemushaEnrollmentJournalSelectionV1, Vec<u8>)> {
        let method = KagemushaCoreCoordinatorMethodV1::InitialEnrollment;
        archive_boundary::validate_request(method, request_frame)
            .map_err(|_| KagemushaEnrollmentJournalErrorV1::Invalid)?;
        let fields = kagemusha_core_coordinator_decode_request_v1(request_frame)
            .map_err(|_| KagemushaEnrollmentJournalErrorV1::Invalid)?;
        if phase(&fields)? != 1 {
            return Err(KagemushaEnrollmentJournalErrorV1::Invalid);
        }
        pins.validate()?;
        let account_i105 = String::from_utf8(fields[1].clone())
            .map_err(|_| KagemushaEnrollmentJournalErrorV1::Invalid)?;
        let mut state = self.lock()?;
        if state.image.records.len() >= MAX_ATTEMPTS {
            return Err(KagemushaEnrollmentJournalErrorV1::Capacity);
        }
        if state
            .image
            .records
            .iter()
            .any(|record| record.account_i105 == account_i105)
        {
            return Err(KagemushaEnrollmentJournalErrorV1::Conflict);
        }
        let deadline = NativeDeadlineV1::start(DEADLINE)
            .map_err(|_| KagemushaEnrollmentJournalErrorV1::Unavailable)?;
        let ticket = (0..16)
            .find_map(|_| {
                let mut raw = [0; 8];
                OsRng.try_fill_bytes(&mut raw).ok()?;
                let candidate = u64::from_le_bytes(raw);
                (candidate != 0
                    && !state
                        .image
                        .records
                        .iter()
                        .any(|record| record.ticket == candidate))
                .then_some(candidate)
            })
            .ok_or(KagemushaEnrollmentJournalErrorV1::Unavailable)?;
        let client_nonce = random_nonzero()?;
        let lane_id = (0..16)
            .find_map(|_| {
                let candidate = random_nonzero().ok()?;
                (!state
                    .image
                    .records
                    .iter()
                    .any(|record| record.lane_id == candidate))
                .then_some(candidate)
            })
            .ok_or(KagemushaEnrollmentJournalErrorV1::Unavailable)?;
        let record = RecordV1 {
            account_i105: account_i105.clone(),
            ticket,
            client_nonce,
            release_id: pins.release_id,
            hardware_profile_id: pins.hardware_profile_id,
            issuer_policy_id: pins.issuer_policy_id,
            app_policy_digest: pins.app_policy_digest,
            lane_id,
            phase: PhaseV1::Selected,
            challenge_request_digest: [0; 32],
            challenge_response: Vec::new(),
            proof_request_digest: [0; 32],
            proof_response: Vec::new(),
            finish_request_digest: [0; 32],
            finish_response: Vec::new(),
        };
        let mut next = state.image.clone();
        next.records.push(record);
        self.persist(&mut state, next)?;
        state.deadlines.insert(ticket, deadline);
        let response = kagemusha_core_coordinator_encode_response_v1(&[
            ticket.to_le_bytes().to_vec(),
            client_nonce.to_vec(),
            pins.release_id.to_vec(),
            pins.hardware_profile_id.to_vec(),
            lane_id.to_vec(),
        ])
        .map_err(|_| KagemushaEnrollmentJournalErrorV1::Invalid)?;
        archive_boundary::validate_response(method, request_frame, &response)
            .map_err(|_| KagemushaEnrollmentJournalErrorV1::Invalid)?;
        Ok((
            KagemushaEnrollmentJournalSelectionV1 {
                account_i105,
                ticket,
                client_nonce,
                release_id: pins.release_id,
                hardware_profile_id: pins.hardware_profile_id,
                lane_id,
            },
            response,
        ))
    }

    /// Retain the exact phase-1 ticket under the originally persisted independent pins.
    /// This never calls `select` and therefore cannot create a second nonce or deadline.
    pub fn retain_live(
        self: &Arc<Self>,
        selection: KagemushaEnrollmentJournalSelectionV1,
        pins: KagemushaEnrollmentJournalPinsV1,
    ) -> Result<KagemushaEnrollmentLiveSelectionV1> {
        pins.validate()?;
        let live = KagemushaEnrollmentLiveSelectionV1 {
            journal: Arc::clone(self),
            selection,
            pins,
        };
        live.require_live()?;
        Ok(live)
    }

    /// Persist one phase intent before verifier or device I/O, or return its exact prior result.
    /// Only phases 2, 3 and 5 use this entry point; phase 4 reads retained proof bytes.
    pub fn reserve(
        &self,
        owner: &KagemushaEnrollmentJournalSelectionV1,
        request_frame: &[u8],
    ) -> Result<KagemushaEnrollmentJournalDispatchV1> {
        let method = KagemushaCoreCoordinatorMethodV1::InitialEnrollment;
        archive_boundary::validate_request(method, request_frame)
            .map_err(|_| KagemushaEnrollmentJournalErrorV1::Invalid)?;
        let fields = kagemusha_core_coordinator_decode_request_v1(request_frame)
            .map_err(|_| KagemushaEnrollmentJournalErrorV1::Invalid)?;
        let phase = phase(&fields)?;
        if !matches!(phase, 2 | 3 | 5)
            || request_frame.len() > KAGEMUSHA_CORE_COORDINATOR_MAX_REQUEST_BYTES_V1
        {
            return Err(KagemushaEnrollmentJournalErrorV1::Invalid);
        }
        let ticket = ticket(&fields)?;
        let digest = request_digest(phase, request_frame);
        let mut state = self.lock()?;
        let index = state
            .image
            .records
            .iter()
            .position(|record| record.ticket == ticket)
            .ok_or(KagemushaEnrollmentJournalErrorV1::Invalid)?;
        let record = &state.image.records[index];
        if !record.matches_owner(owner) || owner.ticket != ticket {
            return Err(KagemushaEnrollmentJournalErrorV1::Invalid);
        }
        let (before, inflight, ready, old_digest, old_response) = match phase {
            2 => (
                PhaseV1::Selected,
                PhaseV1::ChallengeInFlight,
                PhaseV1::ChallengeReady,
                record.challenge_request_digest,
                &record.challenge_response,
            ),
            3 => (
                PhaseV1::ChallengeReady,
                PhaseV1::ProofInFlight,
                PhaseV1::ProofReady,
                record.proof_request_digest,
                &record.proof_response,
            ),
            5 => (
                PhaseV1::ProofReady,
                PhaseV1::FinishInFlight,
                PhaseV1::Finished,
                record.finish_request_digest,
                &record.finish_response,
            ),
            _ => unreachable!(),
        };
        if record.phase != PhaseV1::Frozen && old_digest == digest && !old_response.is_empty() {
            let response = old_response.clone();
            self.require_fresh_snapshot(&mut state)?;
            archive_boundary::validate_response(method, request_frame, &response)
                .map_err(|_| KagemushaEnrollmentJournalErrorV1::Store)?;
            return Ok(KagemushaEnrollmentJournalDispatchV1::Retained(response));
        }
        if record.phase == ready {
            return Err(KagemushaEnrollmentJournalErrorV1::Conflict);
        }
        if record.phase != before {
            return Err(KagemushaEnrollmentJournalErrorV1::Frozen);
        }
        if phase == 2 {
            // This comparison is independent of the issuer signature check. The qualified
            // backend must verify that signature and the raw app evidence before publishing.
            let prep = &fields[2];
            if prep[0] != 1
                || prep[17..49] != record.client_nonce
                || prep[81..113] != record.release_id
                || prep[113..145] != record.hardware_profile_id
                || prep[177..209] != record.lane_id
            {
                return Err(KagemushaEnrollmentJournalErrorV1::Invalid);
            }
        }
        self.active_index(&mut state, ticket)?;
        let mut next = state.image.clone();
        let changed = &mut next.records[index];
        changed.phase = inflight;
        match phase {
            2 => changed.challenge_request_digest = digest,
            3 => changed.proof_request_digest = digest,
            5 => changed.finish_request_digest = digest,
            _ => unreachable!(),
        }
        self.persist(&mut state, next)?;
        Ok(KagemushaEnrollmentJournalDispatchV1::Execute(
            KagemushaEnrollmentJournalReservationV1 {
                ticket,
                phase,
                request_digest: digest,
                request_frame: request_frame.to_vec(),
            },
        ))
    }

    /// Persist the *already independently verified* complete response before exposing it.
    /// An invalid response leaves the consumed phase in flight and cannot authorize a retry.
    pub fn publish(
        &self,
        reservation: KagemushaEnrollmentJournalReservationV1,
        response_frame: &[u8],
    ) -> Result<Vec<u8>> {
        let maximum = match reservation.phase {
            2 => MAX_CHALLENGE_RESPONSE_BYTES,
            3 => MAX_PROOF_RESPONSE_BYTES,
            5 => MAX_FINISH_RESPONSE_BYTES,
            _ => return Err(KagemushaEnrollmentJournalErrorV1::Invalid),
        };
        if response_frame.is_empty() || response_frame.len() > maximum {
            return Err(KagemushaEnrollmentJournalErrorV1::Invalid);
        }
        archive_boundary::validate_response(
            KagemushaCoreCoordinatorMethodV1::InitialEnrollment,
            &reservation.request_frame,
            response_frame,
        )
        .map_err(|_| KagemushaEnrollmentJournalErrorV1::Invalid)?;
        let mut state = self.lock()?;
        let index = self.active_index(&mut state, reservation.ticket)?;
        let record = &state.image.records[index];
        let expected = match reservation.phase {
            2 => (PhaseV1::ChallengeInFlight, record.challenge_request_digest),
            3 => (PhaseV1::ProofInFlight, record.proof_request_digest),
            5 => (PhaseV1::FinishInFlight, record.finish_request_digest),
            _ => unreachable!(),
        };
        if record.phase != expected.0
            || expected.1 != reservation.request_digest
            || request_digest(reservation.phase, &reservation.request_frame)
                != reservation.request_digest
        {
            return Err(KagemushaEnrollmentJournalErrorV1::Frozen);
        }
        let mut next = state.image.clone();
        let changed = &mut next.records[index];
        match reservation.phase {
            2 => {
                changed.challenge_response = response_frame.to_vec();
                changed.phase = PhaseV1::ChallengeReady;
            }
            3 => {
                changed.proof_response = response_frame.to_vec();
                changed.phase = PhaseV1::ProofReady;
            }
            5 => {
                changed.finish_response = response_frame.to_vec();
                changed.phase = PhaseV1::Finished;
            }
            _ => unreachable!(),
        }
        self.persist(&mut state, next)?;
        Ok(response_frame.to_vec())
    }

    /// Read only the exact durable proof response. This never dispatches a second signature.
    pub fn read_proof(
        &self,
        owner: &KagemushaEnrollmentJournalSelectionV1,
        request_frame: &[u8],
    ) -> Result<Vec<u8>> {
        let method = KagemushaCoreCoordinatorMethodV1::InitialEnrollment;
        archive_boundary::validate_request(method, request_frame)
            .map_err(|_| KagemushaEnrollmentJournalErrorV1::Invalid)?;
        let fields = kagemusha_core_coordinator_decode_request_v1(request_frame)
            .map_err(|_| KagemushaEnrollmentJournalErrorV1::Invalid)?;
        if phase(&fields)? != 4 {
            return Err(KagemushaEnrollmentJournalErrorV1::Invalid);
        }
        let mut state = self.lock()?;
        self.require_fresh_snapshot(&mut state)?;
        let id = ticket(&fields)?;
        let index = state
            .image
            .records
            .iter()
            .position(|record| record.ticket == id)
            .ok_or(KagemushaEnrollmentJournalErrorV1::Invalid)?;
        let record = &state.image.records[index];
        if !record.matches_owner(owner) || owner.ticket != id {
            return Err(KagemushaEnrollmentJournalErrorV1::Invalid);
        }
        if record.phase == PhaseV1::Frozen || record.proof_response.is_empty() {
            return Err(KagemushaEnrollmentJournalErrorV1::Frozen);
        }
        let response = record.proof_response.clone();
        archive_boundary::validate_response(method, request_frame, &response)
            .map_err(|_| KagemushaEnrollmentJournalErrorV1::Store)?;
        Ok(response)
    }

    /// Revoke one ticket without freeing its account or lane for ungoverned replacement.
    pub fn cancel(
        &self,
        owner: &KagemushaEnrollmentJournalSelectionV1,
        request_frame: &[u8],
    ) -> Result<()> {
        let method = KagemushaCoreCoordinatorMethodV1::InitialEnrollment;
        archive_boundary::validate_request(method, request_frame)
            .map_err(|_| KagemushaEnrollmentJournalErrorV1::Invalid)?;
        let fields = kagemusha_core_coordinator_decode_request_v1(request_frame)
            .map_err(|_| KagemushaEnrollmentJournalErrorV1::Invalid)?;
        if phase(&fields)? != 6 {
            return Err(KagemushaEnrollmentJournalErrorV1::Invalid);
        }
        let id = ticket(&fields)?;
        let mut state = self.lock()?;
        let index = state
            .image
            .records
            .iter()
            .position(|record| record.ticket == id)
            .ok_or(KagemushaEnrollmentJournalErrorV1::Invalid)?;
        if !state.image.records[index].matches_owner(owner) || owner.ticket != id {
            return Err(KagemushaEnrollmentJournalErrorV1::Invalid);
        }
        if state.image.records[index].phase != PhaseV1::Frozen {
            let mut next = state.image.clone();
            next.records[index].phase = PhaseV1::Frozen;
            self.persist(&mut state, next)?;
        }
        state.deadlines.remove(&id);
        Ok(())
    }

    /// Close or account-switch revocation. No record is erased or made selectable again.
    pub fn revoke_all(&self) -> Result<()> {
        let mut state = self.lock()?;
        if state
            .image
            .records
            .iter()
            .any(|record| record.phase != PhaseV1::Frozen)
        {
            let mut next = state.image.clone();
            for record in &mut next.records {
                record.phase = PhaseV1::Frozen;
            }
            self.persist(&mut state, next)?;
        }
        state.deadlines.clear();
        Ok(())
    }
}

fn phase(fields: &[Vec<u8>]) -> Result<u32> {
    let bytes: [u8; 4] = fields
        .first()
        .ok_or(KagemushaEnrollmentJournalErrorV1::Invalid)?
        .as_slice()
        .try_into()
        .map_err(|_| KagemushaEnrollmentJournalErrorV1::Invalid)?;
    Ok(u32::from_le_bytes(bytes))
}

fn ticket(fields: &[Vec<u8>]) -> Result<u64> {
    let bytes: [u8; 8] = fields
        .get(1)
        .ok_or(KagemushaEnrollmentJournalErrorV1::Invalid)?
        .as_slice()
        .try_into()
        .map_err(|_| KagemushaEnrollmentJournalErrorV1::Invalid)?;
    let ticket = u64::from_le_bytes(bytes);
    if ticket == 0 {
        Err(KagemushaEnrollmentJournalErrorV1::Invalid)
    } else {
        Ok(ticket)
    }
}

fn request_digest(phase: u32, request_frame: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(REQUEST_DOMAIN);
    hash.update(phase.to_le_bytes());
    hash.update((request_frame.len() as u64).to_le_bytes());
    hash.update(request_frame);
    hash.finalize().into()
}

fn random_nonzero() -> Result<[u8; 32]> {
    let mut result = [0; 32];
    OsRng
        .try_fill_bytes(&mut result)
        .map_err(|_| KagemushaEnrollmentJournalErrorV1::Unavailable)?;
    if result == [0; 32] {
        Err(KagemushaEnrollmentJournalErrorV1::Unavailable)
    } else {
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
