// Anonymous-ballot residents against the ordinary queue's pressure
// (`specs/parliament_private_ballot_design.md` §3.6). The resident ledger counts a ballot only
// in its partition, so no admission, claim, release, removal or last-reader refund of a ballot
// saturates or relieves ordinary admission, which still saturates on ordinary residents.

use crate::governance::parliament_anonymous::ballot_input::tests::{
    AYE_0, AYE_1, AYE_2, CastingFixtureV1, envelope,
};
use iroha_data_model::governance::anonymous_ballot::{
    BallotScopeV1, CanonicalAnonymousBallotEnvelopeV1, NullifierV1,
};

/// The signerless carrier of `envelope`.
fn ballot(envelope: CanonicalAnonymousBallotEnvelopeV1) -> AcceptedTransaction<'static> {
    AcceptedTransaction::new_unchecked_entrypoint(std::borrow::Cow::Owned(
        TransactionEntrypoint::AnonymousBallot(envelope),
    ))
}

/// A queue of `capacity` ordinary residents and, when given, `max_retained_bytes` ordinary
/// retained bytes; the ballot partition keeps its default budget.
fn ballot_queue(
    time_source: &TimeSource,
    capacity: usize,
    max_retained_bytes: Option<NonZeroU64>,
) -> Queue {
    let defaults = Config::default();
    Queue::test(
        Config {
            transaction_time_to_live: Duration::from_secs(100),
            capacity: NonZeroUsize::new(capacity).expect("nonzero"),
            max_retained_bytes: max_retained_bytes.unwrap_or(defaults.max_retained_bytes),
            ..defaults
        },
        time_source,
    )
}

/// Make the ordinary input `tx` resident through the residence step both admission paths end
/// with, its ordinary capacity and byte checks included. The signed checks before it need a
/// World the ballot fixture does not commit.
fn reside(
    queue: &Queue,
    state: &State,
    tx: AcceptedTransaction<'static>,
) -> Result<EntrypointHash, Error> {
    let view = state.view();
    let encoded_len = Queue::compute_tx_encoded_len(&tx);
    let checked = tx.into_checked(&view).map_err(|_| Error::InBlockchain)?;
    let hash = checked.hash_as_entrypoint();
    let enqueued_at_ms = queue.validation_timestamp_ms(checked.as_accepted());
    let route = RoutingDecision::default();
    let prepared = PreparedQueueAdmission {
        hash,
        sccp_exempt: None,
        routing_decision: route,
        routing_plan: RoutingPlan::single(route),
        encoded_len,
        proposal_gas_cost: 0,
        enqueued_at_ms,
        expires_at: queue
            .expiry_deadline_with_enqueue_timestamp(checked.as_accepted(), Some(enqueued_at_ms)),
        frame_digest: Queue::entrypoint_frame_digest(checked.as_accepted()),
        fee_reservation: None,
        #[cfg(feature = "telemetry")]
        pending_teu: 0,
        checked,
    };
    queue
        .enqueue_prepared_admissions(vec![prepared], None, &view.execution_budget())
        .map(|_| hash)
        .map_err(|(_, failure)| failure.err)
}

/// `(residents, FIFO entries, retained bytes)` of `partition` in the resident ledger.
fn counts(queue: &Queue, partition: ResidentPartition) -> (usize, usize, u64) {
    queue.resident_accounting.get().map_or((0, 0, 0), |ledger| {
        let counters: &ResidentCounters = ledger.counters(partition);
        (
            counters.active_count.load(Ordering::Relaxed),
            counters.queued_count.load(Ordering::Relaxed),
            counters.retained_bytes.load(Ordering::Relaxed),
        )
    })
}

#[test]
fn ballots_never_move_ordinary_pressure_and_ordinary_residents_still_saturate_it() {
    let fixture = CastingFixtureV1::open();
    let state = fixture.state();
    let (_clock, time_source) =
        TimeSource::new_mock(Duration::from_millis(fixture.closing_ms() - 5_000));
    // One ordinary resident at most.
    let queue = ballot_queue(&time_source, 1, None);
    let mut watch = queue.backpressure_handle().subscribe();
    for index in [AYE_0, AYE_1] {
        queue
            .push(ballot(envelope(index)), state.view())
            .expect("the partition's own budget");
    }

    // Two residents, neither ordinary: every ordinary signal stays empty and healthy.
    assert_eq!((queue.active_len(), queue.queued_len()), (2, 2));
    assert_eq!(counts(&queue, ResidentPartition::Ordinary), (0, 0, 0));
    let (ballots, ballots_queued, ballot_bytes) = counts(&queue, ResidentPartition::Ballot);
    assert_eq!((ballots, ballots_queued), (2, 2));
    assert_eq!(ballot_bytes, queue.ballot_residents().1);
    assert_eq!(queue.retained_bytes(), ballot_bytes);
    assert!(!watch.borrow_and_update().is_saturated());
    assert_eq!(
        queue.current_backpressure(),
        BackpressureState::Healthy {
            queued: 0,
            capacity: nonzero!(1_usize),
        }
    );
    let snapshot = queue.pressure_snapshot();
    assert_eq!(
        (
            snapshot.tracked_tx_count,
            snapshot.queued_tx_count,
            snapshot.retained_bytes,
        ),
        (0, 0, 0)
    );
    assert!(!snapshot.is_saturated());
    queue.assert_pressure_counters_consistent_for_tests();

    // One ordinary resident fills the ordinary capacity: the queue saturates and refuses a
    // second one, and the partition still admits ballots.
    let ordinary = reside(&queue, &state, accepted_tx_by_someone(&time_source))
        .expect("the ordinary capacity");
    assert!(watch.borrow_and_update().is_saturated());
    assert_eq!(
        queue.current_backpressure(),
        BackpressureState::Saturated {
            queued: 1,
            capacity: nonzero!(1_usize),
        }
    );
    let snapshot = queue.pressure_snapshot();
    assert_eq!(
        (snapshot.tracked_tx_count, snapshot.queued_tx_count),
        (1, 1)
    );
    assert!(snapshot.saturated_by_count && !snapshot.saturated_by_bytes);
    assert!(matches!(
        reside(&queue, &state, accepted_tx_by_someone(&time_source)),
        Err(Error::Full)
    ));
    queue
        .push(ballot(envelope(AYE_2)), state.view())
        .expect("ordinary saturation leaves the partition's budget alone");
    assert_eq!(counts(&queue, ResidentPartition::Ballot).0, 3);
    assert_eq!(counts(&queue, ResidentPartition::Ordinary).0, 1);
    assert!(queue.current_backpressure().is_saturated());
    queue.assert_pressure_counters_consistent_for_tests();

    // The ordinary resident leaves: the queue is healthy with three ballots resident.
    assert_eq!(queue.remove_committed_hashes([ordinary], None), 1);
    assert!(!watch.borrow_and_update().is_saturated());
    assert!(!queue.current_backpressure().is_saturated());
    assert_eq!(counts(&queue, ResidentPartition::Ordinary), (0, 0, 0));
    assert_eq!(queue.active_len(), 3);
    queue.assert_pressure_counters_consistent_for_tests();
}

#[test]
fn ballots_keep_their_gossip_backlog_slots_whatever_the_ordinary_capacity() {
    let fixture = CastingFixtureV1::open();
    let state = fixture.state();
    let (_clock, time_source) =
        TimeSource::new_mock(Duration::from_millis(fixture.closing_ms() - 5_000));
    // One ordinary resident at most, three resident ballots: each is gossiped.
    let queue = ballot_queue(&time_source, 1, None);
    let mut carriers = Vec::new();
    for index in [AYE_0, AYE_1, AYE_2] {
        queue
            .push(ballot(envelope(index)), state.view())
            .expect("the partition's own budget");
        carriers.push(ballot(envelope(index)).hash_as_entrypoint());
    }
    let mut gossiped = queue
        .gossip_batch(16, &state.view())
        .iter()
        .map(|entry| entry.hash)
        .collect::<Vec<_>>();
    gossiped.sort();
    carriers.sort();
    assert_eq!(gossiped, carriers);
    assert_eq!(queue.active_len(), 3);
}

#[test]
fn a_released_ballot_whose_reader_lives_keeps_its_bytes_out_of_the_ordinary_budget() {
    let fixture = CastingFixtureV1::open();
    let state = fixture.state();
    let (_clock, time_source) =
        TimeSource::new_mock(Duration::from_millis(fixture.closing_ms() - 5_000));
    let ordinary = accepted_tx_by_someone(&time_source);
    let ordinary_cost = Queue::retained_byte_cost(Queue::compute_tx_encoded_len(&ordinary));
    // Room for exactly one ordinary resident's bytes.
    let queue = ballot_queue(&time_source, 16, NonZeroU64::new(ordinary_cost));
    queue
        .push(ballot(envelope(AYE_0)), state.view())
        .expect("the partition's own byte budget");
    let carrier = ballot(envelope(AYE_0)).hash_as_entrypoint();
    let ballot_cost = counts(&queue, ResidentPartition::Ballot).2;
    assert!(
        ballot_cost > ordinary_cost,
        "the ballot alone would exceed the ordinary byte budget"
    );
    assert!(!queue.current_backpressure().is_saturated());

    // A reader keeps the removed ballot's shell. The partition index released it at removal,
    // the ledger charges its bytes to the partition until the last reader drops, and the
    // ordinary budget never sees them.
    let reader = queue
        .txs
        .get(&carrier)
        .expect("a resident ballot")
        .value()
        .clone();
    assert_eq!(queue.remove_committed_hashes([carrier], None), 1);
    assert_eq!(queue.ballot_residents(), (0, 0));
    assert_eq!(
        counts(&queue, ResidentPartition::Ballot),
        (0, 0, ballot_cost)
    );
    assert_eq!(counts(&queue, ResidentPartition::Ordinary), (0, 0, 0));
    assert!(!queue.current_backpressure().is_saturated());
    let ordinary =
        reside(&queue, &state, ordinary).expect("the released ballot holds no ordinary bytes");

    // The ordinary resident's own bytes saturate the ordinary byte budget, and the ballot's
    // last refund leaves that pressure as it was.
    let snapshot = queue.pressure_snapshot();
    assert_eq!(snapshot.retained_bytes, ordinary_cost);
    assert!(snapshot.saturated_by_bytes && !snapshot.saturated_by_count);
    assert!(queue.current_backpressure().is_saturated());
    drop(reader);
    assert_eq!(counts(&queue, ResidentPartition::Ballot), (0, 0, 0));
    assert!(queue.current_backpressure().is_saturated());
    assert_eq!(queue.remove_committed_hashes([ordinary], None), 1);
    assert!(!queue.current_backpressure().is_saturated());
    assert_eq!(queue.retained_bytes(), 0);
    queue.assert_pressure_counters_consistent_for_tests();
}

#[test]
fn a_claimed_ballot_not_yet_resident_frees_no_ordinary_capacity() {
    let fixture = CastingFixtureV1::open();
    let state = fixture.state();
    let (_clock, time_source) =
        TimeSource::new_mock(Duration::from_millis(fixture.closing_ms() - 5_000));
    let queue = ballot_queue(&time_source, 1, None);
    reside(&queue, &state, accepted_tx_by_someone(&time_source)).expect("the ordinary capacity");
    let ordinary_bytes = queue.ordinary_retained_bytes();
    assert!(queue.current_backpressure().is_saturated());

    // A ballot between its claim (admission step 5) and its residence (step 6): the partition
    // index holds it, the resident ledger does not count it yet.
    let carrier = ballot(envelope(AYE_0)).hash_as_entrypoint();
    let identity = (BallotScopeV1::new([1; 64]), NullifierV1::new([2; 128]));
    assert!(
        queue
            .pending_ballots
            .lock()
            .claim(identity, carrier, 1 << 20)
    );
    assert_eq!(queue.ordinary_active_len(), 1);
    assert_eq!(queue.ordinary_retained_bytes(), ordinary_bytes);
    assert!(queue.current_backpressure().is_saturated());
    assert!(queue.pressure_snapshot().saturated_by_count);
    assert!(
        matches!(
            reside(&queue, &state, accepted_tx_by_someone(&time_source)),
            Err(Error::Full)
        ),
        "a pending claim makes no ordinary room"
    );

    // Its admission refused after the claim releases it, and ordinary pressure is unchanged.
    queue.pending_ballots.lock().release(&carrier);
    assert!(queue.current_backpressure().is_saturated());
    assert_eq!(queue.ordinary_retained_bytes(), ordinary_bytes);
    queue.assert_pressure_counters_consistent_for_tests();
}

#[test]
fn a_ballot_leaves_through_every_exit_with_its_partition_counts_consistent() {
    let mut fixture = CastingFixtureV1::open();
    let closing_ms = fixture.closing_ms();
    let state = fixture.state();
    let (clock, time_source) = TimeSource::new_mock(Duration::from_millis(closing_ms - 5_000));
    let queue = ballot_queue(&time_source, 1, None);
    let push = |index| {
        queue
            .push(ballot(envelope(index)), state.view())
            .expect("an admissible ballot");
    };
    let carrier = |index| ballot(envelope(index)).hash_as_entrypoint();
    // After every exit the ledger, the FIFO and the partition index agree on the ballots, and
    // ordinary pressure is exactly that of the `ordinary` ordinary residents.
    let settled = |ordinary: usize| {
        assert_eq!(counts(&queue, ResidentPartition::Ordinary).0, ordinary);
        assert_eq!(queue.current_backpressure().is_saturated(), ordinary == 1);
        let (ballots, queued, bytes) = counts(&queue, ResidentPartition::Ballot);
        let (indexed, indexed_bytes) = queue.ballot_residents();
        assert_eq!((ballots, queued, bytes), (indexed, indexed, indexed_bytes));
        queue.assert_pressure_counters_consistent_for_tests();
    };
    reside(&queue, &state, accepted_tx_by_someone(&time_source)).expect("the ordinary capacity");
    settled(1);

    // Removal after a block committed the carrier.
    push(AYE_0);
    push(AYE_1);
    settled(1);
    assert_eq!(queue.remove_committed_hashes([carrier(AYE_0)], None), 1);
    settled(1);

    // Committed State decides a ballot, and the next gossip pass retires it.
    fixture.accept(closing_ms - 5, &envelope(AYE_1));
    let committed = fixture.state();
    assert!(queue.anonymous_ballot_decided(&carrier(AYE_1), &committed.view()));
    let _ = queue.gossip_batch(16, &committed.view());
    assert!(!queue.txs.contains_key(&carrier(AYE_1)));
    settled(1);

    // A node that stops accepting work clears every resident.
    push(AYE_2);
    settled(1);
    queue.clear_all();
    assert_eq!(queue.active_len(), 0);
    settled(0);

    // The queue's time to live expires a ballot and an ordinary resident alike.
    reside(&queue, &state, accepted_tx_by_someone(&time_source)).expect("the ordinary capacity");
    push(AYE_2);
    settled(1);
    clock.advance(Duration::from_secs(101));
    assert_eq!(queue.cull_expired_entries_if_due(), 2);
    assert_eq!(queue.active_len(), 0);
    settled(0);
}
