//! Deterministic arbitration between Sumeragi v2 timers and admitted work.
//!
//! The kernel is clock-free and dependency-free: callers project their clock
//! and queue state to three booleans, then execute the selected work. This is
//! the authoritative branch relation shared by production and formal
//! refinement.

use super::refinement::{
    ProductionSchedulerTraceProjection,
    production_scheduler_trace_refines_protected_ownership_kernel,
};

// Constructor expressions are arguments because the ordinary Rust and Verus
// instantiations use different result types. Keeping the branch conditions in
// one macro prevents either side from silently changing timer/FIFO priority.
macro_rules! schedule_select_body {
    (
        $fifo_owed:expr,
        $timeout_due:expr,
        $periodic_timer_due:expr,
        $fifo_ready:expr,
        $timeout_result:expr,
        $periodic_timer_result:expr,
        $fifo_result:expr,
        $idle_result:expr $(,)?
    ) => {{
        if $timeout_due {
            $timeout_result
        } else if $fifo_ready && $fifo_owed {
            $fifo_result
        } else if $periodic_timer_due {
            $periodic_timer_result
        } else if $fifo_ready {
            $fifo_result
        } else {
            $idle_result
        }
    }};
}

/// Persistent state required by the scheduling decision.
///
/// A non-timeout timer may run before an already-admitted command, preserving
/// prompt retransmission. Once that happens, the command is owed the next slot
/// even if the monotonic clock advances far enough for the timer to remain due.
/// Absolute round timeout may still preempt that debt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScheduleState {
    pub(crate) fifo_owed: bool,
}

/// One source selected for a serialized runtime step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduledWork {
    /// Emit the absolute round timeout.
    Timeout,
    /// Emit one non-timeout periodic timer, currently retransmission.
    PeriodicTimer,
    /// Dispatch the oldest admitted FIFO command.
    Fifo,
    /// No timer is due and the FIFO is empty.
    Idle,
}

impl ScheduleState {
    /// Select exactly one source and return the next arbitration state.
    pub fn select(
        self,
        timeout_due: bool,
        periodic_timer_due: bool,
        fifo_ready: bool,
    ) -> (ScheduledWork, Self) {
        let (selected, next) = schedule_select_body!(
            self.fifo_owed,
            timeout_due,
            periodic_timer_due,
            fifo_ready,
            (
                ScheduledWork::Timeout,
                Self {
                    fifo_owed: fifo_ready,
                },
            ),
            (
                ScheduledWork::PeriodicTimer,
                Self {
                    fifo_owed: fifo_ready,
                },
            ),
            (ScheduledWork::Fifo, Self { fifo_owed: false }),
            (ScheduledWork::Idle, Self { fifo_owed: false }),
        );
        let scheduler_trace = ProductionSchedulerTraceProjection {
            fifo_owed_before: self.fifo_owed,
            timeout_due,
            periodic_timer_due,
            fifo_ready,
            selected: match selected {
                ScheduledWork::Timeout => 1,
                ScheduledWork::PeriodicTimer => 2,
                ScheduledWork::Fifo => 3,
                ScheduledWork::Idle => 0,
            },
            fifo_owed_after: next.fifo_owed,
        };
        if !production_scheduler_trace_refines_protected_ownership_kernel(scheduler_trace) {
            panic!("Sumeragi v2 scheduler lost the selected progress owner");
        }
        (selected, next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_has_absolute_priority_and_preserves_fifo_debt() {
        let (work, next) = ScheduleState { fifo_owed: true }.select(true, true, true);
        assert_eq!(work, ScheduledWork::Timeout);
        assert!(next.fifo_owed);
    }

    #[test]
    fn periodic_timer_can_delay_ready_fifo_only_once() {
        let (work, next) = ScheduleState::default().select(false, true, true);
        assert_eq!(work, ScheduledWork::PeriodicTimer);
        assert!(next.fifo_owed);

        let (work, next) = next.select(false, true, true);
        assert_eq!(work, ScheduledWork::Fifo);
        assert!(!next.fifo_owed);
    }

    #[test]
    fn idle_and_uncontended_fifo_do_not_create_debt() {
        assert_eq!(
            ScheduleState::default().select(false, false, false),
            (ScheduledWork::Idle, ScheduleState::default())
        );
        assert_eq!(
            ScheduleState::default().select(false, false, true),
            (ScheduledWork::Fifo, ScheduleState::default())
        );
    }
}
