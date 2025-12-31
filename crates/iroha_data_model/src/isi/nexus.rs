use super::*;
use crate::{
    account::AccountId,
    metadata::Metadata,
    nexus::DataSpaceId,
};

isi! {
    /// Set or clear emergency validators used for lane relay quorum recovery.
    pub struct SetLaneRelayEmergencyValidators {
        /// Dataspace whose validator pool is being overridden.
        pub dataspace_id: DataSpaceId,
        /// Validators added to the pool when quorum is at risk.
        pub validators: Vec<AccountId>,
        /// Optional block height (inclusive) after which the override expires.
        #[norito(skip_serializing_if = "Option::is_none")]
        #[norito(default)]
        pub expires_at_height: Option<u64>,
        /// Optional metadata describing the override decision.
        #[norito(default)]
        pub metadata: Metadata,
    }
}

impl crate::seal::Instruction for SetLaneRelayEmergencyValidators {}
