//! Smart contract registry lifecycle events.

use iroha_data_model_derive::model;

pub use self::model::*;
use super::*;

#[model]
mod model {
    use super::*;

    /// Smart contract registry events emitted when manifests, bytecode, or instance
    /// bindings change.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        iroha_data_model_derive::EventSet,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum SmartContractEvent {
        /// Contract bytecode was registered on-chain.
        CodeRegistered(ContractCodeRegistered),
        /// Contract bytecode was removed from on-chain storage.
        CodeRemoved(ContractCodeRemoved),
        /// Contract instance binding was activated.
        InstanceActivated(ContractInstanceActivated),
        /// Contract instance binding was deactivated.
        InstanceDeactivated(ContractInstanceDeactivated),
    }

    /// Payload describing a new code registration.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct ContractCodeRegistered {
        /// Code hash of the registered program.
        pub code_hash: iroha_crypto::Hash,
        /// Account that submitted the registration.
        pub registrar: crate::account::AccountId,
    }

    /// Payload describing a code removal operation.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct ContractCodeRemoved {
        /// Code hash of the removed program.
        pub code_hash: iroha_crypto::Hash,
        /// Account that requested removal.
        pub removed_by: crate::account::AccountId,
        /// Optional human-readable reason for auditability.
        #[norito(default)]
        pub reason: Option<String>,
    }

    /// Payload describing an instance activation.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct ContractInstanceActivated {
        /// Governance namespace.
        pub namespace: String,
        /// Contract identifier within the namespace.
        pub contract_id: String,
        /// Code hash bound to the instance.
        pub code_hash: iroha_crypto::Hash,
        /// Operator that performed the activation.
        pub activated_by: crate::account::AccountId,
    }

    /// Payload describing an instance deactivation.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct ContractInstanceDeactivated {
        /// Governance namespace.
        pub namespace: String,
        /// Contract identifier within the namespace.
        pub contract_id: String,
        /// Previously bound code hash.
        pub previous_code_hash: iroha_crypto::Hash,
        /// Operator that performed the deactivation.
        pub deactivated_by: crate::account::AccountId,
        /// Optional audit reason supplied by the caller.
        #[norito(default)]
        pub reason: Option<String>,
    }
}
