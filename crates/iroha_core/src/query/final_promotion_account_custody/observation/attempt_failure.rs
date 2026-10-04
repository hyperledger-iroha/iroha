//! Original signed custody survives every incomplete verification attempt.

use super::*;

/// An unchanged original signed Check and its local refusal or completed rejection.
#[must_use = "retain the original pending Check through local retry or explicit retirement"]
pub struct FinalPromotionAccountCheckAttemptFailureV1 {
    pub(super) error: crate::execution_attempt::ExecutionAttemptError<Error>,
    pub(super) pending: PendingFinalPromotionAccountCheckV1,
}
impl FinalPromotionAccountCheckAttemptFailureV1 {
    /// Borrow the original refusal; local pressure is never a finality rejection.
    pub fn error(&self) -> &crate::execution_attempt::ExecutionAttemptError<Error> {
        &self.error
    }
    /// Inspect only a completed semantic rejection.
    pub fn rejection(&self) -> Option<Error> {
        match &self.error {
            crate::execution_attempt::ExecutionAttemptError::Rejected(error) => Some(*error),
            _ => None,
        }
    }
    /// Whether the same original attempt may retry while its unchanged deadline remains live.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self.error,
            crate::execution_attempt::ExecutionAttemptError::Deferred(_)
        )
    }
    /// Original absolute deadline, including after late clock or current-row failure.
    pub fn deadline(&self) -> std::time::Instant {
        self.pending.deadline()
    }
    /// Borrow the original signed Check without cloning or authorizing replacement signing.
    pub fn signed_transaction(&self) -> &SignedTransaction {
        self.pending.signed_transaction()
    }
    /// Retry from original pending custody, never from a partial verified result.
    pub fn into_pending(self) -> PendingFinalPromotionAccountCheckV1 {
        self.pending
    }
}
impl std::fmt::Debug for FinalPromotionAccountCheckAttemptFailureV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FinalPromotionAccountCheckAttemptFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for FinalPromotionAccountCheckAttemptFailureV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.error, f)
    }
}
impl std::error::Error for FinalPromotionAccountCheckAttemptFailureV1 {}
