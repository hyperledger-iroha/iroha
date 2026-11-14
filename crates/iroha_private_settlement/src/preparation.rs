//! One existing workflow round owns original source/secret/proof custody.
use super::*;
use iroha::workflow::{
    CustodyHandleV1, WorkflowJournalV1, WorkflowRunnerLease, WorkflowStorageError,
};
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicUsize, Ordering},
};

mod leg_pipeline;
use leg_pipeline::{Admission, ProverHelpers, Ready};

pub fn driver_identity() -> DriverIdentityV1 {
    DriverIdentityV1 {
        id: "private_settlement_preparation".into(),
        schema_version: 1,
    }
}
pub struct NoPreparationEffects(pub WorkflowBindingV1);
impl WorkflowAccount for NoPreparationEffects {
    fn binding(&self) -> WorkflowBindingV1 {
        self.0.clone()
    }
    fn sign<'a>(
        &'a self,
        _: &'a [u8],
    ) -> WorkflowFuture<'a, std::result::Result<iroha::workflow::SignedStepTransaction, StepFailure>>
    {
        Box::pin(async {
            Err(StepFailure::new(
                WorkflowErrorCode::UnsupportedOperation,
                "preparation has no transaction signer",
            ))
        })
    }
    fn identify(&self, _: &[u8]) -> std::result::Result<TransactionIdentityV1, StepFailure> {
        Err(StepFailure::new(
            WorkflowErrorCode::UnsupportedOperation,
            "preparation has no transaction identity",
        ))
    }
}
impl WorkflowLedger for NoPreparationEffects {
    fn dispatch<'a>(
        &'a self,
        _: &'a [u8],
        _: &'a TransactionIdentityV1,
    ) -> WorkflowFuture<'a, DispatchOutcome> {
        Box::pin(async {
            DispatchOutcome::Refused {
                detail: "preparation has no ledger dispatch".into(),
            }
        })
    }
    fn committed_outcome<'a>(
        &'a self,
        _: &'a TransactionIdentityV1,
    ) -> WorkflowFuture<'a, CommittedOutcome> {
        Box::pin(async {
            CommittedOutcome::ReadFailed {
                detail: "preparation has no ledger observation".into(),
            }
        })
    }
}
pub struct PreparationDriver {
    account: AccountClient,
    request: OneCallPrivateSettlementRequest,
    storage: Arc<dyn WorkflowStorage>,
    artifacts: Arc<FilePrivateSettlementArtifacts>,
    source: Arc<dyn PrivateSettlementContextSource>,
    wallet: Arc<dyn PrivateSettlementWallet>,
    verifier: Arc<Mutex<SumeragiFinalityVerifier>>,
    reads: AtomicUsize,
    retained_bytes: AtomicUsize,
    error: Mutex<Option<OneCallPrivateSettlementError>>,
    worker: Arc<tokio::sync::Semaphore>,
    verified: Arc<VerifiedProofs>,
}

/// Artifacts whose proof this driver has verified against its one immutable request.
///
/// Proof verification is a function of the request's intent and the artifact's statement
/// and proof, all of which the artifact digest and this driver fix. One run otherwise
/// verifies each leg three times: inside the prover before it releases the proof, when the
/// artifact is bound, and when the completed round rereads it. The record is process-local
/// and never persisted, so a cold process verifies every artifact it loads.
#[derive(Default)]
struct VerifiedProofs(Mutex<BTreeSet<Digest32>>);
impl VerifiedProofs {
    /// Record a proof the prover released only after verifying it under this request.
    fn record(&self, digest: Digest32) -> std::result::Result<(), OneCallPrivateSettlementError> {
        lock(&self.0)?.insert(digest);
        Ok(())
    }
    /// Run `verify` unless `digest` is recorded, and record it once `verify` succeeds.
    fn verify_once(
        &self,
        digest: Digest32,
        verify: impl FnOnce() -> std::result::Result<(), OneCallPrivateSettlementError>,
    ) -> std::result::Result<(), OneCallPrivateSettlementError> {
        if lock(&self.0)?.contains(&digest) {
            return Ok(());
        }
        verify()?;
        self.record(digest)
    }
}

// Original artifact ownership includes the round lease until native digesting/final assembly
// and actual worker TLS cleanup finish. The canonical worker remains the sole lifecycle owner.
struct OriginalArtifacts {
    artifacts: Vec<Artifact>,
    // Derived from the same authenticated cut on every reopen; never node configuration.
    max_carrier_bytes: u64,
    lease: Box<dyn WorkflowRunnerLease>,
}
impl OriginalArtifacts {
    async fn complete<T: Send + 'static>(
        self,
        worker: Arc<tokio::sync::Semaphore>,
        work: impl FnOnce(Vec<Artifact>) -> std::result::Result<T, OneCallPrivateSettlementError>
        + Send
        + 'static,
    ) -> std::result::Result<T, OneCallPrivateSettlementError> {
        let permit = worker
            .try_acquire_owned()
            .map_err(|_| OneCallPrivateSettlementError::Capacity)?;
        let Self {
            artifacts,
            lease,
            max_carrier_bytes: _,
        } = self;
        let (result, original_lease) =
            run_native_worker(permit, lease, move || work(artifacts)).await?;
        drop(original_lease);
        result
    }
}
fn lock<T>(
    value: &Mutex<T>,
) -> std::result::Result<std::sync::MutexGuard<'_, T>, OneCallPrivateSettlementError> {
    value
        .lock()
        .map_err(|_| OneCallPrivateSettlementError::Capacity)
}
fn storage_error(error: WorkflowStorageError) -> OneCallPrivateSettlementError {
    OneCallPrivateSettlementError::Provider(eyre::Report::new(error))
}
fn remaining_finality_custody(
    artifacts: &FilePrivateSettlementArtifacts,
    maximum_total: usize,
    retained: usize,
) -> std::result::Result<Arc<FilePrivateSettlementFinality>, OneCallPrivateSettlementError> {
    if retained == 0 {
        return Err(OneCallPrivateSettlementError::Pending);
    }
    let remaining = maximum_total
        .checked_sub(retained)
        .ok_or(OneCallPrivateSettlementError::Capacity)?
        .min(MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES);
    Ok(Arc::new(
        artifacts
            .finality()
            .with_maximum_bytes(remaining)
            .map_err(PrivateSettlementError::from)?,
    ))
}

impl PreparationDriver {
    pub(super) fn session_request(&self) -> &OneCallPrivateSettlementRequest {
        &self.request
    }

    pub fn new(
        account: AccountClient,
        request: OneCallPrivateSettlementRequest,
        storage: Arc<dyn WorkflowStorage>,
        artifacts: Arc<FilePrivateSettlementArtifacts>,
        source: Arc<dyn PrivateSettlementContextSource>,
        wallet: Arc<dyn PrivateSettlementWallet>,
        mut verifier: SumeragiFinalityVerifier,
    ) -> Self {
        // A future selected cut needs only the newest three decision witnesses.
        // Preserve a caller's older selected-cut witnesses when its tip is already ahead.
        if verifier.authenticated_height().unwrap_or(0) <= request.intent().authority_context_height
        {
            verifier.retain_recent_decisions(3);
        }
        Self {
            account,
            request,
            storage,
            artifacts,
            source,
            wallet,
            verifier: Arc::new(Mutex::new(verifier)),
            reads: AtomicUsize::new(0),
            retained_bytes: AtomicUsize::new(0),
            error: Mutex::new(None),
            worker: Arc::new(tokio::sync::Semaphore::new(1)),
            verified: Arc::default(),
        }
    }
    pub fn begin_call(
        &self,
        reads: usize,
    ) -> std::result::Result<(), OneCallPrivateSettlementError> {
        self.reads.store(reads, Ordering::Relaxed);
        *lock(&self.error)? = None;
        Ok(())
    }
    pub fn settlement_handoff(
        &self,
        options: &PrivateSettlementRunOptions,
    ) -> std::result::Result<
        (SumeragiFinalityVerifier, PrivateSettlementRunOptions),
        OneCallPrivateSettlementError,
    > {
        let remaining = u16::try_from(self.reads.load(Ordering::Relaxed))
            .map_err(|_| OneCallPrivateSettlementError::Capacity)?;
        let mut options = *options;
        options.max_finality_proof_reads = remaining;
        Ok((lock(&self.verifier)?.clone(), options))
    }
    /// Keep settlement checkpoint bytes inside the original one-call artifact allowance.
    /// The complete original context, preparation prefix and proof artifacts were reloaded
    /// and authenticated by `recovered_request` before this handoff can be used.
    pub fn settlement_finality_custody(
        &self,
    ) -> std::result::Result<Arc<FilePrivateSettlementFinality>, OneCallPrivateSettlementError>
    {
        remaining_finality_custody(
            &self.artifacts,
            self.request.0.max_total_artifact_bytes,
            self.retained_bytes.load(Ordering::Relaxed),
        )
    }

    pub fn take_error(
        &self,
    ) -> std::result::Result<Option<OneCallPrivateSettlementError>, OneCallPrivateSettlementError>
    {
        Ok(lock(&self.error)?.take())
    }
    fn remember(&self, error: OneCallPrivateSettlementError) -> StepFailure {
        // A transient operational category, never a terminal ledger/round verdict.
        // Keep the original typed owner for the public caller; persist no provider detail.
        let code = error.workflow_code();
        if let Ok(mut slot) = self.error.lock() {
            *slot = Some(error);
        }
        StepFailure::new(
            code,
            "private preparation original cause retained outside journal",
        )
    }
    async fn selected_block(
        &self,
        key: ArtifactKey,
        mut lease: Box<dyn WorkflowRunnerLease>,
    ) -> std::result::Result<
        (VerifiedSumeragiBlock, Box<dyn WorkflowRunnerLease>, usize),
        OneCallPrivateSettlementError,
    > {
        let height = self.request.intent().authority_context_height;
        let maximum = self
            .request
            .0
            .max_artifact_bytes
            .min(self.request.0.max_total_artifact_bytes);
        let artifacts = self.artifacts.clone();
        let network = self.request.intent().network_id;
        let (chain, instance, scope, initial_epoch) = {
            let selected = lock(&self.verifier)?;
            (
                selected.chain_id().to_owned(),
                selected.instance(),
                selected.root_scope(),
                selected.initial_epoch().clone(),
            )
        };
        let permit = self
            .worker
            .clone()
            .try_acquire_owned()
            .map_err(|_| OneCallPrivateSettlementError::Capacity)?;
        // Import the native checkpoint on the same bounded worker as its encrypted read.
        // Keeping nested canonical decoding off the async workflow's call stack also
        // keeps synchronous authentication from blocking its current-thread executor.
        let (saved, returned_lease) = run_native_worker(permit, lease, move || {
            (|| -> std::result::Result<_, OneCallPrivateSettlementError> {
                let Some((checkpoint, length)) = artifacts.read_finality(key, maximum)? else {
                    return Ok(None);
                };
                if checkpoint.height() > height
                    || checkpoint.network_id() != network
                    || checkpoint.chain_id() != chain
                {
                    return Err(OneCallPrivateSettlementError::Binding);
                }
                // AEAD selects the local original, never a checkpoint returned by a peer.
                // The independently supplied genesis owner still fixes every root identity.
                let (restored, tip) = SumeragiFinalityVerifier::from_trusted_checkpoint_with_tip(
                    &checkpoint,
                    &network,
                    &chain,
                )?;
                if restored.instance() != instance
                    || restored.root_scope() != scope
                    || restored.initial_epoch() != &initial_epoch
                {
                    return Err(OneCallPrivateSettlementError::Binding);
                }
                Ok(Some((restored, tip, length)))
            })()
        })
        .await?;
        lease = returned_lease;
        let mut checkpoint_length = 0;
        if let Some((restored, tip, length)) = saved? {
            checkpoint_length = length;
            // This file is the prefix selected for this pending preparation. The caller
            // may bring a newer verifier whose bounded history no longer retains this cut.
            *lock(&self.verifier)? = restored;
            if tip.height() == height {
                return Ok((tip, lease, length));
            }
        }
        loop {
            let next = lock(&self.verifier)?
                .authenticated_height()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(OneCallPrivateSettlementError::Binding)?;
            self.reads
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
                .map_err(|_| OneCallPrivateSettlementError::Capacity)?;
            let target = next.min(height);
            let response = self
                .account
                .read_sumeragi_finality_proof_response_async(
                    NonZeroU64::new(target).ok_or(OneCallPrivateSettlementError::Binding)?,
                    SumeragiRootScope::Global,
                )
                .await
                .map_err(OneCallPrivateSettlementError::Provider)?;
            // Keep the committed in-memory owner unchanged until the new local
            // checkpoint has a durable acknowledgement. On cancellation after publication,
            // the next caller imports that original file under the same round lease.
            if next - 1 > height {
                // The caller already authenticated a later tip and supplied this older
                // decision's witnesses. Borrow them without duplicating its history.
                let verifier = self.verifier.clone();
                let permit = self
                    .worker
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| OneCallPrivateSettlementError::Capacity)?;
                let (block, returned_lease) = run_native_worker(permit, lease, move || {
                    let proof = response
                        .decode_checked()
                        .map_err(OneCallPrivateSettlementError::Provider)?;
                    Ok::<_, OneCallPrivateSettlementError>(
                        lock(&verifier)?.verify_retained_decision(&proof)?,
                    )
                })
                .await?;
                return Ok((block?, returned_lease, checkpoint_length));
            }
            let mut candidate = lock(&self.verifier)?.clone();
            let artifacts = self.artifacts.clone();
            let permit = self
                .worker
                .clone()
                .try_acquire_owned()
                .map_err(|_| OneCallPrivateSettlementError::Capacity)?;
            let (advanced, returned_lease) = run_native_worker(permit, lease, move || {
                (|| -> std::result::Result<_, OneCallPrivateSettlementError> {
                    let proof = response
                        .decode_checked()
                        .map_err(OneCallPrivateSettlementError::Provider)?;
                    let block = if next > height {
                        candidate.verify_retained_decision(&proof)?
                    } else {
                        candidate.verify(&proof)?
                    };
                    let checkpoint = candidate.export_checkpoint(&proof)?;
                    candidate.retain_recent_decisions(3);
                    let length = artifacts.publish_finality(key, &checkpoint, maximum)?;
                    Ok((candidate, block, length))
                })()
            })
            .await?;
            lease = returned_lease;
            let (candidate, block, length) = advanced?;
            checkpoint_length = length;
            *lock(&self.verifier)? = candidate;
            if target == height {
                return Ok((block, lease, checkpoint_length));
            }
        }
    }
    fn status(
        &self,
        context: RoundContext<'_>,
    ) -> std::result::Result<RoundStatusReport, OneCallPrivateSettlementError> {
        if context.step_index != 0
            || context.descriptor.intent_digest != self.request.0.digest
            || context.journal.identity.request_context_digest != self.request.0.digest
        {
            return Err(OneCallPrivateSettlementError::Binding);
        }
        Ok(RoundStatusReport {
            round_id: context.round_id,
            context_digest: self.request.0.digest,
            participants: Vec::new(),
        })
    }
    async fn originals(
        &self,
        round: Digest32,
        may_prepare: bool,
    ) -> std::result::Result<Option<OriginalArtifacts>, OneCallPrivateSettlementError> {
        // The storage owner retains the original per-round proof lease across network waits
        // and moves it into each native worker. It is not cloneable: cancellation cannot
        // release custody while that worker still holds secrets or publishes its artifact.
        let lease_id = WorkflowId::from_bytes(domain_digest(
            b"iroha:private-settlement:proof-worker:v1\0",
            &[&round],
        ));
        let mut lease = self
            .storage
            .acquire_runner(&lease_id)
            .map_err(storage_error)?;
        let request = self.request.clone();
        let selectors = PrivateSettlementContextRequestV1 {
            pools: request
                .intent()
                .legs
                .iter()
                .map(|leg| PrivateSettlementPoolKeyV1 {
                    route: leg.route,
                    pool_id: leg.pool_id,
                })
                .collect(),
        };
        selectors.validate()?;
        let context_key = ArtifactKey {
            request: request.0.digest,
            round,
            ordinal: 0,
        };
        let (block, returned_lease, checkpoint_length) =
            self.selected_block(context_key, lease).await?;
        lease = returned_lease;
        let block = Arc::new(block);
        let artifacts = self.artifacts.clone();
        let maximum = request.0.max_artifact_bytes;
        let permit = self
            .worker
            .clone()
            .try_acquire_owned()
            .map_err(|_| OneCallPrivateSettlementError::Capacity)?;
        let (original, original_lease) = run_native_worker(permit, lease, move || {
            artifacts.read_context(context_key, maximum)
        })
        .await?;
        lease = original_lease;
        let (proof, context_length, publish) = if let Some((proof, length)) = original? {
            (proof, length, false)
        } else {
            if !may_prepare {
                return Ok(None);
            }
            let proof = self
                .source
                .fetch(request.intent().authority_context_height, selectors.clone())
                .await?;
            let length = norito::canonical_frame_len(&proof)?;
            if length > maximum {
                return Err(OneCallPrivateSettlementError::Capacity);
            }
            (proof, length, true)
        };
        let retained_length = context_length
            .checked_add(checkpoint_length)
            .ok_or(OneCallPrivateSettlementError::Capacity)?;
        if retained_length > request.0.max_total_artifact_bytes {
            return Err(OneCallPrivateSettlementError::Capacity);
        }
        let permit = self
            .worker
            .clone()
            .try_acquire_owned()
            .map_err(|_| OneCallPrivateSettlementError::Capacity)?;
        let artifacts = self.artifacts.clone();
        let original_block = block.clone();
        let (original, original_lease) = run_native_worker(permit, lease, move || {
            (|| -> std::result::Result<_, OneCallPrivateSettlementError> {
                proof.verify(
                    request.intent().network_id,
                    request.0.schemas.as_slice(),
                    &selectors,
                    &original_block,
                )?;
                // Public mismatches on any later leg must refuse before the first wallet
                // opening or reservation. Reuse the producer's Model-owned exact join.
                let mut max_carrier_bytes = request.0.max_carrier_bytes;
                for (index, member) in proof.contexts.iter().enumerate() {
                    let selected = member.verify(
                        request.intent().network_id,
                        request.0.schemas.as_slice(),
                        selectors.pools[index],
                        &original_block,
                    )?;
                    selected
                        .verify_intent(request.intent(), request.intent().legs[index].ordinal)?;
                    max_carrier_bytes =
                        max_carrier_bytes.min(validate_selected_request(&request, &selected)?);
                    if selected.pool_cut().block_time_ms() < request.0.minimum_context_time_ms {
                        return Err(OneCallPrivateSettlementError::Binding);
                    }
                }
                let digest = domain_digest(
                    b"iroha:private-settlement:original-contexts:v1\0",
                    &[&norito::encode_canonical(&proof)?],
                );
                // All exact selected context is durable before the first wallet call. One
                // physical source and one atomic encrypted file prevent partial-cut stranding.
                if publish {
                    artifacts.publish_context(context_key, &proof, maximum)?;
                }
                Ok((Arc::new(proof), digest, max_carrier_bytes))
            })()
        })
        .await?;
        lease = original_lease;
        let (contexts, context_digest, max_carrier_bytes) = original?;
        // One admitted worker and the original round lease cover every leg. Wallet calls
        // and artifact publication stay on that worker in leg order; only the proofs of
        // already reserved legs overlap, and each window joins before it is published.
        let request = self.request.clone();
        let permit = self
            .worker
            .clone()
            .try_acquire_owned()
            .map_err(|_| OneCallPrivateSettlementError::Capacity)?;
        let artifacts = self.artifacts.clone();
        let wallet = self.wallet.clone();
        let verified_proofs = Arc::clone(&self.verified);
        let (prepared, original_lease) = run_native_worker(permit, lease, move || {
            LegOriginals {
                request: &request,
                round,
                contexts: &contexts,
                context_digest,
                block: &block,
                artifacts: &artifacts,
                wallet: &*wallet,
                verified: &verified_proofs,
                may_prepare,
            }
            .prepare(retained_length, &leg_pipeline::PROCESS_PROVER_HELPERS)
        })
        .await?;
        lease = original_lease;
        let Some((complete, bytes)) = prepared? else {
            return Ok(None);
        };
        self.retained_bytes.store(bytes, Ordering::Relaxed);
        Ok(Some(OriginalArtifacts {
            artifacts: complete,
            max_carrier_bytes,
            lease,
        }))
    }

    async fn report(
        &self,
        context: RoundContext<'_>,
        may_prepare: bool,
    ) -> std::result::Result<RoundReport, OneCallPrivateSettlementError> {
        let status = self.status(context)?;
        let Some(artifacts) = self.originals(context.round_id, may_prepare).await? else {
            return Ok(RoundReport::Waiting(status));
        };
        let round_id = context.round_id;
        artifacts
            .complete(self.worker.clone(), move |artifacts| {
                let digests = artifacts
                    .iter()
                    .map(Artifact::digest)
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                Ok(RoundReport::Completed {
                    status,
                    transcript: norito::encode_canonical(&digests)?,
                    custody_handles: vec![CustodyHandleV1 {
                        label: "encrypted_prepared_artifacts".into(),
                        reference: round_id.to_vec(),
                    }],
                })
            })
            .await
    }
    pub async fn recovered_request(
        &self,
        journal: Option<&WorkflowJournalV1>,
    ) -> std::result::Result<PrivateSettlementRequest, OneCallPrivateSettlementError> {
        let journal = journal.ok_or(OneCallPrivateSettlementError::Pending)?;
        let round = journal.steps[0]
            .round
            .as_ref()
            .ok_or(OneCallPrivateSettlementError::Binding)?;
        let round_id = round
            .round_id
            .ok_or(OneCallPrivateSettlementError::Binding)?;
        if !journal.steps[0].is_done() {
            return Err(OneCallPrivateSettlementError::Pending);
        }
        let transcript = round
            .transcript
            .clone()
            .ok_or(OneCallPrivateSettlementError::Binding)?;
        let Some(artifacts) = self.originals(round_id, false).await? else {
            return Err(OneCallPrivateSettlementError::Pending);
        };
        let request = self.request.clone();
        let max_carrier_bytes = artifacts.max_carrier_bytes;
        artifacts
            .complete(self.worker.clone(), move |artifacts| {
                let digests = artifacts
                    .iter()
                    .map(Artifact::digest)
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                let recorded: Vec<Digest32> = norito::decode_canonical_with_limits(
                    &transcript,
                    norito::canonical_decode_limits(transcript.len()),
                )?;
                if recorded != digests {
                    return Err(OneCallPrivateSettlementError::Binding);
                }
                let inputs = artifacts
                    .into_iter()
                    .map(Artifact::into_input)
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                let bundle = finalize_atomic_private_settlement_provisional_bundle_v1(
                    request.intent().clone(),
                    inputs,
                )
                .map_err(AtomicPrivateSettlementPreparationErrorV1::Wallet)?;
                Ok(PrivateSettlementRequest::new(
                    bundle.materials,
                    request.0.endpoints.clone(),
                    max_carrier_bytes,
                )?)
            })
            .await
    }

    pub async fn reconcile_verified(
        &self,
        journal: Option<&WorkflowJournalV1>,
        prepared: PrivateSettlementRequest,
        outcome: Arc<VerifiedPrivateSettlementOutcome>,
    ) -> std::result::Result<(), OneCallPrivateSettlementError> {
        let journal = journal.ok_or(OneCallPrivateSettlementError::Binding)?;
        if journal.plan() != self.request.plan()
            || !journal.steps[0].is_done()
            || prepared.manifest().proof_intent() != *self.request.intent()
        {
            return Err(OneCallPrivateSettlementError::Binding);
        }
        let round = journal.steps[0]
            .round
            .as_ref()
            .and_then(|round| round.round_id)
            .ok_or(OneCallPrivateSettlementError::Binding)?;
        let lease_id = WorkflowId::from_bytes(domain_digest(
            b"iroha:private-settlement:proof-worker:v1\0",
            &[&round],
        ));
        let lease = self
            .storage
            .acquire_runner(&lease_id)
            .map_err(storage_error)?;
        let permit = self
            .worker
            .clone()
            .try_acquire_owned()
            .map_err(|_| OneCallPrivateSettlementError::Capacity)?;
        let wallet = self.wallet.clone();
        let (result, original_lease) = run_native_worker(permit, lease, move || {
            wallet.reconcile_verified(round, &prepared, &outcome)
        })
        .await?;
        drop(original_lease);
        result
    }
}
/// Everything one round's legs are prepared from, borrowed by the original native worker.
struct LegOriginals<'a> {
    request: &'a OneCallPrivateSettlementRequest,
    round: Digest32,
    contexts: &'a PrivateSettlementContextsProofV1,
    context_digest: Digest32,
    block: &'a VerifiedSumeragiBlock,
    artifacts: &'a FilePrivateSettlementArtifacts,
    wallet: &'a dyn PrivateSettlementWallet,
    verified: &'a VerifiedProofs,
    may_prepare: bool,
}

/// One leg whose openings were checked and whose wallet inputs are reserved.
struct AdmittedLeg<'a> {
    context: iroha_data_model::nexus::VerifiedPrivateSettlementContextV1<'a>,
    opening: PrivateSettlementRestrictedOpening,
    secrets: AtomicPrivateSettlementAuthorizedWalletLegV1,
}

/// One leg's proof, released by the prover only after verifying it.
struct ProvedLeg<'a> {
    context: iroha_data_model::nexus::VerifiedPrivateSettlementContextV1<'a>,
    policy: PrivateSettlementAuditPolicyV1,
    prepared: AtomicPrivateSettlementPreparedLegV1,
}

impl<'a> LegOriginals<'a> {
    /// Reopen or prepare every leg and return the artifacts with the retained byte total.
    /// `None` means a leg is absent and this call may not prepare it.
    fn prepare(
        &self,
        retained: usize,
        helpers: &ProverHelpers,
    ) -> std::result::Result<Option<(Vec<Artifact>, usize)>, OneCallPrivateSettlementError> {
        let mut bytes = retained;
        let artifacts = leg_pipeline::run_legs(
            self.request.intent().legs.len(),
            helpers,
            |index| self.admit(index),
            |index, leg| self.prove(index, leg),
            |index, ready| self.complete(index, ready, &mut bytes),
        )?;
        Ok(artifacts.map(|artifacts| (artifacts, bytes)))
    }

    fn key(&self, index: usize) -> ArtifactKey {
        ArtifactKey {
            request: self.request.0.digest,
            round: self.round,
            ordinal: self.request.intent().legs[index].ordinal,
        }
    }

    // On the original worker, in leg order: every check and wallet effect that precedes a
    // proof. A leg is reserved only after its openings match the selected cut.
    fn admit(
        &self,
        index: usize,
    ) -> std::result::Result<
        Admission<AdmittedLeg<'a>, (Artifact, usize)>,
        OneCallPrivateSettlementError,
    > {
        let request = self.request;
        let leg = &request.intent().legs[index];
        let context = self.contexts.contexts[index].verify(
            request.intent().network_id,
            request.0.schemas.as_slice(),
            PrivateSettlementPoolKeyV1 {
                route: leg.route,
                pool_id: leg.pool_id,
            },
            self.block,
        )?;
        if context.pool_cut().block_time_ms() < request.0.minimum_context_time_ms {
            return Err(OneCallPrivateSettlementError::Binding);
        }
        if let Some(existing) = self
            .artifacts
            .read(self.key(index), request.0.max_artifact_bytes)?
        {
            validate_artifact(
                request,
                &context,
                self.context_digest,
                &existing.0,
                self.verified,
            )?;
            return Ok(Admission::Reopened(existing));
        }
        if !self.may_prepare {
            return Ok(Admission::Absent);
        }
        let terms = &request.0.terms[index];
        let opening = self
            .wallet
            .open_policy(request.intent(), leg.ordinal, terms)?;
        context.verify_openings(
            &opening.governance,
            &opening.policy,
            &terms.asset,
            opening.governance.body.asset_binding_salt,
            request.intent().expiry_height,
        )?;
        let secrets =
            self.wallet
                .reserve_or_reopen(self.round, request.intent(), leg.ordinal, terms)?;
        let plaintext = secrets.plaintext();
        let asset_matches = plaintext.asset_definition_id == terms.asset;
        let reimbursement_matches = plaintext.sponsor_reimbursement_amount == terms.reimbursement;
        if plaintext.payer != terms.payer
            || plaintext.recipient != terms.recipient
            || !asset_matches
            || plaintext.amount != terms.amount
            || !reimbursement_matches
            || plaintext.memo != terms.memo
        {
            return Err(OneCallPrivateSettlementError::Binding);
        }
        Ok(Admission::Admitted(AdmittedLeg {
            context,
            opening,
            secrets,
        }))
    }

    // On any prover thread: reads only its own admitted leg and the immutable request.
    fn prove(
        &self,
        index: usize,
        leg: AdmittedLeg<'a>,
    ) -> std::result::Result<ProvedLeg<'a>, OneCallPrivateSettlementError> {
        let request = self.request;
        let terms = &request.0.terms[index];
        let AdmittedLeg {
            context,
            opening,
            secrets,
        } = leg;
        let prepared = {
            // The same pure check admission passed; it only rebuilds the borrowed view.
            let verified = context.verify_openings(
                &opening.governance,
                &opening.policy,
                &terms.asset,
                opening.governance.body.asset_binding_salt,
                request.intent().expiry_height,
            )?;
            prepare_atomic_private_settlement_leg_v1(
                request.intent(),
                request.intent().legs[index].ordinal,
                &verified,
                secrets,
                request.0.padding,
            )?
        };
        Ok(ProvedLeg {
            context,
            policy: opening.policy,
            prepared,
        })
    }

    // On the original worker, in leg order, after every prover of the window joined:
    // bind and validate the artifact, charge the aggregate budget, then publish it.
    fn complete(
        &self,
        index: usize,
        ready: Ready<ProvedLeg<'a>, (Artifact, usize)>,
        bytes: &mut usize,
    ) -> std::result::Result<Artifact, OneCallPrivateSettlementError> {
        let request = self.request;
        let within_total = |bytes: usize, length: usize| {
            bytes
                .checked_add(length)
                .filter(|total| *total <= request.0.max_total_artifact_bytes)
                .ok_or(OneCallPrivateSettlementError::Capacity)
        };
        let (artifact, length) = match ready {
            Ready::Reopened(existing) => existing,
            Ready::Proved(ProvedLeg {
                context,
                policy,
                prepared,
            }) => {
                let artifact = Artifact {
                    key: self.key(index),
                    context_id: context.pool_cut().context_id(),
                    context_digest: self.context_digest,
                    statement: prepared.statement,
                    proof: prepared.proof,
                    delta: prepared.delta,
                    capsule: prepared.audit_capsule,
                    policy,
                    authority: context.authority().clone(),
                    retention_height: request.0.retention_height,
                };
                // The prover released this proof only after verifying it under the
                // cut's height; at the request's height that is the check below.
                if artifact.statement.authority_context_height
                    == request.intent().authority_context_height
                {
                    self.verified.record(artifact.digest()?)?;
                }
                validate_artifact(
                    request,
                    &context,
                    self.context_digest,
                    &artifact,
                    self.verified,
                )?;
                let length = norito::canonical_frame_len(&artifact)?;
                within_total(*bytes, length)?;
                self.artifacts
                    .publish(&artifact, request.0.max_artifact_bytes)?;
                (artifact, length)
            }
        };
        *bytes = within_total(*bytes, length)?;
        Ok(artifact)
    }
}

// Only the authenticated selected cut supplies semantic bounds. The request may choose
// smaller local carrier custody, but it cannot expand the committed transaction contract.
fn validate_selected_request(
    request: &OneCallPrivateSettlementRequest,
    context: &iroha_data_model::nexus::VerifiedPrivateSettlementContextV1<'_>,
) -> std::result::Result<u64, OneCallPrivateSettlementError> {
    let limits = context.admission_limits();
    if !limits.permits_padding(request.0.padding)
        // A cold original can be uploaded at any still-authorized height through expiry.
        // Its signed retention must satisfy the store's minimum even at that last height.
        || request.intent().expiry_height
            .checked_add(limits.sidecar_retention_blocks)
            .is_none_or(|minimum| request.0.retention_height < minimum)
    {
        return Err(OneCallPrivateSettlementError::Binding);
    }
    Ok(request.0.max_carrier_bytes.min(limits.max_carrier_bytes))
}

fn validate_material_limits(
    limits: &iroha_data_model::privacy::PrivateSettlementValidityLimitsV1,
    artifact: &Artifact,
) -> std::result::Result<(), OneCallPrivateSettlementError> {
    if artifact.proof.len() as u64 > limits.max_proof_bytes
        || norito::canonical_frame_len(&artifact.capsule)? as u64 > limits.max_capsule_bytes
        || !limits.permits_padding(artifact.capsule.padding)
        || u16::from(artifact.policy.body.min_approvals) < limits.min_auditor_approvals
    {
        return Err(OneCallPrivateSettlementError::Binding);
    }
    Ok(())
}

// Bind `artifact` to the request and selected context, then verify its proof unless this
// driver already verified these exact artifact bytes.
fn validate_artifact(
    request: &OneCallPrivateSettlementRequest,
    context: &iroha_data_model::nexus::VerifiedPrivateSettlementContextV1<'_>,
    context_digest: Digest32,
    artifact: &Artifact,
    verified: &VerifiedProofs,
) -> std::result::Result<(), OneCallPrivateSettlementError> {
    validate_selected_request(request, context)?;
    validate_material_limits(context.admission_limits(), artifact)?;
    let statement = &artifact.statement;
    if artifact.capsule.padding != request.0.padding
        || artifact.context_digest != context_digest
        || artifact.context_id != context.pool_cut().context_id()
        || artifact.authority != *context.authority()
        || artifact.retention_height != request.0.retention_height
        || statement.old_root != context.pool_cut().pool().root
        || statement.old_epoch != context.pool_cut().pool().epoch
        || artifact.policy.policy_digest != context.pool_cut().governance().audit_policy_digest
    {
        return Err(OneCallPrivateSettlementError::Binding);
    }
    for nullifier in &statement.nullifiers {
        context.pool_cut().verify_nullifier_absent(*nullifier)?;
    }
    for output in &statement.output_commitments {
        context.pool_cut().verify_output_absent(*output)?;
    }
    AtomicPrivateSettlementProvisionalLegInputV1::new(
        artifact.prepared(),
        artifact.policy.clone(),
        artifact.authority.clone(),
        artifact.retention_height,
    )
    .map_err(AtomicPrivateSettlementPreparationErrorV1::Wallet)?;
    verified.verify_once(artifact.digest()?, || {
        verify_atomic_private_settlement_intent_v1(
            request.intent(),
            statement,
            *request.intent().network_id.as_genesis_hash().as_ref(),
            request.intent().authority_context_height,
            &artifact.proof,
        )
        .map_err(AtomicPrivateSettlementPreparationErrorV1::Proof)?;
        Ok(())
    })
}
impl WorkflowDriver for PreparationDriver {
    fn identity(&self) -> DriverIdentityV1 {
        driver_identity()
    }
    fn supports_round_continuation(&self) -> bool {
        true
    }
    fn start_round<'a>(
        &'a self,
        context: RoundContext<'a>,
    ) -> WorkflowFuture<'a, std::result::Result<RoundReport, StepFailure>> {
        Box::pin(async move {
            self.report(context, true)
                .await
                .map_err(|e| self.remember(e))
        })
    }
    fn recover_round<'a>(
        &'a self,
        context: RoundContext<'a>,
    ) -> WorkflowFuture<'a, std::result::Result<RoundRecovery, StepFailure>> {
        Box::pin(async move {
            self.report(context, false)
                .await
                .map(RoundRecovery::Found)
                .map_err(|e| self.remember(e))
        })
    }
    fn continue_round<'a>(
        &'a self,
        context: RoundContext<'a>,
        observed: RoundStatusReport,
    ) -> WorkflowFuture<'a, std::result::Result<RoundReport, StepFailure>> {
        Box::pin(async move {
            if self.status(context).map_err(|e| self.remember(e))? != observed {
                return Err(self.remember(OneCallPrivateSettlementError::Binding));
            }
            self.report(context, true)
                .await
                .map_err(|e| self.remember(e))
        })
    }
}

#[cfg(test)]
mod completion_tests;
#[cfg(test)]
mod policy_tests;

#[cfg(test)]
mod finality_budget_tests {
    use super::*;

    #[test]
    fn handoff_finality_custody_uses_only_the_original_aggregate_remainder() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("artifacts");
        let artifacts = FilePrivateSettlementArtifacts::open(&path, &[53; 32]).unwrap();
        assert!(matches!(
            remaining_finality_custody(&artifacts, 100, 0),
            Err(OneCallPrivateSettlementError::Pending)
        ));
        let remainder = remaining_finality_custody(&artifacts, 100, 99).unwrap();
        assert!(remainder.with_maximum_bytes(1).is_ok());
        assert!(
            remainder.with_maximum_bytes(2).is_err(),
            "the finalized record cannot obtain another full artifact budget"
        );
        assert!(remaining_finality_custody(&artifacts, 100, 100).is_err());
        assert!(remaining_finality_custody(&artifacts, 100, 101).is_err());
        let maximum =
            remaining_finality_custody(&artifacts, MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES + 1024, 1)
                .unwrap();
        assert!(
            maximum
                .with_maximum_bytes(MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES)
                .is_ok()
        );
        assert!(
            maximum
                .with_maximum_bytes(MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES + 1)
                .is_err()
        );
        assert!(Arc::ptr_eq(artifacts.finality(), artifacts.finality()));
        assert_eq!(
            std::fs::read_dir(&path).unwrap().count(),
            0,
            "budget selection and accessor create no finality or lock file and discard no original"
        );
    }
}

#[cfg(test)]
mod verified_proofs_tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn a_proof_is_verified_once_per_driver_and_only_a_success_is_recorded() {
        let verified = VerifiedProofs::default();
        let runs = Cell::new(0_u32);
        let verify = |outcome: bool| {
            verified.verify_once([7; 32], || {
                runs.set(runs.get() + 1);
                if outcome {
                    Ok(())
                } else {
                    Err(OneCallPrivateSettlementError::Binding)
                }
            })
        };
        // A refused proof is never recorded: the next read verifies it again.
        assert!(matches!(
            verify(false),
            Err(OneCallPrivateSettlementError::Binding)
        ));
        assert!(matches!(
            verify(false),
            Err(OneCallPrivateSettlementError::Binding)
        ));
        assert_eq!(runs.get(), 2);
        verify(true).unwrap();
        assert_eq!(runs.get(), 3);
        // Once verified, the same artifact bytes are not verified again, whatever the caller.
        verify(true).unwrap();
        verify(false).unwrap();
        assert_eq!(runs.get(), 3);
        // Another artifact is another proof.
        verified
            .verify_once([8; 32], || {
                runs.set(runs.get() + 1);
                Ok(())
            })
            .unwrap();
        assert_eq!(runs.get(), 4);
    }

    #[test]
    fn a_proof_the_prover_released_needs_no_second_verification() {
        let verified = VerifiedProofs::default();
        verified.record([9; 32]).unwrap();
        verified
            .verify_once([9; 32], || Err(OneCallPrivateSettlementError::Binding))
            .unwrap();
        // The record is this driver's alone: a fresh one verifies the same bytes.
        assert!(
            VerifiedProofs::default()
                .verify_once([9; 32], || Err(OneCallPrivateSettlementError::Binding))
                .is_err()
        );
    }
}
