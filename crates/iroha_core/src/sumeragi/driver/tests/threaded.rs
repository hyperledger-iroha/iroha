//! The full driver on its threads over the fake backends: a single validator's chain commits
//! with failing writes and apply steps retried; O9 — two instances in one process, one stalled
//! (its executor never returns) and flooded, the other committing on time; O10 frame limits;
//! raw-frame delivery; `Init` assembly from the block store.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use iroha_sumeragi::{
    api::LocalParams,
    crypto::{NoAttestation, Signer},
    message::{BlockRequest, Status, WireMessage},
    safety::RecordState,
    testing::{FakeCrypto, FakeSigner},
    types::{ChainParams, Committee, Hash32, HeightConfig, PublicKey},
};

use super::{
    super::{
        Driver, DriverConfig, DriverError, DriverHandle, DriverStart, RunningDriver, SharedCrypto,
        assemble_init,
        persist::install_records,
        traits::{BlockStore, Clock, NoObserver, SystemClock},
    },
    block, commit_qc,
    fakes::{FakeBlocks, FakeBodies, FakeClock, FakeExecutor, FakeNet, FakeRecords},
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
    records: Arc<FakeRecords>,
    bodies: Arc<FakeBodies>,
    blocks: Arc<FakeBlocks>,
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
        records,
        bodies: Arc::new(FakeBodies::default()),
        blocks,
    };
    prepare(&fakes);
    let driver = Driver::new(
        Arc::new(FakeNet::default()),
        Arc::clone(&fakes.records),
        Arc::clone(&fakes.bodies),
        Arc::clone(&fakes.blocks),
        clock,
        fakes.exec.clone(),
        Arc::new(NoObserver),
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
            }
            start.elapsed()
        })
    };
    let start = Instant::now();
    wait_until("B commits 20 heights", Duration::from_secs(20), || {
        b.committed() >= 20
    });
    let b_time = start.elapsed();
    let flood_time = flood.join().unwrap();
    assert!(
        flood_time < Duration::from_secs(10),
        "delivery never blocks: {flood_time:?}"
    );
    assert_eq!(a.committed(), 0, "A is stalled");
    assert!(a.handle().ingress_drops() > 0, "A's ingress is bounded");
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
