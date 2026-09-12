//! Governance status wire records.
use iroha_schema::IntoSchema;
use norito::{
    core::DecodeFromSlice,
    derive::{NoritoDeserialize, NoritoSerialize},
};

/// Governance-related telemetry snapshot embedded into [`Status`].
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_torii_shared::status::governance::GovernanceStatus",
    frame = "iroha_telemetry::metrics::GovernanceStatus"
)]
pub struct GovernanceStatus {
    /// Current proposal counts grouped by status.
    pub proposals: GovernanceProposalCounters,
    /// Protected-namespace enforcement counters.
    pub protected_namespace: GovernanceProtectedNamespaceCounters,
    /// Manifest admission outcomes observed at queue ingress.
    pub manifest_admission: GovernanceManifestAdmissionCounters,
    /// Manifest quorum enforcement counters.
    pub manifest_quorum: GovernanceManifestQuorumCounters,
    /// Recent manifest activations (most recent first).
    pub recent_manifest_activations: Vec<GovernanceManifestActivation>,
    /// Total lanes that remain sealed awaiting governance manifests.
    pub sealed_lanes_total: u32,
    /// Aliases of lanes that remain sealed awaiting governance manifests.
    pub sealed_lane_aliases: Vec<String>,
    /// Total registered citizens with an active bond.
    pub citizens_total: u64,
}
/// Counts of governance proposals per status.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_torii_shared::status::governance::GovernanceProposalCounters",
    frame = "iroha_telemetry::metrics::GovernanceProposalCounters"
)]
pub struct GovernanceProposalCounters {
    /// Proposals whose latest attempt is active or certified.
    pub proposed: u64,
    /// Proposals whose latest attempt was rejected.
    pub rejected: u64,
    /// Proposals that completed enactment.
    pub enacted: u64,
    /// Proposals whose certified compare-and-set predecessor was superseded.
    pub superseded: u64,
    /// Proposals whose certified effect failed atomically at execution.
    pub execution_failed: u64,
}
/// Counters tracking protected-namespace admission decisions.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_torii_shared::status::governance::GovernanceProtectedNamespaceCounters",
    frame = "iroha_telemetry::metrics::GovernanceProtectedNamespaceCounters"
)]
pub struct GovernanceProtectedNamespaceCounters {
    /// Total number of protected-namespace admission checks.
    pub total_checks: u64,
    /// Checks that passed and were allowed.
    pub allowed: u64,
    /// Checks that were rejected at admission time.
    pub rejected: u64,
}
/// Counters tracking manifest admission decisions (pre-quorum/protection breakdown).
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_torii_shared::status::governance::GovernanceManifestAdmissionCounters",
    frame = "iroha_telemetry::metrics::GovernanceManifestAdmissionCounters"
)]
pub struct GovernanceManifestAdmissionCounters {
    /// Total number of manifest admission checks.
    pub total_checks: u64,
    /// Admissions that succeeded.
    pub allowed: u64,
    /// Rejections due to missing manifest data.
    pub missing_manifest: u64,
    /// Rejections because the authority was not a manifest validator.
    pub non_validator_authority: u64,
    /// Rejections triggered by quorum enforcement.
    pub quorum_rejected: u64,
    /// Rejections triggered by protected-namespace policies.
    pub protected_namespace_rejected: u64,
    /// Rejections triggered by runtime hook policies.
    pub runtime_hook_rejected: u64,
}
/// Counters tracking manifest quorum enforcement.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_torii_shared::status::governance::GovernanceManifestQuorumCounters",
    frame = "iroha_telemetry::metrics::GovernanceManifestQuorumCounters"
)]
pub struct GovernanceManifestQuorumCounters {
    /// Total number of quorum evaluations.
    pub total_checks: u64,
    /// Evaluations that satisfied the manifest quorum.
    pub satisfied: u64,
    /// Evaluations rejected due to insufficient approvals.
    pub rejected: u64,
}
/// Record of a manifest activation produced by governance enactment.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_torii_shared::status::governance::GovernanceManifestActivation",
    frame = "iroha_telemetry::metrics::GovernanceManifestActivation"
)]
pub struct GovernanceManifestActivation {
    /// Canonical contract address whose manifest was activated.
    pub contract_address: String,
    /// Hex-encoded code hash pinned by the activation.
    pub code_hash_hex: String,
    /// Optional ABI hash associated with the activation.
    pub abi_hash_hex: Option<String>,
    /// Block height at which the activation was committed.
    pub height: u64,
    /// Wall-clock timestamp in milliseconds when the activation was recorded.
    pub activated_at_ms: u64,
}

impl norito::core::SerializePayload for GovernanceStatus {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.proposals,
            self.protected_namespace,
            self.manifest_admission,
            self.manifest_quorum,
            self.recent_manifest_activations.clone(),
            self.sealed_lanes_total,
            self.sealed_lane_aliases.clone(),
            self.citizens_total,
        );
        norito::core::SerializePayload::serialize(&payload, writer)
    }
}

impl<'a> norito::core::DeserializePayload<'a> for GovernanceStatus {
    fn deserialize(archived: &'a norito::core::Archived<GovernanceStatus>) -> Self {
        let (
            proposals,
            protected_namespace,
            manifest_admission,
            manifest_quorum,
            recent_manifest_activations,
            sealed_lanes_total,
            sealed_lane_aliases,
            citizens_total,
        ): (
            GovernanceProposalCounters,
            GovernanceProtectedNamespaceCounters,
            GovernanceManifestAdmissionCounters,
            GovernanceManifestQuorumCounters,
            Vec<GovernanceManifestActivation>,
            u32,
            Vec<String>,
            u64,
        ) = norito::core::DeserializePayload::deserialize(archived.cast());
        Self {
            proposals,
            protected_namespace,
            manifest_admission,
            manifest_quorum,
            recent_manifest_activations,
            sealed_lanes_total,
            sealed_lane_aliases,
            citizens_total,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for GovernanceStatus {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (
            (
                proposals,
                protected_namespace,
                manifest_admission,
                manifest_quorum,
                recent_manifest_activations,
                sealed_lanes_total,
                sealed_lane_aliases,
                citizens_total,
            ),
            used,
        ) = <(
            GovernanceProposalCounters,
            GovernanceProtectedNamespaceCounters,
            GovernanceManifestAdmissionCounters,
            GovernanceManifestQuorumCounters,
            Vec<GovernanceManifestActivation>,
            u32,
            Vec<String>,
            u64,
        )>::decode_from_slice(bytes)?;
        Ok((
            Self {
                proposals,
                protected_namespace,
                manifest_admission,
                manifest_quorum,
                recent_manifest_activations,
                sealed_lanes_total,
                sealed_lane_aliases,
                citizens_total,
            },
            used,
        ))
    }
}

impl norito::core::SerializePayload for GovernanceProposalCounters {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.proposed,
            self.rejected,
            self.enacted,
            self.superseded,
            self.execution_failed,
        );
        norito::core::SerializePayload::serialize(&payload, writer)
    }
}

impl<'a> norito::core::DeserializePayload<'a> for GovernanceProposalCounters {
    fn deserialize(archived: &'a norito::core::Archived<GovernanceProposalCounters>) -> Self {
        let (proposed, rejected, enacted, superseded, execution_failed): (u64, u64, u64, u64, u64) =
            norito::core::DeserializePayload::deserialize(archived.cast());
        Self {
            proposed,
            rejected,
            enacted,
            superseded,
            execution_failed,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for GovernanceProposalCounters {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((proposed, rejected, enacted, superseded, execution_failed), used) =
            <(u64, u64, u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                proposed,
                rejected,
                enacted,
                superseded,
                execution_failed,
            },
            used,
        ))
    }
}

impl norito::core::SerializePayload for GovernanceProtectedNamespaceCounters {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (self.total_checks, self.allowed, self.rejected);
        norito::core::SerializePayload::serialize(&payload, writer)
    }
}

impl<'a> norito::core::DeserializePayload<'a> for GovernanceProtectedNamespaceCounters {
    fn deserialize(
        archived: &'a norito::core::Archived<GovernanceProtectedNamespaceCounters>,
    ) -> Self {
        let (total_checks, allowed, rejected): (u64, u64, u64) =
            norito::core::DeserializePayload::deserialize(archived.cast());
        Self {
            total_checks,
            allowed,
            rejected,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for GovernanceProtectedNamespaceCounters {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((total_checks, allowed, rejected), used) =
            <(u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                total_checks,
                allowed,
                rejected,
            },
            used,
        ))
    }
}
impl<'a> DecodeFromSlice<'a> for GovernanceManifestAdmissionCounters {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (
            (
                total_checks,
                allowed,
                missing_manifest,
                non_validator_authority,
                quorum_rejected,
                protected_namespace_rejected,
                runtime_hook_rejected,
            ),
            used,
        ) = <(u64, u64, u64, u64, u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                total_checks,
                allowed,
                missing_manifest,
                non_validator_authority,
                quorum_rejected,
                protected_namespace_rejected,
                runtime_hook_rejected,
            },
            used,
        ))
    }
}

impl norito::core::SerializePayload for GovernanceManifestQuorumCounters {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (self.total_checks, self.satisfied, self.rejected);
        norito::core::SerializePayload::serialize(&payload, writer)
    }
}

impl<'a> norito::core::DeserializePayload<'a> for GovernanceManifestQuorumCounters {
    fn deserialize(archived: &'a norito::core::Archived<GovernanceManifestQuorumCounters>) -> Self {
        let (total_checks, satisfied, rejected): (u64, u64, u64) =
            norito::core::DeserializePayload::deserialize(archived.cast());
        Self {
            total_checks,
            satisfied,
            rejected,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for GovernanceManifestQuorumCounters {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((total_checks, satisfied, rejected), used) =
            <(u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                total_checks,
                satisfied,
                rejected,
            },
            used,
        ))
    }
}

impl norito::core::SerializePayload for GovernanceManifestActivation {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.contract_address.clone(),
            self.code_hash_hex.clone(),
            self.abi_hash_hex.clone(),
            self.height,
            self.activated_at_ms,
        );
        norito::core::SerializePayload::serialize(&payload, writer)
    }
}

impl<'a> norito::core::DeserializePayload<'a> for GovernanceManifestActivation {
    fn deserialize(archived: &'a norito::core::Archived<GovernanceManifestActivation>) -> Self {
        let (contract_address, code_hash_hex, abi_hash_hex, height, activated_at_ms): (
            String,
            String,
            Option<String>,
            u64,
            u64,
        ) = norito::core::DeserializePayload::deserialize(archived.cast());
        Self {
            contract_address,
            code_hash_hex,
            abi_hash_hex,
            height,
            activated_at_ms,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for GovernanceManifestActivation {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((contract_address, code_hash_hex, abi_hash_hex, height, activated_at_ms), used) =
            <(String, String, Option<String>, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                contract_address,
                code_hash_hex,
                abi_hash_hex,
                height,
                activated_at_ms,
            },
            used,
        ))
    }
}

#[cfg(test)]
mod captured_frame_identity_tests {
    #[test]
    fn observed_declared_identities() {
        crate::captured_identity_tests::assert_bidirectional::<
            super::GovernanceManifestAdmissionCounters,
        >("iroha_torii_shared::status::governance::GovernanceManifestAdmissionCounters");
    }
    #[test]
    fn observed_manual_frame_goldens() {
        crate::captured_identity_tests::assert_manual::<super::GovernanceStatus>(
            "GovernanceStatus",
        );
        crate::captured_identity_tests::assert_manual::<super::GovernanceProposalCounters>(
            "GovernanceProposalCounters",
        );
        crate::captured_identity_tests::assert_manual::<super::GovernanceProtectedNamespaceCounters>(
            "GovernanceProtectedNamespaceCounters",
        );
        crate::captured_identity_tests::assert_manual::<super::GovernanceManifestQuorumCounters>(
            "GovernanceManifestQuorumCounters",
        );
        crate::captured_identity_tests::assert_manual::<super::GovernanceManifestActivation>(
            "GovernanceManifestActivation",
        );
    }
}
