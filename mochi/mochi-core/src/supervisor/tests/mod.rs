use super::*;
use iroha_crypto::PublicKey;
use iroha_data_model::peer::PeerId;
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

include!("core_and_snapshot.rs");
include!("generation_and_runtime.rs");
