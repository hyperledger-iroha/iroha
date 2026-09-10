//! Test-only account/device possession session registry and exact recovery-selection checks.
//!
//! Historical recovery does not renew enrollment/KYC or permit new monetary work.
//! TODO: connect a qualified native Core owner that authenticates the complete current
//! checkpoint and independently pinned release before beginning or completing possession.
//! The registry currently has only structural test owners; the exported C boundary remains
//! unavailable until the qualified backend is installed.

use std::sync::{Arc, Mutex};

use iroha_core::zk::kagemusha_v1_state::KagemushaRecoveryEnrollmentBindingV1;

use super::{
    enrolled_open::{
        EnrolledOpenAuthoritySourceV1, PendingEnrolledOpenV1, VerifiedEnrolledOpenEvidenceV1,
        VerifiedEnrolledOpenV1,
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
    fn begin_possession(&self, deadline: NativeDeadlineV1) -> Result<PendingEnrolledOpenV1>;
    fn prepare_possession(
        &self,
        verified: VerifiedEnrolledOpenV1,
    ) -> Result<PreparedRecoveredOpenV1>;
    fn install_possession(&mut self, prepared: PreparedRecoveredOpenV1);
    fn observer_mut(&mut self) -> &mut NativeStartupQualificationOwnerV1;
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
    pub(super) fn begin(&self, owner: Arc<Mutex<O>>) -> Result<BegunRecoveredSessionV1> {
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
                owner.begin_possession(deadline)?
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
            });
            Ok((enrollment, owner, pending))
        })?;
        let mut begun = begun.ok_or(RegistryError::Rejected)?;
        begun.attempt_id = attempt_id;
        Ok(begun)
    }

    /// Consume actual account/device proofs, revalidate the still-current complete native source
    /// under its owner lock, and atomically publish the prepared observer after cancellation checks.
    pub(super) fn complete(
        &self,
        attempt_id: u64,
        account_signature: &[u8],
        full_device_response: &[u8],
    ) -> Result<u64> {
        let completion = self.registry.take_completion(attempt_id)?;
        self.registry.finish(
            completion,
            |pending| {
                pending
                    .complete(account_signature, full_device_response)
                    .map_err(|_| RegistryError::Rejected)
            },
            |owner, verified| owner.prepare_possession(verified),
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
        self.registry
            .dispatch(self.registry.invocation(handle)?, |owner| {
                operation(owner.observer_mut())
            })
    }
}

#[cfg(test)]
mod tests;
