//! Persistence helpers for DA replay cursors, receipts, and spool artifacts.

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use eyre::{WrapErr, eyre};
use iroha_core::da::{LaneEpoch, ReplayFingerprint};
use iroha_crypto::{PublicKey, Signature};
use iroha_data_model::{da::prelude::*, nexus::LaneId};
use iroha_logger::{debug, warn};
use norito::{
    decode_from_bytes,
    json::{self, JsonDeserialize, JsonSerialize},
    to_bytes,
};
use sorafs_manifest::pdp::PdpCommitmentV1;

const CURSOR_FILE_NAME: &str = "replay_cursors.norito.json";
const RECEIPT_FILE_PREFIX: &str = "da-receipt";
/// Placeholder signature bytes used before signing DA receipts.
pub(crate) const RECEIPT_SIGNATURE_PLACEHOLDER: [u8; 64] = [0; 64];
pub(super) const STORED_RECEIPT_VERSION: u16 = 1;
const DA_COMMITMENT_SCHEDULE_ENTRY_VERSION: u16 = 1;

/// Persistent store tracking the highest sequence observed per `(lane, epoch)`.
pub struct ReplayCursorStore {
    dir: PathBuf,
    inner: Mutex<ReplayCursorState>,
}

#[derive(Default)]
struct ReplayCursorState {
    highest: HashMap<LaneEpoch, u64>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct CursorSnapshot {
    version: u32,
    entries: Vec<CursorEntry>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct CursorEntry {
    lane_id: u32,
    epoch: u64,
    highest_sequence: u64,
}

impl ReplayCursorStore {
    /// Load the replay cursor store from disk, returning an empty store when no snapshot exists.
    pub fn open(path: PathBuf) -> eyre::Result<Self> {
        fs::create_dir_all(&path).wrap_err_with(|| {
            format!("failed to create DA replay directory at {}", path.display())
        })?;
        let file_path = path.join(CURSOR_FILE_NAME);
        let state = if file_path.exists() {
            let data = fs::read(&file_path).wrap_err_with(|| {
                format!(
                    "failed to read DA replay snapshot at {}",
                    file_path.display()
                )
            })?;
            let snapshot: CursorSnapshot =
                json::from_slice(&data).wrap_err("failed to decode DA replay snapshot")?;
            ReplayCursorState::from_snapshot(snapshot)
        } else {
            ReplayCursorState::default()
        };

        Ok(Self::with_state(path, state))
    }

    /// Create an empty store backed by the provided directory (creating it if missing).
    pub fn empty(path: PathBuf) -> eyre::Result<Self> {
        fs::create_dir_all(&path).wrap_err_with(|| {
            format!("failed to create DA replay directory at {}", path.display())
        })?;
        Ok(Self::with_state(path, ReplayCursorState::default()))
    }

    /// Create an in-memory store (persistence disabled).
    pub fn in_memory() -> Self {
        Self {
            dir: PathBuf::new(),
            inner: Mutex::new(ReplayCursorState::default()),
        }
    }

    fn with_state(path: PathBuf, state: ReplayCursorState) -> Self {
        Self {
            dir: path,
            inner: Mutex::new(state),
        }
    }

    /// Access the known highest sequences for seeding the replay cache.
    pub fn highest_sequences(&self) -> Vec<(LaneEpoch, u64)> {
        let guard = self.inner.lock().expect("mutex poisoned");
        guard
            .highest
            .iter()
            .map(|(lane_epoch, highest)| (*lane_epoch, *highest))
            .collect()
    }

    /// Record a newly observed sequence for the provided `(lane, epoch)` window.
    pub fn record(&self, lane_epoch: LaneEpoch, sequence: u64) -> eyre::Result<()> {
        let mut guard = self.inner.lock().expect("mutex poisoned");
        let entry = guard.highest.entry(lane_epoch).or_insert(0);
        if *entry >= sequence {
            return Ok(());
        }
        *entry = sequence;
        drop(guard);
        self.persist_snapshot()
    }

    fn persist_snapshot(&self) -> eyre::Result<()> {
        if self.dir.as_os_str().is_empty() {
            // Persistence disabled; operate in-memory only.
            return Ok(());
        }
        let snapshot = {
            let guard = self.inner.lock().expect("mutex poisoned");
            guard.to_snapshot()
        };

        let data = json::to_vec(&snapshot).wrap_err("failed to encode DA replay snapshot")?;
        let file_path = self.dir.join(CURSOR_FILE_NAME);
        let tmp_path = replay_cursor_temp_path(&file_path);
        fs::write(&tmp_path, data).wrap_err_with(|| {
            format!(
                "failed to write DA replay snapshot temp file at {}",
                tmp_path.display()
            )
        })?;
        fs::rename(&tmp_path, &file_path).wrap_err_with(|| {
            format!(
                "failed to move DA replay snapshot temp file {} into place {}",
                tmp_path.display(),
                file_path.display()
            )
        })?;
        Ok(())
    }
}

pub(super) fn replay_cursor_temp_path(path: &Path) -> PathBuf {
    path.with_added_extension("tmp")
}

/// Receipt entry captured in the durable receipt log.
#[derive(Clone, Debug)]
pub struct DaReceiptLogEntry {
    /// Lane/epoch this receipt belongs to.
    pub lane_epoch: LaneEpoch,
    /// Sequence number scoped to the lane/epoch.
    pub sequence: u64,
    /// Manifest hash referenced by the receipt.
    pub manifest_hash: BlobDigest,
    /// Full DA ingest receipt payload.
    pub receipt: DaIngestReceipt,
}

#[derive(Clone, Debug, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub(super) struct StoredDaReceipt {
    pub(super) version: u16,
    pub(super) sequence: u64,
    pub(super) receipt: DaIngestReceipt,
}

/// Outcome returned after attempting to insert a receipt into the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReceiptInsertOutcome {
    /// Receipt stored successfully.
    Stored {
        /// Whether the per-lane cursor advanced.
        cursor_advanced: bool,
    },
    /// Receipt was already present on disk.
    Duplicate {
        /// Path of the existing receipt file.
        path: PathBuf,
    },
    /// Receipt reused a sequence number with a different manifest hash.
    ManifestConflict {
        /// Manifest hash already recorded.
        expected: BlobDigest,
        /// Manifest hash observed in the new receipt.
        observed: BlobDigest,
    },
    /// Sequence regressed relative to the latest stored entry.
    StaleSequence {
        /// Highest sequence currently recorded for the lane/epoch.
        highest: u64,
    },
}

#[derive(Clone)]
struct ReceiptMeta {
    manifest_hash: BlobDigest,
    path: PathBuf,
    receipt: DaIngestReceipt,
}

pub(super) fn unsigned_receipt_bytes(receipt: &DaIngestReceipt) -> eyre::Result<Vec<u8>> {
    let mut unsigned = receipt.clone();
    unsigned.operator_signature = Signature::from_bytes(&RECEIPT_SIGNATURE_PLACEHOLDER);
    to_bytes(&unsigned).map_err(|err| eyre!(err))
}

fn verify_receipt_signature(
    receipt: &DaIngestReceipt,
    signer_public_key: &PublicKey,
) -> eyre::Result<()> {
    let unsigned_bytes = unsigned_receipt_bytes(receipt)?;
    receipt
        .operator_signature
        .verify(signer_public_key, &unsigned_bytes)
        .map_err(|err| eyre!(err))
}

/// Durable log of DA ingest receipts keyed by `(lane, epoch, sequence)`.
#[derive(Clone)]
pub struct DaReceiptLog {
    dir: PathBuf,
    cursor_store: Arc<ReplayCursorStore>,
    signer_public_key: PublicKey,
    index: Arc<Mutex<BTreeMap<LaneEpoch, BTreeMap<u64, ReceiptMeta>>>>,
}

impl DaReceiptLog {
    /// Open (or create) a receipt log rooted at `dir`.
    pub fn open(
        dir: PathBuf,
        cursor_store: Arc<ReplayCursorStore>,
        signer_public_key: PublicKey,
    ) -> eyre::Result<Self> {
        if dir.as_os_str().is_empty() {
            return Err(eyre!("receipt log directory must not be empty"));
        }
        fs::create_dir_all(&dir)
            .wrap_err_with(|| format!("failed to create DA receipt directory {}", dir.display()))?;

        let (index, highest_map) = Self::load_existing(&dir, &signer_public_key)?;
        for (lane_epoch, highest) in highest_map {
            if let Err(err) = cursor_store.record(lane_epoch, highest) {
                warn!(?err, ?lane_epoch, "failed to seed receipt cursor from disk");
            }
        }

        Ok(Self {
            dir,
            cursor_store,
            signer_public_key,
            index: Arc::new(Mutex::new(index)),
        })
    }

    /// Construct an in-memory receipt log (no on-disk persistence).
    pub fn in_memory(cursor_store: Arc<ReplayCursorStore>, signer_public_key: PublicKey) -> Self {
        Self {
            dir: PathBuf::new(),
            cursor_store,
            signer_public_key,
            index: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Append a receipt to the log, enforcing monotonic sequence ordering per lane/epoch.
    pub fn append(
        &self,
        lane_epoch: LaneEpoch,
        sequence: u64,
        receipt: DaIngestReceipt,
        fingerprint: ReplayFingerprint,
    ) -> eyre::Result<ReceiptInsertOutcome> {
        if receipt.lane_id != lane_epoch.lane_id || receipt.epoch != lane_epoch.epoch {
            return Err(eyre!(
                "receipt lane/epoch mismatch: key {lane_epoch:?} vs receipt {}@{}",
                receipt.lane_id.as_u32(),
                receipt.epoch
            ));
        }

        verify_receipt_signature(&receipt, &self.signer_public_key)
            .wrap_err("DA receipt signature verification failed")?;
        let manifest_hash = receipt.manifest_hash;
        let mut guard = self.index.lock().expect("receipt index mutex poisoned");
        let lane_index = guard.entry(lane_epoch).or_default();

        if let Some(existing) = lane_index.get(&sequence) {
            if existing.manifest_hash == manifest_hash {
                return Ok(ReceiptInsertOutcome::Duplicate {
                    path: existing.path.clone(),
                });
            }
            return Ok(ReceiptInsertOutcome::ManifestConflict {
                expected: existing.manifest_hash,
                observed: manifest_hash,
            });
        }

        if let Some((&highest, _)) = lane_index.iter().next_back() {
            if sequence <= highest {
                return Ok(ReceiptInsertOutcome::StaleSequence { highest });
            }
        }

        let path = self.write_receipt_file(&receipt, sequence, &fingerprint)?;
        let cursor_advanced = lane_index
            .keys()
            .last()
            .copied()
            .map_or(true, |prev| sequence > prev);
        self.cursor_store
            .record(lane_epoch, sequence)
            .wrap_err("failed to persist receipt cursor")?;

        lane_index.insert(
            sequence,
            ReceiptMeta {
                manifest_hash,
                path: path.clone(),
                receipt: receipt.clone(),
            },
        );

        Ok(ReceiptInsertOutcome::Stored { cursor_advanced })
    }

    /// Load receipts for a `(lane, epoch)` window in sequence order.
    pub fn receipts_for(&self, lane_epoch: LaneEpoch) -> Vec<DaReceiptLogEntry> {
        let guard = self.index.lock().expect("receipt index mutex poisoned");
        let Some(entries) = guard.get(&lane_epoch) else {
            return Vec::new();
        };

        let mut out = Vec::with_capacity(entries.len());
        for (sequence, meta) in entries {
            out.push(DaReceiptLogEntry {
                lane_epoch,
                sequence: *sequence,
                manifest_hash: meta.manifest_hash,
                receipt: meta.receipt.clone(),
            });
        }
        out
    }

    fn load_existing(
        dir: &Path,
        signer_public_key: &PublicKey,
    ) -> eyre::Result<(
        BTreeMap<LaneEpoch, BTreeMap<u64, ReceiptMeta>>,
        BTreeMap<LaneEpoch, u64>,
    )> {
        let mut index: BTreeMap<LaneEpoch, BTreeMap<u64, ReceiptMeta>> = BTreeMap::new();
        let mut highest: BTreeMap<LaneEpoch, u64> = BTreeMap::new();

        if !dir.exists() {
            return Ok((index, highest));
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if !Self::is_receipt_file(&path) {
                continue;
            }
            let stored = match Self::decode_receipt(&path) {
                Ok(stored) => stored,
                Err(err) => {
                    warn!(
                        ?err,
                        path = %path.display(),
                        "skipping DA receipt with invalid encoding"
                    );
                    continue;
                }
            };
            let StoredDaReceipt {
                sequence, receipt, ..
            } = stored;
            if let Err(err) = verify_receipt_signature(&receipt, signer_public_key) {
                warn!(
                    ?err,
                    path = %path.display(),
                    "skipping DA receipt with invalid operator signature"
                );
                continue;
            }
            let lane_epoch = LaneEpoch::new(receipt.lane_id, receipt.epoch);
            let manifest_hash = receipt.manifest_hash;
            let lane_map = index.entry(lane_epoch).or_default();
            if let Some(existing) = lane_map.get(&sequence) {
                if existing.manifest_hash != manifest_hash {
                    return Err(eyre!(
                        "conflicting receipt for lane {:?} seq {} ({} vs {})",
                        lane_epoch,
                        sequence,
                        hex::encode(existing.manifest_hash.as_bytes()),
                        hex::encode(manifest_hash.as_bytes())
                    ));
                }
                continue;
            }

            highest
                .entry(lane_epoch)
                .and_modify(|current| *current = (*current).max(sequence))
                .or_insert(sequence);
            lane_map.insert(
                sequence,
                ReceiptMeta {
                    manifest_hash,
                    path,
                    receipt,
                },
            );
        }

        Ok((index, highest))
    }

    fn write_receipt_file(
        &self,
        receipt: &DaIngestReceipt,
        sequence: u64,
        fingerprint: &ReplayFingerprint,
    ) -> eyre::Result<PathBuf> {
        if self.dir.as_os_str().is_empty() {
            return Ok(PathBuf::new());
        }

        match persist_da_receipt(&self.dir, receipt, sequence, fingerprint) {
            Ok(Some(path)) => Ok(path),
            Ok(None) => Ok(PathBuf::new()),
            Err(err) => Err(err.into()),
        }
    }

    fn decode_receipt(path: &Path) -> eyre::Result<StoredDaReceipt> {
        let data = fs::read(path)?;
        let stored = decode_from_bytes::<StoredDaReceipt>(&data).map_err(|err| eyre!(err))?;
        if stored.version != STORED_RECEIPT_VERSION {
            return Err(eyre!(
                "unsupported DA receipt version {} (expected {})",
                stored.version,
                STORED_RECEIPT_VERSION
            ));
        }
        Ok(stored)
    }

    fn is_receipt_file(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with(RECEIPT_FILE_PREFIX) && name.ends_with(".norito"))
            .unwrap_or(false)
    }
}

impl ReplayCursorState {
    fn from_snapshot(snapshot: CursorSnapshot) -> Self {
        if snapshot.version != 1 {
            warn!(
                version = snapshot.version,
                "unknown DA replay snapshot version; treating as v1"
            );
        }

        let mut highest = HashMap::new();
        for entry in snapshot.entries {
            let lane_epoch = LaneEpoch::new(LaneId::from(entry.lane_id), entry.epoch);
            highest.insert(lane_epoch, entry.highest_sequence);
        }
        Self { highest }
    }

    fn to_snapshot(&self) -> CursorSnapshot {
        let mut entries = Vec::with_capacity(self.highest.len());
        for (lane_epoch, highest_sequence) in &self.highest {
            entries.push(CursorEntry {
                lane_id: lane_epoch.lane_id.as_u32(),
                epoch: lane_epoch.epoch,
                highest_sequence: *highest_sequence,
            });
        }
        CursorSnapshot {
            version: 1,
            entries,
        }
    }
}

pub(super) fn persist_da_receipt(
    spool_dir: &Path,
    receipt: &DaIngestReceipt,
    sequence: u64,
    fingerprint: &ReplayFingerprint,
) -> std::io::Result<Option<PathBuf>> {
    if spool_dir.as_os_str().is_empty() {
        return Ok(None);
    }

    fs::create_dir_all(spool_dir)?;

    let lane = receipt.lane_id.as_u32();
    let ticket_hex = hex::encode(receipt.storage_ticket.as_ref());
    let fingerprint_hex = hex::encode(fingerprint.as_bytes());
    let file_name = format!(
        "{RECEIPT_FILE_PREFIX}-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.norito",
        epoch = receipt.epoch,
    );
    let target_path = spool_dir.join(&file_name);
    if target_path.exists() {
        return Ok(Some(target_path));
    }

    let tmp_name = format!(
        ".{RECEIPT_FILE_PREFIX}-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.tmp-{}",
        std::process::id(),
        epoch = receipt.epoch,
    );
    let tmp_path = spool_dir.join(tmp_name);
    let encoded = to_bytes(&StoredDaReceipt {
        version: STORED_RECEIPT_VERSION,
        sequence,
        receipt: receipt.clone(),
    })
    .map_err(|err| std::io::Error::new(ErrorKind::Other, err))?;

    match fs::write(&tmp_path, encoded) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
    }

    if let Err(err) = fs::rename(&tmp_path, &target_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    debug!(
        path = ?target_path,
        lane = lane,
        epoch = receipt.epoch,
        sequence,
        ticket = %ticket_hex,
        "queued DA ingest receipt for fanout spool"
    );

    Ok(Some(target_path))
}

pub(super) fn load_da_receipts(spool_dir: &Path) -> std::io::Result<Vec<StoredDaReceipt>> {
    if spool_dir.as_os_str().is_empty() || !spool_dir.exists() {
        return Ok(Vec::new());
    }

    let mut receipts = Vec::new();
    for entry in fs::read_dir(spool_dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|raw| raw.to_str()) else {
            continue;
        };
        if !name.starts_with(RECEIPT_FILE_PREFIX) || !name.ends_with(".norito") {
            continue;
        }
        let data = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                warn!(?err, path = %path.display(), "failed to read DA receipt; skipping");
                continue;
            }
        };
        let stored = match decode_from_bytes::<StoredDaReceipt>(&data) {
            Ok(stored) => stored,
            Err(err) => {
                warn!(
                    ?err,
                    path = %path.display(),
                    "failed to decode DA receipt; skipping"
                );
                continue;
            }
        };
        if stored.version != STORED_RECEIPT_VERSION {
            warn!(
                path = %path.display(),
                version = stored.version,
                expected = STORED_RECEIPT_VERSION,
                "unsupported DA receipt version; skipping"
            );
            continue;
        }
        receipts.push(stored);
    }

    receipts.sort_by(|lhs, rhs| {
        (
            lhs.receipt.lane_id.as_u32(),
            lhs.receipt.epoch,
            lhs.sequence,
            lhs.receipt.manifest_hash.as_ref(),
        )
            .cmp(&(
                rhs.receipt.lane_id.as_u32(),
                rhs.receipt.epoch,
                rhs.sequence,
                rhs.receipt.manifest_hash.as_ref(),
            ))
    });
    Ok(receipts)
}

pub(super) fn load_manifest_from_spool(
    spool_dir: &Path,
    ticket: &StorageTicketId,
) -> std::io::Result<Vec<u8>> {
    if spool_dir.as_os_str().is_empty() {
        return Err(std::io::Error::new(
            ErrorKind::NotFound,
            "manifest spool directory is not configured",
        ));
    }
    let ticket_hex = hex::encode(ticket.as_bytes());
    let needle = format!("-{ticket_hex}-");
    let entries = fs::read_dir(spool_dir)?;
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        if let Some(name) = file_name.to_str() {
            if name.starts_with("manifest-") && name.contains(&needle) {
                return fs::read(entry.path());
            }
        }
    }
    Err(std::io::Error::new(
        ErrorKind::NotFound,
        "manifest not found for storage ticket",
    ))
}

pub(super) fn load_pdp_commitment_from_spool(
    spool_dir: &Path,
    ticket: &StorageTicketId,
) -> std::io::Result<Vec<u8>> {
    if spool_dir.as_os_str().is_empty() {
        return Err(std::io::Error::new(
            ErrorKind::NotFound,
            "PDP spool directory is not configured",
        ));
    }
    let ticket_hex = hex::encode(ticket.as_bytes());
    let needle = format!("-{ticket_hex}-");
    let entries = fs::read_dir(spool_dir)?;
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        if let Some(name) = file_name.to_str() {
            if name.starts_with("pdp-commitment-") && name.contains(&needle) {
                return fs::read(entry.path());
            }
        }
    }
    Err(std::io::Error::new(
        ErrorKind::NotFound,
        "PDP commitment not found for storage ticket",
    ))
}

pub(super) fn persist_manifest_for_sorafs(
    spool_dir: &Path,
    manifest_bytes: &[u8],
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: &StorageTicketId,
    fingerprint: &ReplayFingerprint,
) -> std::io::Result<Option<PathBuf>> {
    if spool_dir.as_os_str().is_empty() {
        return Ok(None);
    }

    fs::create_dir_all(spool_dir)?;

    let lane = lane_id.as_u32();
    let ticket_hex = hex::encode(storage_ticket.as_ref());
    let fingerprint_hex = hex::encode(fingerprint.as_bytes());
    let file_name = format!(
        "manifest-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.norito"
    );
    let target_path = spool_dir.join(file_name);
    if target_path.exists() {
        return Ok(Some(target_path));
    }

    let tmp_name = format!(
        ".manifest-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.tmp-{}",
        std::process::id()
    );
    let tmp_path = spool_dir.join(tmp_name);

    match fs::write(&tmp_path, manifest_bytes) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
    }

    if let Err(err) = fs::rename(&tmp_path, &target_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    debug!(
        path = ?target_path,
        lane = lane,
        epoch,
        sequence,
        ticket = %ticket_hex,
        "queued DA manifest for SoraFS orchestration"
    );

    Ok(Some(target_path))
}

pub(super) fn persist_pdp_commitment(
    spool_dir: &Path,
    commitment: &PdpCommitmentV1,
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: &StorageTicketId,
    fingerprint: &ReplayFingerprint,
) -> std::io::Result<Option<PathBuf>> {
    if spool_dir.as_os_str().is_empty() {
        return Ok(None);
    }

    fs::create_dir_all(spool_dir)?;

    let lane = lane_id.as_u32();
    let ticket_hex = hex::encode(storage_ticket.as_ref());
    let fingerprint_hex = hex::encode(fingerprint.as_bytes());
    let file_name = format!(
        "pdp-commitment-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.norito"
    );
    let target_path = spool_dir.join(file_name);
    if target_path.exists() {
        return Ok(Some(target_path));
    }

    let tmp_name = format!(
        ".pdp-commitment-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.tmp-{}",
        std::process::id()
    );
    let tmp_path = spool_dir.join(tmp_name);
    let encoded =
        to_bytes(commitment).map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;

    match fs::write(&tmp_path, encoded) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
    }

    if let Err(err) = fs::rename(&tmp_path, &target_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    debug!(
        path = ?target_path,
        lane = lane,
        epoch,
        sequence,
        ticket = %ticket_hex,
        "queued PDP commitment for SoraFS orchestration"
    );

    Ok(Some(target_path))
}

pub(super) fn persist_da_commitment_record(
    spool_dir: &Path,
    record: &DaCommitmentRecord,
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: &StorageTicketId,
    fingerprint: &ReplayFingerprint,
) -> std::io::Result<Option<PathBuf>> {
    if spool_dir.as_os_str().is_empty() {
        return Ok(None);
    }

    fs::create_dir_all(spool_dir)?;

    let lane = lane_id.as_u32();
    let ticket_hex = hex::encode(storage_ticket.as_ref());
    let fingerprint_hex = hex::encode(fingerprint.as_bytes());
    let file_name = format!(
        "da-commitment-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.norito"
    );
    let target_path = spool_dir.join(file_name);
    if target_path.exists() {
        return Ok(Some(target_path));
    }

    let tmp_name = format!(
        ".da-commitment-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.tmp-{}",
        std::process::id()
    );
    let tmp_path = spool_dir.join(tmp_name);
    let encoded =
        to_bytes(record).map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;

    match fs::write(&tmp_path, encoded) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
    }

    if let Err(err) = fs::rename(&tmp_path, &target_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    debug!(
        path = ?target_path,
        lane = lane,
        epoch,
        sequence,
        ticket = %ticket_hex,
        "queued DA commitment record for bundle ingestion"
    );

    Ok(Some(target_path))
}

#[derive(Clone, Debug, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
/// On-disk schedule entry combining commitment record and PDP commitment bytes.
pub(super) struct DaCommitmentScheduleEntry {
    /// Entry layout version for future migrations.
    pub(super) version: u16,
    /// Commitment record payload.
    pub(super) record: DaCommitmentRecord,
    /// Encoded PDP commitment bytes.
    pub(super) pdp_commitment: Vec<u8>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn persist_da_commitment_schedule_entry(
    spool_dir: &Path,
    record: &DaCommitmentRecord,
    pdp_commitment_bytes: &[u8],
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: &StorageTicketId,
    fingerprint: &ReplayFingerprint,
) -> std::io::Result<Option<PathBuf>> {
    if spool_dir.as_os_str().is_empty() {
        return Ok(None);
    }

    fs::create_dir_all(spool_dir)?;

    let lane = lane_id.as_u32();
    let ticket_hex = hex::encode(storage_ticket.as_ref());
    let fingerprint_hex = hex::encode(fingerprint.as_bytes());
    let file_name = format!(
        "da-commitment-schedule-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.norito"
    );
    let target_path = spool_dir.join(file_name);
    if target_path.exists() {
        return Ok(Some(target_path));
    }

    let tmp_name = format!(
        ".da-commitment-schedule-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.tmp-{}",
        std::process::id()
    );
    let tmp_path = spool_dir.join(tmp_name);
    let entry = DaCommitmentScheduleEntry {
        version: DA_COMMITMENT_SCHEDULE_ENTRY_VERSION,
        record: record.clone(),
        pdp_commitment: pdp_commitment_bytes.to_vec(),
    };
    let encoded =
        to_bytes(&entry).map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;

    match fs::write(&tmp_path, encoded) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
    }

    if let Err(err) = fs::rename(&tmp_path, &target_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    debug!(
        path = ?target_path,
        lane = lane,
        epoch,
        sequence,
        ticket = %ticket_hex,
        "queued DA commitment schedule entry for bundle ingestion"
    );

    Ok(Some(target_path))
}

pub(super) fn persist_da_pin_intent(
    spool_dir: &Path,
    intent: &DaPinIntent,
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: &StorageTicketId,
    fingerprint: &ReplayFingerprint,
) -> std::io::Result<Option<PathBuf>> {
    if spool_dir.as_os_str().is_empty() {
        return Ok(None);
    }

    fs::create_dir_all(spool_dir)?;

    let lane = lane_id.as_u32();
    let ticket_hex = hex::encode(storage_ticket.as_ref());
    let fingerprint_hex = hex::encode(fingerprint.as_bytes());
    let file_name = format!(
        "da-pin-intent-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.norito"
    );
    let target_path = spool_dir.join(file_name);
    if target_path.exists() {
        return Ok(Some(target_path));
    }

    let tmp_name = format!(
        ".da-pin-intent-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.tmp-{}",
        std::process::id()
    );
    let tmp_path = spool_dir.join(tmp_name);
    let encoded =
        to_bytes(intent).map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;

    match fs::write(&tmp_path, encoded) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
    }

    if let Err(err) = fs::rename(&tmp_path, &target_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    debug!(
        path = ?target_path,
        lane = lane,
        epoch,
        sequence,
        ticket = %ticket_hex,
        "queued DA pin intent for registry ingestion"
    );

    Ok(Some(target_path))
}
