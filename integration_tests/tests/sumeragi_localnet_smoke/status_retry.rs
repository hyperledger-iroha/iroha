//! Retry only explicit status snapshot contention inside the caller's existing deadline.

use std::{future::Future, time::Duration};

use eyre::Result;

/// The caller must bound the complete read, including these producer-requested waits.
pub(super) async fn read<T, F>(mut fetch: impl FnMut() -> F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    loop {
        match fetch().await {
            Ok(status) => return Ok(status),
            Err(error) => {
                let Some(iroha::Error::StatusUnavailable {
                    reason: Some(iroha::StatusFailureReason::StateBusy),
                    retry_after,
                }) = error.downcast_ref::<iroha::Error>()
                else {
                    return Err(error);
                };
                let delay = retry_after
                    .unwrap_or_default()
                    .max(Duration::from_millis(200));
                tokio::time::sleep(delay).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::ready;

    #[tokio::test]
    async fn contention_retries_until_a_fresh_response() {
        let mut attempts = 0;
        let actual = read(|| {
            attempts += 1;
            ready(if attempts == 1 {
                Err(iroha::Error::StatusUnavailable {
                    reason: Some(iroha::StatusFailureReason::StateBusy),
                    retry_after: None,
                }
                .into())
            } else {
                Ok(7)
            })
        })
        .await
        .unwrap();
        assert_eq!((attempts, actual), (2, 7));
    }

    #[tokio::test]
    async fn unknown_or_non_contention_failures_are_not_retried() {
        for reason in [None, Some(iroha::StatusFailureReason::StateUnavailable)] {
            let mut attempts = 0;
            let failure = read::<(), _>(|| {
                attempts += 1;
                ready(Err(iroha::Error::StatusUnavailable {
                    reason,
                    retry_after: None,
                }
                .into()))
            })
            .await
            .expect_err("only explicit contention is retryable");
            assert_eq!(attempts, 1);
            assert!(matches!(failure.downcast_ref::<iroha::Error>(),
                Some(iroha::Error::StatusUnavailable { reason: actual, .. }) if *actual == reason));
        }
    }

    #[tokio::test]
    async fn producer_retry_after_cannot_extend_the_callers_deadline() {
        let mut attempts = 0;
        let outcome = tokio::time::timeout(
            Duration::from_millis(20),
            read::<(), _>(|| {
                attempts += 1;
                ready(Err(iroha::Error::StatusUnavailable {
                    reason: Some(iroha::StatusFailureReason::StateBusy),
                    retry_after: Some(Duration::from_secs(60)),
                }
                .into()))
            }),
        )
        .await;
        assert!(outcome.is_err());
        assert_eq!(attempts, 1);
    }
}
