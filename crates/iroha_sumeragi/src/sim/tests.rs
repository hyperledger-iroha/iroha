//! Scenario tests (§13.3) over many seeds, the multi-node named deterministic test deferred by
//! the state-machine stage (`det_l12`), and an ignored performance report.
//!
//! Seeds: `SUMERAGI_SIM_SEEDS` per scenario (default 5 in debug, 20 in release),
//! `SUMERAGI_SIM_SEED_BASE`, or `SUMERAGI_SIM_SEED` for exactly one seed.

use super::{
    byz::Strategy,
    host::{FakeHost, Host, Start},
    run,
    scenario::{Perf, Profile, Scenario},
    scenarios::{self, Builder},
    world::{World, seeds},
};
use crate::types::Millis;

fn default_seeds() -> u64 {
    if cfg!(debug_assertions) { 5 } else { 20 }
}

/// Run `builder` over the configured seeds; panic with the first failure report.
fn sweep(name: &str, builder: Builder) -> Vec<World> {
    let mut worlds = Vec::new();
    let mut failures = Vec::new();
    for seed in seeds(default_seeds()) {
        match run(builder(seed)) {
            Ok(world) => worlds.push(world),
            Err(report) => failures.push((seed, report)),
        }
    }
    let sum = |f: &dyn Fn(&World) -> u64| worlds.iter().map(f).sum::<u64>();
    let heights = sum(&|w| w.oracle.refs.iter().map(|r| r.len() as u64).sum());
    let view_changes = sum(&|w| {
        w.oracle
            .refs
            .iter()
            .map(|r| r.values().filter(|b| b.view > 0).count() as u64)
            .sum()
    });
    // Peak start level (§9.2) → honest replicas.
    let mut peaks = std::collections::BTreeMap::<u32, usize>::new();
    for world in &worlds {
        for r in world.honest() {
            *peaks
                .entry(world.oracle.reps[r].max_start_level)
                .or_default() += 1;
        }
    }
    eprintln!(
        "{name}: {} seeds passed, {} failed; heights {heights}, commits after a view change \
         {view_changes}, crashes {}, evidence {}, lost {}, ingress drops {}, peak start levels \
         {peaks:?}",
        worlds.len(),
        failures.len(),
        sum(&|w| w.stats.crashes),
        sum(&|w| w.stats.evidence),
        sum(&|w| w.stats.lost),
        sum(&|w| w.replicas.iter().map(|r| r.host.ingress_drops()).sum()),
    );
    if let Some((seed, report)) = failures.first() {
        let seeds: Vec<u64> = failures.iter().map(|(s, _)| *s).collect();
        panic!("{name}: failing seeds {seeds:?}; first (seed {seed}):\n{report}");
    }
    worlds
}

#[test]
fn smoke_all_honest() {
    for n in [1, 4, 7] {
        let world = run(scenarios::smoke(0, n)).unwrap_or_else(|e| panic!("{e}"));
        let heights: Vec<u64> = world.honest().iter().map(|r| world.committed(*r)).collect();
        assert!(heights.iter().all(|h| *h >= 30), "{heights:?}");
    }
}

#[test]
fn sim_is_deterministic() {
    let a = run(scenarios::f09(3)).unwrap_or_else(|e| panic!("{e}"));
    let b = run(scenarios::f09(3)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(a.stats.events, b.stats.events);
    assert_eq!(a.stats.bytes, b.stats.bytes);
    let tips = |w: &World| -> Vec<_> { w.oracle.refs[0].values().map(|b| b.bh).collect() };
    assert_eq!(tips(&a), tips(&b));
}

macro_rules! scenario_test {
    ($test:ident, $name:literal, $builder:path) => {
        #[test]
        fn $test() {
            sweep($name, $builder);
        }
    };
}

scenario_test!(f01_crashed_leaders, "F1", scenarios::f01);
scenario_test!(f02_silent_proxy_tail, "F2", scenarios::f02);
scenario_test!(f03_withholding_proxy_tail, "F3", scenarios::f03);
scenario_test!(f04_silent_or_slow_set_a, "F4", scenarios::f04);
scenario_test!(f05_equivocating_leader, "F5", scenarios::f05);
scenario_test!(f06_vote_withholders, "F6", scenarios::f06);
scenario_test!(f07_adversarial_tc_composition, "F7", scenarios::f07);
scenario_test!(f08_split_brain, "F8", scenarios::f08);
scenario_test!(f09_loss_dup_reorder, "F9", scenarios::f09);
scenario_test!(f10_delay_spikes, "F10", scenarios::f10);
scenario_test!(f11_partitions, "F11", scenarios::f11);
scenario_test!(f12_clock_skew_drift, "F12", scenarios::f12);
scenario_test!(f13_crash_restart_churn, "F13", scenarios::f13);
scenario_test!(f14_whole_cluster_restart, "F14", scenarios::f14);
#[test]
fn f15_slow_executors() {
    // `(variant, peak start level, final start level)` → honest replicas.
    let mut levels = std::collections::BTreeMap::<(u64, u32, u32), usize>::new();
    for world in sweep("F15", scenarios::f15) {
        // §9.2 adaptation: where every executor is slower than `T(0)/2` (variants 1, 4 and 5),
        // the start level rose at every honest replica; in variant 4 executions become fast at
        // 20 s, so it decayed back to 0 by the end. In variant 5 only non-empty blocks are
        // slow (`E ≥ T(1)`), so without the adaptation only `EMPTY` blocks would commit.
        let variant = world.seed % 6;
        if variant == 5 {
            let payload = world.oracle.refs[0]
                .values()
                .filter(|b| b.header.payload_len > 0)
                .count();
            assert!(
                payload >= 5,
                "seed {}: only {payload} non-empty blocks of {} committed",
                world.seed,
                world.oracle.refs[0].len()
            );
        }
        if variant != 1 && variant != 4 && variant != 5 {
            continue;
        }
        for r in world.honest() {
            let Some(core) = world.replicas[r].host.core() else {
                continue;
            };
            let max = world.oracle.reps[r].max_start_level;
            let last = core.status().start_level;
            assert!(
                max >= 2,
                "seed {}: replica {r} start level peaked at {max}",
                world.seed
            );
            if variant == 4 {
                assert_eq!(last, 0, "seed {}: replica {r} did not decay", world.seed);
            }
            *levels.entry((variant, max, last)).or_default() += 1;
        }
    }
    eprintln!("F15 start levels (variant, peak, final) → replicas: {levels:?}");
}
scenario_test!(f16_validator_set_change, "F16", scenarios::f16);
#[test]
fn f17_far_behind_joiner() {
    for world in sweep("F17", scenarios::f17) {
        // The joiner (machine 4, empty store at start) caught up past the pre-built chain.
        let joiner = world.replica_of(4, 0).unwrap_or(0);
        let prebuilt = world.oracle.refs[0].values().filter(|b| b.at == 0).count() as u64;
        assert!(
            world.committed(joiner) > prebuilt,
            "seed {}: joiner at {} of {prebuilt}",
            world.seed,
            world.committed(joiner)
        );
    }
}
scenario_test!(f18_floods, "F18", scenarios::f18);
scenario_test!(f19_poison_payload, "F19", scenarios::f19);
scenario_test!(f20_cross_instance_replay, "F20", scenarios::f20);
scenario_test!(f21_divergent_executor, "F21", scenarios::f21);
scenario_test!(f22_idle_chain, "F22", scenarios::f22);
scenario_test!(f23_non_3f1_committees, "F23", scenarios::f23);
#[test]
fn f24_record_corruption_and_loss() {
    for world in sweep("F24", scenarios::f24) {
        // R2 works under every variant (forged, replayed and relayed echoes, rolled-back key
        // stores, reinstalled keys): every honest node that lost a record anchors again.
        for r in world.honest() {
            let Some(core) = world.replicas[r].host.core() else {
                continue;
            };
            let status = core.status();
            assert!(
                status.halted.is_some() || !status.unanchored,
                "seed {}: replica {r} is still unanchored at the end",
                world.seed
            );
        }
    }
}
scenario_test!(f25_relay_tampering, "F25", scenarios::f25);
scenario_test!(f26_byzantine_responders, "F26", scenarios::f26);
scenario_test!(f27_storage_faults, "F27", scenarios::f27);
scenario_test!(f28_key_rotation, "F28", scenarios::f28);
scenario_test!(f29_cpu_flood, "F29", scenarios::f29);
scenario_test!(f32_cluster_restart_lock_or_cqc, "F32", scenarios::f32);
scenario_test!(f33_hidden_pqc, "F33", scenarios::f33);
scenario_test!(f34_late_entrants, "F34", scenarios::f34);
scenario_test!(f35_local_queue_asymmetry, "F35", scenarios::f35);
#[test]
fn f36_late_leaders() {
    // `(n, peak start level)` → honest replicas.
    let mut levels = std::collections::BTreeMap::<(usize, u32), usize>::new();
    for world in sweep("F36", scenarios::f36) {
        // §9.2: a late but valid proposal or body never raises an honest start level (from
        // the anchor or the proposal's acceptance, every late turn would raise it by one).
        let n = world.instances[0].committee(1).n();
        for r in world.honest() {
            let peak = world.oracle.reps[r].max_start_level;
            assert!(
                peak == 0,
                "seed {}: replica {r} start level peaked at {peak}",
                world.seed
            );
            *levels.entry((n, peak)).or_default() += 1;
        }
    }
    eprintln!("F36 start levels (n, peak) → replicas: {levels:?}");
}

/// §7.4 (Restart), §13.1: every `Init` carries a fresh nonce drawn from the seeded PRNG.
#[test]
fn det_r4_fresh_nonce_per_init() {
    let world = World::new(scenarios::smoke(0, 4));
    let a = world.init_for(0).nonce;
    let b = world.init_for(0).nonce;
    let c = world.init_for(1).nonce;
    assert!(a != b && b != c && a != c, "{a} {b} {c}");
    // Deterministic from the seed.
    let again = World::new(scenarios::smoke(0, 4));
    assert_eq!(again.init_for(0).nonce, a);
}

#[test]
fn f30_max_size_blocks() {
    for world in sweep("F30", scenarios::f30) {
        assert_eq!(
            world.stats.oversize, 0,
            "no honest message exceeds the frame limit"
        );
    }
}

#[test]
fn f31_independent_finality() {
    for world in sweep("F31", scenarios::f31) {
        // While one instance stalls, the other keeps committing.
        for (inst, (from, until)) in [(1usize, (10_000, 30_000)), (0, (40_000, 60_000))] {
            let during = world.oracle.refs[inst]
                .values()
                .filter(|b| (from..until).contains(&b.at))
                .count();
            assert!(
                during >= 10,
                "instance {inst} committed {during} blocks while the other stalled"
            );
        }
    }
}

/// F22 over 100 000 heights (flat memory, heartbeat cadence). Heavy: run with `--release
/// --ignored`.
#[test]
#[ignore = "heavy: 10^5 heights"]
fn f22_idle_chain_100k_heights() {
    let world = run(scenarios::f22_heights(0, 100_000)).unwrap_or_else(|e| panic!("{e}"));
    let height = world.honest().iter().map(|r| world.committed(*r)).min();
    eprintln!(
        "F22 100k: min committed height {height:?}, signatures logged {}",
        world.log.borrow().len()
    );
    assert!(height.is_some_and(|h| h >= 99_000));
}

/// `det_l12_tick_ahead_of_flood` (ML12, deferred from the state-machine stage): under an
/// ingress flood at the per-peer bound, the fake driver delivers `Tick` ahead of every message
/// (O5), so every tick is handled within `ε_tick` of `next_wakeup()`. The same run with FIFO
/// ingress (the ML12 mutation, a driver property) misses that bound, which shows the check
/// detects it.
#[test]
fn det_l12_tick_ahead_of_flood() {
    let build = |fifo: bool| {
        let mut sc = Scenario::base("det_l12", 12, 4);
        sc.duration = 30_000;
        sc.byz = vec![(
            3,
            vec![
                Strategy::CpuFlood,
                Strategy::Flood,
                Strategy::ForgeVotes,
                Strategy::ForgeCommitQc,
            ],
        )];
        // Verification is expensive enough that the flood saturates the CPU.
        for m in 0..4 {
            sc.set_profile(
                m,
                Profile {
                    cpu_us_per_pairing: 8_000,
                    fifo_ingress: fifo,
                    ..Profile::default()
                },
            );
        }
        sc.checks.perf = Perf::P6;
        sc.checks.progress = 3;
        sc
    };
    let world = run(build(false)).unwrap_or_else(|e| panic!("{e}"));
    let late = |w: &World| -> Millis {
        w.honest()
            .iter()
            .map(|r| w.replicas[*r].max_tick_late)
            .max()
            .unwrap_or(0)
    };
    let eps = world.bounds(0, 0).eps_tick;
    assert!(
        late(&world) <= eps,
        "priority lanes: {} ≤ {eps}",
        late(&world)
    );
    let mut fifo = World::new(build(true));
    fifo.checks.perf = Perf::None;
    fifo.checks.liveness = false;
    fifo.run_until(30_000);
    assert!(
        late(&fifo) > eps,
        "FIFO ingress must be detected: tick lateness {} vs ε {eps}",
        late(&fifo)
    );
}

/// Performance report: simulated commit latency and messages per height for n = 4 and n = 22
/// in the normal case, with a silent proxy tail, with one crashed set-A member, and with 10 %
/// loss. Run with `cargo test --release -p iroha_sumeragi sim::tests::report -- --ignored
/// --nocapture`.
#[test]
#[ignore = "report"]
fn report() {
    eprintln!(
        "{:<4} {:<22} {:>8} {:>10} {:>10} {:>10} {:>12}",
        "n", "case", "heights", "gap avg", "gap p99", "latency", "msgs/height"
    );
    for n in [4usize, 22] {
        for case in [
            "normal",
            "silent proxy tail",
            "crashed set-A member",
            "10% loss",
        ] {
            let mut sc = Scenario::base("report", 1, n);
            sc.duration = 120_000;
            sc.checks.progress = 0;
            match case {
                "silent proxy tail" => {
                    sc.byz = vec![(1, vec![Strategy::WithholdQcs(super::byz::Deliver::Nobody)])];
                }
                "crashed set-A member" => {
                    // A member that is in set A of height 5 (and not its leader).
                    let (topo, machine_of) = super::world::preview(&sc, 5);
                    let round = topo.round(0);
                    let index = round.set_a().get(1).copied().unwrap_or(0);
                    let m = machine_of[usize::try_from(index).unwrap_or(0)];
                    sc.script.push((0, super::scenario::Fault::Crash(m)));
                }
                "10% loss" => {
                    sc.net.loss_ppm = 100_000;
                    sc.net.post_heal_loss_ppm = 100_000;
                }
                _ => {}
            }
            let world = run(sc).unwrap_or_else(|e| panic!("{e}"));
            let blocks: Vec<_> = world.oracle.refs[0].values().collect();
            let heights = u64::try_from(blocks.len()).unwrap_or(0);
            let mut gaps: Vec<Millis> = blocks.windows(2).map(|w| w[1].at - w[0].at).collect();
            gaps.sort_unstable();
            let avg = gaps.iter().sum::<Millis>() / u64::try_from(gaps.len().max(1)).unwrap_or(1);
            let p99 = gaps
                .get((gaps.len() * 99).div_ceil(100).saturating_sub(1))
                .copied()
                .unwrap_or(0);
            let latency = world.oracle.commit_latency();
            let msgs = world.stats.packets.iter().sum::<u64>() / heights.max(1);
            eprintln!(
                "{n:<4} {case:<22} {heights:>8} {avg:>8}ms {p99:>8}ms {latency:>8}ms {msgs:>12}"
            );
        }
    }
}

/// F37 (§3.7): flagged blocks commit under forging, withholding and stripping members, and
/// each committed flagged block's `CommitQC` carries at least `q` valid attestations (the O-ATT
/// oracle checks every honest commit; this counts that flagged blocks were committed at all).
#[test]
fn f37_commit_attestation() {
    let worlds = sweep("F37", scenarios::f37);
    let flagged: usize = worlds
        .iter()
        .map(|w| {
            w.oracle.refs[0]
                .values()
                .filter(|b| b.header.attest)
                .count()
        })
        .sum();
    let total: usize = worlds.iter().map(|w| w.oracle.refs[0].len()).sum();
    eprintln!("F37: {flagged} of {total} committed blocks flagged");
    assert!(
        flagged * 10 >= total,
        "a share of the committed blocks is flagged"
    );
}

/// O-ATT counts the signers of a flagged `CommitQC` itself (independently of
/// `verify_attestations`, which MA11 mutates): exactly `q`, one genuine attestation each, and
/// the block's flag; `q + 1` genuine ones, a missing one or a cleared flag fail.
#[test]
fn o_att_requires_exactly_q_attested_signers() {
    use crate::{
        message::{Block, BlockHeader, Qc, VoteKind},
        preimage,
        testing::fake_attestation,
        types::{AggregateSignature, Bitmap, Hash32, SIGNATURE_LEN},
    };
    let world = World::new(scenarios::f37(0));
    let inst = &world.instances[0];
    let committee = inst.committee(1).clone();
    let (n, q) = (committee.n(), committee.q());
    let header = BlockHeader {
        instance: inst.id,
        height: 1,
        origin_view: 0,
        parent_hash: inst.genesis_hash,
        parent_result: inst.genesis_result,
        payload_hash: Hash32([1; 32]),
        payload_len: 1,
        proposer: 0,
        skipped_leaders: Vec::new(),
        attest: true,
    };
    let block = Block {
        header,
        payload: vec![0],
    };
    let (bh, result) = (Hash32([2; 32]), Hash32([3; 32]));
    let statement = preimage::att_preimage(&inst.id, 1, &bh, &result);
    let qc_of = |count: usize| {
        let signers: Vec<u32> = (0..u32::try_from(count).unwrap()).collect();
        Qc {
            kind: VoteKind::Commit,
            instance: inst.id,
            height: 1,
            view: 0,
            block_hash: bh,
            result,
            attest: true,
            signers: Bitmap::from_indices(n, signers.iter().copied()).unwrap(),
            agg_sig: AggregateSignature([0; SIGNATURE_LEN]),
            attestations: (signers.iter())
                .map(|i| fake_attestation(committee.get(*i).unwrap(), 1, &statement))
                .collect(),
        }
    };
    assert_eq!(world.attested(0, &block, &qc_of(q)), Ok(()));
    assert!(world.attested(0, &block, &qc_of(q + 1)).is_err(), "q + 1");
    let mut missing = qc_of(q);
    missing.attestations.pop();
    assert!(world.attested(0, &block, &missing).is_err(), "missing");
    let cleared = Qc {
        attest: false,
        attestations: Vec::new(),
        ..qc_of(q)
    };
    assert!(world.attested(0, &block, &cleared).is_err(), "flag");
}

std::thread_local! {
    /// Inputs handled by [`Wrapped`] hosts on this test thread.
    static WRAPPED_CALLS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// A trivial external node implementation (§13.5): it delegates every call to the fake driver
/// and counts the inputs it handles.
#[derive(Default)]
struct Wrapped(FakeHost);

impl Host for Wrapped {
    fn start(&mut self, start: Start) -> Result<Vec<crate::api::Action>, crate::api::ConfigError> {
        self.0.start(start)
    }

    fn crash(&mut self) {
        self.0.crash();
    }

    fn running(&self) -> bool {
        self.0.running()
    }

    fn receive(
        &mut self,
        from: crate::types::PublicKey,
        msg: crate::message::WireMessage,
        class: crate::message::TrafficClass,
    ) {
        self.0.receive(from, msg, class);
    }

    fn deliver(&mut self, event: crate::api::Event) {
        self.0.deliver(event);
    }

    fn has_input(&self) -> bool {
        self.0.has_input()
    }

    fn next_input(&mut self, now: Millis) -> Option<crate::api::Event> {
        self.0.next_input(now)
    }

    fn handle(&mut self, now: Millis, event: crate::api::Event) -> Vec<crate::api::Action> {
        WRAPPED_CALLS.with(|calls| calls.set(calls.get() + 1));
        self.0.handle(now, event)
    }

    fn next_wakeup(&self) -> Millis {
        self.0.next_wakeup()
    }

    fn persisting(&mut self, write: u64) {
        self.0.persisting(write);
    }

    fn gate(&mut self, effect: crate::api::Action) -> Option<crate::api::Action> {
        self.0.gate(effect)
    }

    fn durable(&mut self, write: u64) -> Vec<crate::api::Action> {
        self.0.durable(write)
    }

    fn core(&self) -> Option<&crate::Core> {
        self.0.core()
    }

    fn held(&self) -> Vec<crate::api::Action> {
        self.0.held()
    }

    fn ingress_drops(&self) -> u64 {
        self.0.ingress_drops()
    }
}

fn wrapped_host(_machine: usize, _instance: usize) -> Box<dyn Host> {
    Box::new(Wrapped::default())
}

/// §13.5 host seam: a scenario runs through an external node implementation (here a wrapper
/// that delegates to the fake driver) with every oracle, and the run is identical to the one on
/// the default host — the seam carries every input and effect, crashes and restarts included
/// (F13 churn).
#[test]
fn host_seam_runs_a_wrapped_host() {
    let cases: [(&str, Builder, u64); 2] = [("F9", scenarios::f09, 3), ("F13", scenarios::f13, 1)];
    for (name, builder, seed) in cases {
        let plain = run(builder(seed)).unwrap_or_else(|e| panic!("{e}"));
        WRAPPED_CALLS.with(|calls| calls.set(0));
        let mut sc = builder(seed);
        sc.host = wrapped_host;
        let wrapped = run(sc).unwrap_or_else(|e| panic!("{e}"));
        let calls = WRAPPED_CALLS.with(std::cell::Cell::get);
        assert_eq!(
            calls, wrapped.stats.events,
            "{name}: every input went through the host"
        );
        assert!(calls > 1_000, "{name}: {calls}");
        assert_eq!(plain.stats.events, wrapped.stats.events, "{name}");
        assert_eq!(plain.stats.bytes, wrapped.stats.bytes, "{name}");
        assert_eq!(plain.stats.crashes, wrapped.stats.crashes, "{name}");
        let chain = |w: &World| -> Vec<_> { w.oracle.refs[0].values().map(|b| b.bh).collect() };
        assert_eq!(
            chain(&plain),
            chain(&wrapped),
            "{name}: the same committed chain"
        );
    }
}
