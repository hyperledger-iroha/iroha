//! Constant-space fairness for fresh original-source AMX admission.
//!
//! This planner supplies scheduling only. Every visit must reconstruct the original opaque
//! source, and the ledger remains the sole owner of admission, outcomes and retry authority.
//! Existing durable pending work and the active expiry cursor are serviced by the caller
//! before this fresh gate. Restart discards this planner and restores only real pending work.

/// Stable position in one route's ascending authenticated source enumeration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct AdmissionPosition {
    pub(super) height: u64,
    /// Index in the original per-height enumeration, including skipped source intents.
    pub(super) ordinal: u64,
    /// Exact original workflow request context; coordinates alone never select a replacement.
    pub(super) plan: [u8; 32],
}

/// One finite admission round, which may span many complete source sweeps.
#[derive(Default)]
pub(super) struct FreshAdmissions {
    frontier: Option<u64>,
    cursor: Option<AdmissionPosition>,
    selected: Option<AdmissionPosition>,
}

impl FreshAdmissions {
    /// Capture the finite source frontier once, retaining it through admission refusal.
    pub(super) fn frontier(&mut self, observed_tip: u64) -> u64 {
        let frontier = *self.frontier.get_or_insert(observed_tip);
        if cfg!(all(test, sumeragi_daemon_mutation = "HC332")) {
            return observed_tip;
        }
        frontier
    }

    /// Select fresh ready work before reserving capacity or beginning expiry observation.
    ///
    /// The caller visits original intents in ascending `(height, ordinal)` order through
    /// `frontier()`, checking already pending work first. A refusal leaves this exact selected
    /// position in place, so released capacity cannot be reacquired by an earlier completed
    /// source. A future signed wake neither consumes a slot nor obstructs later ready work.
    /// If an unstarted selected source becomes not ready, it is skipped for this round only.
    pub(super) fn consider(&mut self, position: AdmissionPosition, ready: bool) -> bool {
        if position.height < 2
            || self
                .frontier
                .is_none_or(|frontier| position.height > frontier)
            || self.cursor.is_some_and(|cursor| position <= cursor)
            || self.selected.is_some_and(|selected| selected != position)
        {
            return false;
        }
        self.selected = Some(position);
        if !ready {
            self.skip(position);
            return false;
        }
        true
    }

    /// Advance only after the original ledger reservation has actually succeeded.
    ///
    /// The caller may also acknowledge an exact selected source that is now present in the
    /// authenticated pending inventory after an uncertain admission acknowledgement. Other
    /// pending sources must keep progressing without moving this fresh cursor. For expiry,
    /// reserve first and retain that original lease through observation and delivery; if the
    /// caller observes first, do not call this until later ledger admission really succeeds.
    pub(super) fn admitted(&mut self, position: AdmissionPosition) -> bool {
        self.advance(position)
    }

    /// Skip one selected unusable attempt for this round without claiming a ledger outcome.
    ///
    /// Use for an independently classified non-capacity refusal, not capacity contention or
    /// an unfinished expiry read. The next admission round may examine this original again.
    pub(super) fn skip(&mut self, position: AdmissionPosition) -> bool {
        self.advance(position)
    }

    fn advance(&mut self, position: AdmissionPosition) -> bool {
        if self.selected != Some(position) {
            return false;
        }
        self.cursor = Some(position);
        self.selected = None;
        true
    }

    /// Finish one complete traversal through the captured frontier.
    ///
    /// Return whether this admission round ended. An unresolved selected admission retains
    /// both frontier and cursor across another source sweep. The caller still restarts source
    /// traversal and continues servicing all original pending work. Call only after the entire
    /// captured source interval was visited, never after a source read refusal or partial pass.
    pub(super) fn finish_sweep(&mut self) -> bool {
        if self.selected.is_some() && !cfg!(all(test, sumeragi_daemon_mutation = "HC488")) {
            return false;
        }
        self.frontier = None;
        self.cursor = None;
        self.selected = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(height: u64, ordinal: u64, id: u8) -> AdmissionPosition {
        AdmissionPosition {
            height,
            ordinal,
            plan: [id; 32],
        }
    }

    // Only a deterministic scheduling owner, not a finality or proof substitute. Pending
    // service precedes fresh discovery and completion removes its finite slot immediately.
    // This deliberately exercises a release between pending recovery and source admission.
    #[derive(Default)]
    struct OneSlot {
        pending: Option<(AdmissionPosition, usize)>,
        admissions: Vec<AdmissionPosition>,
        completed: Vec<AdmissionPosition>,
    }

    impl OneSlot {
        fn service(&mut self) {
            if let Some((position, visits)) = &mut self.pending {
                *visits += 1;
                if *visits == 24 {
                    self.completed.push(*position);
                    self.pending = None;
                }
            }
        }
        fn visit(&mut self, planner: &mut FreshAdmissions, position: AdmissionPosition) {
            if self.pending.is_some_and(|(pending, _)| pending == position) {
                planner.admitted(position);
                return;
            }
            if planner.consider(position, true) && self.pending.is_none() {
                self.pending = Some((position, 0));
                self.admissions.push(position);
                assert!(planner.admitted(position));
            }
        }
    }

    fn exercise_pair(first: AdmissionPosition, next: AdmissionPosition) {
        let mut planner = FreshAdmissions::default();
        let mut owner = OneSlot::default();
        for _ in 0..49 {
            owner.service();
            planner.frontier(next.height);
            owner.visit(&mut planner, first);
            owner.visit(&mut planner, next);
            planner.finish_sweep();
        }
        assert_eq!(
            owner.admissions.get(..2),
            Some([first, next].as_slice()),
            "completed early source must not reacquire before the waiting later source"
        );
        assert!(
            owner.completed.contains(&next),
            "waiting source must finish its original 24 visits"
        );
    }

    #[test]
    fn completed_early_source_cannot_starve_later_pending_admission() {
        exercise_pair(position(70, 0, 1), position(80, 0, 2));
    }

    #[test]
    fn same_height_original_ordinals_receive_fair_admission() {
        exercise_pair(position(70, 3, 1), position(70, 4, 2));
    }

    #[test]
    fn growing_source_frontier_cannot_move_waiting_admission() {
        let first = position(70, 0, 1);
        let next = position(80, 0, 2);
        let mut planner = FreshAdmissions::default();
        let mut owner = OneSlot::default();
        for sweep in 0..25 {
            owner.service();
            assert_eq!(
                planner.frontier(80 + sweep),
                80,
                "fresh admission must retain its original finite frontier through refusal"
            );
            owner.visit(&mut planner, first);
            owner.visit(&mut planner, next);
            assert!(!planner.consider(position(81 + sweep, 0, 3), true));
            let ended = planner.finish_sweep();
            assert_eq!(ended, sweep == 24);
        }
        assert_eq!(owner.admissions, [first, next]);
        assert_eq!(planner.frontier(200), 200);
    }

    #[test]
    fn restart_services_existing_pending_before_fresh_capacity_is_available() {
        let first = position(70, 0, 1);
        let restored = position(80, 0, 2);
        let mut planner = FreshAdmissions::default();
        let mut owner = OneSlot {
            pending: Some((restored, 17)),
            ..OneSlot::default()
        };
        for _ in 0..7 {
            owner.service();
            planner.frontier(80);
            owner.visit(&mut planner, first);
            owner.visit(&mut planner, restored);
            planner.finish_sweep();
        }
        assert_eq!(owner.completed, [restored]);
        assert_eq!(owner.admissions, [first]);
        assert_eq!(owner.pending, Some((first, 0)));
    }

    #[test]
    fn future_signed_wake_does_not_reserve_or_block_ready_work() {
        let future = position(70, 0, 1);
        let ready = position(80, 0, 2);
        let mut planner = FreshAdmissions::default();
        planner.frontier(80);
        assert!(!planner.consider(future, false));
        assert!(planner.consider(ready, true));
        assert!(planner.admitted(ready));
        assert!(planner.finish_sweep());
        planner.frontier(80);
        assert!(planner.consider(future, true));
    }

    #[test]
    fn selected_expiry_survives_ready_decision_and_later_admission_refusal() {
        let expiry = position(70, 0, 1);
        let later = position(80, 0, 2);
        let mut planner = FreshAdmissions::default();
        planner.frontier(80);
        for _ in 0..24 {
            assert!(planner.consider(expiry, true));
            assert!(!planner.consider(later, true));
            assert!(!planner.finish_sweep());
        }
        // The expiry observer may now return Ready or Decided and clear its internal active
        // cursor. That never acknowledges an ordinary slot which still refused admission.
        assert!(!planner.finish_sweep());
        assert!(planner.consider(expiry, true));
        assert!(!planner.admitted(AdmissionPosition {
            plan: [9; 32],
            ..expiry
        }));
        assert!(!planner.consider(later, true));
        assert!(planner.admitted(expiry));
        assert!(!planner.consider(expiry, true));
        assert!(planner.consider(later, true));
    }

    #[test]
    fn reserve_before_expiry_keeps_original_pending_owner_independent() {
        let expiry = position(70, 0, 1);
        let later = position(80, 0, 2);
        let mut planner = FreshAdmissions::default();
        let mut owner = OneSlot::default();
        planner.frontier(80);
        owner.visit(&mut planner, expiry);
        owner.visit(&mut planner, later);
        for _ in 0..24 {
            assert!(!planner.finish_sweep());
            assert_eq!(planner.frontier(100), 80);
            owner.service();
            owner.visit(&mut planner, expiry);
            owner.visit(&mut planner, later);
        }
        assert_eq!(owner.admissions, [expiry, later]);
    }

    #[test]
    fn exact_selection_and_complete_finite_traversal_control_cursor_reset() {
        let first = position(70, 0, 1);
        let later = position(70, 1, 2);
        let mut planner = FreshAdmissions::default();
        assert!(!planner.consider(first, true));
        assert!(!planner.admitted(first));
        assert!(!planner.skip(first));
        assert_eq!(planner.frontier(70), 70);
        assert!(!planner.consider(position(1, 0, 4), true));
        assert!(!planner.consider(position(71, 0, 4), true));
        assert!(planner.consider(first, true));
        assert!(!planner.skip(later));
        assert!(planner.skip(first));
        assert!(!planner.consider(first, true));
        assert!(planner.consider(later, true));
        assert!(planner.admitted(later));
        assert!(planner.finish_sweep());
        assert_eq!(planner.frontier(90), 90);
        assert!(planner.consider(first, true));
    }

    #[test]
    fn empty_source_round_can_advance_without_allocating_scheduling_history() {
        let mut planner = FreshAdmissions::default();
        assert_eq!(planner.frontier(1), 1);
        assert!(!planner.consider(position(2, 0, 1), true));
        assert!(planner.finish_sweep());
        assert_eq!(planner.frontier(2), 2);
        assert!(planner.consider(position(2, 0, 1), true));
        assert!(!planner.finish_sweep());
    }
}
