//! Owner-only bridge-key directory of the SCCP attestor (`specs/sccp.md` §4.9).
//!
//! The directory is created `0700`; each key is one regular `0600` file `<address-hex>.key`
//! holding an [`SccpBridgeKeyFileV1`] frame whose address matches the file name. Symlinks,
//! other modes, foreign names and mismatched frames are refused. Writes are atomic: a
//! temporary file is written and fsynced, renamed into place, and the directory is fsynced.

use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::Write as _,
    os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
};

use iroha_sccp::v1::key_file::{SccpBridgeKeyFileV1, key_file_name, parse_key_file_name};

/// Why the key directory or one of its files is refused.
#[derive(Debug, thiserror::Error)]
pub(crate) enum KeyStoreError {
    /// The directory could not be created or read.
    #[error("bridge-key directory {path}: {source}")]
    Io {
        /// Offending path.
        path: PathBuf,
        /// Underlying error.
        source: std::io::Error,
    },
    /// The path is not an owner-only directory or regular file.
    #[error("bridge-key path {path} is refused: {reason}")]
    Refused {
        /// Offending path.
        path: PathBuf,
        /// Why it is refused.
        reason: &'static str,
    },
    /// A key file frame is malformed or does not match its name.
    #[error("bridge-key file {path} is malformed: {reason}")]
    Malformed {
        /// Offending path.
        path: PathBuf,
        /// What is wrong.
        reason: String,
    },
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> KeyStoreError + '_ {
    move |source| KeyStoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// The attestor's bridge-key directory.
#[derive(Debug, Clone)]
pub(crate) struct KeyStore {
    dir: PathBuf,
}

impl KeyStore {
    /// Open `dir`, creating it `0700` when absent.
    ///
    /// # Errors
    ///
    /// Fails when the directory cannot be created, is a symlink or not a directory, or grants
    /// any access to group or others.
    pub(crate) fn open(dir: &Path) -> Result<Self, KeyStoreError> {
        if fs::symlink_metadata(dir).is_err() {
            DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)
                .map_err(io(dir))?;
        }
        let metadata = fs::symlink_metadata(dir).map_err(io(dir))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(KeyStoreError::Refused {
                path: dir.to_path_buf(),
                reason: "not a directory (symlinks are refused)",
            });
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(KeyStoreError::Refused {
                path: dir.to_path_buf(),
                reason: "grants access to group or others (expected 0700)",
            });
        }
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// Return the directory path.
    #[cfg(test)]
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// Load every key file, returning the keys and the refusals of files that are not keys.
    ///
    /// Hidden files (temporary writes) are skipped silently.
    ///
    /// # Errors
    ///
    /// Fails only when the directory cannot be listed.
    pub(crate) fn load(
        &self,
    ) -> Result<(Vec<SccpBridgeKeyFileV1>, Vec<KeyStoreError>), KeyStoreError> {
        let mut keys = Vec::new();
        let mut refused = Vec::new();
        let mut entries: Vec<_> = fs::read_dir(&self.dir)
            .map_err(io(&self.dir))?
            .filter_map(Result::ok)
            .collect();
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                refused.push(KeyStoreError::Refused {
                    path: entry.path(),
                    reason: "file name is not UTF-8",
                });
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            match Self::load_file(&entry.path(), name) {
                Ok(key) => keys.push(key),
                Err(error) => refused.push(error),
            }
        }
        Ok((keys, refused))
    }

    fn load_file(path: &Path, name: &str) -> Result<SccpBridgeKeyFileV1, KeyStoreError> {
        let expected = parse_key_file_name(name).ok_or_else(|| KeyStoreError::Refused {
            path: path.to_path_buf(),
            reason: "not a `<address-hex>.key` file",
        })?;
        let metadata = fs::symlink_metadata(path).map_err(io(path))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(KeyStoreError::Refused {
                path: path.to_path_buf(),
                reason: "not a regular file (symlinks are refused)",
            });
        }
        if metadata.permissions().mode() & 0o777 != 0o600 {
            return Err(KeyStoreError::Refused {
                path: path.to_path_buf(),
                reason: "mode is not 0600",
            });
        }
        let bytes = zeroize::Zeroizing::new(fs::read(path).map_err(io(path))?);
        let key =
            SccpBridgeKeyFileV1::from_frame(&bytes).map_err(|error| KeyStoreError::Malformed {
                path: path.to_path_buf(),
                reason: error.to_string(),
            })?;
        let address = key.address().map_err(|error| KeyStoreError::Malformed {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
        if address != expected {
            return Err(KeyStoreError::Malformed {
                path: path.to_path_buf(),
                reason: "the key's address differs from its file name".to_owned(),
            });
        }
        Ok(key)
    }

    /// Write `key` atomically as a new `0600` file.
    ///
    /// # Errors
    ///
    /// Fails when the key cannot be encoded, a file with its name exists, or any write,
    /// fsync or rename fails.
    pub(crate) fn write(&self, key: &SccpBridgeKeyFileV1) -> Result<PathBuf, KeyStoreError> {
        let address = key.address().map_err(|error| KeyStoreError::Malformed {
            path: self.dir.clone(),
            reason: error.to_string(),
        })?;
        let name = key_file_name(&address);
        let path = self.dir.join(&name);
        if fs::symlink_metadata(&path).is_ok() {
            return Err(KeyStoreError::Refused {
                path,
                reason: "a key file with this address already exists",
            });
        }
        let frame = key.to_frame().map_err(|error| KeyStoreError::Malformed {
            path: path.clone(),
            reason: error.to_string(),
        })?;
        let temporary = self.dir.join(format!(".{name}.tmp-{}", std::process::id()));
        let written = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(frame.as_slice())?;
            file.sync_all()?;
            fs::rename(&temporary, &path)?;
            File::open(&self.dir)?.sync_all()
        })();
        if let Err(source) = written {
            let _ = fs::remove_file(&temporary);
            return Err(KeyStoreError::Io { path, source });
        }
        Ok(path)
    }

    /// Delete the key file of `address` and fsync the directory.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be removed.
    pub(crate) fn delete(&self, address: &[u8; 20]) -> Result<(), KeyStoreError> {
        let path = self.dir.join(key_file_name(address));
        fs::remove_file(&path).map_err(io(&path))?;
        File::open(&self.dir)
            .and_then(|dir| dir.sync_all())
            .map_err(io(&self.dir))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(seed: u8) -> SccpBridgeKeyFileV1 {
        SccpBridgeKeyFileV1::new([seed; 32], u64::from(seed)).expect("valid secret")
    }

    #[test]
    fn keys_are_written_owner_only_and_reloaded() {
        let root = tempfile::tempdir().expect("tempdir");
        let dir = root.path().join("sccp").join("bridge-keys");
        let store = KeyStore::open(&dir).expect("created");
        assert_eq!(
            fs::metadata(&dir).expect("dir").permissions().mode() & 0o777,
            0o700
        );
        let first = key(3);
        let path = store.write(&first).expect("write");
        assert_eq!(
            fs::metadata(&path).expect("file").permissions().mode() & 0o777,
            0o600
        );
        store.write(&first).expect_err("no overwrite");
        store.write(&key(4)).expect("second");
        let (keys, refused) = store.load().expect("load");
        assert!(refused.is_empty(), "{refused:?}");
        let addresses: Vec<_> = keys
            .iter()
            .map(|key| key.address().expect("address"))
            .collect();
        assert!(addresses.contains(&first.address().expect("address")));
        assert_eq!(keys.len(), 2);
        store
            .delete(&first.address().expect("address"))
            .expect("delete");
        assert_eq!(store.load().expect("load").0.len(), 1);
    }

    #[test]
    fn open_directories_symlinks_and_foreign_files_are_refused() {
        let root = tempfile::tempdir().expect("tempdir");
        let open = root.path().join("open");
        fs::create_dir(&open).expect("dir");
        fs::set_permissions(&open, fs::Permissions::from_mode(0o755)).expect("chmod");
        KeyStore::open(&open).expect_err("group-readable directory");

        let dir = root.path().join("keys");
        let store = KeyStore::open(&dir).expect("created");
        let good = key(5);
        let good_path = store.write(&good).expect("write");
        // A symlinked key, a loose mode and a foreign name are refused; the good key loads.
        let other = key(6);
        let other_name = key_file_name(&other.address().expect("address"));
        std::os::unix::fs::symlink(&good_path, dir.join(&other_name)).expect("symlink");
        let loose = key(7);
        let loose_path = store.write(&loose).expect("write");
        fs::set_permissions(&loose_path, fs::Permissions::from_mode(0o644)).expect("chmod");
        fs::write(dir.join("notes.txt"), b"hello").expect("foreign");
        let (keys, refused) = store.load().expect("load");
        assert_eq!(keys.len(), 1);
        assert_eq!(
            keys[0].address().expect("address"),
            good.address().expect("address")
        );
        assert_eq!(refused.len(), 3, "{refused:?}");

        let linked = root.path().join("linked");
        std::os::unix::fs::symlink(&dir, &linked).expect("symlink");
        KeyStore::open(&linked).expect_err("symlinked directory");
    }

    #[test]
    fn a_frame_under_the_wrong_name_is_malformed() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = KeyStore::open(&root.path().join("keys")).expect("created");
        let key = key(8);
        let other = key_file_name(&self::key(9).address().expect("address"));
        let path = store.dir().join(other);
        let frame = key.to_frame().expect("frame");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .expect("file");
        file.write_all(frame.as_slice()).expect("write");
        let (keys, refused) = store.load().expect("load");
        assert!(keys.is_empty());
        assert!(matches!(
            refused.as_slice(),
            [KeyStoreError::Malformed { .. }]
        ));
    }
}
