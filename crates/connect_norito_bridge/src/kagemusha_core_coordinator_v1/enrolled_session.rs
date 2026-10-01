//! Account/device possession session registry and exact recovery-selection checks.
//!
//! Historical recovery does not renew enrollment/KYC or permit new monetary work.
//! Production ownership requires the concrete authenticated Core and its held native journals;
//! structural fixture owners remain confined to the test module.

use std::sync::{Arc, Mutex};

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
}

pub(super) struct AuthenticatedRecoveredOwnerV1 {
    core: KagemushaAuthenticatedCoreOwnerV1,
    native_key: KagemushaDevicePublicKeyV1,
    observer: NativeStartupQualificationOwnerV1,
    evidence: Option<VerifiedEnrolledOpenEvidenceV1>,
}

impl AuthenticatedRecoveredOwnerV1 {
    pub(super) fn new(
        core: KagemushaAuthenticatedCoreOwnerV1,
        native_key: KagemushaDevicePublicKeyV1,
    ) -> Result<Self> {
        let observer =
            NativeStartupQualificationOwnerV1::from_authenticated_core_owner(&core, &native_key)
                .map_err(|_| RegistryError::Rejected)?;
        Ok(Self {
            core,
            native_key,
            observer,
            evidence: None,
        })
    }
    pub(super) fn require_pending_original(&self, pending: &BegunRecoveredSessionV1) -> Result<()> {
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
        Ok(())
    }
}

impl sealed::RecoveredOwner for AuthenticatedRecoveredOwnerV1 {}

impl RecoveredOwnerAccessV1 for AuthenticatedRecoveredOwnerV1 {
    fn require_current(&self) -> Result<()> {
        let selected = self
            .core
            .current_recovery_selection()
            .map_err(|_| RegistryError::Rejected)?;
        if let Some(evidence) = &self.evidence {
            if evidence.enrollment_binding() != *selected.enrollment_binding()
                || evidence.authority_source()
                    != &authenticated_recovery_source(&selected)
                        .map_err(|_| RegistryError::Rejected)?
            {
                return Err(RegistryError::Rejected);
            }
        }
        Ok(())
    }
    fn begin_possession(&self, deadline: NativeDeadlineV1) -> Result<PendingEnrolledOpenV1> {
        self.require_current()?;
        PendingEnrolledOpenV1::from_authenticated_core_owner(&self.core, &self.native_key, deadline)
            .map_err(|_| RegistryError::Rejected)
    }
    fn prepare_possession(
        &self,
        verified: VerifiedEnrolledOpenV1,
    ) -> Result<PreparedRecoveredOpenV1> {
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
        Ok(prepared)
    }
    fn install_possession(&mut self, prepared: PreparedRecoveredOpenV1) {
        self.observer = prepared.observer;
        self.evidence = Some(prepared.evidence);
    }
    fn observer_mut(&mut self) -> &mut NativeStartupQualificationOwnerV1 {
        &mut self.observer
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
        let completed = self
            .registry
            .dispatch(self.registry.invocation(handle)?, |owner| {
                owner.require_current()?;
                let value = operation(owner.observer_mut());
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
