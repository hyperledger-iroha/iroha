//! File-backed body store of the Sumeragi driver (`specs/sumeragi.md` §7.4 body durability,
//! §8.4, §12.3 O2): the bodies of blocks the core accepted and whose height is not applied yet.
//!
//! Layout: `<root>/bodies/<instance hex>/<height, 20 digits>/<block hash hex>`, each file the
//! canonical Norito encoding of the core `Block`, written atomically and durably (temp file,
//! fsync, rename, directory fsync). A body already held for a block hash is never replaced.
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
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use iroha_sumeragi::{message::Block, types::Hash32};
use parking_lot::Mutex;

use super::{
    driver::{SharedCrypto, traits::BodyStore},
    records::{Faults, FsStep, NoFaults, create_dir_durable, sync_dir, write_atomic},
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
    /// Total size of the stored bodies.
    bytes: Mutex<u64>,
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

/// Sum the sizes of the files under `dir`, removing leftover temporary files.
fn scan(dir: &Path) -> io::Result<u64> {
    let mut total = 0u64;
    for height in fs::read_dir(dir)? {
        let height = height?;
        if !height.file_type()?.is_dir() {
            continue;
        }
        for body in fs::read_dir(height.path())? {
            let body = body?;
            if body.file_name().to_string_lossy().ends_with(TEMP_SUFFIX) {
                fs::remove_file(body.path())?;
                continue;
            }
            total = total.saturating_add(body.metadata()?.len());
        }
    }
    Ok(total)
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
    ) -> io::Result<Self> {
        Self::open_with_faults(root, instance, hasher, limits, Arc::new(NoFaults))
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
        faults: Arc<dyn Faults>,
    ) -> io::Result<Self> {
        let dir = root.join("bodies").join(hex::encode(instance.0));
        create_dir_durable(&*faults, &dir)?;
        let bytes = scan(&dir)?;
        Ok(Self {
            dir,
            hasher,
            faults,
            limits,
            bytes: Mutex::new(bytes),
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

    /// Total size of the stored bodies, in bytes.
    pub fn bytes(&self) -> u64 {
        *self.bytes.lock()
    }

    /// The heights that hold bodies, ascending.
    ///
    /// # Errors
    /// An I/O failure.
    pub fn heights(&self) -> io::Result<Vec<u64>> {
        let mut heights: Vec<u64> = fs::read_dir(&self.dir)?
            .filter_map(Result::ok)
            .filter_map(|entry| parse_height(&entry.file_name().to_string_lossy()))
            .collect();
        heights.sort_unstable();
        Ok(heights)
    }
}

impl BodyStore for FileBodyStore {
    fn put(&self, block_hash: &Hash32, block: &Block) -> io::Result<()> {
        let path = self.body_path(block.header.height, block_hash);
        if path.is_file() {
            return Ok(());
        }
        let bytes = norito::encode_canonical(block)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if self.bytes().saturating_add(len) > self.limits.max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "sumeragi body store is full",
            ));
        }
        let height_dir = self.dir.join(height_name(block.header.height));
        create_dir_durable(&*self.faults, &height_dir)?;
        write_atomic(
            &*self.faults,
            &height_dir,
            &hex::encode(block_hash.0),
            &bytes,
        )?;
        let mut total = self.bytes.lock();
        *total = total.saturating_add(len);
        Ok(())
    }

    fn get(&self, height: u64, block_hash: &Hash32) -> Option<Block> {
        let path = self.body_path(height, block_hash);
        let len = fs::metadata(&path).ok()?.len();
        if len > MAX_BODY_FILE_BYTES {
            iroha_logger::warn!(?path, len, "sumeragi body file too large; ignored");
            return None;
        }
        let bytes = fs::read(&path).ok()?;
        let block = norito::decode_canonical::<Block>(&bytes).ok();
        let intact = block
            .as_ref()
            .is_some_and(|b| b.header.height == height && b.hash(&*self.hasher) == *block_hash);
        if !intact {
            iroha_logger::warn!(?path, "sumeragi body file is corrupt; ignored");
            return None;
        }
        block
    }

    fn prune_through(&self, height: u64) -> io::Result<()> {
        let mut removed = 0u64;
        let mut result = Ok(());
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let Some(h) = parse_height(&entry.file_name().to_string_lossy()) else {
                continue;
            };
            if h > height {
                continue;
            }
            let size: u64 = fs::read_dir(entry.path())?
                .filter_map(Result::ok)
                .filter_map(|f| f.metadata().ok())
                .map(|m| m.len())
                .sum();
            let step = self
                .faults
                .before(FsStep::Remove, &entry.path())
                .and_then(|()| fs::remove_dir_all(entry.path()));
            match step {
                Ok(()) => removed = removed.saturating_add(size),
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        if removed > 0 {
            let mut total = self.bytes.lock();
            *total = total.saturating_sub(removed);
        }
        result?;
        sync_dir(&*self.faults, &self.dir)
    }
}

#[cfg(test)]
mod tests {
    use iroha_sumeragi::{
        message::BlockHeader, preimage::payload_hash, testing::FakeCrypto, types::Hash32,
    };

    use super::*;

    const I: Hash32 = Hash32([0x0A; 32]);

    fn hasher() -> SharedCrypto {
        Arc::new(FakeCrypto::new())
    }

    fn block(height: u64, payload: Vec<u8>) -> (Hash32, Block) {
        let crypto = FakeCrypto::new();
        let block = Block {
            header: BlockHeader {
                instance: I,
                height,
                origin_view: 0,
                parent_hash: Hash32([1; 32]),
                parent_result: Hash32([2; 32]),
                payload_hash: payload_hash(&crypto, &payload),
                payload_len: u32::try_from(payload.len()).unwrap(),
                proposer: 0,
                skipped_leaders: Vec::new(),
                attest: false,
            },
            payload,
        };
        (block.hash(&crypto), block)
    }

    fn open(root: &Path, limits: BodyLimits) -> FileBodyStore {
        FileBodyStore::open(root, &I, hasher(), limits).unwrap()
    }

    #[test]
    fn put_get_prune_and_reopen() {
        let root = tempfile::tempdir().unwrap();
        let store = open(root.path(), BodyLimits::default());
        assert!(store.dir().ends_with(hex::encode(I.0)));
        let (h2, b2) = block(2, vec![1; 100]);
        let (h3a, b3a) = block(3, vec![2; 50]);
        let (h3b, b3b) = block(3, Vec::new());
        for (hash, block) in [(&h2, &b2), (&h3a, &b3a), (&h3b, &b3b)] {
            store.put(hash, block).unwrap();
        }
        assert!(store.body_path(2, &h2).is_file());
        assert_eq!(store.get(2, &h2), Some(b2.clone()));
        assert_eq!(store.get(3, &h3b), Some(b3b.clone()));
        assert_eq!(store.get(3, &h2), None, "wrong height");
        assert_eq!(store.get(4, &h3a), None, "not held");
        assert_eq!(store.heights().unwrap(), vec![2, 3]);
        let bytes = store.bytes();
        assert!(bytes > 150);
        // Idempotent put; a reopened store sees the same bodies and size.
        store.put(&h2, &b2).unwrap();
        assert_eq!(store.bytes(), bytes);
        let reopened = open(root.path(), BodyLimits::default());
        assert_eq!(reopened.bytes(), bytes);
        assert_eq!(reopened.get(3, &h3a), Some(b3a));
        // Prune through the applied height.
        reopened.prune_through(2).unwrap();
        assert_eq!(reopened.heights().unwrap(), vec![3]);
        assert_eq!(reopened.get(2, &h2), None);
        assert!(reopened.bytes() < bytes);
        reopened.prune_through(10).unwrap();
        assert!(reopened.heights().unwrap().is_empty());
        assert_eq!(reopened.bytes(), 0);
        assert!(format!("{reopened:?}").contains("FileBodyStore"));
    }

    /// A body already held for a block hash is never replaced; a corrupt or foreign file is
    /// not served.
    #[test]
    fn never_replaced_and_corruption_detected() {
        let root = tempfile::tempdir().unwrap();
        let store = open(root.path(), BodyLimits::default());
        let (h, b) = block(5, vec![7; 10]);
        store.put(&h, &b).unwrap();
        let (_, other) = block(5, vec![8; 10]);
        store.put(&h, &other).unwrap();
        assert_eq!(store.get(5, &h), Some(b.clone()), "not replaced");
        let path = store.body_path(5, &h);
        fs::write(&path, norito::encode_canonical(&other).unwrap()).unwrap();
        assert_eq!(store.get(5, &h), None, "hash mismatch");
        fs::write(&path, b"junk").unwrap();
        assert_eq!(store.get(5, &h), None, "undecodable");
    }

    /// The byte bound fails a put like a full disk, and pruning frees the space.
    #[test]
    fn bounded_by_bytes() {
        let root = tempfile::tempdir().unwrap();
        let (h1, b1) = block(1, vec![1; 200]);
        let (h2, b2) = block(2, vec![2; 200]);
        let one = u64::try_from(norito::encode_canonical(&b1).unwrap().len()).unwrap();
        let store = open(
            root.path(),
            BodyLimits {
                max_bytes: 2 * one - 1,
            },
        );
        store.put(&h1, &b1).unwrap();
        let err = store.put(&h2, &b2).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::StorageFull);
        assert_eq!(store.get(2, &h2), None);
        store.prune_through(1).unwrap();
        store.put(&h2, &b2).unwrap();
        assert_eq!(store.get(2, &h2), Some(b2));
    }

    struct FailAt(FsStep);

    impl Faults for FailAt {
        fn before(&self, step: FsStep, _path: &Path) -> io::Result<()> {
            if step == self.0 {
                Err(io::Error::from_raw_os_error(28))
            } else {
                Ok(())
            }
        }
    }

    /// A failed write leaves no body and no accounted bytes; leftover temporary files are
    /// removed at open; a failed prune keeps the size accounting exact.
    #[test]
    fn failures_leave_no_partial_state() {
        let root = tempfile::tempdir().unwrap();
        open(root.path(), BodyLimits::default());
        let faulty = FileBodyStore::open_with_faults(
            root.path(),
            &I,
            hasher(),
            BodyLimits::default(),
            Arc::new(FailAt(FsStep::Rename)),
        )
        .unwrap();
        let (h, b) = block(4, vec![3; 30]);
        assert!(faulty.put(&h, &b).is_err());
        assert_eq!(faulty.get(4, &h), None);
        assert_eq!(faulty.bytes(), 0);
        let store = open(root.path(), BodyLimits::default());
        assert_eq!(store.bytes(), 0, "the temporary file was removed");
        store.put(&h, &b).unwrap();
        let faulty = FileBodyStore::open_with_faults(
            root.path(),
            &I,
            hasher(),
            BodyLimits::default(),
            Arc::new(FailAt(FsStep::Remove)),
        )
        .unwrap();
        let before = faulty.bytes();
        assert!(faulty.prune_through(4).is_err());
        assert_eq!(faulty.bytes(), before);
        assert_eq!(faulty.get(4, &h), Some(b));
    }

    #[test]
    fn height_names_round_trip() {
        for h in [0, 1, 42, u64::MAX] {
            assert_eq!(parse_height(&height_name(h)), Some(h));
        }
        assert_eq!(parse_height("12"), None);
        assert_eq!(parse_height("0000000000000000000x"), None);
        assert_eq!(BodyLimits::default().max_bytes, 1 << 30);
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("x")).unwrap();
        assert_eq!(scan(root.path()).unwrap(), 0);
    }
}
