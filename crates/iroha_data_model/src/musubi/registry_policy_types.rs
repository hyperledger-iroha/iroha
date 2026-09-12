/// Admission mode for new archives, releases, and aliases.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(tag = "kind", content = "value", deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::musubi::MusubiRegistryAdmissionModeV1")]
pub enum MusubiRegistryAdmissionModeV1 {
    /// Reject new archives, releases, and aliases.
    Closed,
    /// Admit only allowlisted stable dataspaces.
    Allowlisted,
    /// Public admission subject to normal ownership and payment checks.
    Open,
}
/// Versioned first-release Musubi registry policy.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::musubi::MusubiRegistryPolicyV1")]
pub struct MusubiRegistryPolicyV1 {
    /// Must equal one.
    pub version: u8,
    /// Compare-and-set policy revision.
    pub revision: u64,
    /// Admission mode.
    pub mode: MusubiRegistryAdmissionModeV1,
    /// Sorted stable dataspaces used only by allowlisted mode.
    pub allowlisted_dataspaces: Vec<DataSpaceId>,
    /// Prospective alias prices.
    pub alias_pricing: MusubiAliasPricingPolicyV1,
}
impl Default for MusubiRegistryPolicyV1 {
    fn default() -> Self {
        Self {
            version: MUSUBI_REGISTRY_VERSION_V1,
            revision: 1,
            mode: MusubiRegistryAdmissionModeV1::Open,
            allowlisted_dataspaces: Vec::new(),
            alias_pricing: MusubiAliasPricingPolicyV1::GENESIS,
        }
    }
}

#[cfg(test)]
mod captured_registry_policy_types_schema_tests;
