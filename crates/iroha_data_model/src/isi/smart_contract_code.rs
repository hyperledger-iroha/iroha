use super::*;
use crate::smart_contract::manifest::ContractManifest;

isi! {
    /// Register a smart contract manifest keyed by `code_hash` into the WSV.
    ///
    /// Note: current implementation stores only the manifest. Large code
    /// artifacts may be referenced off-chain by `code_hash`.
    pub struct RegisterSmartContractCode {
        /// Manifest containing `code_hash` (required) and `abi_hash`.
        pub manifest: ContractManifest,
    }
}

impl crate::seal::Instruction for RegisterSmartContractCode {}

isi! {
    /// Deactivate a contract instance by removing the `contract_address` binding.
    ///
    /// Deactivation acts as a governance kill-switch for compromised deployments. The address
    /// becomes unavailable immediately, while provenance information (caller and optional reason)
    /// is emitted via the data event stream.
    pub struct DeactivateContractInstance {
        /// Canonical contract address.
        pub contract_address: crate::smart_contract::ContractAddress,
        /// Optional audit reason describing why the instance was deactivated.
        #[norito(default)]
        pub reason: Option<String>,
    }
}

impl crate::seal::Instruction for DeactivateContractInstance {}

isi! {
    /// Activate a contract instance by binding `contract_address` to a `code_hash`.
    ///
    /// This creates or updates the canonical routing for a contract address. Nodes use this
    /// mapping to resolve which bytecode to execute for calls into that address.
    pub struct ActivateContractInstance {
        /// Canonical contract address.
        pub contract_address: crate::smart_contract::ContractAddress,
        /// Content-addressed code hash (Blake2b-32) of the `.to` bytecode to bind.
        pub code_hash: iroha_crypto::Hash,
    }
}

impl crate::seal::Instruction for ActivateContractInstance {}

isi! {
    /// Register compiled contract bytecode on-chain keyed by its `code_hash`.
    ///
    /// The bytecode is the full compiled `.to` image including the IVM header.
    /// Nodes verify that `code_hash` equals the Blake2b-32 digest of the program body
    /// (bytes after the IVM header) before storing.
    pub struct RegisterSmartContractBytes {
        /// Hash of the program body bytes (after IVM header).
        pub code_hash: iroha_crypto::Hash,
        /// Full compiled `.to` image (including IVM header).
        pub code: Vec<u8>,
    }
}

impl crate::seal::Instruction for RegisterSmartContractBytes {}

isi! {
    /// Remove compiled contract bytecode from on-chain storage.
    ///
    /// Removal succeeds only when no manifests or active instances reference the supplied
    /// `code_hash`. Governance operators can provide an optional audit reason that surfaces
    /// alongside the emitted removal event.
    pub struct RemoveSmartContractBytes {
        /// Hash of the program body bytes (after the IVM header) identifying the artifact to delete.
        pub code_hash: iroha_crypto::Hash,
        /// Optional audit reason explaining why the bytecode was removed.
        #[norito(default)]
        pub reason: Option<String>,
    }
}

impl crate::seal::Instruction for RemoveSmartContractBytes {}
