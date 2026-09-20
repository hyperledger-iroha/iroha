//! Fixed public causes for a Torii GET `/status` unavailable response.

/// Recognized GET `/status` HTTP 503 reason carried by `x-iroha-reject-code`.
///
/// These codes describe producer failure classes, not transaction outcomes or
/// retry instructions. Unrecognized or absent codes are unclassified by clients.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusFailureReason {
    /// Telemetry is disabled for the snapshot owner.
    Disabled,
    /// The bounded telemetry mailbox could not admit the request.
    MailboxUnavailable,
    /// The telemetry actor ended without a response.
    ActorClosed,
    /// The whole status service deadline elapsed; this does not name a lock timeout.
    DeadlineElapsed,
    /// State target or journal capture was unavailable; several causes share this reason.
    StateUnavailable,
    /// The classified State journal checkpoint changed.
    CheckpointChanged,
    /// An applied block was missing from the required Kura prefix.
    MissingBlock,
    /// The Kura block sequence differed from the captured State journal.
    JournalMismatch,
    /// A classified status counter overflowed.
    CounterOverflow,
    /// Status counters differed from the owned classified prefix.
    CounterMismatch,
    /// The classified height differed from the reply's owned State target.
    MetricsStale,
    /// The active telemetry visibility policy does not permit this endpoint.
    ProfileRestricted,
}

impl StatusFailureReason {
    /// Return the exact public error-envelope and reject-header code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Disabled => "status_telemetry_disabled",
            Self::MailboxUnavailable => "status_mailbox_unavailable",
            Self::ActorClosed => "status_actor_closed",
            Self::DeadlineElapsed => "status_deadline_elapsed",
            Self::StateUnavailable => "status_state_unavailable",
            Self::CheckpointChanged => "status_checkpoint_changed",
            Self::MissingBlock => "status_missing_block",
            Self::JournalMismatch => "status_journal_mismatch",
            Self::CounterOverflow => "status_counter_overflow",
            Self::CounterMismatch => "status_counter_mismatch",
            Self::MetricsStale => "status_metrics_stale",
            Self::ProfileRestricted => "telemetry_profile_restricted",
        }
    }

    /// Recognize one exact code without accepting aliases or arbitrary text.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "status_telemetry_disabled" => Some(Self::Disabled),
            "status_mailbox_unavailable" => Some(Self::MailboxUnavailable),
            "status_actor_closed" => Some(Self::ActorClosed),
            "status_deadline_elapsed" => Some(Self::DeadlineElapsed),
            "status_state_unavailable" => Some(Self::StateUnavailable),
            "status_checkpoint_changed" => Some(Self::CheckpointChanged),
            "status_missing_block" => Some(Self::MissingBlock),
            "status_journal_mismatch" => Some(Self::JournalMismatch),
            "status_counter_overflow" => Some(Self::CounterOverflow),
            "status_counter_mismatch" => Some(Self::CounterMismatch),
            "status_metrics_stale" => Some(Self::MetricsStale),
            "telemetry_profile_restricted" => Some(Self::ProfileRestricted),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StatusFailureReason;

    #[test]
    fn status_failure_codes_are_exact_and_distinct() {
        let cases = [
            (StatusFailureReason::Disabled, "status_telemetry_disabled"),
            (
                StatusFailureReason::MailboxUnavailable,
                "status_mailbox_unavailable",
            ),
            (StatusFailureReason::ActorClosed, "status_actor_closed"),
            (
                StatusFailureReason::DeadlineElapsed,
                "status_deadline_elapsed",
            ),
            (
                StatusFailureReason::StateUnavailable,
                "status_state_unavailable",
            ),
            (
                StatusFailureReason::CheckpointChanged,
                "status_checkpoint_changed",
            ),
            (StatusFailureReason::MissingBlock, "status_missing_block"),
            (
                StatusFailureReason::JournalMismatch,
                "status_journal_mismatch",
            ),
            (
                StatusFailureReason::CounterOverflow,
                "status_counter_overflow",
            ),
            (
                StatusFailureReason::CounterMismatch,
                "status_counter_mismatch",
            ),
            (StatusFailureReason::MetricsStale, "status_metrics_stale"),
            (
                StatusFailureReason::ProfileRestricted,
                "telemetry_profile_restricted",
            ),
        ];
        for (index, (reason, code)) in cases.iter().enumerate() {
            assert_eq!(reason.code(), *code);
            assert_eq!(StatusFailureReason::from_code(code), Some(*reason));
            assert!(cases[..index].iter().all(|(_, previous)| previous != code));
        }
    }

    #[test]
    fn status_failure_codes_do_not_accept_unclassified_input_as_a_reason() {
        for code in [
            "",
            "unclassified",
            "status_metrics_unavailable",
            " status_state_unavailable",
            "status_state_unavailable ",
            "STATUS_STATE_UNAVAILABLE",
            "status_state_unavailable,status_state_unavailable",
        ] {
            assert_eq!(StatusFailureReason::from_code(code), None);
        }
    }
}
