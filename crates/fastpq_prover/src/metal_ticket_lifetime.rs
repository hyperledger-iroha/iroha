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

/// Controlled wait-policy comparison; there is no shipping policy override.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TestWaitStrategy {
    YieldThenNotify,
    ActiveOneMillisecond,
    ActiveTwoMilliseconds,
    ActiveSixteenMilliseconds,
}

#[cfg(test)]
impl TestWaitStrategy {
    fn active_duration(self) -> Option<Duration> {
        match self {
            Self::YieldThenNotify => None,
            Self::ActiveOneMillisecond => Some(Duration::from_millis(1)),
            Self::ActiveTwoMilliseconds => Some(Duration::from_millis(2)),
            Self::ActiveSixteenMilliseconds => Some(Duration::from_millis(16)),
        }
    }
}

#[cfg(test)]
thread_local! {
    static TEST_WAIT_STRATEGY: std::cell::Cell<TestWaitStrategy> = const {
        std::cell::Cell::new(TestWaitStrategy::YieldThenNotify)
    };
}

/// Restore even nested diagnostic scopes during unwind, on the same thread.
#[cfg(test)]
pub(super) struct TestWaitStrategyScope {
    previous: TestWaitStrategy,
    same_thread: core::marker::PhantomData<std::rc::Rc<()>>,
}

#[cfg(test)]
impl TestWaitStrategyScope {
    pub(super) fn enter(strategy: TestWaitStrategy) -> Self {
        Self {
            previous: TEST_WAIT_STRATEGY.with(|slot| slot.replace(strategy)),
            same_thread: core::marker::PhantomData,
        }
    }
}

#[cfg(test)]
impl Drop for TestWaitStrategyScope {
    fn drop(&mut self) {
        TEST_WAIT_STRATEGY.with(|slot| slot.set(self.previous));
    }
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
        self.wait_with_clock(status, quarantined, notification, Instant::now)
    }

    // Shipping entry points bind this private seam to Instant::now. Tests can
    // advance the same deadline without depending on scheduler timing.
    fn wait_with_clock(
        &self,
        status: impl FnMut() -> Completion,
        quarantined: impl Fn() -> bool,
        notification: &CompletionNotification,
        mut now: impl FnMut() -> Instant,
    ) -> Completion {
        let deadline = *self.deadline.get_or_init(|| now() + self.timeout);
        Self::wait_until_with_clock(deadline, status, quarantined, notification, now)
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
        self.wait_one_with_clock(status, quarantined, notification, Instant::now)
    }

    fn wait_one_with_clock(
        &self,
        status: impl FnMut() -> Completion,
        quarantined: impl Fn() -> bool,
        notification: &CompletionNotification,
        mut now: impl FnMut() -> Instant,
    ) -> Completion {
        let deadline = self
            .deadline
            .get()
            .copied()
            .unwrap_or_else(|| now() + self.timeout);
        let result = Self::wait_until_with_clock(deadline, status, quarantined, notification, now);
        if result != Completion::Completed {
            let _ = self.deadline.set(deadline);
        }
        result
    }

    fn wait_until_with_clock(
        deadline: Instant,
        mut status: impl FnMut() -> Completion,
        quarantined: impl Fn() -> bool,
        notification: &CompletionNotification,
        mut now: impl FnMut() -> Instant,
    ) -> Completion {
        let mut polls = 0_usize;
        #[cfg(test)]
        let active_until = TEST_WAIT_STRATEGY.with(|slot| {
            slot.get()
                .active_duration()
                .map(|duration| (now() + duration).min(deadline))
        });
        loop {
            match status() {
                Completion::Pending => {}
                result => return result,
            }
            if quarantined() || now() >= deadline {
                return Completion::Uncertain;
            }
            #[cfg(test)]
            if let Some(active_until) = active_until {
                if now() < active_until {
                    // Status and quarantine are checked on every iteration.
                    // Active polling never turns a notification into success.
                    std::hint::spin_loop();
                    continue;
                }
                // Replace the original yield allowance, then use exactly its
                // callback notification and bounded missing-callback fallback.
                polls = 64;
            }
            polls = polls.saturating_add(1);
            if polls <= 64 {
                thread::yield_now();
            } else {
                // A completion callback wakes this wait without depending on
                // the scheduler's sleep-timer tolerance. Keep bounded fallback
                // polling for absent callbacks and another command's quarantine.
                let remaining = deadline.saturating_duration_since(now());
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
        use std::cell::Cell;

        let budget = DrainBudget::new(Duration::from_millis(2));
        let notification = CompletionNotification::default();
        notification.notify();
        let start = Instant::now();
        let now = Cell::new(start);
        let polls = Cell::new(0_usize);
        let consumed_notifications = Cell::new(0_usize);
        assert_eq!(
            budget.wait_one_with_clock(
                || {
                    let poll = polls.get() + 1;
                    assert!(poll <= 70, "the original deadline must end this wait");
                    polls.set(poll);
                    if poll > 65 {
                        // Polls 65 through 69 must consume the prior signal in
                        // the notification path, rather than publishing it.
                        assert!(!*notification.signaled.lock().unwrap());
                        consumed_notifications.set(consumed_notifications.get() + 1);
                    }
                    // Repeated notifications and spurious wakeups are only hints. The
                    // unchanged Pending status must still exhaust the original budget.
                    notification.notify();
                    notification.changed.notify_all();
                    if poll == 70 {
                        now.set(start + Duration::from_millis(2));
                    }
                    Completion::Pending
                },
                || false,
                &notification,
                || now.get(),
            ),
            Completion::Uncertain
        );
        assert!(polls.get() > 1);
        assert_eq!(polls.get(), 70);
        assert_eq!(consumed_notifications.get(), 5);
        let deadline = *budget.deadline.get().unwrap();
        assert_eq!(deadline, start + Duration::from_millis(2));
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

    #[test]
    fn test_wait_strategy_scope_is_thread_local_and_restored_on_unwind() {
        assert_eq!(
            TEST_WAIT_STRATEGY.with(std::cell::Cell::get),
            TestWaitStrategy::YieldThenNotify
        );
        {
            let _outer = TestWaitStrategyScope::enter(TestWaitStrategy::ActiveOneMillisecond);
            assert!(
                std::panic::catch_unwind(|| {
                    let _inner =
                        TestWaitStrategyScope::enter(TestWaitStrategy::ActiveTwoMilliseconds);
                    panic!("diagnostic wait scope unwind");
                })
                .is_err()
            );
            assert_eq!(
                TEST_WAIT_STRATEGY.with(std::cell::Cell::get),
                TestWaitStrategy::ActiveOneMillisecond
            );
            assert_eq!(
                thread::spawn(|| TEST_WAIT_STRATEGY.with(std::cell::Cell::get))
                    .join()
                    .unwrap(),
                TestWaitStrategy::YieldThenNotify
            );
        }
        assert_eq!(
            TEST_WAIT_STRATEGY.with(std::cell::Cell::get),
            TestWaitStrategy::YieldThenNotify
        );
    }

    #[test]
    fn active_wait_strategy_uses_notification_then_missing_callback_fallback() {
        use std::cell::Cell;
        for strategy in [
            TestWaitStrategy::ActiveOneMillisecond,
            TestWaitStrategy::ActiveTwoMilliseconds,
            TestWaitStrategy::ActiveSixteenMilliseconds,
        ] {
            let _scope = TestWaitStrategyScope::enter(strategy);
            let start = Instant::now();
            let now = Cell::new(start);
            let notification = CompletionNotification::default();
            notification.notify();
            let mut polls = 0;
            let budget = DrainBudget::new(Duration::from_secs(1));
            assert_eq!(
                budget.wait_one_with_clock(
                    || {
                        polls += 1;
                        match polls {
                            1 => {}
                            2 => {
                                // The early hint stays pending throughout the
                                // active interval and cannot authorize success.
                                assert!(*notification.signaled.lock().unwrap());
                                now.set(start + strategy.active_duration().unwrap());
                            }
                            3 => {
                                assert!(!*notification.signaled.lock().unwrap());
                                // No second callback: the unchanged fallback
                                // must still return to the actual-status poll.
                            }
                            4 => return Completion::Completed,
                            _ => panic!("status should finish after the missing callback"),
                        }
                        Completion::Pending
                    },
                    || false,
                    &notification,
                    || now.get(),
                ),
                Completion::Completed
            );
            assert_eq!(polls, 4);
            assert!(budget.deadline.get().is_none());
        }
    }

    #[test]
    fn active_wait_strategy_preserves_status_first_and_shared_absolute_deadline() {
        use std::cell::Cell;
        for strategy in [
            TestWaitStrategy::ActiveOneMillisecond,
            TestWaitStrategy::ActiveTwoMilliseconds,
            TestWaitStrategy::ActiveSixteenMilliseconds,
        ] {
            let _scope = TestWaitStrategyScope::enter(strategy);
            let start = Instant::now();
            let now = Cell::new(start);
            let notification = CompletionNotification::default();
            notification.notify();
            let budget = DrainBudget::new(Duration::from_millis(1));
            let later = budget.clone();
            let mut polls = 0;
            assert_eq!(
                budget.wait_one_with_clock(
                    || {
                        polls += 1;
                        now.set(start + Duration::from_micros(250 * polls));
                        Completion::Pending
                    },
                    || false,
                    &notification,
                    || now.get(),
                ),
                Completion::Uncertain
            );
            assert_eq!(polls, 4);
            let deadline = start + Duration::from_millis(1);
            assert_eq!(*budget.deadline.get().unwrap(), deadline);
            assert!(*notification.signaled.lock().unwrap());
            assert_eq!(
                later.wait_one_with_clock(
                    || Completion::Pending,
                    || false,
                    &notification,
                    || now.get(),
                ),
                Completion::Uncertain
            );
            for terminal in [Completion::Completed, Completion::Failed] {
                assert_eq!(
                    later.wait_one_with_clock(|| terminal, || true, &notification, || now.get(),),
                    terminal
                );
            }
            assert_eq!(*later.deadline.get().unwrap(), deadline);
        }
    }
}
