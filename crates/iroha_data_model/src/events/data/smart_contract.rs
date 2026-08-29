//! Smart contract registry lifecycle events.
pub use self::model::*;
use super::*;
use iroha_data_model_derive::model;
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
        /// Revocable Parliament lifecycle delegation changed.
        ParliamentDelegationChanged(ContractParliamentDelegationChanged),
        /// A two-step ownership transfer was offered.
        OwnershipTransferOffered(ContractOwnershipTransferOffered),
        /// An outstanding ownership offer was cancelled.
        OwnershipTransferCancelled(ContractOwnershipTransferCancelled),
        /// A two-step ownership transfer completed.
        OwnershipTransferred(ContractOwnershipTransferred),
        /// Parliament imposed a time-bounded emergency execution hold.
        EmergencyHoldPlaced(ContractEmergencyHoldPlaced),
        /// Parliament completed the retrospective for an expired emergency hold.
        EmergencyHoldRetrospectiveCompleted(ContractEmergencyHoldRetrospectiveCompleted),
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
        /// Canonical contract address.
        pub contract_address: crate::smart_contract::ContractAddress,
        /// Code hash bound to the instance.
        pub code_hash: iroha_crypto::Hash,
        /// Operator that performed the activation.
        pub activated_by: crate::account::AccountId,
        /// Complete lifecycle record after activation.
        pub lifecycle: crate::smart_contract::ContractLifecycleControlV1,
    }
    /// Payload describing an instance deactivation.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct ContractInstanceDeactivated {
        /// Canonical contract address.
        pub contract_address: crate::smart_contract::ContractAddress,
        /// Previously bound code hash.
        pub previous_code_hash: iroha_crypto::Hash,
        /// Operator that performed the deactivation.
        pub deactivated_by: crate::account::AccountId,
        /// Optional audit reason supplied by the caller.
        #[norito(default)]
        pub reason: Option<String>,
        /// Complete lifecycle record after deactivation.
        pub lifecycle: crate::smart_contract::ContractLifecycleControlV1,
    }
    /// Payload describing a revocable delegation change.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct ContractParliamentDelegationChanged {
        /// Contract whose delegation changed.
        pub contract_address: crate::smart_contract::ContractAddress,
        /// New delegation state.
        pub delegation: crate::smart_contract::ContractParliamentDelegationV1,
        /// Account owner that authorized the change.
        pub changed_by: crate::account::AccountId,
        /// New lifecycle revision.
        pub revision: u64,
        /// Complete lifecycle record after the delegation change.
        pub lifecycle: crate::smart_contract::ContractLifecycleControlV1,
    }
    /// Payload describing a newly recorded ownership offer.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct ContractOwnershipTransferOffered {
        /// Contract whose ownership was offered.
        pub contract_address: crate::smart_contract::ContractAddress,
        /// Current owner that authorized the offer.
        pub current_owner: crate::smart_contract::ContractLifecycleOwnerV1,
        /// Proposed next owner.
        pub pending_owner: crate::smart_contract::ContractLifecycleOwnerV1,
        /// New lifecycle revision.
        pub revision: u64,
        /// Complete lifecycle record after recording the offer.
        pub lifecycle: crate::smart_contract::ContractLifecycleControlV1,
    }
    /// Payload describing cancellation of an ownership offer.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct ContractOwnershipTransferCancelled {
        /// Contract whose offer was cancelled.
        pub contract_address: crate::smart_contract::ContractAddress,
        /// Owner that cancelled the offer.
        pub owner: crate::smart_contract::ContractLifecycleOwnerV1,
        /// New lifecycle revision.
        pub revision: u64,
        /// Complete lifecycle record after cancelling the offer.
        pub lifecycle: crate::smart_contract::ContractLifecycleControlV1,
    }
    /// Payload describing a completed ownership transfer.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct ContractOwnershipTransferred {
        /// Contract whose owner changed.
        pub contract_address: crate::smart_contract::ContractAddress,
        /// Previous lifecycle owner.
        pub previous_owner: crate::smart_contract::ContractLifecycleOwnerV1,
        /// Accepted lifecycle owner.
        pub new_owner: crate::smart_contract::ContractLifecycleOwnerV1,
        /// New lifecycle revision.
        pub revision: u64,
        /// Complete lifecycle record after accepting the transfer.
        pub lifecycle: crate::smart_contract::ContractLifecycleControlV1,
    }
    /// Payload describing a Parliament emergency hold.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct ContractEmergencyHoldPlaced {
        /// Contract whose execution is contained.
        pub contract_address: crate::smart_contract::ContractAddress,
        /// Complete bounded hold record.
        pub hold: crate::smart_contract::ContractEmergencyHoldV1,
        /// New lifecycle revision.
        pub revision: u64,
        /// Complete lifecycle record after imposing the hold.
        pub lifecycle: crate::smart_contract::ContractLifecycleControlV1,
    }
    /// Complete audit record for a certified emergency-hold retrospective.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct ContractEmergencyHoldRetrospectiveCompleted {
        /// Contract whose expired containment record was reviewed.
        pub contract_address: crate::smart_contract::ContractAddress,
        /// Exact hold removed by the retrospective.
        pub prior_hold: crate::smart_contract::ContractEmergencyHoldV1,
        /// Non-zero root of Parliament's certified retrospective finding.
        pub retrospective_finding_root: [u8; 32],
        /// New lifecycle revision.
        pub revision: u64,
        /// Complete lifecycle record after clearing the reviewed hold.
        pub lifecycle: crate::smart_contract::ContractLifecycleControlV1,
    }
}
