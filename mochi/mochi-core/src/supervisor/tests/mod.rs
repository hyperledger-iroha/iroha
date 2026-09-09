//! Supervisor generation, process, and storage contract regressions.

use super::genesis_material::{
    GENERATED_GENESIS_RECORD_MAX_BYTES_V1, GENESIS_EXPECTED_HASH_FILE_NAME,
    GENESIS_PUBLIC_KEY_FILE_NAME, TEST_FINALIZE_KAGAMI_STUB_SIGNATURE, TemporaryGenesisKeyFile,
    read_generated_genesis_record, read_generated_genesis_record_inner,
    validate_kagami_manifest_chain,
};
use super::snapshot_restore::{
    SNAPSHOT_RESTORE_COMMIT_FILE_NAME, SNAPSHOT_RESTORE_JOURNAL_FILE_NAME, StagedPeerRestore,
    write_pending_restore_journal, write_restore_commit_marker, write_restore_commit_marker_with,
};
use super::*;
use iroha_crypto::{HashOf, PublicKey};
use iroha_data_model::{
    block::BlockHeader, isi::kagemusha_v1::KagemushaMintFinalityGenesisParametersV1, peer::PeerId,
};
use iroha_genesis::{GenesisTopologyEntry, RawGenesisTransaction};
#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt, symlink};
use std::{
    collections::HashSet,
    env,
    ffi::OsString,
    io::ErrorKind,
    net::TcpListener,
    path::Path,
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tokio::runtime::Runtime;

fn collect_files_recursive(
    root: &Path,
    files: &mut Vec<std::path::PathBuf>,
) -> std::io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files_recursive(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

mod genesis;
use genesis::{KagamiStub, StandaloneKagamiStub};

include!("core_and_snapshot.rs");
include!("generation_and_runtime.rs");

mod stream_reader;

mod port_allocation;
