//! The kernel with a real core: O1/O2 routing through the barrier and the ordered persistence
//! queue, O5 input priority (a due `Tick` first, whatever is queued), O7.

use std::sync::Arc;

use iroha_sumeragi::{
    api::{Action, CommittedTip, Event, Init, LocalFault, LocalParams},
    crypto::Attestation,
    message::{BlockRequest, TrafficClass, WireMessage},
    safety::{RecordState, SafetyRecord},
    testing::FakeValidators,
    types::{ChainParams, Hash32, HeightConfig, Millis},
};
use parking_lot::Mutex;

use super::{
    super::{
        Completion, Kernel, KernelStart, Op, Report,
        exec::{ExecDone, ExecOp},
        ingress::{Ingress, IngressLimits},
        persist::{Backoff, Write},
    },
    block, commit_qc,
};

const INSTANCE: Hash32 = Hash32([5; 32]);

/// A kernel of member 0 of a four-member committee at genesis, and the committee's keys.
pub(super) fn start_kernel(now: Millis) -> (Kernel, FakeValidators) {
    let vals = FakeValidators::new(4, 7, None);
    let me = vals.key(0);
    let crypto = vals.crypto.clone();
    let record = SafetyRecord::fresh(INSTANCE, me.clone(), 0, None)
        .encode(&crypto)
        .unwrap();
    let config = HeightConfig {
        committee: vals.committee.clone(),
        params: ChainParams::default(),
    };
    let init = Init {
        instance: INSTANCE,
        records: vec![(me, RecordState::Present(record), false)],
        genesis_height: 0,
        demotion_window: 128,
        nonce: 1,
        tip: CommittedTip {
            height: 0,
            block_hash: Hash32([0xa0; 32]),
            result: Hash32([0xa1; 32]),
            header: None,
            commit_qc: None,
        },
        configs: vec![(1, config.clone()), (2, config)],
        recent_headers: Vec::new(),
    };
    let start = KernelStart {
        local: LocalParams::default(),
        init,
        signers: vec![Box::new(vals.signer(0).clone())],
        crypto: Box::new(crypto.clone()),
        hasher: Box::new(crypto),
        attestation: Attestation::none(),
        now,
        ingress: Arc::new(Mutex::new(Ingress::new(IngressLimits::default()))),
        backoff: Backoff::default(),
    };
    let (kernel, _) = Kernel::start(start).unwrap();
    (kernel, vals)
}

fn request(height: u64) -> WireMessage {
    WireMessage::BlockRequest(BlockRequest {
        instance: INSTANCE,
        height,
        block_hash: Hash32::ZERO,
    })
}

/// O5: a due `Tick` comes before any queued input however many are queued (Tick lateness is
/// bounded by one event), then local events, then messages; the Tick consumes its deadline.
#[test]
fn tick_first_then_local_then_messages() {
    let (mut kernel, vals) = start_kernel(1_000);
    for h in 0..1_000 {
        kernel.receive(vals.key(1), request(h), TrafficClass::Control);
    }
    kernel.deliver(Event::PayloadReady { req: 99 });
    let due = kernel.core().next_wakeup();
    assert!(due < Millis::MAX);
    assert_eq!(kernel.next_input(due), Some(Event::Tick));
    kernel.handle(due, Event::Tick);
    assert!(
        kernel.core().next_wakeup() > due,
        "the Tick consumed its deadline"
    );
    assert_eq!(
        kernel.next_input(due),
        Some(Event::PayloadReady { req: 99 })
    );
    assert!(matches!(
        kernel.next_input(due),
        Some(Event::Message { .. })
    ));
    // Under the flood, the next deadline is again served first.
    let next = kernel.core().next_wakeup();
    assert_eq!(kernel.next_input(next), Some(Event::Tick));
    assert!(kernel.has_input());
    assert_eq!(kernel.next_wakeup(), next);
}

/// O7 and routing by instance: the node's own messages and other instances' are dropped.
#[test]
fn own_and_foreign_messages_are_dropped() {
    let (mut kernel, vals) = start_kernel(0);
    kernel.receive(vals.key(0), request(1), TrafficClass::Control);
    let foreign = WireMessage::BlockRequest(BlockRequest {
        instance: Hash32([6; 32]),
        height: 1,
        block_hash: Hash32::ZERO,
    });
    kernel.receive(vals.key(1), foreign, TrafficClass::Control);
    assert!(!kernel.has_input());
    kernel.receive(vals.key(1), request(1), TrafficClass::Control);
    assert!(kernel.has_input());
    assert_eq!(kernel.ingress_drops(), 0);
}

fn record(height: u64, vals: &FakeValidators) -> Box<SafetyRecord> {
    Box::new(SafetyRecord::fresh(INSTANCE, vals.key(0), height, None))
}

/// Complete every start-up operation (writes durable, builds `EMPTY`) until none is left.
fn settle(kernel: &mut Kernel) {
    loop {
        let ops = kernel.poll(0);
        if ops.is_empty() {
            return;
        }
        for op in ops {
            match op {
                Op::Persist { seq, .. } => {
                    kernel.complete(
                        0,
                        Completion::Persisted {
                            seq,
                            result: Ok(()),
                        },
                    );
                }
                Op::Exec(ExecOp::Build { .. }) => kernel.complete(
                    0,
                    Completion::Exec(ExecDone::Built {
                        payload: Vec::new(),
                        attest: false,
                    }),
                ),
                Op::Exec(ExecOp::Execute { .. }) => {
                    kernel.complete(0, Completion::Exec(ExecDone::Executed(None)));
                }
                Op::Exec(ExecOp::Discard { .. }) => {
                    kernel.complete(0, Completion::Exec(ExecDone::Discarded));
                }
                Op::Exec(ExecOp::Reject { .. }) => {
                    kernel.complete(0, Completion::Exec(ExecDone::Rejected));
                }
                Op::Exec(op) => panic!("no commit at start-up: {op:?}"),
                Op::Send { .. } | Op::Serve(_) | Op::Report(_) => {}
            }
        }
    }
}

/// O1/O2: after a `PersistSafety`, gated effects wait for its durability and leave in order;
/// exempt actions (execution, faults, bodies) go at once; one write at a time, in order, so
/// the record is durable only after the body stored before it; a failed write is retried, not
/// skipped; the node never addresses itself (O7).
#[test]
fn barrier_and_ordered_persistence() {
    let (mut kernel, vals) = start_kernel(0);
    settle(&mut kernel);
    let b1 = block(1, Hash32([0xa0; 32]), Hash32([0xa1; 32]), Vec::new());
    let send = |h: u64| Action::Broadcast {
        to: vec![vals.key(0), vals.key(1)],
        msg: request(h),
    };
    kernel.route(vec![
        Action::StoreBody { block: b1.clone() },
        Action::PersistSafety(record(1, &vals)),
        send(1),
        Action::Execute {
            block: b1.clone(),
            req: 1,
        },
        Action::LocalFault(LocalFault::RecordMissing),
        Action::CommitBlock {
            block: b1.clone(),
            commit_qc: commit_qc(&b1, Hash32([3; 32])),
        },
    ]);
    let ops = kernel.poll(0);
    assert!(ops.contains(&Op::Report(Report::Fault(LocalFault::RecordMissing))));
    let body = ops
        .iter()
        .find_map(|op| match op {
            Op::Persist {
                seq,
                write: Write::Body(_),
            } => Some(*seq),
            _ => None,
        })
        .expect("the body is written first");
    assert!(
        ops.iter()
            .any(|op| matches!(op, Op::Exec(ExecOp::Execute { .. })))
    );
    assert!(!ops.iter().any(|op| matches!(
        op,
        Op::Send { .. }
            | Op::Persist {
                write: Write::Record(_),
                ..
            }
    )));
    assert_eq!(kernel.held().len(), 2, "the broadcast and the commit wait");
    // The body is durable: the record is written next; the effects still wait.
    kernel.complete(
        0,
        Completion::Persisted {
            seq: body,
            result: Ok(()),
        },
    );
    let ops = kernel.poll(0);
    let [Op::Persist { seq, write }] = &ops[..] else {
        panic!("{ops:?}")
    };
    let (seq, write) = (*seq, write.clone());
    assert_eq!(seq, body + 1);
    assert!(matches!(write, Write::Record(_)));
    // The record write fails: retried after the backoff, nothing released meanwhile.
    kernel.complete(
        0,
        Completion::Persisted {
            seq,
            result: Err(write),
        },
    );
    assert!(kernel.poll(5).is_empty());
    let ops = kernel.poll(10);
    assert!(
        matches!(&ops[..], [Op::Persist { seq: s, .. }] if *s == seq),
        "retried, not skipped"
    );
    kernel.complete(
        10,
        Completion::Persisted {
            seq,
            result: Ok(()),
        },
    );
    let ops = kernel.poll(10);
    assert_eq!(
        ops,
        vec![Op::Send {
            to: vec![vals.key(1)],
            msg: request(1)
        }],
        "released in order, without the node's own key; the commit waits for the executor"
    );
    assert!(kernel.held().is_empty());
    assert!(kernel.exec().busy());
}

/// A local event from serving reaches the core like any local event.
#[test]
fn served_bodies_become_local_events() {
    let (mut kernel, _) = start_kernel(0);
    let b1 = block(1, Hash32([0xa0; 32]), Hash32([0xa1; 32]), Vec::new());
    kernel.complete(
        0,
        Completion::Served(Event::BodyAvailable { block: b1.clone() }),
    );
    kernel.transactions_available();
    let mut now = 0;
    loop {
        match kernel.next_input(now) {
            Some(Event::Tick) => {
                kernel.handle(now, Event::Tick);
                now += 1;
            }
            other => {
                assert_eq!(other, Some(Event::BodyAvailable { block: b1 }));
                break;
            }
        }
    }
}
