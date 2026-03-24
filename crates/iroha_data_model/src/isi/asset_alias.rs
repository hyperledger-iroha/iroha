//! Asset-definition alias binding instructions.

use super::*;

isi! {
    /// Bind, update, or clear an alias for an existing asset definition.
    ///
    /// `alias = None` clears the current binding.
    /// `alias = Some(...)` sets/updates the binding and optionally refreshes lease metadata.
    pub struct SetAssetDefinitionAlias {
        /// Asset definition that should be updated.
        pub asset_definition_id: AssetDefinitionId,
        /// Alias literal (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`). `None` clears
        /// the binding.
        #[norito(default)]
        pub alias: Option<AssetDefinitionAlias>,
        /// Optional lease expiry timestamp (unix ms). When absent, the binding is treated as non-expiring.
        #[norito(default)]
        pub lease_expiry_ms: Option<u64>,
    }
}

impl SetAssetDefinitionAlias {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.asset_definition.alias.set";

    /// Create a binding/update instruction.
    #[must_use]
    pub fn bind(
        asset_definition_id: AssetDefinitionId,
        alias: AssetDefinitionAlias,
        lease_expiry_ms: Option<u64>,
    ) -> Self {
        Self {
            asset_definition_id,
            alias: Some(alias),
            lease_expiry_ms,
        }
    }

    /// Create an instruction that clears the current alias binding.
    #[must_use]
    pub fn clear(asset_definition_id: AssetDefinitionId) -> Self {
        Self {
            asset_definition_id,
            alias: None,
            lease_expiry_ms: None,
        }
    }
}

impl crate::seal::Instruction for SetAssetDefinitionAlias {}
