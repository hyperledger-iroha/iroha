//! Durable execution routing context committed by a block header.

use iroha_crypto::HashOf;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    nexus::{DataSpaceId, LaneId},
    transaction::signed::TransactionEntrypoint,
};

/// Routing context used to execute one external block entrypoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ExternalExecutionContext {
    /// Hash of the external entrypoint this context belongs to.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Lane selected for execution.
    pub lane_id: LaneId,
    /// Dataspace selected for execution.
    pub dataspace_id: DataSpaceId,
}

impl ExternalExecutionContext {
    /// Construct routing context for one external entrypoint.
    #[must_use]
    pub const fn new(
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
    ) -> Self {
        Self {
            entrypoint_hash,
            lane_id,
            dataspace_id,
        }
    }
}

/// Ordered execution context for external entrypoints in a block payload.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct BlockExecutionContextBundle {
    /// Routing context entries aligned with the block's external entrypoints.
    pub external: Vec<ExternalExecutionContext>,
}

impl BlockExecutionContextBundle {
    /// Construct an ordered execution context bundle.
    #[must_use]
    pub const fn new(external: Vec<ExternalExecutionContext>) -> Self {
        Self { external }
    }

    /// Returns true when the bundle carries no execution context.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.external.is_empty()
    }
}
