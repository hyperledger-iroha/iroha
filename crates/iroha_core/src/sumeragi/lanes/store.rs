//! The block store of a lane instance (`specs/sumeragi_lanes.md` §4.5): one durable frame per
//! lane height holding the certified block and its `CommitQC`, in a directory named by the
//! instance id. Writes are atomic (temp file, fsync, rename, directory fsync), so a crash leaves
//! either the previous tip or the new one, never a partial frame.

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use iroha_sumeragi::{
    message::{Block, Qc, SyncEntry, VoteKind},
    types::Hash32,
};
use norito::codec::{DecodeAll as _, Encode as _};
use parking_lot::{Condvar, Mutex};

use super::super::{
    driver::{SharedCrypto, traits::BlockStore},
    records::{Faults, NoFaults, create_dir_durable, write_atomic},
};

const FRAME_SUFFIX: &str = "frame";

/// File-backed [`BlockStore`] of one lane instance.
pub struct FileLaneBlockStore {
    dir: PathBuf,
    crypto: SharedCrypto,
    faults: Arc<dyn Faults>,
    height: Mutex<u64>,
    grown: Condvar,
}

impl core::fmt::Debug for FileLaneBlockStore {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("FileLaneBlockStore")
            .field("dir", &self.dir)
            .field("height", &*self.height.lock())
            .finish_non_exhaustive()
    }
}

fn frame_name(height: u64) -> String {
    format!("{height:020}.{FRAME_SUFFIX}")
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

impl FileLaneBlockStore {
    /// Open (creating it if absent) the store of lane instance `instance` under `root`: the
    /// stored heights must be exactly `1..=tip`; stray temporary files of an interrupted write
    /// are removed.
    ///
    /// # Errors
    /// An I/O failure, or stored frames that are not contiguous from height 1.
    pub fn open(root: &Path, instance: &Hash32, crypto: SharedCrypto) -> io::Result<Self> {
        Self::open_with_faults(root, instance, crypto, Arc::new(NoFaults))
    }

    /// [`Self::open`] with injected filesystem faults (tests).
    ///
    /// # Errors
    /// See [`Self::open`].
    pub fn open_with_faults(
        root: &Path,
        instance: &Hash32,
        crypto: SharedCrypto,
        faults: Arc<dyn Faults>,
    ) -> io::Result<Self> {
        let dir = root.join(hex::encode(instance.0));
        create_dir_durable(&*faults, &dir)?;
        let mut heights = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            match name.strip_suffix(&format!(".{FRAME_SUFFIX}")) {
                Some(height) => heights.push(
                    height
                        .parse::<u64>()
                        .map_err(|_| invalid(format!("unexpected lane frame {name}")))?,
                ),
                None if name.ends_with(".tmp") => fs::remove_file(entry.path())?,
                None => return Err(invalid(format!("unexpected file {name} in lane store"))),
            }
        }
        heights.sort_unstable();
        for (index, height) in heights.iter().enumerate() {
            if *height != index as u64 + 1 {
                return Err(invalid(format!(
                    "lane store heights are not contiguous from 1 (found {height})"
                )));
            }
        }
        let tip = heights.last().copied().unwrap_or(0);
        Ok(Self {
            dir,
            crypto,
            faults,
            height: Mutex::new(tip),
            grown: Condvar::new(),
        })
    }

    /// Block until the stored tip reaches `height` or `timeout` passes; whether it did.
    #[must_use]
    pub fn wait_for(&self, height: u64, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut tip = self.height.lock();
        while *tip < height {
            if self.grown.wait_until(&mut tip, deadline).timed_out() {
                return *tip >= height;
            }
        }
        true
    }

    fn read(&self, height: u64) -> io::Result<SyncEntry> {
        let bytes = fs::read(self.dir.join(frame_name(height)))?;
        let entry = SyncEntry::decode_all(&mut bytes.as_slice())
            .map_err(|error| invalid(format!("lane frame {height}: {error}")))?;
        self.check(&entry.block, &entry.commit_qc, height)?;
        Ok(entry)
    }

    fn check(&self, block: &Block, commit_qc: &Qc, height: u64) -> io::Result<()> {
        if block.header.height != height
            || commit_qc.height != height
            || commit_qc.kind != VoteKind::Commit
            || commit_qc.instance != block.header.instance
            || commit_qc.epoch != block.header.epoch
            || commit_qc.attest != block.header.attest
            || commit_qc.block_hash != block.hash(&*self.crypto)
            || !block.body_ok(&*self.crypto)
        {
            return Err(invalid(format!(
                "lane block {height} is not certified by its commit certificate"
            )));
        }
        Ok(())
    }
}

impl BlockStore for FileLaneBlockStore {
    fn height(&self) -> u64 {
        *self.height.lock()
    }

    fn entry(&self, height: u64) -> Option<SyncEntry> {
        if height == 0 || height > self.height() {
            return None;
        }
        self.read(height).ok()
    }

    fn append(&self, block: &Block, commit_qc: &Qc) -> io::Result<()> {
        let mut tip = self.height.lock();
        let height = block.header.height;
        self.check(block, commit_qc, height)?;
        if height <= *tip {
            // An exact retry of a stored height succeeds without a second write.
            let stored = self.read(height)?;
            if stored.block == *block {
                return Ok(());
            }
            return Err(invalid(format!("another lane block is stored at {height}")));
        }
        if height != tip.saturating_add(1) {
            return Err(invalid(format!(
                "lane append of {height} after tip {}",
                *tip
            )));
        }
        let frame = SyncEntry {
            block: block.clone(),
            commit_qc: commit_qc.clone(),
        }
        .encode();
        write_atomic(&*self.faults, &self.dir, &frame_name(height), &frame)?;
        *tip = height;
        self.grown.notify_all();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use iroha_sumeragi::{
        message::{BlockHeader, Qc},
        preimage::payload_hash,
        testing::FakeCrypto,
        types::{AggregateSignature, Bitmap, SIGNATURE_LEN},
    };

    use super::*;
    use crate::sumeragi::records::FsStep;

    const INSTANCE: Hash32 = Hash32([5; 32]);

    fn crypto() -> SharedCrypto {
        Arc::new(FakeCrypto::new())
    }

    fn certified(crypto: &SharedCrypto, height: u64, parent: Hash32) -> (Block, Qc) {
        let payload = vec![u8::try_from(height).expect("small"); 16];
        let header = BlockHeader {
            instance: INSTANCE,
            epoch: iroha_sumeragi::types::EpochId {
                epoch: 0,
                context: Hash32([7; 32]),
            },
            height,
            origin_view: 0,
            parent_hash: parent,
            parent_result: Hash32([2; 32]),
            payload_hash: payload_hash(&**crypto, &payload),
            payload_len: 16,
            proposer: 0,
            skipped_leaders: Vec::new(),
            attest: false,
        };
        let block = Block { header, payload };
        let qc = Qc {
            kind: VoteKind::Commit,
            instance: INSTANCE,
            epoch: iroha_sumeragi::types::EpochId {
                epoch: 0,
                context: Hash32([7; 32]),
            },
            height,
            view: 0,
            block_hash: block.hash(&**crypto),
            result: Hash32([3; 32]),
            attest: false,
            signers: Bitmap::from_indices(1, [0]).expect("bitmap"),
            agg_sig: AggregateSignature([1; SIGNATURE_LEN]),
            attestations: Vec::new(),
        };
        (block, qc)
    }

    #[test]
    fn appends_are_contiguous_idempotent_and_survive_reopening() {
        let dir = tempfile::tempdir().expect("tempdir");
        let crypto = crypto();
        let store =
            FileLaneBlockStore::open(dir.path(), &INSTANCE, Arc::clone(&crypto)).expect("open");
        assert_eq!(store.height(), 0);
        let (first, first_qc) = certified(&crypto, 1, Hash32([1; 32]));
        for mutation in 0..3 {
            let mut foreign = first_qc.clone();
            match mutation {
                0 => foreign.instance = Hash32([0x91; 32]),
                1 => foreign.epoch.context = Hash32([0x92; 32]),
                _ => foreign.attest = !first.header.attest,
            }
            assert!(store.append(&first, &foreign).is_err());
            assert_eq!(store.height(), 0);
        }
        store.append(&first, &first_qc).expect("append 1");
        store.append(&first, &first_qc).expect("an exact retry");
        let (gap, gap_qc) = certified(&crypto, 3, Hash32([1; 32]));
        assert!(store.append(&gap, &gap_qc).is_err(), "a gap");
        let (second, second_qc) = certified(&crypto, 2, first.hash(&*crypto));
        store.append(&second, &second_qc).expect("append 2");
        let (mut other, mut other_qc) = certified(&crypto, 2, Hash32([9; 32]));
        other.header.parent_hash = Hash32([8; 32]);
        other_qc.block_hash = other.hash(&*crypto);
        assert!(store.append(&other, &other_qc).is_err(), "a conflict");
        let (_, mut wrong_qc) = certified(&crypto, 3, second.hash(&*crypto));
        wrong_qc.block_hash = Hash32([7; 32]);
        assert!(
            store.append(&gap, &wrong_qc).is_err(),
            "an uncertified block"
        );
        let reopened =
            FileLaneBlockStore::open(dir.path(), &INSTANCE, Arc::clone(&crypto)).expect("reopen");
        assert_eq!(reopened.height(), 2);
        assert_eq!(reopened.entry(2).expect("entry").block, second);
        assert_eq!(reopened.entry(3), None);
        assert_eq!(reopened.entry(0), None);
    }

    struct FailRename(AtomicUsize);
    impl Faults for FailRename {
        fn before(&self, step: FsStep, _path: &Path) -> io::Result<()> {
            if step == FsStep::Rename && self.0.fetch_sub(1, Ordering::SeqCst) > 0 {
                return Err(io::Error::other("injected"));
            }
            Ok(())
        }
    }

    #[test]
    fn a_failed_write_leaves_the_previous_tip_and_no_partial_frame() {
        let dir = tempfile::tempdir().expect("tempdir");
        let crypto = crypto();
        let store = FileLaneBlockStore::open_with_faults(
            dir.path(),
            &INSTANCE,
            Arc::clone(&crypto),
            Arc::new(FailRename(AtomicUsize::new(1))),
        )
        .expect("open");
        let (first, first_qc) = certified(&crypto, 1, Hash32([1; 32]));
        assert!(store.append(&first, &first_qc).is_err());
        assert_eq!(store.height(), 0);
        let reopened =
            FileLaneBlockStore::open(dir.path(), &INSTANCE, Arc::clone(&crypto)).expect("reopen");
        assert_eq!(reopened.height(), 0, "the temporary file is not a frame");
        store.append(&first, &first_qc).expect("the retry succeeds");
        assert_eq!(store.height(), 1);
    }

    #[test]
    fn non_contiguous_stores_are_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let crypto = crypto();
        let path = dir.path().join(hex::encode(INSTANCE.0));
        fs::create_dir_all(&path).expect("dir");
        fs::write(path.join(frame_name(2)), b"x").expect("write");
        assert!(FileLaneBlockStore::open(dir.path(), &INSTANCE, crypto).is_err());
    }
}
