//! Node-owned immutable checkpoint delivery source, pinned to independent startup identity.

use iroha_config::parameters::actual::ToriiRuntimeCheckpoint;
use iroha_core::execution_attempt::ExecutionAttemptError;
use iroha_core::snapshot::transfer::transport::{
    Attempt, CheckpointTransport, Error, ExportedCheckpoint, TransferLimits,
};
use iroha_data_model::NetworkId;
use iroha_model_base::chain::ChainId;
use std::sync::Arc;

/// Open only the configured immutable export using its own finite source pool.
/// The source is transport data; normal checkpoint startup still establishes State authority.
pub(crate) fn load(
    config: &ToriiRuntimeCheckpoint,
    chain: &ChainId,
    network: NetworkId,
    minimum_height: Option<u64>,
) -> Attempt<Option<Arc<parking_lot::Mutex<ExportedCheckpoint>>>> {
    let Some(directory) = &config.package_dir else {
        return Ok(None);
    };
    let budget = iroha_allocation::AllocationBudget::new(config.source_memory_bytes.get());
    let mut package = ExportedCheckpoint::open(
        &directory.resolve_relative_path(),
        chain,
        network,
        TransferLimits {
            max_artifacts: config.max_artifacts,
            max_total_bytes: config.max_total_bytes,
            max_artifact_bytes: config.max_artifact_bytes,
            chunk_bytes: config.chunk_bytes,
        },
        &budget,
    )?;
    let descriptor = package.descriptor()?;
    if minimum_height.is_some_and(|floor| descriptor.height < floor) {
        return Err(ExecutionAttemptError::Rejected(Error::Invalid(
            "configured checkpoint package is below the authenticated startup floor",
        )));
    }
    Ok(Some(Arc::new(parking_lot::Mutex::new(package))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_config_base::WithOrigin;
    use iroha_core::snapshot::transfer::transport::MAX_CHECKPOINT_TRANSFER_CHUNK_BYTES;
    use iroha_crypto::{Hash, HashOf};

    #[test]
    fn runtime_checkpoint_delivery_requires_an_explicit_existing_source() {
        let chain: ChainId = "checkpoint-delivery".parse().unwrap();
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"genesis")));
        let mut config = ToriiRuntimeCheckpoint::default();
        assert!(load(&config, &chain, network, None).unwrap().is_none());
        assert!(config.chunk_bytes.get() <= MAX_CHECKPOINT_TRANSFER_CHUNK_BYTES);
        assert_eq!(
            iroha_config::parameters::defaults::torii::runtime_checkpoint::MAX_CHUNK_BYTES,
            MAX_CHECKPOINT_TRANSFER_CHUNK_BYTES,
        );
        let directory = tempfile::tempdir().unwrap();
        let absent = directory.path().join("missing-export");
        config.package_dir = Some(WithOrigin::inline(absent.clone()));
        let error = match load(&config, &chain, network, None) {
            Ok(_) => panic!("missing source cannot become a checkpoint provider"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ExecutionAttemptError::Rejected(Error::Io(_))
        ));
        assert!(
            !absent.exists(),
            "source inspection cannot create an export"
        );
    }
}
