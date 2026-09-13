//! Shared finality polling state and typed, read-only HTTP backpressure.
//! Unresolved deadlines retain the exact public transaction hash and observation count.

use std::time::{Duration, Instant};

use eyre::{Result, eyre};

use super::{
    HashOf, PipelineTransactionStatusResponse, Response, SignedTransaction, TransactionWaitOptions,
    TransactionWaitOutcome, TxConfirmationStatus, transaction_wait_outcome,
    tx_confirmation_final_report, tx_confirmation_unresolved_final_report,
    validate_global_pipeline_status_response,
};

/// Exact authoritative proof that a signed transaction can no longer become `Applied`.
///
/// Only global, state-resolved `Rejected` or `Expired` responses qualify. This retains the full
/// typed rejection payload so durable consumers never classify errors by display text.
#[derive(
    Debug, Clone, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
pub struct TransactionFinalityFailure {
    response: PipelineTransactionStatusResponse,
}
impl TransactionFinalityFailure {
    /// Classify one exact global status response using the SDK's canonical finality rules.
    ///
    /// # Errors
    /// Rejects malformed, hash-mismatched, wrong-scope, or unsupported status responses.
    pub fn from_response(
        hash: HashOf<SignedTransaction>,
        response: PipelineTransactionStatusResponse,
    ) -> Result<Option<Self>> {
        let status = validate_global_pipeline_status_response(&response, hash)?;
        Ok((response.resolved_from == "state"
            && matches!(
                status,
                TxConfirmationStatus::Rejected(_) | TxConfirmationStatus::Expired
            ))
        .then_some(Self { response }))
    }
    /// Return the exact hash, status/reason, scope, resolution source, and optional ledger height.
    #[must_use]
    pub const fn response(&self) -> &PipelineTransactionStatusResponse {
        &self.response
    }
    /// Validate retained failure evidence against its exact signed transaction hash.
    ///
    /// # Errors
    /// Rejects retained evidence that is not a canonical state-resolved global fixed failure.
    pub fn validate_for_hash(&self, hash: HashOf<SignedTransaction>) -> Result<()> {
        if Self::from_response(hash, self.response.clone())?.is_none() {
            return Err(eyre!(
                "retained status is not a global state-resolved fixed terminal failure"
            ));
        }
        Ok(())
    }
}
impl std::fmt::Display for TransactionFinalityFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "transaction {} reached state-resolved fixed terminal failure status `{}`; status={:?}",
            self.response.hash, self.response.status.kind, self.response.status
        )
    }
}
impl std::error::Error for TransactionFinalityFailure {}

#[derive(Debug, thiserror::Error)]
#[error("Failed to get pipeline transaction status: 429 Too Many Requests {body}")]
struct Backpressure {
    retry_after: Option<Duration>,
    body: String,
}

/// Preserve backpressure as an error for one-shot reads; only finality waits retry it.
pub(super) fn backpressure_response(response: &Response<Vec<u8>>) -> eyre::Report {
    let mut headers = response.headers().get_all(http::header::RETRY_AFTER).iter();
    let retry_after = match headers.next() {
        None => None,
        Some(value) => {
            // Torii emits delta seconds. Ambiguous or malformed retry instructions
            // fail explicitly instead of guessing a shorter delay.
            let seconds = value.to_str().ok().filter(|value| {
                !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
            });
            let Some(seconds) = seconds.and_then(|value| value.parse::<u64>().ok()) else {
                return eyre!("pipeline status 429 has invalid Retry-After delta seconds");
            };
            if headers.next().is_some() {
                return eyre!("pipeline status 429 has multiple Retry-After values");
            }
            Some(Duration::from_secs(seconds))
        }
    };
    Backpressure {
        retry_after,
        body: String::from_utf8_lossy(response.body()).into_owned(),
    }
    .into()
}

/// One finality decision rule shared by blocking and asynchronous transports.
pub(super) struct PollState {
    hash: HashOf<SignedTransaction>,
    options: TransactionWaitOptions,
    started: Instant,
    attempts: u64,
    last_status: Option<String>,
    last_backpressure: Option<eyre::Report>,
    delay: Duration,
}

impl PollState {
    pub(super) fn new(
        hash: HashOf<SignedTransaction>,
        options: TransactionWaitOptions,
    ) -> Result<Self> {
        if options.poll_interval.is_zero() {
            return Err(eyre!(
                "transaction wait poll_interval must be greater than zero"
            ));
        }
        Ok(Self {
            hash,
            options,
            started: Instant::now(),
            attempts: 0,
            last_status: None,
            last_backpressure: None,
            delay: options.poll_interval,
        })
    }

    pub(super) fn begin_poll(&mut self) -> Result<()> {
        // Preserve the initial observation even for a zero timeout, but never
        // issue an extra read after sleeping to the confirmation deadline.
        if self.attempts != 0 && self.started.elapsed() >= self.options.timeout {
            return Err(self.timeout_error());
        }
        self.attempts = self.attempts.saturating_add(1);
        Ok(())
    }

    pub(super) fn observe(
        &mut self,
        result: Result<Option<PipelineTransactionStatusResponse>>,
    ) -> Result<Option<TransactionWaitOutcome>> {
        self.delay = self.options.poll_interval;
        let response = match result {
            Ok(response) => {
                self.last_backpressure = None;
                response
            }
            Err(error) => {
                let Some(backpressure) = error.downcast_ref::<Backpressure>() else {
                    return Err(error);
                };
                self.delay = self.delay.max(backpressure.retry_after.unwrap_or_default());
                self.last_backpressure = Some(error);
                return Ok(None);
            }
        };
        let Some(response) = response else {
            return Ok(None);
        };
        let kind = response.status.kind.as_str();
        self.last_status = Some(kind.to_owned());
        match validate_global_pipeline_status_response(&response, self.hash)? {
            TxConfirmationStatus::Applied if response.resolved_from == "state" => Ok(Some(
                transaction_wait_outcome(response, self.attempts, self.started.elapsed()),
            )),
            TxConfirmationStatus::Rejected(_) | TxConfirmationStatus::Expired
                if response.resolved_from == "state" =>
            {
                let failure = TransactionFinalityFailure::from_response(self.hash, response)?
                    .ok_or_else(|| {
                        eyre!("fixed terminal status failed canonical classification")
                    })?;
                Err(tx_confirmation_final_report(failure.into()))
            }
            _ => Ok(None),
        }
    }

    pub(super) fn next_delay(&mut self) -> Result<Duration> {
        let elapsed = self.started.elapsed();
        if elapsed >= self.options.timeout {
            return Err(self.timeout_error());
        }
        Ok(self.delay.min(self.options.timeout.saturating_sub(elapsed)))
    }

    fn timeout_error(&mut self) -> eyre::Report {
        let status = self.last_status.as_deref().unwrap_or("not_observed");
        let message = format!(
            "transaction {} did not reach state-resolved Applied within {} ms; last_status={status}; attempts={}",
            self.hash,
            self.options.timeout.as_millis(),
            self.attempts
        );
        let report = if let Some(error) = self.last_backpressure.take() {
            error.wrap_err(message)
        } else {
            eyre!(message)
        };
        tx_confirmation_unresolved_final_report(report)
    }
}
