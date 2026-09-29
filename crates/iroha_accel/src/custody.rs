//! Driver-independent work state used at the native enqueue/completion boundary.

/// Exact-stream state. A native failure never acts as completion evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Prepared,
    Pending,
    Complete,
}

impl Phase {
    pub(crate) fn begin(&mut self, usable: bool) -> bool {
        if !usable {
            return false;
        }
        *self = Self::Pending;
        true
    }

    pub(crate) fn complete(&mut self, uncertain: bool) -> bool {
        if *self != Self::Pending || uncertain {
            return false;
        }
        *self = Self::Complete;
        true
    }

    pub(crate) fn may_publish(self, usable: bool) -> bool {
        self != Self::Pending && usable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_enqueue_preserves_the_prior_phase() {
        let mut phase = Phase::Prepared;
        assert!(!phase.begin(false));
        assert_eq!(phase, Phase::Prepared);
    }

    #[test]
    fn uncertain_or_pending_work_cannot_publish() {
        let mut phase = Phase::Prepared;
        assert!(phase.begin(true));
        assert!(!phase.may_publish(true));
        assert!(!phase.complete(true));
        assert_eq!(phase, Phase::Pending);
        assert!(!phase.may_publish(false));
    }

    #[test]
    fn exact_completion_can_publish_then_next_enqueue_removes_permission() {
        let mut phase = Phase::Prepared;
        assert!(phase.begin(true));
        assert!(phase.complete(false));
        assert!(phase.may_publish(true));
        assert!(!phase.may_publish(false));
        assert!(phase.begin(true));
        assert!(!phase.may_publish(true));
    }

    #[test]
    fn unsubmitted_work_is_not_a_completion_witness() {
        let mut phase = Phase::Prepared;
        assert!(!phase.complete(false));
        assert_eq!(phase, Phase::Prepared);
    }
}
