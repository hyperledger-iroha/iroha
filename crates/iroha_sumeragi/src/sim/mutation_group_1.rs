//! Mutation group 1 (§13.4): the fake-driver rules SR34 (O3: apply compares the commitment with
//! `commit_qc.result`, mutation MS34) and LR14 (the builder quarantines on `PayloadRejected`,
//! mutation ML14), tested on the simulator's fake driver.
//!
//! The state-machine harness of `machine::tests` has no apply comparison and no payload builder,
//! so the named tests there (`det_s34_apply_divergence_halts`, `det_l14_poison_quarantined`)
//! only check the core's half of each rule. The `_strong` tests here run the unmodified cores
//! under the simulator's driver, whose apply and builder carry the mutations:
//!
//! - [`det_s34_apply_divergence_halts_strong`]: a node whose executor diverges deterministically
//!   (its cached and its re-executed commitments differ from `commit_qc.result`) reports
//!   `ApplyDiverged` for the first non-empty committed block and halts without applying it;
//!   with a reference executor the same run applies that block.
//! - [`det_l14_poison_quarantined_strong`]: one poison transaction is offered to every builder;
//!   each builder proposes it at most once, quarantines exactly it (never the honest
//!   transactions it shared a block with), and every other transaction commits.
//! - [`f21_divergent_executor_halts`]: the F21 scenario with the O3 halt oracle that F21 lacks
//!   (the simulator's apply records `qc.result` whatever the local result, so without this
//!   check a divergent node that silently applies is indistinguishable from a halted one).

use std::collections::{BTreeMap, BTreeSet};

use super::{
    driver::{decode_txs, encode_tx},
    run,
    scenario::{Fault, Profile, Scenario},
    scenarios,
    world::{World, seeds},
};
use crate::{
    api::{Event, HaltReason},
    types::{Hash32, Millis},
};

fn default_seeds() -> u64 {
    if cfg!(debug_assertions) { 5 } else { 20 }
}

// ---- MS34: apply compares the commitment (O3, SR34) ---------------------------------------

/// The machine whose executor diverges in the MS34 test (any member would do: whatever its
/// roles, a divergent node must halt).
const X: usize = 2;

/// Four honest machines with the default workload; `X`'s executor diverges from the start when
/// `divergent`, and `X` is down from `t = 1` until `down_until` when given (so that it applies
/// the certified blocks after a sync, without any cached post-state).
fn s34_scenario(seed: u64, divergent: bool, down_until: Option<Millis>) -> Scenario {
    let mut sc = Scenario::base("det_s34", seed, 4);
    sc.duration = 20_000;
    if divergent {
        sc.set_profile(
            X,
            Profile {
                divergent: true,
                ..Profile::default()
            },
        );
        sc.checks.may_halt = vec![X];
        sc.checks.may_fault = vec![X];
    }
    if let Some(until) = down_until {
        sc.script.push((1, Fault::Crash(X)));
        sc.script.push((until, Fault::Restart(X)));
        // One block per sync response: the core handles `ApplyDiverged` before it commits the
        // next synced height (the fake driver treats a `CommitBlock` above a diverged height as
        // a non-extending commit).
        sc.local.sync_batch = 1;
    }
    sc
}

/// The first height of the reference chain of instance `inst` whose block is non-empty (the
/// fake divergent executor diverges only on non-empty payloads).
fn first_nonempty(world: &World, inst: usize) -> Option<u64> {
    world.oracle.refs[inst]
        .values()
        .find(|b| b.header.payload_len > 0)
        .map(|b| b.header.height)
}

/// SR34 / O3 (§12.3, §12.5 item 3): the driver compares the commitment of the block it applies
/// (its cached post-state, else a re-execution) with `commit_qc.result`; a divergent executor
/// reports `ApplyDiverged` instead of applying, and the core halts that instance.
#[test]
fn det_s34_apply_divergence_halts_strong() {
    s34_check(34);
}

fn s34_check(seed: u64) {
    // Control: with the reference executor, X applies the first non-empty block and more.
    let world = run(s34_scenario(seed, false, None)).unwrap_or_else(|e| panic!("{e}"));
    let h1 = first_nonempty(&world, 0).expect("the workload fills a block");
    let x = world.replica_of(X, 0).expect("X is a member");
    assert_eq!(world.replicas[x].halted, None);
    assert!(
        world.replicas[x].applied.0 >= h1 + 3,
        "control: X applied {} (first non-empty height {h1})",
        world.replicas[x].applied.0
    );
    // (a) X executes every proposal itself, so apply first finds its own (divergent or
    // missing) post-state, then re-executes; (b) X was down while the chain moved on, so
    // apply after the sync re-executes without any cached post-state.
    for down_until in [None, Some(8_000)] {
        let world = run(s34_scenario(seed, true, down_until)).unwrap_or_else(|e| panic!("{e}"));
        let h1 = first_nonempty(&world, 0).expect("the workload fills a block");
        let certified = world.oracle.refs[0][&h1].clone();
        let rep = &world.replicas[x];
        let halt = Some(HaltReason::ApplyDiverged { height: h1 });
        assert_eq!(
            rep.halted, halt,
            "{down_until:?}: X must halt at the first non-empty height {h1}, not apply it"
        );
        let core = rep.host.core().expect("X is running");
        assert_eq!(core.status().halted, halt);
        assert_eq!(
            rep.applied.0,
            h1 - 1,
            "{down_until:?}: X applied past the diverging block"
        );
        if let Some((_, own)) = rep.exec.cache.get(&certified.bh) {
            assert_ne!(*own, certified.result, "the setup diverges");
        }
        // The divergence stays local: every other node keeps committing and never halts.
        for r in world.honest().into_iter().filter(|r| *r != x) {
            assert_eq!(world.replicas[r].halted, None, "replica {r}");
            assert!(
                world.replicas[r].applied.0 >= h1 + 5,
                "replica {r} applied {}",
                world.replicas[r].applied.0
            );
        }
    }
}

/// O3 halt oracle of F21 (§12.5 item 3): every honest machine whose executor diverged ends the
/// run halted by `ApplyDiverged` at a non-empty certified height it never applied.
fn divergent_nodes_halted(world: &World) -> Result<(), String> {
    for (m, machine) in world.machines.iter().enumerate() {
        if !machine.profile.divergent || machine.byz || !machine.up {
            continue;
        }
        for r in machine.replicas.iter().flatten().copied() {
            let rep = &world.replicas[r];
            let Some(HaltReason::ApplyDiverged { height }) = rep.halted else {
                return Err(world.report(&format!(
                    "O3: divergent machine {m} (replica {r}) applied up to {} without \
                     ApplyDiverged (halted {:?})",
                    rep.applied.0, rep.halted
                )));
            };
            let nonempty = world.oracle.refs[rep.inst]
                .get(&height)
                .is_some_and(|b| b.header.payload_len > 0);
            if !nonempty || rep.applied.0 >= height {
                return Err(world.report(&format!(
                    "O3: divergent machine {m} (replica {r}) halted at {height} (non-empty \
                     {nonempty}) with applied height {}",
                    rep.applied.0
                )));
            }
        }
    }
    Ok(())
}

/// F21 with the O3 halt oracle: a nondeterministic executor at one honest node; only that node
/// may halt, and it must (MS34).
#[test]
fn f21_divergent_executor_halts() {
    let mut passed = 0usize;
    let mut failures = Vec::new();
    for seed in seeds(default_seeds()) {
        let mut world = World::new(scenarios::f21(seed));
        match world.run().and_then(|()| divergent_nodes_halted(&world)) {
            Ok(()) => passed += 1,
            Err(report) => failures.push((seed, report)),
        }
    }
    eprintln!("F21+O3: {passed} seeds passed, {} failed", failures.len());
    if let Some((seed, report)) = failures.first() {
        let seeds: Vec<u64> = failures.iter().map(|(s, _)| *s).collect();
        panic!("F21+O3: failing seeds {seeds:?}; first (seed {seed}):\n{report}");
    }
}

// ---- ML14: the builder quarantines on PayloadRejected (§4.2, §12.2) -----------------------

/// Id of the injected poison transaction (the workload numbers its transactions from 1).
const POISON: u64 = 0;
/// When the poison transaction is offered.
const POISON_AT: Millis = 3_000;

/// Offer the poison transaction to every running builder of instance 0, as the workload
/// offers a transaction (including the owed `PayloadReady`), and log it as poison.
fn inject_poison(w: &mut World) {
    let pad = 32;
    w.txs[0].insert(POISON, (w.now, true, None));
    for r in 0..w.replicas.len() {
        let m = w.replicas[r].machine;
        if w.replicas[r].inst != 0 || !w.machines[m].up {
            continue;
        }
        let rep = &mut w.replicas[r];
        rep.txs.insert(POISON, encode_tx(POISON, true, pad));
        if let Some(req) = rep.pending_ready.take() {
            rep.host.deliver(Event::PayloadReady { req });
        }
        w.refresh(r);
    }
}

/// Whether a payload carries the poison transaction.
fn has_poison(payload: &[u8]) -> bool {
    decode_txs(payload).iter().any(|(id, _)| *id == POISON)
}

/// LR14 / O-TXP (§4.2, §12.2): after `PayloadRejected` the builder quarantines exactly the
/// transaction that makes a block `Invalid` on its own; the poison is proposed at most once by
/// each builder, and every other transaction (also those that shared a block with it) commits.
#[test]
fn det_l14_poison_quarantined_strong() {
    l14_check(14);
}

fn l14_check(seed: u64) {
    let mut sc = Scenario::base("det_l14", seed, 4);
    sc.duration = 40_000;
    sc.checks.txp = true;
    sc.script
        .push((POISON_AT, Fault::Custom(Box::new(inject_poison))));
    let mut world = World::new(sc);
    // Every block that carried the poison, by hash: (proposer machine, height, honest
    // transactions in it). Bodies stay in the body store until their height is applied, which
    // takes at least one more view, so sampling every 10 ms sees each of them.
    let mut poison_blocks: BTreeMap<Hash32, (usize, u64, Vec<u64>)> = BTreeMap::new();
    while world.failure.is_none() && world.now < world.duration {
        let next = world.now + 10;
        world.run_until(next);
        for rep in &world.replicas {
            for (bh, block) in &rep.bodies {
                if poison_blocks.contains_key(bh) || !has_poison(&block.payload().as_slice()) {
                    continue;
                }
                let h = block.header().height;
                let key = world.instances[rep.inst]
                    .committee(h)
                    .get(block.header().proposer)
                    .expect("the proposer is a member");
                let proposer = world.key_owner[key];
                let honest: Vec<u64> = decode_txs(&block.payload().as_slice())
                    .into_iter()
                    .filter(|(id, _)| *id != POISON)
                    .map(|(id, _)| id)
                    .collect();
                poison_blocks.insert(*bh, (proposer, h, honest));
            }
        }
    }
    if let Some(violation) = &world.failure {
        panic!("{}", world.report(violation));
    }
    // The end-of-run oracles (O-TXP among them), reported after the builder checks below.
    let finished = world.run();
    assert!(
        !poison_blocks.is_empty(),
        "the poison transaction was never proposed"
    );
    // Proposed once, then never again by that builder.
    let mut per_builder: BTreeMap<usize, Vec<u64>> = BTreeMap::new();
    for (proposer, h, _) in poison_blocks.values() {
        per_builder.entry(*proposer).or_default().push(*h);
    }
    for (m, heights) in &per_builder {
        assert_eq!(
            heights.len(),
            1,
            "machine {m} proposed the poison transaction again (heights {heights:?})"
        );
    }
    // The builder of every poison block quarantined exactly the poison transaction; no builder
    // quarantined anything else.
    let only_poison: BTreeSet<u64> = [POISON].into_iter().collect();
    for (r, rep) in world.replicas.iter().enumerate() {
        assert!(
            rep.quarantine.is_subset(&only_poison),
            "replica {r} quarantined honest transactions: {:?}",
            rep.quarantine
        );
        if per_builder.contains_key(&rep.machine) {
            assert_eq!(rep.quarantine, only_poison, "replica {r}");
            assert!(!rep.txs.contains_key(&POISON), "replica {r}");
        }
    }
    // The poison never commits; the honest transactions it was wrapped with do.
    let log = &world.txs[0];
    assert_eq!(log[&POISON].2, None, "the poison transaction committed");
    for (_, h, honest) in poison_blocks.values() {
        for id in honest {
            assert!(
                log.get(id).is_some_and(|(_, _, at)| at.is_some()),
                "transaction {id} of the poison block at height {h} never committed"
            );
        }
    }
    let after: Vec<bool> = log
        .iter()
        .filter(|(id, (at, _, _))| **id != POISON && *at > POISON_AT)
        .filter(|(_, (at, _, _))| *at + 10_000 <= world.duration)
        .map(|(_, (_, _, committed))| committed.is_some())
        .collect();
    assert!(after.len() >= 50, "{} transactions", after.len());
    assert!(
        after.iter().all(|c| *c),
        "{} of {} later transactions committed",
        after.iter().filter(|c| **c).count(),
        after.len()
    );
    finished.unwrap_or_else(|e| panic!("{e}"));
}
