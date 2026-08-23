//! Fail-closed staged release lifecycle for Kagemusha V4 issuance.
//!
//! Release activation installs a reviewed release in the `Staged` phase. Public
//! issuance remains disabled until a later enable transition authenticates the
//! finalized activation, its post-activation canary, and four-validator
//! post-canary liveness. Cancellation and deactivation retain durable terminal
//! state instead of deleting the lifecycle record.

use crate::{
    account::AccountId, block::BlockHeader, peer::PeerId, proof::VerifyingKeyId,
    transaction::SignedTransaction,
};
use iroha_crypto::{Algorithm, HashOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use super::{
    KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS, KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT,
    KagemushaExactBytesDigestV1, KagemushaRecursiveSpendArtifactBindingV4,
    KagemushaV4ActivationFinalityReceiptV1, KagemushaV4ActivationReceiptExpectationsArtifactV1,
    KagemushaV4PostCanaryValidatorLivenessEvidenceV1, KagemushaV4PromotionBindingV1,
    KagemushaV4PromotionReservationV1, KagemushaV4TairaCanaryAuthorizationV1,
    KagemushaV4TairaCanaryEvidenceV1, OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1,
    OfflineDeviceAttestationPolicy,
};

/// Stable metadata-key prefix for one manifest-scoped lifecycle record.
pub const KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_KEY_PREFIX_V1: &str =
    "kagemusha_release_lifecycle_v4_";
/// First and only lifecycle wire version.
pub const KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1: u16 = 1;
/// Schema id of one persisted lifecycle state.
pub const KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_SCHEMA_V1: &str =
    "iroha.kagemusha.v4.release_lifecycle_state.v1";
/// Schema id of a complete staged-to-enabled witness.
pub const KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_SCHEMA_V1: &str =
    "iroha.kagemusha.v4.issuance_enable_witness.v1";
/// Schema id of a staged-release cancellation transition.
pub const KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1: &str =
    "iroha.kagemusha.v4.release_cancellation.v1";
/// Schema id of an enabled-release deactivation transition.
pub const KAGEMUSHA_V4_RELEASE_DEACTIVATION_SCHEMA_V1: &str =
    "iroha.kagemusha.v4.release_deactivation.v1";
/// Maximum canonical bytes retained for one lifecycle state.
pub const KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_MAX_BYTES_V1: usize =
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1 + 256 * 1024;
/// Maximum canonical bytes accepted for the complete enable witness.
pub const KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_MAX_BYTES_V1: usize = 64 * 1024 * 1024;
/// Maximum canonical bytes accepted for cancellation or deactivation input.
pub const KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1: usize = 64 * 1024;
const KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_DECODE_STACK_BYTES_V1: usize = 32 * 1024 * 1024;

/// Derive the sole manifest-scoped lifecycle storage key.
///
/// # Errors
///
/// Returns [`KagemushaV4ReleaseLifecycleValidationError`] for the all-zero
/// manifest digest, which cannot identify a reviewed release.
pub fn kagemusha_v4_release_lifecycle_state_key(
    manifest_sha256: &[u8; 32],
) -> Result<String, KagemushaV4ReleaseLifecycleValidationError> {
    require_nonzero(manifest_sha256, "lifecycle.manifest_sha256")?;
    Ok(format!(
        "{KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_KEY_PREFIX_V1}{}",
        hex::encode(manifest_sha256)
    ))
}

/// Closed governance reason recorded for a terminal lifecycle transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "reason", content = "value", rename_all = "snake_case")]
pub enum KagemushaV4ReleaseLifecycleReasonV1 {
    /// The signed four-validator post-canary liveness closure could not be obtained.
    LivenessClosureFailed,
    /// Governance cancelled a staged release before issuance was enabled.
    GovernanceCancelled,
    /// Governance stopped issuance after an enabled release became unsafe.
    EmergencyDeactivation,
    /// Governance withdrew a policy required by the release.
    PolicyWithdrawn,
}

/// Complete bounded evidence needed to move a staged release to enabled.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4IssuanceEnableWitnessV1 {
    /// Exact enable-witness schema.
    pub schema: String,
    /// Enable-witness version.
    pub version: u16,
    /// Exact canonical digest of the currently stored staged lifecycle state.
    pub expected_predecessor_lifecycle: KagemushaExactBytesDigestV1,
    /// Unique non-zero replay identity for this enable transition.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transition_id: [u8; 32],
    /// Root-controller reservation for the exact release promotion.
    pub promotion_reservation: KagemushaV4PromotionReservationV1,
    /// Root-controller expectations for the staged activation transaction.
    pub stage_expectations: KagemushaV4ActivationReceiptExpectationsArtifactV1,
    /// Independent proof that the staged activation finalized successfully.
    pub stage_finality_receipt: KagemushaV4ActivationFinalityReceiptV1,
    /// Controller-authorized exact post-stage canary transaction.
    pub canary_authorization: KagemushaV4TairaCanaryAuthorizationV1,
    /// Independent proof that the authorized canary finalized successfully.
    pub canary_evidence: KagemushaV4TairaCanaryEvidenceV1,
    /// Signed post-canary liveness evidence from all four qualified validators.
    pub validator_liveness_evidence: KagemushaV4PostCanaryValidatorLivenessEvidenceV1,
}

impl KagemushaV4IssuanceEnableWitnessV1 {
    /// Decode one exact canonical enable witness under its outer byte ceiling.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4ReleaseLifecycleValidationError`] for empty,
    /// oversized, non-canonical, malformed, or cross-spliced input.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, KagemushaV4ReleaseLifecycleValidationError> {
        check_input_size(bytes, KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_MAX_BYTES_V1)?;
        let value: Self = std::thread::scope(|scope| {
            let decoder = std::thread::Builder::new()
                .name("kagemusha-enable-witness-decode".to_owned())
                .stack_size(KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_DECODE_STACK_BYTES_V1)
                .spawn_scoped(scope, || {
                    norito::decode_canonical_with_limits(
                        bytes,
                        norito::canonical_decode_limits(bytes.len()),
                    )
                    .map_err(|_| KagemushaV4ReleaseLifecycleValidationError::Decode)
                })
                .map_err(|_| KagemushaV4ReleaseLifecycleValidationError::Decode)?;
            decoder
                .join()
                .map_err(|_| KagemushaV4ReleaseLifecycleValidationError::Decode)?
        })?;
        value.validate()?;
        require_canonical_bytes(&value, bytes)?;
        Ok(value)
    }

    /// Validate the bounded witness and all exact cross-artifact identities.
    ///
    /// Cryptographic finality and live consensus context remain executor checks;
    /// this method rejects malformed, oversized, or cross-promotion model data
    /// before those contextual checks run.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4ReleaseLifecycleValidationError`] for any invalid
    /// schema, transition identity, artifact shape, exact-byte digest, or
    /// cross-artifact binding.
    pub fn validate(&self) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        if self.schema != KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_SCHEMA_V1
            || self.version != KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1
        {
            return Err(invalid("enable_witness"));
        }
        validate_exact_bytes(
            &self.expected_predecessor_lifecycle,
            "enable_witness.expected_predecessor_lifecycle",
        )?;
        require_nonzero(&self.transition_id, "enable_witness.transition_id")?;

        let reservation = canonical_artifact(
            &self.promotion_reservation,
            "enable_witness.promotion_reservation",
        )?;
        let expectations = canonical_artifact(
            &self.stage_expectations,
            "enable_witness.stage_expectations",
        )?;
        let receipt = canonical_artifact(
            &self.stage_finality_receipt,
            "enable_witness.stage_finality_receipt",
        )?;
        let authorization = canonical_artifact(
            &self.canary_authorization,
            "enable_witness.canary_authorization",
        )?;
        let canary_evidence =
            canonical_artifact(&self.canary_evidence, "enable_witness.canary_evidence")?;
        let liveness_evidence = canonical_artifact(
            &self.validator_liveness_evidence,
            "enable_witness.validator_liveness_evidence",
        )?;
        enforce_encoded_size(self, KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_MAX_BYTES_V1)?;

        self.promotion_reservation
            .verify(&self.promotion_reservation.body.promotion_controller)
            .map_err(|_| invalid("enable_witness.promotion_reservation"))?;
        KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_canonical(&expectations)
            .map_err(|_| invalid("enable_witness.stage_expectations"))?;
        KagemushaV4ActivationFinalityReceiptV1::decode_canonical(&receipt)
            .map_err(|_| invalid("enable_witness.stage_finality_receipt"))?;
        KagemushaV4TairaCanaryAuthorizationV1::decode_canonical(&authorization)
            .map_err(|_| invalid("enable_witness.canary_authorization"))?;
        KagemushaV4TairaCanaryEvidenceV1::decode_canonical(&canary_evidence)
            .map_err(|_| invalid("enable_witness.canary_evidence"))?;
        KagemushaV4PostCanaryValidatorLivenessEvidenceV1::decode_canonical(&liveness_evidence)
            .map_err(|_| invalid("enable_witness.validator_liveness_evidence"))?;

        let reservation_id = exact_digest(&reservation)?;
        let expectations_id = exact_digest(&expectations)?;
        let receipt_id = exact_digest(&receipt)?;
        let authorization_id = exact_digest(&authorization)?;
        let binding = &self.stage_expectations.body.binding;
        let receipt_body = &self.stage_finality_receipt.body;
        let authorization_body = &self.canary_authorization.reservation.body;
        let permit_body = &authorization_body.permit.body;
        let canary_body = &self.canary_evidence.body;
        let liveness_body = &self.validator_liveness_evidence.body;
        let challenge = &liveness_body.challenge.body;
        let anchor = &challenge.canary_anchor;

        if self.promotion_reservation.body.promotion_id != binding.promotion_id
            || self.promotion_reservation.body.manifest_sha256 != binding.manifest_sha256
            || self.promotion_reservation.body.promotion_controller != binding.promotion_controller
            || binding.promotion_reservation != reservation_id
            || self.stage_expectations.body.promotion_reservation != reservation_id
            || receipt_body.promotion_reservation != reservation_id
            || receipt_body.activation_expectations_artifact != expectations_id
            || receipt_body.binding != *binding
            || permit_body.binding != *binding
            || permit_body.activation_expectations_artifact != expectations_id
            || permit_body.activation_finality_receipt != receipt_id
            || canary_body.promotion_id != binding.promotion_id
            || canary_body.network_id != binding.network_id
            || canary_body.promotion_reservation != reservation_id
            || canary_body.activation_expectations_artifact != expectations_id
            || canary_body.activation_finality_receipt != receipt_id
            || canary_body.canary_authorization != authorization_id
            || canary_body.issuer != receipt_body.issuer
            || authorization_body.canary_transaction_intent != canary_body.canary_transaction_intent
            || authorization_body.canary_transaction_wire != canary_body.canary_transaction_wire
            || challenge.binding != *binding
            || challenge.issuer != receipt_body.issuer
            || anchor.activation_finality_receipt != receipt_id
            || anchor.canary_authorization != authorization_id
            || anchor.canary_transaction_intent != canary_body.canary_transaction_intent
            || anchor.canary_transaction_wire != canary_body.canary_transaction_wire
            || anchor.canary_finalized_height != canary_body.finalized_height
            || anchor.canary_finalized_block_hash != canary_body.finalized_block_hash
            || liveness_body.endpoint_challenge == [0; 32]
        {
            return Err(invalid("enable_witness.cross_binding"));
        }
        Ok(())
    }

    /// Return the exact identity of the validated canonical witness bytes.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4ReleaseLifecycleValidationError`] when validation
    /// or canonical encoding fails.
    pub fn exact_bytes_digest(
        &self,
    ) -> Result<KagemushaExactBytesDigestV1, KagemushaV4ReleaseLifecycleValidationError> {
        self.validate()?;
        exact_digest(&canonical_artifact(self, "enable_witness")?)
    }
}

/// Governance-authenticated cancellation of a staged release.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ReleaseCancellationV1 {
    /// Exact cancellation schema.
    pub schema: String,
    /// Cancellation version.
    pub version: u16,
    /// Exact promotion whose staged lifecycle must be cancelled.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub promotion_id: [u8; 32],
    /// Exact promoted manifest identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub manifest_sha256: [u8; 32],
    /// Exact canonical digest of the currently stored staged state.
    pub expected_predecessor_lifecycle: KagemushaExactBytesDigestV1,
    /// Unique non-zero replay identity for this transition.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transition_id: [u8; 32],
    /// Closed reason for cancelling before enablement.
    pub reason: KagemushaV4ReleaseLifecycleReasonV1,
    /// Optional exact immutable evidence supporting the governance decision.
    pub evidence: Option<KagemushaExactBytesDigestV1>,
}

impl KagemushaV4ReleaseCancellationV1 {
    /// Decode one bounded exact canonical cancellation.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4ReleaseLifecycleValidationError`] for empty,
    /// oversized, non-canonical, or invalid input.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, KagemushaV4ReleaseLifecycleValidationError> {
        check_input_size(bytes, KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1)?;
        let value: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| KagemushaV4ReleaseLifecycleValidationError::Decode)?;
        value.validate()?;
        require_canonical_bytes(&value, bytes)?;
        Ok(value)
    }

    /// Validate the exact predecessor, release identity, replay id, and reason.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4ReleaseLifecycleValidationError`] for any invalid
    /// field or oversized canonical transition.
    pub fn validate(&self) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        if self.schema != KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1
            || self.version != KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1
            || !matches!(
                self.reason,
                KagemushaV4ReleaseLifecycleReasonV1::LivenessClosureFailed
                    | KagemushaV4ReleaseLifecycleReasonV1::GovernanceCancelled
                    | KagemushaV4ReleaseLifecycleReasonV1::PolicyWithdrawn
            )
        {
            return Err(invalid("cancellation"));
        }
        validate_transition_identity(
            &self.promotion_id,
            &self.manifest_sha256,
            &self.expected_predecessor_lifecycle,
            &self.transition_id,
            self.evidence.as_ref(),
            "cancellation",
        )?;
        enforce_encoded_size(self, KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1)
    }
}

/// Governance-authenticated terminal deactivation of enabled issuance.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ReleaseDeactivationV1 {
    /// Exact deactivation schema.
    pub schema: String,
    /// Deactivation version.
    pub version: u16,
    /// Exact promotion whose issuance must be stopped.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub promotion_id: [u8; 32],
    /// Exact promoted manifest identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub manifest_sha256: [u8; 32],
    /// Exact canonical digest of the currently stored enabled state.
    pub expected_predecessor_lifecycle: KagemushaExactBytesDigestV1,
    /// Unique non-zero replay identity for this transition.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transition_id: [u8; 32],
    /// Closed reason for stopping issuance.
    pub reason: KagemushaV4ReleaseLifecycleReasonV1,
    /// Optional exact immutable evidence supporting the governance decision.
    pub evidence: Option<KagemushaExactBytesDigestV1>,
}

impl KagemushaV4ReleaseDeactivationV1 {
    /// Decode one bounded exact canonical deactivation.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4ReleaseLifecycleValidationError`] for empty,
    /// oversized, non-canonical, or invalid input.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, KagemushaV4ReleaseLifecycleValidationError> {
        check_input_size(bytes, KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1)?;
        let value: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| KagemushaV4ReleaseLifecycleValidationError::Decode)?;
        value.validate()?;
        require_canonical_bytes(&value, bytes)?;
        Ok(value)
    }

    /// Validate the exact predecessor, release identity, replay id, and reason.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4ReleaseLifecycleValidationError`] for any invalid
    /// field or oversized canonical transition.
    pub fn validate(&self) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        if self.schema != KAGEMUSHA_V4_RELEASE_DEACTIVATION_SCHEMA_V1
            || self.version != KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1
            || !matches!(
                self.reason,
                KagemushaV4ReleaseLifecycleReasonV1::EmergencyDeactivation
                    | KagemushaV4ReleaseLifecycleReasonV1::PolicyWithdrawn
            )
        {
            return Err(invalid("deactivation"));
        }
        validate_transition_identity(
            &self.promotion_id,
            &self.manifest_sha256,
            &self.expected_predecessor_lifecycle,
            &self.transition_id,
            self.evidence.as_ref(),
            "deactivation",
        )?;
        enforce_encoded_size(self, KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1)
    }
}

/// Consensus projection retained after a successful staged-to-enabled transition.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ReleaseEnabledV1 {
    /// Exact predecessor state authenticated by the enable witness.
    pub expected_staged_lifecycle: KagemushaExactBytesDigestV1,
    /// Unique replay identity of the enable transition.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transition_id: [u8; 32],
    /// Exact canonical enable-witness identity.
    pub enable_witness_norito: KagemushaExactBytesDigestV1,
    /// Payload-only intent of the governance enable transaction.
    pub enable_transaction_intent: HashOf<SignedTransaction>,
    /// Consensus height at which issuance became enabled.
    pub enabled_at_height: u64,
    /// Consensus block time at which issuance became enabled.
    pub enabled_at_unix_ms: u64,
    /// Exact canonical four-validator liveness evidence identity.
    pub validator_liveness_evidence: KagemushaExactBytesDigestV1,
    /// Authenticated payload-only canary transaction intent.
    pub canary_transaction_intent: HashOf<SignedTransaction>,
    /// Authenticated finalized canary carrier height.
    pub canary_finalized_height: u64,
    /// Authenticated finalized canary carrier hash.
    pub canary_finalized_block_hash: HashOf<BlockHeader>,
    /// Common non-zero endpoint challenge signed by all validators.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub endpoint_challenge: [u8; 32],
    /// Four exact qualified validators in canonical order.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_array"))]
    pub validator_ids: [PeerId; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    /// Authenticated durable-tip height observed from each validator.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_array"))]
    pub observed_tip_heights: [u64; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    /// Exact terminal height of the shared post-canary proof chain.
    pub highest_observed_tip_height: u64,
}

impl KagemushaV4ReleaseEnabledV1 {
    fn validate(&self) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        validate_exact_bytes(
            &self.expected_staged_lifecycle,
            "enabled.expected_staged_lifecycle",
        )?;
        validate_exact_bytes(&self.enable_witness_norito, "enabled.enable_witness_norito")?;
        validate_exact_bytes(
            &self.validator_liveness_evidence,
            "enabled.validator_liveness_evidence",
        )?;
        require_nonzero(&self.transition_id, "enabled.transition_id")?;
        require_nonzero(&self.endpoint_challenge, "enabled.endpoint_challenge")?;
        if self.enabled_at_height == 0
            || self.enabled_at_unix_ms == 0
            || self.canary_finalized_height == 0
            || self.enabled_at_height <= self.canary_finalized_height
            || typed_hash_is_zero(&self.enable_transaction_intent)
            || typed_hash_is_zero(&self.canary_transaction_intent)
            || typed_hash_is_zero(&self.canary_finalized_block_hash)
            || self.highest_observed_tip_height < self.canary_finalized_height
            || self
                .observed_tip_heights
                .iter()
                .any(|height| *height < self.canary_finalized_height)
            || self.observed_tip_heights.iter().copied().max()
                != Some(self.highest_observed_tip_height)
        {
            return Err(invalid("enabled"));
        }
        let mut previous = None;
        for validator_id in &self.validator_ids {
            if validator_id.public_key().try_algorithm() != Ok(Algorithm::BlsNormal)
                || previous.is_some_and(|prior: &PeerId| prior >= validator_id)
            {
                return Err(invalid("enabled.validator_ids"));
            }
            previous = Some(validator_id);
        }
        Ok(())
    }
}

/// Durable terminal projection for a cancelled staged release.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ReleaseCancelledV1 {
    /// Exact governance cancellation input.
    pub cancellation: KagemushaV4ReleaseCancellationV1,
    /// Payload-only intent of the cancellation transaction.
    pub cancellation_transaction_intent: HashOf<SignedTransaction>,
    /// Consensus height at which cancellation became terminal.
    pub cancelled_at_height: u64,
    /// Consensus block time at which cancellation became terminal.
    pub cancelled_at_unix_ms: u64,
}

impl KagemushaV4ReleaseCancelledV1 {
    fn validate(&self) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        self.cancellation.validate()?;
        if self.cancelled_at_height == 0
            || self.cancelled_at_unix_ms == 0
            || typed_hash_is_zero(&self.cancellation_transaction_intent)
        {
            return Err(invalid("cancelled"));
        }
        Ok(())
    }
}

/// Durable terminal projection for a deactivated formerly enabled release.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ReleaseDeactivatedV1 {
    /// Immutable projection of the prior enabled state.
    pub enabled: KagemushaV4ReleaseEnabledV1,
    /// Exact governance deactivation input.
    pub deactivation: KagemushaV4ReleaseDeactivationV1,
    /// Payload-only intent of the deactivation transaction.
    pub deactivation_transaction_intent: HashOf<SignedTransaction>,
    /// Consensus height at which deactivation became terminal.
    pub deactivated_at_height: u64,
    /// Consensus block time at which deactivation became terminal.
    pub deactivated_at_unix_ms: u64,
}

impl KagemushaV4ReleaseDeactivatedV1 {
    fn validate(&self) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        self.enabled.validate()?;
        self.deactivation.validate()?;
        if self.deactivated_at_height <= self.enabled.enabled_at_height
            || self.deactivated_at_unix_ms < self.enabled.enabled_at_unix_ms
            || typed_hash_is_zero(&self.deactivation_transaction_intent)
            || self.deactivation.transition_id == self.enabled.transition_id
        {
            return Err(invalid("deactivated"));
        }
        Ok(())
    }
}

/// Closed persisted lifecycle phase; absence is deliberately not a phase.
///
/// Payload-bearing phases use owned boxes so the phase discriminant stays
/// uniformly small. Norito therefore encodes each payload in its own bounded,
/// length-delimited owned-value frame.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(tag = "phase", content = "value", rename_all = "snake_case")]
pub enum KagemushaV4ReleaseLifecyclePhaseV1 {
    /// Release is installed for canary execution but public issuance is disabled.
    Staged,
    /// Public issuance is enabled by complete signed post-canary evidence.
    Enabled(Box<KagemushaV4ReleaseEnabledV1>),
    /// A staged release was cancelled and can never be enabled.
    Cancelled(Box<KagemushaV4ReleaseCancelledV1>),
    /// A formerly enabled release no longer permits new issuance.
    Deactivated(Box<KagemushaV4ReleaseDeactivatedV1>),
}

/// Manifest-scoped consensus lifecycle record for one installed V4 release.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ReleaseLifecycleStateV1 {
    /// Exact lifecycle-state schema.
    pub schema: String,
    /// Lifecycle-state version.
    pub version: u16,
    /// Complete promotion, release, network, catalog, and execution identity.
    pub promotion_binding: KagemushaV4PromotionBindingV1,
    /// Installed manifest generation and digest.
    pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
    /// Exact multisignature account governing every lifecycle transition.
    pub governance_authority: AccountId,
    /// Payload-only intent of the stage transaction.
    pub stage_transaction_intent: HashOf<SignedTransaction>,
    /// Consensus height at which the release entered `Staged`.
    pub staged_at_height: u64,
    /// Consensus block time at which the release entered `Staged`.
    pub staged_at_unix_ms: u64,
    /// Exact canonical release-record identity installed by staging.
    pub release_record_norito: KagemushaExactBytesDigestV1,
    /// Exact governed policy retained for this release's future redemptions.
    pub device_attestation_policy: OfflineDeviceAttestationPolicy,
    /// Registry id of the installed EqAffine/Vesta verifier key.
    pub step_eq_verifier_key_id: VerifyingKeyId,
    /// Registry id of the installed EpAffine/Pallas verifier key.
    pub step_ep_verifier_key_id: VerifyingKeyId,
    /// Non-zero release verifier generation installed by staging.
    pub verifier_version: u32,
    /// Current non-optional lifecycle phase.
    pub phase: KagemushaV4ReleaseLifecyclePhaseV1,
}

impl KagemushaV4ReleaseLifecycleStateV1 {
    /// Decode one exact bounded canonical lifecycle state.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4ReleaseLifecycleValidationError`] for empty,
    /// oversized, non-canonical, or invalid state.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, KagemushaV4ReleaseLifecycleValidationError> {
        check_input_size(bytes, KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_MAX_BYTES_V1)?;
        let value: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| KagemushaV4ReleaseLifecycleValidationError::Decode)?;
        value.validate()?;
        require_canonical_bytes(&value, bytes)?;
        Ok(value)
    }

    /// Return whether and only whether this persisted state permits new issuance.
    #[must_use]
    pub const fn issuance_enabled(&self) -> bool {
        matches!(&self.phase, KagemushaV4ReleaseLifecyclePhaseV1::Enabled(_))
    }

    /// Validate release identity, governance authority, phase ordering, and exact predecessor.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4ReleaseLifecycleValidationError`] for malformed or
    /// oversized state, a weak authority, an illegal terminal transition, or a
    /// predecessor digest that is not the exact canonical previous state.
    pub fn validate(&self) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        self.validate_common_fields()?;
        self.validate_phase_transition()?;
        enforce_encoded_size(self, KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_MAX_BYTES_V1)
    }

    /// Return the exact identity of the validated canonical lifecycle state.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4ReleaseLifecycleValidationError`] when validation
    /// or canonical encoding fails.
    pub fn exact_bytes_digest(
        &self,
    ) -> Result<KagemushaExactBytesDigestV1, KagemushaV4ReleaseLifecycleValidationError> {
        self.validate()?;
        self.canonical_digest_unchecked()
    }

    fn validate_common_fields(&self) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        if self.schema != KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_SCHEMA_V1
            || self.version != KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1
            || self.staged_at_height == 0
            || self.staged_at_unix_ms == 0
            || self.verifier_version == 0
            || typed_hash_is_zero(&self.stage_transaction_intent)
            || !self.step_eq_verifier_key_id.is_portable_registry_id()
            || !self.step_ep_verifier_key_id.is_portable_registry_id()
            || self.step_eq_verifier_key_id == self.step_ep_verifier_key_id
        {
            return Err(invalid("lifecycle"));
        }
        self.promotion_binding
            .validate()
            .map_err(|_| invalid("lifecycle.promotion_binding"))?;
        self.artifact_binding
            .validate()
            .map_err(|_| invalid("lifecycle.artifact_binding"))?;
        validate_exact_bytes(
            &self.release_record_norito,
            "lifecycle.release_record_norito",
        )?;
        let device_attestation_policy_norito = canonical_artifact(
            &self.device_attestation_policy,
            "lifecycle.device_attestation_policy",
        )?;
        check_input_size(
            &device_attestation_policy_norito,
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1,
        )?;
        if self.promotion_binding.manifest_sha256 != self.artifact_binding.manifest_sha256 {
            return Err(invalid("lifecycle.manifest_identity"));
        }
        if self.promotion_binding.release_record_sha256 != self.release_record_norito.sha256 {
            return Err(invalid("lifecycle.release_record_identity"));
        }
        if !self
            .promotion_binding
            .device_attestation_policy_norito
            .matches_bytes(&device_attestation_policy_norito)
        {
            return Err(invalid("lifecycle.device_attestation_policy_identity"));
        }
        let Some(governance_policy) = self.governance_authority.controller().multisig_policy()
        else {
            return Err(invalid("lifecycle.governance_authority"));
        };
        if usize::from(governance_policy.threshold())
            < KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS
            || governance_policy.members().len() < KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS
        {
            return Err(invalid("lifecycle.governance_authority"));
        }
        Ok(())
    }

    fn validate_phase_transition(&self) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        let staged_predecessor = self.with_phase(KagemushaV4ReleaseLifecyclePhaseV1::Staged);
        let staged_predecessor_id = staged_predecessor.canonical_digest_unchecked()?;
        match &self.phase {
            KagemushaV4ReleaseLifecyclePhaseV1::Staged => {}
            KagemushaV4ReleaseLifecyclePhaseV1::Enabled(enabled) => {
                self.validate_enabled_transition(enabled, staged_predecessor_id)?;
            }
            KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(cancelled) => {
                self.validate_cancelled_transition(cancelled, staged_predecessor_id)?;
            }
            KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(deactivated) => {
                self.validate_deactivated_transition(deactivated, staged_predecessor_id)?;
            }
        }
        Ok(())
    }

    fn validate_enabled_transition(
        &self,
        enabled: &KagemushaV4ReleaseEnabledV1,
        staged_predecessor_id: KagemushaExactBytesDigestV1,
    ) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        enabled.validate()?;
        if enabled.expected_staged_lifecycle != staged_predecessor_id
            || enabled.enabled_at_height <= self.staged_at_height
            || enabled.enabled_at_unix_ms < self.staged_at_unix_ms
            || enabled.enable_transaction_intent == self.stage_transaction_intent
        {
            return Err(invalid("lifecycle.enabled_transition"));
        }
        Ok(())
    }

    fn validate_cancelled_transition(
        &self,
        cancelled: &KagemushaV4ReleaseCancelledV1,
        staged_predecessor_id: KagemushaExactBytesDigestV1,
    ) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        cancelled.validate()?;
        if cancelled.cancellation.promotion_id != self.promotion_binding.promotion_id
            || cancelled.cancellation.manifest_sha256 != self.promotion_binding.manifest_sha256
            || cancelled.cancellation.expected_predecessor_lifecycle != staged_predecessor_id
            || cancelled.cancelled_at_height <= self.staged_at_height
            || cancelled.cancelled_at_unix_ms < self.staged_at_unix_ms
            || cancelled.cancellation_transaction_intent == self.stage_transaction_intent
        {
            return Err(invalid("lifecycle.cancelled_transition"));
        }
        Ok(())
    }

    fn validate_deactivated_transition(
        &self,
        deactivated: &KagemushaV4ReleaseDeactivatedV1,
        staged_predecessor_id: KagemushaExactBytesDigestV1,
    ) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        deactivated.validate()?;
        if deactivated.deactivation.promotion_id != self.promotion_binding.promotion_id
            || deactivated.deactivation.manifest_sha256 != self.promotion_binding.manifest_sha256
            || deactivated.enabled.expected_staged_lifecycle != staged_predecessor_id
            || deactivated.enabled.enabled_at_height <= self.staged_at_height
            || deactivated.enabled.enabled_at_unix_ms < self.staged_at_unix_ms
            || deactivated.enabled.enable_transaction_intent == self.stage_transaction_intent
        {
            return Err(invalid("lifecycle.deactivated_transition"));
        }
        let enabled_predecessor = self.with_phase(KagemushaV4ReleaseLifecyclePhaseV1::Enabled(
            Box::new(deactivated.enabled.clone()),
        ));
        if deactivated.deactivation.expected_predecessor_lifecycle
            != enabled_predecessor.canonical_digest_unchecked()?
        {
            return Err(invalid("lifecycle.deactivated_predecessor"));
        }
        Ok(())
    }

    fn with_phase(&self, phase: KagemushaV4ReleaseLifecyclePhaseV1) -> Self {
        let mut state = self.clone();
        state.phase = phase;
        state
    }

    fn canonical_digest_unchecked(
        &self,
    ) -> Result<KagemushaExactBytesDigestV1, KagemushaV4ReleaseLifecycleValidationError> {
        exact_digest(&canonical_artifact(self, "lifecycle")?)
    }
}

/// Failure while decoding or validating a release lifecycle value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum KagemushaV4ReleaseLifecycleValidationError {
    /// One named structural, identity, or transition field is invalid.
    #[error("invalid Kagemusha V4 release lifecycle field: {0}")]
    InvalidField(&'static str),
    /// Canonical lifecycle encoding failed.
    #[error("failed to encode canonical Kagemusha V4 release lifecycle data")]
    Encode,
    /// Bounded canonical lifecycle decoding failed.
    #[error("failed to decode canonical Kagemusha V4 release lifecycle data")]
    Decode,
    /// A lifecycle artifact violates its outer byte ceiling.
    #[error("Kagemusha V4 release lifecycle artifact is {actual} bytes; maximum is {maximum}")]
    Size {
        /// Actual supplied or encoded byte length.
        actual: usize,
        /// Maximum accepted byte length.
        maximum: usize,
    },
}

fn invalid(field: &'static str) -> KagemushaV4ReleaseLifecycleValidationError {
    KagemushaV4ReleaseLifecycleValidationError::InvalidField(field)
}

fn require_nonzero(
    value: &[u8; 32],
    field: &'static str,
) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
    if value == &[0; 32] {
        return Err(invalid(field));
    }
    Ok(())
}

fn validate_exact_bytes(
    value: &KagemushaExactBytesDigestV1,
    field: &'static str,
) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
    value.validate().map_err(|_| invalid(field))
}

fn validate_transition_identity(
    promotion_id: &[u8; 32],
    manifest_sha256: &[u8; 32],
    predecessor: &KagemushaExactBytesDigestV1,
    transition_id: &[u8; 32],
    evidence: Option<&KagemushaExactBytesDigestV1>,
    field: &'static str,
) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
    require_nonzero(promotion_id, field)?;
    require_nonzero(manifest_sha256, field)?;
    require_nonzero(transition_id, field)?;
    validate_exact_bytes(predecessor, field)?;
    if let Some(evidence) = evidence {
        validate_exact_bytes(evidence, field)?;
    }
    Ok(())
}

fn check_input_size(
    bytes: &[u8],
    maximum: usize,
) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(KagemushaV4ReleaseLifecycleValidationError::Size {
            actual: bytes.len(),
            maximum,
        });
    }
    Ok(())
}

fn canonical_artifact<T: Encode>(
    value: &T,
    _field: &'static str,
) -> Result<Vec<u8>, KagemushaV4ReleaseLifecycleValidationError> {
    norito::encode_canonical(value).map_err(|_| KagemushaV4ReleaseLifecycleValidationError::Encode)
}

fn exact_digest(
    bytes: &[u8],
) -> Result<KagemushaExactBytesDigestV1, KagemushaV4ReleaseLifecycleValidationError> {
    KagemushaExactBytesDigestV1::from_bytes(bytes)
        .map_err(|_| KagemushaV4ReleaseLifecycleValidationError::Encode)
}

fn enforce_encoded_size<T: Encode>(
    value: &T,
    maximum: usize,
) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
    let bytes = canonical_artifact(value, "size")?;
    if bytes.len() > maximum {
        return Err(KagemushaV4ReleaseLifecycleValidationError::Size {
            actual: bytes.len(),
            maximum,
        });
    }
    Ok(())
}

fn require_canonical_bytes<T: Encode>(
    value: &T,
    bytes: &[u8],
) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
    if canonical_artifact(value, "canonical")? != bytes {
        return Err(KagemushaV4ReleaseLifecycleValidationError::Decode);
    }
    Ok(())
}

fn typed_hash_is_zero<T>(value: &HashOf<T>) -> bool {
    value.as_ref().as_ref().iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact(seed: u8) -> KagemushaExactBytesDigestV1 {
        KagemushaExactBytesDigestV1 {
            byte_len: u64::from(seed) + 1,
            sha256: [seed.max(1); 32],
        }
    }

    fn cancellation() -> KagemushaV4ReleaseCancellationV1 {
        KagemushaV4ReleaseCancellationV1 {
            schema: KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
            promotion_id: [1; 32],
            manifest_sha256: [2; 32],
            expected_predecessor_lifecycle: exact(3),
            transition_id: [4; 32],
            reason: KagemushaV4ReleaseLifecycleReasonV1::GovernanceCancelled,
            evidence: Some(exact(5)),
        }
    }

    fn deactivation() -> KagemushaV4ReleaseDeactivationV1 {
        KagemushaV4ReleaseDeactivationV1 {
            schema: KAGEMUSHA_V4_RELEASE_DEACTIVATION_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
            promotion_id: [1; 32],
            manifest_sha256: [2; 32],
            expected_predecessor_lifecycle: exact(3),
            transition_id: [6; 32],
            reason: KagemushaV4ReleaseLifecycleReasonV1::EmergencyDeactivation,
            evidence: None,
        }
    }

    #[test]
    fn lifecycle_state_key_is_exact_and_rejects_zero_manifest_digest() {
        assert_eq!(
            kagemusha_v4_release_lifecycle_state_key(&[0xAB; 32])
                .expect("non-zero manifest digest"),
            format!(
                "{KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_KEY_PREFIX_V1}{}",
                "ab".repeat(32)
            )
        );
        assert!(kagemusha_v4_release_lifecycle_state_key(&[0; 32]).is_err());
    }

    #[test]
    fn terminal_transitions_are_bounded_canonical_and_reason_closed() {
        let cancellation = cancellation();
        cancellation.validate().expect("valid cancellation");
        let bytes = norito::encode_canonical(&cancellation).expect("encode cancellation");
        assert_eq!(
            KagemushaV4ReleaseCancellationV1::decode_canonical(&bytes)
                .expect("decode cancellation"),
            cancellation
        );

        let deactivation = deactivation();
        deactivation.validate().expect("valid deactivation");
        let bytes = norito::encode_canonical(&deactivation).expect("encode deactivation");
        assert_eq!(
            KagemushaV4ReleaseDeactivationV1::decode_canonical(&bytes)
                .expect("decode deactivation"),
            deactivation
        );

        let mut invalid_cancel = cancellation;
        invalid_cancel.reason = KagemushaV4ReleaseLifecycleReasonV1::EmergencyDeactivation;
        assert!(invalid_cancel.validate().is_err());
        let mut invalid_deactivate = deactivation;
        invalid_deactivate.reason = KagemushaV4ReleaseLifecycleReasonV1::GovernanceCancelled;
        assert!(invalid_deactivate.validate().is_err());
    }

    #[test]
    fn boxed_lifecycle_phase_payload_has_bounded_layout_and_canonical_roundtrip() {
        assert!(
            std::mem::size_of::<KagemushaV4ReleaseLifecyclePhaseV1>()
                <= 2 * std::mem::size_of::<usize>(),
            "lifecycle phase payloads must remain behind one-word indirections",
        );
        let phase = KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(Box::new(
            KagemushaV4ReleaseCancelledV1 {
                cancellation: cancellation(),
                cancellation_transaction_intent: HashOf::from_untyped_unchecked(
                    iroha_crypto::Hash::new(b"boxed lifecycle cancellation"),
                ),
                cancelled_at_height: 1,
                cancelled_at_unix_ms: 1,
            },
        ));
        let bytes = norito::encode_canonical(&phase).expect("encode boxed lifecycle phase");
        let decoded: KagemushaV4ReleaseLifecyclePhaseV1 = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .expect("decode boxed lifecycle phase");
        assert_eq!(decoded, phase);
        assert_eq!(
            norito::encode_canonical(&decoded).expect("re-encode boxed lifecycle phase"),
            bytes,
        );
    }

    #[test]
    fn bounded_decoders_reject_empty_and_trailing_input() {
        assert!(KagemushaV4IssuanceEnableWitnessV1::decode_canonical(&[]).is_err());
        assert!(KagemushaV4ReleaseLifecycleStateV1::decode_canonical(&[]).is_err());
        assert!(KagemushaV4ReleaseCancellationV1::decode_canonical(&[]).is_err());
        assert!(KagemushaV4ReleaseDeactivationV1::decode_canonical(&[]).is_err());

        let mut bytes = norito::encode_canonical(&cancellation()).expect("encode cancellation");
        bytes.push(0);
        assert!(KagemushaV4ReleaseCancellationV1::decode_canonical(&bytes).is_err());
    }
}
