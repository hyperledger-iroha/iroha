//! Persistence helpers for DA replay cursors, receipts, and spool artifacts.

#![allow(clippy::redundant_pub_crate)]

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
const RECEIPT_SIGNING_PAYLOAD_VERSION: u16 = 1;
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

        verify_receipt_signature(&receipt, sequence, &self.signer_public_key)
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
            if let Err(err) = verify_receipt_signature(&receipt, sequence, signer_public_key) {
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
        validate_receipt_filename(path, &stored)?;
        Ok(stored)
    }

    fn is_receipt_file(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with(RECEIPT_FILE_PREFIX) && name.ends_with(".norito"))
            .unwrap_or(false)
    }
}

#[derive(Clone, Copy)]
struct ReceiptFileKey {
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: StorageTicketId,
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
    let _ = parse_fixed_hex_32(fingerprint_hex)
        .ok_or_else(|| eyre!("receipt filename fingerprint is not 32-byte hex"))?;

    Ok(ReceiptFileKey {
        lane_id,
        epoch,
        sequence,
        storage_ticket,
    })
}

fn validate_receipt_filename(path: &Path, stored: &StoredDaReceipt) -> eyre::Result<()> {
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
    Ok(())
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
            let _ = fs::remove_file(tmp_path);
            Ok(())
        }
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(tmp_path);
            existing_artifact_path_if_matching(target_path, expected, artifact).map(|_| ())
        }
        Err(err) => {
            let _ = fs::remove_file(tmp_path);
            Err(err)
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
        std::process::id(),
        epoch = receipt.epoch,
    );
    let tmp_path = spool_dir.join(tmp_name);

    match fs::write(&tmp_path, &encoded) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
    }

    install_artifact_without_overwrite(
        &tmp_path,
        &target_path,
        &encoded,
        "DA receipt artifact",
    )?;

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
        let stored = match DaReceiptLog::decode_receipt(&path) {
            Ok(stored) => stored,
            Err(err) => {
                warn!(
                    ?err,
                    path = %path.display(),
                    "failed to load DA receipt; skipping"
                );
                continue;
            }
        };
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
    fs::read(path)
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
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if let Some(key) = parse_spool_artifact_file_key(name, prefix) {
            if key.storage_ticket == *ticket {
                matches.push((key, entry.path()));
            }
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
        std::process::id()
    );
    let tmp_path = spool_dir.join(tmp_name);

    match fs::write(&tmp_path, &encoded) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
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
        std::process::id()
    );
    let tmp_path = spool_dir.join(tmp_name);

    match fs::write(&tmp_path, &encoded) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
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
        std::process::id()
    );
    let tmp_path = spool_dir.join(tmp_name);

    match fs::write(&tmp_path, &encoded) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
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
        std::process::id()
    );
    let tmp_path = spool_dir.join(tmp_name);

    match fs::write(&tmp_path, &encoded) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
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
