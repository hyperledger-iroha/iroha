//! Await an active admission owner without dispatching or renewing the exact request.

use std::time::Duration;

use iroha_core::sumeragi::QueuePlanInputCapacityErrorV1;

#[derive(Debug, thiserror::Error)]
pub(super) enum WaitError {
    #[error(transparent)]
    Capacity(QueuePlanInputCapacityErrorV1),
    #[error("{0}")]
    Deadline(&'static str),
}

/// Bound waiting by the original local clock and the unchanged authenticated wire clock.
pub(super) fn remaining(
    deadline: tokio::time::Instant,
    deadline_unix_ms: u64,
) -> Result<Duration, &'static str> {
    let absolute = super::validate_torii_proxy_deadline(deadline_unix_ms)?;
    let local = deadline
        .checked_duration_since(tokio::time::Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or("original Torii proxy deadline expired awaiting an active admission owner")?;
    Ok(absolute.min(local))
}

/// An unusable retry budget cannot disprove a previous exact durable admission.
pub(super) fn deadline_response(
    request: &super::ToriiProxyRequestKindV1,
    reason: impl Into<String>,
) -> super::Response {
    if let super::ToriiProxyRequestKindV1::SubmitTransaction {
        transaction,
        admission: super::ToriiProxyTransactionAdmissionV1::QueuePlanSynced,
        ..
    } = request
    {
        // Bind uncertainty to the actual entrypoint, never an unverified claim.
        super::queue_plan_outcome_unknown_response(
            transaction.hash(),
            super::signed_transaction_hash_for_entrypoint(transaction),
            reason,
        )
    } else {
        super::torii_proxy_error_response(
            super::StatusCode::REQUEST_TIMEOUT,
            "proxy_deadline_exceeded",
            reason,
        )
    }
}

/// Keep the caller's exact input and memory reservation on its cancellable future stack.
/// Only closed live ingress is awaited; missing recovery, fail-stop, invalid inputs and
/// capacity failures return immediately. Each wake rechecks the complete current owner.
pub(super) async fn wait(
    mut check: impl FnMut() -> Result<(), QueuePlanInputCapacityErrorV1>,
    remaining: impl Fn() -> Result<Duration, &'static str>,
) -> Result<(), WaitError> {
    loop {
        remaining().map_err(WaitError::Deadline)?;
        match check() {
            Ok(()) => return remaining().map(|_| ()).map_err(WaitError::Deadline),
            Err(QueuePlanInputCapacityErrorV1::Inactive) => {}
            Err(error) => return Err(WaitError::Capacity(error)),
        }
        let budget = remaining().map_err(WaitError::Deadline)?;
        // The Core check returns Inactive before canonical sizing, so polling
        // a closed owner neither re-encodes inputs nor caches capacity authority.
        tokio::time::sleep(budget.min(Duration::from_millis(25))).await;
    }
}

#[cfg(test)]
mod tests;
