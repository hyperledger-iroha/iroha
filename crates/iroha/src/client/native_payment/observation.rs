//! Proof-gated carrier reads and original business receipt recovery.

use super::finality_custody::BoundNativePaymentFinality;
use super::*;
use crate::workflow::{WorkflowRunnerLease, WorkflowStorageError};
use crate::{
    query::QueryError,
    workflow::{CommittedOutcome, DispatchOutcome, TransactionIdentityV1, WorkflowLedger},
};
use iroha_data_model::{
    block::{BlockHeader, SignedBlock},
    executor::ValidationFail,
    query::{
        CommittedTransaction,
        error::{FindError, QueryExecutionFail},
        settlement::FindSettlementReceiptById,
    },
    sumeragi_finality::{
        FinalityError, FinalityReadError, SumeragiFinalityProof, VerifiedSumeragiBlock,
    },
    transaction::error::TransactionRejectionReason,
};
use std::{num::NonZeroU64, sync::Weak};
mod finality;

pub(super) struct ObservationCustody {
    _lease: Mutex<Box<dyn WorkflowRunnerLease>>,
}
pub(super) enum PaymentProof {
    Response(super::super::SumeragiFinalityProofResponse),
    #[cfg(test)]
    Fixture(Box<SumeragiFinalityProof>),
}
impl PaymentProof {
    pub(super) fn decode_checked(
        self,
    ) -> Result<SumeragiFinalityProof, NativePaymentObservationError> {
        match self {
            Self::Response(response) => response
                .decode_checked()
                .map_err(NativePaymentObservationError::Transport),
            #[cfg(test)]
            Self::Fixture(proof) => Ok(*proof),
        }
    }
}

/// Original observation/verification failure. Local resource errors are not ledger rejection.
#[derive(Debug, thiserror::Error)]
pub enum NativePaymentObservationError {
    /// Existing typed singular-query failure.
    #[error(transparent)]
    Query(#[from] QueryError),
    /// Existing finality transport/codec failure, preserving the original error chain.
    #[error("finality/status transport failed: {0}")]
    Transport(#[source] eyre::Report),
    /// The native finality verifier's original typed error, including resource custody.
    #[error(transparent)]
    Finality(#[from] FinalityReadError),
    /// Exact transaction input/output membership authentication failed.
    #[error(transparent)]
    Execution(#[from] FinalityError),
    /// Complete payment, network or authenticated fee verification failed.
    #[error(transparent)]
    Receipt(#[from] super::super::PaymentReceiptVerificationError),
    /// Original canonical encoder failure.
    #[error("native payment encoding failed: {0}")]
    Encoding(#[source] norito::Error),
    /// Local bounded proof-read budget; retry with the retained verifier or a larger local budget.
    #[error("native payment proof-read budget exhausted; verification remains resumable")]
    ReadBudgetExhausted,
    /// No exact candidate is present at the selected locator.
    #[error("payment locator differs from its authenticated block or carrier")]
    CandidateMismatch,
    /// Authenticated original fees differ from the caller's exact independent authorization.
    #[error("original payment fee authorization differs from the retained request")]
    FeeAuthorizationMismatch,
    /// Stored completion changed or no longer matches the independently reverified original.
    #[error("stored payment completion differs from fresh verified evidence")]
    CompletionMismatch,
    /// A local worker or synchronization owner failed; no consensus conclusion was made.
    #[error(transparent)]
    Worker(#[from] super::super::NativeWorkerError),
    /// Original operation lease failure; no monetary outcome or retry authority is granted.
    #[error(transparent)]
    Custody(#[from] WorkflowStorageError),
    /// Original authenticated pending-checkpoint custody failure.
    #[error(transparent)]
    FinalityCustody(#[from] NativePaymentFinalityStorageError),
    /// The original synchronous in-memory owner became poisoned.
    #[error("payment observation synchronization owner failed")]
    Synchronization,
    /// The single admitted native worker for this operation is still occupied.
    #[error("payment observation worker capacity unavailable")]
    Capacity,
}

#[derive(Clone, Copy)]
pub(super) struct PaymentLocator {
    pub height: u64,
    pub block_hash: HashOf<BlockHeader>,
}
pub(super) enum CarrierLocation {
    Absent,
    Observed(Option<u64>),
}

pub(super) trait PaymentSource: Send + Sync {
    fn settlement<'a>(
        &'a self,
        id: &'a iroha_data_model::isi::SettlementId,
    ) -> WorkflowFuture<'a, Result<Option<PaymentLocator>, NativePaymentObservationError>>;
    fn carrier(
        &self,
        hash: HashOf<SignedTransaction>,
    ) -> WorkflowFuture<'_, Result<CarrierLocation, NativePaymentObservationError>>;
    fn proof(
        &self,
        height: NonZeroU64,
    ) -> WorkflowFuture<'_, Result<PaymentProof, NativePaymentObservationError>>;
}

pub(super) struct OnlinePaymentSource(pub AccountClient);
impl PaymentSource for OnlinePaymentSource {
    fn settlement<'a>(
        &'a self,
        id: &'a iroha_data_model::isi::SettlementId,
    ) -> WorkflowFuture<'a, Result<Option<PaymentLocator>, NativePaymentObservationError>> {
        Box::pin(async move {
            match self
                .0
                .query_single(FindSettlementReceiptById::new(id.clone()))
                .await
            {
                Ok(receipt) => Ok(Some(PaymentLocator {
                    height: receipt.block_height,
                    block_hash: receipt.block_hash,
                })),
                Err(QueryError::Validation(ValidationFail::QueryFailed(
                    QueryExecutionFail::Find(FindError::SettlementReceipt(missing)),
                ))) if &missing == id => Ok(None),
                Err(error) => Err(error.into()),
            }
        })
    }
    fn carrier(
        &self,
        hash: HashOf<SignedTransaction>,
    ) -> WorkflowFuture<'_, Result<CarrierLocation, NativePaymentObservationError>> {
        Box::pin(async move {
            let response = self
                .0
                .client()
                .fetch_transaction_status_response_global(hash)
                .await
                .map_err(NativePaymentObservationError::Transport)?;
            let Some(response) = response else {
                return Ok(CarrierLocation::Absent);
            };
            super::super::validate_global_pipeline_status_response(&response, hash)
                .map_err(NativePaymentObservationError::Transport)?;
            // Every status, including Applied, Rejected and Expired, selects a candidate only.
            Ok(CarrierLocation::Observed(response.status.block_height))
        })
    }
    fn proof(
        &self,
        height: NonZeroU64,
    ) -> WorkflowFuture<'_, Result<PaymentProof, NativePaymentObservationError>> {
        Box::pin(async move {
            self.0
                .client()
                .read_sumeragi_finality_proof_response_async(height, SumeragiRootScope::Global)
                .await
                .map(PaymentProof::Response)
                .map_err(NativePaymentObservationError::Transport)
        })
    }
}

pub(super) struct NativePaymentLedger {
    pub request: NativePaymentRequest,
    session: NativePaymentSession,
    dispatch: Arc<dyn WorkflowLedger>,
    source: Arc<dyn PaymentSource>,
    reads_remaining: AtomicUsize,
    receipt: Mutex<Option<VerifiedPaymentReceipt>>,
    error: Mutex<Option<NativePaymentObservationError>>,
    storage: Arc<dyn WorkflowStorage>,
    finality: BoundNativePaymentFinality,
    observation_lease: WorkflowId,
    observation: Mutex<Weak<ObservationCustody>>,
    worker: Arc<tokio::sync::Semaphore>,
}
fn lock<T>(
    value: &Mutex<T>,
) -> Result<std::sync::MutexGuard<'_, T>, NativePaymentObservationError> {
    value
        .lock()
        .map_err(|_| NativePaymentObservationError::Synchronization)
}
impl NativePaymentLedger {
    pub fn new(
        request: NativePaymentRequest,
        session: NativePaymentSession,
        dispatch: Arc<dyn WorkflowLedger>,
        source: Arc<dyn PaymentSource>,
        storage: Arc<dyn WorkflowStorage>,
        finality: BoundNativePaymentFinality,
    ) -> Self {
        let observation_lease = WorkflowId::from_bytes(domain_digest(
            b"iroha:native-payment:observation-worker:v1\0",
            &[request.intent.network_id.as_bytes(), &request.digest],
        ));
        Self {
            request,
            session,
            dispatch,
            source,
            reads_remaining: AtomicUsize::new(0),
            receipt: Mutex::new(None),
            error: Mutex::new(None),
            storage,
            finality,
            observation_lease,
            observation: Mutex::new(Weak::new()),
            worker: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
    pub fn begin_call(
        &self,
        reads: usize,
    ) -> Result<Arc<ObservationCustody>, NativePaymentObservationError> {
        let custody = Arc::new(ObservationCustody {
            _lease: Mutex::new(self.storage.acquire_runner(&self.observation_lease)?),
        });
        *lock(&self.observation)? = Arc::downgrade(&custody);
        self.reads_remaining.store(reads, Ordering::Relaxed);
        *lock(&self.error)? = None;
        *lock(&self.receipt)? = None;
        Ok(custody)
    }
    pub fn take_receipt(
        &self,
    ) -> Result<Option<VerifiedPaymentReceipt>, NativePaymentObservationError> {
        Ok(lock(&self.receipt)?.take())
    }
    pub fn take_error(
        &self,
    ) -> Result<Option<NativePaymentObservationError>, NativePaymentObservationError> {
        Ok(lock(&self.error)?.take())
    }

    async fn read_proof(&self, height: u64) -> Result<PaymentProof, NativePaymentObservationError> {
        let height =
            NonZeroU64::new(height).ok_or(NativePaymentObservationError::CandidateMismatch)?;
        self.reads_remaining
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
            .map_err(|_| NativePaymentObservationError::ReadBudgetExhausted)?;
        self.source.proof(height).await
    }
    fn check_fees(
        request: &NativePaymentRequest,
        receipt: &VerifiedPaymentReceipt,
    ) -> Result<(), NativePaymentObservationError> {
        let TransactionEntrypoint::External(transaction) = receipt.evidence().entrypoint() else {
            return Err(NativePaymentObservationError::CandidateMismatch);
        };
        if transaction.fee_payment_intent() != &request.fee_payment
            || transaction.authority() != request.intent.settlement_id.owner()
        {
            return Err(NativePaymentObservationError::FeeAuthorizationMismatch);
        }
        request
            .validate_payload(transaction.payload())
            .map_err(|_| NativePaymentObservationError::CandidateMismatch)?;
        Ok(())
    }
    pub async fn recover_business(
        &self,
        marker: Option<&NativePaymentCompletionV1>,
    ) -> Result<Option<VerifiedPaymentReceipt>, NativePaymentObservationError> {
        self.finality_business(marker).await
    }
    async fn read_carrier(
        &self,
        identity: &TransactionIdentityV1,
    ) -> Result<CommittedOutcome, NativePaymentObservationError> {
        let (outcome, receipt) = self.finality_carrier(*identity).await?;
        if let Some(receipt) = receipt {
            *lock(&self.receipt)? = Some(receipt);
        }
        Ok(outcome)
    }
}
impl WorkflowLedger for NativePaymentLedger {
    fn dispatch<'a>(
        &'a self,
        bytes: &'a [u8],
        identity: &'a TransactionIdentityV1,
    ) -> WorkflowFuture<'a, DispatchOutcome> {
        Box::pin(async move {
            match self.request.checked_signed_carrier(bytes) {
                Ok((_, actual)) if &actual == identity => {
                    self.dispatch.dispatch(bytes, identity).await
                }
                Ok(_) => DispatchOutcome::NotAdmitted {
                    code: WorkflowErrorCode::BindingMismatch,
                    detail: "native payment dispatch identity differs from its signed bytes".into(),
                },
                Err(error) => DispatchOutcome::NotAdmitted {
                    code: error.code,
                    detail: error.detail,
                },
            }
        })
    }
    fn committed_outcome<'a>(
        &'a self,
        identity: &'a TransactionIdentityV1,
    ) -> WorkflowFuture<'a, CommittedOutcome> {
        Box::pin(async move {
            match self.read_carrier(identity).await {
                Ok(outcome) => outcome,
                Err(error) => {
                    let detail = error.to_string();
                    if let Ok(mut slot) = lock(&self.error)
                        && slot.is_none()
                    {
                        *slot = Some(error);
                    }
                    CommittedOutcome::ReadFailed { detail }
                }
            }
        })
    }
}

fn evidence(
    block: &SignedBlock,
    index: u32,
) -> Result<CommittedTransaction, NativePaymentObservationError> {
    let entrypoint = block
        .network_entrypoint_at(index as usize)
        .ok_or(NativePaymentObservationError::CandidateMismatch)?
        .clone();
    let (output_index, _) = block
        .network_output_at(index)
        .ok_or(NativePaymentObservationError::CandidateMismatch)?;
    let output = block
        .execution_outputs()
        .get(output_index as usize)
        .ok_or(NativePaymentObservationError::CandidateMismatch)?
        .clone();
    Ok(CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash: entrypoint.hash(),
        entrypoint_proof: block
            .network_input_proof(index)
            .ok_or(NativePaymentObservationError::CandidateMismatch)?,
        entrypoint,
        output_hash: HashOf::new(&output),
        output_proof: block
            .output_proof(output_index)
            .ok_or(NativePaymentObservationError::CandidateMismatch)?,
        output,
    })
}
