//! Scenario tests (§13.3) over many seeds, the multi-node named deterministic test deferred by
//! the state-machine stage (`det_l12`), and an ignored performance report.
//!
//! Seeds: `SUMERAGI_SIM_SEEDS` per scenario (default 5 in debug, 20 in release),
//! `SUMERAGI_SIM_SEED_BASE`, or `SUMERAGI_SIM_SEED` for exactly one seed.

use super::{
    byz::Strategy,
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
        sum(&|w| w.replicas.iter().map(|r| r.lanes.dropped).sum()),
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
            let Some(core) = world.replicas[r].core.as_ref() else {
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
            let Some(core) = world.replicas[r].core.as_ref() else {
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
