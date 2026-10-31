//! Exact authenticated carrier outcomes and final-tip original receipt recovery.

use super::super::finality_custody::{
    BoundPrivateSettlementFinality, FinalityLookup, FinalityPhase, FinalityTerminalKind,
    LoadedPendingFinality, PendingFinalityPin, PendingFinalityPrefix, PendingFinalityTerminal,
};
use super::*;
use iroha_data_model::{
    block::SignedBlock, query::CommittedTransaction, sumeragi_finality::VerifiedSumeragiBlock,
    transaction::TransactionEntrypoint,
};

mod finality;
use std::{
    num::NonZeroU64,
    sync::atomic::{AtomicUsize, Ordering},
};

pub(super) struct SettlementLedger {
    account: AccountClient,
    request: PrivateSettlementRequest,
    state: Arc<Mutex<Option<PublicContext>>>,
    verifier: Arc<Mutex<SumeragiFinalityVerifier>>,
    reads: AtomicUsize,
    error: Mutex<Option<PrivateSettlementError>>,
    worker: Arc<tokio::sync::Semaphore>,
    storage: Arc<dyn WorkflowStorage>,
    finality: BoundPrivateSettlementFinality,
    finality_lease: WorkflowId,
}
impl SettlementLedger {
    pub fn new(
        account: AccountClient,
        request: PrivateSettlementRequest,
        state: Arc<Mutex<Option<PublicContext>>>,
        mut verifier: SumeragiFinalityVerifier,
        worker: Arc<tokio::sync::Semaphore>,
        storage: Arc<dyn WorkflowStorage>,
        finality: BoundPrivateSettlementFinality,
    ) -> Self {
        verifier.retain_recent_decisions(3);
        let finality_lease = WorkflowId::from_bytes(domain_digest(
            b"iroha:private-settlement:global-phase-worker:v1\0",
            &[request.manifest().network_id.as_bytes(), &request.0.digest],
        ));
        Self {
            account,
            request,
            state,
            verifier: Arc::new(Mutex::new(verifier)),
            reads: AtomicUsize::new(0),
            error: Mutex::new(None),
            worker,
            storage,
            finality,
            finality_lease,
        }
    }
    pub fn begin_call(&self, reads: usize) -> std::result::Result<(), PrivateSettlementError> {
        self.reads.store(reads, Ordering::Relaxed);
        *lock(&self.error)? = None;
        Ok(())
    }
    pub fn take_error(
        &self,
    ) -> std::result::Result<Option<PrivateSettlementError>, PrivateSettlementError> {
        Ok(lock(&self.error)?.take())
    }
    pub fn remember_error(&self, error: PrivateSettlementError) -> StepFailure {
        let report = failure(&error);
        if let Ok(mut slot) = lock(&self.error)
            && slot.is_none()
        {
            *slot = Some(error);
        }
        report
    }
    async fn proof_response(
        &self,
        height: u64,
    ) -> std::result::Result<crate::client::SumeragiFinalityProofResponse, PrivateSettlementError>
    {
        let height = NonZeroU64::new(height).ok_or(PrivateSettlementError::Binding)?;
        self.reads
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
            .map_err(|_| PrivateSettlementError::ReadBudget)?;
        let response = self
            .account
            .client()
            .read_sumeragi_finality_proof_response_async(height, SumeragiRootScope::Global)
            .await
            .map_err(PrivateSettlementError::Protocol)?;
        Ok(response)
    }
    async fn located(
        &self,
        phase: FinalityPhase,
        identity: &TransactionIdentityV1,
        height: Option<u64>,
    ) -> Result<Option<(VerifiedSumeragiBlock, CommittedTransaction)>, PrivateSettlementError> {
        Ok(self
            .finality_located(phase, Some(*identity), height)
            .await?
            .found())
    }
    pub async fn verify_registration(
        &self,
        journal: &WorkflowJournalV1,
    ) -> std::result::Result<(), PrivateSettlementError> {
        let attempt = journal
            .steps
            .get(REGISTER)
            .and_then(|s| s.transaction.as_ref())
            .and_then(|t| t.attempts.last())
            .ok_or(PrivateSettlementError::Binding)?;
        let identity = attempt
            .identity
            .as_ref()
            .ok_or(PrivateSettlementError::Binding)?;
        let height = attempt.evidence.as_ref().and_then(|e| e.block_height);
        let (block, evidence) = self
            .located(FinalityPhase::Register, identity, height)
            .await?
            .ok_or(PrivateSettlementError::Unresolved)?;
        // The native entry was already fully checked under the original observation lease.
        let _ = block;
        if evidence.result().is_ok() {
            Ok(())
        } else {
            Err(PrivateSettlementError::Unresolved)
        }
    }
    pub async fn recover_business(
        &self,
        journal: &WorkflowJournalV1,
    ) -> std::result::Result<Option<VerifiedPrivateSettlementOutcome>, PrivateSettlementError> {
        // Abort needs the original availability-complete manifest, not a Prepare quorum.
        // Before availability completes we have no independently retained full manifest.
        if !journal.steps[AVAILABILITY].is_done() {
            return Ok(None);
        }
        let prepared = journal.steps[PREPARE].is_done();
        if prepared {
            if let Some(outcome) = self
                .finality_located(FinalityPhase::Finalize, None, None)
                .await?
                .terminal()
            {
                return Ok(Some(outcome));
            }
        }
        if let Some(outcome) = self
            .finality_located(FinalityPhase::Abort, None, None)
            .await?
            .terminal()
        {
            return Ok(Some(outcome));
        }
        // Exact persisted carrier identity is a locator only. A later included rejection
        // cannot become a terminal receipt or erase an independently pinned Abort/settlement.
        if prepared
            && let Some(attempt) = journal.steps[FINALIZE]
                .transaction
                .as_ref()
                .and_then(|t| t.attempts.last())
            && let Some(identity) = attempt.identity
            && let Some(outcome) = self
                .finality_located(
                    FinalityPhase::Finalize,
                    Some(identity),
                    attempt.evidence.as_ref().and_then(|e| e.block_height),
                )
                .await?
                .terminal()
        {
            return Ok(Some(outcome));
        }
        let bundle_id = {
            let state = lock(&self.state)?;
            state
                .as_ref()
                .ok_or(PrivateSettlementError::Binding)?
                .manifest()
                .bundle_id
        };
        // This response supplies a height only. The original manifest/barrier plus native
        // successful input/output construct the terminal capability inside the existing worker.
        let lease = self
            .storage
            .acquire_runner(&self.finality_lease)
            .map_err(PrivateSettlementError::Custody)?;
        let permit = self
            .worker
            .clone()
            .try_acquire_owned()
            .map_err(|_| PrivateSettlementError::Capacity)?;
        let client = self.account.client().clone();
        let (response, lease) = run_native_worker(permit, lease, move || {
            client.private_settlement_bundle_receipt_v1(bundle_id)
        })
        .await?;
        drop(lease);
        let response = response.map_err(PrivateSettlementError::Protocol)?;
        let (phase, height) = match response {
            PrivateSettlementBundleReceiptResponseV1::Finalized(receipt) if prepared => {
                (FinalityPhase::Finalize, receipt.finalized_height)
            }
            PrivateSettlementBundleReceiptResponseV1::Aborted(receipt) => {
                (FinalityPhase::Abort, receipt.finalized_height)
            }
            PrivateSettlementBundleReceiptResponseV1::Finalized(_)
            | PrivateSettlementBundleReceiptResponseV1::Pending { .. } => return Ok(None),
        };
        self.finality_located(phase, None, Some(height))
            .await?
            .terminal()
            .map(Some)
            .ok_or(PrivateSettlementError::Binding)
    }
    async fn outcome(
        &self,
        identity: &TransactionIdentityV1,
    ) -> std::result::Result<CommittedOutcome, PrivateSettlementError> {
        let phase = match lock(&self.state)?
            .as_ref()
            .ok_or(PrivateSettlementError::Binding)?
            .phase()
        {
            REGISTER => FinalityPhase::Register,
            FINALIZE => FinalityPhase::Finalize,
            _ => return Err(PrivateSettlementError::Binding),
        };
        let entry = match self.finality_located(phase, Some(*identity), None).await? {
            finality::LocatedOutcome::Absent => return Ok(CommittedOutcome::ExactAbsence),
            finality::LocatedOutcome::Pending => return Ok(CommittedOutcome::ObservedNotFinal),
            finality::LocatedOutcome::Found(entry) => entry,
        };
        let (block, evidence) = (entry.block, entry.evidence);
        if !evidence.result().is_ok() {
            return Ok(CommittedOutcome::Rejected {
                detail: "independently authenticated original execution rejected this carrier"
                    .into(),
            });
        }
        // checked_entry already verified the exact phase/fees and any successful terminal
        // capability under the native worker; no BLS/native re-verification runs on this executor.
        Ok(CommittedOutcome::Applied {
            block_height: Some(block.height()),
        })
    }
}
impl WorkflowLedger for SettlementLedger {
    fn dispatch<'a>(
        &'a self,
        bytes: &'a [u8],
        expected: &'a TransactionIdentityV1,
    ) -> WorkflowFuture<'a, DispatchOutcome> {
        Box::pin(async move {
            let transaction = match super::super::super::workflow_ledger::decode_signed(bytes) {
                Ok(transaction) => transaction,
                Err(error) => {
                    return DispatchOutcome::NotAdmitted {
                        code: error.code,
                        detail: error.detail,
                    };
                }
            };
            let checked = (|| {
                check_payload(
                    &self.request,
                    lock(&self.state)?.as_ref(),
                    transaction.payload(),
                    true,
                )?;
                transaction
                    .verify_signature()
                    .map_err(|_| PrivateSettlementError::Binding)?;
                if &super::identity(&transaction) != expected
                    || bytes.len() as u64 > self.request.0.max_carrier_bytes
                {
                    return Err(PrivateSettlementError::Binding);
                }
                Ok::<_, PrivateSettlementError>(())
            })();
            if let Err(error) = checked {
                return DispatchOutcome::NotAdmitted {
                    code: WorkflowErrorCode::BindingMismatch,
                    detail: error.to_string(),
                };
            }
            let permit = match self.worker.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    return DispatchOutcome::NotAdmitted {
                        code: WorkflowErrorCode::LocalResourceUnavailable,
                        detail: "original private-settlement worker is still active".into(),
                    };
                }
            };
            let client = self.account.client().clone();
            // Reuse the existing sponsor-authenticated exact private carrier endpoint,
            // including its authenticated identical-carrier acknowledgement behavior.
            match run_native_worker(permit, (), move || {
                client.submit_private_settlement_bundle_v1(&PrivateSettlementBundleSubmitRequestV1 { transaction })
            }).await {
                Ok((Ok(_), ())) => DispatchOutcome::Accepted,
                _ => DispatchOutcome::Ambiguous { detail: "private-settlement submission may have been admitted; recover the exact original identity".into() },
            }
        })
    }
    fn committed_outcome<'a>(
        &'a self,
        identity: &'a TransactionIdentityV1,
    ) -> WorkflowFuture<'a, CommittedOutcome> {
        Box::pin(async move {
            match self.outcome(identity).await {
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
// Out-of-range authenticated locators are binding failures, never truncated indices.
pub(super) fn network_index(index: usize) -> std::result::Result<u32, PrivateSettlementError> {
    u32::try_from(index).map_err(|_| PrivateSettlementError::Binding)
}

pub(super) fn evidence(
    block: &SignedBlock,
    index: u32,
) -> std::result::Result<CommittedTransaction, PrivateSettlementError> {
    let entrypoint = block
        .network_entrypoint_at(index as usize)
        .ok_or(PrivateSettlementError::Binding)?
        .clone();
    let (output_index, _) = block
        .network_output_at(index)
        .ok_or(PrivateSettlementError::Binding)?;
    let output = block
        .execution_outputs()
        .get(output_index as usize)
        .ok_or(PrivateSettlementError::Binding)?
        .clone();
    Ok(CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash: entrypoint.hash(),
        entrypoint_proof: block
            .network_input_proof(index)
            .ok_or(PrivateSettlementError::Binding)?,
        entrypoint,
        output_hash: HashOf::new(&output),
        output_proof: block
            .output_proof(output_index)
            .ok_or(PrivateSettlementError::Binding)?,
        output,
    })
}
