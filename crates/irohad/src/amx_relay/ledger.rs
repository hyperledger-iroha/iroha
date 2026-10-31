//! Destination proof verification of an exact relay carrier, including rejected execution.

use std::{
    num::NonZeroU64,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use eyre::{Result, eyre};
use iroha::{
    client::{AccountClient, Client},
    workflow::{
        CommittedOutcome, DispatchOutcome, SignedStepTransaction, StepFailure,
        TransactionIdentityV1, WorkflowAccount, WorkflowBindingV1, WorkflowErrorCode,
        WorkflowFuture, WorkflowLedger,
    },
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::{SignedBlock, consensus::SumeragiRootScope},
    isi::InstructionBox,
    query::CommittedTransaction,
    sumeragi_amx::{AmxDebitApprovalV1, VerifiedAmxDebitApprovalV1},
    sumeragi_finality::{SumeragiFinalityProof, SumeragiFinalityVerifier, VerifiedSumeragiBlock},
    transaction::{
        Executable, FeePaymentIntent, SignedTransaction, TransactionEntrypoint, TransactionPayload,
    },
};

use super::pending_custody::{
    PendingCustody, PendingKey, PendingKind, PendingObservationLease, Progress, Slot,
};
use iroha_version::codec::DecodeVersioned as _;

// Status selects a locator only. Its success/rejection/expiry text is never an outcome.
enum Location {
    Absent,
    Observed(Option<u64>),
}
enum ProofInput {
    Response(iroha::client::SumeragiFinalityProofResponse),
    #[cfg(test)]
    Fixture(Box<SumeragiFinalityProof>),
}
impl ProofInput {
    fn decode_checked(self) -> Result<SumeragiFinalityProof> {
        match self {
            Self::Response(response) => response.decode_checked(),
            #[cfg(test)]
            Self::Fixture(proof) => Ok(*proof),
        }
    }
}

trait Source: Send + Sync {
    fn locate(&self, hash: HashOf<SignedTransaction>) -> WorkflowFuture<'_, Result<Location>>;
    fn proof(&self, height: NonZeroU64) -> WorkflowFuture<'_, Result<ProofInput>>;
    // Untrusted scan bound only: successful effects require independently verified blocks.
    fn applied_height(&self) -> WorkflowFuture<'_, Result<u64>>;
    fn approval<'a>(
        &'a self,
        _approval: &'a AmxDebitApprovalV1,
        _relay: &'a AccountId,
        _schema: Hash,
        _block: &'a VerifiedSumeragiBlock,
    ) -> WorkflowFuture<'a, Result<VerifiedAmxDebitApprovalV1>> {
        Box::pin(async { Err(eyre!("exact AMX debit approval proof is unavailable")) })
    }
}
struct OnlineSource {
    client: Arc<Client>,
    scope: SumeragiRootScope,
}
impl Source for OnlineSource {
    fn approval<'a>(
        &'a self,
        approval: &'a AmxDebitApprovalV1,
        relay: &'a AccountId,
        schema: Hash,
        block: &'a VerifiedSumeragiBlock,
    ) -> WorkflowFuture<'a, Result<VerifiedAmxDebitApprovalV1>> {
        Box::pin(async move {
            self.client
                .get_amx_debit_approval_state_async(approval, relay, schema, block)
                .await
        })
    }
    fn applied_height(&self) -> WorkflowFuture<'_, Result<u64>> {
        Box::pin(async move { Ok(self.client.status().get().await?.blocks) })
    }
    fn locate(&self, hash: HashOf<SignedTransaction>) -> WorkflowFuture<'_, Result<Location>> {
        Box::pin(async move {
            let response = match self.scope {
                SumeragiRootScope::Global => {
                    self.client
                        .fetch_transaction_status_response_global(hash)
                        .await?
                }
                SumeragiRootScope::Dataspace { .. } => {
                    self.client
                        .fetch_transaction_status_response_local(hash)
                        .await?
                }
            };
            Ok(response.map_or(Location::Absent, |response| {
                Location::Observed(response.status.block_height)
            }))
        })
    }
    fn proof(&self, height: NonZeroU64) -> WorkflowFuture<'_, Result<ProofInput>> {
        Box::pin(async move {
            self.client
                .read_sumeragi_finality_proof_response_async(height, self.scope)
                .await
                .map(ProofInput::Response)
        })
    }
}

#[derive(Clone)]
pub(super) struct RelayLedger {
    source: Arc<dyn Source>,
    dispatch: Arc<dyn WorkflowLedger>,
    original: SumeragiFinalityVerifier,
    custody: Arc<PendingCustody>,
    network: NetworkId,
    authority: AccountId,
    fee_payment: FeePaymentIntent,
    instruction: Arc<Mutex<Option<Hash>>>,
    approval: Arc<Mutex<Option<AmxDebitApprovalV1>>>,
    source_plan: Arc<Mutex<Option<[u8; 32]>>>,
    handoff: Arc<Mutex<Option<(PendingKey, bool)>>>,
    max_reads: usize,
    reads: Arc<AtomicUsize>,
    #[cfg(test)]
    test_lease: Arc<Mutex<Option<PendingObservationLease>>>,
}
fn locked<T>(owner: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>> {
    owner
        .lock()
        .map_err(|_| eyre!("AMX destination verifier owner poisoned"))
}
// Classify the original typed source, never its display string. These are
// operational failures at an evidence gate, not authoritative monetary outcomes.
fn approval_evidence_failure(error: eyre::Report) -> StepFailure {
    use iroha_data_model::sumeragi::finality::NativeFinalityDecodeError;
    use iroha_torii_shared::amx_approval_read::{
        AmxApprovalReadFailureV1, AmxApprovalReadReasonV1,
    };
    let http_code = |status| match status {
        401 | 403 => WorkflowErrorCode::AuthorizationDenied,
        404 | 405 | 501 => WorkflowErrorCode::UnsupportedOperation,
        _ => WorkflowErrorCode::EvidenceUnavailable,
    };
    let decode_code = |source: &norito::core::DecodeAttemptError| match source.kind() {
        norito::core::DecodeAttemptErrorKind::Invalid => WorkflowErrorCode::NonCanonicalEncoding,
        norito::core::DecodeAttemptErrorKind::Allocator
        | norito::core::DecodeAttemptErrorKind::EnclosingLimit => {
            WorkflowErrorCode::LocalResourceUnavailable
        }
    };
    let native_decode_code = |error: &NativeFinalityDecodeError| match error {
        NativeFinalityDecodeError::Resource(source) => decode_code(source),
        NativeFinalityDecodeError::Invalid(_) | NativeFinalityDecodeError::Malformed(_) => {
            WorkflowErrorCode::ProofInvalid
        }
    };
    let finality_read_code = |failure: &iroha_data_model::sumeragi_finality::FinalityReadError| {
        use iroha_data_model::sumeragi_finality::{FinalityReadError, GenesisReadError};
        match failure {
            FinalityReadError::Invalid(_)
            | FinalityReadError::Genesis(GenesisReadError::Invalid(_)) => {
                WorkflowErrorCode::ProofInvalid
            }
            FinalityReadError::DecodeResource(source) => decode_code(source),
            FinalityReadError::ScratchLimit(_) | FinalityReadError::Allocator(_) => {
                WorkflowErrorCode::LocalResourceUnavailable
            }
            FinalityReadError::Genesis(GenesisReadError::Json(source)) => {
                // This JSON bound does not retain a classified
                // original decoder attempt. Do not guess capacity from its label.
                if source.is_decode_resource_limit() {
                    WorkflowErrorCode::EvidenceUnavailable
                } else {
                    WorkflowErrorCode::ProofInvalid
                }
            }
        }
    };
    let code = if error
        .downcast_ref::<iroha::client::NativeWorkerError>()
        .is_some()
    {
        WorkflowErrorCode::LocalResourceUnavailable
    } else if let Some(failure) = error.downcast_ref::<super::pending_custody::CustodyError>() {
        use super::pending_custody::CustodyError;
        match failure {
            CustodyError::Binding => WorkflowErrorCode::BindingMismatch,
            CustodyError::Capacity | CustodyError::Io(_) => {
                WorkflowErrorCode::LocalResourceUnavailable
            }
            CustodyError::Busy | CustodyError::Conflict => WorkflowErrorCode::JournalConflict,
            CustodyError::Signing(_) => WorkflowErrorCode::AuthorizationDenied,
            CustodyError::Codec(source) if source.is_decode_resource_limit() => {
                // A raw codec limit retains no classified decoder attempt. It proves neither
                // malformed custody nor an authenticated local-capacity refusal.
                WorkflowErrorCode::EvidenceUnavailable
            }
            CustodyError::Codec(_) => WorkflowErrorCode::NonCanonicalEncoding,
            CustodyError::Finality(source) => finality_read_code(source),
            CustodyError::Execution(_) => WorkflowErrorCode::ProofInvalid,
        }
    } else if let Some(failure) = error.downcast_ref::<AmxApprovalReadFailureV1>() {
        if !failure.is_valid() {
            WorkflowErrorCode::ProofInvalid
        } else {
            match failure.reason {
                AmxApprovalReadReasonV1::Capacity => WorkflowErrorCode::LocalResourceUnavailable,
                AmxApprovalReadReasonV1::CutChanged => WorkflowErrorCode::EvidenceUnavailable,
            }
        }
    } else if let Some(failure) = error.downcast_ref::<iroha::Error>() {
        match failure {
            iroha::Error::Api { error, .. } => http_code(error.status()),
            iroha::Error::Http { status, .. } => http_code(*status),
            iroha::Error::RequestSigning { .. } | iroha::Error::TransactionSigning(_) => {
                WorkflowErrorCode::AuthorizationDenied
            }
            iroha::Error::Context(_) | iroha::Error::ResponseBinding { .. } => {
                WorkflowErrorCode::BindingMismatch
            }
            iroha::Error::InvalidRequest { .. } => WorkflowErrorCode::InvalidInput,
            iroha::Error::Decode { .. } => WorkflowErrorCode::ProofInvalid,
            iroha::Error::ResponseTooLarge { .. } => WorkflowErrorCode::ResourceLimitExceeded,
            iroha::Error::CanonicalDecode(failure) => decode_code(&failure.source),
            iroha::Error::NativeFinalityDecode(failure) => native_decode_code(&failure.source),
            _ => WorkflowErrorCode::EvidenceUnavailable,
        }
    } else if let Some(failure) = error.downcast_ref::<NativeFinalityDecodeError>() {
        native_decode_code(failure)
    } else if let Some(failure) =
        error.downcast_ref::<iroha_data_model::sumeragi_finality::FinalityReadError>()
    {
        finality_read_code(failure)
    } else if error
        .downcast_ref::<iroha_data_model::sumeragi_finality::FinalityError>()
        .is_some()
    {
        WorkflowErrorCode::ProofInvalid
    } else {
        WorkflowErrorCode::EvidenceUnavailable
    };
    StepFailure::new(code, error.to_string())
}

impl RelayLedger {
    pub(super) fn new(
        client: Arc<Client>,
        account: &AccountClient,
        dispatch: Arc<dyn WorkflowLedger>,
        verifier: SumeragiFinalityVerifier,
        scope: SumeragiRootScope,
        fee_payment: FeePaymentIntent,
        max_reads: usize,
        custody: Arc<PendingCustody>,
    ) -> Self {
        Self {
            network: verifier.initial_epoch().network_id,
            authority: account.authority().clone(),
            fee_payment,
            instruction: Arc::new(Mutex::new(None)),
            approval: Arc::new(Mutex::new(None)),
            source_plan: Arc::new(Mutex::new(None)),
            handoff: Arc::new(Mutex::new(None)),
            source: Arc::new(OnlineSource { client, scope }),
            dispatch,
            original: verifier.into_genesis_verifier(),
            custody,
            max_reads,
            reads: Arc::new(AtomicUsize::new(max_reads)),
            #[cfg(test)]
            test_lease: Arc::new(Mutex::new(None)),
        }
    }
    pub(super) fn begin_observation(
        &self,
        source_plan: [u8; 32],
        instruction: Hash,
        approval: Option<AmxDebitApprovalV1>,
        handoff_effect: Option<Hash>,
    ) -> Result<PendingObservationLease> {
        let lease = self
            .custody
            .acquire_observation(source_plan, instruction, handoff_effect)?;
        let carrier = PendingKey {
            source_plan,
            instruction,
            kind: handoff_effect.map_or(PendingKind::Carrier, PendingKind::HandoffCarrier),
        };
        let handoff = handoff_effect.map(|effect| PendingKey {
            source_plan,
            instruction,
            kind: PendingKind::Handoff(effect),
        });
        let mut group = vec![carrier];
        group.extend(handoff.clone());
        self.custody.reserve_group(&group)?;
        *locked(&self.source_plan)? = Some(source_plan);
        *locked(&self.instruction)? = Some(instruction);
        *locked(&self.approval)? = approval;
        *locked(&self.handoff)? = handoff.map(|key| (key, false));
        self.reads.store(self.max_reads, Ordering::Relaxed);
        Ok(lease)
    }

    // Native proof work shares one bounded route worker. Only this already admitted
    // observation's original native lock is passed outside the closure, so cancellation
    // cannot release it while a worker or its thread-local destructors remain alive.
    async fn native<T: Send + 'static>(
        &self,
        work: impl FnOnce(Self) -> T + Send + 'static,
    ) -> Result<T> {
        let (permit, custody) = self.custody.native_custody()?;
        self.check_observation(
            &custody,
            custody.source_plan,
            custody.instruction,
            custody.handoff_effect,
        )?;
        let ledger = self.clone();
        let (output, _custody) =
            iroha::client::run_native_worker(permit, custody, move || work(ledger)).await?;
        Ok(output)
    }

    async fn load_native(
        &self,
        slot: Slot,
    ) -> Result<Option<Box<super::pending_custody::LoadedProgress>>> {
        Ok(self
            .native(move |ledger| ledger.custody.load(&slot).map(|saved| saved.map(Box::new)))
            .await??)
    }

    async fn advance_native(
        &self,
        slot: Slot,
        expected: Option<[u8; 32]>,
        progress: Progress,
        mut candidate: SumeragiFinalityVerifier,
        response: ProofInput,
        height: u64,
        handoff: Option<Hash>,
    ) -> Result<(
        SumeragiFinalityVerifier,
        VerifiedSumeragiBlock,
        [u8; 32],
        bool,
    )> {
        self.native(move |ledger| {
            let proof = response.decode_checked()?;
            if proof.height() != height {
                return Err(eyre!("AMX destination proof differs from requested height"));
            }
            let block = candidate.verify(&proof)?;
            let found = handoff
                .map(|effect| successful_handoff(&block, &ledger.network, effect))
                .transpose()?
                .unwrap_or(false);
            candidate.retain_recent_decisions(3);
            let digest = ledger.custody.compare_and_publish(
                &slot,
                expected.as_ref(),
                progress,
                &candidate,
                &proof,
            )?;
            Ok((candidate, block, digest, found))
        })
        .await?
    }

    /// Signed local presence selects pending scheduling only, never execution or signing.
    pub(super) fn has_pending(
        &self,
        source_plan: [u8; 32],
        instruction: Hash,
        handoff_effect: Option<Hash>,
    ) -> Result<bool> {
        let carrier = PendingKey {
            source_plan,
            instruction,
            kind: handoff_effect.map_or(PendingKind::Carrier, PendingKind::HandoffCarrier),
        };
        let carrier_present = self.custody.contains(&Slot::Pending(carrier))?;
        let handoff_present = if let Some(effect) = handoff_effect {
            self.custody.contains(&Slot::Pending(PendingKey {
                source_plan,
                instruction,
                kind: PendingKind::Handoff(effect),
            }))?
        } else {
            false
        };
        Ok(carrier_present || handoff_present)
    }

    /// Require the original native lease and the exact independently selected route visit.
    pub(super) fn check_observation(
        &self,
        lease: &PendingObservationLease,
        source_plan: [u8; 32],
        instruction: Hash,
        handoff_effect: Option<Hash>,
    ) -> Result<()> {
        self.custody.check_observation(lease)?;
        let actual_handoff = locked(&self.handoff)?
            .as_ref()
            .map(|(key, _)| match key.kind {
                PendingKind::Handoff(effect) => Ok(effect),
                _ => Err(eyre!("AMX observation has a foreign handoff kind")),
            })
            .transpose()?;
        if lease.source_plan != source_plan
            || lease.instruction != instruction
            || lease.handoff_effect != handoff_effect
            || *locked(&self.source_plan)? != Some(source_plan)
            || *locked(&self.instruction)? != Some(instruction)
            || actual_handoff != handoff_effect
        {
            return Err(eyre!("AMX observation differs from its original custody"));
        }
        Ok(())
    }

    #[cfg(test)]
    fn with_test_observation(self) -> Self {
        let instruction = *self.instruction.lock().unwrap();
        let approval = self.approval.lock().unwrap().clone();
        if let Some(instruction) = instruction {
            self.begin_test_observation(instruction, approval).unwrap();
        }
        self
    }

    #[cfg(test)]
    fn begin_test_observation(
        &self,
        instruction: Hash,
        approval: Option<AmxDebitApprovalV1>,
    ) -> Result<()> {
        self.begin_test_bound_observation(*instruction.as_ref(), instruction, approval, None)
    }

    #[cfg(test)]
    fn begin_test_handoff(&self, expected: &InstructionBox) -> Result<()> {
        let instruction = instruction_digest(expected)?;
        self.begin_test_bound_observation(
            *instruction.as_ref(),
            instruction,
            None,
            handoff_effect(expected)?,
        )
    }

    #[cfg(test)]
    fn begin_test_bound_observation(
        &self,
        source_plan: [u8; 32],
        instruction: Hash,
        approval: Option<AmxDebitApprovalV1>,
        handoff: Option<Hash>,
    ) -> Result<()> {
        *self.test_lease.lock().unwrap() = None;
        let lease = self.begin_observation(source_plan, instruction, approval, handoff)?;
        *self.test_lease.lock().unwrap() = Some(lease);
        Ok(())
    }
    fn carrier_key(&self) -> Result<PendingKey> {
        let kind = match locked(&self.handoff)?.as_ref() {
            Some((
                PendingKey {
                    kind: PendingKind::Handoff(effect),
                    ..
                },
                _,
            )) => PendingKind::HandoffCarrier(*effect),
            Some(_) => return Err(eyre!("AMX observation has a foreign handoff kind")),
            None => PendingKind::Carrier,
        };
        Ok(PendingKey {
            source_plan: locked(&self.source_plan)?
                .ok_or_else(|| eyre!("AMX observation is not admitted"))?,
            instruction: locked(&self.instruction)?
                .ok_or_else(|| eyre!("AMX original instruction unavailable"))?,
            kind,
        })
    }

    /// Recover this original effect through its own contiguous inspected prefix.
    pub(super) async fn recover_handoff(&self, expected: &InstructionBox) -> Result<bool> {
        if cfg!(all(test, sumeragi_daemon_mutation = "HC331")) {
            return Ok(false);
        }
        let effect = handoff_effect(expected)?
            .ok_or_else(|| eyre!("original boundary intent has no handoff"))?;
        let key = locked(&self.handoff)?
            .as_ref()
            .map(|(key, _)| key.clone())
            .ok_or_else(|| eyre!("AMX handoff group was not admitted before observation"))?;
        if key.kind != PendingKind::Handoff(effect) {
            return Err(eyre!("AMX handoff differs from admitted source"));
        }
        let slot = Slot::Pending(key.clone());
        let load_slot = slot.clone();
        let (saved, recovered) = self
            .native(move |ledger| {
                let saved = ledger
                    .custody
                    .load(&load_slot)?
                    .ok_or_else(|| eyre!("AMX admitted handoff is missing"))?;
                let recovered = saved
                    .tip
                    .as_ref()
                    .map(|tip| successful_handoff(tip, &ledger.network, effect))
                    .transpose()?
                    .unwrap_or(false);
                Ok::<_, eyre::Report>((Box::new(saved), recovered))
            })
            .await??;
        let mut digest = saved.digest;
        let mut verifier = saved.verifier.unwrap_or_else(|| self.original.clone());
        let captured_tip = match saved.progress {
            Progress::Reserved => self.source.applied_height().await?,
            Progress::Handoff {
                inspected_height,
                captured_tip,
            } => {
                if recovered {
                    *locked(&self.handoff)? = Some((key, true));
                    return Ok(true);
                }
                if inspected_height < captured_tip {
                    captured_tip
                } else {
                    captured_tip.max(self.source.applied_height().await?)
                }
            }
            _ => return Err(eyre!("AMX saved handoff kind differs")),
        };
        if captured_tip < 2 {
            return Ok(false);
        }
        loop {
            let next = verifier
                .authenticated_height()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| eyre!("AMX handoff height overflow"))?;
            if next > captured_tip {
                return Ok(false);
            }
            let response = self.proof(next).await?;
            let (candidate, _, published, found) = self
                .advance_native(
                    slot.clone(),
                    Some(digest),
                    Progress::Handoff {
                        inspected_height: next,
                        captured_tip,
                    },
                    verifier,
                    response,
                    next,
                    (next >= 2).then_some(effect),
                )
                .await?;
            digest = published;
            verifier = candidate;
            if found {
                *locked(&self.handoff)? = Some((key, true));
                return Ok(true);
            }
        }
    }

    /// Called only at the original durable route boundary, while its outer lease remains held.
    /// Business handoff success alone never retires this relayer's unknown exact carrier.
    pub(super) async fn finish_observation(&self, lease: &PendingObservationLease) -> Result<()> {
        let lease = lease.clone();
        self.native(move |ledger| ledger.finish_observation_native(&lease))
            .await?
    }

    fn finish_observation_native(&self, lease: &PendingObservationLease) -> Result<()> {
        self.check_observation(
            lease,
            lease.source_plan,
            lease.instruction,
            lease.handoff_effect,
        )?;
        let key = self.carrier_key()?;
        if key.source_plan != lease.source_plan || key.instruction != lease.instruction {
            return Err(eyre!(
                "AMX completion differs from its original observation lease"
            ));
        }
        let saved = self
            .custody
            .load(&Slot::Pending(key.clone()))?
            .ok_or_else(|| eyre!("AMX admitted carrier custody is missing"))?;
        let carrier_block = match &saved.progress {
            Progress::Reserved => None,
            Progress::Carrier {
                identity,
                target_height,
            } => {
                let tip = saved
                    .tip
                    .as_ref()
                    .ok_or_else(|| eyre!("AMX exact carrier still unresolved"))?;
                if tip.height() != *target_height {
                    return Err(eyre!("AMX exact carrier prefix is unfinished"));
                }
                self.carrier_evidence(tip, identity)?;
                Some(tip)
            }
            _ => return Err(eyre!("AMX exact signed carrier still unresolved")),
        };
        let handoff = locked(&self.handoff)?.clone();
        let handoff_saved = if let Some((handoff, _)) = handoff {
            let PendingKind::Handoff(effect) = handoff.kind else {
                return Err(eyre!("AMX handoff kind changed"));
            };
            let saved_handoff = self
                .custody
                .load(&Slot::Pending(handoff.clone()))?
                .ok_or_else(|| eyre!("AMX handoff custody missing"))?;
            let found = saved_handoff
                .tip
                .as_ref()
                .map(|tip| successful_handoff(tip, &self.network, effect))
                .transpose()?
                .unwrap_or(false)
                || carrier_block
                    .map(|tip| successful_handoff(tip, &self.network, effect))
                    .transpose()?
                    .unwrap_or(false);
            if !found {
                return Err(eyre!("AMX handoff effect still unresolved"));
            }
            Some((handoff, saved_handoff.digest))
        } else {
            None
        };
        self.custody
            .retire_after_durable_route_result(&key, &saved.digest)?;
        if let Some((key, digest)) = handoff_saved {
            self.custody
                .retire_after_durable_route_result(&key, &digest)?;
        }
        self.custody.check_observation(lease)?;
        Ok(())
    }

    // Journal labels and checksums are not authorization. Inspect the actual payload
    // against the independently reconstructed source before signing or dispatching.
    fn check_payload(&self, payload: &TransactionPayload) -> Result<()> {
        let expected = locked(&self.instruction)?
            .ok_or_else(|| eyre!("AMX carrier requires a freshly authenticated source intent"))?;
        let Executable::Instructions(instructions) = payload.instructions() else {
            return Err(eyre!("AMX carrier is not the exact native instruction"));
        };
        let [instruction] = instructions.as_ref() else {
            return Err(eyre!(
                "AMX carrier must contain exactly the original relay instruction"
            ));
        };
        if payload.network_id() != Some(&self.network)
            || payload.authority() != &self.authority
            || instruction_digest(instruction)? != expected
            || !payload.metadata.is_empty()
            || payload.attachments.is_some()
        {
            return Err(eyre!(
                "AMX carrier differs from authenticated source, route authority or fee intent"
            ));
        }
        Ok(())
    }
    fn check_authorized_payload(&self, payload: &TransactionPayload) -> Result<()> {
        self.check_payload(payload)?;
        if payload.fee_payment_intent() != &self.fee_payment {
            return Err(eyre!(
                "AMX carrier fee intent differs from current explicit route authorization"
            ));
        }
        Ok(())
    }
    async fn authorize_debit(&self, payload: &TransactionPayload) -> Result<(), StepFailure> {
        use iroha_data_model::isi::sumeragi_amx::PrepareAmxV1;
        self.check_authorized_payload(payload).map_err(|error| {
            StepFailure::new(WorkflowErrorCode::BindingMismatch, error.to_string())
        })?;
        let key = self.carrier_key().map_err(approval_evidence_failure)?;
        if !self
            .custody
            .contains(&Slot::Pending(key))
            .map_err(|error| approval_evidence_failure(error.into()))?
        {
            return Err(StepFailure::new(
                WorkflowErrorCode::LocalResourceUnavailable,
                "AMX original admission is missing",
            ));
        }
        let Executable::Instructions(instructions) = payload.instructions() else {
            unreachable!("checked native payload")
        };
        if instructions[0]
            .as_any()
            .downcast_ref::<PrepareAmxV1>()
            .is_none()
        {
            return Ok(());
        }
        let approval = locked(&self.approval)
            .map_err(approval_evidence_failure)?
            .clone()
            .ok_or_else(|| {
                StepFailure::new(
                    WorkflowErrorCode::BindingMismatch,
                    "native AMX Prepare has no independently selected debit approval",
                )
            })?;
        // Status is only a locator. Once this route has authenticated a later cut, a
        // stale/Byzantine status reply cannot roll consent verification back to an old
        // grant. A fresh process still establishes a historical fact, not proof that
        // no later revoke exists; native execution always rechecks current permission.
        let height = self
            .source
            .applied_height()
            .await
            .map_err(approval_evidence_failure)?;
        let block = self
            .verified_block(height)
            .await
            .map_err(approval_evidence_failure)?;
        let schema = iroha_core::state::State::native_world_schema_hash_v1()
            .map_err(|error| StepFailure::new(WorkflowErrorCode::BindingMismatch, error))?;
        let verified = self
            .source
            .approval(&approval, &self.authority, schema, &block)
            .await
            .map_err(approval_evidence_failure)?;
        if verified.approval() != &approval
            || verified.relay() != &self.authority
            || verified.height() != block.height()
        {
            return Err(StepFailure::new(
                WorkflowErrorCode::BindingMismatch,
                "native AMX debit approval proof differs from its original intent and relay",
            ));
        }
        Ok(())
    }
    fn check_signed(
        &self,
        bytes: &[u8],
        identity: &TransactionIdentityV1,
    ) -> Result<(), StepFailure> {
        let transaction = SignedTransaction::decode_all_versioned(bytes).map_err(|error| {
            let code = if error.is_local_resource_refusal() {
                WorkflowErrorCode::LocalResourceUnavailable
            } else {
                WorkflowErrorCode::NonCanonicalEncoding
            };
            StepFailure::new(code, error.to_string())
        })?;
        self.check_authorized_payload(transaction.payload())
            .map_err(|error| {
                StepFailure::new(WorkflowErrorCode::BindingMismatch, error.to_string())
            })?;
        transaction.verify_signature().map_err(|error| {
            StepFailure::new(WorkflowErrorCode::BindingMismatch, error.to_string())
        })?;
        let hash = norito::core::classify_decode_attempt(|| transaction.try_hash_as_entrypoint())
            .map(Hash::from)
            .map_err(|error| {
                let code = match error.kind() {
                    norito::core::DecodeAttemptErrorKind::Allocator
                    | norito::core::DecodeAttemptErrorKind::EnclosingLimit => {
                        WorkflowErrorCode::LocalResourceUnavailable
                    }
                    norito::core::DecodeAttemptErrorKind::Invalid => {
                        WorkflowErrorCode::NonCanonicalEncoding
                    }
                };
                StepFailure::new(code, error.to_string())
            })?;
        if hash.as_ref() != &identity.signed_transaction_hash
            || hash.as_ref() != &identity.entrypoint_hash
        {
            return Err(StepFailure::new(
                WorkflowErrorCode::BindingMismatch,
                "AMX signed bytes differ from journal transaction identity",
            ));
        }
        Ok(())
    }
    async fn proof(&self, height: u64) -> Result<ProofInput> {
        self.reads.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| remaining.checked_sub(1))
            .map_err(|_| eyre!("AMX destination proof-read budget exhausted; retained verifier remains resumable"))?;
        self.source
            .proof(NonZeroU64::new(height).ok_or_else(|| eyre!("zero AMX destination locator"))?)
            .await
    }
    /// The live approval cursor is independent of all replayed carrier/handoff walks.
    async fn verified_block(&self, height: u64) -> Result<VerifiedSumeragiBlock> {
        if height < 2 {
            return Err(eyre!("AMX approval must select a non-genesis block"));
        }
        let saved = self.load_native(Slot::LiveApproval).await?;
        let mut digest = saved.as_ref().map(|saved| saved.digest);
        let target = height.max(
            saved
                .as_ref()
                .and_then(|saved| saved.tip.as_ref())
                .map_or(0, |tip| tip.height()),
        );
        if let Some(tip) = saved.as_ref().and_then(|saved| saved.tip.as_ref())
            && tip.height() == target
        {
            return Ok(tip.clone());
        }
        let mut verifier = saved
            .and_then(|saved| saved.verifier)
            .unwrap_or_else(|| self.original.clone());
        loop {
            let next = verifier
                .authenticated_height()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| eyre!("AMX approval height overflow"))?;
            let response = self.proof(next).await?;
            let (candidate, block, published, _) = self
                .advance_native(
                    Slot::LiveApproval,
                    digest,
                    Progress::LiveApproval,
                    verifier,
                    response,
                    next,
                    None,
                )
                .await?;
            digest = Some(published);
            verifier = candidate;
            if next == target {
                return Ok(block);
            }
        }
    }

    async fn select_carrier_identity(&self, identity: &TransactionIdentityV1) -> Result<()> {
        let identity = *identity;
        self.native(move |ledger| ledger.select_carrier_identity_native(&identity))
            .await?
    }

    fn select_carrier_identity_native(&self, identity: &TransactionIdentityV1) -> Result<()> {
        let key = self.carrier_key()?;
        let saved = self
            .custody
            .load(&Slot::Pending(key.clone()))?
            .ok_or_else(|| eyre!("AMX carrier was not admitted before signing"))?;
        match &saved.progress {
            Progress::Reserved => {
                self.custody
                    .bind_signed_identity(&key, &saved.digest, *identity)?;
            }
            Progress::SignedCarrier { identity: original }
            | Progress::Carrier {
                identity: original, ..
            } if original == identity => (),
            Progress::Carrier {
                identity: original,
                target_height,
            } => {
                let tip = saved
                    .tip
                    .as_ref()
                    .ok_or_else(|| eyre!("AMX original carrier remains unresolved"))?;
                if tip.height() != *target_height {
                    return Err(eyre!("AMX original carrier prefix is unfinished"));
                }
                let evidence = self.carrier_evidence(tip, original)?;
                self.custody.rebind_after_durable_rejected_attempt(
                    &key,
                    &saved.digest,
                    *identity,
                    &evidence,
                )?;
            }
            _ => return Err(eyre!("AMX unknown signed identity cannot be replaced")),
        }
        Ok(())
    }

    /// A decoded journal reset cannot mint another identity while original signed
    /// custody remains unresolved. Existing bytes still use dispatch/recovery directly.
    async fn authorize_new_identity(&self) -> Result<(), StepFailure> {
        self.native(move |ledger| ledger.authorize_new_identity_native())
            .await
            .map_err(approval_evidence_failure)?
    }

    fn authorize_new_identity_native(&self) -> Result<(), StepFailure> {
        let key = self.carrier_key().map_err(approval_evidence_failure)?;
        let saved = self
            .custody
            .load(&Slot::Pending(key))
            .map_err(|error| approval_evidence_failure(error.into()))?
            .ok_or_else(|| {
                StepFailure::new(
                    WorkflowErrorCode::LocalResourceUnavailable,
                    "AMX original admission is missing",
                )
            })?;
        if matches!(saved.progress, Progress::Reserved) {
            return Ok(());
        }
        if let Progress::Carrier {
            identity,
            target_height,
        } = saved.progress
            && let Some(tip) = saved.tip
            && tip.height() == target_height
        {
            let evidence = self
                .carrier_evidence(&tip, &identity)
                .map_err(approval_evidence_failure)?;
            if !evidence.result().is_ok() {
                // Only the engine's explicit next ordinal invokes signing after this
                // native gate; current source, fee, consent and retry caps still apply.
                return Ok(());
            }
        }
        Err(StepFailure::new(
            WorkflowErrorCode::EvidenceUnavailable,
            "AMX original identity lacks an authenticated rejection; recover or retransmit its exact bytes",
        ))
    }

    async fn carrier_block(
        &self,
        identity: &TransactionIdentityV1,
        height: u64,
    ) -> Result<VerifiedSumeragiBlock> {
        if height < 2 {
            return Err(eyre!("AMX carrier must select a non-genesis block"));
        }
        self.select_carrier_identity(identity).await?;
        let slot = Slot::Pending(self.carrier_key()?);
        let saved = self
            .load_native(slot.clone())
            .await?
            .ok_or_else(|| eyre!("AMX carrier custody is missing"))?;
        let mut digest = saved.digest;
        let progress = Progress::Carrier {
            identity: *identity,
            target_height: height,
        };
        if saved.tip.as_ref().is_some_and(|tip| tip.height() == height) {
            return self
                .native(move |ledger| {
                    if saved.progress != progress {
                        ledger.custody.compare_and_publish(
                            &slot,
                            Some(&digest),
                            progress,
                            saved
                                .verifier
                                .as_ref()
                                .ok_or_else(|| eyre!("AMX missing native prefix"))?,
                            saved
                                .proof
                                .as_ref()
                                .ok_or_else(|| eyre!("AMX missing native tip"))?,
                        )?;
                    }
                    saved.tip.ok_or_else(|| eyre!("AMX missing native tip"))
                })
                .await?;
        }
        let mut verifier = saved
            .verifier
            .filter(|verifier| {
                verifier
                    .authenticated_height()
                    .is_none_or(|tip| tip < height)
            })
            .unwrap_or_else(|| self.original.clone());
        loop {
            let next = verifier
                .authenticated_height()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| eyre!("AMX carrier height overflow"))?;
            let response = self.proof(next).await?;
            let (candidate, block, published, _) = self
                .advance_native(
                    slot.clone(),
                    Some(digest),
                    progress.clone(),
                    verifier,
                    response,
                    next,
                    None,
                )
                .await?;
            digest = published;
            verifier = candidate;
            if next == height {
                return Ok(block);
            }
        }
    }

    fn carrier_evidence(
        &self,
        verified: &VerifiedSumeragiBlock,
        identity: &TransactionIdentityV1,
    ) -> Result<CommittedTransaction> {
        let hash =
            HashOf::from_untyped_unchecked(Hash::prehashed(identity.signed_transaction_hash));
        let block = verified.block();
        let index = block
            .network_entrypoints()
            .enumerate()
            .find_map(|(index, entry)| {
                let TransactionEntrypoint::External(transaction) = entry else {
                    return None;
                };
                (transaction.hash() == hash
                    && Hash::from(transaction.hash_as_entrypoint()).as_ref()
                        == &identity.entrypoint_hash)
                    .then_some(index)
            })
            .ok_or_else(|| eyre!("AMX destination block lacks the exact signed input"))?;
        let TransactionEntrypoint::External(transaction) = block
            .network_entrypoint_at(index)
            .ok_or_else(|| eyre!("AMX carrier missing"))?
        else {
            return Err(eyre!("AMX carrier is not externally signed"));
        };
        if !cfg!(all(test, sumeragi_daemon_mutation = "HC266")) {
            self.check_payload(transaction.payload())?;
        }
        let evidence = evidence(
            block,
            u32::try_from(index).map_err(|_| eyre!("AMX input index exceeds proof format"))?,
        )?;
        verified.verify_transaction_execution(&self.network, &evidence)?;
        Ok(evidence)
    }

    async fn outcome(&self, identity: &TransactionIdentityV1) -> Result<CommittedOutcome> {
        let hash =
            HashOf::from_untyped_unchecked(Hash::prehashed(identity.signed_transaction_hash));
        let height = match self.source.locate(hash).await? {
            // Exact canonical HTTP absence permits only identical-byte retransmission. It
            // cannot close this identity or authorize a fresh signature.
            Location::Absent => return Ok(CommittedOutcome::ExactAbsence),
            Location::Observed(None) => return Ok(CommittedOutcome::ObservedNotFinal),
            Location::Observed(Some(height)) => height,
        };
        let verified = self.carrier_block(identity, height).await?;
        let identity = *identity;
        self.native(move |ledger| {
            let evidence = ledger.carrier_evidence(&verified, &identity)?;
            if evidence.result().is_ok() {
                Ok(CommittedOutcome::Applied {
                    block_height: Some(height),
                })
            } else {
                Ok(CommittedOutcome::Rejected {
                    detail: "authenticated execution rejected this exact AMX carrier".into(),
                })
            }
        })
        .await?
    }
}

pub(super) struct RelayAccount {
    pub(super) account: Arc<dyn WorkflowAccount>,
    pub(super) ledger: Arc<RelayLedger>,
}
impl WorkflowAccount for RelayAccount {
    fn binding(&self) -> WorkflowBindingV1 {
        self.account.binding()
    }
    fn sign<'a>(
        &'a self,
        unsigned_payload: &'a [u8],
    ) -> WorkflowFuture<'a, Result<SignedStepTransaction, StepFailure>> {
        Box::pin(async move {
            let payload: TransactionPayload = norito::core::classify_decode_attempt(|| {
                norito::decode_canonical(unsigned_payload)
            })
            .map_err(|error| {
                let code = match error.kind() {
                    norito::core::DecodeAttemptErrorKind::Allocator
                    | norito::core::DecodeAttemptErrorKind::EnclosingLimit => {
                        WorkflowErrorCode::LocalResourceUnavailable
                    }
                    norito::core::DecodeAttemptErrorKind::Invalid => {
                        WorkflowErrorCode::NonCanonicalEncoding
                    }
                };
                StepFailure::new(code, error.to_string())
            })?;
            self.ledger
                .check_authorized_payload(&payload)
                .map_err(|error| {
                    StepFailure::new(WorkflowErrorCode::BindingMismatch, error.to_string())
                })?;
            // Any operational refusal retains the exact unsigned reservation;
            // its original category cannot become a monetary rejection or capacity.
            if !cfg!(all(test, sumeragi_daemon_mutation = "HC489")) {
                self.ledger.authorize_new_identity().await?;
            }
            self.ledger.authorize_debit(&payload).await?;
            self.account.sign(unsigned_payload).await
        })
    }
    fn identify(&self, signed_transaction: &[u8]) -> Result<TransactionIdentityV1, StepFailure> {
        self.account.identify(signed_transaction)
    }
}

// A transport response cannot prove the transaction was never admitted anywhere.
fn conservative_dispatch(outcome: DispatchOutcome) -> DispatchOutcome {
    match outcome {
        DispatchOutcome::Refused { detail } | DispatchOutcome::NotAdmitted { detail, .. } => {
            DispatchOutcome::Ambiguous { detail }
        }
        other => other,
    }
}
impl WorkflowLedger for RelayLedger {
    fn dispatch<'a>(
        &'a self,
        bytes: &'a [u8],
        identity: &'a TransactionIdentityV1,
    ) -> WorkflowFuture<'a, DispatchOutcome> {
        Box::pin(async move {
            if let Err(error) = self.check_signed(bytes, identity) {
                return DispatchOutcome::NotAdmitted {
                    code: error.code,
                    detail: error.detail,
                };
            }
            if let Err(error) = self.select_carrier_identity(identity).await {
                let failure = approval_evidence_failure(error);
                return DispatchOutcome::NotAdmitted {
                    code: failure.code,
                    detail: failure.detail,
                };
            }
            let transaction = match SignedTransaction::decode_all_versioned(bytes) {
                Ok(transaction) => transaction,
                Err(error) => {
                    return DispatchOutcome::NotAdmitted {
                        code: if error.is_local_resource_refusal() {
                            WorkflowErrorCode::LocalResourceUnavailable
                        } else {
                            WorkflowErrorCode::NonCanonicalEncoding
                        },
                        detail: error.to_string(),
                    };
                }
            };
            if let Err(error) = self.authorize_debit(transaction.payload()).await {
                return DispatchOutcome::NotAdmitted {
                    code: error.code,
                    detail: error.detail,
                };
            }
            conservative_dispatch(self.dispatch.dispatch(bytes, identity).await)
        })
    }
    fn committed_outcome<'a>(
        &'a self,
        identity: &'a TransactionIdentityV1,
    ) -> WorkflowFuture<'a, CommittedOutcome> {
        Box::pin(async move {
            self.outcome(identity)
                .await
                .unwrap_or_else(|error| CommittedOutcome::ReadFailed {
                    detail: error.to_string(),
                })
        })
    }
}

// Local equality key for a verified handoff scan, never a proof or dispatch authority.
// Both expected and observed handoffs use this sole nominal frame; the borrowed
// original header and result are streamed without copying their private preimages.
#[derive(norito::SerializePayload, norito::NoritoSchema)]
#[norito_schema(name = "irohad::amx_relay::HandoffEffect")]
struct HandoffEffect<'a> {
    direction: u8,
    dataspace: iroha_model_base::topology::DataSpaceId,
    consensus_header: &'a [u8],
    result_preimage: &'a [u8],
}

pub(super) fn handoff_effect(instruction: &InstructionBox) -> Result<Option<Hash>> {
    use iroha_data_model::isi::sumeragi_amx::{RelayAmxHandoffV1, RelayGlobalAmxHandoffV1};
    let (direction, dataspace, proof) =
        if let Some(value) = instruction.as_any().downcast_ref::<RelayAmxHandoffV1>() {
            (0u8, value.dataspace, &value.proof)
        } else if let Some(value) = instruction
            .as_any()
            .downcast_ref::<RelayGlobalAmxHandoffV1>()
        {
            (1u8, value.dataspace, &value.proof)
        } else {
            return Ok(None);
        };
    // A different valid exact-quorum QC may certify the same original boundary.
    // Both its source header and certified result remain byte-exact; destination
    // successful execution authenticates the candidate's own QC and authorization.
    let original = HandoffEffect {
        direction,
        dataspace,
        consensus_header: proof.block.consensus_header.as_slice(),
        result_preimage: proof.block.result_preimage.as_slice(),
    };
    let mut cause = None;
    let hash = Hash::new_from_writer(|writer| {
        norito::core::write_canonical_to_writer(&original, writer).map_err(|error| {
            cause = Some(error);
            std::io::Error::from(std::io::ErrorKind::InvalidData)
        })
    });
    if let Some(cause) = cause {
        return Err(cause.into());
    }
    Ok(Some(hash?))
}

fn successful_handoff(
    verified: &VerifiedSumeragiBlock,
    network: &NetworkId,
    expected: Hash,
) -> Result<bool> {
    for (index, entry) in verified.block().network_entrypoints().enumerate() {
        let TransactionEntrypoint::External(transaction) = entry else {
            continue;
        };
        let Executable::Instructions(instructions) = transaction.instructions() else {
            continue;
        };
        let mut matches = false;
        for instruction in instructions {
            if handoff_effect(instruction)? == Some(expected) {
                matches = true;
                break;
            }
        }
        if !matches {
            continue;
        }
        let index = u32::try_from(index).map_err(|_| eyre!("handoff input index overflow"))?;
        let evidence = evidence(verified.block(), index)?;
        verified.verify_transaction_execution(network, &evidence)?;
        if evidence.result().is_ok() {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn instruction_digest(instruction: &InstructionBox) -> Result<Hash> {
    let mut cause = None;
    let hash = Hash::new_from_writer(|writer| {
        norito::core::write_canonical_to_writer(instruction, writer).map_err(|error| {
            cause = Some(error);
            std::io::Error::from(std::io::ErrorKind::InvalidData)
        })
    });
    if let Some(cause) = cause {
        return Err(cause.into());
    }
    Ok(hash?)
}

fn evidence(block: &SignedBlock, index: u32) -> Result<CommittedTransaction> {
    let entrypoint = block
        .network_entrypoint_at(index as usize)
        .ok_or_else(|| eyre!("AMX carrier input missing"))?
        .clone();
    let (output_index, _) = block
        .network_output_at(index)
        .ok_or_else(|| eyre!("AMX carrier execution missing"))?;
    let output = block
        .execution_outputs()
        .get(output_index as usize)
        .ok_or_else(|| eyre!("AMX output missing"))?
        .clone();
    Ok(CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash: entrypoint.hash(),
        entrypoint_proof: block
            .network_input_proof(index)
            .ok_or_else(|| eyre!("AMX input witness missing"))?,
        entrypoint,
        output_hash: HashOf::new(&output),
        output_proof: block
            .output_proof(output_index)
            .ok_or_else(|| eyre!("AMX output witness missing"))?,
        output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::workflow::WorkflowErrorCode;

    #[test]
    fn transport_refusal_cannot_close_an_amx_identity() {
        for outcome in [
            DispatchOutcome::Refused {
                detail: "rejected".into(),
            },
            DispatchOutcome::NotAdmitted {
                code: WorkflowErrorCode::LocalResourceUnavailable,
                detail: "busy".into(),
            },
        ] {
            assert!(matches!(
                conservative_dispatch(outcome),
                DispatchOutcome::Ambiguous { .. }
            ));
        }
        assert_eq!(
            conservative_dispatch(DispatchOutcome::Accepted),
            DispatchOutcome::Accepted
        );
        assert_eq!(
            conservative_dispatch(DispatchOutcome::AlreadyKnown),
            DispatchOutcome::AlreadyKnown
        );
    }
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod proof_tests;

#[cfg(test)]
#[path = "retry_tests.rs"]
mod retry_tests;

#[cfg(test)]
#[path = "handoff_recovery_tests.rs"]
mod handoff_recovery_tests;

#[cfg(test)]
#[path = "online_tests.rs"]
mod online_tests;

#[cfg(test)]
#[path = "handoff_effect_tests.rs"]
mod handoff_effect_tests;

#[cfg(test)]
#[path = "approval_tests.rs"]
mod approval_tests;

#[cfg(test)]
#[path = "expiry_recovery_tests.rs"]
mod expiry_recovery_tests;

#[cfg(test)]
#[path = "ledger_pending_progress_tests.rs"]
mod pending_progress_tests;
