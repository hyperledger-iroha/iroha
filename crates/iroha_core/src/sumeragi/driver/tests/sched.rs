//! The execution scheduler with the fake executor, one operation at a time through the
//! executor thread's `run_exec` (§12.3 O3, O4).

use std::collections::BTreeMap;

use iroha_sumeragi::{
    api::{Event, ExecOutcome},
    message::{Block, Qc},
    sim::driver::{encode_tx, reference_exec},
    types::{ChainParams, Committee, Hash32, HeightConfig, Millis, PublicKey},
};

use super::{
    super::{
        exec::{ExecDone, ExecOp, ExecSched},
        persist::Backoff,
        run_exec,
        traits::{BlockStore, Executor},
    },
    block, commit_qc,
    fakes::{FakeBlocks, FakeExecutor},
    hash,
};

const G: Hash32 = Hash32([0xa0; 32]);
const RG: Hash32 = Hash32([0xa1; 32]);

fn config() -> HeightConfig {
    HeightConfig {
        committee: Committee::new(vec![PublicKey::new(vec![1; 32]).unwrap()]).unwrap(),
        params: ChainParams::default(),
    }
}

fn result_of(parent_result: &Hash32, block: &Block) -> Hash32 {
    match reference_exec(parent_result, &block.payload) {
        ExecOutcome::Valid(r) => r,
        other => panic!("{other:?}"),
    }
}

/// A block of `height` on `(parent, parent_result)` and its result.
fn child(height: u64, parent: (Hash32, Hash32), tag: u64) -> (Block, Hash32, Hash32) {
    let b = block(height, parent.0, parent.1, encode_tx(tag, false, 4));
    let r = result_of(&parent.1, &b);
    let bh = hash(&b);
    (b, bh, r)
}

struct Rig {
    sched: ExecSched,
    exec: FakeExecutor,
    blocks: FakeBlocks,
    events: Vec<Event>,
    now: Millis,
}

impl Rig {
    fn new() -> Self {
        Self {
            sched: ExecSched::new(0, Backoff::default()),
            exec: FakeExecutor::new(G, RG, config()),
            blocks: FakeBlocks::default(),
            events: Vec::new(),
            now: 0,
        }
    }

    /// Start the next operation, if any.
    fn start(&mut self) -> Option<ExecOp> {
        self.sched.next(self.now)
    }

    /// Perform `op` on the fake executor and hand the answer back.
    fn finish(&mut self, op: ExecOp) -> Option<u64> {
        let done = run_exec(&mut self.exec, &self.blocks, op);
        let applied = self.sched.done(self.now, done);
        self.events.extend(self.sched.take_events());
        applied
    }

    /// Run operations until the scheduler is idle.
    fn drain(&mut self) {
        while let Some(op) = self.start() {
            self.finish(op);
        }
        self.events.extend(self.sched.take_events());
    }

    /// Answers per request so far.
    fn answers(&self) -> BTreeMap<u64, Vec<ExecOutcome>> {
        let mut out: BTreeMap<u64, Vec<ExecOutcome>> = BTreeMap::new();
        for event in &self.events {
            if let Event::Executed { req, outcome, .. } = event {
                out.entry(*req).or_default().push(outcome.clone());
            }
        }
        out
    }
}

/// O4: the most recent `Execute` runs first; one whose parent post-state is not held is parked
/// (never `Failed`) and runs once the parent executed.
#[test]
fn most_recent_first_and_parking() {
    let mut rig = Rig::new();
    let (b1, bh1, r1) = child(1, (G, RG), 1);
    let (b2, bh2, r2) = child(2, (bh1, r1), 2);
    rig.sched.execute(1, bh1, b1);
    rig.sched.execute(2, bh2, b2);
    let first = rig.start().unwrap();
    assert!(matches!(&first, ExecOp::Execute { block_hash, .. } if *block_hash == bh2));
    rig.finish(first);
    assert!(rig.events.is_empty(), "parked, not answered");
    assert_eq!(rig.sched.outstanding(), 2);
    rig.drain();
    let answers = rig.answers();
    assert_eq!(answers[&1], vec![ExecOutcome::Valid(r1)]);
    assert_eq!(answers[&2], vec![ExecOutcome::Valid(r2)]);
    assert_eq!(rig.sched.outstanding(), 0);
}

/// O4: a discard answers waiting jobs left out `Cancelled` at once and a running one when it
/// finishes; the kept job still runs; the executor drops the post-states.
#[test]
fn discard_cancels_waiting_and_running() {
    let mut rig = Rig::new();
    let (a, bha, ra) = child(1, (G, RG), 1);
    let (b, bhb, _) = child(1, (G, RG), 2);
    let (c, bhc, _) = child(1, (G, RG), 3);
    rig.sched.execute(1, bha, a);
    rig.sched.execute(2, bhb, b);
    let running = rig.start().unwrap();
    rig.sched.execute(3, bhc, c);
    rig.sched.discard(1, vec![bha]);
    rig.events.extend(rig.sched.take_events());
    assert_eq!(
        rig.answers()[&3],
        vec![ExecOutcome::Cancelled],
        "queued job"
    );
    rig.finish(running);
    assert_eq!(
        rig.answers()[&2],
        vec![ExecOutcome::Cancelled],
        "running job"
    );
    let next = rig.start().unwrap();
    assert!(
        matches!(next, ExecOp::Discard { height: 1, .. }),
        "{next:?}"
    );
    rig.finish(next);
    assert!(!rig.exec.state.lock().cache.contains_key(&bhb));
    rig.drain();
    assert_eq!(rig.answers()[&1], vec![ExecOutcome::Valid(ra)]);
    assert!(rig.answers().values().all(|a| a.len() == 1));
}

/// O3: `CommitBlock` arrives while that block executes: the execution finishes, its post-state
/// is reused — one execution, not two — and `BlockApplied` carries the header.
#[test]
fn commit_during_execution_executes_once() {
    let mut rig = Rig::new();
    let (b1, bh1, r1) = child(1, (G, RG), 1);
    rig.sched.execute(1, bh1, b1.clone());
    let running = rig.start().unwrap();
    rig.sched.commit(b1.clone(), commit_qc(&b1, r1));
    assert!(rig.start().is_none(), "one operation at a time");
    assert_eq!(rig.finish(running), None);
    rig.drain();
    assert_eq!(rig.exec.executions(&bh1), 1);
    assert_eq!(rig.answers()[&1], vec![ExecOutcome::Valid(r1)]);
    assert!(rig.events.iter().any(|e| matches!(
        e,
        Event::BlockApplied { height: 1, block_hash, header, .. }
            if *block_hash == bh1 && **header == b1.header
    )));
    assert_eq!(rig.blocks.height(), 1, "durable in the block store first");
    assert_eq!(rig.sched.applied(), 1);
}

/// O3: an `Execute` of the committed block still queued is not started again: the apply
/// executes the block once and answers it.
#[test]
fn commit_answers_a_queued_execution() {
    let mut rig = Rig::new();
    let (b1, bh1, r1) = child(1, (G, RG), 1);
    rig.sched.execute(1, bh1, b1.clone());
    rig.sched.commit(b1.clone(), commit_qc(&b1, r1));
    let first = rig.start().unwrap();
    assert!(matches!(first, ExecOp::Prepare(_)), "{first:?}");
    rig.finish(first);
    rig.drain();
    assert_eq!(rig.exec.executions(&bh1), 1);
    assert_eq!(rig.answers()[&1], vec![ExecOutcome::Valid(r1)]);
}

/// O3: a missing post-state is recomputed (no divergence); a local commitment other than the
/// certified one is `ApplyDiverged` and apply stops.
#[test]
fn missing_cache_recomputes_and_mismatch_diverges() {
    let mut rig = Rig::new();
    let (b1, bh1, r1) = child(1, (G, RG), 1);
    rig.sched.commit(b1.clone(), commit_qc(&b1, r1));
    rig.drain();
    assert_eq!(rig.sched.applied(), 1, "re-executed, not diverged");
    let (b2, bh2, r2) = child(2, (bh1, r1), 2);
    rig.sched
        .commit(b2.clone(), commit_qc(&b2, Hash32([7; 32])));
    rig.drain();
    assert!(rig.events.iter().any(|e| matches!(
        e,
        Event::ApplyDiverged { height: 2, block_hash, local_result }
            if *block_hash == bh2 && *local_result == r2
    )));
    let (b3, _, _) = child(3, (bh2, r2), 3);
    rig.sched
        .commit(b3.clone(), commit_qc(&b3, Hash32([8; 32])));
    rig.drain();
    assert_eq!(rig.sched.applied(), 1);
    assert_eq!(
        rig.blocks.height(),
        1,
        "nothing appended after a divergence"
    );
}

/// Local apply failures (executor and block store) are retried with backoff and never
/// skipped.
#[test]
fn apply_failures_are_retried() {
    let mut rig = Rig::new();
    let (b1, _, r1) = child(1, (G, RG), 1);
    rig.exec.state.lock().fail_apply = 1;
    rig.blocks.fail_next(1);
    rig.sched.commit(b1.clone(), commit_qc(&b1, r1));
    rig.drain();
    assert_eq!(rig.sched.applied(), 0);
    assert_eq!(rig.sched.wakeup(), 10);
    rig.now = 10;
    rig.drain();
    assert_eq!(rig.sched.applied(), 0, "the append failed next");
    assert_eq!(
        rig.sched.wakeup(),
        20,
        "the backoff restarts after a success"
    );
    rig.now = 20;
    rig.drain();
    assert_eq!(rig.sched.applied(), 1);
    assert_eq!(rig.sched.wakeup(), Millis::MAX);
}

/// Building waits for the parent's apply; `PayloadReady` follows an `EMPTY` build at most
/// once, and a newer request supersedes an older one.
#[test]
fn build_after_parent_apply_and_payload_ready() {
    let mut rig = Rig::new();
    let (b1, _, r1) = child(1, (G, RG), 1);
    rig.sched.build(4, 2, 0, 1024, 100);
    rig.sched.build(5, 2, 1, 1024, 100);
    assert!(rig.start().is_none(), "height 1 is not applied yet");
    rig.sched.commit(b1.clone(), commit_qc(&b1, r1));
    rig.drain();
    let built: Vec<u64> = rig
        .events
        .iter()
        .filter_map(|e| match e {
            Event::PayloadBuilt { req, payload, .. } if payload.is_empty() => Some(*req),
            _ => None,
        })
        .collect();
    assert_eq!(built, vec![5]);
    rig.sched.transactions_available();
    rig.sched.transactions_available();
    let ready = rig.sched.take_events();
    assert_eq!(ready, vec![Event::PayloadReady { req: 5 }]);
    rig.exec.add_tx(9);
    rig.sched.build(6, 2, 2, 1024, 100);
    rig.drain();
    assert!(
        rig.events.iter().any(
            |e| matches!(e, Event::PayloadBuilt { req: 6, payload, .. } if !payload.is_empty())
        )
    );
    rig.sched.transactions_available();
    assert!(rig.sched.take_events().is_empty(), "no EMPTY build pending");
}

/// A transaction arriving while a build runs (its queue read may predate it) is not lost: an
/// `EMPTY` answer is followed by `PayloadReady` at once (E4).
#[test]
fn arrival_during_a_build_follows_an_empty_answer() {
    let mut rig = Rig::new();
    let (b1, _, r1) = child(1, (G, RG), 1);
    rig.sched.commit(b1.clone(), commit_qc(&b1, r1));
    rig.drain();
    rig.events.clear();
    rig.sched.build(7, 2, 0, 1024, 100);
    let op = rig.start().expect("the build starts");
    rig.sched.transactions_available();
    assert!(rig.sched.take_events().is_empty(), "nothing is pending yet");
    rig.finish(op);
    assert_eq!(
        rig.events
            .iter()
            .filter(|e| matches!(e, Event::PayloadBuilt { .. } | Event::PayloadReady { .. }))
            .cloned()
            .collect::<Vec<_>>(),
        vec![
            Event::PayloadBuilt {
                req: 7,
                payload: Vec::new(),
                attest: false,
            },
            Event::PayloadReady { req: 7 },
        ]
    );
    rig.sched.transactions_available();
    assert!(rig.sched.take_events().is_empty(), "at most once per request");
    // Without an arrival the answer waits for one, as before.
    rig.events.clear();
    rig.sched.build(8, 2, 1, 1024, 100);
    rig.drain();
    assert!(!rig.events.iter().any(|e| matches!(e, Event::PayloadReady { .. })));
    rig.sched.transactions_available();
    assert_eq!(rig.sched.take_events(), vec![Event::PayloadReady { req: 8 }]);
}

/// Requests of heights applied meanwhile are answered on the apply (never left waiting), and
/// new ones at once.
#[test]
fn applied_heights_answer_waiting_requests() {
    let mut rig = Rig::new();
    let (b1, bh1, r1) = child(1, (G, RG), 1);
    let (x1, bhx, _) = child(1, (G, RG), 99);
    rig.sched.execute(7, bhx, x1.clone());
    rig.sched.commit(b1.clone(), commit_qc(&b1, r1));
    rig.sched.reject(1, 0, bhx);
    rig.drain();
    let answers = rig.answers();
    assert_eq!(answers[&7], vec![ExecOutcome::Cancelled]);
    rig.sched.execute(8, bh1, b1);
    rig.events.extend(rig.sched.take_events());
    assert_eq!(rig.answers()[&8], vec![ExecOutcome::Cancelled]);
    assert_eq!(rig.exec.state.lock().rejected, vec![bhx]);
    assert_eq!(rig.sched.outstanding(), 0);
}

/// O4 under random interleavings: every `Execute` is answered exactly once, whatever the
/// discards, commits and the order of operations.
#[test]
fn every_execute_is_answered_exactly_once() {
    for seed in 0..200u64 {
        let mut rng = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let mut next = |bound: u64| {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng % bound
        };
        let mut rig = Rig::new();
        // The canonical chain c1..c4 and a fork block per height.
        let mut chain = vec![(G, RG)];
        let mut blocks: Vec<(Block, Hash32, Hash32)> = Vec::new();
        let mut forks: Vec<(Block, Hash32)> = Vec::new();
        for h in 1..=4u64 {
            let parent = chain[usize::try_from(h - 1).unwrap()];
            let (b, bh, r) = child(h, parent, h);
            let (x, bhx, _) = child(h, parent, 100 + h);
            chain.push((bh, r));
            blocks.push((b, bh, r));
            forks.push((x, bhx));
        }
        let mut requests = 0u64;
        let mut committed = 0usize;
        let mut running: Option<ExecOp> = None;
        for _ in 0..60 {
            match next(6) {
                0 | 1 => {
                    let i = usize::try_from(next(4)).unwrap();
                    requests += 1;
                    let (b, bh) = if next(2) == 0 {
                        (blocks[i].0.clone(), blocks[i].1)
                    } else {
                        (forks[i].0.clone(), forks[i].1)
                    };
                    rig.sched.execute(requests, bh, b);
                }
                2 => {
                    let h = next(4) + 1;
                    let i = usize::try_from(h - 1).unwrap();
                    let keep = if next(2) == 0 {
                        vec![blocks[i].1]
                    } else {
                        Vec::new()
                    };
                    rig.sched.discard(h, keep);
                }
                3 if committed < 4 => {
                    let (b, _, r) = &blocks[committed];
                    rig.sched.commit(b.clone(), commit_qc(b, *r));
                    committed += 1;
                }
                _ => {
                    if let Some(op) = running.take() {
                        rig.finish(op);
                    } else {
                        running = rig.start();
                    }
                }
            }
            rig.events.extend(rig.sched.take_events());
        }
        if let Some(op) = running.take() {
            rig.finish(op);
        }
        for (b, _, r) in &blocks[committed..] {
            rig.sched.commit(b.clone(), commit_qc(b, *r));
        }
        rig.drain();
        let answers = rig.answers();
        assert_eq!(rig.sched.outstanding(), 0, "seed {seed}");
        for req in 1..=requests {
            let got = answers.get(&req).map_or(0, Vec::len);
            assert_eq!(got, 1, "seed {seed}: request {req} answered {got} times");
        }
        assert!(
            answers
                .values()
                .flatten()
                .all(|o| !matches!(o, ExecOutcome::Failed(_) | ExecOutcome::Invalid)),
            "seed {seed}: a missing post-state is never Failed or Invalid"
        );
    }
}

/// An executor that panics.
struct Panicking;

impl Executor for Panicking {
    fn execute(&mut self, _: &Block, _: &Hash32) -> Option<ExecOutcome> {
        panic!("boom")
    }
    fn discard(&mut self, _: u64, _: &[Hash32]) {}
    fn prepare(&mut self, _: &Block, _: &Qc) -> Result<Option<Hash32>, String> {
        panic!("boom")
    }
    fn commit(&mut self, _: &Block, _: &Qc) -> Result<HeightConfig, String> {
        panic!("boom")
    }
    fn build(&mut self, _: u64, _: u64, _: u32, _: u32) -> (Vec<u8>, bool) {
        panic!("boom")
    }
    fn reject(&mut self, _: u64, _: u64, _: &Hash32) {}
}

/// Executor panics are local failures (`Failed`, retried apply, `EMPTY` build), never
/// invalidity, and never kill the executor thread.
#[test]
fn executor_panics_become_local_failures() {
    let blocks = FakeBlocks::default();
    let (b1, bh1, r1) = child(1, (G, RG), 1);
    let mut exec = Panicking;
    let done = run_exec(
        &mut exec,
        &blocks,
        ExecOp::Execute {
            block: std::sync::Arc::new(b1.clone()),
            block_hash: bh1,
        },
    );
    assert!(matches!(
        done,
        ExecDone::Executed(Some(ExecOutcome::Failed(_)))
    ));
    let commit = std::sync::Arc::new(super::super::exec::Commit {
        qc: commit_qc(&b1, r1),
        block: b1,
    });
    assert!(matches!(
        run_exec(&mut exec, &blocks, ExecOp::Prepare(commit.clone())),
        ExecDone::Prepared(Err(_))
    ));
    assert!(matches!(
        run_exec(&mut exec, &blocks, ExecOp::Commit(commit)),
        ExecDone::Committed(Err(_))
    ));
    let built = run_exec(
        &mut exec,
        &blocks,
        ExecOp::Build {
            req: 1,
            height: 1,
            view: 0,
            max_bytes: 10,
            exec_budget_ms: 10,
        },
    );
    assert_eq!(
        built,
        ExecDone::Built {
            payload: Vec::new(),
            attest: false
        }
    );
}

/// An executor with a single live overlay (the shape of the node's State executor): any call
/// other than `commit` after a `prepare` drops the prepared post-state, and `commit` fails
/// without it; `fail_commits` commits fail after consuming it.
struct Overlay {
    inner: FakeExecutor,
    prepared: Option<Hash32>,
    fail_commits: u32,
    calls: Vec<&'static str>,
}

impl Overlay {
    fn new() -> Self {
        Self {
            inner: FakeExecutor::new(G, RG, config()),
            prepared: None,
            fail_commits: 0,
            calls: Vec::new(),
        }
    }

    fn other(&mut self, call: &'static str) {
        self.prepared = None;
        self.calls.push(call);
    }
}

impl Executor for Overlay {
    fn execute(&mut self, block: &Block, block_hash: &Hash32) -> Option<ExecOutcome> {
        self.other("execute");
        self.inner.execute(block, block_hash)
    }
    fn discard(&mut self, height: u64, keep: &[Hash32]) {
        self.other("discard");
        self.inner.discard(height, keep);
    }
    fn prepare(&mut self, block: &Block, commit_qc: &Qc) -> Result<Option<Hash32>, String> {
        self.calls.push("prepare");
        let result = self.inner.prepare(block, commit_qc);
        self.prepared = matches!(result, Ok(Some(_))).then_some(commit_qc.block_hash);
        result
    }
    fn commit(&mut self, block: &Block, commit_qc: &Qc) -> Result<HeightConfig, String> {
        self.calls.push("commit");
        if self.prepared.take() != Some(commit_qc.block_hash) {
            return Err("no prepared overlay".to_owned());
        }
        if self.fail_commits > 0 {
            self.fail_commits -= 1;
            return Err("injected".to_owned());
        }
        self.inner.commit(block, commit_qc)
    }
    fn build(&mut self, height: u64, view: u64, max_bytes: u32, budget: u32) -> (Vec<u8>, bool) {
        self.other("build");
        self.inner.build(height, view, max_bytes, budget)
    }
    fn reject(&mut self, height: u64, view: u64, block_hash: &Hash32) {
        self.other("reject");
        self.inner.reject(height, view, block_hash);
    }
}

/// Run the scheduler's operations at `now` until it has none.
fn drive(
    sched: &mut ExecSched,
    exec: &mut Overlay,
    blocks: &FakeBlocks,
    now: Millis,
) -> Vec<Event> {
    let mut events = Vec::new();
    while let Some(op) = sched.next(now) {
        let done = run_exec(exec, blocks, op);
        sched.done(now, done);
        events.extend(sched.take_events());
    }
    events
}

/// While an append backs off, nothing else runs on the executor (no execution, discard,
/// rejection or build between a prepare and its commit), so a single-overlay executor commits
/// once the append succeeds; the other work runs afterwards.
#[test]
fn apply_runs_alone_while_backing_off() {
    let mut sched = ExecSched::new(0, Backoff::default());
    let mut exec = Overlay::new();
    let blocks = FakeBlocks::default();
    let (b1, bh1, r1) = child(1, (G, RG), 1);
    let (b2, bh2, r2) = child(2, (bh1, r1), 2);
    let (x2, bhx, _) = child(2, (bh1, r1), 99);
    blocks.fail_next(1);
    sched.commit(b1.clone(), commit_qc(&b1, r1));
    sched.execute(1, bh2, b2);
    sched.reject(1, 0, Hash32([9; 32]));
    let first = drive(&mut sched, &mut exec, &blocks, 0);
    assert!(first.is_empty(), "{first:?}");
    assert_eq!(
        exec.calls,
        vec!["prepare"],
        "the append failed; nothing else ran"
    );
    // Work arriving during the backoff waits too.
    sched.execute(2, bhx, x2);
    sched.discard(2, vec![bh2]);
    sched.build(5, 2, 0, 1024, 100);
    assert!(drive(&mut sched, &mut exec, &blocks, 9).is_empty());
    assert_eq!(exec.calls, vec!["prepare"]);
    assert_eq!(sched.wakeup(), 10);
    let events = drive(&mut sched, &mut exec, &blocks, 10);
    assert_eq!(&exec.calls[..2], &["prepare", "commit"]);
    assert_eq!(sched.applied(), 1);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::BlockApplied { height: 1, .. }))
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Executed { req: 1, outcome: ExecOutcome::Valid(r), .. } if *r == r2
    )));
    assert!(exec.calls.contains(&"build") && exec.calls.contains(&"reject"));
}

/// A commit that fails is retried after a fresh prepare — the prepared post-state may be gone
/// — and without a second append (the block is durable already).
#[test]
fn failed_commit_prepares_again_without_a_second_append() {
    let mut sched = ExecSched::new(0, Backoff::default());
    let mut exec = Overlay::new();
    exec.fail_commits = 1;
    let blocks = FakeBlocks::default();
    let (b1, bh1, r1) = child(1, (G, RG), 1);
    sched.commit(b1.clone(), commit_qc(&b1, r1));
    let (b2, _, _) = child(2, (bh1, r1), 2);
    let events = drive(&mut sched, &mut exec, &blocks, 0);
    assert!(events.is_empty());
    assert_eq!(exec.calls, vec!["prepare", "commit"]);
    assert_eq!(blocks.height(), 1, "appended once");
    sched.execute(1, hash(&b2), b2);
    assert!(
        drive(&mut sched, &mut exec, &blocks, 5).is_empty(),
        "backing off alone"
    );
    let events = drive(&mut sched, &mut exec, &blocks, 10);
    assert_eq!(
        &exec.calls[..4],
        &["prepare", "commit", "prepare", "commit"]
    );
    assert_eq!(blocks.height(), 1, "no second append");
    assert_eq!(sched.applied(), 1);
    let applied = events
        .iter()
        .filter(|e| matches!(e, Event::BlockApplied { .. }))
        .count();
    assert_eq!(applied, 1);
}

/// Discards of one height merge while they wait (keeping what both keep), and rejections are
/// deduplicated and capped: the queues other than the `Execute`s stay bounded.
#[test]
fn discards_merge_and_rejections_are_bounded() {
    let mut rig = Rig::new();
    let (b1, bh1, r1) = child(1, (G, RG), 1);
    let (a, bha, _) = child(2, (bh1, r1), 2);
    let (b, bhb, _) = child(2, (bh1, r1), 3);
    rig.sched.execute(1, bh1, b1);
    let running = rig.start().unwrap();
    for _ in 0..100 {
        rig.sched.discard(2, vec![bha, bhb]);
    }
    rig.sched.discard(2, vec![bha]);
    for i in 0..200u8 {
        rig.sched.reject(2, 0, Hash32([i; 32]));
        rig.sched.reject(2, 0, Hash32([i; 32]));
    }
    assert_eq!(rig.sched.queued_ops(), 1 + 64, "one discard, 64 rejections");
    rig.finish(running);
    rig.sched.execute(2, bha, a);
    rig.sched.execute(3, bhb, b);
    let next = rig.start().unwrap();
    assert_eq!(
        next,
        ExecOp::Discard {
            height: 2,
            keep: vec![bha]
        }
    );
    rig.finish(next);
    rig.drain();
    let rejected = rig.exec.state.lock().rejected.clone();
    assert_eq!(rejected.len(), 64);
    assert_eq!(rejected[0], Hash32([136; 32]), "the oldest were dropped");
}
