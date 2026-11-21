//! Executable isolated source-removal rehearsal, deliberately separate from release authority.
//!
//! The parent keeps real all-root retirement evidence and reviewed original source handles
//! alive while compiling an isolated candidate. That candidate restores the original oldest
//! accepted checkpoints and replays their certified suffixes. Neither an input JSON file nor
//! a child process's report substitutes for those checks. These engineering-key fixtures do
//! not qualify a deployed network, a toolchain, or permission to delete shared source.

use super::*;
use crate::{
    kura::Kura,
    snapshot::{
        self,
        checkpoints::{ReplayedFloorCheckpoint, SnapshotCheckpoints},
    },
    state::StateReadOnly as _,
};
use iroha_allocation::AllocationBudget;
use iroha_config::{
    base::WithOrigin,
    kura::{FsyncMode, InitMode},
    parameters::{actual, defaults},
};
use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
};

mod build;
mod replay;

const HANDLER: &str = "crates/iroha_core/src/smartcontracts/isi/account_signing_key/execute.rs";
const READER: &str = "crates/iroha_core/src/smartcontracts/isi/account_signing_key.rs";
const DISPATCH: &str = "crates/iroha_core/src/smartcontracts/isi/mod.rs";
const CATALOG: &str = "crates/iroha_core/src/runtime/catalog.rs";
const SUPPORT: &str = "crates/iroha_core/src/runtime/catalog/generic_carriers.rs";
const CANDIDATE_TEST: &str = "runtime::retirement::source_candidate::tests::certified::rehearsal::removed_candidate_restores_original_oldest_floors";

// This is an independently reviewed source inventory, not a digest of whatever happens to
// be in the working tree at invocation. A concurrent hook edit must stop this rehearsal.
// Regenerate only after reviewing that exact source delta and its retained dependencies.
const REVIEWED: &[(&str, u64, &str)] = &[
    (
        HANDLER,
        5022,
        "cd20cbfcfc3ad15605a8f800b56388ffc0b459de827361db1479ba797115e253",
    ),
    (
        READER,
        1915,
        "1bc1ada114b20209bd38f89ea036c1ed63e434c1a13e4282ce90ece9b205d682",
    ),
    (
        DISPATCH,
        154713,
        "ee2e8064cff2598fa79fe45a30efd962c39a976054644f983851152b8d9c10fc",
    ),
    (
        CATALOG,
        75597,
        "3bbb8a533dc0b7fb2976348364fefe5f661d90baa203213acf941673333956cb",
    ),
    (
        SUPPORT,
        11154,
        "1bd39bd9eaeba43abd771cd12354ede85ebbe168ad8b9d7f8636b6cc23ee7378",
    ),
];

#[derive(norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct RootInput {
    network: NetworkId,
    floor: u64,
    tip: u64,
    tip_hash: [u8; 32],
}

#[derive(norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct Inputs {
    parent: RootInput,
    child: RootInput,
}

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap_or_else(|error| std::panic::resume_unwind(error));
}

fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn fixture_dir() -> PathBuf {
    let value = std::env::var_os("IROHA_N14_SOURCE_REMOVAL_DIR")
        .expect("explicit isolated rehearsal directory is required");
    let path = PathBuf::from(value);
    assert!(path.is_absolute(), "rehearsal directory must be absolute");
    path
}

fn snapshot_key() -> KeyPair {
    KeyPair::from_seed(vec![0xA9; 32], Algorithm::Ed25519)
}

fn manifest(owner: ImplementationCodeV1) -> (SourceManifestV1, SourceRemovalRequestV1) {
    use source_ownership::{RetainedRoleV1, RetainedUseV1};
    let mut units = REVIEWED
        .iter()
        .map(|(path, bytes, hash)| SourceUnitV1 {
            path: (*path).into(),
            bytes: *bytes,
            sha256: hex::decode(hash).unwrap().try_into().unwrap(),
            native_owners: if *path == HANDLER {
                vec![owner.clone()]
            } else {
                Vec::new()
            },
            retained_uses: if *path == HANDLER {
                Vec::new()
            } else {
                vec![RetainedUseV1 {
                    role: match *path {
                        READER => RetainedRoleV1::StateStorage,
                        CATALOG => RetainedRoleV1::DescriptorOrReceipt,
                        _ => RetainedRoleV1::RuntimeProfile,
                    },
                    owner: "retained original reader, dispatch or immutable catalog owner".into(),
                }]
            },
        })
        .collect::<Vec<_>>();
    units.sort_by(|left, right| left.path.cmp(&right.path));
    (
        SourceManifestV1 { version: 1, units },
        SourceRemovalRequestV1 {
            paths: vec![HANDLER.into()],
            native_owners: vec![owner],
        },
    )
}

fn read_limits() -> SourceReadLimits {
    SourceReadLimits {
        max_file_bytes: 1024 * 1024,
        max_total_bytes: 4 * 1024 * 1024,
    }
}

fn keep(chain: &CertifiedTestChain, directory: &Path) {
    let chunk = NonZeroUsize::new(4096).unwrap();
    snapshot::try_write_snapshot(chain.state(), directory, &snapshot_key(), chunk).unwrap();
    let generation =
        std::fs::read_to_string(directory.join(snapshot::SNAPSHOT_CURRENT_FILE_NAME)).unwrap();
    snapshot::checkpoint_store::keep(
        directory,
        &directory
            .join(snapshot::SNAPSHOT_GENERATIONS_DIR_NAME)
            .join(generation.trim_end()),
        chain.height(),
        chain.committed(chain.height()).block_time_ms(),
        defaults::snapshot::MAX_PAYLOAD_BYTES,
        chunk,
    )
    .unwrap();
}

// The producer is quiescent while copying. The candidate subsequently obtains the actual
// stopped-store lock through ordinary Fast admission; it has no fixture lock bypass.
fn copy_store_files(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == crate::kura::STORE_ROOT_LOCK_FILE_NAME {
            continue;
        }
        let metadata = entry.file_type().unwrap();
        assert!(!metadata.is_symlink());
        let destination = to.join(entry.file_name());
        if metadata.is_dir() {
            std::fs::create_dir(&destination).unwrap();
            copy_store_files(&entry.path(), &destination);
            std::fs::set_permissions(&destination, entry.metadata().unwrap().permissions())
                .unwrap();
        } else {
            assert!(metadata.is_file());
            std::fs::copy(entry.path(), destination).unwrap();
        }
    }
}

fn kura_config(path: &Path) -> actual::Kura {
    actual::Kura {
        init_mode: InitMode::Fast,
        store_dir: WithOrigin::inline(path.to_path_buf()),
        max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
        native_context_archive_max_bytes: defaults::kura::NATIVE_CONTEXT_ARCHIVE_MAX_BYTES,
        blocks_in_memory: defaults::kura::BLOCKS_IN_MEMORY,
        history_checkpoint_cache_capacity: defaults::kura::HISTORY_CHECKPOINT_CACHE_CAPACITY,
        block_hash_history_bytes: defaults::kura::BLOCK_HASH_HISTORY_BYTES,
        transaction_history_bytes: defaults::kura::TRANSACTION_HISTORY_BYTES,
        membership_storage: defaults::kura::MEMBERSHIP_STORAGE_POLICY,
        fastpq_artifacts: defaults::kura::FASTPQ_ARTIFACT_POLICY,
        debug_output_new_blocks: false,
        fsync_mode: FsyncMode::Batched,
        fsync_interval: defaults::kura::FSYNC_INTERVAL,
    }
}

fn snapshot_config(path: &Path) -> actual::Snapshot {
    actual::Snapshot {
        mode: Default::default(),
        create_every_ms: defaults::snapshot::CREATE_EVERY.into(),
        store_dir: WithOrigin::inline(path.to_path_buf()),
        merkle_chunk_size_bytes: NonZeroUsize::new(4096).unwrap(),
        max_payload_bytes: defaults::snapshot::MAX_PAYLOAD_BYTES,
        max_read_buffer_bytes: defaults::snapshot::MAX_READ_BUFFER_BYTES,
        resources: Default::default(),
        verification_public_key: Some(snapshot_key().public_key().clone()),
        signing_private_key: None,
        upgrade_checkpoint: Default::default(),
    }
}

#[test]
#[ignore = "explicit isolated build lane and original checkpoint rehearsal; never part of an ordinary unit-test run"]
fn build_removed_candidate_and_replay_original_oldest_floors() {
    run(build::rehearse);
}

#[test]
#[ignore = "only the isolated rebuilt candidate may run this original-checkpoint consumer"]
fn removed_candidate_restores_original_oldest_floors() {
    run(replay::rehearse);
}
