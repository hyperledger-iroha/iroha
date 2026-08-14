//! Canonical self-digesting Exact12 public capability manifest.
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use super::{
    PRIVACY_EXACT12_CAPABILITY_MANIFEST_DIGEST_DOMAIN_V1, PrivacyCapabilityRowV1,
    PrivacyCapabilityRowValidationErrorV1, PrivacyCapabilitySnapshotV1,
    PrivacyCapabilitySnapshotValidationErrorV1, PrivacyCompiledProfileResultV1,
    PrivacyCompiledProfileUnavailableReasonV1, PrivacyConsensusPolicyV1,
    PrivacyExact12CapabilityManifestDigestV1, PrivacyPolicyValidationErrorV1,
    PrivacyProtocolActivationRecordV1, PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1,
};
/// Exact public Exact12 capability-manifest wire version.
pub const PRIVACY_EXACT12_CAPABILITY_MANIFEST_VERSION_V1: u32 = 1;
/// Canonical public operation schema selected by one retained protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "operation_schema", content = "value", deny_unknown_fields)
)]
pub enum PrivacyOperationSchemaV1 {
    /// ZK-ACE authorization action.
    #[cfg_attr(feature = "json", norito(rename = "zk_ace_authorization_action_v1"))]
    ZkAceAuthorizationActionV1,
    /// Anonymous PGC payment action.
    #[cfg_attr(feature = "json", norito(rename = "anonymous_pgc_payment_action_v1"))]
    AnonymousPgcPaymentActionV1,
    /// `VeRange` range-proof component.
    #[cfg_attr(feature = "json", norito(rename = "verange_range_proof_v1"))]
    VeRangeRangeProofV1,
    /// ZK-AMS admission and provisioning action.
    #[cfg_attr(
        feature = "json",
        norito(rename = "zk_ams_admission_and_provisioning_v1")
    )]
    ZkAmsAdmissionAndProvisioningV1,
    /// Vega credential presentation action.
    #[cfg_attr(feature = "json", norito(rename = "vega_credential_presentation_v1"))]
    VegaCredentialPresentationV1,
    /// ZK-X509 identity presentation action.
    #[cfg_attr(feature = "json", norito(rename = "zk_x509_identity_presentation_v1"))]
    ZkX509IdentityPresentationV1,
    /// Revised Jindo polynomial-evaluation component.
    #[cfg_attr(feature = "json", norito(rename = "jindo_polynomial_evaluation_v1"))]
    JindoPolynomialEvaluationV1,
    /// Bootle/Lantern credential presentation action.
    #[cfg_attr(
        feature = "json",
        norito(rename = "bootle_lantern_credential_presentation_v1")
    )]
    BootleLanternCredentialPresentationV1,
    /// Orchard private-note action.
    #[cfg_attr(feature = "json", norito(rename = "orchard_note_action_v1"))]
    OrchardNoteActionV1,
    /// FCMP++ membership payment action.
    #[cfg_attr(feature = "json", norito(rename = "fcmp_membership_payment_v1"))]
    FcmpMembershipPaymentV1,
    /// IVM private-note action.
    #[cfg_attr(feature = "json", norito(rename = "ivm_private_note_action_v1"))]
    IvmPrivateNoteActionV1,
    /// Post-quantum MASP private-note action.
    #[cfg_attr(feature = "json", norito(rename = "pq_masp_note_action_v1"))]
    PqMaspNoteActionV1,
}
impl PrivacyOperationSchemaV1 {
    /// Return the sole public string spelling of this operation schema.
    #[must_use]
    pub const fn canonical_label(self) -> &'static str {
        match self {
            Self::ZkAceAuthorizationActionV1 => "zk_ace_authorization_action_v1",
            Self::AnonymousPgcPaymentActionV1 => "anonymous_pgc_payment_action_v1",
            Self::VeRangeRangeProofV1 => "verange_range_proof_v1",
            Self::ZkAmsAdmissionAndProvisioningV1 => "zk_ams_admission_and_provisioning_v1",
            Self::VegaCredentialPresentationV1 => "vega_credential_presentation_v1",
            Self::ZkX509IdentityPresentationV1 => "zk_x509_identity_presentation_v1",
            Self::JindoPolynomialEvaluationV1 => "jindo_polynomial_evaluation_v1",
            Self::BootleLanternCredentialPresentationV1 => {
                "bootle_lantern_credential_presentation_v1"
            }
            Self::OrchardNoteActionV1 => "orchard_note_action_v1",
            Self::FcmpMembershipPaymentV1 => "fcmp_membership_payment_v1",
            Self::IvmPrivateNoteActionV1 => "ivm_private_note_action_v1",
            Self::PqMaspNoteActionV1 => "pq_masp_note_action_v1",
        }
    }
}
/// Closed execution classification for a retained public privacy operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "execution_mode", content = "value", deny_unknown_fields)
)]
pub enum PrivacyExecutionModeV1 {
    /// Authorization action.
    #[cfg_attr(feature = "json", norito(rename = "authorization_action"))]
    AuthorizationAction,
    /// Payment action.
    #[cfg_attr(feature = "json", norito(rename = "payment_action"))]
    PaymentAction,
    /// Standalone proof component.
    #[cfg_attr(feature = "json", norito(rename = "component"))]
    Component,
    /// Admission or provisioning action.
    #[cfg_attr(feature = "json", norito(rename = "admission_action"))]
    AdmissionAction,
    /// Credential or identity presentation action.
    #[cfg_attr(feature = "json", norito(rename = "presentation_action"))]
    PresentationAction,
    /// Private-note action.
    #[cfg_attr(feature = "json", norito(rename = "note_action"))]
    NoteAction,
}
impl PrivacyExecutionModeV1 {
    /// Return the sole public string spelling of this execution mode.
    #[must_use]
    pub const fn canonical_label(self) -> &'static str {
        match self {
            Self::AuthorizationAction => "authorization_action",
            Self::PaymentAction => "payment_action",
            Self::Component => "component",
            Self::AdmissionAction => "admission_action",
            Self::PresentationAction => "presentation_action",
            Self::NoteAction => "note_action",
        }
    }
}
/// Exact feature bitmap exposed by a retained public privacy operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[repr(transparent)]
#[norito(decode_from_slice)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PrivacyFeatureMaskV1(
    /// Exact first-release feature bits.
    pub u8,
);
impl PrivacyFeatureMaskV1 {
    /// Hidden-amount feature bit.
    pub const HIDE_AMOUNT: u8 = 1;
    /// Hidden-sender feature bit.
    pub const HIDE_SENDER: u8 = 1 << 1;
    /// Hidden-receiver feature bit.
    pub const HIDE_RECEIVER: u8 = 1 << 2;
    /// Hidden-asset-type feature bit.
    pub const HIDE_ASSET_TYPE: u8 = 1 << 3;
    /// Post-quantum-operation feature bit.
    pub const POST_QUANTUM: u8 = 1 << 4;
    /// Construct one exact feature bitmap.
    #[must_use]
    pub const fn new(bits: u8) -> Self {
        Self(bits)
    }
    /// Return the raw feature bits.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }
}
/// Evidence-derived local readiness carried by a committed capability row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "readiness", content = "detail", deny_unknown_fields)
)]
pub enum PrivacyCapabilityReadinessV1 {
    /// The retained native profile passed its compiled readiness gates.
    #[cfg_attr(feature = "json", norito(rename = "available"))]
    Available,
    /// The revised Jindo profile is executable but retains an explicit limitation.
    #[cfg_attr(feature = "json", norito(rename = "available-experimental"))]
    AvailableExperimental,
    /// The native profile remains fail-closed for the exact typed reason.
    #[cfg_attr(feature = "json", norito(rename = "unavailable"))]
    Unavailable(PrivacyCompiledProfileUnavailableReasonV1),
}
/// Projection of committed governance lifecycle for one capability row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "activation_state", content = "detail", deny_unknown_fields)
)]
pub enum PrivacyCapabilityActivationStateV1 {
    /// No committed activation record exists.
    #[cfg_attr(feature = "json", norito(rename = "not-registered"))]
    NotRegistered,
    /// Governance committed a future activation.
    #[cfg_attr(feature = "json", norito(rename = "proposed"))]
    Proposed,
    /// Governance currently admits the protocol.
    #[cfg_attr(feature = "json", norito(rename = "active"))]
    Active,
    /// Governance temporarily rejects the protocol.
    #[cfg_attr(feature = "json", norito(rename = "suspended"))]
    Suspended,
    /// Governance permanently retired the protocol.
    #[cfg_attr(feature = "json", norito(rename = "retired"))]
    Retired,
}
/// Explicit limitation retained by a public capability row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "limitation", content = "detail", deny_unknown_fields)
)]
pub enum PrivacyCapabilityLimitationV1 {
    /// Revised Jindo has no distribution-wide knowledge-soundness certificate.
    #[cfg_attr(
        feature = "json",
        norito(rename = "missing-distribution-wide-knowledge-soundness-evidence")
    )]
    MissingDistributionWideKnowledgeSoundnessEvidence,
}
impl PrivacyProtocolIdV1 {
    /// Canonical public operation schema for this retained protocol.
    #[must_use]
    pub const fn expected_operation_schema(self) -> PrivacyOperationSchemaV1 {
        match self {
            Self::ZkAcePqAuthorizationV0 => PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1,
            Self::AnonymousPgcKOutOfNV1 => PrivacyOperationSchemaV1::AnonymousPgcPaymentActionV1,
            Self::VeRangeTransparentRangeV1 => PrivacyOperationSchemaV1::VeRangeRangeProofV1,
            Self::IrohaZkAmsV1 => PrivacyOperationSchemaV1::ZkAmsAdmissionAndProvisioningV1,
            Self::VegaExistingCredentialZkV0 => {
                PrivacyOperationSchemaV1::VegaCredentialPresentationV1
            }
            Self::IrohaZkX509StarkP256V0 => PrivacyOperationSchemaV1::ZkX509IdentityPresentationV1,
            Self::IrohaJindoPolynomialCommitmentV0 => {
                PrivacyOperationSchemaV1::JindoPolynomialEvaluationV1
            }
            Self::IrohaBootleLanternAnoncredV1 => {
                PrivacyOperationSchemaV1::BootleLanternCredentialPresentationV1
            }
            Self::OrchardHalo2ActionsV1 => PrivacyOperationSchemaV1::OrchardNoteActionV1,
            Self::MoneroFcmpPlusPlusV1 => PrivacyOperationSchemaV1::FcmpMembershipPaymentV1,
            Self::IrohaIvmPrivateNoteStarkV1 => PrivacyOperationSchemaV1::IvmPrivateNoteActionV1,
            Self::PqMaspStarkV0 => PrivacyOperationSchemaV1::PqMaspNoteActionV1,
        }
    }
    /// Canonical execution classification for this retained protocol.
    #[must_use]
    pub const fn expected_execution_mode(self) -> PrivacyExecutionModeV1 {
        match self {
            Self::ZkAcePqAuthorizationV0 => PrivacyExecutionModeV1::AuthorizationAction,
            Self::AnonymousPgcKOutOfNV1 | Self::MoneroFcmpPlusPlusV1 => {
                PrivacyExecutionModeV1::PaymentAction
            }
            Self::VeRangeTransparentRangeV1 | Self::IrohaJindoPolynomialCommitmentV0 => {
                PrivacyExecutionModeV1::Component
            }
            Self::IrohaZkAmsV1 => PrivacyExecutionModeV1::AdmissionAction,
            Self::VegaExistingCredentialZkV0
            | Self::IrohaZkX509StarkP256V0
            | Self::IrohaBootleLanternAnoncredV1 => PrivacyExecutionModeV1::PresentationAction,
            Self::OrchardHalo2ActionsV1
            | Self::IrohaIvmPrivateNoteStarkV1
            | Self::PqMaspStarkV0 => PrivacyExecutionModeV1::NoteAction,
        }
    }
    /// Exact feature mask for this retained protocol.
    #[must_use]
    pub const fn expected_feature_mask(self) -> PrivacyFeatureMaskV1 {
        let bits = match self {
            Self::ZkAcePqAuthorizationV0 | Self::IrohaJindoPolynomialCommitmentV0 => 0,
            Self::AnonymousPgcKOutOfNV1 => {
                PrivacyFeatureMaskV1::HIDE_SENDER | PrivacyFeatureMaskV1::HIDE_RECEIVER
            }
            Self::VeRangeTransparentRangeV1 => PrivacyFeatureMaskV1::HIDE_AMOUNT,
            Self::IrohaZkAmsV1
            | Self::VegaExistingCredentialZkV0
            | Self::IrohaZkX509StarkP256V0
            | Self::IrohaBootleLanternAnoncredV1
            | Self::MoneroFcmpPlusPlusV1 => PrivacyFeatureMaskV1::HIDE_SENDER,
            Self::OrchardHalo2ActionsV1 | Self::IrohaIvmPrivateNoteStarkV1 => {
                PrivacyFeatureMaskV1::HIDE_AMOUNT
                    | PrivacyFeatureMaskV1::HIDE_SENDER
                    | PrivacyFeatureMaskV1::HIDE_RECEIVER
            }
            Self::PqMaspStarkV0 => {
                PrivacyFeatureMaskV1::HIDE_AMOUNT
                    | PrivacyFeatureMaskV1::HIDE_SENDER
                    | PrivacyFeatureMaskV1::HIDE_RECEIVER
                    | PrivacyFeatureMaskV1::HIDE_ASSET_TYPE
                    | PrivacyFeatureMaskV1::POST_QUANTUM
            }
        };
        PrivacyFeatureMaskV1::new(bits)
    }
    /// Explicit limitation required for this retained protocol, if any.
    #[must_use]
    pub const fn expected_capability_limitation(self) -> Option<PrivacyCapabilityLimitationV1> {
        match self {
            Self::IrohaJindoPolynomialCommitmentV0 => Some(
                PrivacyCapabilityLimitationV1::MissingDistributionWideKnowledgeSoundnessEvidence,
            ),
            _ => None,
        }
    }
}
/// One row of the canonical public Exact12 capability manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyExact12CapabilityRowV1 {
    /// Closed protocol identity.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Canonical public operation schema.
    pub operation_schema: PrivacyOperationSchemaV1,
    /// Closed execution classification.
    pub execution_mode: PrivacyExecutionModeV1,
    /// Exact amount/sender/receiver/asset/PQ feature bits.
    pub privacy_feature_mask: PrivacyFeatureMaskV1,
    /// Exact compiled profile and all profile/schema bindings, or its typed failure.
    pub compiled_profile: PrivacyCompiledProfileResultV1,
    /// Evidence-derived compiled readiness.
    pub readiness: PrivacyCapabilityReadinessV1,
    /// Projection of the committed governance lifecycle.
    pub activation_state: PrivacyCapabilityActivationStateV1,
    /// Full committed governance record, if registered.
    pub activation: Option<PrivacyProtocolActivationRecordV1>,
    /// Explicit retained limitation; revised Jindo always carries its missing evidence.
    pub limitation: Option<PrivacyCapabilityLimitationV1>,
}
impl PrivacyExact12CapabilityRowV1 {
    fn from_committed_snapshot_row(row: PrivacyCapabilityRowV1) -> Self {
        let readiness = match row.compiled_profile {
            PrivacyCompiledProfileResultV1::Available(_)
                if row.protocol_id == PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0 =>
            {
                PrivacyCapabilityReadinessV1::AvailableExperimental
            }
            PrivacyCompiledProfileResultV1::Available(_) => PrivacyCapabilityReadinessV1::Available,
            PrivacyCompiledProfileResultV1::Unavailable(reason) => {
                PrivacyCapabilityReadinessV1::Unavailable(reason)
            }
        };
        let activation_state = match row.activation.map(|activation| activation.lifecycle) {
            None => PrivacyCapabilityActivationStateV1::NotRegistered,
            Some(PrivacyProtocolLifecycleV1::Proposed(_)) => {
                PrivacyCapabilityActivationStateV1::Proposed
            }
            Some(PrivacyProtocolLifecycleV1::Active(_)) => {
                PrivacyCapabilityActivationStateV1::Active
            }
            Some(PrivacyProtocolLifecycleV1::Suspended(_)) => {
                PrivacyCapabilityActivationStateV1::Suspended
            }
            Some(PrivacyProtocolLifecycleV1::Retired(_)) => {
                PrivacyCapabilityActivationStateV1::Retired
            }
        };
        Self {
            protocol_id: row.protocol_id,
            operation_schema: row.protocol_id.expected_operation_schema(),
            execution_mode: row.protocol_id.expected_execution_mode(),
            privacy_feature_mask: row.protocol_id.expected_feature_mask(),
            compiled_profile: row.compiled_profile,
            readiness,
            activation_state,
            activation: row.activation,
            limitation: row.protocol_id.expected_capability_limitation(),
        }
    }
    /// Return whether this committed row is executable and actively admitted.
    ///
    /// This value cannot be derived from a local compiled-profile catalog: it
    /// additionally requires the `Active` lifecycle copied from committed state.
    /// The caller must obtain the enclosing manifest from authenticated Torii
    /// state or a signed candidate receipt; the manifest digest is content
    /// identity and does not by itself authenticate the producer.
    #[must_use]
    pub const fn is_network_available(&self) -> bool {
        matches!(
            self.readiness,
            PrivacyCapabilityReadinessV1::Available
                | PrivacyCapabilityReadinessV1::AvailableExperimental
        ) && matches!(
            self.activation_state,
            PrivacyCapabilityActivationStateV1::Active
        )
    }
    fn validate_at_committed_height(
        &self,
        committed_height: u64,
    ) -> Result<(), PrivacyExact12CapabilityRowValidationErrorV1> {
        PrivacyCapabilityRowV1 {
            protocol_id: self.protocol_id,
            compiled_profile: self.compiled_profile,
            activation: self.activation,
        }
        .validate_at_committed_height(committed_height)
        .map_err(PrivacyExact12CapabilityRowValidationErrorV1::CapabilityRow)?;
        let expected_operation_schema = self.protocol_id.expected_operation_schema();
        if self.operation_schema != expected_operation_schema {
            return Err(
                PrivacyExact12CapabilityRowValidationErrorV1::OperationSchemaMismatch {
                    expected: expected_operation_schema,
                    actual: self.operation_schema,
                },
            );
        }
        let expected_execution_mode = self.protocol_id.expected_execution_mode();
        if self.execution_mode != expected_execution_mode {
            return Err(
                PrivacyExact12CapabilityRowValidationErrorV1::ExecutionModeMismatch {
                    expected: expected_execution_mode,
                    actual: self.execution_mode,
                },
            );
        }
        let expected_feature_mask = self.protocol_id.expected_feature_mask();
        if self.privacy_feature_mask != expected_feature_mask {
            return Err(
                PrivacyExact12CapabilityRowValidationErrorV1::FeatureMaskMismatch {
                    expected: expected_feature_mask,
                    actual: self.privacy_feature_mask,
                },
            );
        }
        let projected = Self::from_committed_snapshot_row(PrivacyCapabilityRowV1 {
            protocol_id: self.protocol_id,
            compiled_profile: self.compiled_profile,
            activation: self.activation,
        });
        if self.readiness != projected.readiness {
            return Err(
                PrivacyExact12CapabilityRowValidationErrorV1::ReadinessMismatch {
                    expected: projected.readiness,
                    actual: self.readiness,
                },
            );
        }
        if self.activation_state != projected.activation_state {
            return Err(
                PrivacyExact12CapabilityRowValidationErrorV1::ActivationStateMismatch {
                    expected: projected.activation_state,
                    actual: self.activation_state,
                },
            );
        }
        if self.limitation != projected.limitation {
            return Err(
                PrivacyExact12CapabilityRowValidationErrorV1::LimitationMismatch {
                    expected: projected.limitation,
                    actual: self.limitation,
                },
            );
        }
        Ok(())
    }
}
/// Validation failure for one Exact12 capability-manifest row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyExact12CapabilityRowValidationErrorV1 {
    /// The embedded compiled profile or activation is invalid.
    #[error("privacy Exact12 capability row is invalid: {0}")]
    CapabilityRow(PrivacyCapabilityRowValidationErrorV1),
    /// Operation schema differs from the closed protocol mapping.
    #[error("privacy operation schema {actual:?} differs from required {expected:?}")]
    OperationSchemaMismatch {
        /// Required schema.
        expected: PrivacyOperationSchemaV1,
        /// Rejected schema.
        actual: PrivacyOperationSchemaV1,
    },
    /// Execution mode differs from the closed protocol mapping.
    #[error("privacy execution mode {actual:?} differs from required {expected:?}")]
    ExecutionModeMismatch {
        /// Required mode.
        expected: PrivacyExecutionModeV1,
        /// Rejected mode.
        actual: PrivacyExecutionModeV1,
    },
    /// Feature mask differs from the closed protocol mapping.
    #[error("privacy feature mask {actual:?} differs from required {expected:?}")]
    FeatureMaskMismatch {
        /// Required mask.
        expected: PrivacyFeatureMaskV1,
        /// Rejected mask.
        actual: PrivacyFeatureMaskV1,
    },
    /// Readiness was not derived from the exact compiled-profile result.
    #[error("privacy readiness {actual:?} differs from evidence-derived {expected:?}")]
    ReadinessMismatch {
        /// Evidence-derived readiness.
        expected: PrivacyCapabilityReadinessV1,
        /// Rejected readiness.
        actual: PrivacyCapabilityReadinessV1,
    },
    /// Activation-state projection differs from the committed lifecycle.
    #[error("privacy activation state {actual:?} differs from committed {expected:?}")]
    ActivationStateMismatch {
        /// Committed lifecycle projection.
        expected: PrivacyCapabilityActivationStateV1,
        /// Rejected projection.
        actual: PrivacyCapabilityActivationStateV1,
    },
    /// Limitation differs from the closed retained-protocol mapping.
    #[error("privacy limitation {actual:?} differs from required {expected:?}")]
    LimitationMismatch {
        /// Required limitation.
        expected: Option<PrivacyCapabilityLimitationV1>,
        /// Rejected limitation.
        actual: Option<PrivacyCapabilityLimitationV1>,
    },
}
/// Canonical self-digesting v1 Exact12 public capability manifest.
///
/// The manifest is projected only from one validated committed snapshot. Its
/// ordered rows bind public operation metadata, local native evidence, and the
/// exact committed activation state without treating a local catalog as
/// network authority. The digest detects content drift but is not a signature
/// or proof that an untrusted producer read committed state.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[norito(schema_name = "iroha.privacy.exact12-capability-manifest.v1")]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyExact12CapabilityManifestV1 {
    /// Exact manifest schema version.
    pub version: u32,
    /// Height of the immutable committed state view used for this manifest.
    pub committed_height: u64,
    /// Authoritative singleton chain-wide privacy policy.
    pub consensus_policy: PrivacyConsensusPolicyV1,
    /// Exactly twelve capability rows in canonical discriminant order.
    pub protocols: Vec<PrivacyExact12CapabilityRowV1>,
    /// SHA-256 self-digest with this field normalized to zero.
    pub manifest_digest: PrivacyExact12CapabilityManifestDigestV1,
}
impl PrivacyCapabilitySnapshotV1 {
    /// Project this validated committed snapshot into the canonical Exact12 manifest.
    ///
    /// # Errors
    ///
    /// Rejects an invalid source snapshot, canonical encoding failure, or any
    /// failure of the derived manifest invariants.
    pub fn exact12_capability_manifest_v1(
        &self,
    ) -> Result<PrivacyExact12CapabilityManifestV1, PrivacyExact12CapabilityManifestBuildErrorV1>
    {
        self.validate()
            .map_err(PrivacyExact12CapabilityManifestBuildErrorV1::Snapshot)?;
        let mut manifest = PrivacyExact12CapabilityManifestV1 {
            version: PRIVACY_EXACT12_CAPABILITY_MANIFEST_VERSION_V1,
            committed_height: self.committed_height,
            consensus_policy: self.consensus_policy,
            protocols: self
                .protocols
                .iter()
                .copied()
                .map(PrivacyExact12CapabilityRowV1::from_committed_snapshot_row)
                .collect(),
            manifest_digest: PrivacyExact12CapabilityManifestDigestV1::new([0; 32]),
        };
        manifest.manifest_digest = manifest
            .computed_manifest_digest()
            .map_err(PrivacyExact12CapabilityManifestBuildErrorV1::CanonicalEncoding)?;
        manifest
            .validate()
            .map_err(PrivacyExact12CapabilityManifestBuildErrorV1::Manifest)?;
        Ok(manifest)
    }
}
impl PrivacyExact12CapabilityManifestV1 {
    /// Compute the manifest digest with `manifest_digest` normalized to zero.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if canonical encoding unexpectedly fails.
    pub fn computed_manifest_digest(
        &self,
    ) -> Result<PrivacyExact12CapabilityManifestDigestV1, norito::Error> {
        let mut normalized = self.clone();
        normalized.manifest_digest = PrivacyExact12CapabilityManifestDigestV1::new([0; 32]);
        let encoded = norito::encode_canonical(&normalized)?;
        let mut hasher = Sha256::new();
        hasher.update(PRIVACY_EXACT12_CAPABILITY_MANIFEST_DIGEST_DOMAIN_V1);
        hasher.update(
            u64::try_from(encoded.len())
                .expect("Norito output length fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(encoded);
        Ok(PrivacyExact12CapabilityManifestDigestV1::new(
            hasher.finalize().into(),
        ))
    }
    /// Return the one canonical validated manifest encoding.
    ///
    /// # Errors
    ///
    /// Rejects an invalid manifest or canonical encoding failure.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PrivacyExact12CapabilityManifestBuildErrorV1> {
        self.validate()
            .map_err(PrivacyExact12CapabilityManifestBuildErrorV1::Manifest)?;
        norito::encode_canonical(self)
            .map_err(PrivacyExact12CapabilityManifestBuildErrorV1::CanonicalEncoding)
    }
    /// Validate the complete manifest, including its self-digest.
    ///
    /// # Errors
    ///
    /// Rejects version, policy, ordering, row-projection, readiness,
    /// activation, limitation, or digest drift.
    pub fn validate(&self) -> Result<(), PrivacyExact12CapabilityManifestValidationErrorV1> {
        if self.version != PRIVACY_EXACT12_CAPABILITY_MANIFEST_VERSION_V1 {
            return Err(PrivacyExact12CapabilityManifestValidationErrorV1::Version {
                expected: PRIVACY_EXACT12_CAPABILITY_MANIFEST_VERSION_V1,
                actual: self.version,
            });
        }
        self.consensus_policy
            .validate_at_committed_height(self.committed_height)
            .map_err(PrivacyExact12CapabilityManifestValidationErrorV1::ConsensusPolicy)?;
        if self.protocols.len() != PrivacyProtocolIdV1::COUNT {
            return Err(
                PrivacyExact12CapabilityManifestValidationErrorV1::ProtocolCount {
                    expected: PrivacyProtocolIdV1::COUNT,
                    actual: self.protocols.len(),
                },
            );
        }
        for (index, (row, expected)) in self
            .protocols
            .iter()
            .zip(PrivacyProtocolIdV1::ALL)
            .enumerate()
        {
            if row.protocol_id != expected {
                return Err(
                    PrivacyExact12CapabilityManifestValidationErrorV1::ProtocolOrder {
                        index,
                        expected,
                        actual: row.protocol_id,
                    },
                );
            }
            row.validate_at_committed_height(self.committed_height)
                .map_err(|source| {
                    PrivacyExact12CapabilityManifestValidationErrorV1::ProtocolRow {
                        protocol_id: expected,
                        source,
                    }
                })?;
        }
        if self.manifest_digest.is_zero() {
            return Err(PrivacyExact12CapabilityManifestValidationErrorV1::ZeroManifestDigest);
        }
        let expected = self.computed_manifest_digest().map_err(|_| {
            PrivacyExact12CapabilityManifestValidationErrorV1::ManifestDigestEncoding
        })?;
        if self.manifest_digest != expected {
            return Err(
                PrivacyExact12CapabilityManifestValidationErrorV1::ManifestDigestMismatch {
                    expected,
                    actual: self.manifest_digest,
                },
            );
        }
        Ok(())
    }
}
/// Failure projecting a committed snapshot or encoding a validated manifest.
#[derive(Debug, Error)]
pub enum PrivacyExact12CapabilityManifestBuildErrorV1 {
    /// Source committed snapshot was invalid.
    #[error("privacy capability source snapshot is invalid: {0}")]
    Snapshot(PrivacyCapabilitySnapshotValidationErrorV1),
    /// Canonical Norito encoding failed.
    #[error("privacy Exact12 capability manifest encoding failed: {0}")]
    CanonicalEncoding(norito::Error),
    /// Derived manifest failed its closed invariants.
    #[error("privacy Exact12 capability manifest is invalid: {0}")]
    Manifest(PrivacyExact12CapabilityManifestValidationErrorV1),
}
/// Validation failure for a canonical Exact12 capability manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyExact12CapabilityManifestValidationErrorV1 {
    /// Manifest wire version differs from v1.
    #[error("privacy Exact12 capability manifest version {actual} differs from {expected}")]
    Version {
        /// Required version.
        expected: u32,
        /// Rejected version.
        actual: u32,
    },
    /// Singleton consensus policy is invalid at the committed height.
    #[error("privacy Exact12 capability consensus policy is invalid: {0}")]
    ConsensusPolicy(PrivacyPolicyValidationErrorV1),
    /// Manifest row count differs from Exact12.
    #[error("privacy Exact12 capability manifest has {actual} rows; expected {expected}")]
    ProtocolCount {
        /// Required row count.
        expected: usize,
        /// Rejected row count.
        actual: usize,
    },
    /// A row is missing, duplicated, or reordered.
    #[error("privacy Exact12 capability row {index} is {actual:?}; expected {expected:?}")]
    ProtocolOrder {
        /// Zero-based row index.
        index: usize,
        /// Required protocol.
        expected: PrivacyProtocolIdV1,
        /// Rejected protocol.
        actual: PrivacyProtocolIdV1,
    },
    /// One canonical row is invalid.
    #[error("privacy Exact12 capability row {protocol_id:?} is invalid: {source}")]
    ProtocolRow {
        /// Protocol selected by canonical order.
        protocol_id: PrivacyProtocolIdV1,
        /// Exact row failure.
        source: PrivacyExact12CapabilityRowValidationErrorV1,
    },
    /// The manifest digest is the reserved zero placeholder.
    #[error("privacy Exact12 capability manifest digest must be non-zero")]
    ZeroManifestDigest,
    /// Canonical encoding failed while recomputing the digest.
    #[error("privacy Exact12 capability manifest digest encoding failed")]
    ManifestDigestEncoding,
    /// Embedded self-digest differs from the canonical manifest contents.
    #[error("privacy Exact12 capability manifest digest {actual:?} differs from {expected:?}")]
    ManifestDigestMismatch {
        /// Recomputed digest.
        expected: PrivacyExact12CapabilityManifestDigestV1,
        /// Rejected embedded digest.
        actual: PrivacyExact12CapabilityManifestDigestV1,
    },
}
