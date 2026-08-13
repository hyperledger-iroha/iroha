/// Admission mode for new archives, releases, and aliases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
pub enum MusubiRegistryAdmissionModeV1 {
    /// Reject new archives, releases, and aliases.
    Closed,
    /// Admit only allowlisted stable dataspaces.
    Allowlisted,
    /// Public admission subject to normal ownership and payment checks.
    Open,
}
/// Versioned first-release Musubi registry policy.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
