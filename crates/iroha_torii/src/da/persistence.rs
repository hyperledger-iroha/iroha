//! Persistence helpers for DA replay cursors, receipts, and spool artifacts.

#![allow(clippy::redundant_pub_crate)]

use std::{
    collections::{BTreeMap, HashMap},
    ffi::OsStr,
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
};

use eyre::{WrapErr, eyre};
use iroha_core::da::{LaneEpoch, ReplayFingerprint};
use iroha_crypto::{Hash, PublicKey, Signature};
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
const RECEIPT_SIGNING_PAYLOAD_VERSION: u16 = 1;
const DA_COMMITMENT_SCHEDULE_ENTRY_VERSION: u16 = 1;
static ARTIFACT_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CursorSnapshotRelation {
    Equal,
    CandidateAhead,
    CandidateBehind,
    Conflicting,
}

impl ReplayCursorStore {
    /// Load the replay cursor store from disk, returning an empty store when no snapshot exists.
    pub fn open(path: PathBuf) -> eyre::Result<Self> {
        fs::create_dir_all(&path).wrap_err_with(|| {
            format!("failed to create DA replay directory at {}", path.display())
        })?;
        let file_path = path.join(CURSOR_FILE_NAME);
        let tmp_path = replay_cursor_temp_path(&file_path);
        let state = match read_cursor_snapshot(&file_path) {
            Ok(Some(snapshot)) => match read_cursor_snapshot(&tmp_path) {
                Ok(Some(tmp_snapshot)) => {
                    match compare_cursor_snapshots(&snapshot, &tmp_snapshot) {
                        CursorSnapshotRelation::CandidateAhead => {
                            warn!(
                                path = %tmp_path.display(),
                                "recovering newer DA replay cursor temp snapshot"
                            );
                            promote_replay_cursor_temp(&tmp_path, &file_path)?;
                            ReplayCursorState::from_snapshot(tmp_snapshot)
                        }
                        CursorSnapshotRelation::Equal | CursorSnapshotRelation::CandidateBehind => {
                            remove_replay_cursor_temp(&tmp_path)?;
                            ReplayCursorState::from_snapshot(snapshot)
                        }
                        CursorSnapshotRelation::Conflicting => {
                            warn!(
                                path = %tmp_path.display(),
                                "discarding conflicting DA replay cursor temp snapshot"
                            );
                            remove_replay_cursor_temp(&tmp_path)?;
                            ReplayCursorState::from_snapshot(snapshot)
                        }
                    }
                }
                Ok(None) => ReplayCursorState::from_snapshot(snapshot),
                Err(err) => {
                    warn!(
                        ?err,
                        path = %tmp_path.display(),
                        "discarding unreadable DA replay cursor temp snapshot"
                    );
                    remove_replay_cursor_temp(&tmp_path)?;
                    ReplayCursorState::from_snapshot(snapshot)
                }
            },
            Ok(None) => match read_cursor_snapshot(&tmp_path) {
                Ok(Some(snapshot)) => {
                    promote_replay_cursor_temp(&tmp_path, &file_path)?;
                    ReplayCursorState::from_snapshot(snapshot)
                }
                Ok(None) => ReplayCursorState::default(),
                Err(err) => return Err(err),
            },
            Err(err) => match read_cursor_snapshot(&tmp_path) {
                Ok(Some(snapshot)) => {
                    warn!(
                        ?err,
                        path = %file_path.display(),
                        "DA replay cursor snapshot invalid; recovering from temp snapshot"
                    );
                    promote_replay_cursor_temp(&tmp_path, &file_path)?;
                    ReplayCursorState::from_snapshot(snapshot)
                }
                Ok(None) => return Err(err),
                Err(tmp_err) => {
                    warn!(
                        ?tmp_err,
                        path = %tmp_path.display(),
                        "failed to read DA replay cursor temp snapshot"
                    );
                    return Err(err);
                }
            },
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
        let guard = match self.lock_state() {
            Ok(guard) => guard,
            Err(err) => {
                warn!(?err, "failed to read DA replay cursor snapshot from memory");
                return Vec::new();
            }
        };
        guard
            .highest
            .iter()
            .map(|(lane_epoch, highest)| (*lane_epoch, *highest))
            .collect()
    }

    /// Record a newly observed sequence for the provided `(lane, epoch)` window.
    pub fn record(&self, lane_epoch: LaneEpoch, sequence: u64) -> eyre::Result<()> {
        use std::collections::hash_map::Entry;

        let mut guard = self.lock_state()?;
        let previous = match guard.highest.entry(lane_epoch) {
            Entry::Occupied(mut entry) => {
                if *entry.get() >= sequence {
                    return Ok(());
                }
                let previous = *entry.get();
                *entry.get_mut() = sequence;
                Some(previous)
            }
            Entry::Vacant(entry) => {
                entry.insert(sequence);
                None
            }
        };
        let snapshot = guard.to_snapshot();
        if let Err(err) = self.persist_snapshot(&snapshot) {
            match previous {
                Some(previous) => {
                    guard.highest.insert(lane_epoch, previous);
                }
                None => {
                    guard.highest.remove(&lane_epoch);
                }
            }
            return Err(err);
        }
        Ok(())
    }

    fn lock_state(&self) -> eyre::Result<MutexGuard<'_, ReplayCursorState>> {
        self.inner
            .lock()
            .map_err(|_| eyre!("DA replay cursor state mutex poisoned"))
    }

    fn persist_snapshot(&self, snapshot: &CursorSnapshot) -> eyre::Result<()> {
        if self.dir.as_os_str().is_empty() {
            // Persistence disabled; operate in-memory only.
            return Ok(());
        }

        let data = json::to_vec(snapshot).wrap_err("failed to encode DA replay snapshot")?;
        let file_path = self.dir.join(CURSOR_FILE_NAME);
        let tmp_path = replay_cursor_temp_path(&file_path);
        {
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&tmp_path)
                .wrap_err_with(|| {
                    format!(
                        "failed to create DA replay snapshot temp file at {}",
                        tmp_path.display()
                    )
                })?;
            file.write_all(&data).wrap_err_with(|| {
                format!(
                    "failed to write DA replay snapshot temp file at {}",
                    tmp_path.display()
                )
            })?;
            file.sync_all().wrap_err_with(|| {
                format!(
                    "failed to sync DA replay snapshot temp file at {}",
                    tmp_path.display()
                )
            })?;
        }
        fs::rename(&tmp_path, &file_path).wrap_err_with(|| {
            format!(
                "failed to move DA replay snapshot temp file {} into place {}",
                tmp_path.display(),
                file_path.display()
            )
        })?;
        if let Some(parent) = file_path.parent() {
            if !parent.as_os_str().is_empty() {
                sync_dir(parent).wrap_err_with(|| {
                    format!(
                        "failed to sync DA replay snapshot directory at {}",
                        parent.display()
                    )
                })?;
            }
        }
        Ok(())
    }
}

pub(super) fn replay_cursor_temp_path(path: &Path) -> PathBuf {
    path.with_added_extension("tmp")
}

fn sync_dir(path: &Path) -> std::io::Result<()> {
    let file = fs::File::open(path)?;
    file.sync_all()
}

fn read_cursor_snapshot(path: &Path) -> eyre::Result<Option<CursorSnapshot>> {
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(eyre!(err)).wrap_err_with(|| {
                format!("failed to read DA replay snapshot at {}", path.display())
            });
        }
    };
    let snapshot: CursorSnapshot = json::from_slice(&data)
        .wrap_err_with(|| format!("failed to decode DA replay snapshot at {}", path.display()))?;
    validate_cursor_snapshot(path, &snapshot)?;
    Ok(Some(snapshot))
}

fn validate_cursor_snapshot(path: &Path, snapshot: &CursorSnapshot) -> eyre::Result<()> {
    if snapshot.version != 1 {
        return Err(eyre!(
            "unsupported DA replay snapshot version {} at {}",
            snapshot.version,
            path.display()
        ));
    }

    let mut seen = BTreeMap::new();
    for entry in &snapshot.entries {
        let lane_epoch = LaneEpoch::new(LaneId::from(entry.lane_id), entry.epoch);
        if seen.insert(lane_epoch, entry.highest_sequence).is_some() {
            return Err(eyre!(
                "duplicate DA replay cursor entry for lane {} epoch {} at {}",
                entry.lane_id,
                entry.epoch,
                path.display()
            ));
        }
    }

    Ok(())
}

fn compare_cursor_snapshots(
    current: &CursorSnapshot,
    candidate: &CursorSnapshot,
) -> CursorSnapshotRelation {
    let current_entries = cursor_snapshot_map(current);
    let candidate_entries = cursor_snapshot_map(candidate);
    let mut ahead = false;
    let mut behind = false;

    for key in current_entries.keys().chain(candidate_entries.keys()) {
        match (current_entries.get(key), candidate_entries.get(key)) {
            (Some(current), Some(candidate)) if candidate > current => ahead = true,
            (Some(current), Some(candidate)) if candidate < current => behind = true,
            (None, Some(_)) => ahead = true,
            (Some(_), None) => behind = true,
            _ => {}
        }
        if ahead && behind {
            return CursorSnapshotRelation::Conflicting;
        }
    }

    match (ahead, behind) {
        (false, false) => CursorSnapshotRelation::Equal,
        (true, false) => CursorSnapshotRelation::CandidateAhead,
        (false, true) => CursorSnapshotRelation::CandidateBehind,
        (true, true) => CursorSnapshotRelation::Conflicting,
    }
}

fn cursor_snapshot_map(snapshot: &CursorSnapshot) -> BTreeMap<LaneEpoch, u64> {
    let mut entries = BTreeMap::new();
    for entry in &snapshot.entries {
        let key = LaneEpoch::new(LaneId::from(entry.lane_id), entry.epoch);
        entries
            .entry(key)
            .and_modify(|current| {
                if entry.highest_sequence > *current {
                    *current = entry.highest_sequence;
                }
            })
            .or_insert(entry.highest_sequence);
    }
    entries
}

fn remove_replay_cursor_temp(tmp_path: &Path) -> eyre::Result<()> {
    match fs::remove_file(tmp_path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(eyre!(err)).wrap_err_with(|| {
            format!(
                "failed to remove DA replay cursor temp snapshot {}",
                tmp_path.display()
            )
        }),
    }
}

fn promote_replay_cursor_temp(tmp_path: &Path, file_path: &Path) -> eyre::Result<()> {
    match fs::rename(tmp_path, file_path) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            fs::remove_file(file_path).wrap_err_with(|| {
                format!(
                    "failed to remove DA replay cursor snapshot before promoting temp {}",
                    file_path.display()
                )
            })?;
            fs::rename(tmp_path, file_path).wrap_err_with(|| {
                format!(
                    "failed to promote DA replay cursor temp snapshot {} after removal",
                    tmp_path.display()
                )
            })?;
        }
        Err(err) => {
            return Err(eyre!(err)).wrap_err_with(|| {
                format!(
                    "failed to promote DA replay cursor temp snapshot {} into {}",
                    tmp_path.display(),
                    file_path.display()
                )
            });
        }
    };

    if let Some(parent) = file_path.parent() {
        if !parent.as_os_str().is_empty() {
            sync_dir(parent).wrap_err_with(|| {
                format!(
                    "failed to sync DA replay cursor directory after temp promotion at {}",
                    parent.display()
                )
            })?;
        }
    }
    Ok(())
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

#[derive(Clone, Debug, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
struct DaReceiptSigningPayload {
    version: u16,
    sequence: u64,
    receipt: DaIngestReceipt,
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
    /// Receipt reused a sequence number with different signed receipt evidence.
    ReceiptConflict {
        /// Path of the existing receipt file.
        path: PathBuf,
    },
    /// Sequence regressed relative to the latest stored entry.
    StaleSequence {
        /// Highest sequence currently recorded for the lane/epoch.
        highest: u64,
    },
    /// Sequence skipped over the next required slot.
    SequenceGap {
        /// Next sequence expected by the durable receipt log.
        expected_next: u64,
        /// Sequence supplied by the caller.
        observed: u64,
    },
}

#[derive(Clone)]
struct ReceiptMeta {
    manifest_hash: BlobDigest,
    fingerprint: ReplayFingerprint,
    path: PathBuf,
    receipt: DaIngestReceipt,
}

type ReceiptIndex = BTreeMap<LaneEpoch, BTreeMap<u64, ReceiptMeta>>;

pub(super) fn unsigned_receipt_bytes(
    receipt: &DaIngestReceipt,
    sequence: u64,
) -> eyre::Result<Vec<u8>> {
    let mut unsigned = receipt.clone();
    unsigned.operator_signature = Signature::from_bytes(&RECEIPT_SIGNATURE_PLACEHOLDER);
    to_bytes(&DaReceiptSigningPayload {
        version: RECEIPT_SIGNING_PAYLOAD_VERSION,
        sequence,
        receipt: unsigned,
    })
    .map_err(|err| eyre!(err))
}

fn verify_receipt_signature(
    receipt: &DaIngestReceipt,
    sequence: u64,
    signer_public_key: &PublicKey,
) -> eyre::Result<()> {
    let unsigned_bytes = unsigned_receipt_bytes(receipt, sequence)?;
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
    index: Arc<Mutex<ReceiptIndex>>,
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
            cursor_store.record(lane_epoch, highest).wrap_err_with(|| {
                format!("failed to seed receipt cursor from disk for {lane_epoch:?}")
            })?;
        }

        Ok(Self {
            dir,
            cursor_store,
            signer_public_key,
            index: Arc::new(Mutex::new(index)),
        })
    }

    /// Construct a non-durable in-memory receipt log.
    ///
    /// This is suitable for tests, diagnostics, and the runtime fallback used when the durable
    /// receipt directory cannot be opened. Appends fail closed because production ingest requires a
    /// durable receipt file before a request can be acknowledged.
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
        if self.dir.as_os_str().is_empty() {
            return Err(eyre!(
                "DA receipt log is not durable; refusing to acknowledge ingest without a receipt file"
            ));
        }

        verify_receipt_signature(&receipt, sequence, &self.signer_public_key)
            .wrap_err("DA receipt signature verification failed")?;
        let manifest_hash = receipt.manifest_hash;
        let mut guard = self.lock_index()?;
        let lane_index = guard.entry(lane_epoch).or_default();

        if let Some(existing) = lane_index.get(&sequence) {
            if existing.receipt == receipt {
                return Ok(ReceiptInsertOutcome::Duplicate {
                    path: existing.path.clone(),
                });
            }
            if existing.manifest_hash == manifest_hash {
                return Ok(ReceiptInsertOutcome::ReceiptConflict {
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
            if let Some(expected_next) = highest.checked_add(1)
                && sequence != expected_next
            {
                return Ok(ReceiptInsertOutcome::SequenceGap {
                    expected_next,
                    observed: sequence,
                });
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
                fingerprint,
                path: path.clone(),
                receipt: receipt.clone(),
            },
        );

        Ok(ReceiptInsertOutcome::Stored { cursor_advanced })
    }

    /// Return a logged durable receipt for an idempotent ingest retry.
    ///
    /// The receipt is reloaded from disk before returning it so duplicate acknowledgements do not
    /// rely on process-local state.
    pub(crate) fn receipt_for_duplicate(
        &self,
        lane_epoch: LaneEpoch,
        sequence: u64,
        fingerprint: ReplayFingerprint,
    ) -> eyre::Result<Option<(PathBuf, DaIngestReceipt)>> {
        if self.dir.as_os_str().is_empty() {
            return Err(eyre!(
                "DA receipt log is not durable; duplicate receipt cannot be recovered from disk"
            ));
        }

        let (path, receipt) = {
            let guard = self.lock_index()?;
            let Some(meta) = guard
                .get(&lane_epoch)
                .and_then(|entries| entries.get(&sequence))
            else {
                return Ok(None);
            };
            if meta.fingerprint != fingerprint {
                return Ok(None);
            }
            (meta.path.clone(), meta.receipt.clone())
        };

        let stored = Self::decode_receipt(&path)
            .wrap_err_with(|| format!("failed to reload durable DA receipt {}", path.display()))?;
        if stored.sequence != sequence || stored.receipt != receipt {
            return Err(eyre!(
                "durable DA receipt {} no longer matches receipt-log index",
                path.display()
            ));
        }
        verify_receipt_signature(&stored.receipt, stored.sequence, &self.signer_public_key)
            .wrap_err_with(|| {
                format!(
                    "failed to verify reloaded durable DA receipt {}",
                    path.display()
                )
            })?;

        Ok(Some((path, receipt)))
    }

    /// Load receipts for a `(lane, epoch)` window in sequence order.
    pub fn receipts_for(&self, lane_epoch: LaneEpoch) -> Vec<DaReceiptLogEntry> {
        let guard = match self.lock_index() {
            Ok(guard) => guard,
            Err(err) => {
                warn!(?err, ?lane_epoch, "failed to read DA receipt log entries");
                return Vec::new();
            }
        };
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
    ) -> eyre::Result<(ReceiptIndex, BTreeMap<LaneEpoch, u64>)> {
        let mut index: ReceiptIndex = BTreeMap::new();
        let mut highest: BTreeMap<LaneEpoch, u64> = BTreeMap::new();

        if !dir.exists() {
            return Ok((index, highest));
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !artifact_path_matches(&path, RECEIPT_FILE_PREFIX)? {
                continue;
            }
            if !entry.file_type()?.is_file() {
                return Err(eyre!(
                    "durable DA receipt {} is not a regular file",
                    path.display()
                ));
            }
            let (receipt_key, stored) =
                Self::decode_receipt_with_key(&path).wrap_err_with(|| {
                    format!("failed to load durable DA receipt {}", path.display())
                })?;
            let StoredDaReceipt {
                sequence, receipt, ..
            } = stored;
            verify_receipt_signature(&receipt, sequence, signer_public_key).wrap_err_with(
                || format!("failed to verify durable DA receipt {}", path.display()),
            )?;
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
                if existing.receipt != receipt {
                    return Err(eyre!(
                        "conflicting duplicate receipt for lane {:?} seq {} at {} and {}",
                        lane_epoch,
                        sequence,
                        existing.path.display(),
                        path.display()
                    ));
                }
                if existing.fingerprint != receipt_key.fingerprint {
                    return Err(eyre!(
                        "duplicate receipt fingerprint conflict for lane {:?} seq {} at {} and {}",
                        lane_epoch,
                        sequence,
                        existing.path.display(),
                        path.display()
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
                    fingerprint: receipt_key.fingerprint,
                    path,
                    receipt,
                },
            );
        }

        validate_receipt_index_contiguous(&index)?;

        Ok((index, highest))
    }

    fn lock_index(&self) -> eyre::Result<MutexGuard<'_, ReceiptIndex>> {
        self.index
            .lock()
            .map_err(|_| eyre!("DA receipt log index mutex poisoned"))
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
        Self::decode_receipt_with_key(path).map(|(_, stored)| stored)
    }

    fn decode_receipt_with_key(path: &Path) -> eyre::Result<(ReceiptFileKey, StoredDaReceipt)> {
        let data = fs::read(path)?;
        let stored = decode_from_bytes::<StoredDaReceipt>(&data).map_err(|err| eyre!(err))?;
        if stored.version != STORED_RECEIPT_VERSION {
            return Err(eyre!(
                "unsupported DA receipt version {} (expected {})",
                stored.version,
                STORED_RECEIPT_VERSION
            ));
        }
        let key = validate_receipt_filename(path, &stored)?;
        Ok((key, stored))
    }
}

#[derive(Clone, Copy)]
struct ReceiptFileKey {
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: StorageTicketId,
    fingerprint: ReplayFingerprint,
}

fn parse_receipt_file_key(path: &Path) -> eyre::Result<ReceiptFileKey> {
    let name = path
        .file_name()
        .and_then(|raw| raw.to_str())
        .ok_or_else(|| eyre!("receipt filename is not valid UTF-8"))?;
    let rest = name
        .strip_prefix(RECEIPT_FILE_PREFIX)
        .and_then(|name| name.strip_prefix('-'))
        .and_then(|name| name.strip_suffix(".norito"))
        .ok_or_else(|| eyre!("receipt filename does not use the expected prefix/suffix"))?;

    let mut fields = rest.split('-');
    let lane_hex = fields
        .next()
        .ok_or_else(|| eyre!("receipt filename is missing lane id"))?;
    let epoch_hex = fields
        .next()
        .ok_or_else(|| eyre!("receipt filename is missing epoch"))?;
    let sequence_hex = fields
        .next()
        .ok_or_else(|| eyre!("receipt filename is missing sequence"))?;
    let ticket_hex = fields
        .next()
        .ok_or_else(|| eyre!("receipt filename is missing storage ticket"))?;
    let fingerprint_hex = fields
        .next()
        .ok_or_else(|| eyre!("receipt filename is missing fingerprint"))?;
    if fields.next().is_some() {
        return Err(eyre!("receipt filename contains extra fields"));
    }

    let lane_id = parse_fixed_hex_u32(lane_hex, 8)
        .map(LaneId::new)
        .ok_or_else(|| eyre!("receipt filename lane id is not fixed-width hexadecimal u32"))?;
    let epoch = parse_fixed_hex_u64(epoch_hex, 16)
        .ok_or_else(|| eyre!("receipt filename epoch is not fixed-width hexadecimal u64"))?;
    let sequence = parse_fixed_hex_u64(sequence_hex, 16)
        .ok_or_else(|| eyre!("receipt filename sequence is not fixed-width hexadecimal u64"))?;
    let storage_ticket = StorageTicketId::new(
        parse_fixed_hex_32(ticket_hex)
            .ok_or_else(|| eyre!("receipt filename storage ticket is not 32-byte hex"))?,
    );
    let fingerprint = parse_fixed_hex_32(fingerprint_hex)
        .map(ReplayFingerprint::from)
        .ok_or_else(|| eyre!("receipt filename fingerprint is not 32-byte hex"))?;

    Ok(ReceiptFileKey {
        lane_id,
        epoch,
        sequence,
        storage_ticket,
        fingerprint,
    })
}

fn validate_receipt_filename(
    path: &Path,
    stored: &StoredDaReceipt,
) -> eyre::Result<ReceiptFileKey> {
    let key = parse_receipt_file_key(path)?;
    if key.lane_id != stored.receipt.lane_id
        || key.epoch != stored.receipt.epoch
        || key.sequence != stored.sequence
        || key.storage_ticket != stored.receipt.storage_ticket
    {
        return Err(eyre!(
            "receipt filename tuple {:?}/{}:{}/{:?} mismatches body {:?}/{}:{}/{:?}",
            key.lane_id,
            key.epoch,
            key.sequence,
            key.storage_ticket,
            stored.receipt.lane_id,
            stored.receipt.epoch,
            stored.sequence,
            stored.receipt.storage_ticket
        ));
    }
    Ok(key)
}

fn validate_receipt_index_contiguous(index: &ReceiptIndex) -> eyre::Result<()> {
    for (lane_epoch, entries) in index {
        let mut previous: Option<u64> = None;
        for sequence in entries.keys().copied() {
            if let Some(prev) = previous
                && let Some(expected) = prev.checked_add(1)
                && sequence != expected
            {
                return Err(eyre!(
                    "missing DA receipt sequence for lane {:?}: expected {}, found {}",
                    lane_epoch,
                    expected,
                    sequence
                ));
            }
            previous = Some(sequence);
        }
    }
    Ok(())
}

impl ReplayCursorState {
    fn from_snapshot(snapshot: CursorSnapshot) -> Self {
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

fn existing_artifact_path_if_matching(
    target_path: &Path,
    expected: &[u8],
    artifact: &str,
) -> std::io::Result<Option<PathBuf>> {
    if !target_path.exists() {
        return Ok(None);
    }

    let existing = fs::read(target_path)?;
    if existing == expected {
        return Ok(Some(target_path.to_path_buf()));
    }

    Err(std::io::Error::new(
        ErrorKind::InvalidData,
        format!(
            "{artifact} already exists at {} with different bytes",
            target_path.display()
        ),
    ))
}

fn install_artifact_without_overwrite(
    tmp_path: &Path,
    target_path: &Path,
    expected: &[u8],
    artifact: &str,
) -> std::io::Result<()> {
    match fs::hard_link(tmp_path, target_path) {
        Ok(()) => {
            let sync_result = sync_parent_dir(target_path);
            let remove_result = remove_temp_artifact(tmp_path);
            sync_result?;
            remove_result
        }
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            let existing_result =
                existing_artifact_path_if_matching(target_path, expected, artifact).map(|_| ());
            let remove_result = remove_temp_artifact(tmp_path);
            existing_result?;
            remove_result
        }
        Err(err) => {
            remove_temp_artifact(tmp_path)?;
            Err(err)
        }
    }
}

fn write_temp_artifact(tmp_path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(tmp_path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn temp_artifact_write_error(tmp_path: &Path, err: std::io::Error) -> std::io::Error {
    if err.kind() == ErrorKind::AlreadyExists {
        return err;
    }
    remove_temp_artifact(tmp_path).err().unwrap_or(err)
}

fn allocate_artifact_temp_counter(counter: &AtomicU64) -> std::io::Result<u64> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| {
            std::io::Error::new(
                ErrorKind::Other,
                "DA artifact temp suffix counter exhausted",
            )
        })
}

fn artifact_temp_suffix() -> std::io::Result<String> {
    let counter = allocate_artifact_temp_counter(&ARTIFACT_TEMP_COUNTER)?;
    Ok(format!("{}-{counter:016x}", std::process::id()))
}

fn sync_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        sync_dir(parent)?;
    }
    Ok(())
}

fn remove_temp_artifact(tmp_path: &Path) -> std::io::Result<()> {
    match fs::remove_file(tmp_path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(std::io::Error::new(
            err.kind(),
            format!(
                "failed to remove DA temp artifact {}: {err}",
                tmp_path.display()
            ),
        )),
    }
}

#[cfg(test)]
mod temp_artifact_tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use iroha_crypto::KeyPair;
    use tempfile::tempdir;

    use super::*;

    fn poison_replay_cursor_store(store: &ReplayCursorStore) {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _guard = store.inner.lock().expect("initial cursor lock");
            panic!("poison DA replay cursor mutex");
        }));
        assert!(result.is_err(), "poisoning panic should be caught");
    }

    fn poison_receipt_log(log: &DaReceiptLog) {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _guard = log.index.lock().expect("initial receipt-log lock");
            panic!("poison DA receipt log mutex");
        }));
        assert!(result.is_err(), "poisoning panic should be caught");
    }

    fn test_fingerprint(seed: u8) -> ReplayFingerprint {
        ReplayFingerprint::from([seed; blake3::OUT_LEN])
    }

    fn test_receipt(
        signer: &KeyPair,
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        seed: u8,
    ) -> DaIngestReceipt {
        let mut receipt = DaIngestReceipt {
            client_blob_id: BlobDigest::new([seed; 32]),
            lane_id,
            epoch,
            blob_hash: BlobDigest::new([seed.wrapping_add(1); 32]),
            chunk_root: BlobDigest::new([seed.wrapping_add(2); 32]),
            manifest_hash: BlobDigest::new([seed.wrapping_add(3); 32]),
            storage_ticket: StorageTicketId::new([seed.wrapping_add(4); 32]),
            pdp_commitment: Some(vec![seed]),
            stripe_layout: DaStripeLayout::default(),
            queued_at_unix: 1234,
            rent_quote: DaRentQuote::default(),
            operator_signature: Signature::from_bytes(&RECEIPT_SIGNATURE_PLACEHOLDER),
        };
        let unsigned = unsigned_receipt_bytes(&receipt, sequence).expect("test receipt encodes");
        receipt.operator_signature = Signature::new(signer.private_key(), &unsigned);
        receipt
    }

    #[test]
    fn da_temp_artifact_cleanup_reports_unremovable_path() {
        let dir = tempdir().expect("tempdir");
        let tmp_path = dir.path().join(".da.tmp");
        fs::create_dir(&tmp_path).expect("block temp cleanup");

        let err = remove_temp_artifact(&tmp_path).expect_err("directory cleanup should fail");

        assert!(
            err.to_string()
                .contains("failed to remove DA temp artifact"),
            "unexpected error: {err}"
        );
        assert!(
            tmp_path.is_dir(),
            "failed cleanup should leave temp path visible for operator repair"
        );
    }

    #[test]
    fn da_temp_artifact_write_rejects_existing_path_without_truncating() {
        let dir = tempdir().expect("tempdir");
        let tmp_path = dir.path().join(".da.tmp");
        fs::write(&tmp_path, b"existing").expect("seed temp artifact");

        let err = write_temp_artifact(&tmp_path, b"replacement")
            .expect_err("existing temp artifact should not be overwritten");

        assert_eq!(err.kind(), ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read(&tmp_path).expect("read existing temp artifact"),
            b"existing"
        );

        let err =
            temp_artifact_write_error(&tmp_path, std::io::Error::from(ErrorKind::AlreadyExists));
        assert_eq!(err.kind(), ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read(&tmp_path).expect("read existing temp artifact after cleanup helper"),
            b"existing"
        );
    }

    #[test]
    fn da_temp_artifact_counter_rejects_exhaustion_without_wrapping() {
        let counter = AtomicU64::new(u64::MAX);

        let err =
            allocate_artifact_temp_counter(&counter).expect_err("exhausted counter must reject");

        assert_eq!(err.kind(), ErrorKind::Other);
        assert!(
            err.to_string().contains("temp suffix counter exhausted"),
            "unexpected error: {err}"
        );
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn da_temp_artifact_counter_allocates_pre_exhaustion_suffix_once() {
        let counter = AtomicU64::new(u64::MAX - 1);

        let suffix = allocate_artifact_temp_counter(&counter)
            .expect("last non-exhausted temp counter should allocate");

        assert_eq!(suffix, u64::MAX - 1);
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
        assert!(
            allocate_artifact_temp_counter(&counter).is_err(),
            "counter must fail closed once exhausted"
        );
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn da_install_artifact_reports_temp_cleanup_failure_after_link_error() {
        let dir = tempdir().expect("tempdir");
        let tmp_path = dir.path().join(".da.tmp");
        let target_path = dir.path().join("da-target.norito");
        fs::create_dir(&tmp_path).expect("block temp cleanup");

        let err =
            install_artifact_without_overwrite(&tmp_path, &target_path, b"expected", "DA artifact")
                .expect_err("directory temp artifact should fail cleanup");

        assert!(
            err.to_string()
                .contains("failed to remove DA temp artifact"),
            "unexpected error: {err}"
        );
        assert!(
            tmp_path.is_dir(),
            "failed cleanup should leave temp path visible for operator repair"
        );
        assert!(
            !target_path.exists(),
            "failed hard-link install must not create the target artifact"
        );
    }

    #[test]
    fn da_replay_cursor_lock_poison_fails_closed() {
        let store = ReplayCursorStore::in_memory();
        poison_replay_cursor_store(&store);

        assert!(
            store.highest_sequences().is_empty(),
            "poisoned cursor snapshots should not panic or expose stale state"
        );
        let err = store
            .record(LaneEpoch::new(LaneId::new(3), 9), 42)
            .expect_err("poisoned cursor store must reject sequence recording");
        assert!(
            format!("{err:?}").contains("poisoned"),
            "unexpected cursor poison error: {err:?}"
        );
    }

    #[test]
    fn da_receipt_log_lock_poison_fails_closed() {
        let dir = tempdir().expect("tempdir");
        let cursor_store =
            Arc::new(ReplayCursorStore::empty(dir.path().join("cursors")).expect("cursor store"));
        let signer = KeyPair::random();
        let log = DaReceiptLog::open(
            dir.path().join("receipts"),
            cursor_store,
            signer.public_key().clone(),
        )
        .expect("receipt log");
        let lane_epoch = LaneEpoch::new(LaneId::new(4), 10);
        poison_receipt_log(&log);

        assert!(
            log.receipts_for(lane_epoch).is_empty(),
            "poisoned receipt-log reads should not panic"
        );
        let err = log
            .receipt_for_duplicate(lane_epoch, 1, test_fingerprint(0xA1))
            .expect_err("poisoned receipt log must reject duplicate recovery");
        assert!(
            format!("{err:?}").contains("poisoned"),
            "unexpected duplicate-recovery poison error: {err:?}"
        );

        let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1, 0xA1);
        let err = log
            .append(lane_epoch, 1, receipt, test_fingerprint(0xA1))
            .expect_err("poisoned receipt log must reject ingest acknowledgement");
        assert!(
            format!("{err:?}").contains("poisoned"),
            "unexpected append poison error: {err:?}"
        );
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
    let encoded = to_bytes(&StoredDaReceipt {
        version: STORED_RECEIPT_VERSION,
        sequence,
        receipt: receipt.clone(),
    })
    .map_err(|err| std::io::Error::new(ErrorKind::Other, err))?;
    if let Some(path) =
        existing_artifact_path_if_matching(&target_path, &encoded, "DA receipt artifact")?
    {
        return Ok(Some(path));
    }

    let tmp_name = format!(
        ".{RECEIPT_FILE_PREFIX}-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.tmp-{}",
        artifact_temp_suffix()?,
        epoch = receipt.epoch,
    );
    let tmp_path = spool_dir.join(tmp_name);

    match write_temp_artifact(&tmp_path, &encoded) {
        Ok(()) => {}
        Err(err) => return Err(temp_artifact_write_error(&tmp_path, err)),
    }

    install_artifact_without_overwrite(&tmp_path, &target_path, &encoded, "DA receipt artifact")?;

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
    let mut by_key: BTreeMap<(u32, u64, u64), (usize, ReplayFingerprint)> = BTreeMap::new();
    for entry in fs::read_dir(spool_dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(name) = artifact_file_name(&file_name, RECEIPT_FILE_PREFIX)? else {
            continue;
        };
        if !entry.file_type()?.is_file() {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!("DA receipt artifact `{name}` is not a regular file"),
            ));
        }
        let path = entry.path();
        let (receipt_key, stored) =
            DaReceiptLog::decode_receipt_with_key(&path).map_err(|err| {
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("failed to load DA receipt {}: {err}", path.display()),
                )
            })?;
        let key = (
            stored.receipt.lane_id.as_u32(),
            stored.receipt.epoch,
            stored.sequence,
        );
        if let Some((existing_idx, existing_fingerprint)) = by_key.get(&key).copied() {
            let existing: &StoredDaReceipt = &receipts[existing_idx];
            if existing.receipt != stored.receipt {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "conflicting duplicate DA receipt for lane {} epoch {} sequence {}",
                        key.0, key.1, key.2
                    ),
                ));
            }
            if existing_fingerprint != receipt_key.fingerprint {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "duplicate DA receipt fingerprint conflict for lane {} epoch {} sequence {}",
                        key.0, key.1, key.2
                    ),
                ));
            }
            continue;
        }
        by_key.insert(key, (receipts.len(), receipt_key.fingerprint));
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
    let (key, path) = load_single_spool_artifact_path_by_ticket(
        spool_dir,
        ticket,
        "manifest-",
        "manifest spool directory is not configured",
        "manifest not found for storage ticket",
        "multiple manifests found for storage ticket",
    )?;
    let bytes = fs::read(&path)?;
    validate_manifest_spool_body(&bytes, &key)?;
    Ok(bytes)
}

pub(super) fn load_pdp_commitment_from_spool(
    spool_dir: &Path,
    ticket: &StorageTicketId,
) -> std::io::Result<Vec<u8>> {
    let (_, path) = load_single_spool_artifact_path_by_ticket(
        spool_dir,
        ticket,
        "pdp-commitment-",
        "PDP spool directory is not configured",
        "PDP commitment not found for storage ticket",
        "multiple PDP commitments found for storage ticket",
    )?;
    let bytes = fs::read(path)?;
    validate_pdp_commitment_spool_body(&bytes)?;
    Ok(bytes)
}

fn load_single_spool_artifact_path_by_ticket(
    spool_dir: &Path,
    ticket: &StorageTicketId,
    prefix: &str,
    unconfigured_message: &'static str,
    not_found_message: &'static str,
    duplicate_message: &'static str,
) -> std::io::Result<(SpoolArtifactFileKey, PathBuf)> {
    if spool_dir.as_os_str().is_empty() {
        return Err(std::io::Error::new(
            ErrorKind::NotFound,
            unconfigured_message,
        ));
    }
    let mut matches = Vec::new();
    let entries = fs::read_dir(spool_dir)?;
    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(name) = artifact_file_name(&file_name, prefix)? else {
            continue;
        };
        if !entry.file_type()?.is_file() {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!("spool artifact `{name}` is not a regular file"),
            ));
        }
        let key = parse_spool_artifact_file_key(name, prefix).ok_or_else(|| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                format!("malformed spool artifact filename `{name}`"),
            )
        })?;
        if key.storage_ticket == *ticket {
            matches.push((key, entry.path()));
        }
    }
    if matches.is_empty() {
        return Err(std::io::Error::new(ErrorKind::NotFound, not_found_message));
    }
    if matches.len() > 1 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            duplicate_message,
        ));
    }
    matches.sort_by_key(|(key, _)| *key);
    Ok(matches.remove(0))
}

fn artifact_path_matches(path: &Path, prefix: &str) -> std::io::Result<bool> {
    let Some(name) = path.file_name() else {
        return Ok(false);
    };
    artifact_file_name(name, prefix).map(|name| name.is_some())
}

fn artifact_file_name<'a>(name: &'a OsStr, prefix: &str) -> std::io::Result<Option<&'a str>> {
    if let Some(name) = name.to_str() {
        return Ok((name.starts_with(prefix) && name.ends_with(".norito")).then_some(name));
    }
    if non_utf8_artifact_name_matches(name, prefix.as_bytes(), b".norito") {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!("spool artifact filename with prefix `{prefix}` is not valid UTF-8"),
        ));
    }
    Ok(None)
}

#[cfg(unix)]
fn non_utf8_artifact_name_matches(name: &OsStr, prefix: &[u8], suffix: &[u8]) -> bool {
    use std::os::unix::ffi::OsStrExt;

    let bytes = name.as_bytes();
    bytes.starts_with(prefix) && bytes.ends_with(suffix)
}

#[cfg(not(unix))]
fn non_utf8_artifact_name_matches(_name: &OsStr, _prefix: &[u8], _suffix: &[u8]) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn artifact_file_name_rejects_non_utf8_shaped_names() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        for (prefix, raw_name) in [
            ("manifest-", b"manifest-\xFF.norito".to_vec()),
            ("pdp-commitment-", b"pdp-commitment-\xFF.norito".to_vec()),
            (RECEIPT_FILE_PREFIX, b"da-receipt-\xFF.norito".to_vec()),
        ] {
            let name = OsString::from_vec(raw_name);
            let err = artifact_file_name(name.as_os_str(), prefix)
                .expect_err("non-UTF8 shaped artifact name rejects");
            assert_eq!(err.kind(), ErrorKind::InvalidData);
        }
    }

    #[cfg(unix)]
    #[test]
    fn artifact_file_name_ignores_unrelated_non_utf8_names() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let name = OsString::from_vec(b"unrelated-\xFF.norito".to_vec());
        assert!(
            artifact_file_name(name.as_os_str(), "manifest-")
                .expect("unrelated non-UTF8 name is ignored")
                .is_none()
        );
    }
}

fn validate_manifest_spool_body(bytes: &[u8], key: &SpoolArtifactFileKey) -> std::io::Result<()> {
    let manifest = decode_from_bytes::<DaManifestV1>(bytes).map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("manifest spool body is not a DA manifest: {err}"),
        )
    })?;

    if manifest.lane_id.as_u32() != key.lane_id
        || manifest.epoch != key.epoch
        || manifest.storage_ticket != key.storage_ticket
    {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "manifest spool filename tuple does not match manifest body",
        ));
    }

    let mut template = manifest.clone();
    template.storage_ticket = StorageTicketId::default();
    template.issued_at_unix = 0;
    let encoded_template = to_bytes(&template).map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("failed to encode manifest fingerprint template: {err}"),
        )
    })?;
    let fingerprint = ReplayFingerprint::from_hash(blake3::hash(&encoded_template));
    if *fingerprint.as_bytes() != key.fingerprint {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "manifest spool filename fingerprint does not match manifest body",
        ));
    }

    Ok(())
}

fn validate_pdp_commitment_spool_body(bytes: &[u8]) -> std::io::Result<()> {
    let commitment = decode_from_bytes::<PdpCommitmentV1>(bytes).map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("PDP commitment spool body is not a PDP commitment: {err}"),
        )
    })?;
    commitment.validate().map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("PDP commitment spool body is invalid: {err}"),
        )
    })
}

fn invalid_artifact_input(message: &'static str) -> std::io::Error {
    std::io::Error::new(ErrorKind::InvalidInput, message)
}

fn spool_artifact_key_from_inputs(
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: &StorageTicketId,
    fingerprint: &ReplayFingerprint,
) -> SpoolArtifactFileKey {
    SpoolArtifactFileKey {
        lane_id: lane_id.as_u32(),
        epoch,
        sequence,
        storage_ticket: *storage_ticket,
        fingerprint: *fingerprint.as_bytes(),
    }
}

fn validate_manifest_artifact_inputs(
    manifest_bytes: &[u8],
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: &StorageTicketId,
    fingerprint: &ReplayFingerprint,
) -> std::io::Result<()> {
    let key = spool_artifact_key_from_inputs(lane_id, epoch, sequence, storage_ticket, fingerprint);
    validate_manifest_spool_body(manifest_bytes, &key).map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("DA manifest artifact inputs do not match body: {err}"),
        )
    })
}

fn validate_pdp_commitment_artifact_inputs(commitment: &PdpCommitmentV1) -> std::io::Result<()> {
    commitment.validate().map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("PDP commitment artifact body is invalid: {err}"),
        )
    })
}

fn validate_da_commitment_artifact_inputs(
    record: &DaCommitmentRecord,
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: &StorageTicketId,
) -> std::io::Result<()> {
    if record.lane_id != lane_id
        || record.epoch != epoch
        || record.sequence != sequence
        || &record.storage_ticket != storage_ticket
    {
        return Err(invalid_artifact_input(
            "DA commitment artifact filename tuple does not match record body",
        ));
    }
    Ok(())
}

fn validate_da_schedule_artifact_inputs(
    record: &DaCommitmentRecord,
    pdp_commitment_bytes: &[u8],
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: &StorageTicketId,
) -> std::io::Result<()> {
    validate_da_commitment_artifact_inputs(record, lane_id, epoch, sequence, storage_ticket)?;
    validate_pdp_commitment_spool_body(pdp_commitment_bytes).map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("DA commitment schedule PDP body is invalid: {err}"),
        )
    })?;
    let pdp_digest = Hash::new(pdp_commitment_bytes);
    if record.proof_digest.as_ref() != Some(&pdp_digest) {
        return Err(invalid_artifact_input(
            "DA commitment schedule PDP bytes do not match record proof digest",
        ));
    }
    Ok(())
}

fn validate_pin_intent_artifact_inputs(
    intent: &DaPinIntent,
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: &StorageTicketId,
) -> std::io::Result<()> {
    if intent.lane_id != lane_id
        || intent.epoch != epoch
        || intent.sequence != sequence
        || &intent.storage_ticket != storage_ticket
    {
        return Err(invalid_artifact_input(
            "DA pin-intent artifact filename tuple does not match intent body",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SpoolArtifactFileKey {
    lane_id: u32,
    epoch: u64,
    sequence: u64,
    storage_ticket: StorageTicketId,
    fingerprint: [u8; 32],
}

fn parse_spool_artifact_file_key(name: &str, prefix: &str) -> Option<SpoolArtifactFileKey> {
    let rest = name.strip_prefix(prefix)?.strip_suffix(".norito")?;
    let mut fields = rest.split('-');
    let lane_hex = fields.next()?;
    let epoch_hex = fields.next()?;
    let sequence_hex = fields.next()?;
    let ticket_hex = fields.next()?;
    let fingerprint_hex = fields.next()?;
    if fields.next().is_some() {
        return None;
    }
    let lane_id = parse_fixed_hex_u32(lane_hex, 8)?;
    let epoch = parse_fixed_hex_u64(epoch_hex, 16)?;
    let sequence = parse_fixed_hex_u64(sequence_hex, 16)?;
    let storage_ticket = StorageTicketId::new(parse_fixed_hex_32(ticket_hex)?);
    let fingerprint = parse_fixed_hex_32(fingerprint_hex)?;
    Some(SpoolArtifactFileKey {
        lane_id,
        epoch,
        sequence,
        storage_ticket,
        fingerprint,
    })
}

fn parse_fixed_hex_u32(value: &str, width: usize) -> Option<u32> {
    (value.len() == width && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(value, 16).ok())
        .flatten()
}

fn parse_fixed_hex_u64(value: &str, width: usize) -> Option<u64> {
    (value.len() == width && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| u64::from_str_radix(value, 16).ok())
        .flatten()
}

fn parse_fixed_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = [0; 32];
    hex::decode_to_slice(value, &mut bytes).ok()?;
    Some(bytes)
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
    validate_manifest_artifact_inputs(
        manifest_bytes,
        lane_id,
        epoch,
        sequence,
        storage_ticket,
        fingerprint,
    )?;

    fs::create_dir_all(spool_dir)?;

    let lane = lane_id.as_u32();
    let ticket_hex = hex::encode(storage_ticket.as_ref());
    let fingerprint_hex = hex::encode(fingerprint.as_bytes());
    let file_name = format!(
        "manifest-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.norito"
    );
    let target_path = spool_dir.join(file_name);
    if let Some(path) =
        existing_artifact_path_if_matching(&target_path, manifest_bytes, "DA manifest artifact")?
    {
        return Ok(Some(path));
    }

    let tmp_name = format!(
        ".manifest-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.tmp-{}",
        artifact_temp_suffix()?
    );
    let tmp_path = spool_dir.join(tmp_name);

    match write_temp_artifact(&tmp_path, manifest_bytes) {
        Ok(()) => {}
        Err(err) => return Err(temp_artifact_write_error(&tmp_path, err)),
    }

    install_artifact_without_overwrite(
        &tmp_path,
        &target_path,
        manifest_bytes,
        "DA manifest artifact",
    )?;

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
    validate_pdp_commitment_artifact_inputs(commitment)?;

    fs::create_dir_all(spool_dir)?;

    let lane = lane_id.as_u32();
    let ticket_hex = hex::encode(storage_ticket.as_ref());
    let fingerprint_hex = hex::encode(fingerprint.as_bytes());
    let file_name = format!(
        "pdp-commitment-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.norito"
    );
    let target_path = spool_dir.join(file_name);
    let encoded =
        to_bytes(commitment).map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
    if let Some(path) =
        existing_artifact_path_if_matching(&target_path, &encoded, "PDP commitment artifact")?
    {
        return Ok(Some(path));
    }

    let tmp_name = format!(
        ".pdp-commitment-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.tmp-{}",
        artifact_temp_suffix()?
    );
    let tmp_path = spool_dir.join(tmp_name);

    match write_temp_artifact(&tmp_path, &encoded) {
        Ok(()) => {}
        Err(err) => return Err(temp_artifact_write_error(&tmp_path, err)),
    }

    install_artifact_without_overwrite(
        &tmp_path,
        &target_path,
        &encoded,
        "PDP commitment artifact",
    )?;

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
    validate_da_commitment_artifact_inputs(record, lane_id, epoch, sequence, storage_ticket)?;

    fs::create_dir_all(spool_dir)?;

    let lane = lane_id.as_u32();
    let ticket_hex = hex::encode(storage_ticket.as_ref());
    let fingerprint_hex = hex::encode(fingerprint.as_bytes());
    let file_name = format!(
        "da-commitment-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.norito"
    );
    let target_path = spool_dir.join(file_name);
    let encoded =
        to_bytes(record).map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
    if let Some(path) =
        existing_artifact_path_if_matching(&target_path, &encoded, "DA commitment artifact")?
    {
        return Ok(Some(path));
    }

    let tmp_name = format!(
        ".da-commitment-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.tmp-{}",
        artifact_temp_suffix()?
    );
    let tmp_path = spool_dir.join(tmp_name);

    match write_temp_artifact(&tmp_path, &encoded) {
        Ok(()) => {}
        Err(err) => return Err(temp_artifact_write_error(&tmp_path, err)),
    }

    install_artifact_without_overwrite(
        &tmp_path,
        &target_path,
        &encoded,
        "DA commitment artifact",
    )?;

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
    validate_da_schedule_artifact_inputs(
        record,
        pdp_commitment_bytes,
        lane_id,
        epoch,
        sequence,
        storage_ticket,
    )?;

    fs::create_dir_all(spool_dir)?;

    let lane = lane_id.as_u32();
    let ticket_hex = hex::encode(storage_ticket.as_ref());
    let fingerprint_hex = hex::encode(fingerprint.as_bytes());
    let file_name = format!(
        "da-commitment-schedule-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.norito"
    );
    let target_path = spool_dir.join(file_name);
    let entry = DaCommitmentScheduleEntry {
        version: DA_COMMITMENT_SCHEDULE_ENTRY_VERSION,
        record: record.clone(),
        pdp_commitment: pdp_commitment_bytes.to_vec(),
    };
    let encoded =
        to_bytes(&entry).map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
    if let Some(path) = existing_artifact_path_if_matching(
        &target_path,
        &encoded,
        "DA commitment schedule artifact",
    )? {
        return Ok(Some(path));
    }

    let tmp_name = format!(
        ".da-commitment-schedule-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.tmp-{}",
        artifact_temp_suffix()?
    );
    let tmp_path = spool_dir.join(tmp_name);

    match write_temp_artifact(&tmp_path, &encoded) {
        Ok(()) => {}
        Err(err) => return Err(temp_artifact_write_error(&tmp_path, err)),
    }

    install_artifact_without_overwrite(
        &tmp_path,
        &target_path,
        &encoded,
        "DA commitment schedule artifact",
    )?;

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
    validate_pin_intent_artifact_inputs(intent, lane_id, epoch, sequence, storage_ticket)?;

    fs::create_dir_all(spool_dir)?;

    let lane = lane_id.as_u32();
    let ticket_hex = hex::encode(storage_ticket.as_ref());
    let fingerprint_hex = hex::encode(fingerprint.as_bytes());
    let file_name = format!(
        "da-pin-intent-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.norito"
    );
    let target_path = spool_dir.join(file_name);
    let encoded =
        to_bytes(intent).map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
    if let Some(path) =
        existing_artifact_path_if_matching(&target_path, &encoded, "DA pin intent artifact")?
    {
        return Ok(Some(path));
    }

    let tmp_name = format!(
        ".da-pin-intent-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.tmp-{}",
        artifact_temp_suffix()?
    );
    let tmp_path = spool_dir.join(tmp_name);

    match write_temp_artifact(&tmp_path, &encoded) {
        Ok(()) => {}
        Err(err) => return Err(temp_artifact_write_error(&tmp_path, err)),
    }

    install_artifact_without_overwrite(
        &tmp_path,
        &target_path,
        &encoded,
        "DA pin intent artifact",
    )?;

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
