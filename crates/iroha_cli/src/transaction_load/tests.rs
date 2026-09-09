//! Deterministic schedule, ownership, pressure and publication tests for the collector.

use super::*;
use clap::Parser;
use iroha_crypto::Hash;
use iroha_torii_shared::PipelineTransactionStatus;
use std::{
    future::poll_fn,
    sync::{
        Mutex,
        atomic::{AtomicI64, Ordering},
    },
    task::{Poll, Waker},
};

#[derive(Parser)]
struct TestCli {
    #[command(flatten)]
    args: Args,
}
fn arguments(root: &Path) -> Args {
    let root = root.canonicalize().expect("canonical temporary root");
    TestCli::try_parse_from([
        "load",
        "--pair-index",
        "1",
        "--variant",
        "one_lane",
        "--seed",
        &"a".repeat(64),
        "--offered-load-tps",
        "2",
        "--warmup-seconds",
        "0",
        "--measurement-seconds",
        "2",
        "--drain-seconds",
        "1",
        "--max-submission-lag-ms",
        "125",
        "--trace-out",
        root.join("trace.json").to_str().expect("trace path"),
        "--resource-program",
        "/usr/bin/python3",
        "--resource-worker",
        "/tmp/resource_probe_worker.py",
        "--resource-config",
        "/tmp/runtime-only-probe-config.json",
        "--resource-capture-dir",
        "/tmp/new-probe-captures",
        "--resource-interval-ms",
        "1000",
        "--resource-timeout-ms",
        "400",
        "--resource-max-start-lag-ms",
        "100",
        "--diagnostic-out",
        root.join("journal.jsonl").to_str().expect("journal path"),
    ])
    .expect("strict command arguments")
    .args
}
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}
fn exact_hash(seed: u8) -> TransactionHash {
    TransactionHash::from_untyped_unchecked(Hash::prehashed([seed; 32]))
}
fn response(
    hash: TransactionHash,
    status: &str,
    source: &str,
) -> PipelineTransactionStatusResponse {
    PipelineTransactionStatusResponse {
        hash: hash.to_string(),
        scope: "global".to_owned(),
        resolved_from: source.to_owned(),
        status: PipelineTransactionStatus {
            kind: status.to_owned(),
            block_height: Some(1),
        },
    }
}

struct TestClock {
    now: AtomicI64,
    waiters: Mutex<Vec<(i64, Waker)>>,
}
impl TestClock {
    fn new(now: i64) -> Arc<Self> {
        Arc::new(Self {
            now: AtomicI64::new(now),
            waiters: Mutex::new(Vec::new()),
        })
    }
    fn wait(self: Arc<Self>, target: i64) -> BoxFuture<'static, ()> {
        poll_fn(move |context| {
            if self.now() >= target {
                Poll::Ready(())
            } else {
                self.waiters
                    .lock()
                    .expect("waiters")
                    .push((target, context.waker().clone()));
                Poll::Pending
            }
        })
        .boxed()
    }
}
impl Clock for TestClock {
    fn now(&self) -> i64 {
        self.now.load(Ordering::SeqCst)
    }
    fn sleep(self: Arc<Self>, offset: i64) -> BoxFuture<'static, i64> {
        let mut yielded = false;
        poll_fn(move |context| {
            let next = self
                .waiters
                .lock()
                .expect("waiters")
                .iter()
                .map(|(target, _)| *target)
                .filter(|target| *target >= self.now())
                .min()
                .unwrap_or(offset)
                .min(offset);
            // A live future timer parks the collector. Give work woken by earlier
            // select arms that same chance before advancing virtual time, and
            // recompute the next waiter on repoll. An expired timer is ready now.
            if next > self.now() && !yielded {
                yielded = true;
                context.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(next)
            }
        })
        .boxed()
    }
    fn timer_reached(&self, offset: i64) {
        self.now.store(offset, Ordering::SeqCst);
        let mut waiters = self.waiters.lock().expect("waiters");
        let pending = std::mem::take(&mut *waiters);
        for (target, waker) in pending {
            if target <= offset {
                waker.wake();
            } else {
                waiters.push((target, waker));
            }
        }
    }
}

#[derive(Default)]
struct MemoryRecorder {
    events: Mutex<Vec<Value>>,
    capacity: Option<usize>,
}
impl Recorder for MemoryRecorder {
    fn record(&self, event: Value) -> Result<()> {
        let mut events = self.events.lock().expect("events");
        if self
            .capacity
            .is_some_and(|capacity| events.len() >= capacity)
        {
            bail!("bounded test recorder saturated");
        }
        events.push(event);
        Ok(())
    }
}
struct FakeBackend {
    clock: Arc<TestClock>,
    preparation_delay: i64,
    preparation_error: bool,
    observation_error: bool,
    acknowledgment_delay: i64,
    state_delay: i64,
    source: &'static str,
    status: &'static str,
    submission_error: bool,
    duplicate_hash: bool,
    offers: Mutex<BTreeMap<TransactionHash, i64>>,
    preparations: Mutex<Vec<usize>>,
}
impl FakeBackend {
    fn new(clock: Arc<TestClock>) -> Self {
        Self {
            clock,
            preparation_delay: 0,
            preparation_error: false,
            observation_error: false,
            acknowledgment_delay: 2_000_000,
            state_delay: 1_000_000,
            source: "state",
            status: "Applied",
            submission_error: false,
            duplicate_hash: false,
            offers: Mutex::new(BTreeMap::new()),
            preparations: Mutex::new(Vec::new()),
        }
    }
}
impl Backend for FakeBackend {
    type Payload = TransactionHash;
    fn prepare(
        self: Arc<Self>,
        plan: Planned,
    ) -> BoxFuture<'static, Result<Prepared<Self::Payload>>> {
        async move {
            self.preparations
                .lock()
                .expect("preparations")
                .push(plan.sequence);
            self.clock
                .clone()
                .wait(self.clock.now() + self.preparation_delay)
                .await;
            if self.preparation_error {
                bail!("https://user:private-token@peer.invalid/private-token?token=private-token");
            }
            let hash = exact_hash(if self.duplicate_hash {
                1
            } else {
                plan.sequence as u8
            });
            Ok(Prepared {
                payload: hash,
                hash,
                account_index: plan.account_index,
            })
        }
        .boxed()
    }
    fn submit(
        self: Arc<Self>,
        prepared: Prepared<Self::Payload>,
    ) -> BoxFuture<'static, Result<TransactionHash>> {
        async move {
            let offer = self.clock.now();
            assert!(
                self.offers
                    .lock()
                    .expect("offers")
                    .insert(prepared.hash, offer)
                    .is_none(),
                "a submitted identity must never replay"
            );
            self.clock
                .clone()
                .wait(offer + self.acknowledgment_delay)
                .await;
            if self.submission_error {
                bail!("QueuePlan outcome unknown: https://user:private-token@peer.invalid/?token=private-token");
            }
            Ok(prepared.payload)
        }
        .boxed()
    }
    fn observe(
        self: Arc<Self>,
        _account_index: usize,
        hash: TransactionHash,
    ) -> BoxFuture<'static, Result<Option<PipelineTransactionStatusResponse>>> {
        async move {
            let offer = *self
                .offers
                .lock()
                .expect("offers")
                .get(&hash)
                .expect("actual offer before observation");
            self.clock.clone().wait(offer + self.state_delay).await;
            if self.observation_error {
                bail!("remote body contains private-token and an authentication header");
            }
            Ok(Some(response(hash, self.status, self.source)))
        }
        .boxed()
    }
}

#[test]
fn command_and_decimal_schedule_are_exact_without_response_pacing() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut args = arguments(dir.path());
    assert_eq!(args.variant.text(), "one_lane");
    args.offered_load_tps = "3e0".to_owned();
    args.max_submission_lag_ms = "0".to_owned();
    let schedule = Schedule::from_args(&args).expect("rational schedule");
    let rows = schedule.plan(&args.seed, 4).expect("rows");
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[1].plan.scheduled_offset_ns, 333_333_333);
    assert_eq!(rows[2].plan.scheduled_offset_ns, 666_666_666);
    assert_eq!(rows[3].plan.scheduled_offset_ns, NS);
    let paired = schedule.plan(&args.seed, 4).expect("paired rows");
    for (one, four) in rows.iter().zip(&paired) {
        assert_eq!(one.plan.logical_id, four.plan.logical_id);
        assert_eq!(one.plan.account_index, four.plan.account_index);
    }
    assert_eq!(
        decimal_ns("0.000000001", NS as u128).expect("nanosecond"),
        1
    );
    assert!(decimal_ns("0.0000000001", NS as u128).is_err());
    for invalid in ["", "NaN", "-1", "1e100", "1.2.3", " 1", "1e-19"] {
        assert!(decimal(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn schedule_preserves_warmup_boundary_lag_limits_and_hard_bounds() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut args = arguments(dir.path());
    args.warmup_seconds = "2".to_owned();
    let schedule = Schedule::from_args(&args).expect("schedule");
    let rows = schedule.plan(&args.seed, 1).expect("plan");
    assert_eq!(rows[0].plan.scheduled_offset_ns, -3 * NS);
    assert_eq!(rows[4].plan.scheduled_offset_ns, 0);
    assert!(!schedule.within_deadline(Cohort::Warmup, 0));
    assert!(schedule.within_deadline(Cohort::Measurement, 3 * NS));
    assert!(schedule.offer(&rows[4].plan, 125_000_000).is_ok());
    assert!(schedule.offer(&rows[4].plan, 125_000_001).is_err());
    assert!(schedule.offer(&rows[4].plan, -1).is_err());
    args.max_submission_lag_ms = "125.000001".to_owned();
    assert!(Schedule::from_args(&args).is_err());
    args.max_submission_lag_ms = "0".to_owned();
    args.drain_seconds = "301".to_owned();
    assert!(Schedule::from_args(&args).is_err());
    args.drain_seconds = "1".to_owned();
    args.measurement_seconds = "1000000".to_owned();
    assert!(Schedule::from_args(&args).is_err());
    args.max_submissions = 0;
    assert!(Bounds::from_args(&args).is_err());
}

#[test]
fn only_exact_global_state_applied_is_terminal_success() {
    let hash = exact_hash(7);
    assert_eq!(
        classify_observation(hash, &response(hash, "Applied", "state")).expect("state Applied"),
        Some(1)
    );
    for source in ["cache", "queue"] {
        for kind in ["Applied", "Rejected", "Expired"] {
            assert_eq!(
                classify_observation(hash, &response(hash, kind, source)).expect("nonterminal"),
                None
            );
        }
    }
    for kind in ["Queued", "Approved", "Committed"] {
        assert_eq!(
            classify_observation(hash, &response(hash, kind, "state")).expect("nonterminal"),
            None
        );
    }
    for kind in ["Rejected", "Expired", "Unknown"] {
        assert!(classify_observation(hash, &response(hash, kind, "state")).is_err());
    }
    assert!(classify_observation(hash, &response(exact_hash(8), "Applied", "state")).is_err());
    let mut wrong = response(hash, "Applied", "state");
    wrong.scope = "local".to_owned();
    assert!(classify_observation(hash, &wrong).is_err());
    wrong.scope = "global".to_owned();
    wrong.status.block_height = Some(0);
    assert!(classify_observation(hash, &wrong).is_err());
    wrong.status.block_height = None;
    assert!(classify_observation(hash, &wrong).is_err());
}

#[test]
fn collector_observes_applied_before_delayed_ack_without_reordering_or_replay() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let args = arguments(dir.path());
    let schedule = Schedule::from_args(&args).expect("schedule");
    let bounds = Bounds::from_args(&args).expect("bounds");
    let clock = TestClock::new(-NS);
    let backend = Arc::new(FakeBackend::new(clock.clone()));
    let recorder = Arc::new(MemoryRecorder::default());
    let mut rows = schedule.plan(&args.seed, 1).expect("rows");
    let len = rows.len();
    runtime()
        .block_on(collect_phase(
            backend.clone(),
            clock.clone(),
            recorder.clone(),
            &schedule,
            &bounds,
            &mut rows,
            0..len,
            Cohort::Measurement,
            &mut BTreeSet::new(),
        ))
        .expect("complete cohort");
    assert_eq!(
        clock.now(),
        3 * NS,
        "the fixed drain window must finish before publication"
    );
    assert_eq!(backend.offers.lock().expect("offers").len(), len);
    for row in &rows {
        assert!(row.applied.expect("Applied").0 < row.acknowledgment_ns.expect("ack"));
        assert_eq!(row.offer_ns, Some(row.plan.scheduled_offset_ns));
        assert!(row.trace_value().is_ok());
    }
    let events = recorder.events.lock().expect("events");
    assert_eq!(
        events
            .iter()
            .filter(|event| event.get("event").and_then(Value::as_str) == Some("offer"))
            .count(),
        len
    );
}

#[test]
fn preparation_pressure_invalidates_instead_of_moving_the_offer_schedule() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let args = arguments(dir.path());
    let schedule = Schedule::from_args(&args).expect("schedule");
    let bounds = Bounds::from_args(&args).expect("bounds");
    let clock = TestClock::new(-NS);
    let mut backend = FakeBackend::new(clock.clone());
    backend.preparation_delay = 2 * NS;
    let backend = Arc::new(backend);
    let mut rows = schedule.plan(&args.seed, 1).expect("rows");
    let len = rows.len();
    let error = runtime()
        .block_on(collect_phase(
            backend.clone(),
            clock,
            Arc::new(MemoryRecorder::default()),
            &schedule,
            &bounds,
            &mut rows,
            0..len,
            Cohort::Measurement,
            &mut BTreeSet::new(),
        ))
        .expect_err("late signer must fail");
    assert!(error.to_string().contains("not ready"));
    assert!(backend.offers.lock().expect("offers").is_empty());
    assert_eq!(
        rows.len(),
        len,
        "every unoffered scheduled request stays in the ledger"
    );
    assert_eq!(rows[0].plan.scheduled_offset_ns, 0);
}

#[test]
fn submission_ambiguity_never_replays_or_becomes_an_accepted_trace() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let args = arguments(dir.path());
    let schedule = Schedule::from_args(&args).expect("schedule");
    let bounds = Bounds::from_args(&args).expect("bounds");
    let clock = TestClock::new(-NS);
    let mut backend = FakeBackend::new(clock.clone());
    backend.submission_error = true;
    let backend = Arc::new(backend);
    let mut rows = schedule.plan(&args.seed, 1).expect("rows");
    let len = rows.len();
    let error = runtime()
        .block_on(collect_phase(
            backend.clone(),
            clock,
            Arc::new(MemoryRecorder::default()),
            &schedule,
            &bounds,
            &mut rows,
            0..len,
            Cohort::Measurement,
            &mut BTreeSet::new(),
        ))
        .expect_err("ambiguous submission");
    assert!(error.to_string().contains("no replay"));
    assert_eq!(backend.offers.lock().expect("offers").len(), 1);
    assert!(rows[0].hash.is_some());
    assert!(rows[0].offer_ns.is_some());
    assert!(
        rows[0].applied.is_some(),
        "state observation is retained despite admission ambiguity"
    );
    assert!(rows[0].acknowledgment_ns.is_none());
    assert!(rows[0].trace_value().is_err());
}

#[test]
fn cached_outcomes_and_local_capacity_exhaustion_fail_with_all_identities_retained() {
    for (capacity, source, writer_capacity) in [
        (1, "cache", None),
        (4096, "cache", None),
        (4096, "state", Some(0)),
    ] {
        let dir = tempfile::tempdir().expect("temporary directory");
        let mut args = arguments(dir.path());
        args.max_in_flight = capacity;
        let schedule = Schedule::from_args(&args).expect("schedule");
        let bounds = Bounds::from_args(&args).expect("bounds");
        let clock = TestClock::new(-NS);
        let mut backend = FakeBackend::new(clock.clone());
        backend.source = source;
        let backend = Arc::new(backend);
        let mut rows = schedule.plan(&args.seed, 1).expect("rows");
        let len = rows.len();
        let recorder = Arc::new(MemoryRecorder {
            events: Mutex::new(Vec::new()),
            capacity: writer_capacity,
        });
        assert!(
            runtime()
                .block_on(collect_phase(
                    backend.clone(),
                    clock,
                    recorder,
                    &schedule,
                    &bounds,
                    &mut rows,
                    0..len,
                    Cohort::Measurement,
                    &mut BTreeSet::new()
                ))
                .is_err()
        );
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().any(|row| row.trace_value().is_err()));
        if capacity == 1 {
            assert_eq!(backend.offers.lock().expect("offers").len(), 1);
        }
        if writer_capacity == Some(0) {
            assert!(backend.offers.lock().expect("offers").is_empty());
        }
    }
}

#[test]
fn duplicate_prepared_hash_is_rejected_before_it_can_replay() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let args = arguments(dir.path());
    let schedule = Schedule::from_args(&args).expect("schedule");
    let bounds = Bounds::from_args(&args).expect("bounds");
    let clock = TestClock::new(-NS);
    let mut backend = FakeBackend::new(clock.clone());
    backend.duplicate_hash = true;
    let backend = Arc::new(backend);
    let mut rows = schedule.plan(&args.seed, 1).expect("rows");
    let len = rows.len();
    let error = runtime()
        .block_on(collect_phase(
            backend.clone(),
            clock,
            Arc::new(MemoryRecorder::default()),
            &schedule,
            &bounds,
            &mut rows,
            0..len,
            Cohort::Measurement,
            &mut BTreeSet::new(),
        ))
        .expect_err("duplicate hash");
    assert!(error.to_string().contains("duplicated"));
    assert!(backend.offers.lock().expect("offers").len() <= 1);
}

#[test]
fn strict_trace_encoding_and_publication_require_complete_rows_and_absent_paths() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let args = arguments(dir.path());
    let schedule = Schedule::from_args(&args).expect("schedule");
    let mut rows = schedule.plan(&args.seed, 1).expect("plan");
    for (index, row) in rows.iter_mut().enumerate() {
        row.hash = Some(exact_hash(index as u8 + 1));
        row.offer_ns = Some(row.plan.scheduled_offset_ns);
        row.acknowledgment_ns = Some(row.plan.scheduled_offset_ns + 2000);
        row.submission_finished = true;
        row.applied = Some((row.plan.scheduled_offset_ns + 1000, 1));
    }
    publish_trace(&args.trace_out, &args, &rows).expect("publish exact trace");
    let value: Value = json::from_slice(&std::fs::read(&args.trace_out).expect("trace bytes"))
        .expect("strict JSON");
    assert_eq!(
        value.get("schema").and_then(Value::as_str),
        Some(TRACE_SCHEMA)
    );
    let encoded_rows = value
        .get("transactions")
        .and_then(Value::as_array)
        .expect("transaction rows");
    assert_eq!(encoded_rows.len(), rows.len());
    assert_eq!(encoded_rows[0].as_object().expect("row object").len(), 9);
    assert_eq!(
        encoded_rows[0]
            .get("acknowledgment")
            .and_then(Value::as_object)
            .expect("ack object")
            .len(),
        4
    );
    assert_eq!(
        encoded_rows[0]
            .get("applied")
            .and_then(Value::as_object)
            .expect("Applied object")
            .len(),
        6
    );
    let original = std::fs::read(&args.trace_out).expect("original bytes");
    assert!(publish_trace(&args.trace_out, &args, &rows).is_err());
    assert_eq!(
        std::fs::read(&args.trace_out).expect("retained bytes"),
        original
    );
    rows[0].acknowledgment_ns = None;
    assert!(rows[0].trace_value().is_err());
    let mut bytes = Vec::new();
    let mut written = MAX_FILE_BYTES;
    assert!(bounded_write(&mut bytes, &mut written, b"x").is_err());
    assert!(bytes.is_empty());
}

#[test]
fn diagnostic_journal_retains_scheduled_records_and_never_replaces_an_owned_path() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let path = dir
        .path()
        .canonicalize()
        .expect("canonical journal parent")
        .join("journal.jsonl");
    let journal = Journal::start(&path, 1).expect("new journal");
    journal
        .blocking_record(norito::json!({"event": "scheduled", "sequence": 1}))
        .expect("schedule record");
    journal
        .blocking_record(norito::json!({"event": "collection_finished", "passed": false}))
        .expect("failure record");
    journal
        .flush_before_collection()
        .expect("durable precollection barrier");
    assert!(
        !std::fs::read(&path)
            .expect("durable scheduled records")
            .is_empty()
    );
    journal.finish().expect("durable journal");
    let original = std::fs::read(&path).expect("journal bytes");
    assert_eq!(
        String::from_utf8(original.clone())
            .expect("UTF-8")
            .lines()
            .count(),
        2
    );
    assert!(Journal::start(&path, 1).is_err());
    assert_eq!(std::fs::read(&path).expect("retained journal"), original);
}

#[test]
fn full_engine_accepts_ready_ack_and_applied_exactly_at_measurement_deadline() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut args = arguments(dir.path());
    args.offered_load_tps = "1".to_owned();
    args.measurement_seconds = "1".to_owned();
    let schedule = Schedule::from_args(&args).expect("schedule");
    let bounds = Bounds::from_args(&args).expect("bounds");
    let clock = TestClock::new(-NS);
    let mut backend = FakeBackend::new(clock.clone());
    backend.acknowledgment_delay = 2 * NS;
    backend.state_delay = 2 * NS;
    let mut rows = schedule.plan(&args.seed, 1).expect("rows");
    runtime()
        .block_on(collect_phase(
            Arc::new(backend),
            clock.clone(),
            Arc::new(MemoryRecorder::default()),
            &schedule,
            &bounds,
            &mut rows,
            0..1,
            Cohort::Measurement,
            &mut BTreeSet::new(),
        ))
        .expect("inclusive terminal boundary");
    assert_eq!(clock.now(), 2 * NS);
    assert_eq!(rows[0].acknowledgment_ns, Some(2 * NS));
    assert_eq!(rows[0].applied, Some((2 * NS, 1)));
    assert!(rows[0].trace_value().is_ok());
}

#[test]
fn full_engine_rejects_warmup_completions_exactly_at_measurement_origin() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut args = arguments(dir.path());
    args.offered_load_tps = "1".to_owned();
    args.warmup_seconds = "1".to_owned();
    args.measurement_seconds = "1".to_owned();
    let schedule = Schedule::from_args(&args).expect("schedule");
    let bounds = Bounds::from_args(&args).expect("bounds");
    let clock = TestClock::new(-3 * NS);
    let mut backend = FakeBackend::new(clock.clone());
    backend.acknowledgment_delay = 2 * NS;
    backend.state_delay = 2 * NS;
    let mut rows = schedule.plan(&args.seed, 1).expect("rows");
    assert!(
        runtime()
            .block_on(collect_phase(
                Arc::new(backend),
                clock.clone(),
                Arc::new(MemoryRecorder::default()),
                &schedule,
                &bounds,
                &mut rows,
                0..1,
                Cohort::Warmup,
                &mut BTreeSet::new()
            ))
            .is_err()
    );
    assert_eq!(clock.now(), 0);
    assert_eq!(rows[0].offer_ns, Some(-2 * NS));
    assert!(rows[0].trace_value().is_err());
    assert!(
        rows[1].offer_ns.is_none(),
        "failed warmup cannot start measurement"
    );
}

#[test]
fn full_engine_consumes_preparation_ready_at_offer_timer_without_catchup() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut args = arguments(dir.path());
    args.offered_load_tps = "1".to_owned();
    args.measurement_seconds = "1".to_owned();
    args.max_submission_lag_ms = "0".to_owned();
    let schedule = Schedule::from_args(&args).expect("schedule");
    let bounds = Bounds::from_args(&args).expect("bounds");
    let clock = TestClock::new(-NS);
    let mut backend = FakeBackend::new(clock.clone());
    backend.preparation_delay = NS;
    let mut rows = schedule.plan(&args.seed, 1).expect("rows");
    runtime()
        .block_on(collect_phase(
            Arc::new(backend),
            clock,
            Arc::new(MemoryRecorder::default()),
            &schedule,
            &bounds,
            &mut rows,
            0..1,
            Cohort::Measurement,
            &mut BTreeSet::new(),
        ))
        .expect("ready exact-slot signer");
    assert_eq!(rows[0].offer_ns, Some(0));
    assert_eq!(rows[0].plan.scheduled_offset_ns, 0);
}

#[test]
fn root_command_routes_load_through_the_existing_transaction_surface() {
    let parsed = crate::Args::try_parse_from([
        "iroha",
        "--fee-payer",
        "authority",
        "tx",
        "load",
        "--pair-index",
        "1",
        "--variant",
        "four_lane",
        "--seed",
        &"a".repeat(64),
        "--offered-load-tps",
        "20",
        "--warmup-seconds",
        "5",
        "--measurement-seconds",
        "20",
        "--drain-seconds",
        "2",
        "--max-submission-lag-ms",
        "10",
        "--trace-out",
        "/tmp/gscale-trace.json",
        "--resource-program",
        "/usr/bin/python3",
        "--resource-worker",
        "/tmp/resource_probe_worker.py",
        "--resource-config",
        "/tmp/runtime-only-probe-config.json",
        "--resource-capture-dir",
        "/tmp/new-probe-captures",
        "--resource-interval-ms",
        "1000",
        "--resource-timeout-ms",
        "400",
        "--resource-max-start-lag-ms",
        "100",
        "--diagnostic-out",
        "/tmp/gscale-journal.jsonl",
    ])
    .expect("canonical CLI transaction load command");
    let crate::Command::Tx(crate::transaction::Command::Load(args)) = parsed.command else {
        panic!("load must use the existing transaction command owner");
    };
    assert_eq!(args.variant.text(), "four_lane");
    assert_eq!(args.pair_index, 1);
}

#[test]
fn full_engine_releases_completed_capacity_at_the_next_exact_offer_boundary() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut args = arguments(dir.path());
    args.max_in_flight = 1;
    args.max_submissions = 1;
    args.max_submission_lag_ms = "0".to_owned();
    let schedule = Schedule::from_args(&args).expect("schedule");
    let bounds = Bounds::from_args(&args).expect("bounds");
    // Either terminal owner, or both, can finish on the next original offer
    // boundary. Ready completions must release capacity before its admission.
    for (acknowledgment_delay, state_delay) in
        [(NS / 2, 1_000_000), (1_000_000, NS / 2), (NS / 2, NS / 2)]
    {
        let clock = TestClock::new(-NS);
        let mut backend = FakeBackend::new(clock.clone());
        backend.acknowledgment_delay = acknowledgment_delay;
        backend.state_delay = state_delay;
        let backend = Arc::new(backend);
        let mut rows = schedule.plan(&args.seed, 1).expect("rows");
        let len = rows.len();
        runtime()
            .block_on(collect_phase(
                backend.clone(),
                clock,
                Arc::new(MemoryRecorder::default()),
                &schedule,
                &bounds,
                &mut rows,
                0..len,
                Cohort::Measurement,
                &mut BTreeSet::new(),
            ))
            .expect("capacity completes at each exact next offer");
        assert_eq!(backend.offers.lock().expect("offers").len(), 4);
        for row in &rows {
            assert_eq!(row.offer_ns, Some(row.plan.scheduled_offset_ns));
            assert_eq!(
                row.acknowledgment_ns,
                Some(row.plan.scheduled_offset_ns + acknowledgment_delay)
            );
            assert_eq!(
                row.applied,
                Some((row.plan.scheduled_offset_ns + state_delay, 1))
            );
            assert!(row.trace_value().is_ok());
        }
    }
}

#[test]
fn engine_retains_failure_stages_without_remote_credentials_or_unbounded_status_text() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let args = arguments(dir.path());
    let schedule = Schedule::from_args(&args).expect("schedule");
    let bounds = Bounds::from_args(&args).expect("bounds");
    for stage in 0..3 {
        let clock = TestClock::new(-NS);
        let mut backend = FakeBackend::new(clock.clone());
        backend.preparation_error = stage == 0;
        backend.submission_error = stage == 1;
        backend.observation_error = stage == 2;
        let mut rows = schedule.plan(&args.seed, 1).expect("rows");
        let len = rows.len();
        let recorder = Arc::new(MemoryRecorder::default());
        let failure = runtime()
            .block_on(collect_phase(
                Arc::new(backend),
                clock,
                recorder.clone(),
                &schedule,
                &bounds,
                &mut rows,
                0..len,
                Cohort::Measurement,
                &mut BTreeSet::new(),
            ))
            .expect_err("external stage failure invalidates collection");
        assert!(!failure.to_string().contains("private-token"));
        assert!(rows.iter().any(|row| row.failure.is_some()));
        for row in &rows {
            let encoded = json::to_vec(&row.diagnostic_value()).expect("diagnostic JSON");
            assert!(
                !String::from_utf8(encoded)
                    .expect("UTF-8")
                    .contains("private-token")
            );
        }
        for event in recorder.events.lock().expect("events").iter() {
            let encoded = json::to_vec(event).expect("event JSON");
            assert!(
                !String::from_utf8(encoded)
                    .expect("UTF-8")
                    .contains("private-token")
            );
        }
    }
    let hash = exact_hash(42);
    let mut hostile = response(hash, "private-token", "private-token");
    hostile.hash = "private-token".to_owned();
    hostile.scope = "private-token".to_owned();
    let diagnostic = status_diagnostic(0, NS, hash, &hostile);
    let encoded = String::from_utf8(json::to_vec(&diagnostic).expect("JSON")).expect("UTF-8");
    assert!(!encoded.contains("private-token"));
    assert!(encoded.contains("unknown"));
    assert!(classify_observation(hash, &hostile).is_err());
}

#[test]
fn virtual_clock_yields_before_advancing_and_rechecks_new_waiters() {
    let clock = TestClock::new(0);
    let waker = futures::task::noop_waker();
    let mut context = std::task::Context::from_waker(&waker);
    let mut timer = clock.clone().sleep(100);
    assert_eq!(timer.as_mut().poll(&mut context), Poll::Pending);
    assert_eq!(clock.now(), 0);
    // Work woken by another select arm can install an earlier network response
    // before the collector's timer gets its next poll.
    let mut response = clock.clone().wait(50);
    assert_eq!(response.as_mut().poll(&mut context), Poll::Pending);
    assert_eq!(timer.as_mut().poll(&mut context), Poll::Ready(50));
    assert_eq!(clock.now(), 0);
    clock.timer_reached(50);
    assert_eq!(response.as_mut().poll(&mut context), Poll::Ready(()));
    let mut due = clock.clone().sleep(50);
    assert_eq!(due.as_mut().poll(&mut context), Poll::Ready(50));
    assert_eq!(clock.now(), 50);
}
