//! Deterministic schedule, ownership, pressure and publication tests for the collector.

use super::*;
use clap::Parser;
use iroha_crypto::Hash;
use iroha_model_base::chain::ChainId;
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
        "--invocation-id",
        &"b".repeat(64),
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
        "--resource-budget-sha256",
        &"a".repeat(64),
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
        "--local-observer-config",
        "/tmp/peer3-client.toml",
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
    local_delay: i64,
    local_response: Option<PipelineTransactionStatusResponse>,
    local_missing: bool,
    local_cached_until: Option<i64>,
    local_error: bool,
    observation_start_advances_to: Option<i64>,
    observation_dispatches: Mutex<Vec<(ObservationScope, i64)>>,
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
            local_delay: 1_000_000,
            local_response: None,
            local_missing: false,
            local_cached_until: None,
            local_error: false,
            observation_start_advances_to: None,
            observation_dispatches: Mutex::new(Vec::new()),
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
        scope: ObservationScope,
    ) -> BoxFuture<'static, Result<Option<PipelineTransactionStatusResponse>>> {
        // Exercise a backend that starts work while constructing its future.
        self.observation_dispatches
            .lock()
            .expect("dispatches")
            .push((scope, self.clock.now()));
        if let Some(deadline) = self.observation_start_advances_to {
            self.clock.timer_reached(deadline);
        }
        async move {
            let offer = *self
                .offers
                .lock()
                .expect("offers")
                .get(&hash)
                .expect("actual offer before observation");
            if scope == ObservationScope::Local {
                self.clock.clone().wait(offer + self.local_delay).await;
                if self.local_error {
                    bail!("local private-token transport failed");
                }
                if self.local_missing {
                    return Ok(None);
                }
                let mut local = self
                    .local_response
                    .clone()
                    .unwrap_or_else(|| response(hash, "Applied", "state"));
                if self.local_response.is_none() {
                    local.scope = "local".to_owned();
                }
                if self
                    .local_cached_until
                    .is_some_and(|delay| self.clock.now() < offer + delay)
                {
                    local.resolved_from = "cache".to_owned();
                }
                return Ok(Some(local));
            }
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
        classify_observation(
            hash,
            &response(hash, "Applied", "state"),
            ObservationScope::Global
        )
        .expect("state Applied"),
        Some(1)
    );
    for source in ["cache", "queue"] {
        for kind in ["Applied", "Rejected", "Expired"] {
            assert_eq!(
                classify_observation(
                    hash,
                    &response(hash, kind, source),
                    ObservationScope::Global
                )
                .expect("nonterminal"),
                None
            );
        }
    }
    for kind in ["Queued", "Approved", "Committed"] {
        assert_eq!(
            classify_observation(
                hash,
                &response(hash, kind, "state"),
                ObservationScope::Global
            )
            .expect("nonterminal"),
            None
        );
    }
    for kind in ["Rejected", "Expired", "Unknown"] {
        assert!(
            classify_observation(
                hash,
                &response(hash, kind, "state"),
                ObservationScope::Global
            )
            .is_err()
        );
    }
    assert!(
        classify_observation(
            hash,
            &response(exact_hash(8), "Applied", "state"),
            ObservationScope::Global
        )
        .is_err()
    );
    let mut wrong = response(hash, "Applied", "state");
    wrong.scope = "local".to_owned();
    assert!(classify_observation(hash, &wrong, ObservationScope::Global).is_err());
    wrong.scope = "global".to_owned();
    wrong.status.block_height = Some(0);
    assert!(classify_observation(hash, &wrong, ObservationScope::Global).is_err());
    wrong.status.block_height = None;
    assert!(classify_observation(hash, &wrong, ObservationScope::Global).is_err());
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
        row.local_applied = Some((row.plan.scheduled_offset_ns + 1500, 1));
    }
    publish_trace(
        &args.trace_out,
        &args,
        &rows,
        allocation::tests::writers(MAX_FILE_BYTES, MAX_FILE_BYTES).trace,
    )
    .expect("publish exact trace");
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
    assert!(
        publish_trace(
            &args.trace_out,
            &args,
            &rows,
            allocation::tests::writers(MAX_FILE_BYTES, MAX_FILE_BYTES).trace
        )
        .is_err()
    );
    assert_eq!(
        std::fs::read(&args.trace_out).expect("retained bytes"),
        original
    );
    rows[0].acknowledgment_ns = None;
    assert!(rows[0].trace_value().is_err());
    let mut bytes = Vec::new();
    let mut written = MAX_FILE_BYTES;
    assert!(bounded_write(&mut bytes, &mut written, b"x", MAX_FILE_BYTES).is_err());
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
    let journal = Journal::start(
        &path,
        1,
        allocation::tests::writers(MAX_FILE_BYTES, MAX_FILE_BYTES).journal,
    )
    .expect("new journal");
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
    assert!(
        Journal::start(
            &path,
            1,
            allocation::tests::writers(MAX_FILE_BYTES, MAX_FILE_BYTES).journal
        )
        .is_err()
    );
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
        "--invocation-id",
        &"b".repeat(64),
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
        "--resource-budget-sha256",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
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
        "--local-observer-config",
        "/tmp/peer3-client.toml",
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
    let diagnostic = status_diagnostic(0, NS, hash, &hostile, ObservationScope::Global);
    let encoded = String::from_utf8(json::to_vec(&diagnostic).expect("JSON")).expect("UTF-8");
    assert!(!encoded.contains("private-token"));
    assert!(encoded.contains("unknown"));
    assert!(classify_observation(hash, &hostile, ObservationScope::Global).is_err());
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

#[test]
fn trace_allocation_includes_header_rows_and_closing_bytes_without_partial_publication() {
    let dir = tempfile::tempdir().unwrap();
    let args = arguments(dir.path());
    let schedule = Schedule::from_args(&args).unwrap();
    let mut rows = schedule.plan(&args.seed, 1).unwrap();
    for (index, row) in rows.iter_mut().enumerate() {
        row.hash = Some(exact_hash(index as u8 + 1));
        row.offer_ns = Some(row.plan.scheduled_offset_ns);
        row.acknowledgment_ns = Some(row.plan.scheduled_offset_ns + 2000);
        row.submission_finished = true;
        row.applied = Some((row.plan.scheduled_offset_ns + 1000, 1));
        row.local_applied = Some((row.plan.scheduled_offset_ns + 1500, 1));
    }
    let baseline_path = dir.path().canonicalize().unwrap().join("baseline.json");
    publish_trace(
        &baseline_path,
        &args,
        &rows,
        allocation::tests::writers(1, MAX_FILE_BYTES).trace,
    )
    .unwrap();
    let baseline = std::fs::read(&baseline_path).unwrap();
    assert!(baseline.ends_with(b"]}\n"));
    let parsed: Value = json::from_slice(&baseline).unwrap();
    let decoded_rows = parsed.get("transactions").unwrap().as_array().unwrap();
    assert_eq!(decoded_rows.len(), rows.len());
    for (decoded, row) in decoded_rows.iter().zip(&rows) {
        assert_eq!(*decoded, row.trace_value().unwrap());
    }
    for cap in [baseline.len(), baseline.len() - 1, 1] {
        let path = dir
            .path()
            .canonicalize()
            .unwrap()
            .join(format!("trace-{cap}.json"));
        let result = publish_trace(
            &path,
            &args,
            &rows,
            allocation::tests::writers(1, cap).trace,
        );
        let stage = path.with_file_name(format!("trace-{cap}.json.collecting"));
        if cap == baseline.len() {
            assert!(result.is_ok());
            assert_eq!(std::fs::read(&path).unwrap(), baseline);
            assert!(!stage.exists());
        } else {
            assert!(result.is_err());
            assert!(!path.exists());
            assert!(stage.is_file());
            assert!(std::fs::metadata(&stage).unwrap().len() <= cap as u64);
            let before = std::fs::read(&stage).unwrap();
            assert!(
                publish_trace(
                    &path,
                    &args,
                    &rows,
                    allocation::tests::writers(1, MAX_FILE_BYTES).trace
                )
                .is_err()
            );
            assert_eq!(std::fs::read(&stage).unwrap(), before);
        }
    }
}

#[test]
fn trusted_budget_digest_is_required_and_validated_before_any_output_owner() {
    let command = <TestCli as clap::CommandFactory>::command();
    let hash_arg = command
        .get_arguments()
        .find(|arg| arg.get_id() == "resource_budget_sha256")
        .unwrap();
    assert!(hash_arg.is_required_set());
    let dir = tempfile::tempdir().unwrap();
    let mut args = arguments(dir.path());
    let schedule = Schedule::from_args(&args).unwrap();
    assert!(allocation::Expected::new(&args, &schedule, NS).is_ok());
    for invalid in [
        String::new(),
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "g".repeat(64),
    ] {
        args.resource.resource_budget_sha256 = invalid;
        assert!(allocation::Expected::new(&args, &schedule, NS).is_err());
        assert!(!args.trace_out.exists());
        assert!(!args.diagnostic_out.exists());
    }
}

#[test]
fn invalid_client_context_fails_before_collection_or_output_admission() {
    use iroha_i18n::{Bundle, Language, Localizer};

    struct Context {
        config: Config,
        i18n: Localizer,
        printed: usize,
    }
    impl RunContext for Context {
        fn config(&self) -> &Config {
            &self.config
        }
        fn transaction_metadata(&self) -> Option<&Metadata> {
            None
        }
        fn input_instructions(&self) -> bool {
            false
        }
        fn output_instructions(&self) -> bool {
            false
        }
        fn i18n(&self) -> &Localizer {
            &self.i18n
        }
        fn print_data<V: norito::json::JsonSerialize + ?Sized>(&mut self, _data: &V) -> Result<()> {
            self.printed += 1;
            Ok(())
        }
        fn println(&mut self, _data: impl std::fmt::Display) -> Result<()> {
            self.printed += 1;
            Ok(())
        }
    }

    for invalid_endpoint in [true, false] {
        let root = tempfile::tempdir().expect("collector fixture directory");
        let mut args = arguments(root.path());
        // The real run must pass schedule/resource geometry before SDK admission.
        args.measurement_seconds = "20".to_owned();
        let mut config = crate::fallback_config();
        if invalid_endpoint {
            config.torii_api_url = "ftp://127.0.0.1/".parse().expect("fixture URL");
        } else {
            config.account_chain_discriminant = 0;
        }
        let mut context = Context {
            config,
            i18n: Localizer::new(Bundle::Cli, Language::English),
            printed: 0,
        };
        let error = args
            .run(&mut context)
            .expect_err("invalid first-release client context must fail before collection");
        assert!(
            matches!(
                error.downcast_ref::<iroha::Error>(),
                Some(iroha::Error::Context(_))
            ),
            "preserve the real builder context error, not a later worker failure: {error:#}"
        );
        assert_eq!(context.printed, 0);
        assert!(
            std::fs::read_dir(root.path())
                .expect("collector fixture census")
                .next()
                .is_none(),
            "invalid context must not create a journal, trace, or other output"
        );
    }
}

fn local_response(hash: TransactionHash) -> PipelineTransactionStatusResponse {
    let mut local = response(hash, "Applied", "state");
    local.scope = "local".to_owned();
    local
}

#[test]
fn local_observer_is_required_and_cannot_change_the_selected_chain_or_network() {
    use clap::CommandFactory;
    let command = TestCli::command();
    let local = command
        .get_arguments()
        .find(|arg| arg.get_id() == "local_observer_config")
        .expect("explicit observer argument");
    assert!(local.is_required_set());
    assert!(local.get_default_values().is_empty());
    let source = crate::fallback_config();
    let mut observer = source.clone();
    observer.torii_api_url = "http://127.0.0.1:18083/".parse().unwrap();
    assert!(local_observer_client(observer.clone(), &source).is_ok());
    observer.chain = ChainId::from("other-scaling-chain");
    assert!(local_observer_client(observer, &source).is_err());
    let mut observer = source.clone();
    observer.network_id =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x13; 32])));
    assert!(local_observer_client(observer, &source).is_err());
}

#[test]
fn local_observer_preserves_original_client_builder_context_errors() {
    let source = crate::fallback_config();
    for invalid_endpoint in [true, false] {
        let mut observer = source.clone();
        if invalid_endpoint {
            observer.torii_api_url = "ftp://127.0.0.1/".parse().unwrap();
        } else {
            observer.account_chain_discriminant = 0;
        }
        let error = local_observer_client(observer, &source)
            .expect_err("invalid local observer must retain its SDK builder error");
        assert!(
            matches!(
                error.downcast_ref::<iroha::Error>(),
                Some(iroha::Error::Context(_))
            ),
            "original builder context error must remain available: {error:#}"
        );
    }
}

#[test]
fn local_barrier_is_independent_of_global_observation_and_delayed_ack_order() {
    for (global_delay, local_delay, ack_delay) in [
        (1_000_000, 200_000_000, 2_000_000),
        (200_000_000, 1_000_000, 2_000_000),
        (1_000_000, 2_000_000, 200_000_000),
        (2 * NS, 2 * NS, 2 * NS),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut args = arguments(dir.path());
        args.offered_load_tps = "1".to_owned();
        args.measurement_seconds = "1".to_owned();
        let schedule = Schedule::from_args(&args).unwrap();
        let bounds = Bounds::from_args(&args).unwrap();
        let clock = TestClock::new(-NS);
        let mut backend = FakeBackend::new(clock.clone());
        backend.state_delay = global_delay;
        backend.local_delay = local_delay;
        backend.acknowledgment_delay = ack_delay;
        let backend = Arc::new(backend);
        let recorder = Arc::new(MemoryRecorder::default());
        let mut rows = schedule.plan(&args.seed, 1).unwrap();
        runtime()
            .block_on(collect_phase(
                backend.clone(),
                clock.clone(),
                recorder.clone(),
                &schedule,
                &bounds,
                &mut rows,
                0..1,
                Cohort::Measurement,
                &mut BTreeSet::new(),
            ))
            .expect("both evidence owners finish in the original window");
        assert_eq!(clock.now(), 2 * NS);
        assert_eq!(backend.offers.lock().unwrap().len(), 1);
        assert_eq!(rows[0].applied, Some((global_delay, 1)));
        assert_eq!(rows[0].local_applied, Some((local_delay, 1)));
        assert_eq!(rows[0].acknowledgment_ns, Some(ack_delay));
        assert_eq!(rows[0].attempts, 1);
        assert_eq!(rows[0].local_attempts, 1);
        let trace = rows[0].trace_value().unwrap();
        assert_eq!(trace["applied"]["offset_ns"].as_i64(), Some(global_delay));
        assert_eq!(trace["applied"]["scope"].as_str(), Some("global"));
        let final_event = rows[0].diagnostic_value();
        assert_eq!(final_event["local_block_height"].as_u64(), Some(1));
        assert_eq!(
            final_event["local_applied_offset_ns"].as_i64(),
            Some(local_delay)
        );
        let events = recorder.events.lock().unwrap();
        let local = events
            .iter()
            .find(|event| event["event"].as_str() == Some("local_status"))
            .expect("peer-local response is retained");
        assert_eq!(local["hash_matches"].as_bool(), Some(true));
        assert_eq!(local["local_scope_matches"].as_bool(), Some(true));
        assert_eq!(local["resolved_from"].as_str(), Some("state"));
    }
}

#[test]
fn local_missing_cached_global_wrong_height_hash_and_rejections_cannot_close_a_trial() {
    for case in 0..14 {
        let dir = tempfile::tempdir().unwrap();
        let mut args = arguments(dir.path());
        args.offered_load_tps = "1".to_owned();
        args.measurement_seconds = "1".to_owned();
        let schedule = Schedule::from_args(&args).unwrap();
        let bounds = Bounds::from_args(&args).unwrap();
        let clock = TestClock::new(-NS);
        let mut backend = FakeBackend::new(clock.clone());
        let mut local = local_response(exact_hash(1));
        match case {
            0 => backend.local_missing = true,
            1 => local.resolved_from = "cache".to_owned(),
            2 => local.resolved_from = "queue".to_owned(),
            3 => local.scope = "global".to_owned(),
            4 => local.status.block_height = Some(2),
            5 => local.hash = exact_hash(2).to_string(),
            6 => local.status.kind = "Rejected".to_owned(),
            7 => local.status.kind = "Expired".to_owned(),
            8 => local.status.block_height = None,
            9 => local.status.block_height = Some(0),
            10 => backend.local_delay = 2 * NS + 1,
            11 => backend.local_error = true,
            12 => local.status.kind = "Committed".to_owned(),
            13 => local.resolved_from = "private-token".to_owned(),
            _ => unreachable!(),
        }
        backend.local_response = Some(local);
        let backend = Arc::new(backend);
        let recorder = Arc::new(MemoryRecorder::default());
        let mut rows = schedule.plan(&args.seed, 1).unwrap();
        let error = runtime()
            .block_on(collect_phase(
                backend.clone(),
                clock.clone(),
                recorder.clone(),
                &schedule,
                &bounds,
                &mut rows,
                0..1,
                Cohort::Measurement,
                &mut BTreeSet::new(),
            ))
            .expect_err("global success cannot substitute for exact local application");
        assert_eq!(clock.now(), 2 * NS, "case {case} extends no deadline");
        assert_eq!(
            backend.offers.lock().unwrap().len(),
            1,
            "case {case} cannot replay"
        );
        assert_eq!(rows[0].applied, Some((1_000_000, 1)));
        assert!(rows[0].trace_value().is_err(), "case {case}");
        assert!(!error.to_string().contains("private-token"));
        assert!(
            !json::to_string(&rows[0].diagnostic_value())
                .unwrap()
                .contains("private-token")
        );
        assert!(
            !json::to_string(&*recorder.events.lock().unwrap())
                .unwrap()
                .contains("private-token")
        );
        if case == 0 {
            assert!(
                recorder
                    .events
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|event| event["event"].as_str() == Some("local_status_missing"))
            );
        }
    }
}

#[test]
fn local_height_conflict_fails_when_local_arrives_before_global() {
    let dir = tempfile::tempdir().unwrap();
    let args = arguments(dir.path());
    let schedule = Schedule::from_args(&args).unwrap();
    let bounds = Bounds::from_args(&args).unwrap();
    let clock = TestClock::new(-NS);
    let mut backend = FakeBackend::new(clock.clone());
    backend.state_delay = 200_000_000;
    let mut local = local_response(exact_hash(1));
    local.status.block_height = Some(2);
    backend.local_response = Some(local);
    let mut rows = schedule.plan(&args.seed, 1).unwrap();
    let error = runtime()
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
        .expect_err("height conflict");
    assert!(error.to_string().contains("exact block height"));
    assert_eq!(rows[0].applied, Some((200_000_000, 1)));
    assert_eq!(rows[0].local_applied, Some((1_000_000, 2)));
    assert!(!rows[0].settled());
}

#[test]
fn local_barrier_retains_in_flight_capacity_until_the_original_peer_applies() {
    let dir = tempfile::tempdir().unwrap();
    let mut args = arguments(dir.path());
    args.max_in_flight = 1;
    args.max_submission_lag_ms = "0".to_owned();
    let schedule = Schedule::from_args(&args).unwrap();
    let bounds = Bounds::from_args(&args).unwrap();
    for (delay, succeeds, offers) in [(NS / 2, true, 4), (NS / 2 + 1, false, 1)] {
        let clock = TestClock::new(-NS);
        let mut backend = FakeBackend::new(clock.clone());
        backend.local_delay = delay;
        let backend = Arc::new(backend);
        let mut rows = schedule.plan(&args.seed, 1).unwrap();
        let len = rows.len();
        let result = runtime().block_on(collect_phase(
            backend.clone(),
            clock,
            Arc::new(MemoryRecorder::default()),
            &schedule,
            &bounds,
            &mut rows,
            0..len,
            Cohort::Measurement,
            &mut BTreeSet::new(),
        ));
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(backend.offers.lock().unwrap().len(), offers);
        assert_eq!(rows[0].applied, Some((1_000_000, 1)));
        assert_eq!(rows[0].local_applied, Some((delay, 1)));
    }
}

#[test]
fn local_warmup_application_at_measurement_origin_fails_despite_earlier_global_success() {
    let dir = tempfile::tempdir().unwrap();
    let mut args = arguments(dir.path());
    args.offered_load_tps = "1".to_owned();
    args.warmup_seconds = "1".to_owned();
    let schedule = Schedule::from_args(&args).unwrap();
    let bounds = Bounds::from_args(&args).unwrap();
    let clock = TestClock::new(-3 * NS);
    let mut backend = FakeBackend::new(clock.clone());
    backend.local_delay = 2 * NS;
    let mut rows = schedule.plan(&args.seed, 1).unwrap();
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
    assert!(rows[0].applied.is_some());
    assert!(rows[0].local_applied.is_none());
    assert!(rows[1].offer_ns.is_none());
    assert!(rows[0].trace_value().is_err());
}

#[test]
fn global_trace_row_cannot_publish_without_matching_local_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let args = arguments(dir.path());
    let schedule = Schedule::from_args(&args).unwrap();
    let mut rows = schedule.plan(&args.seed, 1).unwrap();
    let row = &mut rows[0];
    row.hash = Some(exact_hash(1));
    row.offer_ns = Some(0);
    row.submission_finished = true;
    row.acknowledgment_ns = Some(2_000_000);
    row.applied = Some((1_000_000, 1));
    assert!(row.trace_value().is_err());
    row.local_applied = Some((3_000_000, 2));
    assert!(row.trace_value().is_err());
    row.local_applied = Some((3_000_000, 1));
    assert!(row.trace_value().is_ok());
}

#[test]
fn local_cached_applied_keeps_polling_until_state_without_changing_global_latency() {
    let dir = tempfile::tempdir().unwrap();
    let mut args = arguments(dir.path());
    args.offered_load_tps = "1".to_owned();
    args.measurement_seconds = "1".to_owned();
    let schedule = Schedule::from_args(&args).unwrap();
    let bounds = Bounds::from_args(&args).unwrap();
    let clock = TestClock::new(-NS);
    let mut backend = FakeBackend::new(clock.clone());
    backend.local_cached_until = Some(200_000_000);
    let recorder = Arc::new(MemoryRecorder::default());
    let mut rows = schedule.plan(&args.seed, 1).unwrap();
    runtime()
        .block_on(collect_phase(
            Arc::new(backend),
            clock,
            recorder.clone(),
            &schedule,
            &bounds,
            &mut rows,
            0..1,
            Cohort::Measurement,
            &mut BTreeSet::new(),
        ))
        .expect("cache cannot close but later local StateApplied can");
    assert_eq!(rows[0].applied, Some((1_000_000, 1)));
    assert_eq!(rows[0].local_applied, Some((201_000_000, 1)));
    assert_eq!(rows[0].attempts, 1);
    assert_eq!(rows[0].local_attempts, 5);
    let events = recorder.events.lock().unwrap();
    let local: Vec<_> = events
        .iter()
        .filter(|event| event["event"].as_str() == Some("local_status"))
        .collect();
    assert_eq!(local.len(), 5);
    assert!(
        local[..4]
            .iter()
            .all(|event| event["resolved_from"].as_str() == Some("cache"))
    );
    assert_eq!(local[4]["resolved_from"].as_str(), Some("state"));
    assert_eq!(
        rows[0].trace_value().unwrap()["applied"]["offset_ns"].as_i64(),
        Some(1_000_000)
    );
}

#[test]
fn global_and_local_share_the_original_observation_capacity_and_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let mut args = arguments(dir.path());
    args.offered_load_tps = "1".to_owned();
    args.measurement_seconds = "1".to_owned();
    args.max_status_requests = 1;
    let schedule = Schedule::from_args(&args).unwrap();
    let bounds = Bounds::from_args(&args).unwrap();
    let clock = TestClock::new(-NS);
    let mut backend = FakeBackend::new(clock.clone());
    backend.state_delay = 2 * NS;
    let mut rows = schedule.plan(&args.seed, 1).unwrap();
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
                Cohort::Measurement,
                &mut BTreeSet::new()
            ))
            .is_err()
    );
    assert_eq!(clock.now(), 2 * NS);
    assert_eq!(rows[0].applied, Some((2 * NS, 1)));
    assert_eq!(rows[0].attempts, 1);
    assert_eq!(
        rows[0].local_attempts, 0,
        "local cannot bypass the single shared read slot"
    );
    assert!(rows[0].local_applied.is_none());
    assert!(rows[0].trace_value().is_err());
}

#[test]
fn deadline_crossing_between_due_scopes_never_dispatches_or_counts_the_second_read() {
    let dir = tempfile::tempdir().unwrap();
    let mut args = arguments(dir.path());
    args.offered_load_tps = "1".to_owned();
    args.measurement_seconds = "1".to_owned();
    let schedule = Schedule::from_args(&args).unwrap();
    let bounds = Bounds::from_args(&args).unwrap();
    let deadline = schedule.deadline(Cohort::Measurement);
    let clock = TestClock::new(-NS);
    let mut backend = FakeBackend::new(clock.clone());
    backend.observation_start_advances_to = Some(deadline);
    let backend = Arc::new(backend);
    let mut records = schedule.plan(&args.seed, 1).unwrap();
    let result = runtime().block_on(collect_phase(
        backend.clone(),
        clock.clone(),
        Arc::new(MemoryRecorder::default()),
        &schedule,
        &bounds,
        &mut records,
        0..1,
        Cohort::Measurement,
        &mut BTreeSet::new(),
    ));
    assert!(result.is_err());
    let dispatches = backend.observation_dispatches.lock().unwrap();
    assert_eq!(
        dispatches.len(),
        1,
        "the second due scope must not reach Backend::observe"
    );
    assert!(dispatches[0].1 < deadline);
    assert_eq!(records[0].attempts + records[0].local_attempts, 1);
    match dispatches[0].0 {
        ObservationScope::Global => {
            assert_eq!(records[0].attempts, 1);
            assert_eq!(records[0].local_attempts, 0);
        }
        ObservationScope::Local => {
            assert_eq!(records[0].local_attempts, 1);
            assert_eq!(records[0].attempts, 0);
        }
    }
    assert!(!records[0].settled());
    assert!(records[0].trace_value().is_err());
}

#[test]
fn queued_observation_checks_the_clock_on_first_poll_not_future_construction() {
    for scope in [ObservationScope::Global, ObservationScope::Local] {
        let clock = TestClock::new(0);
        let backend = Arc::new(FakeBackend::new(clock.clone()));
        let (started, mut receiver) = tokio::sync::mpsc::channel(1);
        let future = dispatch_observation(
            backend.clone(),
            clock.clone(),
            started,
            ObservationRequest {
                index: 0,
                account_index: 0,
                hash: exact_hash(1),
                scope,
                deadline: NS,
            },
        );
        clock.timer_reached(NS);
        let (_, (_, offset, result)) = runtime().block_on(future);
        assert!(result.is_err());
        assert_eq!(offset, NS);
        assert!(backend.observation_dispatches.lock().unwrap().is_empty());
        assert!(
            receiver.try_recv().is_err(),
            "refused reads emit no attempt notification"
        );
    }
}

#[test]
fn bounded_start_notification_must_be_reserved_before_backend_dispatch() {
    let clock = TestClock::new(0);
    let backend = Arc::new(FakeBackend::new(clock.clone()));
    let (started, mut receiver) = tokio::sync::mpsc::channel(1);
    started
        .try_send(ObservationStarted {
            index: 0,
            scope: ObservationScope::Global,
        })
        .unwrap();
    let future = dispatch_observation(
        backend.clone(),
        clock,
        started,
        ObservationRequest {
            index: 1,
            account_index: 0,
            hash: exact_hash(2),
            scope: ObservationScope::Local,
            deadline: NS,
        },
    );
    let (_, (_, _, result)) = runtime().block_on(future);
    assert!(result.is_err());
    assert!(backend.observation_dispatches.lock().unwrap().is_empty());
    assert_eq!(receiver.try_recv().unwrap().index, 0);
    assert!(receiver.try_recv().is_err());
}

#[test]
fn issued_observation_count_survives_cancellation_without_a_completion_event() {
    for scope in [ObservationScope::Global, ObservationScope::Local] {
        let dir = tempfile::tempdir().unwrap();
        let args = arguments(dir.path());
        let schedule = Schedule::from_args(&args).unwrap();
        let mut records = schedule.plan(&args.seed, 1).unwrap();
        let clock = TestClock::new(0);
        let backend = Arc::new(FakeBackend::new(clock.clone()));
        backend.offers.lock().unwrap().insert(exact_hash(1), 0);
        let (started, mut receiver) = tokio::sync::mpsc::channel(1);
        let mut future = dispatch_observation(
            backend.clone(),
            clock.clone(),
            started,
            ObservationRequest {
                index: 0,
                account_index: 0,
                hash: exact_hash(1),
                scope,
                deadline: NS,
            },
        );
        runtime().block_on(poll_fn(|context| {
            assert!(future.as_mut().poll(context).is_pending());
            Poll::Ready(())
        }));
        drop(future);
        drain_observation_starts(&mut records, &mut receiver).unwrap();
        assert_eq!(records[0].attempts + records[0].local_attempts, 1);
        assert_eq!(backend.observation_dispatches.lock().unwrap().len(), 1);
        assert!(records[0].applied.is_none());
        assert!(records[0].local_applied.is_none());
        drain_observation_starts(&mut records, &mut receiver).unwrap();
        assert_eq!(
            records[0].attempts + records[0].local_attempts,
            1,
            "start consumed exactly once"
        );
    }
}

#[test]
fn observation_start_counter_checks_index_and_overflow_without_wrapping() {
    let dir = tempfile::tempdir().unwrap();
    let args = arguments(dir.path());
    let schedule = Schedule::from_args(&args).unwrap();
    let mut records = schedule.plan(&args.seed, 1).unwrap();
    let length = records.len();
    assert!(
        record_observation_start(
            &mut records,
            ObservationStarted {
                index: length,
                scope: ObservationScope::Global
            }
        )
        .is_err()
    );
    records[0].local_attempts = usize::MAX;
    assert!(
        record_observation_start(
            &mut records,
            ObservationStarted {
                index: 0,
                scope: ObservationScope::Local
            }
        )
        .is_err()
    );
    assert_eq!(records[0].local_attempts, usize::MAX);
    assert_eq!(records[0].attempts, 0);
}

fn terminal_test_artifact(path: &Path, raw: &[u8]) -> output::RetainedLoadFile {
    let original = output::JournalOutput::create(path, 4096).unwrap();
    let mut writer =
        terminal_receipt::DigestWriter::new(BufWriter::new(original.writer_file().unwrap()));
    writer.write_all(raw).unwrap();
    writer.flush().unwrap();
    writer.get_ref().sync_all().unwrap();
    let (writer, identity) = writer.finish().unwrap();
    drop(writer);
    original.seal(identity).unwrap()
}

#[test]
fn load_terminal_invocation_is_required_canonical_and_nonzero() {
    assert!(
        TestCli::try_parse_from(["load"])
            .err()
            .unwrap()
            .to_string()
            .contains("--invocation-id")
    );
    for value in [
        String::new(),
        "0".repeat(64),
        "A".repeat(64),
        "a".repeat(63),
        "g".repeat(64),
    ] {
        assert!(terminal_receipt::validate_invocation(&value).is_err());
    }
    assert!(terminal_receipt::validate_invocation(&"a".repeat(64)).is_ok());
    let directory = tempfile::tempdir().unwrap();
    let args = arguments(directory.path());
    assert_eq!(args.invocation_id, "b".repeat(64));
}

#[test]
fn terminal_receipt_binds_actual_writer_digests_without_human_output() {
    let directory = tempfile::tempdir().unwrap();
    let args = arguments(directory.path());
    let journal = terminal_test_artifact(&args.diagnostic_out, b"{\"event\":\"test\"}\n");
    let trace = terminal_test_artifact(&args.trace_out, b"{\"transactions\":[]}\n");
    let ji = journal.identity().unwrap();
    let ti = trace.identity().unwrap();
    let mut reply = Vec::new();
    terminal_receipt::emit(&args, 4, journal, trace, &mut reply).unwrap();
    assert!(reply.len() <= terminal_receipt::MAX_REPLY_BYTES);
    assert_eq!(reply.last(), Some(&b'\n'));
    assert_eq!(reply.iter().filter(|byte| **byte == b'\n').count(), 1);
    let value: Value = json::from_slice(&reply).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 12);
    assert_eq!(value["operation"].as_str(), Some("transaction_load"));
    assert_eq!(
        value["invocation_id"].as_str(),
        Some(args.invocation_id.as_str())
    );
    assert_eq!(
        value["collector_journal_sha256"].as_str(),
        Some(hex::encode(ji.raw_sha256).as_str())
    );
    assert_eq!(
        value["collector_journal_bytes"].as_u64(),
        Some(ji.byte_length)
    );
    assert_eq!(
        value["trace_sha256"].as_str(),
        Some(hex::encode(ti.raw_sha256).as_str())
    );
    assert_eq!(value["trace_bytes"].as_u64(), Some(ti.byte_length));
    assert!(!String::from_utf8(reply).unwrap().contains("Recorded"));
}

#[test]
fn terminal_receipt_retains_both_originals_through_write_and_flush() {
    struct MutatingWriter {
        target: PathBuf,
        mutate_flush: bool,
        fired: bool,
    }
    impl Write for MutatingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if !self.mutate_flush && !self.fired {
                self.fired = true;
                std::fs::write(&self.target, b"foreign output")?;
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            if self.mutate_flush && !self.fired {
                self.fired = true;
                std::fs::write(&self.target, b"foreign output")?;
            }
            Ok(())
        }
    }
    for journal_target in [false, true] {
        for mutate_flush in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let args = arguments(directory.path());
            let journal = terminal_test_artifact(&args.diagnostic_out, b"journal\n");
            let trace = terminal_test_artifact(&args.trace_out, b"trace\n");
            let target = if journal_target {
                args.diagnostic_out.clone()
            } else {
                args.trace_out.clone()
            };
            let mut writer = MutatingWriter {
                target,
                mutate_flush,
                fired: false,
            };
            assert!(terminal_receipt::emit(&args, 4, journal, trace, &mut writer).is_err());
            assert!(writer.fired);
        }
    }
}

#[test]
fn completed_journal_must_match_actual_writer_bytes_and_preserve_original_inode() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().canonicalize().unwrap().join("journal");
    let original = output::JournalOutput::create(&path, 4096).unwrap();
    let mut writer =
        terminal_receipt::DigestWriter::new(BufWriter::new(original.writer_file().unwrap()));
    writer.write_all(b"original").unwrap();
    let (writer, identity) = writer.finish().unwrap();
    drop(writer);
    std::fs::write(&path, b"mutated!").unwrap();
    assert!(original.seal(identity).is_err());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn terminal_pair_rejects_earlier_leaf_replacement_during_later_scan() {
    let temporary = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(temporary.path()).unwrap();
    let journal_path = root.join("journal.jsonl");
    let trace_path = root.join("trace.json");
    let journal = terminal_test_artifact(&journal_path, b"journal\n");
    let trace = terminal_test_artifact(&trace_path, b"trace\n");
    assert_eq!(format!("{journal:?}"), "RetainedLoadFile { .. }");
    let detached = root.join("detached-journal.jsonl");
    assert!(
        journal
            .pair_identity_with_midpoint(&trace, || {
                std::fs::rename(&journal_path, &detached)?;
                std::fs::write(&journal_path, b"journal\n")?;
                Ok(())
            })
            .is_err()
    );
    assert!(journal.identity().is_err());
    assert!(trace.identity().is_err());
    assert_eq!(std::fs::read(&journal_path).unwrap(), b"journal\n");
    assert_eq!(std::fs::read(&detached).unwrap(), b"journal\n");
}

#[test]
fn terminal_digest_counts_actual_short_writes_and_propagates_flush_failure() {
    struct ShortWriter(Vec<u8>);
    impl std::io::Write for ShortWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let length = bytes.len().min(2);
            self.0.extend_from_slice(&bytes[..length]);
            Ok(length)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = terminal_receipt::DigestWriter::new(ShortWriter(Vec::new()));
    writer.write_all(b"abcdef").unwrap();
    let (writer, identity) = writer.finish().unwrap();
    assert_eq!(writer.0, b"abcdef");
    assert_eq!(identity.byte_length, 6);
    assert_eq!(
        identity.raw_sha256,
        <[u8; 32]>::from(Sha256::digest(b"abcdef"))
    );
    struct FailingFlush;
    impl std::io::Write for FailingFlush {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("test flush failure"))
        }
    }
    assert!(
        terminal_receipt::DigestWriter::new(FailingFlush)
            .finish()
            .is_err()
    );
}
