//! Local Norito journal for pending queue routing plans.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, BufReader, Read, Write},
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

    /// Append a put frame.
    ///
    /// # Errors
    /// Returns I/O or Norito encoding errors mapped to I/O.
    pub fn put(&mut self, record: QueuePlanJournalRecordV1) -> io::Result<()> {
        self.append_frame(&QueuePlanJournalFrameV1::Put(record))
    }

    /// Append a remove frame.
    ///
    /// # Errors
    /// Returns I/O or Norito encoding errors mapped to I/O.
    pub fn remove(&mut self, hash: SignedTxHash, plan_digest: Hash) -> io::Result<()> {
        self.tombstones = self.tombstones.saturating_add(1);
        self.append_frame(&QueuePlanJournalFrameV1::Remove { hash, plan_digest })
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

    fn append_frame(&mut self, frame: &QueuePlanJournalFrameV1) -> io::Result<()> {
        write_frame(&mut self.file, frame)?;
        if self.durable_writes {
            self.file.sync_data()?;
        }
        self.compact_if_needed()
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
        reader.read_exact(&mut bytes)?;
        let frame = norito::decode_from_bytes::<QueuePlanJournalFrameV1>(&bytes)
            .map_err(io::Error::other)?;
        frames.push(frame);
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{isi::Log, transaction::TransactionBuilder};
    use iroha_logger::Level;
    use iroha_test_samples::gen_account_in;

    use super::*;

    fn record(label: &str) -> QueuePlanJournalRecordV1 {
        let chain_id = "00000000-0000-0000-0000-000000000000"
            .parse()
            .expect("chain id");
        let (account_id, keypair) = gen_account_in(label);
        let tx = TransactionBuilder::new(chain_id, account_id)
            .with_instructions([Log::new(Level::INFO, label.to_owned())])
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

    #[test]
    fn journal_replays_puts_and_removes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("queue-plan.norito");
        let first = record("first");
        let second = record("second");
        {
            let mut journal =
                QueuePlanJournal::open(&path, 1024 * 1024, true).expect("open journal");
            journal.put(first.clone()).expect("put first");
            journal.put(second.clone()).expect("put second");
            journal
                .remove(first.signed_transaction_hash, first.plan_digest())
                .expect("remove first");
        }
        let journal = QueuePlanJournal::open(&path, 1024 * 1024, true).expect("reopen journal");
        assert_eq!(journal.replay().expect("replay"), vec![second]);
    }
}
