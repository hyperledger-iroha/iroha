//! The upgrade checkpoint: the last block a running network's predecessor release executed before
//! the network adopted the live-upgrade foundation there (`specs/live_runtime_upgrades.md` §13).
//!
//! Blocks at or below the checkpoint are the predecessor's final history. This release never
//! re-executes or replays them: a node restores a snapshot of this release above the checkpoint
//! and executes forward, and only the archived predecessor release verifies the blocks up to it.
//! The configuration (`snapshot.upgrade_checkpoint`) names the checkpoint, and Kura checks it
//! against the block it stores at that height.

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::BlockHeader;

/// A validated upgrade checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeCheckpointV1 {
    /// Height of the last block the predecessor release executed.
    pub height: u64,
    /// Hash of that block.
    pub block_hash: HashOf<BlockHeader>,
}

impl UpgradeCheckpointV1 {
    /// The checkpoint of a validated node configuration.
    #[must_use]
    pub fn from_config(checkpoint: &iroha_config::parameters::user::UpgradeCheckpoint) -> Self {
        Self {
            height: checkpoint.height,
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed(checkpoint.block_hash)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_configured_checkpoint_names_its_height_and_block() {
        let configured = iroha_config::parameters::user::UpgradeCheckpoint {
            height: 5,
            block_hash: [7; 32],
        };
        assert_eq!(
            UpgradeCheckpointV1::from_config(&configured),
            UpgradeCheckpointV1 {
                height: 5,
                block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([7; 32])),
            }
        );
    }
}
