//! One bounded completion deadline and callback wakeup for Metal tickets.

use std::{
    cell::RefCell,
    sync::{Arc, Condvar, Mutex, OnceLock},
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

/// A wakeup hint owned by the existing command-completion permit. Only the
/// command's actual status authorizes publication; this flag never does.
#[derive(Default)]
pub(super) struct CompletionNotification {
    signaled: Mutex<bool>,
    changed: Condvar,
}

impl CompletionNotification {
    pub(super) fn notify(&self) {
        let mut signaled = self
            .signaled
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *signaled = true;
        self.changed.notify_all();
    }

    /// Keep a signal delivered before the waiter locks, then consume it once.
    /// A spurious wake or an early signal merely causes another status check.
    pub(super) fn wait_for(&self, timeout: Duration) -> bool {
        let mut signaled = self
            .signaled
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !*signaled {
            signaled = self
                .changed
                .wait_timeout(signaled, timeout)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        let notified = *signaled;
        *signaled = false;
        notified
    }
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
        notification: &CompletionNotification,
    ) -> Completion {
        let deadline = *self.deadline.get_or_init(|| Instant::now() + self.timeout);
        Self::wait_until(deadline, status, quarantined, notification)
    }

    /// Successful ordinary waits keep their previous per-command allowance.
    /// The first failed wait starts the shared cleanup budget from that wait's
    /// beginning, so later failures/drop cannot repeatedly reset its deadline.
    pub(super) fn wait_one(
        &self,
        status: impl FnMut() -> Completion,
        quarantined: impl Fn() -> bool,
        notification: &CompletionNotification,
    ) -> Completion {
        let deadline = self
            .deadline
            .get()
            .copied()
            .unwrap_or_else(|| Instant::now() + self.timeout);
        let result = Self::wait_until(deadline, status, quarantined, notification);
        if result != Completion::Completed {
            let _ = self.deadline.set(deadline);
        }
        result
    }

    fn wait_until(
        deadline: Instant,
        mut status: impl FnMut() -> Completion,
        quarantined: impl Fn() -> bool,
        notification: &CompletionNotification,
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
            } else {
                // A completion callback wakes this wait without depending on
                // the scheduler's sleep-timer tolerance. Keep bounded fallback
                // polling for absent callbacks and another command's quarantine.
                let remaining = deadline.saturating_duration_since(Instant::now());
                notification.wait_for(remaining.min(Duration::from_millis(1)));
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
    use std::sync::atomic::{AtomicBool, Ordering};

    // Test callbacks wait for the main thread to enter the notification path.
    // Always release and join them, including early timeout and main-thread panic.
    struct WaitingCallback {
        ready: Arc<AtomicBool>,
        worker: Option<thread::JoinHandle<()>>,
    }

    impl WaitingCallback {
        fn finish(mut self) -> thread::Result<()> {
            self.ready.store(true, Ordering::Release);
            self.worker.take().expect("owned callback worker").join()
        }
    }

    impl Drop for WaitingCallback {
        fn drop(&mut self) {
            self.ready.store(true, Ordering::Release);
            if let Some(worker) = self.worker.take() {
                // Normal completion checks the result in finish. During another
                // panic, join without replacing it with a second worker panic.
                let _ = worker.join();
            }
        }
    }

    #[test]
    fn notification_test_worker_finishes_after_early_timeout() {
        let ready = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let worker = {
            let ready = Arc::clone(&ready);
            let finished = Arc::clone(&finished);
            thread::spawn(move || {
                while !ready.load(Ordering::Acquire) {
                    thread::yield_now();
                }
                finished.store(true, Ordering::Release);
            })
        };
        let callback = WaitingCallback {
            ready,
            worker: Some(worker),
        };
        let result = DrainBudget::new(Duration::ZERO).wait_one(
            || Completion::Pending,
            || false,
            &CompletionNotification::default(),
        );
        callback.finish().unwrap();
        assert_eq!(result, Completion::Uncertain);
        assert!(finished.load(Ordering::Acquire));
    }

    #[test]
    fn notification_test_worker_releases_and_joins_during_unwind() {
        let finished = Arc::new(AtomicBool::new(false));
        let result = std::panic::catch_unwind({
            let finished = Arc::clone(&finished);
            move || {
                let ready = Arc::new(AtomicBool::new(false));
                let worker = {
                    let ready = Arc::clone(&ready);
                    thread::spawn(move || {
                        while !ready.load(Ordering::Acquire) {
                            thread::yield_now();
                        }
                        finished.store(true, Ordering::Release);
                    })
                };
                let _callback = WaitingCallback {
                    ready,
                    worker: Some(worker),
                };
                panic!("main callback test unwinds before enabling worker");
            }
        });
        assert!(result.is_err());
        assert!(finished.load(Ordering::Acquire));
    }

    #[test]
    fn notification_before_wait_is_retained_and_consumed_once() {
        let notification = CompletionNotification::default();
        notification.notify();
        assert!(notification.wait_for(Duration::ZERO));
        assert!(!notification.wait_for(Duration::ZERO));
        notification.changed.notify_all();
        assert!(
            !notification.wait_for(Duration::ZERO),
            "a bare condvar wake is not a completion flag"
        );
    }

    #[test]
    fn callback_wakes_wait_and_actual_status_decides_success_or_error() {
        use std::sync::atomic::AtomicU8;
        for expected in [Completion::Completed, Completion::Failed] {
            let notification = Arc::new(CompletionNotification::default());
            let terminal = Arc::new(AtomicU8::new(0));
            let ready = Arc::new(AtomicBool::new(false));
            let callback = {
                let notification = Arc::clone(&notification);
                let terminal = Arc::clone(&terminal);
                let ready = Arc::clone(&ready);
                thread::spawn(move || {
                    while !ready.load(Ordering::Acquire) {
                        thread::yield_now();
                    }
                    terminal.store(
                        if expected == Completion::Completed {
                            1
                        } else {
                            2
                        },
                        Ordering::Release,
                    );
                    notification.notify();
                })
            };
            let callback = WaitingCallback {
                ready: Arc::clone(&ready),
                worker: Some(callback),
            };
            let budget = DrainBudget::new(Duration::from_secs(2));
            let mut polls = 0;
            let observed = budget.wait_one(
                || {
                    polls += 1;
                    if polls > 64 {
                        ready.store(true, Ordering::Release);
                    }
                    match terminal.load(Ordering::Acquire) {
                        1 => Completion::Completed,
                        2 => Completion::Failed,
                        _ => Completion::Pending,
                    }
                },
                || false,
                &notification,
            );
            callback.finish().unwrap();
            assert_eq!(observed, expected);
            assert_eq!(
                budget.deadline.get().is_none(),
                expected == Completion::Completed
            );
        }
    }

    #[test]
    fn premature_and_spurious_notifications_cannot_publish_or_reset_deadline() {
        let budget = DrainBudget::new(Duration::from_millis(2));
        let notification = CompletionNotification::default();
        notification.notify();
        let mut polls = 0;
        assert_eq!(
            budget.wait_one(
                || {
                    polls += 1;
                    // Repeated notifications and spurious wakeups are only hints. The
                    // unchanged Pending status must still exhaust the original budget.
                    notification.notify();
                    notification.changed.notify_all();
                    Completion::Pending
                },
                || false,
                &notification
            ),
            Completion::Uncertain
        );
        assert!(polls > 1);
        let deadline = *budget.deadline.get().unwrap();
        assert_eq!(
            budget.wait(|| Completion::Pending, || false, &notification),
            Completion::Uncertain
        );
        assert_eq!(*budget.deadline.get().unwrap(), deadline);
    }

    #[test]
    fn missing_callback_uses_bounded_status_polling_and_shared_timeout() {
        let notification = CompletionNotification::default();
        let mut polls = 0;
        let budget = DrainBudget::new(Duration::from_secs(2));
        assert_eq!(
            budget.wait_one(
                || {
                    polls += 1;
                    if polls == 68 {
                        Completion::Completed
                    } else {
                        Completion::Pending
                    }
                },
                || false,
                &notification
            ),
            Completion::Completed
        );
        assert_eq!(polls, 68);
        assert!(budget.deadline.get().is_none());

        let timeout = DrainBudget::new(Duration::from_millis(2));
        assert_eq!(
            timeout.wait_one(|| Completion::Pending, || false, &notification),
            Completion::Uncertain
        );
        let deadline = *timeout.deadline.get().unwrap();
        assert_eq!(
            timeout.wait(|| Completion::Pending, || false, &notification),
            Completion::Uncertain
        );
        assert_eq!(*timeout.deadline.get().unwrap(), deadline);
    }

    #[test]
    fn notification_wait_rechecks_another_commands_quarantine() {
        use std::cell::Cell;
        let quarantined = Arc::new(AtomicBool::new(false));
        let ready = Arc::new(AtomicBool::new(false));
        let other = {
            let quarantined = Arc::clone(&quarantined);
            let ready = Arc::clone(&ready);
            thread::spawn(move || {
                while !ready.load(Ordering::Acquire) {
                    thread::yield_now();
                }
                quarantined.store(true, Ordering::Release);
                // Deliberately no notification for this independent command.
            })
        };
        let other = WaitingCallback {
            ready: Arc::clone(&ready),
            worker: Some(other),
        };
        let budget = DrainBudget::new(Duration::from_secs(2));
        let mut polls = 0;
        let saw_quarantine = Cell::new(false);
        let observed = budget.wait_one(
            || {
                polls += 1;
                if polls > 64 {
                    ready.store(true, Ordering::Release);
                }
                Completion::Pending
            },
            || {
                let value = quarantined.load(Ordering::Acquire);
                saw_quarantine.set(saw_quarantine.get() || value);
                value
            },
            &CompletionNotification::default(),
        );
        other.finish().unwrap();
        assert_eq!(observed, Completion::Uncertain);
        assert!(
            saw_quarantine.get(),
            "wait must observe the other command's quarantine"
        );
        assert!(quarantined.load(Ordering::Acquire));
        assert!(budget.deadline.get().is_some());
    }

    #[test]
    fn status_unwind_preserves_notification_and_restores_drain_scope() {
        let notification = CompletionNotification::default();
        let outer = DrainScope::enter(Duration::ZERO);
        let budget = ticket_budget(Duration::ZERO);
        let result = std::panic::catch_unwind(|| {
            let _inner = DrainScope::enter(Duration::from_secs(2));
            notification.notify();
            budget.wait_one(|| panic!("status callback unwind"), || false, &notification);
        });
        assert!(result.is_err());
        assert!(notification.wait_for(Duration::ZERO));
        assert!(Arc::ptr_eq(
            &budget.deadline,
            &ticket_budget(Duration::ZERO).deadline
        ));
        assert_eq!(
            budget.wait(|| Completion::Pending, || false, &notification),
            Completion::Uncertain
        );
        drop(outer);
        assert!(!Arc::ptr_eq(
            &budget.deadline,
            &ticket_budget(Duration::ZERO).deadline
        ));
    }

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
            first.wait(
                || Completion::Pending,
                || false,
                &CompletionNotification::default()
            ),
            Completion::Uncertain
        );
        let deadline = *first.deadline.get().unwrap();
        assert_eq!(
            last.wait(
                || Completion::Pending,
                || false,
                &CompletionNotification::default()
            ),
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
            assert_eq!(
                budget.wait(|| status, || true, &CompletionNotification::default()),
                status
            );
        }
        assert_eq!(
            budget.wait(
                || Completion::Pending,
                || false,
                &CompletionNotification::default()
            ),
            Completion::Uncertain
        );
    }

    #[test]
    fn ordinary_success_does_not_start_the_later_error_cleanup_deadline() {
        let budget = DrainBudget::new(Duration::ZERO);
        assert_eq!(
            budget.wait_one(
                || Completion::Completed,
                || false,
                &CompletionNotification::default()
            ),
            Completion::Completed
        );
        assert!(budget.deadline.get().is_none());
        assert_eq!(
            budget.wait_one(
                || Completion::Failed,
                || false,
                &CompletionNotification::default()
            ),
            Completion::Failed
        );
        let deadline = *budget.deadline.get().unwrap();
        assert_eq!(
            budget.wait_one(
                || Completion::Pending,
                || false,
                &CompletionNotification::default()
            ),
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
                || true,
                &CompletionNotification::default()
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
                || false,
                &CompletionNotification::default()
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
