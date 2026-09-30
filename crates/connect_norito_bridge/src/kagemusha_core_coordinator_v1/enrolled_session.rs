//! Account/device possession session registry and exact recovery-selection checks.
//!
//! Historical recovery does not renew enrollment/KYC or permit new monetary work.
//! Production ownership requires the concrete authenticated Core and its held native journals;
//! structural fixture owners remain confined to the test module.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use iroha_core_zk::kagemusha_v1_state::{
    KagemushaAuthenticatedCoreOwnerV1, KagemushaRecoveryEnrollmentBindingV1,
};
use iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1;

use super::{
    enrolled_open::{
        EnrolledOpenAuthoritySourceV1, PendingEnrolledOpenV1, VerifiedEnrolledOpenEvidenceV1,
        VerifiedEnrolledOpenV1, authenticated_recovery_source,
    },
    native_deadline::NativeDeadlineV1,
    session_registry::{InvocationResult, RegistryError, SessionRegistry},
    startup_qualification::NativeStartupQualificationOwnerV1,
};

type Result<T> = std::result::Result<T, RegistryError>;

mod sealed {
    pub trait RecoveredOwner {}
}

/// Rust-only recovery owner access. The private supertrait excludes external implementations.
pub(super) trait RecoveredOwnerAccessV1: sealed::RecoveredOwner {
    fn require_current(&self) -> Result<()>;
    fn begin_possession(&self, deadline: NativeDeadlineV1) -> Result<PendingEnrolledOpenV1>;
    fn prepare_possession(
        &self,
        verified: VerifiedEnrolledOpenV1,
    ) -> Result<PreparedRecoveredOpenV1>;
    fn install_possession(&mut self, prepared: PreparedRecoveredOpenV1);
    fn observer_mut(&mut self) -> &mut NativeStartupQualificationOwnerV1;
    fn require_observation_ready(&self) -> Result<()> {
        self.require_current()
    }
}

pub(super) struct AuthenticatedRecoveredOwnerV1 {
    core: super::native_core_work::NativeCoreWorkOwnerV1,
    native_key: KagemushaDevicePublicKeyV1,
    observer: NativeStartupQualificationOwnerV1,
    evidence: Option<VerifiedEnrolledOpenEvidenceV1>,
    lease_source: Option<EnrolledOpenAuthoritySourceV1>,
    signer: super::core_authorization_signer::RetainedCoreAuthorizationSignerV1,
    sender_replies: BTreeMap<(u8, [u8; 32]), super::sender_observation::AuthenticatedSenderReplyV1>,
}

impl AuthenticatedRecoveredOwnerV1 {
    pub(super) fn new(
        path: String,
        core: KagemushaAuthenticatedCoreOwnerV1,
        native_key: KagemushaDevicePublicKeyV1,
        signer: Arc<dyn super::KagemushaNativeCoreAuthorizationSignerV1>,
    ) -> Result<Self> {
        let observer =
            NativeStartupQualificationOwnerV1::from_authenticated_core_owner(&core, &native_key)
                .map_err(|_| RegistryError::Rejected)?;
        let signer = super::core_authorization_signer::RetainedCoreAuthorizationSignerV1::new(
            native_key, signer,
        )
        .map_err(|_| RegistryError::Rejected)?;
        Ok(Self {
            core: super::native_core_work::NativeCoreWorkOwnerV1::new(path, core)
                .map_err(|_| RegistryError::Rejected)?,
            native_key,
            observer,
            evidence: None,
            lease_source: None,
            signer,
            sender_replies: BTreeMap::new(),
        })
    }

    pub(super) fn from_pending_commit(
        path: String,
        commit: iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOutgoingCommitV1,
        native_key: KagemushaDevicePublicKeyV1,
        signer: Arc<dyn super::KagemushaNativeCoreAuthorizationSignerV1>,
    ) -> Result<Self> {
        let core =
            super::native_core_work::NativeCoreWorkOwnerV1::from_pending_commit(path, commit)
                .map_err(|_| RegistryError::Rejected)?;
        let observer =
            NativeStartupQualificationOwnerV1::from_original_work_owner(&core, &native_key)
                .map_err(|_| RegistryError::Rejected)?;
        let signer = super::core_authorization_signer::RetainedCoreAuthorizationSignerV1::new(
            native_key, signer,
        )
        .map_err(|_| RegistryError::Rejected)?;
        Ok(Self {
            core,
            native_key,
            observer,
            evidence: None,
            lease_source: None,
            signer,
            sender_replies: BTreeMap::new(),
        })
    }

    pub(super) fn from_pending_incoming(
        path: String,
        cap: iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedIncomingFoldV1,
        native_key: KagemushaDevicePublicKeyV1,
        signer: Arc<dyn super::KagemushaNativeCoreAuthorizationSignerV1>,
    ) -> Result<Self> {
        let core = super::native_core_work::NativeCoreWorkOwnerV1::from_pending_incoming(path, cap)
            .map_err(|_| RegistryError::Rejected)?;
        let observer =
            NativeStartupQualificationOwnerV1::from_original_work_owner(&core, &native_key)
                .map_err(|_| RegistryError::Rejected)?;
        let signer = super::core_authorization_signer::RetainedCoreAuthorizationSignerV1::new(
            native_key, signer,
        )
        .map_err(|_| RegistryError::Rejected)?;
        Ok(Self {
            core,
            native_key,
            observer,
            evidence: None,
            lease_source: None,
            signer,
            sender_replies: BTreeMap::new(),
        })
    }
    // Original public sender replies are admitted independently of the method5/6 caller's
    // archive. Their signature is observation evidence only; private preparation/proof and
    // consuming Core stages remain mandatory before hardware authorization or funds change.
    pub(super) fn accept_sender_reply(&mut self, fields: &[Vec<u8>]) -> Result<()> {
        self.require_current()?;
        if fields.len() != 10 {
            return Err(RegistryError::Rejected);
        }
        let operation = u8::try_from(u32::from_le_bytes(
            fields[0]
                .as_slice()
                .try_into()
                .map_err(|_| RegistryError::Rejected)?,
        ))
        .map_err(|_| RegistryError::Rejected)?;
        let request_id: [u8; 32] = fields[1]
            .as_slice()
            .try_into()
            .map_err(|_| RegistryError::Rejected)?;
        let qualification = self
            .observer
            .sender_qualification(&fields[5..])
            .map_err(|_| RegistryError::Rejected)?;
        let (context, inputs_digest, provider_root) = self
            .core
            .sender_observation_selection(request_id)
            .map_err(|_| RegistryError::Rejected)?;
        let token = super::sender_observation::AuthenticatedSenderReplyV1::authenticate(
            operation,
            request_id,
            &fields[2],
            &fields[3],
            &fields[4],
            &context,
            &qualification,
            provider_root,
        )
        .map_err(|_| RegistryError::Rejected)?;
        if token.command().context != context
            || token
                .command()
                .expected_inputs_digest()
                .map_err(|_| RegistryError::Rejected)?
                != Some(inputs_digest)
        {
            return Err(RegistryError::Rejected);
        }
        let key = (operation, request_id);
        if let Some(previous) = self.sender_replies.get(&key) {
            if previous != &token {
                return Err(RegistryError::Rejected);
            }
        } else {
            // Bound process-local signed observation memory. No original native journal is
            // discarded or replaced when this transient read budget is exhausted.
            if self.sender_replies.len() >= 16 {
                return Err(RegistryError::Rejected);
            }
            self.sender_replies.insert(key, token);
        }
        self.require_current()
    }

    pub(super) fn original_sender_reply(
        &self,
        operation: u8,
        operation_id: [u8; 32],
        bytes: &[u8],
    ) -> Result<&super::sender_observation::AuthenticatedSenderReplyV1> {
        self.require_current()?;
        let original = self
            .sender_replies
            .get(&(operation, operation_id))
            .ok_or(RegistryError::Rejected)?;
        original
            .require_original_reply(bytes)
            .map_err(|_| RegistryError::Rejected)?;
        Ok(original)
    }

    pub(super) fn reserve_sender_operation(&mut self, fields: &[Vec<u8>]) -> Result<[u8; 32]> {
        self.require_current()?;
        if fields.len() != 3 || fields[0] != 5_u32.to_le_bytes() {
            return Err(RegistryError::Rejected);
        }
        let operation_id = fields[1]
            .as_slice()
            .try_into()
            .map_err(|_| RegistryError::Rejected)?;
        let inputs: iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputsV1 =
            norito::decode_canonical_with_limits(
                &fields[2],
                norito::DecodeLimits::new(16 * 1024, 16 * 1024, 64 * 1024, 128 * 1024, 32),
            )
            .map_err(|_| RegistryError::Rejected)?;
        inputs
            .validate_shape(
                &self
                    .core
                    .selected()
                    .map_err(|_| RegistryError::Rejected)?
                    .sender_context()
                    .map_err(|_| RegistryError::Rejected)?,
            )
            .map_err(|_| RegistryError::Rejected)?;
        let retained = self
            .core
            .selected_mut()
            .map_err(|_| RegistryError::Rejected)?
            .reserve_coordinator_operation(operation_id, 5, &fields[2])
            .map_err(|_| RegistryError::Rejected)?;
        self.require_current()?;
        Ok(retained)
    }

    // This retains only the original public intent. The actual preparation, fresh hardware
    // authority, real paired proof and consuming checkpoint stages occur after op5/6.
    pub(super) fn begin_sender_intent(
        &mut self,
        fields: &[Vec<u8>],
    ) -> Result<super::archives::KagemushaCoreSenderPreparationArchiveV1> {
        self.require_current()?;
        let operation_id = fields
            .first()
            .ok_or(RegistryError::Rejected)?
            .as_slice()
            .try_into()
            .map_err(|_| RegistryError::Rejected)?;
        let kind = fields.get(1).ok_or(RegistryError::Rejected)?;
        let (inputs, qualification_start) =
            if kind.as_slice() == 0_u32.to_le_bytes() {
                (
                    iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputsV1::SendSplit {
                        request: fields.get(2).ok_or(RegistryError::Rejected)?.clone(),
                    },
                    3,
                )
            } else if kind.as_slice() == 1_u32.to_le_bytes() {
                let amount = u128::from_le_bytes(
                    fields
                        .get(2)
                        .ok_or(RegistryError::Rejected)?
                        .as_slice()
                        .try_into()
                        .map_err(|_| RegistryError::Rejected)?,
                );
                let beneficiary = norito::decode_canonical_with_limits(
                    fields.get(3).ok_or(RegistryError::Rejected)?,
                    norito::DecodeLimits::new(16 * 1024, 16 * 1024, 64 * 1024, 128 * 1024, 32),
                )
                .map_err(|_| RegistryError::Rejected)?;
                (iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputsV1::RedeemSplit {
                amount, beneficiary,
            }, 4)
            } else {
                return Err(RegistryError::Rejected);
            };
        if fields.len() != qualification_start + 5 {
            return Err(RegistryError::Rejected);
        }
        let qualification = self
            .observer
            .sender_qualification(&fields[qualification_start..])
            .map_err(|_| RegistryError::Rejected)?;
        let context = self
            .core
            .selected()
            .map_err(|_| RegistryError::Rejected)?
            .sender_context()
            .map_err(|_| RegistryError::Rejected)?;
        if context.credential_id != qualification.credential.credential_id
            || context.core_authorization_key_reference
                != qualification.core_authorization_key_reference
            || context.device_policy_binding.hardware_policy_id
                != self
                    .core
                    .selected()
                    .map_err(|_| RegistryError::Rejected)?
                    .authenticated_release()
                    .map_err(|_| RegistryError::Rejected)?
                    .provider_policy_root()
        {
            return Err(RegistryError::Rejected);
        }
        let intent = iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputPreimageV1 {
            version: 1,
            operation_id,
            context,
            inputs,
        };
        let inputs_digest = intent
            .canonical_digest()
            .map_err(|_| RegistryError::Rejected)?;
        self.core
            .selected_mut()
            .map_err(|_| RegistryError::Rejected)?
            .begin_coordinator_sender_intent(&intent)
            .map_err(|_| RegistryError::Rejected)?;
        self.require_current()?;
        Ok(super::archives::KagemushaCoreSenderPreparationArchiveV1 {
            version: 1,
            operation_id,
            context: intent.context,
            inputs_digest,
        })
    }
    pub(super) fn resume_original_publication(&mut self) -> Result<()> {
        self.core
            .resume_publication()
            .map_err(|_| RegistryError::Rejected)?;
        self.advance_native_lease_source()
    }
    fn advance_native_lease_source(&mut self) -> Result<()> {
        let source = self
            .core
            .authority_source()
            .map_err(|_| RegistryError::Rejected)?;
        if self.evidence.is_some() {
            self.lease_source = Some(source);
        }
        Ok(())
    }
    pub(super) fn prove_sender(
        &mut self,
        fields: &[Vec<u8>],
    ) -> Result<super::archives::KagemushaCoreSenderCandidateArchiveV1> {
        self.require_current()?;
        let preparation =
            super::archives::KagemushaCoreSenderPreparationArchiveV1::decode_canonical_exact(
                fields.first().ok_or(RegistryError::Rejected)?,
            )
            .map_err(|_| RegistryError::Rejected)?;
        let signed = self
            .sender_replies
            .get(&(5, preparation.operation_id))
            .ok_or(RegistryError::Rejected)?
            .clone();
        let result = self
            .core
            .prove_sender(fields, &signed, &self.signer)
            .map_err(|_| RegistryError::Rejected);
        // Only an actual native publication may advance the private lease's current source.
        // Neither caller hashes nor the public preparation archive can invoke this path.
        self.advance_native_lease_source()?;
        result
    }
    pub(super) fn build_terminal(&mut self, fields: &[Vec<u8>]) -> Result<Vec<u8>> {
        self.require_current()?;
        let result = self
            .core
            .build_terminal(fields)
            .map_err(|_| RegistryError::Rejected);
        self.advance_native_lease_source()?;
        result
    }
    pub(super) fn incoming_work(
        &mut self,
        method: super::KagemushaCoreCoordinatorMethodV1,
        fields: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>> {
        self.require_current()?;
        let result = match method {
            super::KagemushaCoreCoordinatorMethodV1::StageIncomingOriginal => {
                self.core.stage_incoming_original(fields)
            }
            super::KagemushaCoreCoordinatorMethodV1::PrepareIncomingFold => {
                self.core.prepare_incoming(fields)
            }
            super::KagemushaCoreCoordinatorMethodV1::CompleteIncomingFold => {
                self.core.complete_incoming(fields)
            }
            _ => return Err(RegistryError::Rejected),
        }
        .map_err(|_| RegistryError::Rejected);
        self.advance_native_lease_source()?;
        result
    }
    pub(super) fn require_pending_original(&self, pending: &BegunRecoveredSessionV1) -> Result<()> {
        self.require_current()?;
        pending
            .deadline
            .check()
            .map_err(|_| RegistryError::Rejected)?;
        let selected = self
            .core
            .current_recovery_selection()
            .map_err(|_| RegistryError::Rejected)?;
        if selected.enrollment_binding() != &pending.enrollment
            || authenticated_recovery_source(&selected).map_err(|_| RegistryError::Rejected)?
                != pending.source
        {
            return Err(RegistryError::Rejected);
        }
        pending
            .deadline
            .check()
            .map_err(|_| RegistryError::Rejected)?;
        self.require_current()
    }
    pub(super) fn export_original_state_proof(
        &self,
        operation: [u8; 32],
    ) -> Result<iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingStateProofArchivePairV1> {
        self.core
            .selected()
            .map_err(|_| RegistryError::Rejected)?
            .export_outgoing_state_proof_archives(operation)
            .map_err(|_| RegistryError::Rejected)
    }
}

impl sealed::RecoveredOwner for AuthenticatedRecoveredOwnerV1 {}

impl RecoveredOwnerAccessV1 for AuthenticatedRecoveredOwnerV1 {
    fn require_current(&self) -> Result<()> {
        self.signer.recheck().map_err(|_| RegistryError::Rejected)?;
        self.core
            .recheck_originals()
            .map_err(|_| RegistryError::Rejected)?;
        if let Some(evidence) = &self.evidence {
            let source = self
                .core
                .authority_source()
                .map_err(|_| RegistryError::Rejected)?;
            let expected = self
                .lease_source
                .as_ref()
                .unwrap_or_else(|| evidence.authority_source());
            if &source != expected {
                return Err(RegistryError::Rejected);
            }
        }
        self.signer.recheck().map_err(|_| RegistryError::Rejected)
    }

    fn begin_possession(&self, deadline: NativeDeadlineV1) -> Result<PendingEnrolledOpenV1> {
        self.require_current()?;
        PendingEnrolledOpenV1::from_original_work_owner(&self.core, &self.native_key, deadline)
            .map_err(|_| RegistryError::Rejected)
    }
    fn prepare_possession(
        &self,
        verified: VerifiedEnrolledOpenV1,
    ) -> Result<PreparedRecoveredOpenV1> {
        self.require_current()?;
        let selected = self
            .core
            .current_recovery_selection()
            .map_err(|_| RegistryError::Rejected)?;
        let source =
            authenticated_recovery_source(&selected).map_err(|_| RegistryError::Rejected)?;
        let release = self
            .core
            .authenticated_release()
            .map_err(|_| RegistryError::Rejected)?;
        let prepared = prepare_recovered_observer(&self.observer, selected.enrollment_binding(), &source,
            release.release_id(), release.hardware_policy_digest(),
            crate::kagemusha_device_bridge_v1::sender_payload::hardware_authorization_key_reference_v1(&self.native_key), verified)?;
        let current = self
            .core
            .current_recovery_selection()
            .map_err(|_| RegistryError::Rejected)?;
        if current.enrollment_binding() != selected.enrollment_binding()
            || authenticated_recovery_source(&current).map_err(|_| RegistryError::Rejected)?
                != source
        {
            return Err(RegistryError::Rejected);
        }
        self.require_current()?;
        Ok(prepared)
    }
    fn install_possession(&mut self, prepared: PreparedRecoveredOpenV1) {
        self.observer = prepared.observer;
        self.lease_source = Some(prepared.evidence.authority_source().clone());
        self.evidence = Some(prepared.evidence);
    }
    fn observer_mut(&mut self) -> &mut NativeStartupQualificationOwnerV1 {
        &mut self.observer
    }
    fn require_observation_ready(&self) -> Result<()> {
        self.core.selected().map_err(|_| RegistryError::Rejected)?;
        self.require_current()
    }
}

/// Immutable prepared replacement; construction requires all exact current-source checks.
pub(super) struct PreparedRecoveredOpenV1 {
    observer: NativeStartupQualificationOwnerV1,
    evidence: VerifiedEnrolledOpenEvidenceV1,
}

// Private exact-selection kernel. A qualified owner must supply an opaque current Core
// selection and independent native catalog/key pins. Tests use structural fixtures.
fn prepare_recovered_observer(
    previous: &NativeStartupQualificationOwnerV1,
    enrollment: &KagemushaRecoveryEnrollmentBindingV1,
    source: &EnrolledOpenAuthoritySourceV1,
    release_id: [u8; 32],
    hardware_policy_digest: [u8; 32],
    core_authorization_key_reference: [u8; 32],
    verified: VerifiedEnrolledOpenV1,
) -> Result<PreparedRecoveredOpenV1> {
    let evidence = verified.evidence();
    evidence
        .require_unexpired()
        .map_err(|_| RegistryError::Rejected)?;
    let qualification = &evidence.observation().qualification;
    if evidence.enrollment_binding() != *enrollment
        || evidence.authority_source() != source
        || !matches!(
            source,
            EnrolledOpenAuthoritySourceV1::RecoveryCheckpoint { .. }
        )
        || evidence.initial_enrollment().is_some()
        || qualification.release_id != release_id
        || qualification.hardware_policy_digest != hardware_policy_digest
        || qualification.core_authorization_key_reference != core_authorization_key_reference
    {
        return Err(RegistryError::Rejected);
    }
    let (candidate, evidence) = verified.into_parts().map_err(|_| RegistryError::Rejected)?;
    let observer = previous
        .prepare_reopen(candidate)
        .map_err(|_| RegistryError::Rejected)?;
    Ok(PreparedRecoveredOpenV1 { observer, evidence })
}

/// Public challenge projections for a native-owned attempt; these bytes carry no session lease.
#[derive(Clone)]
pub(super) struct BegunRecoveredSessionV1 {
    pub(super) attempt_id: u64,
    pub(super) canonical_challenge: Vec<u8>,
    pub(super) account_signing_message: [u8; 32],
    pub(super) device_command: Vec<u8>,
    pub(super) device_request_id: [u8; 32],
    pub(super) deadline: NativeDeadlineV1,
    pub(super) source: EnrolledOpenAuthoritySourceV1,
    pub(super) enrollment: KagemushaRecoveryEnrollmentBindingV1,
}

/// Possession adapter over the revocable session kernel. The sealed owner contract excludes
/// external implementations that could replace authenticated recovery with caller state.
pub(super) struct RecoveredEnrolledSessionRegistryV1<O: RecoveredOwnerAccessV1> {
    registry: SessionRegistry<KagemushaRecoveryEnrollmentBindingV1, O, PendingEnrolledOpenV1>,
}

impl<O: RecoveredOwnerAccessV1> RecoveredEnrolledSessionRegistryV1<O> {
    pub(super) fn new() -> Self {
        Self {
            registry: SessionRegistry::new(),
        }
    }

    /// Capture cancellation and the continuous deadline before acquiring the original owner
    /// lock or reading hardware metadata. A late source read cannot reselect a revoked owner.
    #[cfg(test)]
    pub(super) fn begin(&self, owner: Arc<Mutex<O>>) -> Result<BegunRecoveredSessionV1> {
        self.begin_checked(owner, || Ok(()))
    }

    pub(super) fn begin_checked(
        &self,
        owner: Arc<Mutex<O>>,
        guard: impl Fn() -> Result<()>,
    ) -> Result<BegunRecoveredSessionV1> {
        let deadline = NativeDeadlineV1::start(super::enrolled_open::LIFETIME)
            .map_err(|_| RegistryError::Rejected)?;
        let mut begun = None;
        let attempt_id = self.registry.begin(deadline.clone(), |permit| {
            let pending = {
                let owner = owner.lock().map_err(|_| RegistryError::Poisoned)?;
                // A queued begin may have been revoked or replaced while waiting.
                // The permit check is the metadata-read dispatch point; no global
                // registry lock remains held across hardware/source access.
                permit.require_current()?;
                guard()?;
                let pending = owner.begin_possession(deadline)?;
                guard()?;
                pending
            };
            pending
                .require_unexpired()
                .map_err(|_| RegistryError::Rejected)?;
            let enrollment = pending.enrollment_binding();
            begun = Some(BegunRecoveredSessionV1 {
                attempt_id: 0,
                canonical_challenge: pending.challenge_bytes().to_vec(),
                account_signing_message: pending.account_signing_message(),
                device_command: pending.device_command().to_vec(),
                device_request_id: pending.nonce(),
                deadline: pending.deadline(),
                source: pending.authority_source().clone(),
                enrollment: pending.enrollment_binding(),
            });
            Ok((enrollment, owner, pending))
        })?;
        let mut begun = begun.ok_or(RegistryError::Rejected)?;
        begun.attempt_id = attempt_id;
        Ok(begun)
    }

    /// Consume actual account/device proofs, revalidate the still-current complete native source
    /// under its owner lock, and atomically publish the prepared observer after cancellation checks.
    #[cfg(test)]
    pub(super) fn complete(
        &self,
        attempt_id: u64,
        account_signature: &[u8],
        full_device_response: &[u8],
    ) -> Result<u64> {
        self.complete_checked(attempt_id, account_signature, full_device_response, || {
            Ok(())
        })
    }

    pub(super) fn complete_checked(
        &self,
        attempt_id: u64,
        account_signature: &[u8],
        full_device_response: &[u8],
        guard: impl Fn() -> Result<()>,
    ) -> Result<u64> {
        guard()?;
        let completion = self.registry.take_completion(attempt_id)?;
        self.registry.finish(
            completion,
            |pending| {
                pending
                    .complete(account_signature, full_device_response)
                    .map_err(|_| RegistryError::Rejected)
            },
            |owner, verified| {
                guard()?;
                let prepared = owner.prepare_possession(verified)?;
                guard()?;
                Ok(prepared)
            },
            |owner, prepared| {
                owner.install_possession(prepared);
                Ok(())
            },
        )
    }

    pub(super) fn cancel(&self, attempt_id: u64) -> Result<()> {
        self.registry.cancel(attempt_id)
    }

    pub(super) fn close(&self, handle: u64) -> Result<()> {
        self.registry.close(handle)
    }

    pub(super) fn revoke_selection(&self) -> Result<()> {
        self.registry.revoke_selection()
    }

    /// Invoke only the observation owner under the original serialized session. This recovery
    /// adapter exposes no mutable Core machine or monetary invocation/new-work authority.
    pub(super) fn dispatch_observation<T>(
        &self,
        handle: u64,
        operation: impl FnOnce(&mut NativeStartupQualificationOwnerV1) -> T,
    ) -> Result<InvocationResult<T>> {
        self.dispatch_owner(handle, |owner| {
            owner.require_observation_ready()?;
            Ok::<T, RegistryError>(operation(owner.observer_mut()))
        })
        .and_then(|completed| {
            Ok(InvocationResult {
                value: completed.value?,
                session_is_current: completed.session_is_current,
            })
        })
    }

    // Private native dispatch retains the actual owner and original session. Exported public
    // proof bytes cannot supply a transition, ledger permission or substitute Core owner.
    pub(super) fn dispatch_owner<T>(
        &self,
        handle: u64,
        operation: impl FnOnce(&mut O) -> T,
    ) -> Result<InvocationResult<T>> {
        let completed = self
            .registry
            .dispatch(self.registry.invocation(handle)?, |owner| {
                owner.require_current()?;
                let value = operation(owner);
                owner.require_current()?;
                Ok::<T, RegistryError>(value)
            })?;
        Ok(InvocationResult {
            value: completed.value?,
            session_is_current: completed.session_is_current,
        })
    }
}

#[cfg(test)]
mod tests;
