//! Owner-private, locked, immutable native deployment evidence.
use eyre::{Result, WrapErr as _, eyre};
use iroha_fs::{PrivateDirectory, PublishMode};
use norito::json::{JsonDeserialize, JsonSerialize};
use std::{fs::File, path::Path};

const MAX_RECORD_BYTES: usize = 128 * 1024 * 1024;
const MAX_JOURNAL_ENTRIES: usize = 1024;

pub struct Journal {
    directory: PrivateDirectory,
    _lock: File,
}

impl Journal {
    pub(super) fn open(path: &Path, create: bool) -> Result<Self> {
        let directory = if create {
            PrivateDirectory::open_or_create(path)?
        } else {
            PrivateDirectory::open(path)?
        };
        let lock = if create {
            directory.open_lock("lock")?
        } else {
            // Read-only inspection must not repair missing evidence or change journal files.
            directory.open_read("lock")?
        };
        lock.try_lock().map_err(|error| {
            eyre!("deployment journal is already in use or cannot be locked: {error}")
        })?;
        if create {
            lock.sync_all()?;
            directory.sync()?;
        }
        Ok(Self {
            directory,
            _lock: lock,
        })
    }

    pub(super) fn exists(&self, name: &str) -> Result<bool> {
        validate_name(name)?;
        match self.directory.open_read(name) {
            Ok(file) => {
                if file.metadata()?.len() > MAX_RECORD_BYTES as u64 {
                    return Err(eyre!("deployment journal record exceeds fixed byte bound"));
                }
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn require_unattempted(&self) -> Result<()> {
        for name in self.directory.entries(MAX_JOURNAL_ENTRIES)? {
            if !matches!(name.to_str(), Some("lock" | "plan.json" | "cancelled.json")) {
                return Err(eyre!(
                    "only a fully unattempted deployment may be cancelled; retained execution or unknown evidence blocks cancellation"
                ));
            }
        }
        Ok(())
    }

    pub(super) fn read<T: JsonDeserialize>(&self, name: &str) -> Result<T> {
        validate_name(name)?;
        norito::json::from_slice(&self.directory.read(name, MAX_RECORD_BYTES)?)
            .wrap_err("decode closed deployment journal record")
    }

    pub(super) fn put_exact<T: JsonSerialize>(&self, name: &str, record: &T) -> Result<()> {
        validate_name(name)?;
        let bytes = norito::json::to_vec(record)?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(eyre!("deployment journal record exceeds fixed byte bound"));
        }
        if self.exists(name)? {
            if self.directory.read(name, MAX_RECORD_BYTES)?.as_slice() != bytes.as_slice() {
                return Err(eyre!(
                    "immutable deployment journal record {name} disagrees with this operation"
                ));
            }
            return Ok(());
        }
        // Native create-new publication cannot overwrite partial or uncertain evidence. Dispatch
        // follows only after the complete record and directory-publication durability boundary.
        self.directory
            .write_atomic(name, &bytes, PublishMode::CreateNew)?;
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 80
        || !name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.')
        })
        || name == "."
        || name == ".."
    {
        return Err(eyre!("invalid fixed deployment journal record name"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "journal_tests.rs"]
mod tests;
