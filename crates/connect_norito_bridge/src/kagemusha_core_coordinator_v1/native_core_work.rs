//! Concrete consuming native sender pipeline. Physical sources supply originals, never dispatch.

use super::{
    KagemushaCoreCoordinatorBackendErrorV1 as Error,
    archives::{KagemushaCoreSenderCandidateArchiveV1, KagemushaCoreSenderPreparationArchiveV1},
    core_authorization_signer::RetainedCoreAuthorizationSignerV1,
    sender_observation::AuthenticatedSenderReplyV1,
};
use crate::kagemusha_device_bridge_v1::sender_payload::{
    SenderCommandBodyV1, SenderCommandV1, SenderPhaseV1, SenderPreparationSelectorV1,
    SenderRecordV1, SenderReplyBodyV1, SenderWalletContextV1,
};
use iroha_core_zk::{
    kagemusha_v1_recursion::{
        KagemushaArtifactByteResolverV1, KagemushaArtifactErrorV1, KagemushaProductionProverV1,
        KagemushaProductionTerminalProofV1, KagemushaRecursiveVerifierProfileV1,
    },
    kagemusha_v1_state::{
        HardwareTransitionCertificateV1, KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1,
        KagemushaAuthenticatedCoreOwnerV1, KagemushaAuthenticatedCorePublicationV1,
        KagemushaAuthenticatedOutgoingCommitV1, KagemushaAuthenticatedOutgoingProvingSelectionV1,
        KagemushaOriginalOutgoingHardwareCommitV1, KagemushaOutgoingOperationPhaseV1,
        KagemushaOutgoingOperationRecordV1, PreparedOutgoingRecoveryViewV1,
        RedeemSplitPreparationV1, SendSplitPreparationV1, TransitionAuthorizationV1,
    },
};
use iroha_data_model::kagemusha::{
    KagemushaArtifactBindingV1, KagemushaAuthenticatedReleaseV1, KagemushaCommitCertificateV1,
    KagemushaPaymentV1,
};
use std::{
    path::{Component, Path, PathBuf},
    sync::{Arc, OnceLock},
};

type Result<T> = std::result::Result<T, Error>;

/// Original sealed physical preparation for the actual signed op5/6; no Core owner is accepted.
pub enum KagemushaNativeSenderPreparationV1 {
    Send(SendSplitPreparationV1),
    Redemption(RedeemSplitPreparationV1),
}

/// Native-selected exact durable destination. C/JNI requests contain neither field.
#[derive(Clone)]
pub struct KagemushaNativeCorePublicationDestinationV1 {
    pub directory: PathBuf,
    pub checkpoint_operation_id: [u8; 32],
}

/// Original qualified hardware evidence for the same native outgoing preparation.
/// Every value is independently verified by the concrete Core before any private mutation.
#[derive(Clone)]
pub struct KagemushaNativeOutgoingCompletionOriginalsV1 {
    /// Exact original irreversible commit certificate, correlated with signed op7.
    pub certificate: KagemushaCommitCertificateV1,
    /// Original native State Guard certificate, not a host-verified capability.
    pub state_guard: HardwareTransitionCertificateV1,
    /// Original reference used by this prepared State Guard; not a mobile clock.
    pub prepared_reference_ms: u64,
    /// Independently selected fixed publication destination.
    pub destination: KagemushaNativeCorePublicationDestinationV1,
}

/// Independently installed physical/private-input and original artifact custody.
/// This interface cannot implement software Core dispatch or substitute a proof verifier.
pub trait KagemushaNativeCoreWorkSourceV1: Send + Sync + 'static {
    fn recheck_originals(&self) -> Result<()>;
    fn sender_preparation(
        &self,
        owner: &KagemushaAuthenticatedCoreOwnerV1,
        original_command: &[u8],
        original_reply: &[u8],
    ) -> Result<KagemushaNativeSenderPreparationV1>;
    fn publication_destination(
        &self,
        owner: &KagemushaAuthenticatedCoreOwnerV1,
        operation_id: [u8; 32],
        stage: u8,
    ) -> Result<KagemushaNativeCorePublicationDestinationV1>;
    fn prover_originals(
        &self,
        selection: &KagemushaAuthenticatedOutgoingProvingSelectionV1<'_>,
    ) -> Result<(
        KagemushaRecursiveVerifierProfileV1,
        Arc<dyn KagemushaArtifactByteResolverV1>,
    )>;
    fn commit_originals(
        &self,
        owner: &KagemushaAuthenticatedCoreOwnerV1,
        operation_id: [u8; 32],
    ) -> Result<(PathBuf, [u8; 32])>;
    /// Select actual originals for one reservation, mint stage or peer stage.
    fn incoming_stage_originals(
        &self,
        owner: &KagemushaAuthenticatedCoreOwnerV1,
        kind: u32,
        credit_id: [u8; 32],
    ) -> Result<KagemushaNativeIncomingStageOriginalsV1>;
    /// Select independently governed native time, nonce and durable paths for this fold.
    fn incoming_fold_originals(
        &self,
        owner: &KagemushaAuthenticatedCoreOwnerV1,
        kind: u32,
        credit_id: [u8; 32],
    ) -> Result<KagemushaNativeIncomingFoldOriginalsV1>;
    /// Select exact production artifacts under the actual exclusive incoming proving admission.
    fn incoming_prover_originals(
        &self,
        selection: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedIncomingProvingSelectionV1<'_>,
    ) -> Result<(
        KagemushaRecursiveVerifierProfileV1,
        Arc<dyn KagemushaArtifactByteResolverV1>,
    )>;
    /// Re-select the same original destination for a genuine restored exclusive fold.
    fn incoming_recovery_destination(
        &self,
        original: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedIncomingFoldV1,
    ) -> Result<KagemushaNativeCorePublicationDestinationV1>;
    /// Read native physical one-use nonce and original durable paths for an exact verified ACK.
    fn payment_release_originals(
        &self,
        selection: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedPaymentReleaseSelectionV1<'_>,
    ) -> Result<KagemushaNativeOutboxReleaseOriginalsV1>;
    /// Re-select the same publication destination for a restored exclusive outbox release.
    /// This must also work after its completion is fsynced: current-recovery/dispatch accessors
    /// are then intentionally closed, so only retained original completion custody is used.
    fn outbox_release_recovery_destination(
        &self,
        original: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOutboxReleaseV1,
    ) -> Result<KagemushaNativeCorePublicationDestinationV1>;
    /// Locate original completed release storage after process recovery or locator retirement.
    /// Paths are only lookup hints; the installed native Core reopens and verifies exact private
    /// WAL bytes, current Released state and the original signature before accepting a retry.
    /// No mobile frame chooses either directory or the checkpoint identity.
    /// TODO: physical sources must implement durable lookup for evicted or process-recovered IDs.
    fn completed_outbox_release_locator(
        &self,
        _owner: &KagemushaAuthenticatedCoreOwnerV1,
        _operation_id: [u8; 32],
    ) -> Result<KagemushaNativeCompletedOutboxReleaseLocatorV1> {
        Err(Error::Unavailable)
    }
    /// Read original hardware evidence only for this exclusive retained native command.
    fn outgoing_completion_originals(
        &self,
        commit: &KagemushaAuthenticatedOutgoingCommitV1,
        canonical_command: &[u8],
        original_response: &[u8],
    ) -> Result<KagemushaNativeOutgoingCompletionOriginalsV1>;
    /// Select original signed artifact bytes for the genuine committed proving owner.
    fn committed_prover_originals(
        &self,
        selection: &KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_>,
    ) -> Result<(
        KagemushaRecursiveVerifierProfileV1,
        Arc<dyn KagemushaArtifactByteResolverV1>,
    )>;
}

#[path = "native_incoming_work.rs"]
mod incoming_work;
pub use incoming_work::{
    KagemushaNativeIncomingEvidenceSourceV1, KagemushaNativeIncomingFoldOriginalsV1,
    KagemushaNativeIncomingStageOriginalsV1, register_kagemusha_native_incoming_evidence_source_v1,
};

#[path = "native_payment_release.rs"]
mod payment_release;
pub use payment_release::{
    KagemushaNativeCompletedOutboxReleaseLocatorV1, KagemushaNativeOutboxReleaseOriginalsV1,
    KagemushaNativeRedemptionFinalitySourceV1,
    register_kagemusha_native_redemption_finality_source_v1,
};

static SOURCE: OnceLock<Arc<dyn KagemushaNativeCoreWorkSourceV1>> = OnceLock::new();
/// Rust-only immutable installation. A caller-selected release/root or mobile callback cannot
/// reach this intake. Registering source metadata itself grants no proof or monetary authority.
pub fn register_kagemusha_native_core_work_source_v1(
    source: Arc<dyn KagemushaNativeCoreWorkSourceV1>,
) -> Result<()> {
    source.recheck_originals()?;
    SOURCE.set(source).map_err(|_| Error::Rejected)
}

#[derive(Clone)]
pub(super) struct Resolver(pub(super) Arc<dyn KagemushaArtifactByteResolverV1>);
impl KagemushaArtifactByteResolverV1 for Resolver {
    fn resolve_bytes(
        &self,
        binding: KagemushaArtifactBindingV1,
    ) -> std::result::Result<Arc<[u8]>, KagemushaArtifactErrorV1> {
        self.0.resolve_bytes(binding)
    }
    fn open_reader(
        &self,
        binding: KagemushaArtifactBindingV1,
    ) -> std::result::Result<Box<dyn std::io::Read + Send>, KagemushaArtifactErrorV1> {
        self.0.open_reader(binding)
    }
}

enum Stage {
    Selected(Box<KagemushaAuthenticatedCoreOwnerV1>),
    Publication(Box<KagemushaAuthenticatedCorePublicationV1>),
    Commit {
        original: Box<KagemushaAuthenticatedOutgoingCommitV1>,
        request: Vec<Vec<u8>>,
        candidate: KagemushaCoreSenderCandidateArchiveV1,
        canonical_command: Vec<u8>,
    },
    OutboxRelease(Box<iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOutboxReleaseV1>),
    Incoming(Box<iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedIncomingFoldV1>),
    Frozen,
}

struct TerminalAttempt {
    request: Vec<Vec<u8>>,
    operation_id: [u8; 32],
    canonical_command: Vec<u8>,
    completion: Option<KagemushaNativeOutgoingCompletionOriginalsV1>,
    candidate_proof: Option<iroha_data_model::kagemusha::KagemushaPairedProofV1>,
    final_destination: Option<KagemushaNativeCorePublicationDestinationV1>,
}

/// Process-private exclusive holder. No old usable owner escapes a staged transition.
pub(super) struct NativeCoreWorkOwnerV1 {
    path: String,
    stage: Stage,
    original_enrollment: iroha_core_zk::kagemusha_v1_state::KagemushaRecoveryEnrollmentBindingV1,
    selected_source: super::enrolled_open::EnrolledOpenAuthoritySourceV1,
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    terminal_attempt: Option<TerminalAttempt>,
    incoming_attempt: Option<incoming_work::IncomingAttempt>,
    stage_request: Option<Vec<Vec<u8>>>,
    release_attempt: Option<payment_release::ReleaseAttempt>,
    // Bounded actual-origin path hints only. Every use requires original native WAL proof.
    completed_release_locators:
        std::collections::BTreeMap<[u8; 32], KagemushaNativeCompletedOutboxReleaseLocatorV1>,
}
impl NativeCoreWorkOwnerV1 {
    pub(super) fn new(path: String, owner: KagemushaAuthenticatedCoreOwnerV1) -> Result<Self> {
        Self::new_or_retain(path, owner).map_err(|(_, error)| error)
    }
    // All fallible admission happens while the original actual owner remains retained.
    pub(super) fn new_or_retain(
        path: String,
        owner: KagemushaAuthenticatedCoreOwnerV1,
    ) -> std::result::Result<Self, (KagemushaAuthenticatedCoreOwnerV1, Error)> {
        let admitted = (|| {
            let selected = owner
                .current_recovery_selection()
                .map_err(|_| Error::Rejected)?;
            let binding = selected.enrollment_binding().clone();
            let source = super::enrolled_open::authenticated_recovery_source(&selected)
                .map_err(|_| Error::Rejected)?;
            let release = owner.authenticated_release().map_err(|_| Error::Rejected)?;
            Ok((binding, source, release))
        })();
        let (original_enrollment, selected_source, release) = match admitted {
            Ok(value) => value,
            Err(error) => return Err((owner, error)),
        };
        Ok(Self {
            path,
            stage: Stage::Selected(Box::new(owner)),
            original_enrollment,
            selected_source,
            release,
            terminal_attempt: None,
            incoming_attempt: None,
            stage_request: None,
            release_attempt: None,
            completed_release_locators: std::collections::BTreeMap::new(),
        })
    }
    /// Recover only a genuine original native cap and its already fsynced Core command.
    pub(super) fn from_pending_commit(
        path: String,
        commit: KagemushaAuthenticatedOutgoingCommitV1,
    ) -> Result<Self> {
        let selected = commit
            .current_recovery_selection()
            .map_err(|_| Error::Rejected)?;
        let original_enrollment = selected.enrollment_binding().clone();
        let selected_source = super::enrolled_open::authenticated_recovery_source(&selected)
            .map_err(|_| Error::Rejected)?;
        let release = commit
            .authenticated_release()
            .map_err(|_| Error::Rejected)?;
        let canonical_command = commit
            .original_command()
            .map_err(|_| Error::Rejected)?
            .ok_or(Error::Unavailable)?
            .to_vec();
        let record = commit.operation_record().map_err(|_| Error::Rejected)?;
        let command =
            SenderCommandV1::decode_canonical_exact(7, record.operation_id, &canonical_command)
                .map_err(|_| Error::Rejected)?;
        let SenderCommandBodyV1::Commit {
            selector,
            candidate_digest,
            hardware_authorization,
        } = command.body
        else {
            return Err(Error::Rejected);
        };
        let candidate = KagemushaCoreSenderCandidateArchiveV1 {
            version: 1,
            preparation: KagemushaCoreSenderPreparationArchiveV1 {
                version: 1,
                operation_id: record.operation_id,
                context: record.context,
                inputs_digest: record.inputs_digest,
            },
            selector,
            candidate_digest,
            hardware_commit_authorization: hardware_authorization,
        };
        candidate.validate_shape().map_err(|_| Error::Rejected)?;
        commit.recheck_originals().map_err(|_| Error::Rejected)?;
        Ok(Self {
            path,
            stage: Stage::Commit {
                original: Box::new(commit),
                request: Vec::new(),
                candidate,
                canonical_command,
            },
            original_enrollment,
            selected_source,
            release,
            terminal_attempt: None,
            incoming_attempt: None,
            stage_request: None,
            release_attempt: None,
            completed_release_locators: std::collections::BTreeMap::new(),
        })
    }
    pub(super) fn current_recovery_selection(
        &self,
    ) -> Result<iroha_core_zk::kagemusha_v1_state::KagemushaCurrentRecoverySelectionV1<'_>> {
        match &self.stage {
            Stage::Selected(owner) => owner
                .current_recovery_selection()
                .map_err(|_| Error::Rejected),
            Stage::Commit { original, .. } if self.terminal_attempt.is_none() => original
                .current_recovery_selection()
                .map_err(|_| Error::Rejected),
            Stage::Incoming(original)
                if self
                    .incoming_attempt
                    .as_ref()
                    .is_none_or(|attempt| attempt.completion.is_none()) =>
            {
                original
                    .current_recovery_selection()
                    .map_err(|_| Error::Rejected)
            }
            Stage::OutboxRelease(original)
                if self
                    .release_attempt
                    .as_ref()
                    .is_none_or(|a| a.completion.is_none()) =>
            {
                original
                    .current_recovery_selection()
                    .map_err(|_| Error::Rejected)
            }
            _ => Err(Error::Rejected),
        }
    }
    pub(super) fn authenticated_release(&self) -> Result<Arc<KagemushaAuthenticatedReleaseV1>> {
        self.recheck_originals()?;
        Ok(self.release.clone())
    }
    pub(super) fn authority_source(
        &self,
    ) -> Result<super::enrolled_open::EnrolledOpenAuthoritySourceV1> {
        self.recheck_originals()?;
        let selected = match &self.stage {
            Stage::Selected(owner) => Some(
                owner
                    .current_recovery_selection()
                    .map_err(|_| Error::Rejected)?,
            ),
            // The exclusive cap retains the original lease solely for exact completion retry.
            // It cannot yield a usable owner or authorize a new observation/operation.
            Stage::Commit { .. } => None,
            Stage::Publication(_) | Stage::Incoming(_) | Stage::OutboxRelease(_) => None,
            Stage::Frozen => return Err(Error::Rejected),
        };
        if let Some(selected) = selected {
            if selected.enrollment_binding() != &self.original_enrollment {
                return Err(Error::Rejected);
            }
            super::enrolled_open::authenticated_recovery_source(&selected)
                .map_err(|_| Error::Rejected)
        } else {
            Ok(self.selected_source.clone())
        }
    }
    pub(super) fn selected(&self) -> Result<&KagemushaAuthenticatedCoreOwnerV1> {
        match &self.stage {
            Stage::Selected(owner) => Ok(owner),
            _ => Err(Error::Rejected),
        }
    }
    pub(super) fn selected_mut(&mut self) -> Result<&mut KagemushaAuthenticatedCoreOwnerV1> {
        match &mut self.stage {
            Stage::Selected(owner) => Ok(owner),
            _ => Err(Error::Rejected),
        }
    }
    pub(super) fn recheck_originals(&self) -> Result<()> {
        match &self.stage {
            Stage::Selected(owner) => owner
                .current_recovery_selection()
                .map(|_| ())
                .map_err(|_| Error::Rejected),
            Stage::Publication(pending) => pending.recheck_originals().map_err(|_| Error::Rejected),
            Stage::Commit { original, .. } => {
                original.recheck_originals().map_err(|_| Error::Rejected)
            }
            Stage::Incoming(original) => original
                .recheck_retry_originals()
                .map_err(|_| Error::Rejected),
            Stage::OutboxRelease(original) => {
                original.recheck_originals().map_err(|_| Error::Rejected)
            }
            Stage::Frozen => Err(Error::Rejected),
        }
    }
    // Only the same original publication can finish; a lost result retains its exclusive owner.
    pub(super) fn resume_publication(&mut self) -> Result<bool> {
        self.recheck_originals()?;
        if !matches!(self.stage, Stage::Publication(_)) {
            return Ok(false);
        }
        let Stage::Publication(pending) = std::mem::replace(&mut self.stage, Stage::Frozen) else {
            return Err(Error::Rejected);
        };
        match pending.finish() {
            Ok(owner) => {
                let selected = owner
                    .current_recovery_selection()
                    .map_err(|_| Error::Rejected)?;
                if selected.enrollment_binding() != &self.original_enrollment {
                    return Err(Error::Rejected);
                }
                self.selected_source =
                    super::enrolled_open::authenticated_recovery_source(&selected)
                        .map_err(|_| Error::Rejected)?;
                self.stage = Stage::Selected(Box::new(owner));
                Ok(true)
            }
            Err((pending, _)) => {
                self.stage = Stage::Publication(pending);
                Err(Error::Unavailable)
            }
        }
    }
    fn publish(&mut self, pending: KagemushaAuthenticatedCorePublicationV1) -> Result<()> {
        self.stage = Stage::Publication(Box::new(pending));
        self.resume_publication().map(|_| ())
    }
    fn take_selected(&mut self) -> Result<KagemushaAuthenticatedCoreOwnerV1> {
        let Stage::Selected(owner) = std::mem::replace(&mut self.stage, Stage::Frozen) else {
            return Err(Error::Rejected);
        };
        Ok(*owner)
    }
    fn destination(&self, destination: &KagemushaNativeCorePublicationDestinationV1) -> Result<()> {
        require_child(Path::new(&self.path), &destination.directory)?;
        if destination.checkpoint_operation_id == [0; 32] {
            return Err(Error::Rejected);
        }
        Ok(())
    }

    /// Authenticate and publish actual preparation/proof before creating the exclusive command7.
    /// Public archives only choose already retained native intent and original signed device bytes.
    pub(super) fn prove_sender(
        &mut self,
        fields: &[Vec<u8>],
        signed: &AuthenticatedSenderReplyV1,
        signer: &RetainedCoreAuthorizationSignerV1,
    ) -> Result<KagemushaCoreSenderCandidateArchiveV1> {
        if fields.len() != 2 {
            return Err(Error::Rejected);
        }
        let preparation =
            KagemushaCoreSenderPreparationArchiveV1::decode_canonical_exact(&fields[0])
                .map_err(|_| Error::Rejected)?;
        signed.require_original_reply(&fields[1])?;
        if signed.command().operation_id != preparation.operation_id
            || signed.command().context != preparation.context
        {
            return Err(Error::Rejected);
        }
        if let Stage::Commit {
            request, candidate, ..
        } = &self.stage
        {
            if (!request.is_empty() && request != fields) || candidate.preparation != preparation {
                return Err(Error::Rejected);
            }
            let result = candidate.clone();
            self.recheck_originals()?;
            if let Stage::Commit { request, .. } = &mut self.stage {
                if request.is_empty() {
                    *request = fields.to_vec();
                }
            }
            return Ok(result);
        }
        let source = SOURCE.get().ok_or(Error::Unavailable)?.clone();
        source.recheck_originals()?;
        self.resume_publication()?;
        let observed = signed_record(signed)?;
        if observed.phase != SenderPhaseV1::Prepared
            || observed.inputs_digest != preparation.inputs_digest
        {
            return Err(Error::Rejected);
        }
        let recovered = self
            .selected()?
            .recover_coordinator_sender_intent(preparation.operation_id)
            .map_err(|_| Error::Rejected)?;
        match recovered {
            iroha_core_zk::kagemusha_v1_state::KagemushaCoordinatorSenderIntentRecoveryV1::Intent(intent) => {
                if intent.context != preparation.context || intent.canonical_digest().map_err(|_| Error::Rejected)? != preparation.inputs_digest { return Err(Error::Rejected); }
                let physical = source.sender_preparation(self.selected()?, signed.original_command(), signed.original_reply())?;
                let destination = source.publication_destination(self.selected()?, preparation.operation_id, 1)?;
                self.destination(&destination)?;
                source.recheck_originals()?;
                let owner = self.take_selected()?;
                let pending = match physical {
                    KagemushaNativeSenderPreparationV1::Send(input) => owner.stage_send_split(&destination.directory, destination.checkpoint_operation_id, preparation.operation_id, input),
                    KagemushaNativeSenderPreparationV1::Redemption(input) => owner.stage_redeem_split(&destination.directory, destination.checkpoint_operation_id, preparation.operation_id, input),
                }.map_err(|_| Error::Rejected)?;
                require_record_match(&pending.outgoing_operation_record(preparation.operation_id).map_err(|_| Error::Rejected)?, observed)?;
                self.publish(pending)?;
            }
            iroha_core_zk::kagemusha_v1_state::KagemushaCoordinatorSenderIntentRecoveryV1::Indexed(record) => { require_record_match(&record, observed)?; }
            _ => return Err(Error::Rejected),
        }
        source.recheck_originals()?;
        let record = self
            .selected()?
            .recover_coordinator_sender_intent(preparation.operation_id)
            .map_err(|_| Error::Rejected)?;
        let iroha_core_zk::kagemusha_v1_state::KagemushaCoordinatorSenderIntentRecoveryV1::Indexed(
            record,
        ) = record
        else {
            return Err(Error::Rejected);
        };
        if record.phase == KagemushaOutgoingOperationPhaseV1::Prepared {
            let proof = {
                let selection = self
                    .selected()?
                    .outgoing_proving_selection(preparation.operation_id)
                    .map_err(|_| Error::Rejected)?;
                let (profile, resolver) = source.prover_originals(&selection)?;
                let prover =
                    KagemushaProductionProverV1::load(&selection, profile, Resolver(resolver))
                        .map_err(|_| Error::Rejected)?;
                let hash_claim = prover
                    .prove_outgoing_state_hash_claim(&selection)
                    .map_err(|_| Error::Rejected)?;
                prover
                    .prove_outgoing_state(&selection, &hash_claim)
                    .map_err(|_| Error::Rejected)?
                    .proof
            };
            let destination =
                source.publication_destination(self.selected()?, preparation.operation_id, 2)?;
            self.destination(&destination)?;
            source.recheck_originals()?;
            let pending = self
                .take_selected()?
                .stage_outgoing_state_proof(
                    &destination.directory,
                    destination.checkpoint_operation_id,
                    preparation.operation_id,
                    proof,
                )
                .map_err(|_| Error::Rejected)?;
            self.publish(pending)?;
        }
        let (directory, original_nonce) =
            source.commit_originals(self.selected()?, preparation.operation_id)?;
        require_child(Path::new(&self.path), &directory)?;
        if original_nonce == [0; 32] {
            return Err(Error::Rejected);
        }
        source.recheck_originals()?;
        let mut commit = self
            .take_selected()?
            .prepare_outgoing_commit(&directory, preparation.operation_id)
            .map_err(|_| Error::Rejected)?;
        let record = commit.operation_record().map_err(|_| Error::Rejected)?;
        require_record_match(&record, observed)?;
        let authorization = signer.sign_outgoing_commit(&commit, original_nonce)?;
        let candidate = KagemushaCoreSenderCandidateArchiveV1 {
            version: 1,
            preparation,
            selector: SenderPreparationSelectorV1 {
                inputs_digest: record.inputs_digest,
                preparation_id: record.preparation_id,
            },
            candidate_digest: record.candidate_digest.ok_or(Error::Rejected)?,
            hardware_commit_authorization: authorization,
        };
        candidate.validate_shape().map_err(|_| Error::Rejected)?;
        let canonical_command = SenderCommandV1 {
            version: 1,
            operation: 7,
            operation_id: candidate.preparation.operation_id,
            context: candidate.preparation.context.clone(),
            body: SenderCommandBodyV1::Commit {
                selector: candidate.selector,
                candidate_digest: candidate.candidate_digest,
                hardware_authorization: candidate.hardware_commit_authorization.clone(),
            },
        }
        .encode_canonical()
        .map_err(|_| Error::Rejected)?;
        commit
            .retain_original_command(&canonical_command)
            .map_err(|_| Error::Rejected)?;
        self.stage = Stage::Commit {
            original: Box::new(commit),
            request: fields.to_vec(),
            candidate: candidate.clone(),
            canonical_command,
        };
        source.recheck_originals()?;
        self.recheck_originals()?;
        Ok(candidate)
    }

    /// Public observation selections are borrowed only from the actual selected predecessor.
    pub(super) fn sender_observation_selection(
        &self,
        operation: [u8; 32],
    ) -> Result<(
        SenderWalletContextV1,
        SenderWalletContextV1,
        [u8; 32],
        [u8; 32],
    )> {
        self.recheck_originals()?;
        let (context, digest) = match &self.stage {
            Stage::Selected(owner) => match owner.recover_coordinator_sender_intent(operation).map_err(|_| Error::Rejected)? {
                iroha_core_zk::kagemusha_v1_state::KagemushaCoordinatorSenderIntentRecoveryV1::Intent(intent) => {
                    let digest = intent.canonical_digest().map_err(|_| Error::Rejected)?;
                    (intent.context, digest)
                }
                iroha_core_zk::kagemusha_v1_state::KagemushaCoordinatorSenderIntentRecoveryV1::Indexed(record) => (record.context, record.inputs_digest),
                _ => return Err(Error::Rejected),
            },
            Stage::Commit { candidate, .. } if candidate.preparation.operation_id == operation && self.terminal_attempt.is_none() =>
                (candidate.preparation.context.clone(), candidate.preparation.inputs_digest),
            Stage::OutboxRelease(cap) if cap.operation_id().map_err(|_| Error::Rejected)? == operation => {
                let command = SenderCommandV1::decode_canonical_exact(12, operation, cap.original_command().map_err(|_| Error::Rejected)?)
                    .map_err(|_| Error::Rejected)?;
                let digest = command.expected_inputs_digest().map_err(|_| Error::Rejected)?.ok_or(Error::Rejected)?;
                (command.context, digest)
            }
            _ => return Err(Error::Rejected),
        };
        let current = match &self.stage {
            Stage::Selected(owner) => owner.sender_context().map_err(|_| Error::Rejected)?,
            Stage::Commit { candidate, .. } => candidate.preparation.context.clone(),
            Stage::OutboxRelease(cap) => cap.sender_context().map_err(|_| Error::Rejected)?,
            _ => return Err(Error::Rejected),
        };
        self.recheck_originals()?;
        Ok((
            current,
            context,
            digest,
            self.release.provider_policy_root(),
        ))
    }

    /// Require the full original signed op7 before consuming native funds, then produce and
    /// durably publish the genuine terminal proof. Every uncertainty retains the same owner.
    pub(super) fn build_terminal(&mut self, fields: &[Vec<u8>]) -> Result<Vec<u8>> {
        if fields.len() != 2 {
            return Err(Error::Rejected);
        }
        let candidate = KagemushaCoreSenderCandidateArchiveV1::decode_canonical_exact(&fields[0])
            .map_err(|_| Error::Rejected)?;
        if let Some(retained) = &self.terminal_attempt {
            if retained.request != fields {
                return Err(Error::Rejected);
            }
        } else {
            let Stage::Commit {
                candidate: retained,
                canonical_command,
                ..
            } = &self.stage
            else {
                return Err(Error::Rejected);
            };
            if retained != &candidate {
                return Err(Error::Rejected);
            }
            // Structural framing is checked now; the concrete Core independently authenticates
            // signature, exact command, qualification and irreversible certificate before funds.
            iroha_data_model::kagemusha::kagemusha_decode_device_success_response_v1(
                &fields[1],
                7,
                candidate.preparation.operation_id,
            )
            .map_err(|_| Error::Rejected)?;
            self.terminal_attempt = Some(TerminalAttempt {
                request: fields.to_vec(),
                operation_id: candidate.preparation.operation_id,
                canonical_command: canonical_command.clone(),
                completion: None,
                candidate_proof: None,
                final_destination: None,
            });
        }
        self.recheck_originals()?;
        let source = SOURCE.get().ok_or(Error::Unavailable)?.clone();
        source.recheck_originals()?;
        self.resume_publication()?;
        if matches!(self.stage, Stage::Commit { .. }) {
            if self
                .terminal_attempt
                .as_ref()
                .ok_or(Error::Rejected)?
                .completion
                .is_none()
            {
                let Stage::Commit { original, .. } = &self.stage else {
                    return Err(Error::Rejected);
                };
                let attempt = self.terminal_attempt.as_ref().ok_or(Error::Rejected)?;
                let originals = source.outgoing_completion_originals(
                    original,
                    &attempt.canonical_command,
                    &attempt.request[1],
                )?;
                self.destination(&originals.destination)?;
                if originals.prepared_reference_ms == 0 {
                    return Err(Error::Rejected);
                }
                source.recheck_originals()?;
                let proof = original
                    .candidate()
                    .map_err(|_| Error::Rejected)?
                    .recovery_view()
                    .map_err(|_| Error::Rejected)?
                    .candidate_proof
                    .clone();
                let attempt = self.terminal_attempt.as_mut().ok_or(Error::Rejected)?;
                attempt.completion = Some(originals);
                attempt.candidate_proof = Some(proof);
            }
            let attempt = self.terminal_attempt.as_ref().ok_or(Error::Rejected)?;
            let originals = attempt.completion.as_ref().ok_or(Error::Rejected)?.clone();
            let proof = attempt
                .candidate_proof
                .as_ref()
                .ok_or(Error::Rejected)?
                .clone();
            let raw = KagemushaOriginalOutgoingHardwareCommitV1 {
                canonical_command: attempt.canonical_command.clone(),
                original_response: attempt.request[1].clone(),
            };
            let Stage::Commit {
                original,
                request,
                candidate,
                canonical_command,
            } = std::mem::replace(&mut self.stage, Stage::Frozen)
            else {
                return Err(Error::Rejected);
            };
            match original.complete_or_retain(
                originals.certificate,
                TransitionAuthorizationV1::new(originals.state_guard, proof),
                raw,
                originals.prepared_reference_ms,
                &originals.destination.directory,
                originals.destination.checkpoint_operation_id,
            ) {
                Ok(pending) => self.publish(pending)?,
                Err((original, _)) => {
                    self.stage = Stage::Commit {
                        original,
                        request,
                        candidate,
                        canonical_command,
                    };
                    return Err(Error::Unavailable);
                }
            }
        }
        let operation = self
            .terminal_attempt
            .as_ref()
            .ok_or(Error::Rejected)?
            .operation_id;
        if let Ok(envelope) = self.selected()?.original_terminal_envelope(operation) {
            source.recheck_originals()?;
            return Ok(envelope);
        }
        if self
            .terminal_attempt
            .as_ref()
            .ok_or(Error::Rejected)?
            .final_destination
            .is_none()
        {
            let destination = source.publication_destination(self.selected()?, operation, 4)?;
            self.destination(&destination)?;
            source.recheck_originals()?;
            self.terminal_attempt
                .as_mut()
                .ok_or(Error::Rejected)?
                .final_destination = Some(destination);
        }
        let destination = self
            .terminal_attempt
            .as_ref()
            .ok_or(Error::Rejected)?
            .final_destination
            .as_ref()
            .ok_or(Error::Rejected)?
            .clone();
        let material = {
            let selection = self
                .selected()?
                .committed_outgoing_proving_selection(operation)
                .map_err(|_| Error::Rejected)?;
            let (profile, resolver) = source.committed_prover_originals(&selection)?;
            let prover = KagemushaProductionProverV1::load_committed(
                &selection,
                profile,
                Resolver(resolver),
            )
            .map_err(|_| Error::Rejected)?;
            let claim = prover
                .prove_outgoing_terminal_hash_claim(&selection)
                .map_err(|_| Error::Rejected)?;
            let generated = prover
                .prove_outgoing_terminal(&selection, &claim)
                .map_err(|_| Error::Rejected)?;
            match generated {
                KagemushaProductionTerminalProofV1::Payment(proof) => {
                    let committed = selection.committed().map_err(|_| Error::Rejected)?;
                    let PreparedOutgoingRecoveryViewV1::Send {
                        output,
                        encrypted_credit,
                        ..
                    } = committed.candidate.prepared.recovery_view()
                    else {
                        return Err(Error::Rejected);
                    };
                    TerminalMaterial::Payment(KagemushaPaymentV1 {
                        version: 1,
                        output: output.clone(),
                        encrypted_credit: encrypted_credit.to_vec(),
                        commit_certificate: committed.commit_certificate.clone(),
                        proof: proof.proof,
                    })
                }
                KagemushaProductionTerminalProofV1::Redemption(proof) => {
                    TerminalMaterial::Redemption(proof.proof)
                }
            }
        };
        source.recheck_originals()?;
        let owner = self.take_selected()?;
        let pending = match material {
            TerminalMaterial::Payment(payment) => owner.stage_final_payment(
                &destination.directory,
                destination.checkpoint_operation_id,
                payment,
                Vec::new(),
            ),
            TerminalMaterial::Redemption(proof) => owner.stage_final_redemption(
                &destination.directory,
                destination.checkpoint_operation_id,
                proof,
                Vec::new(),
            ),
        }
        .map_err(|_| Error::Rejected)?;
        self.publish(pending)?;
        let envelope = self
            .selected()?
            .original_terminal_envelope(operation)
            .map_err(|_| Error::Rejected)?;
        source.recheck_originals()?;
        self.recheck_originals()?;
        Ok(envelope)
    }
}

enum TerminalMaterial {
    Payment(KagemushaPaymentV1),
    Redemption(iroha_data_model::kagemusha::KagemushaRedemptionProofV1),
}

fn signed_record(signed: &AuthenticatedSenderReplyV1) -> Result<&SenderRecordV1> {
    match &signed.reply().body {
        SenderReplyBodyV1::Lookup(Some(item)) if item.canonical_envelope.is_empty() => {
            Ok(&item.record)
        }
        _ => Err(Error::Rejected),
    }
}
pub(super) fn require_record_match(
    native: &KagemushaOutgoingOperationRecordV1,
    signed: &SenderRecordV1,
) -> Result<()> {
    if native.operation_id != signed.operation_id
        || native.context != signed.context
        || native.inputs_digest != signed.inputs_digest
        || native.operation_kind != signed.operation_kind
        || native.preparation_id != signed.preparation_id
        || native.outbox_reservation_id != signed.outbox_reservation_id
        || native.outcome_id != signed.outcome_id
        || !inputs_match(native.inputs.as_ref(), signed.inputs.as_ref())
    {
        return Err(Error::Rejected);
    }
    Ok(())
}
fn require_child(base: &Path, path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path == base
        || !path.starts_with(base)
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(Error::Rejected);
    }
    Ok(())
}

fn inputs_match(
    native: Option<&iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputsV1>,
    signed: Option<&crate::kagemusha_device_bridge_v1::sender_payload::SenderPublicInputsV1>,
) -> bool {
    use crate::kagemusha_device_bridge_v1::sender_payload::SenderPublicInputsV1 as S;
    use iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputsV1 as N;
    match (native, signed) {
        (Some(N::SendSplit { request: a }), Some(S::SendSplit { request: b })) => a == b,
        (
            Some(N::RedeemSplit {
                amount: a,
                beneficiary: ak,
            }),
            Some(S::RedeemSplit {
                amount: b,
                beneficiary: bk,
            }),
        ) => a == b && ak == bk,
        (None, None) => true,
        _ => false,
    }
}
