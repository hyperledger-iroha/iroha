//! Deterministic clock, schedule and strict protocol controls without child execution.

use super::*;
use std::{
    future::{Future, poll_fn},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
    task::{Context, Poll, Waker},
};

#[derive(Default)]
struct Memory {
    values: Mutex<Vec<Value>>,
}
impl Recorder for Memory {
    fn record(&self, value: Value) -> Result<()> {
        self.values.lock().unwrap().push(value);
        Ok(())
    }
}
struct ManualClock {
    now: AtomicI64,
    waiters: Mutex<Vec<Waker>>,
}
impl ManualClock {
    fn new(now: i64) -> Arc<Self> {
        Arc::new(Self {
            now: AtomicI64::new(now),
            waiters: Mutex::new(Vec::new()),
        })
    }
    fn advance(&self, now: i64) {
        assert!(now >= self.now());
        self.now.store(now, Ordering::SeqCst);
        for waker in std::mem::take(&mut *self.waiters.lock().unwrap()) {
            waker.wake();
        }
    }
}
impl Clock for ManualClock {
    fn now(&self) -> i64 {
        self.now.load(Ordering::SeqCst)
    }
    fn sleep(self: Arc<Self>, offset: i64) -> BoxFuture<'static, i64> {
        poll_fn(move |cx| {
            if self.now() >= offset {
                Poll::Ready(self.now())
            } else {
                self.waiters.lock().unwrap().push(cx.waker().clone());
                Poll::Pending
            }
        })
        .boxed()
    }
}
struct Scripted {
    clock: Arc<ManualClock>,
    delay: i64,
    outcome: Outcome,
    broken_finish: bool,
    requests: Mutex<Vec<Request>>,
    aborted: AtomicBool,
}
impl Scripted {
    fn new(clock: Arc<ManualClock>) -> Self {
        Self {
            clock,
            delay: 10,
            outcome: Outcome::Complete,
            broken_finish: false,
            requests: Mutex::new(Vec::new()),
            aborted: AtomicBool::new(false),
        }
    }
}
impl Probe for Scripted {
    fn exchange(&self, request: Request) -> BoxFuture<'_, Result<Response>> {
        async move {
            self.requests.lock().unwrap().push(request);
            let target = self.clock.now() + self.delay;
            wait_until(self.clock.clone(), target).await;
            if request.kind == Kind::Finish && self.broken_finish {
                bail!("injected unsuccessful closure");
            }
            Ok(Response {
                outcome: self.outcome,
                manifest: (request.kind != Kind::Finish).then(|| Manifest {
                    name: request.kind.manifest_name(request.sequence),
                    sha256: "a".repeat(64),
                    bytes: 1,
                }),
            })
        }
        .boxed()
    }
    fn abort(&self) {
        self.aborted.store(true, Ordering::SeqCst);
    }
}
fn plan() -> Plan {
    Plan {
        interval_ns: 100,
        timeout_ns: 40,
        start_lag_ns: 10,
        final_offset_ns: 300,
        samples: 4,
        lifetime: Duration::from_secs(2),
    }
}
fn poll<T>(future: &mut std::pin::Pin<Box<impl Future<Output = T>>>) -> Poll<T> {
    future
        .as_mut()
        .poll(&mut Context::from_waker(futures::task::noop_waker_ref()))
}
fn reply(request: Request, outcome: &str, manifest: Value) -> Vec<u8> {
    let mut bytes = json::to_vec(
        &norito::json!({"schema": RESPONSE_SCHEMA, "kind": (request.kind.text()),
        "sequence": (request.sequence), "outcome": outcome, "manifest": manifest}),
    )
    .unwrap();
    bytes.push(b'\n');
    bytes
}
fn reference(request: Request) -> Value {
    norito::json!({"name": (request.kind.manifest_name(request.sequence)), "sha256": ("a".repeat(64)), "bytes": 1})
}

#[test]
fn sampler_brackets_every_fixed_sample_through_complete_drain_then_closes() {
    let clock = ManualClock::new(-100);
    let probe = Scripted::new(clock.clone());
    let recorder = Arc::new(Memory::default());
    let mut run = Box::pin(collect(&probe, clock.clone(), recorder.clone(), plan()));
    assert!(poll(&mut run).is_pending());
    assert!(probe.requests.lock().unwrap().is_empty());
    for time in [0, 10, 100, 110, 200, 210, 300, 310] {
        clock.advance(time);
        assert!(poll(&mut run).is_pending());
    }
    clock.advance(320);
    assert!(matches!(poll(&mut run), Poll::Ready(Ok(()))));
    let requests = probe.requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .map(|r| (r.kind, r.sequence))
            .collect::<Vec<_>>(),
        vec![
            (Kind::Sample, 1),
            (Kind::Sample, 2),
            (Kind::Sample, 3),
            (Kind::Sample, 4),
            (Kind::Finish, 5)
        ]
    );
    let events = recorder.values.lock().unwrap();
    let observations: Vec<_> = events
        .iter()
        .filter(|v| v.get("event").and_then(Value::as_str) == Some("resource_observation"))
        .collect();
    assert_eq!(observations.len(), 4);
    for (index, row) in observations.iter().enumerate() {
        assert_eq!(
            row.get("scheduled_offset_ns").and_then(Value::as_i64),
            Some(index as i64 * 100)
        );
        assert_eq!(
            row.get("start_offset_ns").and_then(Value::as_i64),
            Some(index as i64 * 100)
        );
        assert_eq!(
            row.get("end_offset_ns").and_then(Value::as_i64),
            Some(index as i64 * 100 + 10)
        );
        assert!(json::to_vec(row).unwrap().len() < MAX_EVENT_BYTES);
    }
    assert!(!probe.aborted.load(Ordering::SeqCst));
}

#[test]
fn slow_resource_requests_do_not_move_independent_fixed_offer_times() {
    let clock = ManualClock::new(0);
    let mut probe = Scripted::new(clock.clone());
    probe.delay = 30;
    let recorder = Arc::new(Memory::default());
    let offered = Arc::new(Mutex::new(Vec::new()));
    let offers = {
        let clock = clock.clone();
        let offered = offered.clone();
        async move {
            for scheduled in [0, 25, 50, 75, 100, 125, 150, 175, 200] {
                wait_until(clock.clone(), scheduled).await;
                offered.lock().unwrap().push((scheduled, clock.now()));
            }
        }
    };
    let mut both = Box::pin(async {
        futures::join!(offers, collect(&probe, clock.clone(), recorder, plan()))
    });
    assert!(poll(&mut both).is_pending());
    for time in [25, 30, 50, 75, 100, 125, 130, 150, 175, 200, 230, 300, 330] {
        clock.advance(time);
        assert!(poll(&mut both).is_pending());
    }
    clock.advance(360);
    assert!(matches!(poll(&mut both), Poll::Ready(((), Ok(())))));
    assert_eq!(
        *offered.lock().unwrap(),
        [0, 25, 50, 75, 100, 125, 150, 175, 200].map(|v| (v, v))
    );
}

#[test]
fn whole_clock_deadline_rejects_boundary_response_and_aborts_only_probe() {
    for delay in [40, 41, 10_000] {
        let clock = ManualClock::new(0);
        let mut probe = Scripted::new(clock.clone());
        probe.delay = delay;
        let memory = Arc::new(Memory::default());
        let mut run = Box::pin(collect(&probe, clock.clone(), memory.clone(), plan()));
        assert!(poll(&mut run).is_pending());
        clock.advance(40);
        assert!(matches!(poll(&mut run), Poll::Ready(Err(_))));
        assert!(probe.aborted.load(Ordering::SeqCst));
        assert_eq!(probe.requests.lock().unwrap().len(), 1);
        assert!(!memory.values.lock().unwrap().iter().any(
            |v| v.get("event").and_then(Value::as_str) == Some("resource_collection_finished")
        ));
    }
}

#[test]
fn late_start_and_unavailable_observation_never_publish_success() {
    for unavailable in [false, true] {
        let clock = ManualClock::new(if unavailable { 0 } else { 11 });
        let mut probe = Scripted::new(clock.clone());
        probe.outcome = Outcome::Unavailable;
        let memory = Arc::new(Memory::default());
        let mut run = Box::pin(collect(&probe, clock.clone(), memory.clone(), plan()));
        if unavailable {
            assert!(poll(&mut run).is_pending());
            clock.advance(10);
        }
        assert!(matches!(poll(&mut run), Poll::Ready(Err(_))));
        assert!(probe.aborted.load(Ordering::SeqCst));
        assert_eq!(
            probe.requests.lock().unwrap().len(),
            usize::from(unavailable)
        );
        assert!(!memory.values.lock().unwrap().iter().any(
            |v| v.get("event").and_then(Value::as_str) == Some("resource_collection_finished")
        ));
    }
}

#[test]
fn successful_samples_still_require_successful_finish_closure() {
    let clock = ManualClock::new(0);
    let mut probe = Scripted::new(clock.clone());
    probe.broken_finish = true;
    let memory = Arc::new(Memory::default());
    let mut run = Box::pin(collect(&probe, clock.clone(), memory.clone(), plan()));
    assert!(poll(&mut run).is_pending());
    for time in [10, 100, 110, 200, 210, 300, 310] {
        clock.advance(time);
        assert!(poll(&mut run).is_pending());
    }
    clock.advance(320);
    assert!(matches!(poll(&mut run), Poll::Ready(Err(_))));
    assert_eq!(
        probe.requests.lock().unwrap().last().unwrap().kind,
        Kind::Finish
    );
    assert!(probe.aborted.load(Ordering::SeqCst));
}

#[test]
fn response_protocol_rejects_wrong_sequence_identity_shape_and_unbounded_reference() {
    let request = Request {
        kind: Kind::Sample,
        sequence: 7,
        timeout_ms: 400,
    };
    let valid = reply(request, "complete", reference(request));
    assert_eq!(
        parse_response(request, &valid).unwrap().outcome,
        Outcome::Complete
    );
    let value: Value = json::from_slice(&valid).unwrap();
    for (key, replacement) in [
        ("schema", norito::json!("other")),
        ("kind", norito::json!("preflight")),
        ("sequence", norito::json!(6)),
        ("outcome", norito::json!("pass")),
        ("offset", norito::json!(0)),
    ] {
        let mut mutant = value.clone();
        mutant
            .as_object_mut()
            .unwrap()
            .insert(key.to_owned(), replacement);
        let mut bytes = json::to_vec(&mutant).unwrap();
        bytes.push(b'\n');
        assert!(parse_response(request, &bytes).is_err());
    }
    for (key, replacement) in [
        ("name", norito::json!("../sample-0000000007.json")),
        ("sha256", norito::json!("A".repeat(64))),
        ("bytes", norito::json!(0)),
        ("bytes", norito::json!(MAX_MANIFEST_BYTES + 1)),
        ("raw", norito::json!("body")),
    ] {
        let mut manifest = reference(request);
        manifest
            .as_object_mut()
            .unwrap()
            .insert(key.to_owned(), replacement);
        assert!(parse_response(request, &reply(request, "complete", manifest)).is_err());
    }
    assert!(parse_response(request, &reply(request, "complete", Value::Null)).is_err());
    assert!(parse_response(request, &reply(request, "unavailable", Value::Null)).is_err());
    assert_eq!(
        parse_response(request, &reply(request, "failed", Value::Null))
            .unwrap()
            .outcome,
        Outcome::Failed
    );
    assert!(parse_response(request, &vec![b'a'; MAX_IPC_BYTES + 1]).is_err());
    assert!(parse_response(request, b"").is_err());
    assert!(parse_response(request, &[valid.as_slice(), b"{}\n"].concat()).is_err());
}

#[test]
fn protocol_finish_has_no_manifest_and_preflight_uses_sequence_zero() {
    let preflight = Request {
        kind: Kind::Preflight,
        sequence: 0,
        timeout_ms: 400,
    };
    let finish = Request {
        kind: Kind::Finish,
        sequence: 9,
        timeout_ms: 400,
    };
    assert!(
        parse_response(
            preflight,
            &reply(preflight, "complete", reference(preflight))
        )
        .is_ok()
    );
    assert!(parse_response(finish, &reply(finish, "complete", Value::Null)).is_ok());
    assert!(parse_response(finish, &reply(finish, "complete", reference(finish))).is_err());
    let line = preflight.line().unwrap();
    assert!(line.len() < MAX_IPC_BYTES);
    let value: Value = json::from_slice(&line).unwrap();
    assert_eq!(
        value.get("schema").and_then(Value::as_str),
        Some(REQUEST_SCHEMA)
    );
    assert_eq!(value.as_object().unwrap().len(), 4);
}

#[test]
fn sampling_geometry_is_exact_bounded_and_covers_final_endpoint() {
    let args = Args {
        resource_program: "/unused/python".into(),
        resource_worker: "/unused/worker".into(),
        resource_budget_sha256: "a".repeat(64),
        resource_config: "/unused/secret".into(),
        resource_capture_dir: "/unused/new".into(),
        resource_interval_ms: 1000,
        resource_timeout_ms: 400,
        resource_max_start_lag_ms: 100,
    };
    let schedule = Schedule {
        rate_numerator: 2,
        rate_denominator: 1,
        warmup_ns: 2 * NS,
        measurement_ns: 20 * NS,
        drain_ns: NS,
        lag_ns: 0,
    };
    let bounds = Bounds {
        lookahead: 256,
        preparations: 8,
        ahead_ns: NS,
        submissions: 256,
        in_flight: 4096,
        observations: 64,
        poll_ns: 50_000_000,
    };
    let plan = Plan::new(&args, &schedule, &bounds).unwrap();
    assert_eq!(plan.samples, 22);
    assert_eq!(plan.final_offset_ns, 21 * NS);
    for (interval, timeout, lag) in [
        (0, 400, 100),
        (3, 1, 0),
        (1000, 501, 100),
        (1000, 400, 251),
        (u64::MAX, 1, 0),
    ] {
        let changed = Args {
            resource_interval_ms: interval,
            resource_timeout_ms: timeout,
            resource_max_start_lag_ms: lag,
            resource_program: args.resource_program.clone(),
            resource_worker: args.resource_worker.clone(),
            resource_config: args.resource_config.clone(),
            resource_capture_dir: args.resource_capture_dir.clone(),
        };
        assert!(Plan::new(&changed, &schedule, &bounds).is_err());
    }
    let too_short = Schedule {
        measurement_ns: 19 * NS,
        ..schedule.clone()
    };
    assert!(Plan::new(&args, &too_short, &bounds).is_err());
    let too_many = Schedule {
        measurement_ns: 100_000 * NS,
        ..schedule
    };
    assert!(Plan::new(&args, &too_many, &bounds).is_err());
    assert!(!json::to_string(&plan.value()).unwrap().contains("unused"));
}

#[test]
fn delayed_final_start_cannot_extend_the_endpoint_completion_deadline() {
    for delay in [29, 30] {
        let clock = ManualClock::new(0);
        let mut probe = Scripted::new(clock.clone());
        probe.delay = delay;
        let memory = Arc::new(Memory::default());
        let mut run = Box::pin(collect(&probe, clock.clone(), memory.clone(), plan()));
        assert!(poll(&mut run).is_pending());
        for time in [30, 100, 130, 200, 230, 310] {
            clock.advance(time);
            assert!(poll(&mut run).is_pending());
        }
        clock.advance(310 + delay);
        if delay == 30 {
            assert!(matches!(poll(&mut run), Poll::Ready(Err(_))));
            assert!(probe.aborted.load(Ordering::SeqCst));
            assert_eq!(probe.requests.lock().unwrap().len(), 4);
        } else {
            assert!(poll(&mut run).is_pending());
            clock.advance(339 + delay);
            assert!(matches!(poll(&mut run), Poll::Ready(Ok(()))));
            let events = memory.values.lock().unwrap();
            let last = events
                .iter()
                .filter(|value| {
                    value.get("event").and_then(Value::as_str) == Some("resource_observation")
                })
                .last()
                .unwrap();
            assert_eq!(
                last.get("scheduled_offset_ns").and_then(Value::as_i64),
                Some(300)
            );
            assert_eq!(
                last.get("start_offset_ns").and_then(Value::as_i64),
                Some(310)
            );
            assert_eq!(last.get("end_offset_ns").and_then(Value::as_i64), Some(339));
        }
    }
}

#[test]
fn response_decoder_rejects_duplicate_keys_before_exact_shape_validation() {
    let request = Request {
        kind: Kind::Sample,
        sequence: 7,
        timeout_ms: 400,
    };
    let valid = reply(request, "complete", reference(request));
    assert!(parse_response(request, &valid).is_ok());
    let text = String::from_utf8(valid).unwrap();
    let duplicate = text.replacen("{", "{\"sequence\":7,", 1);
    assert!(parse_response(request, duplicate.as_bytes()).is_err());
    let nested_duplicate = text.replacen("\"manifest\":{", "\"manifest\":{\"bytes\":1,", 1);
    assert_ne!(nested_duplicate, text);
    assert!(parse_response(request, nested_duplicate.as_bytes()).is_err());
}

#[test]
fn resource_arguments_are_mandatory_and_never_create_an_unsampled_mode() {
    use clap::Parser;
    #[derive(clap::Parser)]
    struct ResourceCli {
        #[command(flatten)]
        args: Args,
    }
    let full = [
        "resource",
        "--resource-program",
        "/unused/python",
        "--resource-worker",
        "/unused/worker",
        "--resource-config",
        "/unused/runtime-secret",
        "--resource-capture-dir",
        "/unused/captures",
        "--resource-interval-ms",
        "1000",
        "--resource-timeout-ms",
        "400",
        "--resource-max-start-lag-ms",
        "100",
    ];
    assert!(ResourceCli::try_parse_from(full).is_ok());
    assert_eq!(
        ResourceCli::try_parse_from(full)
            .unwrap()
            .args
            .resource_interval_ms,
        1000
    );
    assert!(ResourceCli::try_parse_from(["resource"]).is_err());
    for position in (1..full.len()).step_by(2) {
        let missing: Vec<_> = full
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                (index != position && index != position + 1).then_some(*value)
            })
            .collect();
        assert!(ResourceCli::try_parse_from(missing).is_err());
    }
}
