//! The persist-before-effect barrier (`specs/sumeragi.md` §12.3 O2, §7.4).
//!
//! After a `PersistSafety`, every later externally visible action — `Send`, `Broadcast`,
//! `CommitBlock` (and anything later served from the block store), `ServeBlocks`, `ServeBody`,
//! `FetchBody`, `ReportEvidence`, and `Halt` — takes effect only once that record is durable,
//! in the order the core emitted them (O1). The exempt actions (`Execute`, `DiscardExecution`,
//! `BuildPayload`, `PayloadRejected`, `LocalFault`, `StoreBody`) never wait. Records are
//! identified by the sequence numbers of the ordered persistence queue, so "durable up to `s`"
//! also covers every earlier `StoreBody` (§7.4 body durability).

use std::collections::VecDeque;

use iroha_sumeragi::api::Action;

/// Whether `action` waits behind a pending safety record (O2). `PersistSafety` itself is the
/// barrier and is not gated.
pub fn gated(action: &Action) -> bool {
    !matches!(
        action,
        Action::PersistSafety(_)
            | Action::StoreBody { .. }
            | Action::Execute { .. }
            | Action::DiscardExecution { .. }
            | Action::BuildPayload { .. }
            | Action::PayloadRejected { .. }
            | Action::LocalFault(_)
    )
}

/// Effects held behind the latest pending safety record.
#[derive(Debug, Default)]
pub struct Barrier {
    /// Sequence number of the latest record not yet durable.
    pending: Option<u64>,
    /// Held effects with the record they wait for, in emission order.
    held: VecDeque<(u64, Action)>,
}

impl Barrier {
    /// The record with sequence number `seq` was queued: later gated effects wait for it.
    pub fn persisting(&mut self, seq: u64) {
        self.pending = Some(seq);
    }

    /// An action the core emitted: returned to be performed now if it is exempt or no record is
    /// pending, otherwise held.
    pub fn admit(&mut self, action: Action) -> Option<Action> {
        match self.pending {
            Some(seq) if gated(&action) => {
                self.held.push_back((seq, action));
                None
            }
            _ => Some(action),
        }
    }

    /// The writes up to sequence number `durable` are durable: the held effects that waited
    /// for them, in order.
    pub fn release(&mut self, durable: u64) -> Vec<Action> {
        if self.pending.is_some_and(|seq| seq <= durable) {
            self.pending = None;
        }
        let mut released = Vec::new();
        while self.held.front().is_some_and(|(seq, _)| *seq <= durable) {
            if let Some((_, action)) = self.held.pop_front() {
                released.push(action);
            }
        }
        released
    }

    /// The held effects in order (an observation for tests and diagnostics).
    pub fn held(&self) -> impl Iterator<Item = &Action> {
        self.held.iter().map(|(_, action)| action)
    }
}

#[cfg(test)]
mod tests {
    use iroha_sumeragi::{
        api::{HaltReason, LocalFault},
        message::{BlockRequest, WireMessage},
        types::{Hash32, PublicKey},
    };

    use super::*;

    fn send(height: u64) -> Action {
        Action::Send {
            to: PublicKey::new(vec![1; 32]).unwrap(),
            msg: WireMessage::BlockRequest(BlockRequest {
                instance: Hash32::ZERO,
                height,
                block_hash: Hash32::ZERO,
            }),
        }
    }

    /// The O2 table: which actions wait for a pending record.
    #[test]
    fn gated_versus_exempt() {
        let key = PublicKey::new(vec![1; 32]).unwrap();
        let exempt = [
            Action::DiscardExecution {
                height: 1,
                keep: Vec::new(),
            },
            Action::BuildPayload {
                req: 1,
                height: 1,
                view: 0,
                max_bytes: 1,
                exec_budget_ms: 1,
            },
            Action::PayloadRejected {
                height: 1,
                view: 0,
                block_hash: Hash32::ZERO,
            },
            Action::LocalFault(LocalFault::RecordMissing),
        ];
        for action in &exempt {
            assert!(!gated(action), "{action:?}");
        }
        let effects = [
            send(1),
            Action::Broadcast {
                to: vec![key.clone()],
                msg: WireMessage::BlockRequest(BlockRequest {
                    instance: Hash32::ZERO,
                    height: 1,
                    block_hash: Hash32::ZERO,
                }),
            },
            Action::FetchBody {
                height: 1,
                block_hash: Hash32::ZERO,
                peers: Vec::new(),
            },
            Action::ServeBody {
                to: key.clone(),
                height: 1,
                block_hash: Hash32::ZERO,
            },
            Action::ServeBlocks {
                to: key,
                from_height: 1,
                max_count: 1,
                max_bytes: 1,
            },
            Action::Halt(HaltReason::DriverAnomaly),
        ];
        for action in &effects {
            assert!(gated(action), "{action:?}");
        }
    }

    /// Held effects leave in emission order, each once its record is durable; exempt actions
    /// never wait; without a pending record nothing is held.
    #[test]
    fn hold_and_release_in_order() {
        let mut barrier = Barrier::default();
        assert_eq!(barrier.admit(send(1)), Some(send(1)), "no pending record");
        barrier.persisting(3);
        assert_eq!(barrier.admit(send(2)), None);
        assert_eq!(barrier.admit(send(3)), None);
        let fault = Action::LocalFault(LocalFault::RecordMissing);
        assert_eq!(barrier.admit(fault.clone()), Some(fault), "exempt");
        barrier.persisting(5);
        assert_eq!(barrier.admit(send(4)), None);
        assert_eq!(barrier.held().count(), 3);
        assert!(barrier.release(2).is_empty(), "record 3 is not durable yet");
        assert_eq!(barrier.release(4), vec![send(2), send(3)]);
        assert_eq!(barrier.admit(send(5)), None, "record 5 still pending");
        assert_eq!(barrier.release(5), vec![send(4), send(5)]);
        assert_eq!(barrier.admit(send(6)), Some(send(6)));
        assert_eq!(barrier.held().count(), 0);
    }
}
