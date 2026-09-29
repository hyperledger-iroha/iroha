use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_config::{
    base::WithOrigin,
    kura::FsyncMode,
    parameters::{
        actual::{Kura as KuraConfig, LaneConfig as RuntimeLaneConfig},
        defaults::kura::{
            BLOCKS_IN_MEMORY, FSYNC_INTERVAL, MAX_DISK_USAGE_BYTES,
        },
    },
};
use iroha_data_model::{
    block::SignedBlock,
    nexus::{LaneCatalog, LaneConfig as ModelLaneConfig, LaneLifecycleParameterV1},
};
use nonzero_ext::nonzero;
use std::{
    collections::BTreeMap,
    fs,
    num::NonZeroU32,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir as RawTempDir;
fn test_network_id(label: &[u8]) -> iroha_data_model::NetworkId {
    iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        iroha_crypto::Hash::new(label),
    ))
}

/// Explicit network used by the structural geometry and canonical-storage fixtures.
fn geometry_fixture_network_id() -> iroha_data_model::NetworkId {
    crate::kura::tests::native_storage_network_id()
}

/// Snapshot every physical entry without following links, for refusal/no-write assertions.
fn native_observation_tree(root: &std::path::Path) -> BTreeMap<std::path::PathBuf, Option<Vec<u8>>> {
    fn visit(root: &std::path::Path, directory: &std::path::Path, result: &mut BTreeMap<std::path::PathBuf, Option<Vec<u8>>>) {
        if !directory.exists() { return; }
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            let bytes = metadata.is_file().then(|| fs::read(&path).unwrap());
            result.insert(path.strip_prefix(root).unwrap().to_owned(), bytes);
            if metadata.is_dir() { visit(root, &path, result); }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}
