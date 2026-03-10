//! Asset usage policy types shared between issuer/domain/dataspace enforcement paths.

use std::collections::{BTreeMap, BTreeSet};

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{account::AccountId, asset::AssetDefinitionId, domain::DomainId, nexus::DataSpaceId};

/// Metadata key on an [`crate::asset::AssetDefinition`] storing [`AssetIssuerUsagePolicyV1`].
pub const ASSET_ISSUER_USAGE_POLICY_METADATA_KEY: &str = "iroha:asset_issuer_usage_policy_v1";
/// Metadata key on a [`crate::domain::Domain`] storing [`DomainAssetUsagePolicyV1`].
pub const DOMAIN_ASSET_USAGE_POLICY_METADATA_KEY: &str = "iroha:domain_asset_usage_policy_v1";

/// Issuer-controlled baseline policy for an asset definition.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize,
        crate::DeriveFastJson
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct AssetIssuerUsagePolicyV1 {
    /// When `true`, every participating subject must have an explicit binding.
    #[norito(default)]
    pub require_subject_binding: bool,
    /// Subject-level bindings describing allowed domain/dataspace contexts.
    #[norito(default)]
    pub subject_bindings: BTreeMap<AccountId, AssetSubjectBindingV1>,
}

impl AssetIssuerUsagePolicyV1 {
    /// Retrieve the binding for `subject` if one is present.
    #[must_use]
    pub fn binding_for(&self, subject: &AccountId) -> Option<&AssetSubjectBindingV1> {
        self.subject_bindings.get(subject)
    }
}

/// Subject-level binding payload inside [`AssetIssuerUsagePolicyV1`].
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize,
        crate::DeriveFastJson
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct AssetSubjectBindingV1 {
    /// Allowed domain contexts for this subject. Empty set means domain-neutral.
    #[norito(default)]
    pub allowed_domains: BTreeSet<DomainId>,
    /// Allowed dataspace contexts for this subject. Empty set means dataspace-neutral.
    #[norito(default)]
    pub allowed_dataspaces: BTreeSet<DataSpaceId>,
}

impl AssetSubjectBindingV1 {
    /// Return `true` when this binding allows `domain`.
    #[must_use]
    pub fn allows_domain(&self, domain: &DomainId) -> bool {
        self.allowed_domains.is_empty() || self.allowed_domains.contains(domain)
    }

    /// Return `true` when this binding allows `dataspace`.
    #[must_use]
    pub fn allows_dataspace(&self, dataspace: DataSpaceId) -> bool {
        self.allowed_dataspaces.is_empty() || self.allowed_dataspaces.contains(&dataspace)
    }
}

/// Domain-owner overlay policy for asset usage inside a specific domain.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize,
        crate::DeriveFastJson
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct DomainAssetUsagePolicyV1 {
    /// Optional allow-list. Empty means "allow any unless denied".
    #[norito(default)]
    pub allowed_assets: BTreeSet<AssetDefinitionId>,
    /// Explicit deny-list. Deny wins.
    #[norito(default)]
    pub denied_assets: BTreeSet<AssetDefinitionId>,
}

impl DomainAssetUsagePolicyV1 {
    /// Evaluate whether `asset_definition` is allowed by this overlay.
    #[must_use]
    pub fn allows(&self, asset_definition: &AssetDefinitionId) -> bool {
        if self.denied_assets.contains(asset_definition) {
            return false;
        }
        self.allowed_assets.is_empty() || self.allowed_assets.contains(asset_definition)
    }
}
