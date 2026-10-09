//! Lexical parser-only predecessor sharing; every retained graph closes before return.
//!
//! This pass is accepted only by read-only parsing and wallet inspection. Effectful dispatch
//! APIs never accept it. Every newly decoded body's complete local census still runs; only
//! already verified, pointer-identical predecessor closures avoid repeated full graph walks.

use super::*;
use crate::managed::stream_token_custody::body_history::SnapshotReadPass;
use std::{cell::RefCell, sync::Arc};

pub(in crate::managed) struct EnrollmentReadPass<'a> {
    snapshot: &'a SnapshotReadPass<'a>,
    latest: RefCell<Option<Arc<History>>>,
    count: std::cell::Cell<usize>,
}
impl<'a> EnrollmentReadPass<'a> {
    pub(in crate::managed) fn run<T>(
        snapshot: &'a SnapshotReadPass<'a>,
        action: impl FnOnce(&Self) -> Result<T>,
    ) -> Result<T> {
        let pass = Self {
            snapshot,
            latest: RefCell::new(None),
            count: std::cell::Cell::new(0),
        };
        let result = action(&pass);
        // Keep the exact File/Arc chain live even when the parser returns an ordinary error.
        // The final full census authenticates every original record and receipt. No verdict
        // survives this lexical pass; changed custody overrides the inner parser error.
        if let Some(history) = pass.latest.borrow().as_ref() {
            history.require_current()?;
        }
        result
    }
    pub(super) fn snapshot(&self) -> Option<&SnapshotReadPass<'a>> {
        (!norito::core::decode_limits_active()).then_some(self.snapshot)
    }
    pub(super) fn covers_predecessor(&self, prior: Option<&VerifiedUnsignedClosure>) -> bool {
        if norito::core::decode_limits_active() {
            return false;
        }
        let Some(prior) = prior else { return true };
        let latest = self.latest.borrow();
        let mut next = latest.as_ref();
        for _ in 0..MAX_ATTEMPTS {
            let Some(current) = next else { return false };
            if Arc::ptr_eq(current, prior.shared_history()) {
                return true;
            }
            next = current
                .scope
                .predecessor()
                .map(VerifiedUnsignedClosure::shared_history);
        }
        false
    }
    pub(super) fn validate(&self, history: &History) -> Result<()> {
        if self.covers_predecessor(history.scope.predecessor()) {
            history.require_current_local(self.snapshot())
        } else {
            history.require_current()
        }
    }
    pub(super) fn remember(&self, closure: &VerifiedUnsignedClosure) -> Result<()> {
        let prior = self.latest.borrow();
        let predecessor = closure.retained_history().scope.predecessor();
        match (prior.as_ref(), predecessor) {
            (None, None) => {}
            (Some(prior), Some(predecessor))
                if Arc::ptr_eq(prior, predecessor.shared_history()) => {}
            _ => {
                return Err(invalid(
                    "parser predecessor closure changed its exact chain",
                ));
            }
        }
        let count = self
            .count
            .get()
            .checked_add(1)
            .filter(|count| *count <= MAX_ATTEMPTS)
            .ok_or_else(|| invalid("parser history exceeds its body bound"))?;
        drop(prior);
        self.latest
            .replace(Some(Arc::clone(closure.shared_history())));
        self.count.set(count);
        Ok(())
    }
}

#[cfg(test)]
impl EnrollmentReadPass<'_> {
    pub(in crate::managed) fn test_retain_predecessor(&self, history: &History) -> Result<()> {
        let prior = history
            .scope
            .predecessor()
            .ok_or_else(|| invalid("test parser predecessor absent"))?;
        prior.require_retained_with_pass(None)?;
        self.remember(prior)
    }
    pub(in crate::managed) fn test_validate(&self, history: &History) -> Result<()> {
        self.validate(history)
    }
}
