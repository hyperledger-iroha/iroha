//! Atomic private cross-dataspace settlement carrier instructions.
//!
//! The carrier contains only the public manifest, opaque deltas, and committee
//! certificates. Proof bytes, audit capsules, plaintext parties, assets,
//! amounts, and memos never cross this instruction boundary.

use super::*;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    nexus::{
        ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1, AtomicPrivateSettlementV1, DataSpaceId,
        PrivateSettlementAbortReasonV1, PrivateSettlementCommitBundleV1,
        PrivateSettlementPoolGovernanceLifecycleV1, PrivateSettlementPoolGovernanceV1,
        PrivateSettlementRouteV1,
    },
    privacy::{PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1, PrivacyCommitmentV1, PrivacyPoolIdV1},
};

/// Structural failure for a public private-settlement pool activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PrivateSettlementPoolActivationValidationErrorV1 {
    /// The instruction used an unsupported wire version.
    #[error("unsupported private-settlement pool activation version {actual}")]
    UnsupportedVersion {
        /// Actual version.
        actual: u8,
    },
    /// The public route is universal or has a reserved lane incarnation.
    #[error("private-settlement pool activation route is invalid")]
    InvalidRoute,
    /// A required public pool, policy, epoch, or digest field is reserved.
    #[error("private-settlement pool activation binding is invalid")]
    InvalidBinding,
    /// The governance revision or activation interval is invalid.
    #[error("private-settlement pool activation lifecycle is invalid")]
    InvalidLifecycle,
    /// A private-note pool cannot be bootstrapped without an origin commitment.
    #[error("private-settlement pool activation has no origin commitments")]
    EmptyInitialCommitments,
    /// The bounded origin commitment set is too large.
    #[error("private-settlement pool activation has {count} origin commitments; maximum is {max}")]
    TooManyInitialCommitments {
        /// Actual commitment count.
        count: usize,
        /// Protocol maximum.
        max: usize,
    },
    /// One origin commitment used the reserved zero value.
    #[error("private-settlement pool activation commitment {index} is zero")]
    ZeroInitialCommitment {
        /// Invalid commitment index.
        index: usize,
    },
    /// Origin commitments are duplicated or not in their canonical order.
    #[error(
        "private-settlement pool activation commitments are not strictly increasing at {index}"
    )]
    NonCanonicalInitialCommitments {
        /// First non-canonical commitment index.
        index: usize,
    },
    /// A supplied restricted record is invalid.
    #[error("private-settlement restricted pool governance is invalid")]
    InvalidRestrictedGovernance,
    /// The public activation is not the exact projection of a restricted record.
    #[error("private-settlement pool activation projection does not match restricted governance")]
    RestrictedProjectionMismatch,
}

/// Structural failure for a public private-settlement policy rotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PrivateSettlementPoolRotationValidationErrorV1 {
    /// The instruction used an unsupported wire version.
    #[error("unsupported private-settlement pool rotation version {actual}")]
    UnsupportedVersion {
        /// Actual version.
        actual: u8,
    },
    /// The public route is universal or has a reserved incarnation.
    #[error("private-settlement pool rotation route is invalid")]
    InvalidRoute,
    /// A required pool, asset, policy, key epoch, or digest field is reserved.
    #[error("private-settlement pool rotation binding is invalid")]
    InvalidBinding,
    /// The replacement governance lifecycle is invalid.
    #[error("private-settlement pool rotation lifecycle is invalid")]
    InvalidLifecycle,
    /// A supplied restricted replacement record is invalid.
    #[error("private-settlement restricted replacement governance is invalid")]
    InvalidRestrictedGovernance,
    /// The public replacement is not the exact projection of the restricted record.
    #[error("private-settlement pool rotation projection does not match restricted governance")]
    RestrictedProjectionMismatch,
}

isi! {
    /// Activate one governed confidential settlement pool using only public commitments.
    ///
    /// The literal asset identifier and asset-binding salt remain in restricted
    /// governance storage. Consensus persists this redacted projection and the
    /// canonical origin commitment set only.
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    pub struct ActivatePrivateSettlementPoolV1 {
        /// Wire version; must be [`ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1`].
        pub version: u8,
        /// Exact dataspace, lane, and active lane incarnation hosting the pool.
        pub route: PrivateSettlementRouteV1,
        /// Stable opaque private-note pool identifier.
        pub pool_id: PrivacyPoolIdV1,
        /// Commitment to the restricted route, pool, literal asset, and salt.
        pub asset_binding_commitment: iroha_crypto::Hash,
        /// Digest of the exact local auditor policy.
        pub audit_policy_digest: iroha_crypto::Hash,
        /// Non-zero signing and encryption key epoch of that policy.
        pub audit_key_epoch: u64,
        /// Governed activation interval and monotonic revision.
        pub lifecycle: PrivateSettlementPoolGovernanceLifecycleV1,
        /// Self-digest of the complete restricted governance record.
        pub governance_digest: iroha_crypto::Hash,
        /// Canonically ordered, non-empty private-note origin commitments.
        pub initial_commitments: Vec<PrivacyCommitmentV1>,
    }
}

impl crate::seal::Instruction for ActivatePrivateSettlementPoolV1 {}

impl ActivatePrivateSettlementPoolV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.private_settlement.activate_pool.v1";

    /// Construct the public activation from one validated restricted governance record.
    ///
    /// # Errors
    ///
    /// Returns a typed error if the restricted record or public origin is invalid.
    pub fn from_restricted(
        governance: &PrivateSettlementPoolGovernanceV1,
        initial_commitments: Vec<PrivacyCommitmentV1>,
    ) -> Result<Self, PrivateSettlementPoolActivationValidationErrorV1> {
        governance.validate().map_err(|_| {
            PrivateSettlementPoolActivationValidationErrorV1::InvalidRestrictedGovernance
        })?;
        let activation = Self {
            version: governance.body.version,
            route: governance.body.route,
            pool_id: governance.body.pool_id,
            asset_binding_commitment: governance.body.asset_binding_commitment,
            audit_policy_digest: governance.body.audit_policy_digest,
            audit_key_epoch: governance.body.audit_key_epoch,
            lifecycle: governance.body.lifecycle,
            governance_digest: governance.governance_digest,
            initial_commitments,
        };
        activation.validate_against_restricted(governance)?;
        Ok(activation)
    }

    /// Validate the complete public, fixed-profile activation shape.
    ///
    /// # Errors
    ///
    /// Returns a typed error for reserved fields, invalid lifecycle bounds, or
    /// a non-canonical origin commitment set.
    pub fn validate(&self) -> Result<(), PrivateSettlementPoolActivationValidationErrorV1> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(
                PrivateSettlementPoolActivationValidationErrorV1::UnsupportedVersion {
                    actual: self.version,
                },
            );
        }
        if self.route.dataspace_id == DataSpaceId::UNIVERSAL
            || self
                .route
                .lane_incarnation
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(PrivateSettlementPoolActivationValidationErrorV1::InvalidRoute);
        }
        if self.pool_id.is_zero()
            || self
                .asset_binding_commitment
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || self
                .audit_policy_digest
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || self.audit_key_epoch == 0
            || self
                .governance_digest
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(PrivateSettlementPoolActivationValidationErrorV1::InvalidBinding);
        }
        if self.lifecycle.governance_revision == 0
            || self.lifecycle.activation_height == 0
            || self
                .lifecycle
                .retirement_height
                .is_some_and(|retirement| retirement <= self.lifecycle.activation_height)
        {
            return Err(PrivateSettlementPoolActivationValidationErrorV1::InvalidLifecycle);
        }
        if self.initial_commitments.is_empty() {
            return Err(PrivateSettlementPoolActivationValidationErrorV1::EmptyInitialCommitments);
        }
        if self.initial_commitments.len() > PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1 {
            return Err(
                PrivateSettlementPoolActivationValidationErrorV1::TooManyInitialCommitments {
                    count: self.initial_commitments.len(),
                    max: PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1,
                },
            );
        }
        let mut previous = None;
        for (index, commitment) in self.initial_commitments.iter().copied().enumerate() {
            if commitment.is_zero() {
                return Err(
                    PrivateSettlementPoolActivationValidationErrorV1::ZeroInitialCommitment {
                        index,
                    },
                );
            }
            if previous.is_some_and(|value| value >= commitment) {
                return Err(
                    PrivateSettlementPoolActivationValidationErrorV1::NonCanonicalInitialCommitments {
                        index,
                    },
                );
            }
            previous = Some(commitment);
        }
        Ok(())
    }

    /// Verify this public projection against one exact restricted record.
    ///
    /// # Errors
    ///
    /// Returns a uniform mismatch without revealing which restricted field differed.
    pub fn validate_against_restricted(
        &self,
        governance: &PrivateSettlementPoolGovernanceV1,
    ) -> Result<(), PrivateSettlementPoolActivationValidationErrorV1> {
        self.validate()?;
        governance.validate().map_err(|_| {
            PrivateSettlementPoolActivationValidationErrorV1::InvalidRestrictedGovernance
        })?;
        if self.version != governance.body.version
            || self.route != governance.body.route
            || self.pool_id != governance.body.pool_id
            || self.asset_binding_commitment != governance.body.asset_binding_commitment
            || self.audit_policy_digest != governance.body.audit_policy_digest
            || self.audit_key_epoch != governance.body.audit_key_epoch
            || self.lifecycle != governance.body.lifecycle
            || self.governance_digest != governance.governance_digest
        {
            return Err(
                PrivateSettlementPoolActivationValidationErrorV1::RestrictedProjectionMismatch,
            );
        }
        Ok(())
    }
}

isi! {
    /// Atomically rotate one live pool to a new auditor policy and key epoch.
    ///
    /// Consensus preserves the pool frontier and replay sets. The exact prior
    /// governance digest prevents stale or concurrent replacements, while the
    /// restricted asset identifier and opening salt remain off the public wire.
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    pub struct RotatePrivateSettlementPoolPolicyV1 {
        /// Wire version; must be [`ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1`].
        pub version: u8,
        /// Exact dataspace, lane, and active lane incarnation hosting the pool.
        pub route: PrivateSettlementRouteV1,
        /// Stable opaque pool whose frontier must be preserved.
        pub pool_id: PrivacyPoolIdV1,
        /// Exact current restricted-governance digest being replaced.
        pub expected_governance_digest: iroha_crypto::Hash,
        /// Immutable salted commitment to the same restricted asset binding.
        pub asset_binding_commitment: iroha_crypto::Hash,
        /// Digest of the replacement auditor policy.
        pub audit_policy_digest: iroha_crypto::Hash,
        /// Strictly newer replacement signing and encryption key epoch.
        pub audit_key_epoch: u64,
        /// Replacement activation interval and next governance revision.
        pub lifecycle: PrivateSettlementPoolGovernanceLifecycleV1,
        /// Self-digest of the complete restricted replacement record.
        pub governance_digest: iroha_crypto::Hash,
    }
}

impl crate::seal::Instruction for RotatePrivateSettlementPoolPolicyV1 {}

impl RotatePrivateSettlementPoolPolicyV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.private_settlement.rotate_pool_policy.v1";

    /// Construct a public rotation from the exact prior digest and restricted replacement.
    ///
    /// # Errors
    ///
    /// Returns a typed error if the restricted record or public projection is invalid.
    pub fn from_restricted(
        expected_governance_digest: iroha_crypto::Hash,
        replacement: &PrivateSettlementPoolGovernanceV1,
    ) -> Result<Self, PrivateSettlementPoolRotationValidationErrorV1> {
        replacement.validate().map_err(|_| {
            PrivateSettlementPoolRotationValidationErrorV1::InvalidRestrictedGovernance
        })?;
        let rotation = Self {
            version: replacement.body.version,
            route: replacement.body.route,
            pool_id: replacement.body.pool_id,
            expected_governance_digest,
            asset_binding_commitment: replacement.body.asset_binding_commitment,
            audit_policy_digest: replacement.body.audit_policy_digest,
            audit_key_epoch: replacement.body.audit_key_epoch,
            lifecycle: replacement.body.lifecycle,
            governance_digest: replacement.governance_digest,
        };
        rotation.validate_against_restricted(replacement)?;
        Ok(rotation)
    }

    /// Validate the public fixed-profile rotation shape.
    ///
    /// # Errors
    ///
    /// Returns a typed error for reserved fields or invalid lifecycle bounds.
    pub fn validate(&self) -> Result<(), PrivateSettlementPoolRotationValidationErrorV1> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(
                PrivateSettlementPoolRotationValidationErrorV1::UnsupportedVersion {
                    actual: self.version,
                },
            );
        }
        if self.route.dataspace_id == DataSpaceId::UNIVERSAL
            || self
                .route
                .lane_incarnation
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(PrivateSettlementPoolRotationValidationErrorV1::InvalidRoute);
        }
        if self.pool_id.is_zero()
            || self
                .expected_governance_digest
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || self
                .asset_binding_commitment
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || self
                .audit_policy_digest
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || self.audit_key_epoch == 0
            || self
                .governance_digest
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || self.expected_governance_digest == self.governance_digest
        {
            return Err(PrivateSettlementPoolRotationValidationErrorV1::InvalidBinding);
        }
        if self.lifecycle.governance_revision == 0
            || self.lifecycle.activation_height == 0
            || self
                .lifecycle
                .retirement_height
                .is_some_and(|retirement| retirement <= self.lifecycle.activation_height)
        {
            return Err(PrivateSettlementPoolRotationValidationErrorV1::InvalidLifecycle);
        }
        Ok(())
    }

    /// Verify this public projection against one exact restricted replacement record.
    ///
    /// # Errors
    ///
    /// Returns a uniform mismatch without identifying any restricted field.
    pub fn validate_against_restricted(
        &self,
        replacement: &PrivateSettlementPoolGovernanceV1,
    ) -> Result<(), PrivateSettlementPoolRotationValidationErrorV1> {
        self.validate()?;
        replacement.validate().map_err(|_| {
            PrivateSettlementPoolRotationValidationErrorV1::InvalidRestrictedGovernance
        })?;
        if self.version != replacement.body.version
            || self.route != replacement.body.route
            || self.pool_id != replacement.body.pool_id
            || self.asset_binding_commitment != replacement.body.asset_binding_commitment
            || self.audit_policy_digest != replacement.body.audit_policy_digest
            || self.audit_key_epoch != replacement.body.audit_key_epoch
            || self.lifecycle != replacement.body.lifecycle
            || self.governance_digest != replacement.governance_digest
        {
            return Err(
                PrivateSettlementPoolRotationValidationErrorV1::RestrictedProjectionMismatch,
            );
        }
        Ok(())
    }
}

isi! {
    /// Publish one sponsor-authorized opaque abort or expiry marker.
    ///
    /// The complete public manifest binds the sponsor, network, expiry, and
    /// bundle identity. Execution derives the compact terminal receipt at the
    /// current global height; no restricted leg material enters this carrier.
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    pub struct AbortAtomicPrivateSettlementV1 {
        /// Exact immutable public manifest for the aborted bundle.
        pub manifest: AtomicPrivateSettlementV1,
        /// Non-sensitive terminal reason class.
        pub reason: PrivateSettlementAbortReasonV1,
    }
}

impl crate::seal::Instruction for AbortAtomicPrivateSettlementV1 {}

impl AbortAtomicPrivateSettlementV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.private_settlement.abort_atomic_bundle.v1";

    /// Construct one public terminal marker carrier.
    #[must_use]
    pub const fn new(
        manifest: AtomicPrivateSettlementV1,
        reason: PrivateSettlementAbortReasonV1,
    ) -> Self {
        Self { manifest, reason }
    }
}

isi! {
    /// Finalize one complete, committee-certified private settlement bundle.
    ///
    /// Consensus accepts this instruction only as the exact direct instruction
    /// of a transaction signed by the manifest sponsor with the identical fee
    /// intent. Core then validates every participant certificate and applies all
    /// opaque state deltas in one ledger state transaction.
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    pub struct FinalizeAtomicPrivateSettlementV1 {
        /// Complete compact certified bundle for every canonical participant leg.
        pub commit_bundle: PrivateSettlementCommitBundleV1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool_activation() -> ActivatePrivateSettlementPoolV1 {
        ActivatePrivateSettlementPoolV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            route: PrivateSettlementRouteV1 {
                dataspace_id: DataSpaceId::new(7),
                lane_id: crate::nexus::LaneId::new(9),
                lane_incarnation: iroha_crypto::Hash::new([0x11]),
            },
            pool_id: PrivacyPoolIdV1::new([0x22; 32]),
            asset_binding_commitment: iroha_crypto::Hash::new([0x33]),
            audit_policy_digest: iroha_crypto::Hash::new([0x44]),
            audit_key_epoch: 3,
            lifecycle: PrivateSettlementPoolGovernanceLifecycleV1 {
                governance_revision: 1,
                activation_height: 10,
                retirement_height: Some(1_000),
            },
            governance_digest: iroha_crypto::Hash::new([0x55]),
            initial_commitments: vec![
                PrivacyCommitmentV1::new([0x61; 32]),
                PrivacyCommitmentV1::new([0x62; 32]),
            ],
        }
    }

    fn pool_rotation() -> RotatePrivateSettlementPoolPolicyV1 {
        let activation = pool_activation();
        RotatePrivateSettlementPoolPolicyV1 {
            version: activation.version,
            route: activation.route,
            pool_id: activation.pool_id,
            expected_governance_digest: activation.governance_digest,
            asset_binding_commitment: activation.asset_binding_commitment,
            audit_policy_digest: iroha_crypto::Hash::new(b"replacement audit policy"),
            audit_key_epoch: activation.audit_key_epoch + 1,
            lifecycle: PrivateSettlementPoolGovernanceLifecycleV1 {
                governance_revision: activation.lifecycle.governance_revision + 1,
                activation_height: 20,
                retirement_height: Some(2_000),
            },
            governance_digest: iroha_crypto::Hash::new(b"replacement governance"),
        }
    }

    #[test]
    fn pool_activation_has_a_stable_registered_wire_id() {
        let registry = crate::isi::registry::default();
        assert!(registry.contains(ActivatePrivateSettlementPoolV1::WIRE_ID));
        assert_eq!(
            registry.wire_id(core::any::type_name::<ActivatePrivateSettlementPoolV1>()),
            Some(ActivatePrivateSettlementPoolV1::WIRE_ID)
        );
    }

    #[test]
    fn pool_activation_rejects_noncanonical_origin_commitments() {
        let activation = pool_activation();
        activation.validate().expect("canonical fixture");

        let mut reordered = activation.clone();
        reordered.initial_commitments.reverse();
        assert_eq!(
            reordered.validate(),
            Err(
                PrivateSettlementPoolActivationValidationErrorV1::NonCanonicalInitialCommitments {
                    index: 1,
                }
            )
        );

        let mut empty = activation;
        empty.initial_commitments.clear();
        assert_eq!(
            empty.validate(),
            Err(PrivateSettlementPoolActivationValidationErrorV1::EmptyInitialCommitments)
        );
    }

    #[test]
    fn pool_activation_instruction_box_roundtrips_canonically() {
        let activation = pool_activation();
        let boxed = InstructionBox::from(activation.clone());
        let bytes = norito::encode_canonical(&boxed).expect("encode activation instruction");
        let decoded: InstructionBox =
            norito::decode_canonical(&bytes).expect("decode activation instruction");
        assert_eq!(
            decoded
                .as_any()
                .downcast_ref::<ActivatePrivateSettlementPoolV1>(),
            Some(&activation)
        );
    }

    #[test]
    fn pool_rotation_has_a_stable_registered_wire_id_and_roundtrips() {
        let registry = crate::isi::registry::default();
        assert!(registry.contains(RotatePrivateSettlementPoolPolicyV1::WIRE_ID));
        assert_eq!(
            registry.wire_id(core::any::type_name::<RotatePrivateSettlementPoolPolicyV1>()),
            Some(RotatePrivateSettlementPoolPolicyV1::WIRE_ID)
        );
        let rotation = pool_rotation();
        rotation.validate().expect("canonical rotation");
        let boxed = InstructionBox::from(rotation.clone());
        let bytes = norito::encode_canonical(&boxed).expect("encode rotation instruction");
        let decoded: InstructionBox =
            norito::decode_canonical(&bytes).expect("decode rotation instruction");
        assert_eq!(
            decoded
                .as_any()
                .downcast_ref::<RotatePrivateSettlementPoolPolicyV1>(),
            Some(&rotation)
        );
    }

    #[test]
    fn finalization_carrier_has_a_stable_registered_wire_id() {
        let registry = crate::isi::registry::default();
        assert!(registry.contains(FinalizeAtomicPrivateSettlementV1::WIRE_ID));
        assert_eq!(
            registry.wire_id(core::any::type_name::<FinalizeAtomicPrivateSettlementV1>()),
            Some(FinalizeAtomicPrivateSettlementV1::WIRE_ID)
        );
    }

    #[test]
    fn abort_carrier_has_a_stable_registered_wire_id() {
        let registry = crate::isi::registry::default();
        assert!(registry.contains(AbortAtomicPrivateSettlementV1::WIRE_ID));
        assert_eq!(
            registry.wire_id(core::any::type_name::<AbortAtomicPrivateSettlementV1>()),
            Some(AbortAtomicPrivateSettlementV1::WIRE_ID)
        );
    }
}

impl crate::seal::Instruction for FinalizeAtomicPrivateSettlementV1 {}

impl FinalizeAtomicPrivateSettlementV1 {
    /// Canonical first-release Norito instruction identifier.
    pub const WIRE_ID: &'static str = "iroha.private_settlement.finalize_atomic_bundle.v1";

    /// Construct a global finalization carrier.
    #[must_use]
    pub const fn new(commit_bundle: PrivateSettlementCommitBundleV1) -> Self {
        Self { commit_bundle }
    }
}
