//! Native Windows local-bridge readers retaining file and ancestry through disk work.

use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use iroha_fs::{FileSnapshot, ReaderDirectory, RetainedFile};
use tokio::io::{AsyncWrite, AsyncWriteExt};

use super::BridgeFileError;

const COPY_CHUNK_BYTES: usize = 64 * 1024;

struct BridgeOriginal {
    file: RetainedFile,
    // Each application directory must remain current-user owned, while ancestors above the
    // configured root may have trusted system ownership under the native reader policy.
    directories: Vec<ReaderDirectory>,
    snapshot: FileSnapshot,
}

impl BridgeOriginal {
    fn revalidate(&self) -> Result<(), BridgeFileError> {
        for directory in &self.directories {
            directory
                .require_current_owner()
                .map_err(|_| BridgeFileError::Changed)?;
        }
        self.file
            .require_current_owner()
            .map_err(|_| BridgeFileError::Changed)?;
        if self.file.snapshot().map_err(|_| BridgeFileError::Changed)? != self.snapshot {
            return Err(BridgeFileError::Changed);
        }
        Ok(())
    }
}

pub(super) struct OpenBridgeFile {
    original: Arc<BridgeOriginal>,
    pub(super) length: u64,
}

pub(super) fn canonical_bridge_root(path: &Path) -> Result<PathBuf, String> {
    let root = ReaderDirectory::open(path)
        .map_err(|error| format!("bridge root is unavailable or unsafe: {error}"))?;
    root.require_current_owner()
        .map_err(|error| format!("bridge root requires current-user custody: {error}"))?;
    Ok(root.path().to_owned())
}

fn open_original(
    root: &Path,
    path: &Path,
    maximum: u64,
) -> Result<OpenBridgeFile, BridgeFileError> {
    let components = path
        .strip_prefix(root)
        .map_err(|_| BridgeFileError::Unsafe)?
        .components()
        .map(|component| match component {
            Component::Normal(name) => Ok(name),
            _ => Err(BridgeFileError::Unsafe),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (name, ancestors) = components.split_last().ok_or(BridgeFileError::Unsafe)?;
    let root = ReaderDirectory::open(root).map_err(|_| BridgeFileError::Unsafe)?;
    root.require_current_owner()
        .map_err(|_| BridgeFileError::Unsafe)?;
    let mut directories = vec![root];
    for segment in ancestors {
        let directory = directories
            .last()
            .ok_or(BridgeFileError::Unsafe)?
            .open_child_optional(segment)
            .map_err(|_| BridgeFileError::Unsafe)?
            .ok_or(BridgeFileError::NotFound)?;
        directory
            .require_current_owner()
            .map_err(|_| BridgeFileError::Unsafe)?;
        directories.push(directory);
    }
    let file = directories
        .last()
        .ok_or(BridgeFileError::Unsafe)?
        .open_retained_regular_optional(name)
        .map_err(|_| BridgeFileError::Unsafe)?
        .ok_or(BridgeFileError::NotFound)?;
    file.require_current_owner()
        .map_err(|_| BridgeFileError::Unsafe)?;
    let snapshot = file.snapshot().map_err(|_| BridgeFileError::Unsafe)?;
    let length = file
        .file()
        .metadata()
        .map_err(|_| BridgeFileError::Io)?
        .len();
    let original = Arc::new(BridgeOriginal {
        file,
        directories,
        snapshot,
    });
    original.revalidate()?;
    if length > maximum {
        return Err(BridgeFileError::TooLarge { length, maximum });
    }
    Ok(OpenBridgeFile { original, length })
}

pub(super) async fn open_bridge_file(
    root: &Path,
    path: PathBuf,
    maximum: u64,
) -> Result<OpenBridgeFile, BridgeFileError> {
    let root = root.to_owned();
    tokio::task::spawn_blocking(move || open_original(&root, &path, maximum))
        .await
        .map_err(|_| BridgeFileError::Io)?
}

async fn read_chunk(
    original: Arc<BridgeOriginal>,
    offset: u64,
    length: usize,
) -> Result<Vec<u8>, BridgeFileError> {
    // A cancelled async request cannot cancel an already-running blocking read. Move the
    // complete original owner into that work, so its file and ancestry survive until it ends.
    tokio::task::spawn_blocking(move || {
        let mut bytes = vec![0; length];
        iroha_fs::read_exact_at(original.file.file(), &mut bytes, offset).map_err(|error| {
            if error.kind() == std::io::ErrorKind::UnexpectedEof {
                BridgeFileError::Truncated
            } else {
                BridgeFileError::Io
            }
        })?;
        Ok(bytes)
    })
    .await
    .map_err(|_| BridgeFileError::Io)?
}

pub(super) async fn copy_bridge_file(
    opened: &OpenBridgeFile,
    writer: &mut (impl AsyncWrite + Unpin),
) -> Result<u64, BridgeFileError> {
    let mut copied = 0;
    while copied < opened.length {
        let length = (opened.length - copied).min(COPY_CHUNK_BYTES as u64) as usize;
        let bytes = read_chunk(Arc::clone(&opened.original), copied, length).await?;
        writer
            .write_all(&bytes)
            .await
            .map_err(|_| BridgeFileError::Io)?;
        copied += bytes.len() as u64;
    }
    Ok(copied)
}

pub(super) async fn verify_bridge_file_unchanged(
    opened: &OpenBridgeFile,
) -> Result<(), BridgeFileError> {
    let original = Arc::clone(&opened.original);
    tokio::task::spawn_blocking(move || original.revalidate())
        .await
        .map_err(|_| BridgeFileError::Changed)?
}

#[cfg(test)]
#[path = "bridge_files_windows_tests.rs"]
mod tests;
