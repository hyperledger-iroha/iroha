//! Local deferred-claim ownership transition; proof decisions remain with the caller.

use super::SUFFIX_UNLOAD_DEFERRED_STEPS_V1;

/// Whether the retained unsettled steps have reached their local batch ceiling.
pub(super) const fn settlement_due(completed: u32, settled: u32) -> bool {
    completed - settled >= SUFFIX_UNLOAD_DEFERRED_STEPS_V1
}

/// The current provisional snapshot, last settled snapshot and retained claims.
pub(super) struct DeferredBatchTransition<'a, S, P> {
    /// Snapshot of the current provisional cursor state.
    pub(super) current: &'a mut S,
    /// Last snapshot whose complete deferred decisions succeeded.
    pub(super) settled: &'a mut S,
    /// The one owned batch of pending decisions, absent in unbatched mode.
    pub(super) pending: &'a mut Option<P>,
}

impl<S: Copy, P> DeferredBatchTransition<'_, S, P> {
    /// Settle at the retained-step ceiling, or earlier for an explicit request.
    /// Preparation authenticates the verifier before ownership moves. A decision
    /// failure discards only the unsettled batch and restores every snapshot field.
    pub(super) fn settle<E>(
        self,
        force: bool,
        completed: impl Fn(&S) -> u32,
        prepare: impl FnOnce() -> Result<P, E>,
        is_empty: impl FnOnce(&P) -> bool,
        decide: impl FnOnce(P) -> Result<(), E>,
    ) -> Result<(), E> {
        if !force && !settlement_due(completed(self.current), completed(self.settled)) {
            return Ok(());
        }
        // A foreign verifier must leave the original pending owner untouched.
        let replacement = prepare()?;
        let Some(pending) = self.pending.take() else {
            return Ok(());
        };
        *self.pending = Some(replacement);
        let decided = if is_empty(&pending) {
            Ok(())
        } else {
            decide(pending)
        };
        match decided {
            Ok(()) => {
                *self.settled = *self.current;
                Ok(())
            }
            Err(error) => {
                *self.current = *self.settled;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Snapshot {
        completed: u32,
        state: [u8; 32],
        adjusted: [u8; 32],
        receives: u32,
        prior_unloads: u32,
        chain: [u8; 32],
    }

    struct Pending {
        claims: Vec<u32>,
        drops: Arc<AtomicUsize>,
    }

    impl Drop for Pending {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn original() -> Snapshot {
        Snapshot {
            completed: 11,
            state: [1; 32],
            adjusted: [2; 32],
            receives: 7,
            prior_unloads: 4,
            chain: [3; 32],
        }
    }

    #[test]
    fn sixty_three_steps_stay_owned_and_sixty_four_failed_decisions_restore_every_field() {
        assert_eq!(SUFFIX_UNLOAD_DEFERRED_STEPS_V1, 64);
        let original = original();
        let mut settled = original;
        let mut current = Snapshot {
            completed: 74,
            state: [4; 32],
            adjusted: [5; 32],
            receives: 60,
            prior_unloads: 14,
            chain: [6; 32],
        };
        let original_drops = Arc::new(AtomicUsize::new(0));
        let replacement_drops = Arc::new(AtomicUsize::new(0));
        let mut pending = Some(Pending {
            claims: (0..126).collect(),
            drops: original_drops.clone(),
        });
        let original_allocation = pending.as_ref().unwrap().claims.as_ptr();
        assert!(!settlement_due(current.completed, settled.completed));
        DeferredBatchTransition {
            current: &mut current,
            settled: &mut settled,
            pending: &mut pending,
        }
        .settle(
            false,
            |s| s.completed,
            || -> Result<Pending, &'static str> {
                panic!("63 retained steps must not replace their owner")
            },
            |_| panic!("63 retained steps must not be drained"),
            |_| panic!("63 retained steps must not be decided early"),
        )
        .unwrap();
        assert_eq!(settled, original);
        assert_eq!(current.completed, 74);
        assert_eq!(
            pending.as_ref().unwrap().claims.as_ptr(),
            original_allocation
        );
        assert_eq!(pending.as_ref().unwrap().claims.len(), 126);
        assert_eq!(original_drops.load(Ordering::SeqCst), 0);

        current.completed += 1;
        current.receives += 1;
        assert!(settlement_due(current.completed, settled.completed));
        pending.as_mut().unwrap().claims.extend([126, 127]);
        let decisions = AtomicUsize::new(0);
        let error = DeferredBatchTransition {
            current: &mut current,
            settled: &mut settled,
            pending: &mut pending,
        }
        .settle(
            false,
            |s| s.completed,
            || {
                Ok(Pending {
                    claims: Vec::new(),
                    drops: replacement_drops.clone(),
                })
            },
            |batch| batch.claims.is_empty(),
            |batch| {
                decisions.fetch_add(1, Ordering::SeqCst);
                assert_eq!(batch.claims, (0..128).collect::<Vec<_>>());
                Err("real-decision failure injected into the local ownership transition")
            },
        )
        .unwrap_err();
        assert!(error.contains("failure"));
        assert_eq!(decisions.load(Ordering::SeqCst), 1);
        assert_eq!(current, original);
        assert_eq!(settled, original);
        assert!(pending.as_ref().unwrap().claims.is_empty());
        assert!(Arc::ptr_eq(
            &pending.as_ref().unwrap().drops,
            &replacement_drops
        ));
        assert_eq!(original_drops.load(Ordering::SeqCst), 1);
        assert_eq!(replacement_drops.load(Ordering::SeqCst), 0);
        drop(pending);
        assert_eq!(replacement_drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn explicit_failed_settle_rolls_back_below_limit_and_foreign_owner_refusal_never_drains() {
        let original = original();
        let mut settled = original;
        let mut current = Snapshot {
            completed: 12,
            state: [9; 32],
            adjusted: [8; 32],
            receives: 8,
            prior_unloads: 4,
            chain: [7; 32],
        };
        let provisional = current;
        let drops = Arc::new(AtomicUsize::new(0));
        let mut pending = Some(Pending {
            claims: vec![7],
            drops: drops.clone(),
        });
        let allocation = pending.as_ref().unwrap().claims.as_ptr();
        let refused = DeferredBatchTransition {
            current: &mut current,
            settled: &mut settled,
            pending: &mut pending,
        }
        .settle(
            true,
            |s| s.completed,
            || Err("foreign verifier"),
            |_| false,
            |_| panic!("foreign verifier must not receive pending claims"),
        );
        assert_eq!(refused, Err("foreign verifier"));
        assert_eq!(current, provisional);
        assert_eq!(settled, original);
        assert_eq!(pending.as_ref().unwrap().claims.as_ptr(), allocation);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        let replacement_drops = Arc::new(AtomicUsize::new(0));
        let failed = DeferredBatchTransition {
            current: &mut current,
            settled: &mut settled,
            pending: &mut pending,
        }
        .settle(
            true,
            |s| s.completed,
            || {
                Ok(Pending {
                    claims: Vec::new(),
                    drops: replacement_drops,
                })
            },
            |batch| batch.claims.is_empty(),
            |batch| {
                assert_eq!(batch.claims, [7]);
                Err("cancelled decision")
            },
        );
        assert_eq!(failed, Err("cancelled decision"));
        assert_eq!(current, original);
        assert_eq!(settled, original);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(pending.as_ref().unwrap().claims.is_empty());
    }
}
