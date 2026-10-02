//! Pure transition admission, not a clock, proof, filesystem or hardware grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Prepare = 1,
    Key = 2,
    RawIssuer = 3,
    Possession = 4,
    Integrity = 5,
    Receipt = 6,
}
impl Step {
    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            1 => Self::Prepare,
            2 => Self::Key,
            3 => Self::RawIssuer,
            4 => Self::Possession,
            5 => Self::Integrity,
            6 => Self::Receipt,
            _ => return None,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) struct Lifecycle {
    completed: u8,
    pending: Option<Step>,
    cancel_requested: bool,
    disposed: bool,
}
impl Lifecycle {
    pub(crate) fn require_start(&self, s: Step) -> Result<(), ()> {
        if self.disposed
            || self.cancel_requested
            || self.pending.is_some()
            || s as u8 != self.completed + 1
        {
            Err(())
        } else {
            Ok(())
        }
    }
    pub(crate) fn invoked(&mut self, s: Step) -> Result<(), ()> {
        self.require_start(s)?;
        self.pending = Some(s);
        Ok(())
    }
    pub(crate) fn captured(&mut self, s: Step) -> Result<(), ()> {
        if self.disposed || self.pending != Some(s) {
            return Err(());
        }
        self.pending = None;
        self.completed = s as u8;
        Ok(())
    }
    pub(crate) fn cancel(&mut self) -> Result<(), ()> {
        if self.disposed || self.cancel_requested {
            return Err(());
        }
        self.cancel_requested = true;
        Ok(())
    }
    pub(crate) fn dispose(&mut self) -> Result<(), ()> {
        if self.disposed
            || self.pending.is_some()
            || !(self.cancel_requested || self.completed == 6)
        {
            return Err(());
        }
        self.disposed = true;
        Ok(())
    }
    pub(crate) fn pending(&self) -> Option<Step> {
        self.pending
    }
    pub(crate) fn completed(&self) -> u8 {
        self.completed
    }
    pub(crate) fn cancelled(&self) -> bool {
        self.cancel_requested
    }
    pub(crate) fn disposed(&self) -> bool {
        self.disposed
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn advance(l: &mut Lifecycle, s: Step) {
        l.invoked(s).unwrap();
        l.captured(s).unwrap();
    }
    #[test]
    fn unknown_is_never_reinvoked() {
        let mut l = Lifecycle::default();
        l.invoked(Step::Prepare).unwrap();
        assert!(l.invoked(Step::Prepare).is_err());
        assert!(l.invoked(Step::Key).is_err());
    }
    #[test]
    fn cancel_does_not_mean_pending_call_ceased() {
        let mut l = Lifecycle::default();
        l.invoked(Step::Prepare).unwrap();
        l.cancel().unwrap();
        assert!(l.dispose().is_err());
        l.captured(Step::Prepare).unwrap();
        l.dispose().unwrap();
    }
    #[test]
    fn cancellation_accepts_original_capture_and_blocks_new_effect() {
        let mut l = Lifecycle::default();
        advance(&mut l, Step::Prepare);
        l.invoked(Step::Key).unwrap();
        l.cancel().unwrap();
        l.captured(Step::Key).unwrap();
        assert!(l.invoked(Step::RawIssuer).is_err());
    }
    #[test]
    fn exact_sequence_is_mandatory() {
        let mut l = Lifecycle::default();
        assert!(l.invoked(Step::Key).is_err());
        advance(&mut l, Step::Prepare);
        assert!(l.invoked(Step::Possession).is_err());
    }
    #[test]
    fn no_capture_without_matching_fence() {
        let mut l = Lifecycle::default();
        assert!(l.captured(Step::Prepare).is_err());
        l.invoked(Step::Prepare).unwrap();
        assert!(l.captured(Step::Key).is_err());
    }
    #[test]
    fn complete_receipt_can_dispose_without_key_deletion() {
        let mut l = Lifecycle::default();
        for tag in 1..=6 {
            advance(&mut l, Step::from_tag(tag).unwrap());
        }
        l.dispose().unwrap();
        assert_eq!(l.completed(), 6);
    }
    #[test]
    fn replay_exact_unknown_preserves_pending() {
        let mut a = Lifecycle::default();
        advance(&mut a, Step::Prepare);
        advance(&mut a, Step::Key);
        a.invoked(Step::RawIssuer).unwrap();
        let b = a;
        assert_eq!(b.pending(), Some(Step::RawIssuer));
        assert!(b.require_start(Step::RawIssuer).is_err());
    }
    #[test]
    fn disposed_is_terminal_for_all_steps() {
        let mut l = Lifecycle::default();
        l.cancel().unwrap();
        l.dispose().unwrap();
        for t in 1..=6 {
            assert!(l.invoked(Step::from_tag(t).unwrap()).is_err());
        }
        assert!(l.cancel().is_err());
        assert!(l.dispose().is_err());
    }
    #[test]
    fn no_timer_transition_exists() {
        let l = Lifecycle::default();
        assert_eq!(l.completed(), 0);
        assert!(!l.disposed());
        assert!(!l.cancelled());
        assert_eq!(l.pending(), None);
    }
    #[test]
    fn every_pending_step_survives_cancel_as_unknown() {
        for t in 1..=6 {
            let mut l = Lifecycle::default();
            for p in 1..t {
                advance(&mut l, Step::from_tag(p).unwrap());
            }
            let s = Step::from_tag(t).unwrap();
            l.invoked(s).unwrap();
            l.cancel().unwrap();
            assert_eq!(l.pending(), Some(s));
            assert!(l.dispose().is_err());
            assert!(l.invoked(s).is_err());
        }
    }
    #[test]
    fn tag_domain_closed() {
        assert_eq!(Step::from_tag(0), None);
        assert_eq!(Step::from_tag(7), None);
        assert_eq!(Step::from_tag(255), None);
    }
}
