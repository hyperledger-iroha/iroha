//! The checkpoints a node holds: the authenticated checkpoints of its own chain in its
//! checkpoint store, by committed height (`specs/live_runtime_upgrades.md` §7.3).
//!
//! A checkpoint at height `h` is the published State after block `h`; block `h + 1` certifies
//! it. A validator vouches for a startup floor only while it holds a restorable checkpoint at
//! exactly the floor's height (`runtime::checkpoint::attest_floor`), so it can still start from
//! it. A generation counts when its signed manifest verifies under the node's snapshot key and
//! names this chain and network, and its five canonical artifacts are present with a payload
//! that matches the signed digest: the checks a restore makes before it reads the State.

use std::{num::NonZeroUsize, path::PathBuf};

use iroha_allocation::AllocationBudget;
use iroha_crypto::{HashOf, PublicKey};
use iroha_data_model::{NetworkId, block::BlockHeader};
use iroha_model_base::chain::ChainId;

use super::{
    Config, SNAPSHOT_DIGEST_FILE_NAME, TryReadError, canonical_snapshot_digest_name,
    signed_generation_manifest, snapshot_generation_is_canonical_for_gc,
};

/// One checkpoint a node holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HeldCheckpoint {
    /// Committed height of the State it holds.
    pub height: u64,
    /// Hash of the block at that height.
    pub block_hash: HashOf<BlockHeader>,
}

/// Reads the checkpoints this node holds in its snapshot store.
#[derive(Clone, Debug)]
pub struct SnapshotCheckpoints {
    store_dir: PathBuf,
    verification_key: PublicKey,
    max_payload_bytes: NonZeroUsize,
    merkle_chunk_size: NonZeroUsize,
    read_buffer_budget: AllocationBudget,
}

impl SnapshotCheckpoints {
    /// The checkpoints of the snapshot store `config` names, authenticated by the snapshot
    /// writer's `verification_key`, read within the node's snapshot `read_buffer_budget`.
    #[must_use]
    pub fn from_config(
        config: &Config,
        verification_key: PublicKey,
        read_buffer_budget: AllocationBudget,
    ) -> Self {
        Self {
            store_dir: config.store_dir.resolve_relative_path(),
            verification_key,
            max_payload_bytes: config.max_payload_bytes,
            merkle_chunk_size: config.merkle_chunk_size_bytes,
            read_buffer_budget,
        }
    }

    /// Every checkpoint of `chain_id` on `network_id` this node holds, by height: the entries of
    /// its checkpoint store (`checkpoint_store`) that pass every check. A store without
    /// checkpoints holds none.
    ///
    /// # Errors
    /// The checkpoint store cannot be read, or holds more entries than any store keeps.
    pub fn held(
        &self,
        chain_id: &ChainId,
        network_id: NetworkId,
    ) -> Result<Vec<HeldCheckpoint>, TryReadError> {
        let directory = super::checkpoint_store::checkpoints_dir(&self.store_dir);
        let entries = super::checkpoint_store::entries(&self.store_dir)
            .map_err(|error| TryReadError::IO(error, directory))?;
        let mut held = Vec::new();
        for (height, _, path) in entries {
            let Some(manifest) = signed_generation_manifest(&path, &self.verification_key) else {
                continue;
            };
            if manifest.chain_id != *chain_id
                || manifest.network_id != network_id
                || manifest.committed_height != height
            {
                continue;
            }
            let Some(block_hash) = manifest.tip_hash else {
                continue;
            };
            // A generation is named by its payload digest; the checkpoint keeps that name in
            // its digest sidecar, against which the payload is checked.
            let Some(name) = std::fs::read_to_string(path.join(SNAPSHOT_DIGEST_FILE_NAME))
                .ok()
                .and_then(|digest| digest.strip_suffix('\n').map(str::to_owned))
                .filter(|name| canonical_snapshot_digest_name(name))
            else {
                continue;
            };
            if !snapshot_generation_is_canonical_for_gc(
                &path,
                &name,
                self.max_payload_bytes,
                self.merkle_chunk_size,
                &self.verification_key,
                &self.read_buffer_budget,
            )
            .unwrap_or(false)
            {
                continue;
            }
            held.push(HeldCheckpoint { height, block_hash });
        }
        held.sort_unstable();
        held.dedup();
        Ok(held)
    }

    /// Whether this node holds a checkpoint of `chain_id` on `network_id` at exactly `height`,
    /// of the block whose hash is `block_hash`.
    #[must_use]
    pub fn holds(
        &self,
        chain_id: &ChainId,
        network_id: NetworkId,
        height: u64,
        block_hash: &[u8; 32],
    ) -> bool {
        self.held(chain_id, network_id).is_ok_and(|held| {
            held.iter().any(|checkpoint| {
                let hash: &[u8; 32] = checkpoint.block_hash.as_ref();
                checkpoint.height == height && hash == block_hash
            })
        })
    }
}
