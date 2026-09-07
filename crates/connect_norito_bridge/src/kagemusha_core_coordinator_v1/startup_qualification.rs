//! Native-owned, bounded fresh observations before and after aggregate-state recovery.
//!
//! Read challenges are deliberately absent from the payment WAL. Recreating this owner requires
//! fresh OS entropy and a new signed device response. A qualification observation authenticates
//! catalog membership and key possession; it grants no bootstrap, trusted-time, revocation or
//! monetary capability. The native backend must share one mutex-owned instance per wallet across
//! handles and invalidate observations on every installed epoch/catalog transition.
//! TODO: wire this owner into the production backend together with hardware-anchored state and
//! durable accepted-response replay. Stock bridge builds continue to return unavailable.

use std::{collections::BTreeMap, time::Duration};

use super::initial_enrollment::FreshIssuerAdmissionV1;
use super::native_deadline::NativeDeadlineV1;

use iroha_core::zk::kagemusha_v1_state::{
    KagemushaRecoveryEnrollmentBindingV1, KagemushaStateMachineV1, KagemushaStateV1,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_HARDWARE_CREDENTIAL_MAX_BYTES_V1, KAGEMUSHA_HARDWARE_PROFILE_MAX_BYTES_V1,
    KagemushaAuthenticatedReleaseV1, KagemushaDevicePublicKeyV1, KagemushaEnabledProfileV1,
    KagemushaHardwareCredentialV1, KagemushaHardwareProfileV1, KagemushaRetailEnrollmentIssuanceV1,
};
use norito::DecodeLimits;
use rand::{TryRngCore as _, rngs::OsRng};

use crate::kagemusha_device_bridge_v1::{
    ObservationWalletContextV1, QualificationProjectionV1, qualification_projection_v1,
    sender_payload::hardware_authorization_key_reference_v1,
    validate_coordinator_observation_binding_v1, verify_observation_reply_v1,
};

/// Closed native observation failures; none grants an empty-wallet or bootstrap decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ObservationErrorV1 {
    InvalidCommand,
    InvalidQualification,
    MissingChallenge,
    Conflict,
    Authentication,
    Entropy,
    Expired,
}

type Result<T> = std::result::Result<T, ObservationErrorV1>;

/// The backend applies a fresh result once; an exact retry only resumes that same observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ObservationDispositionV1 {
    Fresh,
    AlreadyAccepted,
}

/// Signed observation evidence, never a serializable Core monetary capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NativeReadObservationV1 {
    pub(crate) operation: u8,
    pub(crate) nonce: [u8; 32],
    pub(crate) canonical_command: Vec<u8>,
    pub(crate) canonical_reply: Vec<u8>,
    pub(crate) authenticator: Vec<u8>,
    pub(crate) qualification: QualificationProjectionV1,
}

#[derive(Clone)]
struct PendingObservationV1 {
    nonce: [u8; 32],
    command: Vec<u8>,
    candidate: Option<QualificationProjectionV1>,
    accepted: Option<NativeReadObservationV1>,
    deadline: NativeDeadlineV1,
}

// This projection has no production from-parts constructor. Only the independently authenticated
// release below supplies its membership set; device/host fields cannot choose an authority policy.
struct CatalogBindingsV1 {
    release_id: [u8; 32],
    hardware_policy_digest: [u8; 32],
    provider_policy_root: [u8; 32],
    enabled_profiles: Vec<KagemushaEnabledProfileV1>,
    wallet: ObservationWalletContextV1,
    core_key_reference: [u8; 32],
}

impl CatalogBindingsV1 {
    fn state_floor(&self, state: &KagemushaStateV1) -> Result<CoreEpochFloorV1> {
        let enabled = self
            .enabled_profiles
            .iter()
            .find(|enabled| enabled.hardware_profile_id == state.hardware_profile_id)
            .ok_or(ObservationErrorV1::InvalidQualification)?;
        if state.protocol_version != 1
            || state.release_id != self.release_id
            || state.device_policy_binding.hardware_policy_id != self.hardware_policy_digest
            || state.suite_id != enabled.suite_id
            || state.vk_digest != enabled.vk_digest
            || state.policy_epoch != enabled.policy_epoch
            || state.lane.network_id != self.wallet.network_id
            || state.lane.device_lane_id != self.wallet.lane_id
            || state.lane.asset != self.wallet.asset
            || state.asset_incarnation != self.wallet.asset_incarnation
            || state.lane.scale != self.wallet.scale
        {
            return Err(ObservationErrorV1::InvalidQualification);
        }
        Ok(CoreEpochFloorV1 {
            generation: state.hardware_epoch.generation,
            epoch_id: state.hardware_epoch.epoch_id,
            key_reference: state.device_policy_binding.device_key_reference,
        })
    }

    fn validate(&self, qualification: &QualificationProjectionV1) -> Result<()> {
        let profile = &qualification.profile;
        let credential = &qualification.credential;
        profile
            .validate()
            .map_err(|_| ObservationErrorV1::InvalidQualification)?;
        credential
            .validate_against_profile(profile)
            .map_err(|_| ObservationErrorV1::InvalidQualification)?;
        let enabled = self
            .enabled_profiles
            .iter()
            .find(|enabled| enabled.hardware_profile_id == profile.hardware_profile_id)
            .ok_or(ObservationErrorV1::InvalidQualification)?;
        if qualification.release_id != self.release_id
            || qualification.hardware_policy_digest != self.hardware_policy_digest
            || qualification.core_authorization_key_reference != self.core_key_reference
            || profile != &enabled.hardware_profile
            || credential.suite_id != enabled.suite_id
            || credential.policy_epoch != enabled.policy_epoch
            || profile.qualification_report_digest != enabled.qualification_report.sha256
            || credential.network_id != self.wallet.network_id
            || credential.lane_commitment != self.wallet.lane_id
            || self.provider_policy_root == [0; 32]
        {
            return Err(ObservationErrorV1::InvalidQualification);
        }
        Ok(())
    }
}

/// Epoch/key floor projected only from an existing opaque Core state-machine owner.
/// No public from-parts or decoded-snapshot constructor may create this evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CoreEpochFloorV1 {
    generation: u128,
    epoch_id: [u8; 32],
    key_reference: [u8; 32],
}

impl CoreEpochFloorV1 {
    fn for_credential(credential: &KagemushaHardwareCredentialV1) -> Self {
        Self {
            generation: u128::from(credential.hardware_epoch_generation),
            epoch_id: credential.hardware_epoch_id,
            key_reference: credential.device_key_reference,
        }
    }

    fn admits_successor(self, next: Self) -> bool {
        next.generation > self.generation
            || (next.generation == self.generation
                && next.epoch_id == self.epoch_id
                && next.key_reference == self.key_reference)
    }

    fn admits_credential(self, credential: &KagemushaHardwareCredentialV1) -> bool {
        self.admits_successor(Self::for_credential(credential))
    }
}

/// Process-local observation owner, distinct from an authenticated aggregate state machine.
/// It cannot be deserialized or reconstructed from a host operation journal.
pub(crate) struct NativeStartupQualificationOwnerV1 {
    catalog: CatalogBindingsV1,
    // Complete immutable ownership, including account, FI and both dataspace identities.
    // A lane/network match alone cannot transfer an observation owner to another wallet.
    enrollment: KagemushaRecoveryEnrollmentBindingV1,
    current: Option<QualificationProjectionV1>,
    core_epoch_floor: Option<CoreEpochFloorV1>,
    // The stronger of Core's checkpointed original credential and the credential observed in
    // this process. Its admitting release and epoch may precede the current monetary state;
    // Core has already authenticated that provenance before exposing the opaque machine.
    // Invalidating freshness never discards the same-epoch issuance/signature floor.
    last_credential: Option<KagemushaHardwareCredentialV1>,
    pending: BTreeMap<u8, PendingObservationV1>,
}

impl NativeStartupQualificationOwnerV1 {
    /// Pin the owner and exact credential proved by the one-use native issuer ceremony.
    /// This grants no current device observation, hardware clock or monetary authority.
    pub(super) fn from_fresh_issuer_admission(admission: &FreshIssuerAdmissionV1) -> Result<Self> {
        admission
            .deadline()
            .map_err(|_| ObservationErrorV1::Expired)?;
        let subject = &admission.evidence().certificate().subject;
        let mut owner = Self::new(
            admission.release(),
            admission.enrollment_binding().clone(),
            admission.native_authorization_public_key(),
        )?;
        owner.pin_verified_enrollment_issuance(&subject.issuance)?;
        admission
            .deadline()
            .map_err(|_| ObservationErrorV1::Expired)?;
        Ok(owner)
    }

    // Only the fresh-issuer and opaque-Core constructors may supply this projection.
    // Neither a public wallet context nor decoded enrollment fields can construct an owner.
    fn new(
        release: &KagemushaAuthenticatedReleaseV1,
        enrollment: KagemushaRecoveryEnrollmentBindingV1,
        native_authorization_public_key: &KagemushaDevicePublicKeyV1,
    ) -> Result<Self> {
        native_authorization_public_key
            .validate()
            .map_err(|_| ObservationErrorV1::InvalidQualification)?;
        let wallet = Self::enrolled_wallet_context(&enrollment)?;
        Ok(Self {
            catalog: CatalogBindingsV1 {
                release_id: release.release_id(),
                hardware_policy_digest: release.hardware_policy_digest(),
                provider_policy_root: release.provider_policy_root(),
                enabled_profiles: release.enabled_profiles().to_vec(),
                wallet,
                core_key_reference: hardware_authorization_key_reference_v1(
                    native_authorization_public_key,
                ),
            },
            enrollment,
            current: None,
            core_epoch_floor: None,
            last_credential: None,
            pending: BTreeMap::new(),
        })
    }

    fn enrolled_wallet_context(
        enrollment: &KagemushaRecoveryEnrollmentBindingV1,
    ) -> Result<ObservationWalletContextV1> {
        let owner = &enrollment.owner;
        if owner
            .enrollment_id()
            .map_err(|_| ObservationErrorV1::InvalidQualification)?
            != enrollment.enrollment_id
        {
            return Err(ObservationErrorV1::InvalidQualification);
        }
        Ok(ObservationWalletContextV1 {
            network_id: owner.runtime.network_id,
            lane_id: owner.lane_id,
            asset: owner.runtime.asset.clone(),
            asset_incarnation: owner.runtime.asset_incarnation,
            scale: owner.runtime.scale,
        })
    }

    // Production supplies only the exact issuance borrowed from the opaque fresh admission.
    // Keeping this check private permits targeted substitution tests without forging evidence.
    fn pin_verified_enrollment_issuance(
        &mut self,
        issuance: &KagemushaRetailEnrollmentIssuanceV1,
    ) -> Result<()> {
        let enabled = self
            .catalog
            .enabled_profiles
            .iter()
            .find(|enabled| enabled.hardware_profile_id == issuance.credential.hardware_profile_id)
            .ok_or(ObservationErrorV1::InvalidQualification)?;
        self.catalog.validate(&QualificationProjectionV1 {
            release_id: issuance.release_id,
            hardware_policy_digest: issuance.hardware_policy_digest,
            core_authorization_key_reference: issuance.core_authorization_key_reference,
            profile: enabled.hardware_profile,
            credential: issuance.credential,
        })?;
        self.last_credential = Some(issuance.credential);
        Ok(())
    }

    /// Restore an observer around an actual authenticated Core wallet. The constructor cannot
    /// accept a decoded host snapshot, selected credential or reply as the epoch floor.
    pub(crate) fn from_state_machine<R, G, H>(
        release: &KagemushaAuthenticatedReleaseV1,
        machine: &KagemushaStateMachineV1<R, G, H>,
        native_authorization_public_key: &KagemushaDevicePublicKeyV1,
    ) -> Result<Self> {
        let mut owner = Self::new(
            release,
            machine.enrollment_binding().clone(),
            native_authorization_public_key,
        )?;
        owner.advance_from_state_machine(machine)?;
        Ok(owner)
    }

    /// Apply a hardware-anchored restore or installed state transition before another read.
    /// Only the opaque Core owner supplies this floor; no public state/host archive overload exists.
    pub(crate) fn advance_from_state_machine<R, G, H>(
        &mut self,
        machine: &KagemushaStateMachineV1<R, G, H>,
    ) -> Result<()> {
        let next = self.catalog.state_floor(machine.state())?;
        self.advance_validated_core_floor(
            machine.enrollment_binding(),
            next,
            machine.accepted_credential_floor().credential,
        )
    }

    /// Replace native catalog bindings after Core has installed that exact release. Preserve
    /// both epoch and already accepted credential floors while discarding observation freshness.
    pub(crate) fn repin_from_state_machine<R, G, H>(
        &mut self,
        release: &KagemushaAuthenticatedReleaseV1,
        machine: &KagemushaStateMachineV1<R, G, H>,
        native_authorization_public_key: &KagemushaDevicePublicKeyV1,
    ) -> Result<()> {
        let next = Self::from_state_machine(release, machine, native_authorization_public_key)?;
        self.replace_validated_owner(next)
    }

    // Production reaches this only after from_state_machine has checked the authenticated
    // release and actual Core owner. Keeping the replacement separate permits focused lifecycle
    // tests without adding a public from-parts qualification or state-machine constructor.
    fn replace_validated_owner(&mut self, mut next: Self) -> Result<()> {
        let next_floor = next
            .core_epoch_floor
            .ok_or(ObservationErrorV1::InvalidQualification)?;
        if self.enrollment != next.enrollment
            || self.catalog.wallet != next.catalog.wallet
            || self
                .core_epoch_floor
                .is_some_and(|floor| !floor.admits_successor(next_floor))
        {
            return Err(ObservationErrorV1::InvalidQualification);
        }
        let accepted = next
            .last_credential
            .ok_or(ObservationErrorV1::InvalidQualification)?;
        let strongest = Self::merge_credential_floor(self.last_credential, accepted)?;
        next.invalidate();
        next.last_credential = Some(strongest);
        *self = next;
        Ok(())
    }

    /// Prepare a newly authenticated recovery observation without weakening this process's
    /// credential floor. The caller must separately verify the candidate's still-current full
    /// Core checkpoint. This borrows the original owner and preserves its state on rejection.
    pub(super) fn prepare_reopen(&self, mut next: Self) -> Result<Self> {
        let next_floor = next
            .core_epoch_floor
            .ok_or(ObservationErrorV1::InvalidQualification)?;
        let current = next
            .current
            .as_ref()
            .ok_or(ObservationErrorV1::InvalidQualification)?;
        if self.enrollment != next.enrollment
            || self.catalog.wallet != next.catalog.wallet
            || self
                .core_epoch_floor
                .is_some_and(|floor| !floor.admits_successor(next_floor))
            || next.last_credential != Some(current.credential)
            || Self::merge_credential_floor(self.last_credential, current.credential)?
                != current.credential
        {
            return Err(ObservationErrorV1::InvalidQualification);
        }
        // The candidate's fresh operation-1 nonce/signature remain owned by the replacement.
        // Older pending reads stay with the retired observer and cannot publish on this owner.
        next.last_credential = Some(current.credential);
        Ok(next)
    }

    // Both inputs come from the same opaque, hardware-checkpointed Core machine. The exact
    // original credential can legitimately precede its current state after offline rotation;
    // never reinterpret it under the current catalog or require the current epoch/key.
    fn advance_validated_core_floor(
        &mut self,
        enrollment: &KagemushaRecoveryEnrollmentBindingV1,
        next: CoreEpochFloorV1,
        accepted: KagemushaHardwareCredentialV1,
    ) -> Result<()> {
        if enrollment != &self.enrollment
            || accepted.network_id != self.catalog.wallet.network_id
            || accepted.lane_commitment != self.catalog.wallet.lane_id
            || !CoreEpochFloorV1::for_credential(&accepted).admits_successor(next)
        {
            return Err(ObservationErrorV1::InvalidQualification);
        }
        let strongest = Self::merge_credential_floor(self.last_credential, accepted)?;
        self.advance_core_floor(next)?;
        self.last_credential = Some(strongest);
        Ok(())
    }

    // This comparison supplies no credential authority: both operands were authenticated by
    // Core or by a fresh catalog-bound observation. Equal issuance times cannot order distinct
    // signed credentials, and an epoch's key cannot change through a credential renewal.
    fn merge_credential_floor(
        previous: Option<KagemushaHardwareCredentialV1>,
        next: KagemushaHardwareCredentialV1,
    ) -> Result<KagemushaHardwareCredentialV1> {
        let Some(previous) = previous else {
            return Ok(next);
        };
        if previous.network_id != next.network_id
            || previous.lane_commitment != next.lane_commitment
        {
            return Err(ObservationErrorV1::InvalidQualification);
        }
        match next
            .hardware_epoch_generation
            .cmp(&previous.hardware_epoch_generation)
        {
            std::cmp::Ordering::Less => Ok(previous),
            std::cmp::Ordering::Greater => Ok(next),
            std::cmp::Ordering::Equal => {
                if previous.hardware_epoch_id != next.hardware_epoch_id
                    || previous.device_public_key != next.device_public_key
                    || previous.device_key_reference != next.device_key_reference
                    || (previous.issued_at_ms == next.issued_at_ms && previous != next)
                {
                    return Err(ObservationErrorV1::InvalidQualification);
                }
                Ok(if next.issued_at_ms > previous.issued_at_ms {
                    next
                } else {
                    previous
                })
            }
        }
    }

    fn advance_core_floor(&mut self, next: CoreEpochFloorV1) -> Result<()> {
        if next.generation == 0
            || next.epoch_id == [0; 32]
            || next.key_reference == [0; 32]
            || self
                .core_epoch_floor
                .is_some_and(|floor| !floor.admits_successor(next))
        {
            return Err(ObservationErrorV1::InvalidQualification);
        }
        self.invalidate();
        self.core_epoch_floor = Some(next);
        Ok(())
    }

    /// Begin a fresh bounded read. Repeating this method supersedes the previous attempt for
    /// this operation; a lost begin/read response therefore needs no retained host read intent.
    pub(crate) fn begin(&mut self, operation: u8, command: &[u8]) -> Result<[u8; 32]> {
        if !validate_coordinator_observation_binding_v1(operation, command) {
            return Err(ObservationErrorV1::InvalidCommand);
        }
        if operation != 1 && self.current.is_none() {
            return Err(ObservationErrorV1::InvalidQualification);
        }
        let deadline = NativeDeadlineV1::start(Duration::from_secs(120))
            .map_err(|_| ObservationErrorV1::Expired)?;
        let mut nonce = [0; 32];
        OsRng
            .try_fill_bytes(&mut nonce)
            .map_err(|_| ObservationErrorV1::Entropy)?;
        if nonce == [0; 32] || self.pending.values().any(|entry| entry.nonce == nonce) {
            return Err(ObservationErrorV1::Entropy);
        }
        self.pending.insert(
            operation,
            PendingObservationV1 {
                nonce,
                command: command.to_vec(),
                candidate: None,
                accepted: None,
                deadline,
            },
        );
        Ok(nonce)
    }

    /// Method 2 is only a candidate for the outstanding operation-1 read. Its projection lacks
    /// a request nonce and therefore cannot install a current key or authorize any operation.
    pub(crate) fn stage_qualification(&mut self, fields: &[Vec<u8>]) -> Result<()> {
        if fields.len() != 6 || fields[5].as_slice() != self.catalog.hardware_policy_digest {
            return Err(ObservationErrorV1::InvalidQualification);
        }
        let candidate = self.decode_qualification_fields(&fields[..5])?;
        let pending = self
            .pending
            .get_mut(&1)
            .ok_or(ObservationErrorV1::MissingChallenge)?;
        if pending.accepted.is_none() {
            pending
                .deadline
                .check()
                .map_err(|_| ObservationErrorV1::Expired)?;
        }
        if pending
            .candidate
            .as_ref()
            .is_some_and(|previous| previous != &candidate)
        {
            return Err(ObservationErrorV1::Conflict);
        }
        pending.candidate = Some(candidate);
        Ok(())
    }

    /// Method 3 consumes exactly the native challenge, command, catalog-selected qualification,
    /// and original low-S reply signature. A duplicate result is explicitly historical.
    pub(crate) fn accept(
        &mut self,
        operation: u8,
        nonce: [u8; 32],
        command: &[u8],
        reply: &[u8],
        authenticator: &[u8],
        qualification_fields: &[Vec<u8>],
    ) -> Result<(ObservationDispositionV1, NativeReadObservationV1)> {
        let pending = self
            .pending
            .get(&operation)
            .ok_or(ObservationErrorV1::MissingChallenge)?;
        if pending.nonce != nonce || pending.command != command {
            return Err(ObservationErrorV1::Conflict);
        }
        if pending.accepted.is_none() {
            pending
                .deadline
                .check()
                .map_err(|_| ObservationErrorV1::Expired)?;
        }
        let qualification = self.decode_qualification_fields(qualification_fields)?;
        if operation == 1 {
            if pending.candidate.as_ref() != Some(&qualification)
                || qualification_projection_v1(reply).as_ref() != Some(&qualification)
            {
                return Err(ObservationErrorV1::InvalidQualification);
            }
        } else if self.current.as_ref() != Some(&qualification) {
            return Err(ObservationErrorV1::InvalidQualification);
        }
        if !verify_observation_reply_v1(
            operation,
            nonce,
            command,
            reply,
            authenticator,
            &qualification,
            &self.catalog.wallet,
        ) {
            return Err(ObservationErrorV1::Authentication);
        }
        let result = NativeReadObservationV1 {
            operation,
            nonce,
            canonical_command: command.to_vec(),
            canonical_reply: reply.to_vec(),
            authenticator: authenticator.to_vec(),
            qualification,
        };
        if let Some(previous) = &pending.accepted {
            return if previous == &result {
                Ok((ObservationDispositionV1::AlreadyAccepted, previous.clone()))
            } else {
                Err(ObservationErrorV1::Conflict)
            };
        }
        // First acceptance must still be timely after signature verification. Exact
        // retries above remain historical and can never refresh this clock or nonce.
        pending
            .deadline
            .check()
            .map_err(|_| ObservationErrorV1::Expired)?;
        if operation == 1
            && self
                .core_epoch_floor
                .is_some_and(|floor| !floor.admits_credential(&result.qualification.credential))
        {
            return Err(ObservationErrorV1::InvalidQualification);
        }
        if operation == 1 && self.current.as_ref() != Some(&result.qualification) {
            if Self::merge_credential_floor(self.last_credential, result.qualification.credential)?
                != result.qualification.credential
            {
                return Err(ObservationErrorV1::InvalidQualification);
            }
            // A pending old-epoch snapshot/watermark/time reply can no longer publish as current.
            self.pending.retain(|operation, _| *operation == 1);
            self.last_credential = Some(result.qualification.credential);
            self.current = Some(result.qualification.clone());
        }
        self.pending
            .get_mut(&operation)
            .ok_or(ObservationErrorV1::MissingChallenge)?
            .accepted = Some(result.clone());
        Ok((ObservationDispositionV1::Fresh, result))
    }

    /// Native installed-state/catalog transitions must invalidate all outstanding observations
    /// before publishing their new epoch. Catalog replacement uses repin_from_state_machine
    /// to retain the prior credential and Core epoch floors.
    pub(crate) fn invalidate(&mut self) {
        self.pending.clear();
        self.current = None;
    }

    fn decode_qualification_fields(&self, fields: &[Vec<u8>]) -> Result<QualificationProjectionV1> {
        if fields.len() != 5
            || fields[0] != 1_u32.to_le_bytes()
            || fields[4] != 0xffff_u32.to_le_bytes()
        {
            return Err(ObservationErrorV1::InvalidQualification);
        }
        fn limits(maximum: usize) -> DecodeLimits {
            DecodeLimits::new(maximum, maximum, maximum * 4, maximum * 8, 32)
        }
        let profile: KagemushaHardwareProfileV1 = norito::decode_canonical_with_limits(
            &fields[2],
            limits(KAGEMUSHA_HARDWARE_PROFILE_MAX_BYTES_V1),
        )
        .map_err(|_| ObservationErrorV1::InvalidQualification)?;
        let credential: KagemushaHardwareCredentialV1 = norito::decode_canonical_with_limits(
            &fields[3],
            limits(KAGEMUSHA_HARDWARE_CREDENTIAL_MAX_BYTES_V1),
        )
        .map_err(|_| ObservationErrorV1::InvalidQualification)?;
        let result = QualificationProjectionV1 {
            release_id: fields[1]
                .as_slice()
                .try_into()
                .map_err(|_| ObservationErrorV1::InvalidQualification)?,
            hardware_policy_digest: self.catalog.hardware_policy_digest,
            core_authorization_key_reference: self.catalog.core_key_reference,
            profile,
            credential,
        };
        self.catalog.validate(&result)?;
        Ok(result)
    }
}

#[cfg(test)]
pub(super) mod tests;
