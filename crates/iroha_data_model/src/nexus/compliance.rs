//! Lane compliance policy data structures shared across hosts and SDKs.

use std::collections::BTreeSet;

use iroha_crypto::{Hash, LaneCommitmentId};
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    account::AccountId,
    asset::AssetDefinitionId,
    domain::DomainId,
    metadata::Metadata,
    nexus::{DataSpaceId, LaneId, UniversalAccountId},
};

/// Identifier attached to a [`LaneCompliancePolicy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[repr(transparent)]
#[norito(transparent, decode_from_slice)]
pub struct LaneCompliancePolicyId(Hash);

impl LaneCompliancePolicyId {
    /// Construct a new identifier from a hash.
    #[must_use]
    pub const fn new(hash: Hash) -> Self {
        Self(hash)
    }

    /// Borrow the underlying hash.
    #[must_use]
    pub const fn as_hash(&self) -> &Hash {
        &self.0
    }
}

impl From<Hash> for LaneCompliancePolicyId {
    fn from(value: Hash) -> Self {
        Self::new(value)
    }
}

impl Default for LaneCompliancePolicyId {
    fn default() -> Self {
        Self(Hash::prehashed([0_u8; Hash::LENGTH]))
    }
}

/// Declarative policy describing per-lane allow/deny rules and audit controls.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct LaneCompliancePolicy {
    /// Unique identifier for the policy payload.
    pub id: LaneCompliancePolicyId,
    /// Monotonic version allowing rollbacks/updates.
    pub version: u32,
    /// Lane the policy applies to.
    pub lane_id: LaneId,
    /// Dataspace the policy references (mirrors manifest metadata).
    pub dataspace_id: DataSpaceId,
    /// Jurisdictional labels associated with the policy.
    #[norito(default)]
    pub jurisdiction: JurisdictionSet,
    /// Deny rules evaluated first (deny-wins).
    #[norito(default)]
    pub deny: Vec<LaneComplianceRule>,
    /// Allow rules evaluated after denials.
    #[norito(default)]
    pub allow: Vec<LaneComplianceRule>,
    /// Transfer limits enforced after allow/deny rules.
    #[norito(default)]
    pub transfer_limits: Vec<TransferLimit>,
    /// Audit retention knobs.
    #[norito(default)]
    pub audit_controls: AuditControls,
    /// Arbitrary metadata for operator dashboards.
    #[norito(default)]
    pub metadata: Metadata,
}

impl LaneCompliancePolicy {
    /// Borrow the policy identifier.
    #[must_use]
    pub const fn id(&self) -> LaneCompliancePolicyId {
        self.id
    }
}

/// Rule evaluated against incoming transactions.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct LaneComplianceRule {
    /// Selector describing which participants are affected.
    pub selector: ParticipantSelector,
    /// Optional reason code emitted when the rule matches.
    pub reason_code: Option<String>,
    /// Optional jurisdictional override when the rule matches.
    #[norito(default)]
    pub jurisdiction_override: JurisdictionSet,
}

impl LaneComplianceRule {
    /// Human-readable reason code (if supplied).
    #[must_use]
    pub fn reason_code(&self) -> Option<&str> {
        self.reason_code.as_deref()
    }
}

/// Participant selector matching incoming transactions.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ParticipantSelector {
    /// Specific account identifier required by the rule.
    #[norito(default)]
    pub account: Option<AccountId>,
    /// Domain identifier required by the rule.
    #[norito(default)]
    pub domain: Option<DomainId>,
    /// Domain prefix matcher (allow lists entire namespace trees).
    #[norito(default)]
    pub domain_prefix: Option<String>,
    /// UAID required by the rule.
    #[norito(default)]
    pub uaid: Option<UniversalAccountId>,
    /// UAID prefix matcher.
    #[norito(default)]
    pub uaid_prefix: Option<UniversalAccountId>,
    /// Optional capability tag associated with the policy.
    #[norito(default)]
    pub capability_tag: Option<String>,
    /// Lane privacy commitments that must be advertised by the manifest (any-of semantics).
    #[norito(default)]
    pub privacy_commitments_any_of: Vec<LaneCommitmentId>,
}

/// Transfer limits expressed per asset or policy bucket.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TransferLimit {
    /// Asset identifier (when scoped to a specific asset). `None` implies XOR notional tracking.
    #[norito(default)]
    pub asset_id: Option<AssetDefinitionId>,
    /// Maximum per-transaction amount expressed in XOR equivalent units.
    #[norito(default)]
    pub max_notional_xor: Option<Numeric>,
    /// Maximum rolling daily exposure (XOR equivalent).
    #[norito(default)]
    pub max_daily_notional_xor: Option<Numeric>,
    /// Optional label describing the bucket (e.g., retail, wholesale).
    #[norito(default)]
    pub bucket: Option<String>,
}

/// Audit knobs controlling how long decision records are persisted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AuditControls {
    /// Whether to persist every denial decision.
    pub record_denials: bool,
    /// Whether to sample successful decisions.
    pub sample_successes: bool,
    /// Retention in milliseconds for stored decision records.
    pub retention_ms: u64,
}

impl Default for AuditControls {
    fn default() -> Self {
        Self {
            record_denials: true,
            sample_successes: false,
            retention_ms: 86_400_000, // 24h default
        }
    }
}

/// Jurisdiction labels applied to a policy or rule override.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct JurisdictionSet {
    /// Backing set of jurisdiction flags.
    #[norito(default)]
    pub flags: BTreeSet<JurisdictionFlag>,
}

impl JurisdictionSet {
    /// Create an empty set.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            flags: BTreeSet::new(),
        }
    }

    /// Insert a new flag.
    pub fn insert(&mut self, flag: JurisdictionFlag) {
        self.flags.insert(flag);
    }

    /// Whether the set contains the provided flag.
    #[must_use]
    pub fn contains(&self, flag: JurisdictionFlag) -> bool {
        self.flags.contains(&flag)
    }
}

/// Enumerates common jurisdiction tags used for reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "flag", content = "state"))]
pub enum JurisdictionFlag {
    /// European Economic Area / EU oversight.
    EuEea,
    /// United States Federal regulator oversight.
    UsFed,
    /// Japanese FIEL oversight.
    JpFiel,
    /// Indicates the policy enforces sanctioned-party screening.
    SanctionsScreened,
}
