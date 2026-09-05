//! Snapshot-bound caller operation index for recoverable outgoing work.
//!
//! The index binds an independently generated caller operation ID to public
//! inputs derived from Core's actual prepared candidate. It carries no proof or
//! hardware authority. Its only intended storage boundary is the existing
//! hardware-anchored [`super::KagemushaStateSnapshotV1`]; a host-side copy must
//! never be treated as monetary state.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    account::AccountId,
    kagemusha::{
        KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1, KAGEMUSHA_ASSET_SCALE_MAX_V1,
        KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1, KagemushaAcknowledgementV1,
        KagemushaLifecycleBindingV1, KagemushaOperationKindV1, KagemushaPaymentRequestV1,
        KagemushaPaymentV1,
    },
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::{
    DevicePolicyBindingV1, DigestV1, DurableOutgoingEnvelopeV1, HardwareEpochV1,
    KAGEMUSHA_STATE_VERSION_V1, KagemushaLaneIdV1, KagemushaOutgoingEnvelopeV1,
    KagemushaStateContextV1, KagemushaStateV1, PreparedOutgoingCandidateV1,
    PreparedOutgoingRecoveryViewV1,
    candidate_lifecycle::{CommittedOutgoingCandidateV1, PersistedOutgoingCandidateV1},
};

/// Domain of the sole canonical caller-operation public-input digest.
pub const KAGEMUSHA_OUTGOING_PUBLIC_INPUTS_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:device:v1:sender-public-inputs";

/// Maximum number of records selected by one recovery page.
pub const KAGEMUSHA_OUTGOING_OPERATION_PAGE_MAX_V1: u16 = 4;

const RECORD_ALLOCATION_SAFETY_BYTES_V1: u64 = 64;
const ACCEPTED_ACKNOWLEDGEMENT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:device:v1:accepted-acknowledgement";

/// Fail-closed outgoing operation-index errors.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum KagemushaOutgoingOperationIndexErrorV1 {
    /// A forbidden zero identity or malformed public input was supplied.
    #[error("invalid Kagemusha outgoing operation binding")]
    InvalidBinding,
    /// A caller ID, preparation, reservation, or outcome was rebound.
    #[error("conflicting Kagemusha outgoing operation binding")]
    Conflict,
    /// A lifecycle stage skipped, regressed, or changed retained bytes.
    #[error("invalid Kagemusha outgoing operation stage")]
    InvalidStage,
    /// The full-width operation-index revision overflowed.
    #[error("Kagemusha outgoing operation revision overflow")]
    RevisionOverflow,
    /// Canonical encoding or its physical byte accounting failed.
    #[error("invalid Kagemusha outgoing operation encoding")]
    CanonicalEncoding,
    /// A pinned page no longer names the current authenticated snapshot.
    #[error("stale Kagemusha outgoing operation page")]
    StalePage,
    /// A recovered index violates its immutable or physical-storage invariants.
    #[error("invalid recovered Kagemusha outgoing operation index")]
    SnapshotIntegrity,
}

/// Result type for outgoing operation-index validation and immutable successors.
pub type KagemushaOutgoingOperationIndexResultV1<T> =
    core::result::Result<T, KagemushaOutgoingOperationIndexErrorV1>;

/// Identity and release context authenticated by a qualified device session.
///
/// The credential ID is intentionally not derived from receiver material. Core
/// currently has no credential registry, so the qualified native session must
/// authenticate `credential_id` before asking Core to bind a new operation.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-wallet-context")]
pub struct KagemushaOutgoingOperationContextV1 {
    /// Stable network, device lane, asset, and scale.
    pub lane: KagemushaLaneIdV1,
    /// Exact proof release active when the operation was prepared.
    pub release: KagemushaStateContextV1,
    /// Qualified-session credential identity, authenticated outside Core.
    pub credential_id: DigestV1,
    /// Hardware counter epoch which owns the operation predecessor.
    pub hardware_epoch: HardwareEpochV1,
    /// Device key and hardware policy which own the operation predecessor.
    pub device_policy_binding: DevicePolicyBindingV1,
}

impl KagemushaOutgoingOperationContextV1 {
    /// Derive all Core-owned fields from the actual prepared predecessor.
    ///
    /// Supplying `credential_id` does not authenticate it. This constructor is
    /// therefore private to the state module and must be called only after a
    /// qualified native session has authenticated that credential.
    pub(super) fn from_prepared(
        credential_id: DigestV1,
        prepared: &PreparedOutgoingCandidateV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<Self> {
        if credential_id == [0; 32] {
            return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
        }
        let predecessor = prepared.private_state_link().0;
        let context = Self {
            lane: predecessor.lane.clone(),
            release: predecessor.context(),
            credential_id,
            hardware_epoch: predecessor.hardware_epoch,
            device_policy_binding: predecessor.device_policy_binding,
        };
        context.validate_against_prepared(prepared)?;
        Ok(context)
    }

    /// Check the stable identity and non-zero shape of this creation context.
    pub fn validate_shape(&self) -> KagemushaOutgoingOperationIndexResultV1<()> {
        if self.lane.network_id.as_bytes() == &[0; 32]
            || self.lane.device_lane_id == [0; 32]
            || self.lane.scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
            || self.release.protocol_version != KAGEMUSHA_STATE_VERSION_V1
            || self.release.suite_id == [0; 32]
            || self.release.vk_digest == [0; 32]
            || self.release.release_id == [0; 32]
            || self.release.hardware_profile_id == [0; 32]
            || self.release.policy_epoch == 0
            || self.release.asset_incarnation.validate().is_err()
            || self.credential_id == [0; 32]
            || self.hardware_epoch.generation == 0
            || self.hardware_epoch.epoch_id == [0; 32]
            || self.device_policy_binding.device_key_reference == [0; 32]
            || self.device_policy_binding.hardware_policy_id == [0; 32]
            || self.lane.normalized_asset_id().is_err()
        {
            return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
        }
        Ok(())
    }

    /// Require exact equality with a context authenticated by the current native session.
    pub fn validate_against_native(
        &self,
        native: &Self,
    ) -> KagemushaOutgoingOperationIndexResultV1<()> {
        self.validate_shape()?;
        native.validate_shape()?;
        if self != native {
            return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
        }
        Ok(())
    }

    /// Check stable-wallet scope and epoch ordering against a current native-session context.
    ///
    /// This does not authenticate the retained record; it may be called only after guarded index
    /// recovery and current native-session authentication.
    pub fn validate_retained_against_native(
        &self,
        native: &Self,
    ) -> KagemushaOutgoingOperationIndexResultV1<()> {
        self.validate_shape()?;
        native.validate_shape()?;
        if self.lane != native.lane
            || self.release.asset_incarnation != native.release.asset_incarnation
            || self.hardware_epoch.generation > native.hardware_epoch.generation
            || (self.hardware_epoch.generation == native.hardware_epoch.generation
                && self.hardware_epoch.epoch_id != native.hardware_epoch.epoch_id)
        {
            return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
        }
        Ok(())
    }

    /// Check exact creation ownership against an actual Core preparation.
    pub fn validate_against_prepared(
        &self,
        prepared: &PreparedOutgoingCandidateV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<()> {
        self.validate_shape()?;
        let hardware = prepared.hardware_statement();
        let predecessor = prepared.private_state_link().0;
        if prepared.version != KAGEMUSHA_STATE_VERSION_V1
            || self.lane != predecessor.lane
            || self.release != predecessor.context()
            || self.hardware_epoch != predecessor.hardware_epoch
            || self.device_policy_binding != predecessor.device_policy_binding
            || hardware.lane != self.lane
            || hardware.predecessor_epoch != self.hardware_epoch
            || hardware.successor_epoch != self.hardware_epoch
            || hardware.predecessor_device_policy_binding != self.device_policy_binding
            || hardware.successor_device_policy_binding != self.device_policy_binding
        {
            return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
        }
        Ok(())
    }

    /// Check whether a retained record belongs to the same stable current wallet.
    ///
    /// This structural check does not authenticate historical authority. The
    /// index must first be recovered through the guarded snapshot path.
    pub fn validate_retained_against_state(
        &self,
        current: &KagemushaStateV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<()> {
        self.validate_shape()?;
        if self.lane != current.lane
            || self.release.asset_incarnation != current.asset_incarnation
            || self.hardware_epoch.generation > current.hardware_epoch.generation
            || (self.hardware_epoch.generation == current.hardware_epoch.generation
                && self.hardware_epoch.epoch_id != current.hardware_epoch.epoch_id)
        {
            return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
        }
        Ok(())
    }

    /// Validate one public lifecycle against this exact creation context.
    ///
    /// This checks public equality only and does not authenticate the context or grant proof
    /// authority.
    pub fn validate_lifecycle(
        &self,
        lifecycle: &KagemushaLifecycleBindingV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<()> {
        lifecycle
            .validate()
            .map_err(|_| KagemushaOutgoingOperationIndexErrorV1::InvalidBinding)?;
        let pool = iroha_data_model::kagemusha::kagemusha_liability_pool_id_v1(
            &self.lane.network_id,
            &self.lane.asset,
            self.release.asset_incarnation,
        )
        .map_err(|_| KagemushaOutgoingOperationIndexErrorV1::InvalidBinding)?;
        if lifecycle.version != KAGEMUSHA_STATE_VERSION_V1
            || lifecycle.network_id != self.lane.network_id
            || lifecycle.protocol_version != self.release.protocol_version
            || lifecycle.suite_id != self.release.suite_id
            || lifecycle.vk_digest != self.release.vk_digest
            || lifecycle.release_id != self.release.release_id
            || lifecycle.asset != self.lane.asset
            || lifecycle.asset_incarnation != self.release.asset_incarnation
            || lifecycle.scale != self.lane.scale
            || lifecycle.liability_pool_id != pool
            || lifecycle.hardware_profile_id != self.release.hardware_profile_id
            || lifecycle.policy_epoch != self.release.policy_epoch
        {
            return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
        }
        Ok(())
    }
}

/// Public inputs fixed before Core accepts an outgoing preparation.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-public-inputs")]
pub enum KagemushaOutgoingPublicInputsV1 {
    /// Exact canonical receiver request bytes.
    SendSplit {
        /// Canonical [`KagemushaPaymentRequestV1`] bytes.
        request: Vec<u8>,
    },
    /// Public redemption amount and canonical beneficiary.
    RedeemSplit {
        /// Positive amount in atomic units.
        amount: u128,
        /// Chain beneficiary of the redemption voucher.
        beneficiary: AccountId,
    },
}

impl KagemushaOutgoingPublicInputsV1 {
    /// Return the monetary operation represented by these public inputs.
    #[must_use]
    pub const fn operation_kind(&self) -> KagemushaOperationKindV1 {
        match self {
            Self::SendSplit { .. } => KagemushaOperationKindV1::SendSplit,
            Self::RedeemSplit { .. } => KagemushaOperationKindV1::RedeemSplit,
        }
    }

    /// Decode and validate the exact canonical send request.
    pub fn decode_send_parts(
        &self,
    ) -> KagemushaOutgoingOperationIndexResultV1<KagemushaPaymentRequestV1> {
        let Self::SendSplit { request } = self else {
            return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
        };
        let request: KagemushaPaymentRequestV1 =
            decode_exact(request, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1)?;
        request
            .validate_shape()
            .map_err(|_| KagemushaOutgoingOperationIndexErrorV1::InvalidBinding)?;
        Ok(request)
    }

    fn from_prepared(
        context: &KagemushaOutgoingOperationContextV1,
        prepared: &PreparedOutgoingCandidateV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<(Self, DigestV1)> {
        context.validate_against_prepared(prepared)?;
        match prepared.recovery_view() {
            PreparedOutgoingRecoveryViewV1::Send {
                request,
                lifecycle,
                output,
                encrypted_credit,
            } => {
                context.validate_lifecycle(lifecycle)?;
                if lifecycle.operation_kind != KagemushaOperationKindV1::SendSplit
                    || lifecycle.request_id != request.request_id
                    || lifecycle.receiver_lane_commitment
                        != request.hardware_credential.lane_commitment
                    || lifecycle.credit_id != output.credit_id
                    || lifecycle.ciphertext_digest
                        != iroha_data_model::kagemusha::kagemusha_ciphertext_digest_v1(
                            encrypted_credit,
                        )
                {
                    return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
                }
                let value = Self::SendSplit {
                    request: canonical_bytes(request)?,
                };
                value.validate_shape(context)?;
                Ok((value, output.credit_id))
            }
            PreparedOutgoingRecoveryViewV1::Redemption { statement, .. } => {
                context.validate_lifecycle(&statement.lifecycle)?;
                if statement.lifecycle.operation_kind != KagemushaOperationKindV1::RedeemSplit
                    || statement.amount == 0
                    || statement.redemption_id == [0; 32]
                {
                    return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
                }
                let value = Self::RedeemSplit {
                    amount: statement.amount,
                    beneficiary: statement.beneficiary.clone(),
                };
                value.validate_shape(context)?;
                Ok((value, statement.redemption_id))
            }
        }
    }

    /// Validate the exact public input shape against its creation context.
    pub fn validate_shape(
        &self,
        context: &KagemushaOutgoingOperationContextV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<()> {
        context.validate_shape()?;
        match self {
            Self::SendSplit { .. } => {
                let request = self.decode_send_parts()?;
                let pool = iroha_data_model::kagemusha::kagemusha_liability_pool_id_v1(
                    &context.lane.network_id,
                    &context.lane.asset,
                    context.release.asset_incarnation,
                )
                .map_err(|_| KagemushaOutgoingOperationIndexErrorV1::InvalidBinding)?;
                if request.network_id != context.lane.network_id
                    || request.release_id != context.release.release_id
                    || request.asset != context.lane.asset
                    || request.asset_incarnation != context.release.asset_incarnation
                    || request.scale != context.lane.scale
                    || request.liability_pool_id != pool
                {
                    return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
                }
                Ok(())
            }
            Self::RedeemSplit { amount, .. } if *amount != 0 => Ok(()),
            Self::RedeemSplit { .. } => Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding),
        }
    }
}

/// Canonical preimage shared by Core and the ABI-23 sender bridge.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-public-input-preimage")]
pub struct KagemushaOutgoingPublicInputPreimageV1 {
    /// Sole schema version.
    pub version: u16,
    /// Independently generated, non-zero caller operation ID.
    pub operation_id: DigestV1,
    /// Immutable authenticated creation context.
    pub context: KagemushaOutgoingOperationContextV1,
    /// Public operation material derived from the actual Core preparation.
    pub inputs: KagemushaOutgoingPublicInputsV1,
}

impl KagemushaOutgoingPublicInputPreimageV1 {
    /// Compute `SHA256(domain || 00 || u64LE(len) || canonical Norito bytes)`.
    pub fn canonical_digest(&self) -> KagemushaOutgoingOperationIndexResultV1<DigestV1> {
        if self.version != KAGEMUSHA_STATE_VERSION_V1 || self.operation_id == [0; 32] {
            return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
        }
        self.inputs.validate_shape(&self.context)?;
        Ok(digest_bytes(
            KAGEMUSHA_OUTGOING_PUBLIC_INPUTS_DOMAIN_V1,
            &canonical_bytes(self)?,
        ))
    }
}

/// Monotonic durable stage of one caller operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub enum KagemushaOutgoingOperationPhaseV1 {
    /// Core accepted the exact preparation and reserved terminal capacity.
    Prepared,
    /// Core verified and persisted the exact private candidate proof.
    CandidatePersisted,
    /// Qualified hardware consumed the predecessor and returned its certificate.
    Committed,
    /// Core authenticated and durably installed the final retry envelope.
    Installed,
    /// A verified terminal delivery acknowledgement retired the retry envelope.
    Released,
}

/// Snapshot-owned immutable operation binding and monotonic recovery projection.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaOutgoingOperationRecordV1 {
    /// Independently generated caller operation ID.
    pub operation_id: DigestV1,
    /// Immutable creation context retained across release and epoch rotation.
    pub context: KagemushaOutgoingOperationContextV1,
    /// Digest of the exact caller ID, creation context, and public inputs.
    pub inputs_digest: DigestV1,
    /// Exact public monetary operation kind.
    pub operation_kind: KagemushaOperationKindV1,
    /// Core-derived identity of the actual sealed preparation.
    pub preparation_id: DigestV1,
    /// Core-derived one-use durable outbox reservation identity.
    pub outbox_reservation_id: DigestV1,
    /// Core-derived credit ID or redemption ID.
    pub outcome_id: DigestV1,
    /// Current durable stage.
    pub phase: KagemushaOutgoingOperationPhaseV1,
    /// Index revision which last changed this record.
    pub record_revision: u128,
    /// Exact public inputs needed to recover non-released work.
    pub inputs: Option<KagemushaOutgoingPublicInputsV1>,
    /// Verified private-candidate public-input digest, when persisted.
    pub candidate_digest: Option<DigestV1>,
    /// Qualified hardware commit-certificate digest, when committed.
    pub commit_certificate_digest: Option<DigestV1>,
    /// Exact terminal retry-envelope digest, when installed.
    pub envelope_digest: Option<DigestV1>,
    /// Verified terminal acknowledgement digest, when released.
    pub acknowledgement_digest: Option<DigestV1>,
    /// Physical bytes reserved before accepting this operation.
    pub(super) reserved_record_bytes: u64,
}

impl KagemushaOutgoingOperationRecordV1 {
    fn validate(&self) -> KagemushaOutgoingOperationIndexResultV1<()> {
        self.context.validate_shape()?;
        if [
            self.operation_id,
            self.inputs_digest,
            self.preparation_id,
            self.outbox_reservation_id,
            self.outcome_id,
        ]
        .contains(&[0; 32])
            || self.record_revision == 0
            || self.reserved_record_bytes == 0
            || self.inputs.as_ref().is_some_and(|inputs| {
                inputs.operation_kind() != self.operation_kind
                    || inputs.validate_shape(&self.context).is_err()
            })
        {
            return Err(KagemushaOutgoingOperationIndexErrorV1::SnapshotIntegrity);
        }
        if let Some(inputs) = &self.inputs {
            let expected = KagemushaOutgoingPublicInputPreimageV1 {
                version: KAGEMUSHA_STATE_VERSION_V1,
                operation_id: self.operation_id,
                context: self.context.clone(),
                inputs: inputs.clone(),
            }
            .canonical_digest()
            .map_err(|_| KagemushaOutgoingOperationIndexErrorV1::SnapshotIntegrity)?;
            if expected != self.inputs_digest {
                return Err(KagemushaOutgoingOperationIndexErrorV1::SnapshotIntegrity);
            }
        }
        let valid_phase = match self.phase {
            KagemushaOutgoingOperationPhaseV1::Prepared => {
                self.inputs.is_some()
                    && self.candidate_digest.is_none()
                    && self.commit_certificate_digest.is_none()
                    && self.envelope_digest.is_none()
                    && self.acknowledgement_digest.is_none()
            }
            KagemushaOutgoingOperationPhaseV1::CandidatePersisted => {
                self.inputs.is_some()
                    && nonzero_option(self.candidate_digest)
                    && self.commit_certificate_digest.is_none()
                    && self.envelope_digest.is_none()
                    && self.acknowledgement_digest.is_none()
            }
            KagemushaOutgoingOperationPhaseV1::Committed => {
                self.inputs.is_some()
                    && nonzero_option(self.candidate_digest)
                    && nonzero_option(self.commit_certificate_digest)
                    && self.envelope_digest.is_none()
                    && self.acknowledgement_digest.is_none()
            }
            KagemushaOutgoingOperationPhaseV1::Installed => {
                self.inputs.is_some()
                    && nonzero_option(self.candidate_digest)
                    && nonzero_option(self.commit_certificate_digest)
                    && nonzero_option(self.envelope_digest)
                    && self.acknowledgement_digest.is_none()
            }
            KagemushaOutgoingOperationPhaseV1::Released => {
                self.inputs.is_none()
                    && nonzero_option(self.candidate_digest)
                    && nonzero_option(self.commit_certificate_digest)
                    && nonzero_option(self.envelope_digest)
                    && nonzero_option(self.acknowledgement_digest)
            }
        };
        if !valid_phase
            || canonical_len(self)? > self.reserved_record_bytes
            || (self.phase != KagemushaOutgoingOperationPhaseV1::Released
                && terminal_record_allocation(self)? != self.reserved_record_bytes)
        {
            return Err(KagemushaOutgoingOperationIndexErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    pub(super) fn validate_against_prepared(
        &self,
        prepared: &PreparedOutgoingCandidateV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<()> {
        self.validate()?;
        let context = KagemushaOutgoingOperationContextV1::from_prepared(
            self.context.credential_id,
            prepared,
        )?;
        let (inputs, outcome_id) =
            KagemushaOutgoingPublicInputsV1::from_prepared(&context, prepared)?;
        let digest = KagemushaOutgoingPublicInputPreimageV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            operation_id: self.operation_id,
            context: context.clone(),
            inputs: inputs.clone(),
        }
        .canonical_digest()?;
        if self.context != context
            || self.inputs_digest != digest
            || self.operation_kind != inputs.operation_kind()
            || self.preparation_id != prepared.preparation_id
            || self.outbox_reservation_id != prepared.outbox_reservation.reservation_id
            || self.outcome_id != outcome_id
            || self.inputs.as_ref().is_some_and(|value| value != &inputs)
        {
            return Err(KagemushaOutgoingOperationIndexErrorV1::Conflict);
        }
        Ok(())
    }

    /// Verify an exact peer-payment acknowledgement against the installed Core envelope.
    ///
    /// Redemption cannot use this path: it requires a distinct authenticated settlement receipt.
    /// The returned digest is only a replay anchor for [`KagemushaOutgoingOperationIndexV1`]; the
    /// caller must still release the exact Core journal envelope in the same atomic successor.
    pub(super) fn verified_payment_acknowledgement_digest(
        &self,
        durable: &DurableOutgoingEnvelopeV1,
        acknowledgement_bytes: &[u8],
    ) -> KagemushaOutgoingOperationIndexResultV1<DigestV1> {
        self.validate()?;
        if self.phase != KagemushaOutgoingOperationPhaseV1::Installed
            || self.preparation_id != durable.committed.candidate.prepared.preparation_id
            || self.outbox_reservation_id
                != durable
                    .committed
                    .candidate
                    .prepared
                    .outbox_reservation
                    .reservation_id
            || self.candidate_digest != Some(durable.committed.candidate.candidate_envelope_digest)
            || self.commit_certificate_digest != Some(durable.committed.commit_certificate_digest)
            || self.envelope_digest != Some(durable.envelope_digest)
        {
            return Err(KagemushaOutgoingOperationIndexErrorV1::Conflict);
        }
        let inputs = self
            .inputs
            .as_ref()
            .ok_or(KagemushaOutgoingOperationIndexErrorV1::InvalidStage)?;
        let request = inputs.decode_send_parts()?;
        let KagemushaOutgoingEnvelopeV1::Payment(payment) = &durable.envelope else {
            return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidStage);
        };
        if canonical_bytes(payment)? != durable.canonical_envelope_bytes
            || payment.output.credit_id != self.outcome_id
        {
            return Err(KagemushaOutgoingOperationIndexErrorV1::Conflict);
        }
        decode_acknowledgement_exact(acknowledgement_bytes, &request, payment)?;
        Ok(digest_bytes(
            ACCEPTED_ACKNOWLEDGEMENT_DOMAIN_V1,
            acknowledgement_bytes,
        ))
    }

    pub(super) fn validate_released_acknowledgement_retry(
        &self,
        acknowledgement_bytes: &[u8],
    ) -> KagemushaOutgoingOperationIndexResultV1<()> {
        self.validate()?;
        let digest = digest_bytes(ACCEPTED_ACKNOWLEDGEMENT_DOMAIN_V1, acknowledgement_bytes);
        if self.phase != KagemushaOutgoingOperationPhaseV1::Released
            || self.acknowledgement_digest != Some(digest)
        {
            return Err(KagemushaOutgoingOperationIndexErrorV1::Conflict);
        }
        Ok(())
    }
}

/// Exact result of preparing one caller operation binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaOutgoingOperationPrepareOutcomeV1 {
    /// A new immutable binding was staged in the candidate successor.
    Inserted,
    /// The exact operation binding already exists at this retained stage.
    AlreadyBound(KagemushaOutgoingOperationPhaseV1),
}

/// Authenticated operation page selected at one full-width index revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaOutgoingOperationPageV1 {
    /// Exact index revision pinned by this selection.
    pub index_revision: u128,
    /// Ordered bounded records after the supplied cursor.
    pub records: Vec<KagemushaOutgoingOperationRecordV1>,
    /// Last returned ID when additional entries exist at the pinned revision.
    pub next_cursor: Option<DigestV1>,
}

/// Snapshot-bound caller-operation index.
///
/// Records and their reserved terminal growth remain charged after release. A
/// finite device may reject a new operation for physical capacity, but it must
/// never evict an old binding and thereby turn a used caller ID into Missing.
/// TODO: qualify authenticated external paging for devices whose durable local
/// storage cannot retain their complete operation history.
#[derive(Clone, Debug, Default, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaOutgoingOperationIndexV1 {
    pub(super) revision: u128,
    pub(super) reserved_bytes: u64,
    pub(super) records: BTreeMap<DigestV1, KagemushaOutgoingOperationRecordV1>,
}

impl KagemushaOutgoingOperationIndexV1 {
    /// Return the full-width stable-wallet index revision.
    #[must_use]
    pub const fn revision(&self) -> u128 {
        self.revision
    }

    /// Return physical bytes permanently charged by accepted operation slots.
    #[must_use]
    pub const fn reserved_bytes(&self) -> u64 {
        self.reserved_bytes
    }

    /// Return the number of retained live records and tombstones.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Return whether no caller operation has ever been accepted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Classify a caller's complete prepare request before producing any new Core candidate.
    ///
    /// An exact existing binding returns its current record at every phase, including a
    /// released tombstone. Any changed creation context or public input under the same caller
    /// ID fails as a permanent conflict. Absence only means that this guarded snapshot contains
    /// no such binding; the caller must still pass the normal proof and hardware-authorized Core
    /// preparation path.
    pub fn classify_existing_prepare(
        &self,
        request: &KagemushaOutgoingPublicInputPreimageV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<Option<&KagemushaOutgoingOperationRecordV1>> {
        let requested_digest = request.canonical_digest()?;
        let Some(existing) = self.records.get(&request.operation_id) else {
            return Ok(None);
        };
        existing.validate()?;
        if existing.context != request.context
            || existing.inputs_digest != requested_digest
            || existing.operation_kind != request.inputs.operation_kind()
            || existing
                .inputs
                .as_ref()
                .is_some_and(|inputs| inputs != &request.inputs)
        {
            return Err(KagemushaOutgoingOperationIndexErrorV1::Conflict);
        }
        Ok(Some(existing))
    }

    /// Bind one caller ID to public inputs derived from the actual preparation.
    ///
    /// This builds a clone-before-install successor. The journal/capacity owner
    /// must charge [`Self::reserved_bytes`] through the existing sender outbox
    /// meter and install all three successors atomically.
    pub(super) fn prepare_successor(
        &self,
        operation_id: DigestV1,
        authenticated_credential_id: DigestV1,
        prepared: &PreparedOutgoingCandidateV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<(Self, KagemushaOutgoingOperationPrepareOutcomeV1)>
    {
        if operation_id == [0; 32] {
            return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
        }
        let context = KagemushaOutgoingOperationContextV1::from_prepared(
            authenticated_credential_id,
            prepared,
        )?;
        let (inputs, outcome_id) =
            KagemushaOutgoingPublicInputsV1::from_prepared(&context, prepared)?;
        let inputs_digest = KagemushaOutgoingPublicInputPreimageV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            operation_id,
            context: context.clone(),
            inputs: inputs.clone(),
        }
        .canonical_digest()?;
        let immutable = (
            &context,
            inputs_digest,
            inputs.operation_kind(),
            prepared.preparation_id,
            prepared.outbox_reservation.reservation_id,
            outcome_id,
        );
        if let Some(existing) = self.records.get(&operation_id) {
            existing.validate()?;
            let matches = (
                &existing.context,
                existing.inputs_digest,
                existing.operation_kind,
                existing.preparation_id,
                existing.outbox_reservation_id,
                existing.outcome_id,
            ) == immutable
                && existing
                    .inputs
                    .as_ref()
                    .is_none_or(|retained| retained == &inputs);
            return if matches {
                Ok((
                    self.clone(),
                    KagemushaOutgoingOperationPrepareOutcomeV1::AlreadyBound(existing.phase),
                ))
            } else {
                Err(KagemushaOutgoingOperationIndexErrorV1::Conflict)
            };
        }
        if self.records.values().any(|existing| {
            existing.preparation_id == prepared.preparation_id
                || existing.outbox_reservation_id == prepared.outbox_reservation.reservation_id
                || existing.outcome_id == outcome_id
        }) {
            return Err(KagemushaOutgoingOperationIndexErrorV1::Conflict);
        }
        let revision = next_revision(self.revision)?;
        let mut record = KagemushaOutgoingOperationRecordV1 {
            operation_id,
            context,
            inputs_digest,
            operation_kind: inputs.operation_kind(),
            preparation_id: prepared.preparation_id,
            outbox_reservation_id: prepared.outbox_reservation.reservation_id,
            outcome_id,
            phase: KagemushaOutgoingOperationPhaseV1::Prepared,
            record_revision: revision,
            inputs: Some(inputs),
            candidate_digest: None,
            commit_certificate_digest: None,
            envelope_digest: None,
            acknowledgement_digest: None,
            reserved_record_bytes: 0,
        };
        record.reserved_record_bytes = terminal_record_allocation(&record)?;
        record.validate()?;
        let mut next = self.clone();
        next.revision = revision;
        next.reserved_bytes = next
            .reserved_bytes
            .checked_add(record.reserved_record_bytes)
            .ok_or(KagemushaOutgoingOperationIndexErrorV1::CanonicalEncoding)?;
        next.records.insert(operation_id, record);
        next.validate_internal(None)?;
        Ok((next, KagemushaOutgoingOperationPrepareOutcomeV1::Inserted))
    }

    /// Advance the indexed preparation after Core persists its verified candidate.
    pub(super) fn candidate_successor(
        &self,
        candidate: &PersistedOutgoingCandidateV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<Self> {
        if let Some(record) = self
            .records
            .values()
            .find(|record| record.preparation_id == candidate.prepared.preparation_id)
        {
            record.validate_against_prepared(&candidate.prepared)?;
        }
        self.progress_successor(
            candidate.prepared.preparation_id,
            KagemushaOutgoingOperationPhaseV1::Prepared,
            |record, revision| {
                if candidate.candidate_envelope_digest == [0; 32] {
                    return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
                }
                record.phase = KagemushaOutgoingOperationPhaseV1::CandidatePersisted;
                record.candidate_digest = Some(candidate.candidate_envelope_digest);
                record.record_revision = revision;
                Ok(())
            },
        )
    }

    /// Advance the indexed candidate after qualified hardware commits it.
    pub(super) fn commit_successor(
        &self,
        committed: &CommittedOutgoingCandidateV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<Self> {
        if let Some(record) = self
            .records
            .values()
            .find(|record| record.preparation_id == committed.candidate.prepared.preparation_id)
        {
            record.validate_against_prepared(&committed.candidate.prepared)?;
        }
        self.progress_successor(
            committed.candidate.prepared.preparation_id,
            KagemushaOutgoingOperationPhaseV1::CandidatePersisted,
            |record, revision| {
                if record.candidate_digest != Some(committed.candidate.candidate_envelope_digest)
                    || committed.commit_certificate_digest == [0; 32]
                {
                    return Err(KagemushaOutgoingOperationIndexErrorV1::Conflict);
                }
                record.phase = KagemushaOutgoingOperationPhaseV1::Committed;
                record.commit_certificate_digest = Some(committed.commit_certificate_digest);
                record.record_revision = revision;
                Ok(())
            },
        )
    }

    /// Advance the indexed commit after Core durably installs its final envelope.
    pub(super) fn install_successor(
        &self,
        envelope: &DurableOutgoingEnvelopeV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<Self> {
        if let Some(record) = self.records.values().find(|record| {
            record.preparation_id == envelope.committed.candidate.prepared.preparation_id
        }) {
            record.validate_against_prepared(&envelope.committed.candidate.prepared)?;
        }
        self.progress_successor(
            envelope.committed.candidate.prepared.preparation_id,
            KagemushaOutgoingOperationPhaseV1::Committed,
            |record, revision| {
                if record.candidate_digest
                    != Some(envelope.committed.candidate.candidate_envelope_digest)
                    || record.commit_certificate_digest
                        != Some(envelope.committed.commit_certificate_digest)
                    || envelope.envelope_digest == [0; 32]
                {
                    return Err(KagemushaOutgoingOperationIndexErrorV1::Conflict);
                }
                record.phase = KagemushaOutgoingOperationPhaseV1::Installed;
                record.envelope_digest = Some(envelope.envelope_digest);
                record.record_revision = revision;
                Ok(())
            },
        )
    }

    /// Retain a terminal tombstone after a separately verified delivery ACK.
    ///
    /// This method does not validate or authorize an acknowledgement. The state
    /// machine may call it only after verifying the operation-specific ACK and
    /// atomically releasing the exact journal envelope.
    pub(super) fn release_successor(
        &self,
        reservation_id: DigestV1,
        envelope_digest: DigestV1,
        verified_acknowledgement_digest: DigestV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<Self> {
        if envelope_digest == [0; 32] || verified_acknowledgement_digest == [0; 32] {
            return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
        }
        let operation_id = self
            .records
            .values()
            .find(|record| record.outbox_reservation_id == reservation_id)
            .map(|record| record.operation_id)
            .ok_or(KagemushaOutgoingOperationIndexErrorV1::InvalidStage)?;
        let existing = self
            .records
            .get(&operation_id)
            .ok_or(KagemushaOutgoingOperationIndexErrorV1::InvalidStage)?;
        if existing.phase == KagemushaOutgoingOperationPhaseV1::Released {
            return if existing.envelope_digest == Some(envelope_digest)
                && existing.acknowledgement_digest == Some(verified_acknowledgement_digest)
            {
                Ok(self.clone())
            } else {
                Err(KagemushaOutgoingOperationIndexErrorV1::Conflict)
            };
        }
        if existing.phase != KagemushaOutgoingOperationPhaseV1::Installed
            || existing.envelope_digest != Some(envelope_digest)
        {
            return Err(KagemushaOutgoingOperationIndexErrorV1::Conflict);
        }
        let revision = next_revision(self.revision)?;
        let mut next = self.clone();
        let record = next
            .records
            .get_mut(&operation_id)
            .ok_or(KagemushaOutgoingOperationIndexErrorV1::InvalidStage)?;
        record.phase = KagemushaOutgoingOperationPhaseV1::Released;
        record.record_revision = revision;
        record.inputs = None;
        record.acknowledgement_digest = Some(verified_acknowledgement_digest);
        record.validate()?;
        next.revision = revision;
        next.validate_internal(None)?;
        Ok(next)
    }

    /// Look up one retained operation, including a released tombstone.
    #[must_use]
    pub fn lookup(&self, operation_id: DigestV1) -> Option<&KagemushaOutgoingOperationRecordV1> {
        self.records.get(&operation_id)
    }

    pub(super) fn record_by_reservation(
        &self,
        reservation_id: DigestV1,
    ) -> Option<&KagemushaOutgoingOperationRecordV1> {
        self.records
            .values()
            .find(|record| record.outbox_reservation_id == reservation_id)
    }

    pub(super) fn records(&self) -> impl Iterator<Item = &KagemushaOutgoingOperationRecordV1> {
        self.records.values()
    }

    /// Select the exact ordered prefix under the current pinned revision.
    pub fn page(
        &self,
        pinned_revision: Option<u128>,
        after: Option<DigestV1>,
        maximum_entries: u16,
    ) -> KagemushaOutgoingOperationIndexResultV1<KagemushaOutgoingOperationPageV1> {
        if pinned_revision.is_some_and(|revision| revision != self.revision)
            || !(1..=KAGEMUSHA_OUTGOING_OPERATION_PAGE_MAX_V1).contains(&maximum_entries)
            || after == Some([0; 32])
            || (after.is_some() && pinned_revision.is_none())
            || after.is_some_and(|cursor| !self.records.contains_key(&cursor))
        {
            return Err(KagemushaOutgoingOperationIndexErrorV1::StalePage);
        }
        let mut selected = self
            .records
            .iter()
            .filter(|(id, _)| after.is_none_or(|cursor| **id > cursor))
            .map(|(_, record)| record.clone())
            .take(usize::from(maximum_entries) + 1)
            .collect::<Vec<_>>();
        let has_more = selected.len() > usize::from(maximum_entries);
        if has_more {
            selected.pop();
        }
        let next_cursor = if has_more {
            Some(
                selected
                    .last()
                    .ok_or(KagemushaOutgoingOperationIndexErrorV1::StalePage)?
                    .operation_id,
            )
        } else {
            None
        };
        Ok(KagemushaOutgoingOperationPageV1 {
            index_revision: self.revision,
            records: selected,
            next_cursor,
        })
    }

    /// Validate the complete recovered index against the stable current wallet.
    pub(super) fn validate_recovered(
        &self,
        current: &KagemushaStateV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<()> {
        self.validate_internal(Some(current))
    }

    fn progress_successor(
        &self,
        preparation_id: DigestV1,
        expected_phase: KagemushaOutgoingOperationPhaseV1,
        mutate: impl FnOnce(
            &mut KagemushaOutgoingOperationRecordV1,
            u128,
        ) -> KagemushaOutgoingOperationIndexResultV1<()>,
    ) -> KagemushaOutgoingOperationIndexResultV1<Self> {
        let operation_id = self
            .records
            .values()
            .find(|record| record.preparation_id == preparation_id)
            .map(|record| record.operation_id);
        let Some(operation_id) = operation_id else {
            // Legacy callers remain functional. Once a preparation is indexed,
            // every later journal transition finds it and must update it.
            return Ok(self.clone());
        };
        let existing = self
            .records
            .get(&operation_id)
            .ok_or(KagemushaOutgoingOperationIndexErrorV1::InvalidStage)?;
        if existing.phase != expected_phase {
            return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidStage);
        }
        let revision = next_revision(self.revision)?;
        let mut next = self.clone();
        let record = next
            .records
            .get_mut(&operation_id)
            .ok_or(KagemushaOutgoingOperationIndexErrorV1::InvalidStage)?;
        mutate(record, revision)?;
        record.validate()?;
        next.revision = revision;
        next.validate_internal(None)?;
        Ok(next)
    }

    fn validate_internal(
        &self,
        current: Option<&KagemushaStateV1>,
    ) -> KagemushaOutgoingOperationIndexResultV1<()> {
        if self.records.is_empty() {
            if self.revision != 0 || self.reserved_bytes != 0 {
                return Err(KagemushaOutgoingOperationIndexErrorV1::SnapshotIntegrity);
            }
            return Ok(());
        }
        if self.revision == 0 {
            return Err(KagemushaOutgoingOperationIndexErrorV1::SnapshotIntegrity);
        }
        let mut preparations = BTreeSet::new();
        let mut reservations = BTreeSet::new();
        let mut outcomes = BTreeSet::new();
        let mut charged = 0_u64;
        let mut stable_scope = None;
        for (operation_id, record) in &self.records {
            record.validate()?;
            if operation_id != &record.operation_id
                || record.record_revision > self.revision
                || !preparations.insert(record.preparation_id)
                || !reservations.insert(record.outbox_reservation_id)
                || !outcomes.insert(record.outcome_id)
            {
                return Err(KagemushaOutgoingOperationIndexErrorV1::SnapshotIntegrity);
            }
            let scope = (
                record.context.lane.clone(),
                record.context.release.asset_incarnation,
            );
            if stable_scope
                .as_ref()
                .is_some_and(|expected| expected != &scope)
            {
                return Err(KagemushaOutgoingOperationIndexErrorV1::SnapshotIntegrity);
            }
            stable_scope.get_or_insert(scope);
            if let Some(current) = current {
                record.context.validate_retained_against_state(current)?;
            }
            charged = charged
                .checked_add(record.reserved_record_bytes)
                .ok_or(KagemushaOutgoingOperationIndexErrorV1::SnapshotIntegrity)?;
        }
        if charged != self.reserved_bytes {
            return Err(KagemushaOutgoingOperationIndexErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
}

fn terminal_record_allocation(
    record: &KagemushaOutgoingOperationRecordV1,
) -> KagemushaOutgoingOperationIndexResultV1<u64> {
    let mut terminal = record.clone();
    terminal.phase = KagemushaOutgoingOperationPhaseV1::Installed;
    terminal.candidate_digest = Some([0xff; 32]);
    terminal.commit_certificate_digest = Some([0xff; 32]);
    terminal.envelope_digest = Some([0xff; 32]);
    terminal.acknowledgement_digest = None;
    canonical_len(&terminal)?
        .checked_add(RECORD_ALLOCATION_SAFETY_BYTES_V1)
        .ok_or(KagemushaOutgoingOperationIndexErrorV1::CanonicalEncoding)
}

fn next_revision(revision: u128) -> KagemushaOutgoingOperationIndexResultV1<u128> {
    revision
        .checked_add(1)
        .ok_or(KagemushaOutgoingOperationIndexErrorV1::RevisionOverflow)
}

fn nonzero_option(value: Option<DigestV1>) -> bool {
    value.is_some_and(|digest| digest != [0; 32])
}

fn canonical_len<T: Encode>(value: &T) -> KagemushaOutgoingOperationIndexResultV1<u64> {
    u64::try_from(canonical_bytes(value)?.len())
        .map_err(|_| KagemushaOutgoingOperationIndexErrorV1::CanonicalEncoding)
}

fn canonical_bytes<T: Encode>(value: &T) -> KagemushaOutgoingOperationIndexResultV1<Vec<u8>> {
    norito::encode_canonical(value)
        .map_err(|_| KagemushaOutgoingOperationIndexErrorV1::CanonicalEncoding)
}

fn decode_exact<T>(bytes: &[u8], maximum: usize) -> KagemushaOutgoingOperationIndexResultV1<T>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
    }
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(maximum, maximum, maximum * 4, maximum * 8, 32),
    )
    .map_err(|_| KagemushaOutgoingOperationIndexErrorV1::CanonicalEncoding)
}

fn decode_acknowledgement_exact(
    bytes: &[u8],
    request: &KagemushaPaymentRequestV1,
    payment: &KagemushaPaymentV1,
) -> KagemushaOutgoingOperationIndexResultV1<KagemushaAcknowledgementV1> {
    if bytes.is_empty() || bytes.len() > KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1 {
        return Err(KagemushaOutgoingOperationIndexErrorV1::InvalidBinding);
    }
    KagemushaAcknowledgementV1::decode_canonical_shape_exact_against(bytes, request, payment)
        .map_err(|_| KagemushaOutgoingOperationIndexErrorV1::InvalidBinding)
}

fn digest_bytes(domain: &[u8], bytes: &[u8]) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}
