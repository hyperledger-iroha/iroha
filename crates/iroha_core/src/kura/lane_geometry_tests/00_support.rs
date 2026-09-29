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
            BLOCKS_IN_MEMORY, FSYNC_INTERVAL, LANE_HISTORY_RETENTION, MAX_DISK_USAGE_BYTES,
        },
    },
};
use iroha_data_model::{
    block::SignedBlock,
    nexus::{LaneCatalog, LaneConfig as ModelLaneConfig},
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
