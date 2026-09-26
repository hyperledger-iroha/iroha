//! The full driver on its threads over the fake backends: a single validator's chain commits
//! with failing and panicking writes and apply steps retried; a stopped worker stops the
//! instance; O9 — two instances in one process, one stalled (its executor never returns) and
//! flooded, the other committing on time; §12.2 serving under a `SyncRequest` flood with the
//! node's own body fetch served on time; O10 frame limits at start and after a committed
//! parameter change; raw-frame delivery; `Init` assembly from the block store.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use iroha_sumeragi::{
    api::{ExecOutcome, HaltReason, LocalParams},
    crypto::{NoAttestation, Signer},
    message::{BlockRequest, BlockResponse, Status, SyncRequest, VoteKind, WireMessage},
    safety::RecordState,
    sim::driver::block_exec,
    testing::{FakeCrypto, FakeSigner, FakeValidators},
    types::{ChainParams, Committee, Hash32, HeightConfig, PublicKey},
};

use super::{
    super::{
        Driver, DriverConfig, DriverError, DriverHandle, DriverStart, ExitGuard, Input, Op,
        RunningDriver, SharedCrypto, Worker, Workers, assemble_init,
        exec::ExecOp,
        persist::install_records,
        traits::{BlockStore, Clock, NoObserver, Observer, SystemClock},
    },
    block, commit_qc,
    fakes::{
        FakeBlocks, FakeBodies, FakeClock, FakeExecutor, FakeNet, FakeRecords, RecordingObserver,
    },
    hash,
};

fn params() -> ChainParams {
    ChainParams {
        block_time: 10,
        idle_block_interval: 30,
        ..ChainParams::default()
    }
}

/// The fakes of one instance, shared with its test.
#[derive(Clone)]
struct Fakes {
    exec: FakeExecutor,
    net: Arc<FakeNet>,
    records: Arc<FakeRecords>,
    bodies: Arc<FakeBodies>,
    blocks: Arc<FakeBlocks>,
    observer: Arc<RecordingObserver>,
}

/// One single-validator instance on its threads.
struct Instance {
    running: RunningDriver,
    key: PublicKey,
    instance: Hash32,
    fakes: Fakes,
}

impl Instance {
    fn handle(&self) -> DriverHandle {
        self.running.handle()
    }

    fn committed(&self) -> u64 {
        self.handle().status().map_or(0, |s| s.committed_height)
    }
}

/// A fresh instance `tag` whose key was generated on the node; `prepare` may set up failures
/// or a closed executor gate before it starts.
fn spawn_instance<C: Clock + 'static>(
    tag: u8,
    clock: Arc<C>,
    prepare: impl FnOnce(&Fakes),
) -> Instance {
    let signer = FakeSigner::from_seed(&[tag], None);
    let key = signer.public_key().clone();
    let crypto: SharedCrypto = Arc::new(FakeCrypto::new());
    let instance = Hash32([tag; 32]);
    let config = HeightConfig {
        committee: Committee::new(vec![key.clone()]).unwrap(),
        params: params(),
    };
    let records = Arc::new(FakeRecords::default());
    records.install_key(&key, true, 1);
    let mut next = 1u128;
    let mut fresh = || {
        next += 1;
        next
    };
    let found = install_records(
        &*records,
        &*crypto,
        &instance,
        &[(key.clone(), false)],
        0,
        false,
        &mut fresh,
    )
    .unwrap();
    assert!(matches!(found[0].1, RecordState::Present(_)));
    let blocks = Arc::new(FakeBlocks::default());
    let genesis = (Hash32([tag ^ 0x11; 32]), Hash32([tag ^ 0x22; 32]));
    let init = assemble_init(
        &*blocks,
        instance,
        0,
        genesis,
        128,
        found,
        vec![(1, config.clone()), (2, config.clone())],
        u64::from(tag),
    )
    .unwrap();
    let fakes = Fakes {
        exec: FakeExecutor::new(genesis.0, genesis.1, config),
        net: Arc::new(FakeNet::default()),
        records,
        bodies: Arc::new(FakeBodies::default()),
        blocks,
        observer: Arc::new(RecordingObserver::default()),
    };
    prepare(&fakes);
    let observer: Arc<dyn Observer> = fakes.observer.clone();
    let driver = Driver::new(
        Arc::clone(&fakes.net),
        Arc::clone(&fakes.records),
        Arc::clone(&fakes.bodies),
        Arc::clone(&fakes.blocks),
        clock,
        fakes.exec.clone(),
        observer,
    );
    let running = driver
        .spawn(
            DriverConfig::default(),
            DriverStart {
                local: LocalParams::default(),
                init,
                signers: vec![Box::new(signer)],
                crypto,
                attestor: Box::new(NoAttestation),
                verifier: Box::new(NoAttestation),
            },
        )
        .unwrap();
    Instance {
        running,
        key,
        instance,
        fakes,
    }
}

fn wait_until(what: &str, limit: Duration, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < limit, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A single validator commits through the whole driver — persistence, executor and serve
/// threads, the loop and the barrier — while record, body and block-store writes and apply
/// steps fail and are retried (never skipped); transactions reach blocks.
#[test]
fn single_validator_commits_through_failures() {
    let node = spawn_instance(1, Arc::new(SystemClock::new()), |fakes| {
        fakes.records.fail_next(3);
        fakes.bodies.fail_next(2);
        fakes.blocks.fail_next(2);
        fakes.exec.state.lock().fail_apply = 2;
    });
    assert!(node.handle().ready());
    for id in 0..20 {
        node.fakes.exec.add_tx(id);
    }
    node.handle().transactions_available();
    wait_until("20 heights", Duration::from_secs(20), || {
        node.committed() >= 20
    });
    let status = node.handle().status().unwrap();
    assert!(status.halted.is_none() && node.handle().halted().is_none());
    assert!(
        node.fakes.blocks.height() >= 19,
        "applied heights are in the block store"
    );
    assert!(node.fakes.records.writes() > 0);
    let payloads = (1..=node.fakes.blocks.height())
        .filter_map(|h| node.fakes.blocks.entry(h))
        .filter(|e| !e.block.payload.is_empty())
        .count();
    assert!(payloads > 0, "transactions were included");
    assert!(
        node.fakes.exec.state.lock().txs.is_empty(),
        "applied transactions left the queue"
    );
    assert!(node.fakes.bodies.len() <= 4, "applied bodies are pruned");
    node.running.shutdown();
}

/// O9: instance A's executor never returns and its ingress is flooded; instance B, in the same
/// process, commits on time, and delivering to A never blocks (the flood is bounded, O6).
#[test]
fn stalled_and_flooded_instance_does_not_delay_another() {
    let a = spawn_instance(3, Arc::new(SystemClock::new()), |fakes| {
        fakes.exec.set_open(false)
    });
    let b = spawn_instance(4, Arc::new(SystemClock::new()), |_| {});
    let flood = {
        let handle = a.handle();
        let instance = a.instance;
        std::thread::spawn(move || {
            let start = Instant::now();
            let mut longest = 0;
            for i in 0..20_000u64 {
                let peer = PublicKey::new(vec![u8::try_from(i % 50).unwrap(); 32]).unwrap();
                let msg = if i % 2 == 0 {
                    WireMessage::BlockRequest(BlockRequest {
                        instance,
                        height: i,
                        block_hash: Hash32::ZERO,
                    })
                } else {
                    WireMessage::Status(Box::new(Status {
                        instance,
                        height: i,
                        view: 0,
                        committed_qc: None,
                        high_pqc: None,
                        high_tc: None,
                        proposal_hash: None,
                        want_proposal: false,
                        probe: None,
                        echo: None,
                    }))
                };
                handle.deliver_message(peer, msg);
                longest = longest.max(handle.shared.ingress.lock().len());
            }
            (start.elapsed(), longest)
        })
    };
    let start = Instant::now();
    wait_until("B commits 20 heights", Duration::from_secs(20), || {
        b.committed() >= 20
    });
    let b_time = start.elapsed();
    let (flood_time, longest) = flood.join().unwrap();
    assert!(
        flood_time < Duration::from_secs(10),
        "delivery never blocks: {flood_time:?}"
    );
    assert_eq!(a.committed(), 0, "A is stalled");
    // Whether A's loop fell behind the flood is a matter of timing; its queues are bounded
    // either way (256 control messages per peer; the drops themselves: `ingress` tests).
    assert!(longest <= 50 * 256, "A's ingress is bounded: {longest}");
    assert!(b_time < Duration::from_secs(20));
    // A recovers once its executor returns.
    a.fakes.exec.set_open(true);
    wait_until("A commits", Duration::from_secs(20), || a.committed() >= 3);
    a.running.shutdown();
    b.running.shutdown();
}

/// Every timer of the driver follows its `Clock` backend: with the local clock stopped an idle
/// chain makes no heartbeat, and once it runs heights commit.
#[test]
fn timers_follow_the_clock_backend() {
    let clock = Arc::new(FakeClock::default());
    let node = spawn_instance(7, Arc::clone(&clock), |_| {});
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(node.committed(), 0, "no local time passed");
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let runner = {
        let (clock, stop) = (Arc::clone(&clock), Arc::clone(&stop));
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                clock.advance(10);
                std::thread::sleep(Duration::from_millis(1));
            }
        })
    };
    wait_until("heartbeats", Duration::from_secs(30), || {
        node.committed() >= 3
    });
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    runner.join().unwrap();
    assert!(clock.now() > 0);
    node.running.shutdown();
}

/// Raw frames: decoded by the driver within the class limit; garbage, the node's own frames
/// and other instances' are refused.
#[test]
fn raw_frames_are_decoded_and_filtered() {
    let node = spawn_instance(5, Arc::new(SystemClock::new()), |fakes| {
        fakes.exec.set_open(false)
    });
    let handle = node.handle();
    let request = |instance| {
        WireMessage::BlockRequest(BlockRequest {
            instance,
            height: 1,
            block_hash: Hash32::ZERO,
        })
        .encode()
        .unwrap()
    };
    let peer = PublicKey::new(vec![9; 32]).unwrap();
    assert!(handle.deliver(&peer, &request(node.instance)));
    assert!(!handle.deliver(&peer, b"garbage"));
    assert!(!handle.deliver(&node.key, &request(node.instance)), "O7");
    assert!(!handle.deliver(&peer, &request(Hash32([0xee; 32]))));
    node.fakes.exec.set_open(true);
    node.running.shutdown();
}

/// O10: a transport limit below the chain parameters' needs refuses the start.
#[test]
fn frame_limit_below_parameters_is_refused() {
    let signer = FakeSigner::from_seed(&[6], None);
    let key = signer.public_key().clone();
    let config = HeightConfig {
        committee: Committee::new(vec![key.clone()]).unwrap(),
        params: params(),
    };
    let blocks = Arc::new(FakeBlocks::default());
    let init = assemble_init(
        &*blocks,
        Hash32([6; 32]),
        0,
        (Hash32([1; 32]), Hash32([2; 32])),
        128,
        vec![(key, RecordState::Absent, false)],
        vec![(1, config.clone()), (2, config.clone())],
        1,
    )
    .unwrap();
    let exec = FakeExecutor::new(Hash32([1; 32]), Hash32([2; 32]), config);
    let driver = Driver::new(
        Arc::new(FakeNet::default()),
        Arc::new(FakeRecords::default()),
        Arc::new(FakeBodies::default()),
        blocks,
        Arc::new(SystemClock::new()),
        exec,
        Arc::new(NoObserver),
    );
    let config = DriverConfig {
        frame_limit: 1024,
        ..DriverConfig::default()
    };
    let refused = driver.spawn(
        config,
        DriverStart {
            local: LocalParams::default(),
            init,
            signers: vec![Box::new(signer)],
            crypto: Arc::new(FakeCrypto::new()),
            attestor: Box::new(NoAttestation),
            verifier: Box::new(NoAttestation),
        },
    );
    assert!(matches!(refused, Err(DriverError::FrameLimit { .. })));
}

/// `Init` from the block store: the tip with its header and `CommitQC`, the last `W + 2`
/// headers, and an error when an entry is missing.
#[test]
fn init_from_the_block_store() {
    let blocks = FakeBlocks::default();
    let (mut parent, mut parent_result) = (Hash32([1; 32]), Hash32([2; 32]));
    let mut headers = Vec::new();
    for h in 1..=5 {
        let b = block(h, parent, parent_result, Vec::new());
        let r = Hash32([u8::try_from(h).unwrap(); 32]);
        blocks.append(&b, &commit_qc(&b, r)).unwrap();
        headers.push(b.header.clone());
        (parent, parent_result) = (hash(&b), r);
    }
    let init = assemble_init(
        &blocks,
        Hash32([5; 32]),
        0,
        (Hash32([1; 32]), Hash32([2; 32])),
        1,
        Vec::new(),
        Vec::new(),
        9,
    )
    .unwrap();
    assert_eq!(init.tip.height, 5);
    assert_eq!(init.tip.block_hash, parent);
    assert_eq!(init.tip.result, parent_result);
    assert!(init.tip.commit_qc.is_some() && init.tip.header.is_some());
    assert_eq!(
        init.recent_headers,
        headers[2..].to_vec(),
        "W + 2 = 3 headers"
    );
    assert_eq!(init.nonce, 9);
    let empty = assemble_init(
        &FakeBlocks::default(),
        Hash32([5; 32]),
        0,
        (Hash32([1; 32]), Hash32([2; 32])),
        1,
        Vec::new(),
        Vec::new(),
        9,
    )
    .unwrap();
    assert_eq!(
        (empty.tip.height, empty.tip.block_hash),
        (0, Hash32([1; 32]))
    );
    assert!(empty.recent_headers.is_empty());
}

/// §12.5: backends that panic — a record write, a block-store append and read — fail like I/O
/// errors and are retried; the instance keeps committing, nothing stops and nothing is
/// reported stopped.
#[test]
fn panicking_backends_are_retried() {
    let node = spawn_instance(8, Arc::new(SystemClock::new()), |fakes| {
        fakes.records.panic_next(2);
        fakes.blocks.panic_appends(2);
        fakes.blocks.panic_reads(1);
    });
    wait_until("10 heights", Duration::from_secs(20), || {
        node.committed() >= 10
    });
    let handle = node.handle();
    assert!(handle.ready(), "{:?}", handle.stopped());
    assert_eq!(handle.stopped(), None);
    assert!(node.fakes.observer.stopped.lock().is_empty());
    assert!(node.fakes.blocks.height() >= 9);
    node.running.shutdown();
    assert!(!handle.ready(), "a shut-down instance is not ready");
}

/// A worker thread that ends stops the instance: the loop stops, the observer is told, the
/// handle is no longer ready and reports the halt as a driver anomaly.
#[test]
fn a_stopped_worker_stops_the_instance() {
    let node = spawn_instance(9, Arc::new(SystemClock::new()), |_| {});
    let handle = node.handle();
    wait_until("a height", Duration::from_secs(20), || {
        node.committed() >= 1
    });
    assert!(handle.ready());
    handle.inputs.send(Input::Exited(Worker::Exec)).unwrap();
    wait_until("the stop", Duration::from_secs(5), || !handle.ready());
    assert_eq!(handle.stopped(), Some(Worker::Exec));
    assert_eq!(handle.halted(), Some(HaltReason::DriverAnomaly));
    assert_eq!(*node.fakes.observer.stopped.lock(), vec![Worker::Exec]);
    let height = node.committed();
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(node.committed(), height, "the instance no longer runs");
    node.running.shutdown();
}

/// A worker thread that ends announces it (however it ends), and a worker that cannot be
/// reached is reported by the dispatch rather than dropping the operation silently.
#[test]
fn worker_exits_and_unreachable_workers_are_detected() {
    let (tx, rx) = mpsc::channel();
    drop(ExitGuard {
        worker: Worker::Persist,
        tx,
    });
    assert!(matches!(rx.try_recv(), Ok(Input::Exited(Worker::Persist))));
    let (persist, _) = mpsc::channel();
    let (exec, exec_rx) = mpsc::channel();
    let (serve, serve_rx) = mpsc::channel();
    drop(serve_rx);
    let workers = Workers {
        net: Arc::new(FakeNet::default()),
        observer: Arc::new(NoObserver),
        persist,
        exec,
        serve,
    };
    let discard = || ExecOp::Discard {
        height: 1,
        keep: Vec::new(),
    };
    assert_eq!(workers.dispatch(vec![Op::Exec(discard())]), Ok(()));
    drop(exec_rx);
    assert_eq!(
        workers.dispatch(vec![Op::Exec(discard())]),
        Err(Worker::Exec)
    );
    let fetch = super::super::serve::ServeRequest::Fetch {
        height: 1,
        block_hash: Hash32::ZERO,
        peers: Vec::new(),
    };
    assert_eq!(workers.dispatch(vec![Op::Serve(fetch)]), Err(Worker::Serve));
}

/// §12.2 serving limits on the real threads: member 0 of a four-member committee, on a slow
/// disk (every block-store read takes 20 ms), is flooded with 10 000 `SyncRequest`s from 50
/// peers. Its own `FetchBody` for a body it lacks under a `CommitQC` still asks the peer at
/// once, it applies the block once the peer answers, and its serving queue stays bounded (a
/// FIFO would have queued 200 s of reads ahead of the fetch).
#[test]
fn serving_flood_does_not_delay_the_nodes_fetch() {
    let vals = FakeValidators::new(4, 11, None);
    let key = vals.key(0);
    let instance = Hash32([5; 32]);
    let crypto: SharedCrypto = Arc::new(vals.crypto.clone());
    let config = HeightConfig {
        committee: vals.committee.clone(),
        params: params(),
    };
    let records = Arc::new(FakeRecords::default());
    records.install_key(&key, true, 1);
    let mut next = 1u128;
    let mut fresh = || {
        next += 1;
        next
    };
    let found = install_records(
        &*records,
        &*crypto,
        &instance,
        &[(key.clone(), false)],
        0,
        false,
        &mut fresh,
    )
    .unwrap();
    let blocks = Arc::new(FakeBlocks::default());
    let genesis = (Hash32([0xa0; 32]), Hash32([0xa1; 32]));
    let init = assemble_init(
        &*blocks,
        instance,
        0,
        genesis,
        128,
        found,
        vec![(1, config.clone()), (2, config.clone())],
        3,
    )
    .unwrap();
    blocks.set_read_delay(20);
    let net = Arc::new(FakeNet::default());
    let exec = FakeExecutor::new(genesis.0, genesis.1, config);
    let driver = Driver::new(
        Arc::clone(&net),
        records,
        Arc::new(FakeBodies::default()),
        Arc::clone(&blocks),
        Arc::new(SystemClock::new()),
        exec,
        Arc::new(NoObserver),
    );
    let running = driver
        .spawn(
            DriverConfig::default(),
            DriverStart {
                local: LocalParams::default(),
                init,
                signers: vec![Box::new(vals.signer(0).clone())],
                crypto,
                attestor: Box::new(NoAttestation),
                verifier: Box::new(NoAttestation),
            },
        )
        .unwrap();
    let handle = running.handle();
    for round in 0..200u64 {
        for peer in 0..50u8 {
            let from = PublicKey::new(vec![peer; 32]).unwrap();
            let request = WireMessage::SyncRequest(SyncRequest {
                instance,
                from_height: round,
                max_count: u16::MAX,
                max_bytes: u32::MAX,
            });
            handle.deliver_message(from, request);
        }
    }
    std::thread::sleep(Duration::from_millis(100));
    let b1 = block(1, genesis.0, genesis.1, vec![4; 64]);
    let ExecOutcome::Valid(r1) = block_exec(&genesis.1, &b1) else {
        panic!("the block executes")
    };
    let qc = vals.qc(
        VoteKind::Commit,
        &instance,
        1,
        0,
        &hash(&b1),
        &r1,
        &[1, 2, 3],
    );
    let status = Status {
        instance,
        height: 2,
        view: 0,
        committed_qc: Some(qc),
        high_pqc: None,
        high_tc: None,
        proposal_hash: None,
        want_proposal: false,
        probe: None,
        echo: None,
    };
    let asked = Instant::now();
    handle.deliver_message(vals.key(1), WireMessage::Status(Box::new(status)));
    wait_until("the body request", Duration::from_secs(3), || {
        net.sent().iter().any(|(to, msg)| {
            *to == vals.key(1)
                && matches!(msg, WireMessage::BlockRequest(r) if r.block_hash == hash(&b1))
        })
    });
    assert!(asked.elapsed() < Duration::from_secs(3));
    let backlog = handle.backlog();
    assert!(backlog.serve <= 2 * 50, "{backlog:?}");
    handle.deliver_message(
        vals.key(1),
        WireMessage::BlockResponse(BlockResponse {
            instance,
            block: b1,
        }),
    );
    wait_until("height 1 applied", Duration::from_secs(5), || {
        handle.status().is_some_and(|s| s.applied_height >= 1)
    });
    assert!(blocks.reads() < 10_000, "most requests were never read");
    let dropped = handle.backlog().serve_dropped;
    assert!(dropped > 0, "the flood was dropped, not queued");
    running.shutdown();
}

/// O10 at run time: every frame is decoded within the transport limit, so a committed rise of
/// `max_block_bytes` (4 MiB to 8 MiB) is followed without a restart; a configuration above the
/// transport limit is reported to the observer.
#[test]
fn frame_limit_follows_committed_configurations() {
    let node = spawn_instance(10, Arc::new(SystemClock::new()), |_| {});
    let handle = node.handle();
    wait_until("a height", Duration::from_secs(20), || {
        node.committed() >= 1
    });
    let raise = |bytes: u32| {
        let mut state = node.fakes.exec.state.lock();
        state.config.params.max_block_bytes = bytes;
    };
    raise(8 << 20);
    let from = node.committed();
    wait_until("the new size in force", Duration::from_secs(20), || {
        node.committed() >= from + 3
    });
    let peer = PublicKey::new(vec![9; 32]).unwrap();
    let response = |bytes: usize| {
        WireMessage::BlockResponse(BlockResponse {
            instance: node.instance,
            block: block(1, Hash32::ZERO, Hash32::ZERO, vec![0; bytes]),
        })
        .encode()
        .unwrap()
    };
    assert!(
        handle.deliver(&peer, &response(6 << 20)),
        "a 6 MiB body decodes under the committed 8 MiB limit"
    );
    assert!(
        !handle.deliver(&peer, &response(17 << 20)),
        "above the transport limit"
    );
    assert!(node.fakes.observer.frame_limits.lock().is_empty());
    raise(32 << 20);
    wait_until("the report", Duration::from_secs(20), || {
        !node.fakes.observer.frame_limits.lock().is_empty()
    });
    let exceeded = node.fakes.observer.frame_limits.lock()[0];
    assert_eq!(exceeded.needed, (32 << 20) + 64 * 1024);
    assert_eq!(exceeded.limit, DriverConfig::default().frame_limit);
    node.running.shutdown();
}
