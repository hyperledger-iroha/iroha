//! Authenticated terminal outcome retained across a pending local wallet cleanup failure.

use crate::OneCallPrivateSettlementError;
use iroha::{client::VerifiedPrivateSettlementOutcome, workflow::WorkflowProgressV1};

/// A terminal private payment whose original local wallet still needs reconciliation.
///
/// This value never reclassifies settled or aborted money as unresolved. The verified outcome and
/// exact optional carrier progress survive the local failure. Reopening the original operation
/// can retry only its existing local reconciliation; this value grants no new spending or
/// carrier-signing authority. Neither this type nor its verified outcome has a decoder.
pub struct PrivateSettlementReconciliationPending {
    outcome: VerifiedPrivateSettlementOutcome,
    progress: Option<WorkflowProgressV1>,
    cause: Box<OneCallPrivateSettlementError>,
}

impl PrivateSettlementReconciliationPending {
    pub(crate) fn new(
        outcome: VerifiedPrivateSettlementOutcome,
        progress: Option<WorkflowProgressV1>,
        cause: OneCallPrivateSettlementError,
    ) -> Self {
        Self {
            outcome,
            progress,
            cause: Box::new(cause),
        }
    }

    /// Borrow the independently verified original terminal outcome.
    #[must_use]
    pub const fn outcome(&self) -> &VerifiedPrivateSettlementOutcome {
        &self.outcome
    }

    /// Borrow the original carrier progress, preserving absence exactly as returned by recovery.
    #[must_use]
    pub const fn progress(&self) -> Option<&WorkflowProgressV1> {
        self.progress.as_ref()
    }

    /// Borrow the original typed local cleanup failure, without replacing its cause.
    #[must_use]
    pub fn cause(&self) -> &OneCallPrivateSettlementError {
        &self.cause
    }

    /// Recover the terminal outcome, original optional progress, and local failure owners.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        VerifiedPrivateSettlementOutcome,
        Option<WorkflowProgressV1>,
        OneCallPrivateSettlementError,
    ) {
        (self.outcome, self.progress, *self.cause)
    }
}

impl std::fmt::Display for PrivateSettlementReconciliationPending {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("private settlement has a verified terminal outcome; local wallet reconciliation pending")
    }
}

impl std::fmt::Debug for PrivateSettlementReconciliationPending {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(
            "PrivateSettlementReconciliationPending(<verified terminal outcome; local custody pending>)",
        )
    }
}

impl std::error::Error for PrivateSettlementReconciliationPending {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
