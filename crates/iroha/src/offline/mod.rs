//! Offline journal utilities for signing devices and operators.
//!
//! OA2 requires every wallet to maintain a durable write-ahead log that ties pending spends to a
//! hash chain and HMAC so replays or tampering can be detected after a crash. This module provides
//! a small append-only journal that records `Pending` and `Committed` markers, exposes the current
//! pending queue, and verifies integrity on every open.

use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
use eyre::{Context as _, Result, eyre};
use hmac::{Hmac, Mac};
use iroha_crypto::{Hash, HashOf};
use sha2::Sha256;

const HEADER_MAGIC: &[u8; 4] = b"IJNL";
const HEADER_VERSION: u8 = 1;
const HEADER_LEN: usize = 5; // magic + version
const HASH_LEN: usize = Hash::LENGTH;
const CHAIN_LEN: usize = 32;
const HMAC_LEN: usize = 32;

type HmacSha256 = Hmac<Sha256>;
type EntrySnapshot = (BTreeMap<Hash, PendingEntry>, HashSet<Hash>, [u8; CHAIN_LEN]);

/// Kind of journal entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EntryKind {
    Pending = 0,
    Committed = 1,
}

impl TryFrom<u8> for EntryKind {
    type Error = eyre::Report;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Pending),
            1 => Ok(Self::Committed),
            other => Err(eyre!("unknown journal entry kind {other}")),
        }
    }
}

/// HMAC key used to authenticate journal records.
#[derive(Clone, Copy)]
pub struct JournalKey {
    hmac_key: [u8; 32],
}

impl JournalKey {
    /// Initialise a key from raw bytes.
    pub fn new(key: [u8; 32]) -> Self {
        Self { hmac_key: key }
    }

    /// Derive a deterministic key from caller-provided entropy.
    pub fn derive_from(seed: &[u8]) -> Self {
        use sha2::Digest;
        let digest = Sha256::digest(seed);
        let mut key = [0u8; 32];
        key.copy_from_slice(&digest);
        Self { hmac_key: key }
    }
}

/// A pending journal entry that has not been committed online yet.
#[derive(Clone, Debug)]
pub struct PendingEntry {
    /// Transaction hash tracked by this entry.
    pub tx_id: Hash,
    /// Envelope or payload bytes persisted.
    pub payload: Vec<u8>,
    /// Wall-clock timestamp (ms) when the record was appended.
    pub recorded_at_ms: u64,
    /// Hash-chain accumulator `R_k = H(R_{k-1} || tx_id)`.
    pub hash_chain: [u8; CHAIN_LEN],
}

/// Durable offline journal used by signing devices.
#[derive(Debug)]
pub struct OfflineJournal {
    file: File,
    path: PathBuf,
    hmac_key: [u8; 32],
    last_chain: [u8; CHAIN_LEN],
    pending: BTreeMap<Hash, PendingEntry>,
    committed: HashSet<Hash>,
}

impl OfflineJournal {
    /// Open (or create) a journal at `path`.
    ///
    /// The journal verifies all existing entries before returning. Tampering or corruption is
    /// reported as an error.
    ///
    /// # Errors
    ///
    /// Returns an error if the journal directory cannot be created, the file cannot be opened,
    /// or existing records fail validation.
    pub fn open(path: impl AsRef<Path>, key: JournalKey) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!("failed to create journal directory {}", parent.display())
            })?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)
            .wrap_err_with(|| format!("failed to open journal {}", path.display()))?;
        ensure_header(&mut file)?;

        let mut existing = Vec::new();
        file.seek(SeekFrom::Start(0))?;
        file.read_to_end(&mut existing)?;

        let (pending, committed, last_chain) = load_entries(&existing, &key.hmac_key)?;
        file.seek(SeekFrom::End(0))?;

        Ok(Self {
            file,
            path,
            hmac_key: key.hmac_key,
            last_chain,
            pending,
            committed,
        })
    }

    /// Append a pending entry for the provided transaction hash (using the current timestamp).
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction is already pending or committed, or if the journal
    /// cannot be updated.
    pub fn append_pending_hash<T>(&mut self, tx_id: HashOf<T>, payload: &[u8]) -> Result<()> {
        self.append_pending(tx_id.into(), payload, now_ms())
    }

    /// Append a pending entry using an explicit timestamp (ms since Unix epoch).
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction is already pending or committed, or if the journal
    /// cannot be updated.
    pub fn append_pending(
        &mut self,
        tx_id: Hash,
        payload: &[u8],
        recorded_at_ms: u64,
    ) -> Result<()> {
        if self.pending.contains_key(&tx_id) {
            return Err(eyre!("transaction {tx_id} already pending in journal"));
        }
        if self.committed.contains(&tx_id) {
            return Err(eyre!("transaction {tx_id} already committed in journal"));
        }

        let record = PendingEntry {
            tx_id,
            payload: payload.to_vec(),
            recorded_at_ms,
            hash_chain: [0; CHAIN_LEN],
        };
        self.write_record(EntryKind::Pending, &record.tx_id, payload, recorded_at_ms)?;
        self.pending.insert(record.tx_id, record);
        Ok(())
    }

    /// Mark a pending entry as committed (using the current timestamp).
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction is not pending or the journal cannot be updated.
    pub fn mark_committed_hash<T>(&mut self, tx_id: HashOf<T>) -> Result<()> {
        self.mark_committed(tx_id.into(), now_ms())
    }

    /// Mark a pending entry as committed (explicit timestamp).
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction is not pending or the journal cannot be updated.
    pub fn mark_committed(&mut self, tx_id: Hash, recorded_at_ms: u64) -> Result<()> {
        if !self.pending.contains_key(&tx_id) {
            return Err(eyre!("transaction {tx_id} not pending"));
        }
        self.write_record(EntryKind::Committed, &tx_id, &[], recorded_at_ms)?;
        self.pending.remove(&tx_id);
        self.committed.insert(tx_id);
        Ok(())
    }

    /// Iterate over the currently pending entries in insertion order.
    pub fn pending(&self) -> impl Iterator<Item = &PendingEntry> {
        self.pending.values()
    }

    /// Return the filesystem path backing this journal.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn write_record(
        &mut self,
        kind: EntryKind,
        tx_id: &Hash,
        payload: &[u8],
        recorded_at_ms: u64,
    ) -> Result<()> {
        let mut record =
            Vec::with_capacity(1 + 8 + 4 + HASH_LEN + payload.len() + CHAIN_LEN + HMAC_LEN);
        record.push(kind as u8);
        record.extend_from_slice(&recorded_at_ms.to_le_bytes());
        let payload_len =
            u32::try_from(payload.len()).map_err(|_| eyre!("journal payload exceeds u32 range"))?;
        record.extend_from_slice(&payload_len.to_le_bytes());
        record.extend_from_slice(tx_id.as_ref());
        record.extend_from_slice(payload);

        let chain = compute_chain(&self.last_chain, tx_id);
        record.extend_from_slice(&chain);

        let hmac = compute_hmac(&self.hmac_key, &self.last_chain, &record);
        record.extend_from_slice(&hmac);

        self.file.write_all(&record)?;
        self.file.sync_data()?;
        self.last_chain = chain;

        if let Some(entry) = self.pending.get_mut(tx_id) {
            entry.hash_chain = chain;
        }

        Ok(())
    }
}

fn ensure_header(file: &mut File) -> Result<()> {
    let metadata = file.metadata()?;
    if metadata.len() >= HEADER_LEN as u64 {
        let mut header = [0u8; HEADER_LEN];
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut header)?;
        if &header[..4] != HEADER_MAGIC || header[4] != HEADER_VERSION {
            return Err(eyre!(
                "offline journal header mismatch (magic={:?}, version={})",
                &header[..4],
                header[4]
            ));
        }
        file.seek(SeekFrom::End(0))?;
        return Ok(());
    }

    file.seek(SeekFrom::Start(0))?;
    file.write_all(HEADER_MAGIC)?;
    file.write_all(&[HEADER_VERSION])?;
    file.sync_data()?;
    file.seek(SeekFrom::End(0))?;
    Ok(())
}

fn load_entries(bytes: &[u8], hmac_key: &[u8; 32]) -> Result<EntrySnapshot> {
    if bytes.len() < HEADER_LEN {
        return Ok((BTreeMap::new(), HashSet::new(), [0; CHAIN_LEN]));
    }
    if &bytes[..4] != HEADER_MAGIC || bytes[4] != HEADER_VERSION {
        return Err(eyre!("offline journal header mismatch"));
    }

    let mut offset = HEADER_LEN;
    let mut prev_chain = [0u8; CHAIN_LEN];
    let mut pending = BTreeMap::new();
    let mut committed = HashSet::new();

    while offset < bytes.len() {
        if offset + 1 + 8 + 4 + HASH_LEN + CHAIN_LEN + HMAC_LEN > bytes.len() {
            return Err(eyre!("truncated journal record"));
        }

        let kind = EntryKind::try_from(bytes[offset])?;
        offset += 1;

        let mut timestamp_bytes = [0u8; 8];
        timestamp_bytes.copy_from_slice(&bytes[offset..offset + 8]);
        let recorded_at_ms = u64::from_le_bytes(timestamp_bytes);
        offset += 8;

        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&bytes[offset..offset + 4]);
        let payload_len = u32::from_le_bytes(len_bytes) as usize;
        offset += 4;

        let mut tx_id_bytes = [0u8; HASH_LEN];
        tx_id_bytes.copy_from_slice(&bytes[offset..offset + HASH_LEN]);
        let tx_id = Hash::prehashed(tx_id_bytes);
        offset += HASH_LEN;

        if offset + payload_len + CHAIN_LEN + HMAC_LEN > bytes.len() {
            return Err(eyre!("truncated journal payload"));
        }

        let payload = &bytes[offset..offset + payload_len];
        offset += payload_len;

        let mut stored_chain = [0u8; CHAIN_LEN];
        stored_chain.copy_from_slice(&bytes[offset..offset + CHAIN_LEN]);
        offset += CHAIN_LEN;

        let mut stored_hmac = [0u8; HMAC_LEN];
        stored_hmac.copy_from_slice(&bytes[offset..offset + HMAC_LEN]);
        offset += HMAC_LEN;

        let mut record_without_hmac =
            Vec::with_capacity(1 + 8 + 4 + HASH_LEN + payload_len + CHAIN_LEN);
        record_without_hmac.push(kind as u8);
        record_without_hmac.extend_from_slice(&recorded_at_ms.to_le_bytes());
        let payload_len_u32 =
            u32::try_from(payload_len).map_err(|_| eyre!("journal payload exceeds u32 range"))?;
        record_without_hmac.extend_from_slice(&payload_len_u32.to_le_bytes());
        record_without_hmac.extend_from_slice(tx_id.as_ref());
        record_without_hmac.extend_from_slice(payload);
        record_without_hmac.extend_from_slice(&stored_chain);

        let expected_chain = compute_chain(&prev_chain, &tx_id);
        if stored_chain != expected_chain {
            return Err(eyre!("journal hash chain mismatch for {}", tx_id));
        }

        let expected_hmac = compute_hmac(hmac_key, &prev_chain, &record_without_hmac);
        if stored_hmac != expected_hmac {
            return Err(eyre!("journal HMAC mismatch for {}", tx_id));
        }

        match kind {
            EntryKind::Pending => {
                pending.insert(
                    tx_id,
                    PendingEntry {
                        tx_id,
                        payload: payload.to_vec(),
                        recorded_at_ms,
                        hash_chain: stored_chain,
                    },
                );
            }
            EntryKind::Committed => {
                pending.remove(&tx_id);
                committed.insert(tx_id);
            }
        }

        prev_chain = stored_chain;
    }

    Ok((pending, committed, prev_chain))
}

fn compute_chain(prev_chain: &[u8; CHAIN_LEN], tx_id: &Hash) -> [u8; CHAIN_LEN] {
    let mut hasher = Blake2bVar::new(CHAIN_LEN).expect("blake2b output length is valid");
    hasher.update(prev_chain);
    hasher.update(tx_id.as_ref());
    let mut chain = [0u8; CHAIN_LEN];
    hasher
        .finalize_variable(&mut chain)
        .expect("output length matches buffer size");
    chain
}

fn compute_hmac(key: &[u8; 32], prev_chain: &[u8; CHAIN_LEN], record: &[u8]) -> [u8; HMAC_LEN] {
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac key length fixed");
    Mac::update(&mut mac, prev_chain);
    Mac::update(&mut mac, record);
    let digest = mac.finalize().into_bytes();
    let mut out = [0u8; HMAC_LEN];
    out.copy_from_slice(&digest);
    out
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_millis();
    u64::try_from(millis).expect("system time fits in u64 milliseconds")
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    fn temp_path() -> PathBuf {
        NamedTempFile::new().unwrap().into_temp_path().to_path_buf()
    }

    #[test]
    fn journal_roundtrip() {
        let path = temp_path();
        let key = JournalKey::derive_from(b"test-key");
        let payload = b"pending-envelope";
        let tx_id = Hash::new(b"tx-A");

        {
            let mut journal = OfflineJournal::open(&path, key).expect("open");
            journal.append_pending(tx_id, payload, 42).expect("append");
        }

        let journal = OfflineJournal::open(&path, key).expect("reopen");
        let entries: Vec<_> = journal.pending().collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].payload, payload);
        assert_eq!(entries[0].recorded_at_ms, 42);
        assert_ne!(entries[0].hash_chain, [0u8; CHAIN_LEN]);
    }

    #[test]
    fn mark_committed_clears_pending() {
        let path = temp_path();
        let key = JournalKey::derive_from(b"test-key");
        let tx_id = Hash::new(b"tx-B");

        let mut journal = OfflineJournal::open(&path, key).expect("open");
        journal
            .append_pending(tx_id, b"payload", 1)
            .expect("append");
        journal.mark_committed(tx_id, 2).expect("commit");
        assert!(journal.pending().next().is_none());
    }

    #[test]
    fn tampering_is_detected() {
        let path = temp_path();
        let key = JournalKey::derive_from(b"test-key");
        let tx_id = Hash::new(b"tx-C");

        {
            let mut journal = OfflineJournal::open(&path, key).expect("open");
            journal
                .append_pending(tx_id, b"payload", 1)
                .expect("append");
        }

        // Flip one byte in the payload.
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        let idx = bytes
            .iter()
            .position(|b| *b == b'p')
            .expect("payload present");
        bytes[idx] ^= 0xFF;
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&bytes).unwrap();
        file.flush().unwrap();

        let err = OfflineJournal::open(&path, key).unwrap_err();
        assert!(err.to_string().contains("HMAC"));
    }
}
