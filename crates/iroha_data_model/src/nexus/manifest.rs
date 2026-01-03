//! Space Directory manifest representations and evaluation helpers.

use std::{convert::TryFrom, fmt, str::FromStr};

use iroha_crypto::Hash;
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::DataSpaceId;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{asset::AssetDefinitionId, name::Name};

/// Universal account identifier shared across all dataspaces.
///
/// UAIDs provide a stable capability anchor for multi-lane Nexus deployments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[repr(transparent)]
#[norito(transparent, decode_from_slice)]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
pub struct UniversalAccountId(Hash);

impl UniversalAccountId {
    /// Construct a UAID from a pre-hashed value (blake2b-32, LSB set to 1).
    #[must_use]
    pub fn from_hash(hash: Hash) -> Self {
        Self(hash)
    }

    /// Borrow the underlying hash.
    #[must_use]
    pub fn as_hash(&self) -> &Hash {
        &self.0
    }
}

impl fmt::Display for UniversalAccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "uaid:{}", self.0)
    }
}

impl From<Hash> for UniversalAccountId {
    fn from(value: Hash) -> Self {
        Self::from_hash(value)
    }
}

impl From<UniversalAccountId> for Hash {
    fn from(value: UniversalAccountId) -> Self {
        value.0
    }
}

impl FromStr for UniversalAccountId {
    type Err = iroha_crypto::error::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Hash::from_str(s).map(Self::from_hash)
    }
}

/// Canonical smart-contract identifier scoped to a dataspace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[repr(transparent)]
#[norito(transparent, decode_from_slice)]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
pub struct SmartContractId(Name);

impl SmartContractId {
    /// Construct an identifier from a [`Name`].
    #[must_use]
    pub fn new(name: Name) -> Self {
        Self(name)
    }

    /// Borrow the underlying [`Name`].
    #[must_use]
    pub fn as_name(&self) -> &Name {
        &self.0
    }
}

impl fmt::Display for SmartContractId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<Name> for SmartContractId {
    fn from(value: Name) -> Self {
        Self::new(value)
    }
}

impl From<SmartContractId> for Name {
    fn from(value: SmartContractId) -> Self {
        value.0
    }
}

impl FromStr for SmartContractId {
    type Err = crate::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Name::from_str(s).map(Self::new)
    }
}

/// Manifest version supported by the Space Directory.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema, Default,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "version", content = "state"))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub enum ManifestVersion {
    /// First capability manifest iteration.
    #[default]
    V1,
}

impl From<ManifestVersion> for u16 {
    fn from(value: ManifestVersion) -> Self {
        match value {
            ManifestVersion::V1 => 1,
        }
    }
}

/// Capability manifest describing deterministic allowances for a UAID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct AssetPermissionManifest {
    /// Schema version used to interpret the manifest.
    pub version: ManifestVersion,
    /// Universal account identifier the manifest applies to.
    pub uaid: UniversalAccountId,
    /// Dataspace hosting the manifest.
    pub dataspace: DataSpaceId,
    /// Timestamp when the manifest was issued (milliseconds since UNIX epoch).
    pub issued_ms: u64,
    /// Epoch (inclusive) when the manifest becomes active.
    pub activation_epoch: u64,
    /// Epoch (inclusive) when the manifest expires, if scheduled.
    #[norito(default)]
    pub expiry_epoch: Option<u64>,
    /// Ordered manifest entries evaluated against incoming requests.
    #[norito(default)]
    pub entries: Vec<ManifestEntry>,
}

impl AssetPermissionManifest {
    fn ensure_epoch_active(&self, epoch: u64) -> Result<(), DenyReason> {
        if epoch < self.activation_epoch {
            return Err(DenyReason::ManifestInactive {
                epoch,
                activation_epoch: self.activation_epoch,
                expiry_epoch: self.expiry_epoch,
            });
        }
        if let Some(expiry) = self.expiry_epoch
            && epoch > expiry
        {
            return Err(DenyReason::ManifestInactive {
                epoch,
                activation_epoch: self.activation_epoch,
                expiry_epoch: self.expiry_epoch,
            });
        }
        Ok(())
    }

    /// Evaluate the manifest against a capability request, applying deny-wins semantics.
    #[must_use]
    pub fn evaluate(&self, request: &CapabilityRequest<'_>) -> ManifestVerdict {
        if request.dataspace != self.dataspace {
            return ManifestVerdict::Denied(DenyReason::NoMatchingRule);
        }

        if let Err(reason) = self.ensure_epoch_active(request.epoch) {
            return ManifestVerdict::Denied(reason);
        }

        let mut allow_candidate: Option<(usize, Allowance)> = None;

        for (idx, entry) in self.entries.iter().enumerate() {
            if !entry.scope.matches(request) {
                continue;
            }

            match &entry.effect {
                ManifestEffect::Deny(directive) => {
                    let note = directive.reason.clone().or_else(|| entry.notes.clone());
                    let entry_index = Self::clamp_entry_index(idx);
                    return ManifestVerdict::Denied(DenyReason::ExplicitRule { entry_index, note });
                }
                ManifestEffect::Allow(allowance) => {
                    allow_candidate = Some((idx, allowance.clone()));
                }
            }
        }

        if let Some((idx, allowance)) = allow_candidate {
            if let (Some(requested), Some(limit)) =
                (request.amount.as_ref(), allowance.max_amount.as_ref())
                && requested > limit
            {
                return ManifestVerdict::Denied(DenyReason::AmountExceeded {
                    requested: requested.clone(),
                    permitted: limit.clone(),
                });
            }

            let entry_index = Self::clamp_entry_index(idx);
            return ManifestVerdict::Allowed(ManifestGrant {
                entry_index,
                allowance,
            });
        }

        ManifestVerdict::Denied(DenyReason::NoMatchingRule)
    }

    fn clamp_entry_index(idx: usize) -> u32 {
        u32::try_from(idx).unwrap_or(u32::MAX)
    }
}

/// Manifest entry describing a scoped allow/deny rule.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct ManifestEntry {
    /// Capability scope matcher.
    pub scope: CapabilityScope,
    /// Allow/deny decision applied when the scope matches the request.
    pub effect: ManifestEffect,
    /// Optional operator-facing notes for logging/auditing.
    #[norito(default)]
    pub notes: Option<String>,
}

/// AMX role enforced by a manifest entry.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    IntoSchema,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(decode_from_slice)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "role", content = "details"))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub enum AmxRole {
    /// Transaction initiator (root of the AMX graph).
    Initiator,
    /// Participant leg in an AMX group.
    Participant,
}

/// Scope definition that determines whether a manifest entry matches a capability request.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct CapabilityScope {
    /// Optional dataspace selector (defaults to manifest dataspace when omitted).
    #[norito(default)]
    pub dataspace: Option<DataSpaceId>,
    /// Optional smart-contract identifier constraint.
    #[norito(default)]
    pub program: Option<SmartContractId>,
    /// Optional method/entry-point constraint.
    #[norito(default)]
    pub method: Option<Name>,
    /// Optional asset definition constraint.
    #[norito(default)]
    pub asset: Option<AssetDefinitionId>,
    /// Optional AMX role requirement.
    #[norito(default)]
    pub role: Option<AmxRole>,
}

impl CapabilityScope {
    fn matches(&self, request: &CapabilityRequest<'_>) -> bool {
        if let Some(dataspace) = self.dataspace
            && request.dataspace != dataspace
        {
            return false;
        }

        if let Some(program) = &self.program {
            match request.program {
                Some(candidate) if candidate == program => {}
                _ => return false,
            }
        }

        if let Some(method) = &self.method {
            match request.method {
                Some(candidate) if candidate == method => {}
                _ => return false,
            }
        }

        if let Some(asset) = &self.asset {
            match request.asset {
                Some(candidate) if candidate == asset => {}
                _ => return false,
            }
        }

        if let Some(role) = self.role
            && request.role != Some(role)
        {
            return false;
        }

        true
    }
}

/// Decision encoded by a manifest entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(feature = "json", norito(tag = "decision", content = "details"))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub enum ManifestEffect {
    /// Allow the scoped capability subject to the provided allowance.
    Allow(Allowance),
    /// Deny the scoped capability with an optional reason.
    Deny(DenyDirective),
}

/// Allowance constraints attached to an `Allow` entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct Allowance {
    /// Optional deterministic amount cap enforced by the host.
    #[norito(default)]
    pub max_amount: Option<Numeric>,
    /// Accounting window applied to the allowance.
    pub window: AllowanceWindow,
}

/// Allowance accounting window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "window", content = "details"))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub enum AllowanceWindow {
    /// Per-slot accounting window.
    PerSlot,
    /// Rolling per-minute allowance.
    PerMinute,
    /// Rolling per-day allowance.
    PerDay,
}

impl AllowanceWindow {
    /// Millisecond duration of the accounting window.
    #[must_use]
    pub const fn duration_ms(self) -> u64 {
        match self {
            Self::PerSlot => 1_000,
            Self::PerMinute => 60_000,
            Self::PerDay => 86_400_000,
        }
    }
}

/// Deny directive metadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct DenyDirective {
    /// Optional reason recorded for the deny rule.
    #[norito(default)]
    pub reason: Option<String>,
}

/// Capability request evaluated against a manifest.
#[derive(Debug, Clone)]
pub struct CapabilityRequest<'a> {
    /// Dataspace the request targets.
    pub dataspace: DataSpaceId,
    /// Optional smart-contract identifier.
    pub program: Option<&'a SmartContractId>,
    /// Optional entry-point name.
    pub method: Option<&'a Name>,
    /// Optional asset definition identifier.
    pub asset: Option<&'a AssetDefinitionId>,
    /// Optional AMX role associated with the request.
    pub role: Option<AmxRole>,
    /// Amount requested for the capability, when applicable.
    pub amount: Option<Numeric>,
    /// Epoch associated with the request.
    pub epoch: u64,
}

impl<'a> CapabilityRequest<'a> {
    /// Construct a new capability request helper.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dataspace: DataSpaceId,
        program: Option<&'a SmartContractId>,
        method: Option<&'a Name>,
        asset: Option<&'a AssetDefinitionId>,
        role: Option<AmxRole>,
        amount: Option<Numeric>,
        epoch: u64,
    ) -> Self {
        Self {
            dataspace,
            program,
            method,
            asset,
            role,
            amount,
            epoch,
        }
    }
}

/// Result of evaluating a manifest against a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestVerdict {
    /// Request satisfied by the manifest.
    Allowed(ManifestGrant),
    /// Request denied with the provided reason.
    Denied(DenyReason),
}

/// Allowance grant returned on successful evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestGrant {
    /// Manifest entry index that matched.
    pub entry_index: u32,
    /// Allowance metadata applied to the request.
    pub allowance: Allowance,
}

/// Deny reason emitted during manifest evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenyReason {
    /// Manifest is not yet active or has expired.
    ManifestInactive {
        /// Request epoch.
        epoch: u64,
        /// Activation epoch.
        activation_epoch: u64,
        /// Optional expiry epoch.
        expiry_epoch: Option<u64>,
    },
    /// Explicit deny rule matched the request.
    ExplicitRule {
        /// Entry index that triggered the deny.
        entry_index: u32,
        /// Optional human-readable note attached to the deny entry.
        note: Option<String>,
    },
    /// Requested amount exceeds the deterministic allowance.
    AmountExceeded {
        /// Amount requested by the capability.
        requested: Numeric,
        /// Allowance threshold.
        permitted: Numeric,
    },
    /// Manifest did not contain a matching allow rule.
    NoMatchingRule,
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use proptest::prelude::*;

    use super::*;

    fn sample_uaid() -> UniversalAccountId {
        UniversalAccountId::from_hash(Hash::new(b"uaid::sample"))
    }

    fn sample_name(value: &str) -> Name {
        value.parse().expect("valid name")
    }

    fn manifest_with_entries(
        dataspace: DataSpaceId,
        entries: Vec<ManifestEntry>,
    ) -> AssetPermissionManifest {
        AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid: sample_uaid(),
            dataspace,
            issued_ms: 0,
            activation_epoch: 0,
            expiry_epoch: None,
            entries,
        }
    }

    fn manifest_request(
        dataspace: DataSpaceId,
        method: &Name,
        amount: Numeric,
    ) -> CapabilityRequest<'_> {
        CapabilityRequest::new(
            dataspace,
            None,
            Some(method),
            None,
            Some(AmxRole::Initiator),
            Some(amount),
            5,
        )
    }

    fn cbdc_manifest_fixture() -> AssetPermissionManifest {
        let uaid_hex = "0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";
        let uaid = UniversalAccountId::from_hash(
            Hash::from_str(uaid_hex).expect("fixture uaid hex must parse"),
        );
        let dataspace = DataSpaceId::new(11);
        let allowance = Allowance {
            max_amount: Some(Numeric::from(500_000_000_u64)),
            window: AllowanceWindow::PerDay,
        };
        let allow_entry = ManifestEntry {
            scope: CapabilityScope {
                dataspace: Some(dataspace),
                program: Some("cbdc.transfer".parse().expect("program id")),
                method: Some(sample_name("transfer")),
                asset: Some("CBDC#centralbank".parse().expect("asset definition")),
                role: Some(AmxRole::Initiator),
            },
            effect: ManifestEffect::Allow(allowance),
            notes: Some("Wholesale transfer allowance (per UAID, per day).".to_owned()),
        };
        let deny_entry = ManifestEntry {
            scope: CapabilityScope {
                dataspace: Some(dataspace),
                program: Some("cbdc.kit".parse().expect("program id")),
                method: Some(sample_name("withdraw")),
                asset: None,
                role: None,
            },
            effect: ManifestEffect::Deny(DenyDirective {
                reason: Some("Withdrawals disabled for this UAID.".to_owned()),
            }),
            notes: Some("Deny wins over any preceding allowance.".to_owned()),
        };
        AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid,
            dataspace,
            issued_ms: 1_762_723_200_000,
            activation_epoch: 4097,
            expiry_epoch: Some(4600),
            entries: vec![allow_entry, deny_entry],
        }
    }

    #[test]
    fn cbdc_manifest_fixture_matches_serialized_json() {
        let manifest = cbdc_manifest_fixture();
        let expected = norito::json::to_value(&manifest).expect("serialize manifest to JSON");
        if std::env::var_os("IROHA_DUMP_MANIFEST_JSON").is_some() {
            let rendered = norito::json::to_string_pretty(&expected).expect("render manifest JSON");
            eprintln!("{rendered}");
        }
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/space_directory/capability/cbdc_wholesale.manifest.json");
        let fixture = fs::read_to_string(&fixture_path).expect("read fixture JSON");
        let fixture_value: norito::json::Value =
            norito::json::from_str(&fixture).expect("parse fixture JSON");
        assert_eq!(fixture_value, expected);
    }

    fn matching_scope(method: &Name) -> CapabilityScope {
        CapabilityScope {
            dataspace: None,
            program: None,
            method: Some(method.clone()),
            asset: None,
            role: Some(AmxRole::Initiator),
        }
    }

    fn foreign_scope(foreign_dataspace: DataSpaceId) -> CapabilityScope {
        CapabilityScope {
            dataspace: Some(foreign_dataspace),
            program: None,
            method: Some(sample_name("other")),
            asset: None,
            role: Some(AmxRole::Participant),
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum EntryKind {
        MatchingAllow,
        MatchingDeny,
        NonMatchingAllow,
        NonMatchingDeny,
    }

    fn entries_with_matching_deny() -> impl Strategy<Value = Vec<EntryKind>> {
        prop::collection::vec(
            prop_oneof![
                Just(EntryKind::MatchingDeny),
                Just(EntryKind::MatchingAllow),
                Just(EntryKind::NonMatchingAllow),
                Just(EntryKind::NonMatchingDeny),
            ],
            1..12,
        )
        .prop_filter("sequence must contain a matching deny", |entries| {
            entries
                .iter()
                .any(|kind| matches!(kind, EntryKind::MatchingDeny))
        })
    }

    fn entries_with_matching_allow_only() -> impl Strategy<Value = Vec<EntryKind>> {
        prop::collection::vec(
            prop_oneof![
                Just(EntryKind::MatchingAllow),
                Just(EntryKind::NonMatchingAllow),
                Just(EntryKind::NonMatchingDeny),
            ],
            1..12,
        )
        .prop_filter("sequence must contain a matching allow", |entries| {
            entries
                .iter()
                .any(|kind| matches!(kind, EntryKind::MatchingAllow))
        })
    }

    fn build_entries(
        kinds: &[EntryKind],
        method: &Name,
        foreign_dataspace: DataSpaceId,
    ) -> Vec<ManifestEntry> {
        let matching_scope = matching_scope(method);
        let foreign_scope = foreign_scope(foreign_dataspace);
        kinds
            .iter()
            .map(|kind| match kind {
                EntryKind::MatchingAllow => ManifestEntry {
                    scope: matching_scope.clone(),
                    effect: ManifestEffect::Allow(Allowance {
                        max_amount: Some(Numeric::new(50, 0)),
                        window: AllowanceWindow::PerSlot,
                    }),
                    notes: None,
                },
                EntryKind::MatchingDeny => ManifestEntry {
                    scope: matching_scope.clone(),
                    effect: ManifestEffect::Deny(DenyDirective { reason: None }),
                    notes: None,
                },
                EntryKind::NonMatchingAllow => ManifestEntry {
                    scope: foreign_scope.clone(),
                    effect: ManifestEffect::Allow(Allowance {
                        max_amount: None,
                        window: AllowanceWindow::PerMinute,
                    }),
                    notes: None,
                },
                EntryKind::NonMatchingDeny => ManifestEntry {
                    scope: foreign_scope.clone(),
                    effect: ManifestEffect::Deny(DenyDirective { reason: None }),
                    notes: None,
                },
            })
            .collect()
    }

    fn first_matching_deny_index(
        entries: &[ManifestEntry],
        request: &CapabilityRequest<'_>,
    ) -> Option<u32> {
        entries.iter().enumerate().find_map(|(idx, entry)| {
            if entry.scope.matches(request) && matches!(entry.effect, ManifestEffect::Deny(_)) {
                return Some(AssetPermissionManifest::clamp_entry_index(idx));
            }
            None
        })
    }

    fn last_matching_allow_index(
        entries: &[ManifestEntry],
        request: &CapabilityRequest<'_>,
    ) -> Option<u32> {
        entries.iter().enumerate().rev().find_map(|(idx, entry)| {
            if entry.scope.matches(request) && matches!(entry.effect, ManifestEffect::Allow(_)) {
                return Some(AssetPermissionManifest::clamp_entry_index(idx));
            }
            None
        })
    }

    #[test]
    fn deny_rule_wins_over_allow() {
        let allowance = Allowance {
            max_amount: Some(Numeric::new(100, 0)),
            window: AllowanceWindow::PerSlot,
        };
        let method = sample_name("transfer");
        let entries = vec![
            ManifestEntry {
                scope: CapabilityScope {
                    dataspace: None,
                    program: None,
                    method: Some(method.clone()),
                    asset: None,
                    role: Some(AmxRole::Initiator),
                },
                effect: ManifestEffect::Allow(allowance),
                notes: None,
            },
            ManifestEntry {
                scope: CapabilityScope {
                    dataspace: None,
                    program: None,
                    method: Some(method.clone()),
                    asset: None,
                    role: Some(AmxRole::Initiator),
                },
                effect: ManifestEffect::Deny(DenyDirective {
                    reason: Some("regulator deny".to_owned()),
                }),
                notes: None,
            },
        ];

        let manifest = AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid: sample_uaid(),
            dataspace: DataSpaceId::new(7),
            issued_ms: 0,
            activation_epoch: 10,
            expiry_epoch: None,
            entries,
        };

        let request = CapabilityRequest::new(
            manifest.dataspace,
            None,
            Some(&method),
            None,
            Some(AmxRole::Initiator),
            Some(Numeric::new(1, 0)),
            12,
        );

        match manifest.evaluate(&request) {
            ManifestVerdict::Denied(DenyReason::ExplicitRule { entry_index, .. }) => {
                assert_eq!(entry_index, 1);
            }
            other => panic!("expected explicit deny, got {other:?}"),
        }
    }

    #[test]
    fn manifest_inactive_outside_epoch_window() {
        let manifest = AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid: sample_uaid(),
            dataspace: DataSpaceId::new(1),
            issued_ms: 0,
            activation_epoch: 5,
            expiry_epoch: Some(10),
            entries: Vec::new(),
        };

        let request = CapabilityRequest::new(manifest.dataspace, None, None, None, None, None, 4);

        assert!(matches!(
            manifest.evaluate(&request),
            ManifestVerdict::Denied(DenyReason::ManifestInactive { .. })
        ));

        let late_request =
            CapabilityRequest::new(manifest.dataspace, None, None, None, None, None, 11);

        assert!(matches!(
            manifest.evaluate(&late_request),
            ManifestVerdict::Denied(DenyReason::ManifestInactive { .. })
        ));
    }

    #[test]
    fn allowance_enforced_for_amounts() {
        let method = sample_name("mint");
        let entries = vec![ManifestEntry {
            scope: CapabilityScope {
                dataspace: None,
                program: None,
                method: Some(method.clone()),
                asset: None,
                role: None,
            },
            effect: ManifestEffect::Allow(Allowance {
                max_amount: Some(Numeric::new(5, 0)),
                window: AllowanceWindow::PerMinute,
            }),
            notes: None,
        }];

        let manifest = AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid: sample_uaid(),
            dataspace: DataSpaceId::new(9),
            issued_ms: 0,
            activation_epoch: 0,
            expiry_epoch: None,
            entries,
        };

        let ok_request = CapabilityRequest::new(
            manifest.dataspace,
            None,
            Some(&method),
            None,
            None,
            Some(Numeric::new(3, 0)),
            1,
        );

        assert!(matches!(
            manifest.evaluate(&ok_request),
            ManifestVerdict::Allowed(ManifestGrant { entry_index: 0, .. })
        ));

        let over_request = CapabilityRequest::new(
            manifest.dataspace,
            None,
            Some(&method),
            None,
            None,
            Some(Numeric::new(10, 0)),
            1,
        );

        match manifest.evaluate(&over_request) {
            ManifestVerdict::Denied(DenyReason::AmountExceeded { .. }) => {}
            other => panic!("expected amount exceeded, got {other:?}"),
        }
    }

    proptest! {
        #[test]
        fn matching_deny_always_wins(
            kinds in entries_with_matching_deny(),
        ) {
            let method = sample_name("transfer_deny_prop");
            let dataspace = DataSpaceId::new(42);
            let entries = build_entries(&kinds, &method, DataSpaceId::new(99));
            let manifest = manifest_with_entries(dataspace, entries.clone());
            let request = manifest_request(dataspace, &method, Numeric::new(1, 0));

            let expected_idx = first_matching_deny_index(&entries, &request).expect("generated at least one matching deny");
            match manifest.evaluate(&request) {
                ManifestVerdict::Denied(DenyReason::ExplicitRule { entry_index, .. }) => {
                    assert_eq!(entry_index, expected_idx, "deny verdict should point at the first matching deny entry");
                }
                other => panic!("expected explicit deny when a matching deny rule exists, got {other:?}"),
            }
        }

        #[test]
        fn last_matching_allow_applied_when_no_denies(
            kinds in entries_with_matching_allow_only(),
        ) {
            let method = sample_name("allow_prop");
            let dataspace = DataSpaceId::new(7);
            let entries = build_entries(&kinds, &method, DataSpaceId::new(11));
            let manifest = manifest_with_entries(dataspace, entries.clone());
            let request = manifest_request(dataspace, &method, Numeric::new(1, 0));

            let expected_idx = last_matching_allow_index(&entries, &request).expect("generated at least one matching allow");
            match manifest.evaluate(&request) {
                ManifestVerdict::Allowed(ManifestGrant { entry_index, .. }) => {
                    assert_eq!(entry_index, expected_idx, "allow verdict should use the last matching allow entry");
                }
                other => panic!("expected allow when no matching deny exists, got {other:?}"),
            }
        }
    }
}
