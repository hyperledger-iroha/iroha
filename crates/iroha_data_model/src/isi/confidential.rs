use super::*;
use crate::confidential::{
    ConfidentialParamsId, ConfidentialStatus, PedersenParams, PoseidonParams,
};

isi! {
    /// Publish a new Pedersen parameter set into the registry.
    pub struct PublishPedersenParams {
        /// Parameter set descriptor to insert.
        pub params: PedersenParams,
    }
}

isi! {
    /// Update lifecycle metadata for an existing Pedersen parameter set.
    pub struct SetPedersenParamsLifecycle {
        /// Identifier of the parameter set to update.
        pub params_id: ConfidentialParamsId,
        /// Lifecycle status to apply.
        pub status: ConfidentialStatus,
        /// Optional activation height override.
        pub activation_height: Option<u64>,
        /// Optional withdraw height override.
        pub withdraw_height: Option<u64>,
    }
}

isi! {
    /// Publish a new Poseidon parameter set into the registry.
    pub struct PublishPoseidonParams {
        /// Parameter set descriptor to insert.
        pub params: PoseidonParams,
    }
}

isi! {
    /// Update lifecycle metadata for an existing Poseidon parameter set.
    pub struct SetPoseidonParamsLifecycle {
        /// Identifier of the parameter set to update.
        pub params_id: ConfidentialParamsId,
        /// Lifecycle status to apply.
        pub status: ConfidentialStatus,
        /// Optional activation height override.
        pub activation_height: Option<u64>,
        /// Optional withdraw height override.
        pub withdraw_height: Option<u64>,
    }
}

impl crate::seal::Instruction for PublishPedersenParams {}
impl crate::seal::Instruction for SetPedersenParamsLifecycle {}
impl crate::seal::Instruction for PublishPoseidonParams {}
impl crate::seal::Instruction for SetPoseidonParamsLifecycle {}
