//! Service-local continuation of one signed Check through its original deadline.
//!
//! No State view, execution lease or native mutex crosses the wait. The move-only
//! Core failure stays on the stack; this boundary returns only after binding,
//! a terminal rejection, or expiry. It neither signs nor manufactures a pool
//! notification for non-capacity errors.

use super::StreamTokenIssuerError;
use iroha_core::query::stream_token_authority::observation::{
    PendingStreamTokenCheckV1, StreamTokenCheckAttemptFailureV1, StreamTokenCheckBindingFailureV1,
    StreamTokenEligibilityTimeIntervalV1, StreamTokenObservationErrorV1,
    VerifiedStreamTokenCheckV1,
};
use std::time::{Duration, Instant};

trait CheckAttempt: Sized {
    fn deadline(&self) -> Instant;
    fn retryable(&self) -> bool;
}
trait BindingAttempt: CheckAttempt {
    type Output;
    fn retry(self) -> Result<Self::Output, Self>;
}

impl CheckAttempt for StreamTokenCheckBindingFailureV1 {
    fn deadline(&self) -> Instant {
        self.deadline()
    }
    fn retryable(&self) -> bool {
        self.error().is_retryable()
    }
}
impl BindingAttempt for StreamTokenCheckBindingFailureV1 {
    type Output = PendingStreamTokenCheckV1;
    fn retry(self) -> Result<Self::Output, Self> {
        self.retry()
    }
}
impl CheckAttempt for StreamTokenCheckAttemptFailureV1 {
    fn deadline(&self) -> Instant {
        self.deadline()
    }
    fn retryable(&self) -> bool {
        self.is_retryable()
    }
}

pub(super) fn finish(
    result: Result<PendingStreamTokenCheckV1, StreamTokenCheckBindingFailureV1>,
) -> Result<PendingStreamTokenCheckV1, StreamTokenIssuerError> {
    drive(
        result,
        BindingAttempt::retry,
        Instant::now,
        std::thread::sleep,
    )
}

/// Finalize the already-bound Check with fresh UTC on every retry and its original deadline.
pub(super) fn verify(
    pending: PendingStreamTokenCheckV1,
    mut clock: impl FnMut()
        -> Result<StreamTokenEligibilityTimeIntervalV1, StreamTokenObservationErrorV1>,
) -> Result<VerifiedStreamTokenCheckV1, StreamTokenIssuerError> {
    let mut attempt = |pending: PendingStreamTokenCheckV1| pending.verify_finalized(&mut clock);
    drive(
        attempt(pending),
        |failure| attempt(failure.into_pending()),
        Instant::now,
        std::thread::sleep,
    )
}

fn drive<T, F: CheckAttempt>(
    mut result: Result<T, F>,
    mut retry: impl FnMut(F) -> Result<T, F>,
    mut now: impl FnMut() -> Instant,
    mut wait: impl FnMut(Duration),
) -> Result<T, StreamTokenIssuerError> {
    let mut backoff = Duration::from_millis(4);
    loop {
        let failure = match result {
            Ok(pending) => return Ok(pending),
            Err(failure) => failure,
        };
        let remaining = failure.deadline().saturating_duration_since(now());
        if !failure.retryable() || remaining.is_zero() {
            return Err(StreamTokenIssuerError::SignerFinalityUnavailable);
        }
        // Backoff is bounded by the owner's unchanged absolute deadline. This
        // is a local service wait, not a fabricated allocation release source.
        wait(backoff.min(remaining));
        if now() >= failure.deadline() {
            return Err(StreamTokenIssuerError::SignerFinalityUnavailable);
        }
        result = retry(failure);
        backoff = backoff.saturating_mul(2).min(Duration::from_millis(64));
    }
}

#[cfg(test)]
#[path = "signer_binding_tests.rs"]
mod tests;
