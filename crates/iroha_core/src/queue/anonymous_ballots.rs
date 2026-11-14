//! The queue's partition of signerless anonymous ballots
//! (`specs/parliament_private_ballot_design.md` §3.6).
//!
//! A ballot has no signer, fee, creation time or TTL, so none of the account, fee, manifest
//! or sequence steps of ordinary admission applies to it. It is admitted here instead, in
//! the order the design fixes:
//!
//! 1. this root carries ballots at all: its native route is lane zero of a global root whose
//!    selection installs the entrypoint, and the carrier fits the global proposer's budget;
//! 2. the cheap residence checks: the carrier is not already resident, and neither the queue
//!    nor its ballot partition is full;
//! 3. the seven ordered admission checks of the protocol crate
//!    (`iroha_parliament_ballot::ballot::precheck_admission_v1`) against the board the
//!    committed view keeps for the scope and the node's local clock: size, canonical frame
//!    and network, a known scope whose window is open or locally due, the V1 profile, the
//!    stamp, the nullifier unused in committed state, and no verified pending holder of
//!    `(scope, nullifier)`;
//! 4. only then the proof, under the partition's verifier bound;
//! 5. the state checks again, atomically with claiming `(scope, nullifier)` for the ballot;
//! 6. residence, through the same resident ledger, FIFO, lease and gossip backlog as every
//!    other input.
//!
//! A nullifier is therefore never reserved for a ballot that has not verified, a verified
//! pending ballot blocks rerandomized copies before their verification, and gossip forwards
//! verified ballots only: nothing else becomes resident. Receivers run the same pipeline.
//!
//! A resident ballot is identified by `(scope, nullifier)`, never by its carrier hash, and
//! committed carriers of ballots join no replay membership. Committed State decides it for
//! good when its nullifier is in its scope's corpus, when its ballot closed, or when its
//! board is gone ([`Queue::anonymous_ballot_decided`]): readers then treat it as committed
//! and the next cleanup retires it. A ballot that a block recorded as outside its window
//! before the window opened stays resident and is offered again.
//!
//! The proposer is offered ballots through the queue's bounded FIFO window
//! ([`BallotOfferV1`]): only ballots whose window committed State has opened and the local
//! clock has not closed, at most the per-block budget `B_work`/`B_bytes` the view commits.
//! Block validation counts with the same budget against the same parent, and execution
//! rechecks every ballot against committed State.
//!
//! **Node-local bounds.** The partition has its own budget, configured in
//! `[queue.anonymous_ballots]` (`iroha_config`): the verified ballots it keeps resident and
//! their retained bytes, the proofs it verifies at once and the verifications it starts per
//! second. Its residents never count against the ordinary queue's capacity, byte budget or
//! age pressure, and ordinary transactions never consume its own. The bounds refuse or delay
//! a submission (`Error::Full`); they never decide whether a ballot is valid.
//!
//! TODO(design §3.6 "Resource bounds", §3.10): priority by stamp work when the partition is
//! full and the bounded exact-payload verification cache are not implemented; the working
//! defaults are unqualified, and their qualification belongs to the saturation obligations
//! of design §13.

use std::collections::BTreeMap;

use iroha_data_model::{
    governance::anonymous_ballot::{
        BallotScopeV1, CanonicalAnonymousBallotEnvelopeV1, NullifierV1,
        ParliamentBallotBoardRecordV1,
    },
    parameter::system::TransactionInclusionRoute,
};
use iroha_parliament_ballot::{
    ballot::{AdmissionErrorV1, AdmissionRejectionV1, BallotProofErrorV1, precheck_admission_v1},
    board::BoardsAtAdmissionV1,
    corpus::closing_ms_v1,
};
use mv::storage::StorageReadOnly;

use super::{
    EntrypointHash, Error, Failure, PreparedQueueAdmission, Queue, RoutingDecision, RoutingPlan,
    TeuBacklogPublication, state_view_height_for_routing, validate_current_admission_route,
};
use crate::{
    governance::parliament_anonymous::ballot_input::{
        BallotBlockBudgetV1, ScopeBoardV1, verifier_input_bytes,
    },
    state::{StateReadOnly, StateView, WorldReadOnly},
    sumeragi::lanes::routing::{GLOBAL_LANE, RoutingSnapshot},
    tx::AcceptedTransaction,
};
use iroha_data_model::transaction::TransactionEntrypoint;

/// One resident ballot of the partition.
#[derive(Debug, Clone, Copy)]
struct PendingBallotV1 {
    identity: (BallotScopeV1, NullifierV1),
    /// The carrier's retained-byte cost, charged to the partition's own byte budget.
    retained_bytes: u64,
}

/// The verified pending ballots of the queue, by identity and by carrier. Maintained with
/// the queue's residents: an identity is claimed after its ballot verified and released
/// when its carrier leaves the queue. The partition's node-local bounds are
/// `[queue.anonymous_ballots]` (`iroha_config`): its residents and their bytes never count
/// against the ordinary queue's capacity, and ordinary transactions never consume its own.
#[derive(Debug, Default)]
pub(super) struct PendingBallotIndexV1 {
    by_identity: BTreeMap<(BallotScopeV1, NullifierV1), EntrypointHash>,
    by_carrier: BTreeMap<EntrypointHash, PendingBallotV1>,
    retained_bytes: u64,
}

/// The verification-rate bucket of the partition: `verification_rate_per_sec` proofs per
/// second with a burst of `verification_burst`, over every local source (HTTP ingress and
/// gossip alike). It is refilled from the queue's own clock and never decides whether a
/// ballot is valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VerificationRateV1 {
    /// Thousandths of a verification available; one verification takes 1,000.
    milli_tokens: u64,
    /// The local time of the last refill.
    refilled_at_ms: u64,
}

impl VerificationRateV1 {
    /// A full bucket at the local time `now_ms`.
    pub(super) fn full(burst: std::num::NonZeroU32, now_ms: u64) -> Self {
        Self {
            milli_tokens: u64::from(burst.get()) * 1_000,
            refilled_at_ms: now_ms,
        }
    }

    /// Take one verification at `now_ms`; `false`, taking nothing, while the bucket is empty.
    pub(super) fn take(
        &mut self,
        rate_per_sec: std::num::NonZeroU32,
        burst: std::num::NonZeroU32,
        now_ms: u64,
    ) -> bool {
        // `rate` verifications per second are `rate` thousandths per millisecond.
        let refill = now_ms
            .saturating_sub(self.refilled_at_ms)
            .saturating_mul(u64::from(rate_per_sec.get()));
        self.milli_tokens = self
            .milli_tokens
            .saturating_add(refill)
            .min(u64::from(burst.get()) * 1_000);
        self.refilled_at_ms = self.refilled_at_ms.max(now_ms);
        if self.milli_tokens < 1_000 {
            return false;
        }
        self.milli_tokens -= 1_000;
        true
    }
}

impl PendingBallotIndexV1 {
    /// Whether a verified pending ballot holds `(scope, nullifier)`.
    pub(super) fn holds(&self, scope: &BallotScopeV1, nullifier: &NullifierV1) -> bool {
        self.by_identity.contains_key(&(*scope, *nullifier))
    }

    /// The identity of the resident ballot `carrier`, if it is one.
    pub(super) fn identity(
        &self,
        carrier: &EntrypointHash,
    ) -> Option<(BallotScopeV1, NullifierV1)> {
        self.by_carrier.get(carrier).map(|pending| pending.identity)
    }

    /// Whether `carrier` is a resident ballot of the partition.
    pub(super) fn is_resident(&self, carrier: &EntrypointHash) -> bool {
        self.by_carrier.contains_key(carrier)
    }

    /// Resident ballots.
    pub(super) fn len(&self) -> usize {
        self.by_carrier.len()
    }

    /// The retained-byte cost of the resident ballots.
    pub(super) fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }

    /// Claim `identity` for the verified ballot `carrier`, whose residence costs
    /// `retained_bytes`. Returns `false`, claiming nothing, when another ballot holds the
    /// identity or the carrier is already indexed.
    fn claim(
        &mut self,
        identity: (BallotScopeV1, NullifierV1),
        carrier: EntrypointHash,
        retained_bytes: u64,
    ) -> bool {
        // SPEC: §13.4 HC813 — a pending `(scope, nullifier)` has one verified holder.
        let held = self.by_identity.contains_key(&identity)
            && !cfg!(all(test, sumeragi_core_mutation = "HC813"));
        if held || self.by_carrier.contains_key(&carrier) {
            return false;
        }
        self.by_identity.insert(identity, carrier);
        self.by_carrier.insert(
            carrier,
            PendingBallotV1 {
                identity,
                retained_bytes,
            },
        );
        self.retained_bytes = self.retained_bytes.saturating_add(retained_bytes);
        true
    }

    /// Release the identity `carrier` holds, if any.
    pub(super) fn release(&mut self, carrier: &EntrypointHash) {
        if let Some(pending) = self.by_carrier.remove(carrier) {
            self.by_identity.remove(&pending.identity);
            self.retained_bytes = self.retained_bytes.saturating_sub(pending.retained_bytes);
        }
    }
}

/// One slot of the partition's verifier bound, returned when dropped.
struct VerifierPermit<'queue>(&'queue std::sync::atomic::AtomicUsize);

impl Drop for VerifierPermit<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}

/// The ballot budget of a block built on the offer's view.
#[derive(Debug)]
enum OfferBudgetV1 {
    /// No ballot was offered yet: the committed policy is read with the first one.
    Unread,
    /// The committed policy does not decode or is out of range: nothing is offered.
    Refused,
    /// What the block may still carry.
    Remaining(BallotBlockBudgetV1),
}

/// What one proposal window may still take of the block's ballot budget.
#[derive(Debug)]
pub(super) struct BallotOfferV1 {
    budget: OfferBudgetV1,
    now_ms: u64,
}

impl BallotOfferV1 {
    /// The offer of a window scanned at the local time `now_ms`.
    pub(super) const fn at(now_ms: u64) -> Self {
        Self {
            budget: OfferBudgetV1::Unread,
            now_ms,
        }
    }
}

/// The closing time of `board`, once its casting clock is established.
fn closing_ms(board: &ParliamentBallotBoardRecordV1) -> Option<u64> {
    let clock = board.casting_clock?;
    closing_ms_v1(
        clock.timestamp_ms,
        board.context.policy.binding.casting_window_ms,
    )
}

/// The queue's refusal of a submission the admission checks rejected.
fn rejection(rejected: &AdmissionRejectionV1) -> Error {
    match rejected {
        // Committed State already accepted the nullifier: the ballot is applied.
        AdmissionRejectionV1::NullifierCommitted => Error::InBlockchain,
        // A verified pending ballot already holds the identity.
        AdmissionRejectionV1::NullifierPending => Error::IsInQueue,
        rejected => Error::UnsupportedTransactionAdmission {
            reason: format!("{}: {rejected}", rejected.code()),
        },
    }
}

/// A local inability to check or verify: never a verdict on the submission.
fn unavailable() -> Error {
    Error::Deferred(ivm::error::ExecutionDeferral::VerifierArtifactsUnavailable.into())
}

impl Queue {
    /// The partition's residents and their retained bytes: outside the ordinary budgets.
    pub(super) fn ballot_residents(&self) -> (usize, u64) {
        let pending = self.pending_ballots.lock();
        (pending.len(), pending.retained_bytes())
    }

    /// Whether `carrier` is a resident ballot of the partition.
    pub(super) fn is_resident_ballot(&self, carrier: &EntrypointHash) -> bool {
        self.pending_ballots.lock().is_resident(carrier)
    }

    /// The residents that count against the ordinary capacity: every one but a ballot.
    pub(super) fn ordinary_active_len(&self) -> usize {
        self.active_len().saturating_sub(self.ballot_residents().0)
    }

    /// The retained bytes that count against the ordinary byte budget.
    pub(super) fn ordinary_retained_bytes(&self) -> u64 {
        self.retained_bytes()
            .saturating_sub(self.ballot_residents().1)
    }

    /// Whether the partition can keep one more ballot whose residence costs `retained_cost`,
    /// within its own count and byte budget.
    fn ballot_partition_has_room(
        pending: &PendingBallotIndexV1,
        bounds: &iroha_config::parameters::actual::QueueAnonymousBallots,
        retained_cost: u64,
    ) -> bool {
        pending.len() < bounds.pending_limit.get()
            && pending.retained_bytes().saturating_add(retained_cost)
                <= bounds.max_retained_bytes.get()
    }

    /// Start one proof verification at the local time `now_ms`, or `false` while the
    /// partition's verification rate is spent.
    fn anonymous_ballot_verification_admitted(&self, now_ms: u64) -> bool {
        let bounds = &self.ballot_bounds;
        self.ballot_verification_rate
            .lock()
            .get_or_insert_with(|| VerificationRateV1::full(bounds.verification_burst, now_ms))
            .take(
                bounds.verification_rate_per_sec,
                bounds.verification_burst,
                now_ms,
            )
    }

    /// Take one verifier slot, or `None` while `queue.anonymous_ballots.verifier_concurrency`
    /// proofs are being verified.
    fn anonymous_ballot_verifier(&self) -> Option<VerifierPermit<'_>> {
        use std::sync::atomic::Ordering;
        let mut busy = self.ballot_verifications.load(Ordering::Acquire);
        loop {
            if busy >= self.ballot_bounds.verifier_concurrency.get() {
                return None;
            }
            match self.ballot_verifications.compare_exchange_weak(
                busy,
                busy + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(VerifierPermit(&self.ballot_verifications)),
                Err(actual) => busy = actual,
            }
        }
    }

    /// Admit the signerless ballot `tx` against the committed `view`, on the physical
    /// `routing_plan` the installed router gave it (module documentation).
    ///
    /// # Errors
    /// The submission with its refusal: [`Error::UnsupportedTransactionAdmission`] carrying
    /// the stable admission code for a rejected ballot, [`Error::InBlockchain`] for a
    /// committed nullifier, [`Error::IsInQueue`] for a pending one or a resident carrier,
    /// [`Error::Full`] as backpressure, and [`Error::Deferred`] when the node cannot check
    /// it. Nothing is reserved for a refused submission.
    #[expect(
        clippy::too_many_lines,
        reason = "the admission order of design §3.6 reads as one sequence"
    )]
    pub(super) fn admit_anonymous_ballot_in_view(
        &self,
        tx: AcceptedTransaction<'static>,
        routing_plan: RoutingPlan,
        view: &StateView<'_>,
        teu_backlog: TeuBacklogPublication,
    ) -> Result<RoutingDecision, Failure> {
        let refuse = |err: Error, tx: AcceptedTransaction<'static>| Failure {
            tx: Box::new(tx),
            err,
        };
        let TransactionEntrypoint::AnonymousBallot(envelope) = tx.entrypoint() else {
            return Err(refuse(
                Error::AdmissionInvariant {
                    reason: "ballot admission received another entrypoint".to_owned(),
                },
                tx,
            ));
        };

        // 1. This root carries ballots, and a proposer can select this carrier.
        if let Err(err) = self.check_startup_admission() {
            return Err(refuse(err, tx));
        }
        if let Err(err) = validate_current_admission_route(&routing_plan) {
            return Err(refuse(err, tx));
        }
        let Some(height) = state_view_height_for_routing(view).checked_add(1) else {
            return Err(refuse(
                Error::UnresolvedRoute {
                    reason: "admission height overflows".to_owned(),
                },
                tx,
            ));
        };
        let native = match RoutingSnapshot::of(view)
            .and_then(|routing| routing.inputs(view.world()).route(&tx, height))
        {
            Ok(native) => native,
            Err(original) => return Err(refuse(Error::Deferred(original), tx)),
        };
        if native != Some(GLOBAL_LANE) {
            return Err(refuse(
                Error::UnsupportedTransactionAdmission {
                    reason: "anonymous_ballot_not_admitted: this root carries no anonymous ballots"
                        .to_owned(),
                },
                tx,
            ));
        }
        let encoded_len = Self::compute_tx_encoded_len(&tx);
        if let Err(never) = view.world().parameters().check_signed_transaction_bytes(
            u64::try_from(encoded_len).unwrap_or(u64::MAX),
            TransactionInclusionRoute::Global,
        ) {
            return Err(refuse(
                Error::UnsupportedTransactionAdmission {
                    reason: never.to_string(),
                },
                tx,
            ));
        }

        // 2. Cheap residence checks, before any ballot work.
        let _ = self.cull_expired_entries_if_due();
        let hash = tx.hash_as_entrypoint();
        let retained_cost = Self::retained_byte_cost(encoded_len);
        let cheap = if self.admission_faulted() {
            Some(Error::AdmissionInvariant {
                reason: "queue admission is unavailable".to_owned(),
            })
        } else if self.txs.contains_key(&hash) {
            Some(Error::IsInQueue)
        } else if !Self::ballot_partition_has_room(
            &self.pending_ballots.lock(),
            &self.ballot_bounds,
            retained_cost,
        ) {
            Some(Error::Full)
        } else {
            None
        };
        if let Some(err) = cheap {
            return Err(refuse(err, tx));
        }

        // 3. The seven ordered checks, against the committed board and the local clock.
        let now_ms = Self::duration_to_millis(self.time_source.get_unix_time());
        let network_id = *view.network_id();
        let store = ScopeBoardV1::new(view.world().parliament_anonymous_boards());
        let admitted = precheck_admission_v1(
            envelope.as_bytes(),
            &BoardsAtAdmissionV1::new(network_id, now_ms, &store, |scope, nullifier| {
                self.pending_ballots.lock().holds(scope, nullifier)
            }),
        );
        // A stored board that does not recompute answered "no board": not a verdict.
        if store.refusal().is_some() {
            return Err(refuse(unavailable(), tx));
        }
        let admitted = match admitted {
            Ok(admitted) => admitted,
            Err(AdmissionErrorV1::Rejected(rejected)) => {
                return Err(refuse(rejection(&rejected), tx));
            }
            Err(AdmissionErrorV1::Unavailable(_)) => return Err(refuse(unavailable(), tx)),
        };

        // 4. The proof, under the verifier bounds: a verifier slot, then the verification
        // rate. Nothing is reserved for the ballot yet.
        let Some(permit) = self.anonymous_ballot_verifier() else {
            return Err(refuse(Error::Full, tx));
        };
        if !self.anonymous_ballot_verification_admitted(now_ms) {
            return Err(refuse(Error::Full, tx));
        }
        let verified = admitted.verify_proof();
        drop(permit);
        let verified = match verified {
            Ok(verified) => verified,
            Err(
                error @ (BallotProofErrorV1::Rejected | BallotProofErrorV1::ProofLength { .. }),
            ) => {
                return Err(refuse(
                    Error::UnsupportedTransactionAdmission {
                        reason: format!("anonymous_ballot_proof_rejected: {error}"),
                    },
                    tx,
                ));
            }
            // The prechecks bound the ballot to this context; any other refusal is local.
            Err(_) => return Err(refuse(unavailable(), tx)),
        };

        // 5. The state checks again, atomically with claiming the identity.
        {
            let mut pending = self.pending_ballots.lock();
            let rechecked = verified.recheck(&BoardsAtAdmissionV1::new(
                network_id,
                now_ms,
                &store,
                |scope, nullifier| pending.holds(scope, nullifier),
            ));
            if let Err(rejected) = rechecked {
                drop(pending);
                return Err(refuse(rejection(&rejected), tx));
            }
            if !Self::ballot_partition_has_room(&pending, &self.ballot_bounds, retained_cost) {
                drop(pending);
                return Err(refuse(Error::Full, tx));
            }
            if !pending.claim(verified.identity(), hash, retained_cost) {
                drop(pending);
                return Err(refuse(Error::IsInQueue, tx));
            }
        }

        // 6. Residence. A ballot's carrier is in no committed replay membership, so the
        // committed-hash check never refuses it; its queue residence expires by the queue's
        // own time to live, like a sealed commitment's.
        let route = routing_plan.coordinator_route();
        let checked = match tx.into_checked(view) {
            Ok(checked) => checked,
            Err((tx, _)) => {
                self.pending_ballots.lock().release(&hash);
                return Err(refuse(Error::InBlockchain, tx));
            }
        };
        let enqueued_at_ms = self.validation_timestamp_ms(checked.as_accepted());
        let prepared = PreparedQueueAdmission {
            hash,
            sccp_exempt: None,
            routing_decision: route,
            routing_plan,
            encoded_len,
            // Fee-exempt and outside ordinary gas: `B_work` and `B_bytes` bound ballots.
            proposal_gas_cost: 0,
            enqueued_at_ms,
            expires_at: self.expiry_deadline_with_enqueue_timestamp(
                checked.as_accepted(),
                Some(enqueued_at_ms),
            ),
            frame_digest: Self::entrypoint_frame_digest(checked.as_accepted()),
            fee_reservation: None,
            #[cfg(feature = "telemetry")]
            pending_teu: 0,
            checked,
        };
        #[cfg(feature = "telemetry")]
        let telemetry = Some(view.telemetry);
        #[cfg(not(feature = "telemetry"))]
        let telemetry = None;
        match self.enqueue_prepared_admissions(vec![prepared], telemetry, &view.execution_budget())
        {
            Ok(notifications) => {
                #[cfg(feature = "telemetry")]
                if teu_backlog == TeuBacklogPublication::AfterAdmission {
                    self.publish_teu_backlog_metrics(telemetry);
                }
                #[cfg(not(feature = "telemetry"))]
                let _ = teu_backlog;
                self.publish_admission_notifications(&notifications);
                Ok(route)
            }
            Err((notifications, failure)) => {
                self.pending_ballots.lock().release(&hash);
                self.publish_admission_notifications(&notifications);
                Err(failure)
            }
        }
    }

    /// Whether committed State has decided the resident ballot `carrier` for good: its
    /// nullifier is in its scope's corpus, its ballot closed, or its board is gone. Such a
    /// ballot can never be accepted; readers treat it as committed and cleanup retires it.
    /// `false` for a carrier that is not a resident ballot.
    pub(super) fn anonymous_ballot_decided(
        &self,
        carrier: &EntrypointHash,
        view: &StateView<'_>,
    ) -> bool {
        let Some((scope, nullifier)) = self.pending_ballots.lock().identity(carrier) else {
            return false;
        };
        view.world()
            .parliament_anonymous_boards()
            .get(&scope)
            .is_none_or(|board| {
                board.closure.is_some()
                    || board
                        .entries
                        .iter()
                        .any(|entry| entry.nullifier == nullifier)
            })
    }

    /// Whether the resident ballot `envelope` (carrier `carrier`) is offered to a block built
    /// on `view` now, charging `offer`: its window is open in committed State and not closed
    /// by the local clock, its nullifier is free, and the block's ballot budget has room.
    /// A ballot that is not offered stays resident.
    pub(super) fn offer_anonymous_ballot(
        &self,
        carrier: &EntrypointHash,
        envelope: &CanonicalAnonymousBallotEnvelopeV1,
        view: &StateView<'_>,
        offer: &mut BallotOfferV1,
    ) -> bool {
        let Some((scope, nullifier)) = self.pending_ballots.lock().identity(carrier) else {
            return false;
        };
        let world = view.world();
        let boards = world.parliament_anonymous_boards();
        let Some(board) = boards.get(&scope) else {
            return false;
        };
        let open = board.opening.is_some()
            && board.closure.is_none()
            && closing_ms(board).is_none_or(|closing| offer.now_ms < closing)
            && !board
                .entries
                .iter()
                .any(|entry| entry.nullifier == nullifier);
        if !open {
            return false;
        }
        if matches!(offer.budget, OfferBudgetV1::Unread) {
            offer.budget = BallotBlockBudgetV1::of(world.parameters())
                .map_or(OfferBudgetV1::Refused, OfferBudgetV1::Remaining);
        }
        let OfferBudgetV1::Remaining(budget) = &mut offer.budget else {
            return false;
        };
        verifier_input_bytes(|scope| boards.get(scope), envelope)
            .is_ok_and(|bytes| budget.charge(bytes).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, num::NonZeroUsize, sync::Arc, time::Duration};

    use iroha_crypto::{Hash, HashOf};
    use iroha_primitives::time::TimeSource;

    use super::*;
    use crate::governance::parliament_anonymous::ballot_input::tests::{
        AYE_0, AYE_1, AYE_2, CastingFixtureV1, FORGED_1, NAY_0, authorization, envelope,
        pinned_offset, stamped,
    };

    fn carrier(tag: u8) -> EntrypointHash {
        HashOf::from_untyped_unchecked(Hash::prehashed([tag; Hash::LENGTH]))
    }

    /// A queue of 100 residents whose clock reads `now_ms`.
    fn queue_at(now_ms: u64) -> Arc<Queue> {
        let (_, time_source) = TimeSource::new_mock(Duration::from_millis(now_ms));
        Arc::new(Queue::test(
            iroha_config::parameters::actual::Queue {
                transaction_time_to_live: Duration::from_secs(100),
                capacity: 100.try_into().expect("nonzero"),
                ..iroha_config::parameters::actual::Queue::default()
            },
            &time_source,
        ))
    }

    fn ballot(envelope: CanonicalAnonymousBallotEnvelopeV1) -> AcceptedTransaction<'static> {
        AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(
            TransactionEntrypoint::AnonymousBallot(envelope),
        ))
    }

    /// The stable admission code a refused submission carries.
    fn refused_code(failure: &Failure) -> String {
        match &failure.err {
            Error::UnsupportedTransactionAdmission { reason } => {
                reason.split(':').next().expect("a code").to_owned()
            }
            other => panic!("an admission rejection: {other:?}"),
        }
    }

    #[test]
    fn a_ballot_is_admitted_by_its_proof_and_offered_within_the_block_budget() {
        let mut fixture = CastingFixtureV1::open();
        let (scope, closing_ms) = (fixture.scope(), fixture.closing_ms());
        let state = fixture.state();
        let queue = queue_at(closing_ms - 10);
        let push = |envelope| queue.push(ballot(envelope), state.view());

        // A verified ballot becomes resident and holds its identity; it costs no ordinary gas.
        push(envelope(AYE_0)).expect("a valid ballot inside its window");
        assert_eq!(queue.active_len(), 1);
        assert_eq!(queue.pending_ballots.lock().len(), 1);
        let first = ballot(envelope(AYE_0)).hash_as_entrypoint();
        assert_eq!(queue.tx_gas_cost.get(&first).map(|cost| *cost), Some(0));
        assert!(queue.contains_pending_hash(first, &state));

        // The same carrier again, and a conflicting choice under the pending nullifier: both
        // are dropped before any proof is read.
        assert!(matches!(
            push(envelope(AYE_0)).expect_err("resident").err,
            Error::IsInQueue
        ));
        assert!(matches!(
            push(envelope(NAY_0)).expect_err("a pending holder").err,
            Error::IsInQueue
        ));
        // Submissions the ordered checks or the proof reject reserve nothing.
        assert_eq!(
            refused_code(
                &push(stamped(&authorization(AYE_1), pinned_offset(AYE_1) + 1))
                    .expect_err("a weak stamp")
            ),
            "anonymous_ballot_insufficient_stamp"
        );
        assert_eq!(
            refused_code(&push(envelope(FORGED_1)).expect_err("a forged proof")),
            "anonymous_ballot_proof_rejected"
        );
        let mut foreign = authorization(AYE_1);
        foreign.scope = BallotScopeV1::new([0xee; 64]);
        assert_eq!(
            refused_code(
                &push(stamped(&foreign, pinned_offset(AYE_1))).expect_err("an unknown scope")
            ),
            "anonymous_ballot_unknown_scope"
        );
        assert_eq!(queue.active_len(), 1);
        assert_eq!(queue.pending_ballots.lock().len(), 1);
        // The forged ballot of seat 1 did not block the seat's honest ballot.
        push(envelope(AYE_1)).expect("the honest holder of the nullifier");
        push(envelope(AYE_2)).expect("a third ballot");
        assert_eq!(queue.pending_ballots.lock().len(), 3);

        // The proposer is offered at most `B_work` ballots, in admission order.
        let offered = |state: &crate::state::State| {
            queue
                .bounded_pending_inputs(&state.view(), NonZeroUsize::new(64).expect("nonzero"))
                .expect("a healthy queue")
                .iter()
                .map(|tx| tx.as_accepted().hash_as_entrypoint())
                .collect::<Vec<_>>()
        };
        let second = ballot(envelope(AYE_1)).hash_as_entrypoint();
        let third = ballot(envelope(AYE_2)).hash_as_entrypoint();
        assert_eq!(offered(&state), [first, second]);

        // A block accepts the first ballot. Committed State has decided it: it is no longer
        // pending or offered, and the next gossip pass retires it and frees its identity.
        fixture.accept(closing_ms - 5, &envelope(AYE_0));
        let committed = fixture.state();
        assert!(queue.anonymous_ballot_decided(&first, &committed.view()));
        assert!(!queue.anonymous_ballot_decided(&second, &committed.view()));
        assert!(!queue.contains_pending_hash(first, &committed));
        assert_eq!(offered(&committed), [second, third]);
        let _ = queue.gossip_batch(16, &committed.view());
        assert_eq!(queue.active_len(), 2);
        assert_eq!(queue.pending_ballots.lock().len(), 2);
        assert!(
            !queue
                .pending_ballots
                .lock()
                .holds(&scope, &authorization(AYE_0).nullifier)
        );

        // A resubmission of the accepted ballot is refused as already applied.
        assert!(matches!(
            queue
                .push(ballot(envelope(AYE_0)), committed.view())
                .expect_err("a committed nullifier")
                .err,
            Error::InBlockchain
        ));
    }

    #[test]
    fn a_closed_window_refuses_new_ballots_and_offers_none() {
        let fixture = CastingFixtureV1::open();
        let closing_ms = fixture.closing_ms();
        let state = fixture.state();
        // Admitted one millisecond before the closing time.
        let queue = queue_at(closing_ms - 1);
        queue
            .push(ballot(envelope(AYE_0)), state.view())
            .expect("inside the window");
        // By the local clock the window has closed: nothing is admitted or offered, and the
        // resident ballot stays until committed State closes its ballot.
        let late = queue_at(closing_ms);
        assert_eq!(
            refused_code(
                &late
                    .push(ballot(envelope(AYE_1)), state.view())
                    .expect_err("the local clock passed the closing time")
            ),
            "anonymous_ballot_casting_closed"
        );
        assert_eq!(late.active_len(), 0);
        let mut offer = BallotOfferV1::at(closing_ms);
        let first = ballot(envelope(AYE_0)).hash_as_entrypoint();
        assert!(!queue.offer_anonymous_ballot(&first, &envelope(AYE_0), &state.view(), &mut offer));
        assert!(!queue.anonymous_ballot_decided(&first, &state.view()));
        let mut offer = BallotOfferV1::at(closing_ms - 1);
        assert!(queue.offer_anonymous_ballot(&first, &envelope(AYE_0), &state.view(), &mut offer));
    }

    #[test]
    fn an_identity_has_one_verified_pending_holder_until_its_carrier_leaves() {
        let mut index = PendingBallotIndexV1::default();
        let scope = BallotScopeV1::new([1; 64]);
        let (first, second) = (NullifierV1::new([2; 128]), NullifierV1::new([3; 128]));
        assert!(!index.holds(&scope, &first));
        assert!(index.claim((scope, first), carrier(1), 7));
        assert!(index.holds(&scope, &first));
        assert!(index.is_resident(&carrier(1)));
        assert_eq!(index.retained_bytes(), 7);
        assert_eq!(index.identity(&carrier(1)), Some((scope, first)));
        // A rerandomized copy of the pending ballot, and a carrier indexed twice.
        assert!(
            !index.claim((scope, first), carrier(2), 9),
            "a pending identity admits no second holder"
        );
        assert!(!index.claim((scope, second), carrier(1), 9));
        assert_eq!((index.len(), index.retained_bytes()), (1, 7));
        // Another nullifier of the scope is never blocked.
        assert!(index.claim((scope, second), carrier(2), 5));
        assert_eq!((index.len(), index.retained_bytes()), (2, 12));
        // Releasing an unknown carrier changes nothing; a released identity is free again.
        index.release(&carrier(9));
        index.release(&carrier(1));
        assert!(!index.holds(&scope, &first));
        assert!(index.holds(&scope, &second));
        assert_eq!(index.identity(&carrier(1)), None);
        assert_eq!(index.retained_bytes(), 5);
        assert!(index.claim((scope, first), carrier(3), 1));
    }

    #[test]
    fn the_verification_rate_refills_from_the_local_clock_up_to_its_burst() {
        let (rate, burst) = (
            std::num::NonZeroU32::new(2).expect("nonzero"),
            std::num::NonZeroU32::new(3).expect("nonzero"),
        );
        let mut bucket = VerificationRateV1::full(burst, 10_000);
        // The burst, then nothing until the clock refills half a second per verification.
        for _ in 0..3 {
            assert!(bucket.take(rate, burst, 10_000));
        }
        assert!(!bucket.take(rate, burst, 10_000));
        assert!(!bucket.take(rate, burst, 10_499));
        assert!(bucket.take(rate, burst, 10_500));
        assert!(!bucket.take(rate, burst, 10_500));
        // A long idle time fills the bucket to its burst and no further; a clock that
        // steps back refills nothing.
        for _ in 0..3 {
            assert!(bucket.take(rate, burst, 99_000));
        }
        assert!(!bucket.take(rate, burst, 99_000));
        assert!(!bucket.take(rate, burst, 50_000));
    }

    #[test]
    fn the_partition_has_its_own_budget_and_verification_rate() {
        let fixture = CastingFixtureV1::open();
        let closing_ms = fixture.closing_ms();
        let state = fixture.state();
        let (clock, time_source) = TimeSource::new_mock(Duration::from_millis(closing_ms - 5_000));
        let config = |ballots: iroha_config::parameters::actual::QueueAnonymousBallots| {
            iroha_config::parameters::actual::Queue {
                transaction_time_to_live: Duration::from_secs(100),
                // One ordinary resident at most: ballots never count against it.
                capacity: 1.try_into().expect("nonzero"),
                anonymous_ballots: ballots,
                ..iroha_config::parameters::actual::Queue::default()
            }
        };
        let defaults = iroha_config::parameters::actual::QueueAnonymousBallots::default();
        let one = std::num::NonZeroU32::MIN;

        // Two ballots are resident although the ordinary capacity is one, and they leave the
        // ordinary budgets and the ordinary pressure untouched.
        let queue = Arc::new(Queue::test(config(defaults), &time_source));
        for index in [AYE_0, AYE_1] {
            queue
                .push(ballot(envelope(index)), state.view())
                .expect("the partition's own budget");
        }
        assert_eq!(queue.active_len(), 2);
        assert_eq!(queue.ordinary_active_len(), 0);
        assert_eq!(queue.ordinary_retained_bytes(), 0);
        assert!(queue.ballot_residents().1 > 0);
        assert!(!queue.current_backpressure().is_saturated());
        assert_eq!(queue.oldest_queued_tx_age_ms(), 0);

        // The partition's count limit and byte budget are its own back-pressure.
        let counted = Arc::new(Queue::test(
            config(iroha_config::parameters::actual::QueueAnonymousBallots {
                pending_limit: 1.try_into().expect("nonzero"),
                ..defaults
            }),
            &time_source,
        ));
        counted
            .push(ballot(envelope(AYE_0)), state.view())
            .expect("the one resident ballot");
        assert!(matches!(
            counted
                .push(ballot(envelope(AYE_1)), state.view())
                .expect_err("the pending limit")
                .err,
            Error::Full
        ));
        let sized = Arc::new(Queue::test(
            config(iroha_config::parameters::actual::QueueAnonymousBallots {
                max_retained_bytes: 1.try_into().expect("nonzero"),
                ..defaults
            }),
            &time_source,
        ));
        assert!(matches!(
            sized
                .push(ballot(envelope(AYE_0)), state.view())
                .expect_err("the byte budget")
                .err,
            Error::Full
        ));

        // One verification per second with a burst of one: the second proof waits for the
        // clock, and the refused submission reserved nothing.
        let paced = Arc::new(Queue::test(
            config(iroha_config::parameters::actual::QueueAnonymousBallots {
                verification_rate_per_sec: one,
                verification_burst: one,
                ..defaults
            }),
            &time_source,
        ));
        paced
            .push(ballot(envelope(AYE_0)), state.view())
            .expect("the burst");
        assert!(matches!(
            paced
                .push(ballot(envelope(AYE_1)), state.view())
                .expect_err("the verification rate is spent")
                .err,
            Error::Full
        ));
        assert_eq!(paced.ballot_residents().0, 1);
        clock.advance(Duration::from_secs(1));
        paced
            .push(ballot(envelope(AYE_1)), state.view())
            .expect("the refilled rate");
        assert_eq!(paced.ballot_residents().0, 2);
    }
}
