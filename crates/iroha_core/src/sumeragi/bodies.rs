//! File-backed body store of the Sumeragi driver (`specs/sumeragi.md` §7.4 body durability,
//! §8.4, §12.3 O2): the bodies of blocks the core accepted and whose height is not applied yet.
//!
//! Layout: `<root>/bodies/<instance hex>/<height, 20 digits>/<block hash hex>`, each file the
//! canonical Norito encoding of the untrusted storage `BodyRecord`, written atomically (temp file,
//! fsync, no-clobber publication, directory fsync). A body already held for a block hash is never replaced.
//! [`BodyStore::prune_through`] drops every height at or below the applied height (the
//! committed body then lives in the block store).
//!
//! The store is bounded by [`BodyLimits::max_bytes`]: a body that would exceed it fails like a
//! full disk and is retried by the persistence worker until pruning frees space. Uncommitted
//! view churn can exhaust this finite capacity. Prepared bodies remain durable because a
//! delayed valid certificate can still require them; deleting superseded in-memory proposals
//! alone is not authority to evict their bodies. Capacity exhaustion stops persistence until
//! storage is recovered, preserving custody rather than silently discarding certified work.

use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use iroha_sumeragi::{
    availability::{AvailabilitySource, AvailableBody},
    types::Hash32,
};
use mv::allocation::AllocationBudget;
use parking_lot::Mutex;

use super::{
    body_read::{BodyReadError, BodyReadJob, BodyReader, FileBodyRead},
    body_record::PreparedBodyWrite,
    driver::{SharedCrypto, traits::BodyStore},
    durable_artifact,
    records::{Faults, FsStep, NoFaults, create_dir_durable, sync_dir},
};

/// Suffix of a temporary file before its rename (left behind only by a crash).
const TEMP_SUFFIX: &str = ".tmp";

/// Bounds of a body store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyLimits {
    /// Largest total size of the stored body files, in bytes.
    pub max_bytes: u64,
}

impl Default for BodyLimits {
    /// Finite 1 GiB custody cap; prolonged uncommitted view churn can exhaust it.
    fn default() -> Self {
        Self { max_bytes: 1 << 30 }
    }
}

/// Largest body file accepted on read (far above the largest block the core accepts).
const MAX_BODY_FILE_BYTES: u64 = 80 * 1024 * 1024;

/// The driver's [`BodyStore`] over files (see the module documentation).
pub struct FileBodyStore {
    dir: PathBuf,
    hasher: SharedCrypto,
    faults: Arc<dyn Faults>,
    limits: BodyLimits,
    instance: Hash32,
    execution_budget: AllocationBudget,
    pending_write: Mutex<Option<PreparedBodyWrite>>,
}

impl core::fmt::Debug for FileBodyStore {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FileBodyStore")
            .field("dir", &self.dir)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

/// The name of a height directory.
fn height_name(height: u64) -> String {
    format!("{height:020}")
}

/// The height of a height directory's name.
fn parse_height(name: &str) -> Option<u64> {
    (name.len() == 20 && name.bytes().all(|b| b.is_ascii_digit()))
        .then(|| name.parse().ok())
        .flatten()
}

/// Sum durable body sizes under the store lock, optionally removing abandoned temp files.
fn scan(dir: &Path, clean_temps: bool) -> io::Result<u64> {
    let mut total = 0u64;
    for height in fs::read_dir(dir)? {
        let height = height?;
        if parse_height(&height.file_name().to_string_lossy()).is_none() {
            continue;
        }
        if !height.file_type()?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "body height is not a directory",
            ));
        }
        for body in fs::read_dir(height.path())? {
            let body = body?;
            if !body.file_type()?.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "body is not a regular file",
                ));
            }
            if body.file_name().to_string_lossy().ends_with(TEMP_SUFFIX) {
                if clean_temps {
                    fs::remove_file(body.path())?;
                }
                continue;
            }
            total = total.checked_add(body.metadata()?.len()).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "body storage size overflow")
            })?;
        }
    }
    Ok(total)
}

/// Serialize mutations and capacity checks across handles and processes. Each operation opens
/// its own descriptor: closing it releases the lock, including on every error path.
fn lock_store(dir: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600).custom_flags(i32::from_ne_bytes(
            rustix::fs::OFlags::NOFOLLOW.bits().to_ne_bytes(),
        ));
    }
    let file = options.open(dir.join(".custody.lock"))?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "body store lock is not a regular file",
        ));
    }
    file.lock()?;
    Ok(file)
}

impl FileBodyStore {
    /// Open (creating if needed) the body store of `instance` under `root`, hashing blocks
    /// with `hasher` (the node's `H`).
    ///
    /// # Errors
    /// An I/O failure.
    pub fn open(
        root: &Path,
        instance: &Hash32,
        hasher: SharedCrypto,
        limits: BodyLimits,
        execution_budget: AllocationBudget,
    ) -> io::Result<Self> {
        Self::open_with_faults(
            root,
            instance,
            hasher,
            limits,
            execution_budget,
            Arc::new(NoFaults),
        )
    }

    /// [`FileBodyStore::open`] with a fault-injection hook.
    ///
    /// # Errors
    /// An I/O failure.
    pub fn open_with_faults(
        root: &Path,
        instance: &Hash32,
        hasher: SharedCrypto,
        limits: BodyLimits,
        execution_budget: AllocationBudget,
        faults: Arc<dyn Faults>,
    ) -> io::Result<Self> {
        let dir = root.join("bodies").join(hex::encode(instance.0));
        durable_artifact::establish_dir(&*faults, &dir)?;
        let _guard = lock_store(&dir)?;
        scan(&dir, true)?;
        Ok(Self {
            dir,
            hasher,
            faults,
            limits,
            instance: *instance,
            execution_budget,
            pending_write: Mutex::new(None),
        })
    }

    /// The directory of this instance's bodies.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The file of the body of `(height, block_hash)`.
    pub fn body_path(&self, height: u64, block_hash: &Hash32) -> PathBuf {
        self.dir
            .join(height_name(height))
            .join(hex::encode(block_hash.0))
    }

    /// Total visible body bytes, including publication awaiting a successful durability retry.
    ///
    /// # Errors
    /// A lock or directory-read failure. Accounting errors are never treated as free space.
    pub fn bytes(&self) -> io::Result<u64> {
        let _guard = lock_store(&self.dir)?;
        scan(&self.dir, false)
    }

    /// The heights that hold bodies, ascending.
    ///
    /// # Errors
    /// An I/O failure.
    pub fn heights(&self) -> io::Result<Vec<u64>> {
        let mut heights = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            if let Some(height) = parse_height(&entry.file_name().to_string_lossy()) {
                if !entry.file_type()?.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "body height is not a directory",
                    ));
                }
                heights.push(height);
            }
        }
        heights.sort_unstable();
        Ok(heights)
    }
}

impl BodyStore for FileBodyStore {
    fn put(&self, block_hash: &Hash32, body: &AvailableBody) -> io::Result<()> {
        if body.header().instance != self.instance
            || body.header().hash(&*self.hasher) != *block_hash
            || !body.admitted_to(&self.execution_budget)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "body identity or original pool mismatch",
            ));
        }
        let mut pending = self.pending_write.lock();
        if let Some(original) = pending.as_ref() {
            if original.body() != body {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "another original body awaits durable publication",
                ));
            }
        } else {
            *pending = Some(PreparedBodyWrite::new(body.clone()));
        }
        let bytes = pending
            .as_mut()
            .expect("original prepared body retained")
            .prepare(&self.execution_budget)
            .map_err(io::Error::other)?;
        let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if len > MAX_BODY_FILE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "body exceeds the read limit",
            ));
        }
        let _guard = lock_store(&self.dir)?;
        let path = self.body_path(body.header().height, block_hash);
        let held = match fs::symlink_metadata(&path) {
            Ok(_) => {
                durable_artifact::check_exact(&path, bytes)?;
                true
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(error),
        };
        if !held
            && scan(&self.dir, false)?
                .checked_add(len)
                .is_none_or(|total| total > self.limits.max_bytes)
        {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "sumeragi body store is full",
            ));
        }
        let height_dir = self.dir.join(height_name(body.header().height));
        create_dir_durable(&*self.faults, &height_dir)?;
        durable_artifact::publish(
            &*self.faults,
            &height_dir,
            &hex::encode(block_hash.0),
            bytes,
        )?;
        *pending = None;
        Ok(())
    }

    fn prune_through(&self, height: u64) -> io::Result<()> {
        let _guard = lock_store(&self.dir)?;
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let Some(h) = parse_height(&entry.file_name().to_string_lossy()) else {
                continue;
            };
            if h > height {
                continue;
            }
            if !entry.file_type()?.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "body height is not a directory",
                ));
            }
            self.faults.before(FsStep::Remove, &entry.path())?;
            fs::remove_dir_all(entry.path())?;
        }
        sync_dir(&*self.faults, &self.dir)
    }
}

impl BodyReader for FileBodyStore {
    fn begin_read(
        &self,
        source: AvailabilitySource,
    ) -> Result<Box<dyn BodyReadJob>, BodyReadError> {
        if source.instance() != self.instance {
            return Err(BodyReadError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "body read source belongs to another instance",
            )));
        }
        let path = self.body_path(source.height(), &source.block_hash());
        Ok(Box::new(FileBodyRead::open(
            source,
            &path,
            MAX_BODY_FILE_BYTES as usize,
            self.execution_budget.clone(),
        )?))
    }
}

#[cfg(test)]
#[path = "bodies/tests.rs"]
mod tests;
