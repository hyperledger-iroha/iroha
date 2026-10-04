//! Strengthened tests of the §13.4 mutation gate, group 3: ML10 (`on_proposal` step 8 without
//! `StoreBody`).
//!
//! `det_l10_cluster_restart_lock_no_cqc` and F32 restart the whole cluster, proposer included.
//! The proposer stores its own block when it proposes (§6.10 rule 3, a separate `StoreBody`
//! site), so after the restart the lock holders fetch the locked block from it and commit even
//! when no acceptor stored the body: ML10 survives both. Here the proposers of the needed block
//! stay down after the restart (crash faults: the proposer in the deterministic test, up to `f`
//! proposers and re-proposers in the scenario F32s). With at most `f` machines down the protocol
//! must still commit, and only the §7.4 body-durability rule — every body a node accepts is
//! stored before it signs about it, so the block of every `PrepareQC` is durably held by
//! `≥ q − f` honest members — keeps the locked block available.

use std::collections::BTreeSet;

use super::{
    byz::NetRule,
    run,
    scenario::{Fault, Scenario},
    scenarios,
    sweep::{Failures, fold_seeds},
    world::{World, seed_iter},
};
use crate::{
    message::Qc,
    types::{Hash32, Millis},
};

/// The lock every honest replica holds at its (common) round height, if all hold the same one
/// in view 0 of that height and none has committed it.
fn common_view0_lock(w: &World) -> Option<(u64, Qc)> {
    let mut common: Option<(u64, Qc)> = None;
    for r in w.honest() {
        let core = w.replicas[r].host.core()?;
        let status = core.status();
        let lock = core.lock()?;
        if status.view != 0 || lock.height != status.height {
            return None;
        }
        match &common {
            None => common = Some((status.height, lock.clone())),
            Some((height, qc)) if *height == status.height && qc == lock => {}
            Some(_) => return None,
        }
    }
    common
}

/// The machine of the proposer of block `bh` at `height`, from any durable copy of its body.
fn proposer_of(w: &World, height: u64, bh: &Hash32) -> Option<usize> {
    let block = w.replicas.iter().find_map(|rep| rep.bodies.get(bh))?;
    let committee = w.instances[0].committee(height);
    let key = committee.get(block.header().proposer)?;
    w.key_owner.get(key).copied()
}

/// `det_l10_cluster_restart_lock_no_cqc_strong` (ML10; strengthens
/// `det_l10_cluster_restart_lock_no_cqc`): n = 4 on the simulator (O2 write barrier, write
/// latency, every oracle). Commit votes are dropped until every node holds `PrepareQC(B)` of
/// view 0 (the Commit signers durably) and no `CommitQC` forms; the whole cluster crashes, and
/// every node but `B`'s proposer restarts (its copy of `B`, stored when it proposed, is gone
/// with it: a crash fault, `f = 1`). The restarted Commit signers restore the lock; whether the
/// `CommitQC` then forms from their re-sent Commits or after a TC (which carries the lock) from
/// a re-proposal of `B`, committing needs `B`'s body, which only the live nodes' durable body
/// stores can supply (§7.4 body durability, §6.9, §6.10 rule 2). Without the acceptors'
/// `StoreBody` (ML10) nobody alive holds `B`, and the height never commits.
#[test]
fn det_l10_cluster_restart_lock_no_cqc_strong() {
    const N: usize = 4;
    let mut sc = Scenario::base("det_l10_strong", 10, N);
    // The network rule below applies before heal; heal is set when the rule is lifted.
    sc.heal_at = Millis::MAX / 4;
    sc.duration = 300_000;
    let mut w = World::new(sc);
    w.run_until(5_000);
    assert!(w.failure.is_none(), "{:?}", w.failure);
    assert!(
        w.honest().iter().all(|r| w.committed(*r) >= 3),
        "the chain runs before the attack"
    );
    let start = w.now;
    w.adv.rules = vec![NetRule::DropCommitVotes {
        from: start,
        until: Millis::MAX,
    }];
    let (height, lock) = loop {
        assert!(
            w.now < start + 30_000,
            "every node locks one PrepareQC in view 0"
        );
        w.run_until(w.now + 2);
        assert!(w.failure.is_none(), "{:?}", w.failure);
        if let Some(found) = common_view0_lock(&w) {
            break found;
        }
    };
    assert!(
        !w.oracle.refs[0].contains_key(&height),
        "no CommitQC formed at {height}"
    );
    // Let the lock records (written before each Commit left) become durable; still view 0.
    w.run_until(w.now + 20);
    assert_eq!(
        common_view0_lock(&w).map(|(h, _)| h),
        Some(height),
        "still locked in view 0 of {height}"
    );
    // The Commit signers (set A) recorded the lock durably; set B signed nothing at `height`.
    let lockers: Vec<usize> = (0..N)
        .filter(|m| {
            w.replica_of(*m, 0).is_some_and(|r| {
                w.replicas[r]
                    .records
                    .values()
                    .any(|d| d.record.lock.as_ref() == Some(&lock))
            })
        })
        .collect();
    assert!(
        lockers.len() >= w.instances[0].committee(height).q(),
        "the Commit signers durably recorded the lock: {lockers:?}"
    );
    let proposer = proposer_of(&w, height, &lock.block_hash).expect("B's proposer stored B");
    let holders: Vec<usize> = (0..N)
        .filter(|m| {
            w.replica_of(*m, 0)
                .is_some_and(|r| w.replicas[r].bodies.contains_key(&lock.block_hash))
        })
        .collect();

    for m in 0..N {
        w.crash(m);
    }
    w.run_until(w.now + 2_000);
    // The proposer stays down (one crash fault); the network is honest again.
    w.adv.rules.clear();
    w.heal_at = w.now;
    let live: Vec<usize> = (0..N).filter(|m| *m != proposer).collect();
    for &m in &live {
        w.restart(m);
        let r = w.replica_of(m, 0).expect("a replica of instance 0");
        let core = w.replicas[r].host.core().expect("restarted");
        assert_eq!(
            core.status().height,
            height,
            "machine {m} restarts at {height}"
        );
        if lockers.contains(&m) {
            assert_eq!(core.lock(), Some(&lock), "machine {m} restores the lock");
        }
    }
    w.run_until(w.now + 60_000);
    assert!(
        w.failure.is_none(),
        "after the restart without B's proposer (machine {proposer}; durable holders of B \
         before the crash: {holders:?}):\n{}",
        w.failure.as_deref().unwrap_or_default()
    );
    let committed = w.oracle.refs[0].get(&height);
    assert_eq!(
        committed.map(|b| (b.bh, b.result)),
        Some((lock.block_hash, lock.result)),
        "the locked block is committed at {height} (durable holders of B before the crash: \
         {holders:?})"
    );
    for &m in &live {
        let r = w.replica_of(m, 0).expect("a replica of instance 0");
        assert!(
            w.committed(r) > height,
            "machine {m} commits past {height}: at {}",
            w.committed(r)
        );
    }
}

/// Restart every crashed machine except up to `f` proposers of a block the restarted cluster
/// needs — the block of a durable lock or of a durable parent `CommitQC` above the replica's
/// block-store tip. A proposer is the header's `proposer` or a machine whose durable record
/// holds its own (re-)proposal of the block; original proposers go first. Proposers store the
/// block when they propose (§6.10 rule 3), so they are the holders that remain when acceptors
/// skip `StoreBody` (ML10). Any `≤ f` machines may stay down, so the choice never excuses a
/// liveness failure.
fn restart_without_proposers(w: &mut World) {
    let mut needed: BTreeSet<(u64, Hash32)> = BTreeSet::new();
    let mut reproposers: BTreeSet<usize> = BTreeSet::new();
    let mut top = 1;
    for r in w.honest() {
        let rep = &w.replicas[r];
        if rep.inst != 0 {
            continue;
        }
        let tip = rep
            .store
            .last()
            .map_or(0, |(block, _)| block.header().height);
        for durable in rep.records.values() {
            top = top.max(durable.record.height);
            let qcs = [
                durable.record.lock.as_ref(),
                durable.record.parent_commit_qc.as_ref(),
            ];
            for qc in qcs.into_iter().flatten() {
                if qc.height > tip {
                    needed.insert((qc.height, qc.block_hash));
                }
            }
        }
    }
    for r in w.honest() {
        let rep = &w.replicas[r];
        let proposed = rep.records.values().any(|durable| {
            durable
                .record
                .proposal
                .as_ref()
                .is_some_and(|p| needed.contains(&(durable.record.height, p.block_hash)))
        });
        if rep.inst == 0 && proposed {
            reproposers.insert(rep.machine);
        }
    }
    let f = w.instances[0].committee(top).f();
    let originals: BTreeSet<usize> = needed
        .iter()
        .filter_map(|(height, bh)| proposer_of(w, *height, bh))
        .collect();
    let mut down: Vec<usize> = originals
        .iter()
        .chain(reproposers.difference(&originals))
        .copied()
        .filter(|m| !w.machines[*m].byz)
        .collect();
    down.truncate(f);
    w.trace(
        0,
        format!("restart without the proposers {down:?} of {needed:?} (f = {f})"),
    );
    for m in 0..w.machines.len() {
        if !w.machines[m].up && !down.contains(&m) {
            w.restart(m);
        }
    }
}

/// F32s: F32 (whole-cluster restart after a `PrepareQC` was locked everywhere but before any
/// `CommitQC` formed, or after a `CommitQC` formed but before any block store made it durable)
/// where up to `f` proposers of the blocks the cluster needs stay down after the restart
/// ([`restart_without_proposers`]; permanent crash faults, so O-LIVE and the progress check
/// still apply to the rest).
pub fn f32_strong(seed: u64) -> Scenario {
    let mut sc = scenarios::f32(seed);
    sc.name = "F32s".to_owned();
    for (_, fault) in &mut sc.script {
        if matches!(fault, Fault::RestartAll) {
            *fault = Fault::Custom(Box::new(restart_without_proposers));
        }
    }
    sc
}

/// Run `builder` over the configured seeds (`SUMERAGI_SIM_SEEDS`, default 5 in debug, 20 in
/// release); panic with the first failure report.
fn sweep(name: &str, builder: fn(u64) -> Scenario) {
    let default = if cfg!(debug_assertions) { 5 } else { 20 };
    let mut failures = Failures::default();
    fold_seeds(
        seed_iter(default),
        |seed| run(builder(seed)).map(drop),
        |seed, result| {
            failures.observe(seed, result);
        },
    );
    failures.finish(name);
}

#[test]
fn f32s_cluster_restart_without_proposers() {
    sweep("F32s", f32_strong);
}
