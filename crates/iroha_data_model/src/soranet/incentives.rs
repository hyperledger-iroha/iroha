//! Incentive and treasury data structures surfaced by the `SoraNet` anonymity layer.
//!
//! The types defined here capture the artefacts required by SNNet-7 to design a staking and
//! reward programme for relays. They provide Norito-serialisable policy descriptors, bandwidth
//! proofs produced by blinded measurement clients, epoch-level performance summaries, and payout
//! instructions targeting the XOR ledger (Sora Credits). Runtime components populate these
//! payloads so the treasury can remunerate reliable relays deterministically while exposing the
//! necessary observability hooks.

use iroha_crypto::Signature;
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize, Parser};

use super::{Digest32, RelayId};
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    isi::{InstructionBox, Transfer},
    metadata::Metadata,
};

/// Identifier assigned to blinded measurement clients.
pub type MeasurementId = Digest32;

/// Configuration knobs controlling relay bonding and slashing policy.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
pub struct RelayBondPolicyV1 {
    /// Minimum bond (denominated in XOR) required for relays that expose an exit hop.
    pub minimum_exit_bond: Numeric,
    /// XOR asset definition used for bonding and payouts.
    pub bond_asset_id: AssetDefinitionId,
    /// Minimum uptime ratio (per mille) required to avoid slashing in a given epoch.
    pub uptime_floor_per_mille: u16,
    /// Slash penalty applied when uptime drops below the configured floor, measured in basis points.
    pub slash_penalty_basis_points: u16,
    /// Optional grace window (epochs) before a newly admitted relay is subject to slashing.
    #[norito(default)]
    pub activation_grace_epochs: u16,
}

impl RelayBondPolicyV1 {
    /// Returns `true` when the configured uptime floor exceeds 100%.
    #[must_use]
    pub const fn uptime_floor_is_strict(&self) -> bool {
        self.uptime_floor_per_mille > 1_000
    }
}

/// Ledger entry recording the bond posted by a `SoraNet` relay.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
pub struct RelayBondLedgerEntryV1 {
    /// Relay fingerprint as advertised in the directory.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub relay_id: RelayId,
    /// Total bond locked for the relay.
    pub bonded_amount: Numeric,
    /// Asset used for the bond (must match [`RelayBondPolicyV1::bond_asset_id`]).
    pub bond_asset_id: AssetDefinitionId,
    /// UNIX timestamp when the bond became active.
    pub bonded_since_unix: u64,
    /// Indicates whether the relay currently advertises exit capability.
    pub exit_capable: bool,
}

impl RelayBondLedgerEntryV1 {
    /// Returns `true` when an exit-capable relay satisfies the policy-defined minimum bond.
    #[must_use]
    pub fn meets_exit_minimum(&self, policy: &RelayBondPolicyV1) -> bool {
        if !self.exit_capable {
            return true;
        }

        if self.bond_asset_id != policy.bond_asset_id {
            return false;
        }

        self.bonded_amount >= policy.minimum_exit_bond
    }
}

/// Confidence metadata attached to a bandwidth proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", norito(tag = "status", content = "details"))]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
pub struct BandwidthConfidenceV1 {
    /// Number of path samples collected for the measurement window.
    pub sample_count: u16,
    /// 95th percentile jitter observed during sampling (milliseconds).
    pub jitter_p95_ms: u16,
    /// Statistical confidence expressed as per-mille (`1_000` == 100%).
    pub confidence_per_mille: u16,
}

impl BandwidthConfidenceV1 {
    /// Returns `true` when the confidence has reached full certainty (100%).
    #[must_use]
    pub const fn is_fully_confident(&self) -> bool {
        self.confidence_per_mille >= 1_000
    }
}

/// Proof emitted by a blinded measurement client verifying relay bandwidth contribution.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
pub struct RelayBandwidthProofV1 {
    /// Relay fingerprint for which the bandwidth was measured.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub relay_id: RelayId,
    /// Identifier of the blinded measurement flow.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub measurement_id: MeasurementId,
    /// Epoch against which the measurement is recorded.
    pub epoch: u32,
    /// Total verified bytes relayed during the measurement window.
    pub verified_bytes: u128,
    /// Account authorised to operate the measurement client.
    pub verifier_id: AccountId,
    /// Timestamp (seconds since UNIX epoch) when the proof was sealed.
    pub issued_at_unix: u64,
    /// Confidence metadata associated with the measurement window.
    pub confidence: BandwidthConfidenceV1,
    /// Signature covering the Norito payload to prevent tampering.
    pub signature: Signature,
    /// Optional metadata surfaced to treasury/observability dashboards.
    #[norito(default)]
    pub metadata: Metadata,
}

impl RelayBandwidthProofV1 {
    /// Returns `true` when the proof falls within the supplied epoch window.
    #[must_use]
    pub fn matches_epoch(&self, epoch: u32) -> bool {
        self.epoch == epoch
    }
}

/// Relay compliance status used when calculating rewards or penalties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
#[allow(clippy::exhaustive_enums)]
pub enum RelayComplianceStatusV1 {
    /// Relay is in good standing with no outstanding incidents.
    Clean,
    /// Relay has minor incidents recorded but remains eligible for rewards.
    Warning,
    /// Relay is suspended or under investigation and should not be rewarded.
    Suspended,
}

impl RelayComplianceStatusV1 {
    /// Returns `true` when the relay may receive rewards.
    #[must_use]
    pub const fn is_reward_eligible(&self) -> bool {
        match self {
            Self::Clean | Self::Warning => true,
            Self::Suspended => false,
        }
    }

    #[must_use]
    fn label(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Warning => "warning",
            Self::Suspended => "suspended",
        }
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for RelayComplianceStatusV1 {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(self.label(), out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for RelayComplianceStatusV1 {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        let parsed = String::json_deserialize(parser)?;
        match parsed.to_ascii_lowercase().as_str() {
            "clean" => Ok(Self::Clean),
            "warning" => Ok(Self::Warning),
            "suspended" => Ok(Self::Suspended),
            other => Err(json::Error::Message(format!(
                "invalid relay compliance status: {other}"
            ))),
        }
    }
}

/// Aggregated metrics for a relay within a specific epoch window.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
pub struct RelayEpochMetricsV1 {
    /// Relay fingerprint as advertised in the directory consensus.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub relay_id: RelayId,
    /// Epoch identifier associated with this metrics record.
    pub epoch: u32,
    /// Number of seconds the relay was observed online during the epoch.
    pub uptime_seconds: u64,
    /// Expected uptime window (seconds) for the epoch (used as denominator).
    pub scheduled_uptime_seconds: u64,
    /// Sum of verified bytes across all accepted bandwidth proofs.
    pub verified_bandwidth_bytes: u128,
    /// Compliance status recorded for the epoch.
    pub compliance: RelayComplianceStatusV1,
    /// Reward score calculated by the treasury pipeline.
    pub reward_score: u64,
    /// Minimum confidence observed across accepted bandwidth proofs (per-mille).
    pub confidence_floor_per_mille: u16,
    /// Blinded measurement identifiers attached to the record for auditability.
    #[norito(default)]
    pub measurement_ids: Vec<MeasurementId>,
    /// Optional metadata (region, rack deployment, operator hints).
    #[norito(default)]
    pub metadata: Metadata,
}

impl RelayEpochMetricsV1 {
    /// Compute the uptime ratio expressed in per-mille (0‒1000).
    #[must_use]
    pub fn uptime_ratio_per_mille(&self) -> u16 {
        if self.scheduled_uptime_seconds == 0 {
            return 0;
        }
        let ratio = u128::from(self.uptime_seconds)
            .saturating_mul(1_000)
            .checked_div(u128::from(self.scheduled_uptime_seconds))
            .unwrap_or(0);
        u16::try_from(ratio.min(1_000)).unwrap_or(1_000)
    }

    /// Returns `true` when the relay meets the uptime floor defined by the policy.
    #[must_use]
    pub fn meets_uptime_floor(&self, policy: &RelayBondPolicyV1) -> bool {
        self.uptime_ratio_per_mille() >= policy.uptime_floor_per_mille.min(1_000)
    }
}

/// Instruction surfaced to the XOR treasury for rewarding a relay.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
pub struct RelayRewardInstructionV1 {
    /// Relay fingerprint for which the payout is being issued.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub relay_id: RelayId,
    /// Epoch for which the payout applies.
    pub epoch: u32,
    /// Account that will receive the XOR payout.
    pub beneficiary: AccountId,
    /// XOR asset definition (typically the same as [`RelayBondPolicyV1::bond_asset_id`]).
    pub payout_asset_id: AssetDefinitionId,
    /// Amount of XOR awarded to the relay for the epoch.
    pub payout_amount: Numeric,
    /// Reward score used to derive the payout (mirrors [`RelayEpochMetricsV1::reward_score`]).
    pub reward_score: u64,
    /// Governance approval artefact emitted by the Sora Parliament budgeting flow.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    #[norito(default)]
    pub budget_approval_id: Option<Digest32>,
    /// Optional metadata (treasury notes, distribution batch ID, audit flags).
    #[norito(default)]
    pub metadata: Metadata,
}

impl RelayRewardInstructionV1 {
    /// Returns `true` when the payout amount is zero.
    #[must_use]
    pub fn is_zero_amount(&self) -> bool {
        self.payout_amount == Numeric::zero()
    }

    /// Convert the reward instruction into a [`Transfer`] instruction from the provided treasury account.
    #[must_use]
    pub fn to_transfer_instruction(&self, treasury_account: &AccountId) -> InstructionBox {
        let asset = AssetId::new(self.payout_asset_id.clone(), treasury_account.clone());
        Transfer::asset_numeric(asset, self.payout_amount.clone(), self.beneficiary.clone()).into()
    }
}

/// Status of a relay reward dispute raised against a payout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
pub enum RelayRewardDisputeStatusV1 {
    /// Dispute has been recorded and awaits treasury review.
    Pending,
    /// Dispute was accepted and the payout will be adjusted.
    Accepted,
    /// Dispute was rejected (payout remains unchanged).
    Rejected,
}

#[allow(clippy::derivable_impls)]
impl Default for RelayRewardDisputeStatusV1 {
    fn default() -> Self {
        Self::Pending
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for RelayRewardDisputeStatusV1 {
    fn json_serialize(&self, out: &mut String) {
        let variant = match self {
            Self::Pending => "Pending",
            Self::Accepted => "Accepted",
            Self::Rejected => "Rejected",
        };
        JsonSerialize::json_serialize(&variant, out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for RelayRewardDisputeStatusV1 {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        let variant = String::json_deserialize(parser)?;
        match variant.as_str() {
            "Pending" => Ok(Self::Pending),
            "Accepted" => Ok(Self::Accepted),
            "Rejected" => Ok(Self::Rejected),
            other => Err(json::Error::Message(format!(
                "invalid dispute status `{other}`"
            ))),
        }
    }
}

/// Record describing a relay reward dispute submitted to the treasury.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
pub struct RelayRewardDisputeV1 {
    /// Relay fingerprint associated with the disputed payout.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub relay_id: RelayId,
    /// Epoch identifier for which the payout was calculated.
    pub epoch: u32,
    /// Account identifier of the operator submitting the dispute.
    pub submitted_by: AccountId,
    /// UNIX timestamp when the dispute was filed.
    pub submitted_at_unix: u64,
    /// Original payout instruction that triggered the dispute.
    pub original_instruction: RelayRewardInstructionV1,
    /// Requested amount (in XOR) the operator believes should have been paid out.
    pub requested_amount: Numeric,
    /// Free-form textual reason supplied by the operator.
    pub reason: String,
    /// Current status of the dispute.
    #[norito(default)]
    pub status: RelayRewardDisputeStatusV1,
    /// Optional metadata captured during review/resolution.
    #[norito(default)]
    pub resolution_metadata: Metadata,
}

impl RelayRewardDisputeV1 {
    /// Construct a new dispute using the original reward instruction and requested amount.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        relay_id: RelayId,
        epoch: u32,
        submitted_by: AccountId,
        submitted_at_unix: u64,
        original_instruction: RelayRewardInstructionV1,
        requested_amount: Numeric,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            relay_id,
            epoch,
            submitted_by,
            submitted_at_unix,
            original_instruction,
            requested_amount,
            reason: reason.into(),
            status: RelayRewardDisputeStatusV1::Pending,
            resolution_metadata: Metadata::default(),
        }
    }

    /// Mark the dispute resolved with the supplied status and attach resolution metadata.
    pub fn resolve(&mut self, status: RelayRewardDisputeStatusV1, metadata: Metadata) {
        self.status = status;
        self.resolution_metadata = metadata;
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_primitives::json::Json;

    use super::*;
    use crate::{domain::DomainId, isi::TransferBox, name::Name};

    fn numeric(value: u64) -> Numeric {
        Numeric::from(value)
    }

    fn sample_account(seed: u8) -> AccountId {
        let _domain = DomainId::from_str("sora").expect("domain id");
        let (public_key, _) = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519).into_parts();
        AccountId::new(public_key)
    }

    #[test]
    fn exit_minimum_passes_when_bond_sufficient() {
        let policy = RelayBondPolicyV1 {
            minimum_exit_bond: numeric(1_000),
            bond_asset_id: AssetDefinitionId::from_str("xor#sora").expect("asset id"),
            uptime_floor_per_mille: 950,
            slash_penalty_basis_points: 250,
            activation_grace_epochs: 2,
        };
        let entry = RelayBondLedgerEntryV1 {
            relay_id: [0_u8; 32],
            bonded_amount: numeric(5_000),
            bond_asset_id: AssetDefinitionId::from_str("xor#sora").expect("asset id"),
            bonded_since_unix: 1_000,
            exit_capable: true,
        };
        assert!(entry.meets_exit_minimum(&policy));
    }

    #[test]
    fn exit_minimum_fails_when_asset_mismatch() {
        let policy = RelayBondPolicyV1 {
            minimum_exit_bond: numeric(1_000),
            bond_asset_id: AssetDefinitionId::from_str("xor#sora").expect("asset id"),
            uptime_floor_per_mille: 950,
            slash_penalty_basis_points: 250,
            activation_grace_epochs: 0,
        };
        let entry = RelayBondLedgerEntryV1 {
            relay_id: [0_u8; 32],
            bonded_amount: numeric(5_000),
            bond_asset_id: AssetDefinitionId::from_str("usd#sora").expect("asset id"),
            bonded_since_unix: 1_000,
            exit_capable: true,
        };
        assert!(!entry.meets_exit_minimum(&policy));
    }

    #[test]
    fn uptime_floor_detects_strict_threshold() {
        let policy = RelayBondPolicyV1 {
            minimum_exit_bond: numeric(500),
            bond_asset_id: AssetDefinitionId::from_str("xor#sora").expect("asset id"),
            uptime_floor_per_mille: 1_250,
            slash_penalty_basis_points: 100,
            activation_grace_epochs: 0,
        };
        assert!(policy.uptime_floor_is_strict());
    }

    #[test]
    fn bandwidth_confidence_detects_full_confidence() {
        let confidence = BandwidthConfidenceV1 {
            sample_count: 128,
            jitter_p95_ms: 12,
            confidence_per_mille: 1_000,
        };
        assert!(confidence.is_fully_confident());
    }

    #[test]
    fn bandwidth_proof_epoch_matching() {
        let proof = RelayBandwidthProofV1 {
            relay_id: [0_u8; 32],
            measurement_id: [1_u8; 32],
            epoch: 42,
            verified_bytes: 1_024,
            verifier_id: sample_account(1),
            issued_at_unix: 5,
            confidence: BandwidthConfidenceV1 {
                sample_count: 16,
                jitter_p95_ms: 20,
                confidence_per_mille: 800,
            },
            signature: Signature::from_bytes(&[2_u8; 64]),
            metadata: Metadata::default(),
        };
        assert!(proof.matches_epoch(42));
        assert!(!proof.matches_epoch(41));
    }

    #[test]
    fn compliance_status_reward_eligibility_flags() {
        assert!(RelayComplianceStatusV1::Clean.is_reward_eligible());
        assert!(RelayComplianceStatusV1::Warning.is_reward_eligible());
        assert!(!RelayComplianceStatusV1::Suspended.is_reward_eligible());
    }

    #[test]
    fn uptime_ratio_handles_zero_schedule() {
        let metrics = RelayEpochMetricsV1 {
            relay_id: [0_u8; 32],
            epoch: 7,
            uptime_seconds: 10,
            scheduled_uptime_seconds: 0,
            verified_bandwidth_bytes: 0,
            compliance: RelayComplianceStatusV1::Clean,
            reward_score: 0,
            confidence_floor_per_mille: 0,
            measurement_ids: Vec::new(),
            metadata: Metadata::default(),
        };
        assert_eq!(metrics.uptime_ratio_per_mille(), 0);
    }

    #[test]
    fn uptime_ratio_caps_at_one_thousand() {
        let metrics = RelayEpochMetricsV1 {
            relay_id: [0_u8; 32],
            epoch: 7,
            uptime_seconds: 2,
            scheduled_uptime_seconds: 1,
            verified_bandwidth_bytes: 0,
            compliance: RelayComplianceStatusV1::Clean,
            reward_score: 0,
            confidence_floor_per_mille: 0,
            measurement_ids: Vec::new(),
            metadata: Metadata::default(),
        };
        assert_eq!(metrics.uptime_ratio_per_mille(), 1_000);
    }

    #[test]
    fn meets_uptime_floor_tracks_policy() {
        let policy = RelayBondPolicyV1 {
            minimum_exit_bond: numeric(1_000),
            bond_asset_id: AssetDefinitionId::from_str("xor#sora").expect("asset id"),
            uptime_floor_per_mille: 900,
            slash_penalty_basis_points: 250,
            activation_grace_epochs: 0,
        };
        let metrics = RelayEpochMetricsV1 {
            relay_id: [0_u8; 32],
            epoch: 7,
            uptime_seconds: 9,
            scheduled_uptime_seconds: 10,
            verified_bandwidth_bytes: 0,
            compliance: RelayComplianceStatusV1::Clean,
            reward_score: 0,
            confidence_floor_per_mille: 0,
            measurement_ids: Vec::new(),
            metadata: Metadata::default(),
        };
        assert!(metrics.meets_uptime_floor(&policy));
    }

    #[test]
    fn reward_instruction_reports_zero_amount() {
        let instruction = RelayRewardInstructionV1 {
            relay_id: [0_u8; 32],
            epoch: 7,
            beneficiary: sample_account(2),
            payout_asset_id: AssetDefinitionId::from_str("xor#sora").expect("asset id"),
            payout_amount: Numeric::zero(),
            reward_score: 42,
            budget_approval_id: Some([0xA1; 32]),
            metadata: Metadata::default(),
        };
        assert!(instruction.is_zero_amount());
    }

    #[test]
    fn reward_instruction_produces_transfer() {
        let instruction = RelayRewardInstructionV1 {
            relay_id: [0xAB; 32],
            epoch: 5,
            beneficiary: sample_account(3),
            payout_asset_id: AssetDefinitionId::from_str("xor#sora").expect("asset id"),
            payout_amount: Numeric::new(5_000, 2),
            reward_score: 875,
            budget_approval_id: Some([0xCD; 32]),
            metadata: Metadata::default(),
        };
        let treasury = sample_account(4);
        let instruction_box = instruction.to_transfer_instruction(&treasury);
        let transfer_box = instruction_box
            .as_any()
            .downcast_ref::<TransferBox>()
            .expect("transfer instruction");
        let TransferBox::Asset(transfer) = transfer_box else {
            panic!("expected asset transfer");
        };
        assert_eq!(transfer.source.definition(), &instruction.payout_asset_id);
        assert_eq!(transfer.source.account(), &treasury);
        assert_eq!(transfer.destination, instruction.beneficiary);
        assert_eq!(transfer.object, instruction.payout_amount);
    }

    #[test]
    fn reward_dispute_defaults_to_pending() {
        let instruction = RelayRewardInstructionV1 {
            relay_id: [0x01; 32],
            epoch: 12,
            beneficiary: sample_account(1),
            payout_asset_id: AssetDefinitionId::from_str("xor#sora").expect("asset id"),
            payout_amount: Numeric::new(1_500, 2),
            reward_score: 800,
            budget_approval_id: Some([0xD1; 32]),
            metadata: Metadata::default(),
        };
        let mut dispute = RelayRewardDisputeV1::new(
            instruction.relay_id,
            instruction.epoch,
            sample_account(9),
            1_700_000_000,
            instruction.clone(),
            Numeric::new(1_600, 2),
            "bandwidth weighting mismatch",
        );
        assert_eq!(dispute.status, RelayRewardDisputeStatusV1::Pending);
        assert_eq!(dispute.original_instruction, instruction);

        let mut resolution = Metadata::default();
        let key = Name::from_str("decision").expect("name");
        resolution.insert(key, Json::new("adjusted"));
        dispute.resolve(RelayRewardDisputeStatusV1::Accepted, resolution.clone());
        assert_eq!(dispute.status, RelayRewardDisputeStatusV1::Accepted);
        assert_eq!(dispute.resolution_metadata, resolution);
    }
}
