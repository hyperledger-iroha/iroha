//! Local Norito journal for pending queue routing plans.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::transaction::{SignedTransaction, TransactionEntrypoint};
use norito::codec::{Decode, Encode};

use super::RoutingPlan;

/// Version of queue plan journal records.
pub const QUEUE_PLAN_JOURNAL_VERSION: u16 = 1;

type SignedTxHash = HashOf<SignedTransaction>;

/// Pending transaction routing-plan journal record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct QueuePlanJournalRecordV1 {
    /// Record format version.
    pub version: u16,
    /// Canonical transaction entrypoint.
    pub entrypoint: TransactionEntrypoint,
    /// Compatibility signed-transaction hash used by queue indexes.
    pub signed_transaction_hash: SignedTxHash,
    /// Full routing plan admitted for this transaction.
    pub routing_plan: RoutingPlan,
    /// Cached gossip payload bytes for retransmission.
    pub gossip_payload: Vec<u8>,
    /// Local enqueue timestamp in milliseconds.
    pub enqueue_timestamp_ms: u64,
}

impl QueuePlanJournalRecordV1 {
    /// Construct a version-1 journal record.
    #[must_use]
    pub fn new(
        entrypoint: TransactionEntrypoint,
        signed_transaction_hash: SignedTxHash,
        routing_plan: RoutingPlan,
        gossip_payload: Arc<Vec<u8>>,
        enqueue_timestamp_ms: u64,
    ) -> Self {
        Self {
            version: QUEUE_PLAN_JOURNAL_VERSION,
            entrypoint,
            signed_transaction_hash,
            routing_plan,
            gossip_payload: gossip_payload.as_ref().clone(),
            enqueue_timestamp_ms,
        }
    }

    /// Digest paired with removals to avoid deleting a re-admitted hash with a new plan.
    #[must_use]
    pub fn plan_digest(&self) -> Hash {
        self.routing_plan.digest()
    }
}

/// One append-only queue plan journal frame.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum QueuePlanJournalFrameV1 {
    /// Add or replace a pending queue record.
    Put(QueuePlanJournalRecordV1),
    /// Tombstone a pending queue record.
    Remove {
        /// Compatibility signed-transaction hash.
        hash: SignedTxHash,
        /// Full routing-plan digest that was removed.
        plan_digest: Hash,
    },
}

/// Append-only queue plan journal with atomic compaction.
pub struct QueuePlanJournal {
    path: PathBuf,
    max_bytes_before_compact: u64,
    durable_writes: bool,
    file: File,
    tombstones: u64,
}

/// Deferred durability work requested by a journal append.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueuePlanJournalFlush {
    sync_data: bool,
    compact: bool,
}

impl QueuePlanJournalFlush {
    /// Merge two deferred flush requests.
    #[must_use]
    pub fn combine(self, other: Self) -> Self {
        Self {
            sync_data: self.sync_data || other.sync_data,
            compact: self.compact || other.compact,
        }
    }

    /// Returns whether there is any deferred work.
    #[must_use]
    pub fn is_needed(self) -> bool {
        self.sync_data || self.compact
    }

    /// Returns whether the journal data file should be synced.
    #[must_use]
    pub fn sync_data(self) -> bool {
        self.sync_data
    }

    /// Returns whether journal compaction should be considered.
    #[must_use]
    pub fn compact(self) -> bool {
        self.compact
    }
}

impl QueuePlanJournal {
    /// Open or create a queue plan journal at `path`.
    ///
    /// # Errors
    /// Returns I/O errors from directory creation or file opening.
    pub fn open(
        path: impl AsRef<Path>,
        max_bytes_before_compact: u64,
        durable_writes: bool,
    ) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        repair_incomplete_tail(&path, durable_writes)?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;
        Ok(Self {
            path,
            max_bytes_before_compact,
            durable_writes,
            file,
            tombstones: 0,
        })
    }

    /// Append a put frame and return deferred durability work for the caller.
    ///
    /// # Errors
    /// Returns I/O or Norito encoding errors mapped to I/O.
    pub fn put_deferred_flush(
        &mut self,
        record: QueuePlanJournalRecordV1,
    ) -> io::Result<QueuePlanJournalFlush> {
        write_frame(&mut self.file, &QueuePlanJournalFrameV1::Put(record))?;
        Ok(QueuePlanJournalFlush {
            sync_data: self.durable_writes,
            compact: false,
        })
    }

    /// Append multiple remove frames and return deferred durability work for the caller.
    ///
    /// # Errors
    /// Returns I/O or Norito encoding errors mapped to I/O.
    pub fn remove_many_deferred_flush<I>(
        &mut self,
        removals: I,
    ) -> io::Result<QueuePlanJournalFlush>
    where
        I: IntoIterator<Item = (SignedTxHash, Hash)>,
    {
        let mut wrote_any = false;
        for (hash, plan_digest) in removals {
            self.tombstones = self.tombstones.saturating_add(1);
            write_frame(
                &mut self.file,
                &QueuePlanJournalFrameV1::Remove { hash, plan_digest },
            )?;
            wrote_any = true;
        }
        if !wrote_any {
            return Ok(QueuePlanJournalFlush::default());
        }
        Ok(QueuePlanJournalFlush {
            sync_data: self.durable_writes,
            compact: true,
        })
    }

    /// Run deferred durability work produced by append methods.
    ///
    /// # Errors
    /// Returns sync or compaction I/O errors.
    #[cfg(test)]
    pub fn flush_deferred(&mut self, flush: QueuePlanJournalFlush) -> io::Result<()> {
        if !flush.is_needed() {
            return Ok(());
        }
        if flush.sync_data {
            self.file.sync_data()?;
        }
        if flush.compact {
            self.compact_if_needed()?;
        }
        Ok(())
    }

    /// Clone the append file handle so callers can sync without holding the journal mutex.
    ///
    /// # Errors
    /// Returns I/O errors from duplicating the file handle.
    pub fn sync_file_clone(&self) -> io::Result<File> {
        self.file.try_clone()
    }

    /// Replay live records from disk.
    ///
    /// # Errors
    /// Returns I/O errors or malformed-frame decode errors mapped to I/O.
    pub fn replay(&self) -> io::Result<Vec<QueuePlanJournalRecordV1>> {
        let mut live = BTreeMap::<(SignedTxHash, Hash), QueuePlanJournalRecordV1>::new();
        for frame in read_frames(&self.path)? {
            match frame {
                QueuePlanJournalFrameV1::Put(record) => {
                    if record.version == QUEUE_PLAN_JOURNAL_VERSION {
                        live.insert(
                            (record.signed_transaction_hash, record.plan_digest()),
                            record,
                        );
                    }
                }
                QueuePlanJournalFrameV1::Remove { hash, plan_digest } => {
                    live.remove(&(hash, plan_digest));
                }
            }
        }
        Ok(live.into_values().collect())
    }

    /// Compact the journal by atomically rewriting live records when worthwhile.
    ///
    /// # Errors
    /// Returns I/O errors from replay, rewrite, sync, or rename.
    pub fn compact_if_needed(&mut self) -> io::Result<()> {
        let size = self.file.metadata()?.len();
        let live = self.replay()?;
        if self.tombstones <= live.len() as u64 && size <= self.max_bytes_before_compact {
            return Ok(());
        }
        let tmp = self.path.with_extension("tmp");
        {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&tmp)?;
            for record in live {
                write_frame(&mut file, &QueuePlanJournalFrameV1::Put(record))?;
            }
            if self.durable_writes {
                file.sync_all()?;
            }
        }
        fs::rename(&tmp, &self.path)?;
        self.file = OpenOptions::new()
            .append(true)
            .read(true)
            .open(&self.path)?;
        self.tombstones = 0;
        Ok(())
    }
}

fn write_frame(file: &mut File, frame: &QueuePlanJournalFrameV1) -> io::Result<()> {
    let bytes = norito::to_bytes(frame).map_err(io::Error::other)?;
    let len = u32::try_from(bytes.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "queue journal frame too large"))?;
    file.write_all(&len.to_le_bytes())?;
    file.write_all(&bytes)?;
    Ok(())
}

fn repair_incomplete_tail(path: &Path, durable_writes: bool) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    let file_len = file.metadata()?.len();
    let mut position = 0_u64;
    let mut valid_end = 0_u64;

    while position < file_len {
        let remaining = file_len.saturating_sub(position);
        if remaining < 4 {
            truncate_journal_tail(&mut file, valid_end, durable_writes)?;
            return Ok(());
        }

        file.seek(SeekFrom::Start(position))?;
        let mut len_bytes = [0_u8; 4];
        file.read_exact(&mut len_bytes)?;
        let payload_len = u32::from_le_bytes(len_bytes) as u64;
        let payload_start = position.checked_add(4).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "queue journal frame length overflow",
            )
        })?;
        let frame_end = payload_start.checked_add(payload_len).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "queue journal frame length overflow",
            )
        })?;

        if frame_end > file_len {
            truncate_journal_tail(&mut file, position, durable_writes)?;
            return Ok(());
        }

        let mut bytes = vec![0_u8; payload_len as usize];
        file.read_exact(&mut bytes)?;
        norito::decode_from_bytes::<QueuePlanJournalFrameV1>(&bytes).map_err(io::Error::other)?;

        valid_end = frame_end;
        position = frame_end;
    }

    Ok(())
}

fn truncate_journal_tail(file: &mut File, valid_end: u64, durable_writes: bool) -> io::Result<()> {
    file.set_len(valid_end)?;
    if durable_writes {
        file.sync_all()?;
    }
    Ok(())
}

fn read_frames(path: &Path) -> io::Result<Vec<QueuePlanJournalFrameV1>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let mut reader = BufReader::new(file);
    let mut frames = Vec::new();
    loop {
        let mut len_bytes = [0_u8; 4];
        match reader.read_exact(&mut len_bytes) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err),
        }
        let len = u32::from_le_bytes(len_bytes) as usize;
        let mut bytes = vec![0_u8; len];
        match reader.read_exact(&mut bytes) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err),
        }
        let frame = norito::decode_from_bytes::<QueuePlanJournalFrameV1>(&bytes)
            .map_err(io::Error::other)?;
        frames.push(frame);
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use std::{fs::OpenOptions, io::Write};

    use iroha_data_model::{
        asset::AssetDefinitionId,
        domain::DomainId,
        escrow::EscrowId,
        isi::{InstructionBox, Log, escrow::OpenAssetLock},
        name::Name,
        transaction::TransactionBuilder,
    };
    use iroha_logger::Level;
    use iroha_test_samples::gen_account_in;

    use super::*;

    fn record(label: &str) -> QueuePlanJournalRecordV1 {
        let instruction: InstructionBox = Log::new(Level::INFO, label.to_owned()).into();
        record_with_instruction(label, instruction)
    }

    fn record_with_instruction(
        label: &str,
        instruction: InstructionBox,
    ) -> QueuePlanJournalRecordV1 {
        let chain_id = "00000000-0000-0000-0000-000000000000"
            .parse()
            .expect("chain id");
        let (account_id, keypair) = gen_account_in(label);
        let tx = TransactionBuilder::new(chain_id, account_id)
            .with_instructions([instruction])
            .sign(keypair.private_key());
        let accepted = crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(tx));
        QueuePlanJournalRecordV1::new(
            accepted.entrypoint().clone(),
            accepted.hash(),
            RoutingPlan::single(super::super::RoutingDecision::default()),
            accepted.entrypoint_bytes(),
            42,
        )
    }

    fn asset_lock_record(label: &str) -> QueuePlanJournalRecordV1 {
        let escrow_id = EscrowId::new(Hash::new(format!("{label}-asset-lock")));
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "xor".parse::<Name>().expect("asset name"),
        );
        let (destination, _) = gen_account_in(&format!("{label}-destination"));
        let instruction: InstructionBox = OpenAssetLock::new(
            escrow_id,
            asset_definition,
            destination,
            iroha_primitives::numeric::Numeric::from(20_u64),
        )
        .into();
        record_with_instruction(label, instruction)
    }

    #[test]
    fn journal_replays_puts_and_removes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("queue-plan.norito");
        let first = record("first");
        let second = record("second");
        {
            let mut journal =
                QueuePlanJournal::open(&path, 1024 * 1024, true).expect("open journal");
            let first_flush = journal
                .put_deferred_flush(first.clone())
                .expect("put first");
            let second_flush = journal
                .put_deferred_flush(second.clone())
                .expect("put second");
            let remove_flush = journal
                .remove_many_deferred_flush([(first.signed_transaction_hash, first.plan_digest())])
                .expect("remove first");
            journal
                .flush_deferred(first_flush.combine(second_flush).combine(remove_flush))
                .expect("flush journal");
        }
        let journal = QueuePlanJournal::open(&path, 1024 * 1024, true).expect("reopen journal");
        assert_eq!(journal.replay().expect("replay"), vec![second]);
    }

    #[test]
    fn journal_replays_asset_lock_instruction_records() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("queue-plan-asset-lock.norito");
        let record = asset_lock_record("asset-lock");
        {
            let mut journal =
                QueuePlanJournal::open(&path, 1024 * 1024, true).expect("open journal");
            let flush = journal
                .put_deferred_flush(record.clone())
                .expect("put asset lock");
            journal.flush_deferred(flush).expect("flush journal");
        }

        let journal = QueuePlanJournal::open(&path, 1024 * 1024, true).expect("reopen journal");
        assert_eq!(journal.replay().expect("replay"), vec![record]);
    }

    #[test]
    fn journal_deferred_flush_preserves_replay_order() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("queue-plan-deferred.norito");
        let first = record("deferred-first");
        let second = record("deferred-second");
        {
            let mut journal =
                QueuePlanJournal::open(&path, 1024 * 1024, true).expect("open journal");
            let first_flush = journal
                .put_deferred_flush(first.clone())
                .expect("deferred first put");
            assert!(first_flush.is_needed());
            let second_flush = journal
                .put_deferred_flush(second.clone())
                .expect("deferred second put");
            let remove_flush = journal
                .remove_many_deferred_flush([(first.signed_transaction_hash, first.plan_digest())])
                .expect("deferred remove");
            journal
                .flush_deferred(first_flush.combine(second_flush).combine(remove_flush))
                .expect("flush deferred writes");
        }

        let journal = QueuePlanJournal::open(&path, 1024 * 1024, true).expect("reopen journal");
        assert_eq!(journal.replay().expect("replay"), vec![second]);
    }

    #[test]
    fn journal_sync_file_clone_can_flush_without_mutable_journal_borrow() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("queue-plan-sync-clone.norito");
        let record = record("sync-clone");
        {
            let mut journal =
                QueuePlanJournal::open(&path, 1024 * 1024, true).expect("open journal");
            let flush = journal
                .put_deferred_flush(record.clone())
                .expect("append record");
            assert!(flush.sync_data());
            assert!(!flush.compact());
            let sync_file = journal.sync_file_clone().expect("clone sync handle");
            sync_file.sync_data().expect("sync cloned handle");
        }

        let journal = QueuePlanJournal::open(&path, 1024 * 1024, true).expect("reopen journal");
        assert_eq!(journal.replay().expect("replay"), vec![record]);
    }

    #[test]
    fn journal_remove_many_replays_without_removed_records() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("queue-plan.norito");
        let first = record("first");
        let second = record("second");
        let third = record("third");
        {
            let mut journal =
                QueuePlanJournal::open(&path, 1024 * 1024, true).expect("open journal");
            let first_flush = journal
                .put_deferred_flush(first.clone())
                .expect("put first");
            let second_flush = journal
                .put_deferred_flush(second.clone())
                .expect("put second");
            let third_flush = journal
                .put_deferred_flush(third.clone())
                .expect("put third");
            let remove_flush = journal
                .remove_many_deferred_flush([
                    (first.signed_transaction_hash, first.plan_digest()),
                    (second.signed_transaction_hash, second.plan_digest()),
                ])
                .expect("remove first two records");
            journal
                .flush_deferred(
                    first_flush
                        .combine(second_flush)
                        .combine(third_flush)
                        .combine(remove_flush),
                )
                .expect("flush journal");
        }

        let journal = QueuePlanJournal::open(&path, 1024 * 1024, true).expect("reopen journal");
        assert_eq!(journal.replay().expect("replay"), vec![third.clone()]);
        assert_eq!(
            read_frames(&path).expect("read compacted frames"),
            vec![QueuePlanJournalFrameV1::Put(third)]
        );
    }

    #[test]
    fn journal_open_truncates_torn_payload_tail_before_append() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("queue-plan.norito");
        let first = record("first");
        let second = record("second");
        let second_bytes =
            norito::to_bytes(&QueuePlanJournalFrameV1::Put(second.clone())).expect("encode second");

        let valid_len;
        {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&path)
                .expect("open raw journal");
            write_frame(&mut file, &QueuePlanJournalFrameV1::Put(first.clone()))
                .expect("write first frame");
            valid_len = file.metadata().expect("valid metadata").len();
            let second_len = u32::try_from(second_bytes.len()).expect("second frame length");
            file.write_all(&second_len.to_le_bytes())
                .expect("write torn length");
            file.write_all(&second_bytes[..second_bytes.len() / 2])
                .expect("write torn payload");
        }
        assert!(
            path.metadata().expect("torn metadata").len() > valid_len,
            "test setup should leave a torn tail"
        );

        let mut journal = QueuePlanJournal::open(&path, 1024 * 1024, true).expect("repair journal");
        assert_eq!(
            path.metadata().expect("repaired metadata").len(),
            valid_len,
            "opening should truncate the incomplete trailing frame"
        );
        assert_eq!(
            journal.replay().expect("replay repaired"),
            vec![first.clone()]
        );

        let flush = journal
            .put_deferred_flush(second.clone())
            .expect("append after repair");
        journal.flush_deferred(flush).expect("flush appended frame");
        let replayed = journal.replay().expect("replay appended");
        assert_eq!(replayed.len(), 2);
        assert!(replayed.contains(&first));
        assert!(replayed.contains(&second));
    }
}
