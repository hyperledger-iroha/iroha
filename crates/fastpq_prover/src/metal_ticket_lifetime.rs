//! One bounded completion deadline shared by a Metal dispatch and its tickets.

use std::{
    cell::RefCell,
    sync::{Arc, OnceLock},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Completion {
    Pending,
    Completed,
    Failed,
    Uncertain,
}

#[derive(Clone)]
pub(super) struct DrainBudget {
    deadline: Arc<OnceLock<Instant>>,
    timeout: Duration,
}

impl DrainBudget {
    fn new(timeout: Duration) -> Self {
        Self {
            deadline: Arc::new(OnceLock::new()),
            timeout,
        }
    }

    pub(super) fn wait(
        &self,
        status: impl FnMut() -> Completion,
        quarantined: impl Fn() -> bool,
    ) -> Completion {
        let deadline = *self.deadline.get_or_init(|| Instant::now() + self.timeout);
        Self::wait_until(deadline, status, quarantined)
    }

    /// Successful ordinary waits keep their previous per-command allowance.
    /// The first failed wait starts the shared cleanup budget from that wait's
    /// beginning, so later failures/drop cannot repeatedly reset its deadline.
    pub(super) fn wait_one(
        &self,
        status: impl FnMut() -> Completion,
        quarantined: impl Fn() -> bool,
    ) -> Completion {
        let deadline = self
            .deadline
            .get()
            .copied()
            .unwrap_or_else(|| Instant::now() + self.timeout);
        let result = Self::wait_until(deadline, status, quarantined);
        if result != Completion::Completed {
            let _ = self.deadline.set(deadline);
        }
        result
    }

    fn wait_until(
        deadline: Instant,
        mut status: impl FnMut() -> Completion,
        quarantined: impl Fn() -> bool,
    ) -> Completion {
        let mut polls = 0_usize;
        loop {
            match status() {
                Completion::Pending => {}
                result => return result,
            }
            if quarantined() || Instant::now() >= deadline {
                return Completion::Uncertain;
            }
            polls = polls.saturating_add(1);
            if polls <= 64 {
                thread::yield_now();
            } else if polls <= 256 {
                thread::sleep(Duration::from_micros(50));
            } else {
                thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

thread_local! {
    static CURRENT_BUDGET: RefCell<Option<DrainBudget>> = const { RefCell::new(None) };
}

pub(super) struct DrainScope(Option<DrainBudget>);

impl DrainScope {
    pub(super) fn enter(timeout: Duration) -> Self {
        CURRENT_BUDGET.with(|slot| {
            let mut slot = slot.borrow_mut();
            let previous = slot.clone();
            if slot.is_none() {
                *slot = Some(DrainBudget::new(timeout));
            }
            Self(previous)
        })
    }
}

impl Drop for DrainScope {
    fn drop(&mut self) {
        CURRENT_BUDGET.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}

pub(super) fn ticket_budget(timeout: Duration) -> DrainBudget {
    CURRENT_BUDGET.with(|slot| {
        slot.borrow()
            .clone()
            .unwrap_or_else(|| DrainBudget::new(timeout))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tickets_and_nested_scopes_retain_one_deadline_after_scope_return() {
        let first;
        let last;
        {
            let _scope = DrainScope::enter(Duration::ZERO);
            first = ticket_budget(Duration::from_secs(120));
            {
                let _nested = DrainScope::enter(Duration::from_secs(240));
                last = ticket_budget(Duration::from_secs(360));
                assert!(Arc::ptr_eq(&first.deadline, &last.deadline));
            }
            assert!(Arc::ptr_eq(
                &first.deadline,
                &ticket_budget(Duration::ZERO).deadline
            ));
        }
        assert_eq!(
            first.wait(|| Completion::Pending, || false),
            Completion::Uncertain
        );
        let deadline = *first.deadline.get().unwrap();
        assert_eq!(
            last.wait(|| Completion::Pending, || false),
            Completion::Uncertain
        );
        assert_eq!(*last.deadline.get().unwrap(), deadline);
        assert!(!Arc::ptr_eq(
            &first.deadline,
            &ticket_budget(Duration::ZERO).deadline
        ));
    }

    #[test]
    fn terminal_status_is_checked_even_after_deadline_or_quarantine() {
        let budget = DrainBudget::new(Duration::ZERO);
        for status in [Completion::Completed, Completion::Failed] {
            assert_eq!(budget.wait(|| status, || true), status);
        }
        assert_eq!(
            budget.wait(|| Completion::Pending, || false),
            Completion::Uncertain
        );
    }

    #[test]
    fn ordinary_success_does_not_start_the_later_error_cleanup_deadline() {
        let budget = DrainBudget::new(Duration::ZERO);
        assert_eq!(
            budget.wait_one(|| Completion::Completed, || false),
            Completion::Completed
        );
        assert!(budget.deadline.get().is_none());
        assert_eq!(
            budget.wait_one(|| Completion::Failed, || false),
            Completion::Failed
        );
        let deadline = *budget.deadline.get().unwrap();
        assert_eq!(
            budget.wait_one(|| Completion::Pending, || false),
            Completion::Uncertain
        );
        assert_eq!(*budget.deadline.get().unwrap(), deadline);
    }

    #[test]
    fn quarantine_prevents_another_wait_budget_and_normal_polling_completes() {
        let budget = DrainBudget::new(Duration::from_secs(120));
        let mut polls = 0;
        assert_eq!(
            budget.wait(
                || {
                    polls += 1;
                    Completion::Pending
                },
                || true
            ),
            Completion::Uncertain
        );
        assert_eq!(polls, 1);
        let mut polls = 0;
        assert_eq!(
            budget.wait(
                || {
                    polls += 1;
                    if polls == 3 {
                        Completion::Completed
                    } else {
                        Completion::Pending
                    }
                },
                || false
            ),
            Completion::Completed
        );
        assert_eq!(polls, 3);
    }

    #[test]
    fn unwind_restores_the_enclosing_scope() {
        let _scope = DrainScope::enter(Duration::ZERO);
        let first = ticket_budget(Duration::ZERO);
        assert!(
            std::panic::catch_unwind(|| {
                let _nested = DrainScope::enter(Duration::from_secs(120));
                panic!("dispatch scope unwind fixture");
            })
            .is_err()
        );
        assert!(Arc::ptr_eq(
            &first.deadline,
            &ticket_budget(Duration::ZERO).deadline
        ));
    }
}
