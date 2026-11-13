//! Durable local memory of the highest authenticated runtime history boundary.
//!
//! This record prevents an older snapshot or removal of adoption configuration from
//! reopening history that this store has already retired. It grants no checkpoint
//! import authority: its genesis and boundary hashes must match the canonical store.
//! Only committed State or the explicitly configured adoption checkpoint advances it.

use std::num::NonZeroUsize;

use iroha_crypto::HashOf;
use iroha_data_model::block::BlockHeader;
use norito::{Decode, Encode};

use super::{Error, Kura, Result};
use crate::state::upgrade_checkpoint::UpgradeCheckpointV1;

pub(super) const FILE_NAME: &str = "runtime-history-boundary.norito";
const MAX_BYTES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::kura::RuntimeHistoryBoundaryV1")]
struct RuntimeHistoryBoundaryV1 {
    version: u32,
    genesis: HashOf<BlockHeader>,
    height: u64,
    block_hash: HashOf<BlockHeader>,
}

impl RuntimeHistoryBoundaryV1 {
    fn checkpoint(self) -> UpgradeCheckpointV1 {
        UpgradeCheckpointV1 {
            height: self.height,
            block_hash: self.block_hash,
        }
    }
}

/// A boundary checked against the original canonical hash journal before any
/// publication locks are acquired. Persisting it reads no block-store locks.
pub(crate) struct PreparedRuntimeHistoryBoundary {
    record: RuntimeHistoryBoundaryV1,
    bytes: Vec<u8>,
}

impl PreparedRuntimeHistoryBoundary {
    /// Persist before State visibility; uncertain writes require recovery.
    pub(crate) fn persist(self, kura: &Kura) -> Result<()> {
        let mut recorded = kura.runtime_history_boundary.lock();
        if let Some(previous) = *recorded {
            if previous.height > self.record.height {
                return Ok(());
            }
            if previous.height == self.record.height {
                return if previous == self.record.checkpoint() {
                    Ok(())
                } else {
                    Err(kura.runtime_boundary_error("conflicting boundary at the same height"))
                };
            }
        }
        // The retained directory writer checks ancestry and single-link private
        // custody and synchronizes both the replacement and its directory.
        kura.durable_mutation_authorized()?;
        let directory = iroha_fs::OwnerDirectory::open(&kura.store_root)
            .map_err(|error| Error::IO(error, kura.store_root.clone()))?;
        kura.validate_runtime_boundary_directory(
            directory
                .identity()
                .map_err(|error| Error::IO(error, kura.store_root.clone()))?,
        )?;
        let usage = kura
            .begin_total_disk_usage_mutation()
            .with_resource_paths(vec![kura.store_root.join(FILE_NAME)]);
        directory
            .write_private_atomic(FILE_NAME, &self.bytes, iroha_fs::PublishMode::Replace)
            .map_err(|error| Error::IO(error, kura.store_root.join(FILE_NAME)))?;
        *recorded = Some(self.record.checkpoint());
        usage.finish_resources_before_disk_rescan();
        Ok(())
    }
}

impl Kura {
    fn validate_runtime_boundary_directory(&self, actual: iroha_fs::FileIdentity) -> Result<()> {
        let original = iroha_fs::FileIdentity::of(&self.store_root_directory.file)
            .map_err(|error| Error::IO(error, self.store_root.clone()))?;
        if actual != original {
            return Err(
                self.runtime_boundary_error("original Kura runtime boundary directory changed")
            );
        }
        Ok(())
    }

    fn runtime_boundary_error(&self, message: &str) -> Error {
        Error::IO(
            std::io::Error::new(std::io::ErrorKind::InvalidData, message),
            self.store_root.join(FILE_NAME),
        )
    }

    /// Load before selecting any snapshot. Corruption, foreign history and an
    /// incomplete canonical prefix are fatal, including emergency Fast startup.
    pub(super) fn load_runtime_history_boundary(&self) -> Result<()> {
        let directory = iroha_fs::ReaderDirectory::open(&self.store_root)
            .map_err(|error| Error::IO(error, self.store_root.clone()))?;
        self.validate_runtime_boundary_directory(
            directory
                .identity()
                .map_err(|error| Error::IO(error, self.store_root.clone()))?,
        )?;
        let Some(bytes) = directory
            .read_private_optional(FILE_NAME, MAX_BYTES)
            .map_err(|error| Error::IO(error, self.store_root.join(FILE_NAME)))?
        else {
            return Ok(());
        };
        let record: RuntimeHistoryBoundaryV1 =
            norito::decode_canonical(&bytes).map_err(Error::NoritoFrame)?;
        if record.version != 1 || record.height == 0 {
            return Err(self.runtime_boundary_error("invalid runtime boundary version or height"));
        }
        if self.get_durable_block_hash(NonZeroUsize::MIN) != Some(record.genesis) {
            return Err(self.runtime_boundary_error("runtime boundary belongs to another genesis"));
        }
        self.validate_runtime_boundary_hash(&record.checkpoint())?;
        *self.runtime_history_boundary.lock() = Some(record.checkpoint());
        Ok(())
    }

    fn validate_runtime_boundary_hash(&self, boundary: &UpgradeCheckpointV1) -> Result<()> {
        let height = usize::try_from(boundary.height)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| {
                self.runtime_boundary_error("invalid runtime history boundary height")
            })?;
        if self.get_durable_block_hash(height) != Some(boundary.block_hash) {
            return Err(self.runtime_boundary_error(
                "runtime history boundary does not match the durable canonical block",
            ));
        }
        Ok(())
    }

    /// The highest boundary already learned by this store or explicitly configured
    /// for adoption. Blocks at or below it must never be re-executed.
    #[must_use]
    pub fn history_boundary(&self) -> Option<UpgradeCheckpointV1> {
        let durable = *self.runtime_history_boundary.lock();
        durable
            .into_iter()
            .chain(self.upgrade_checkpoint().copied())
            .max_by_key(|boundary| boundary.height)
    }

    /// Retain a boundary obtained from authenticated committed State. An older
    /// boundary never lowers the store's floor; the same height must name the same block.
    ///
    /// # Errors
    /// The canonical journal disagrees or the durable record cannot be published.
    pub fn record_history_boundary(&self, boundary: &UpgradeCheckpointV1) -> Result<()> {
        if let Some(prepared) = self.prepare_runtime_history_boundary(boundary)? {
            prepared.persist(self)?;
        }
        Ok(())
    }

    /// Authenticate before publication acquires its physical storage leases.
    pub(crate) fn prepare_runtime_history_boundary(
        &self,
        boundary: &UpgradeCheckpointV1,
    ) -> Result<Option<PreparedRuntimeHistoryBoundary>> {
        if let Some(previous) = *self.runtime_history_boundary.lock() {
            if previous.height == boundary.height && previous.block_hash != boundary.block_hash {
                return Err(self.runtime_boundary_error("conflicting boundary at the same height"));
            }
            if previous.height >= boundary.height {
                return Ok(None);
            }
        }
        self.validate_runtime_boundary_hash(boundary)?;
        let genesis = self
            .get_durable_block_hash(NonZeroUsize::MIN)
            .ok_or_else(|| {
                self.runtime_boundary_error("runtime boundary has no durable genesis")
            })?;
        let record = RuntimeHistoryBoundaryV1 {
            version: 1,
            genesis,
            height: boundary.height,
            block_hash: boundary.block_hash,
        };
        let bytes = norito::encode_canonical(&record).map_err(Error::NoritoFrame)?;
        Ok(Some(PreparedRuntimeHistoryBoundary { record, bytes }))
    }
}

#[cfg(test)]
mod tests;
