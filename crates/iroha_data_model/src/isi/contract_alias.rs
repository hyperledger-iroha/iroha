//! Contract alias binding instructions.

use super::*;

isi! {
    /// Bind, update, or clear an alias for an existing contract address.
    ///
    /// `alias = None` clears the current binding.
    /// `alias = Some(...)` sets or updates the binding and optionally refreshes lease metadata.
    pub struct SetContractAlias {
        /// Contract address that should be updated.
        pub contract_address: ContractAddress,
        /// Alias literal (`<name>::<domain>.<dataspace>` or `<name>::<dataspace>`). `None`
        /// clears the binding.
        #[norito(default)]
        pub alias: Option<ContractAlias>,
        /// Optional lease expiry timestamp (unix ms). When absent, the binding is treated as
        /// non-expiring.
        #[norito(default)]
        pub lease_expiry_ms: Option<u64>,
    }
}

impl SetContractAlias {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.contract.alias.set";

    /// Create a binding or update instruction.
    #[must_use]
    pub fn bind(
        contract_address: ContractAddress,
        alias: ContractAlias,
        lease_expiry_ms: Option<u64>,
    ) -> Self {
        Self {
            contract_address,
            alias: Some(alias),
            lease_expiry_ms,
        }
    }

    /// Create an instruction that clears the current alias binding.
    #[must_use]
    pub fn clear(contract_address: ContractAddress) -> Self {
        Self {
            contract_address,
            alias: None,
            lease_expiry_ms: None,
        }
    }
}

impl crate::seal::Instruction for SetContractAlias {}
