//! Data availability ingest handlers and persistence helpers for Torii.

use std::{
    borrow::{Cow, ToOwned},
    collections::{BTreeMap, HashMap},
    fs::{self, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use axum::{
    extract::{Path as AxumPath, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::Response,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use blake3::{Hasher as Blake3Hasher, hash as blake3_hash};
use eyre::{ContextCompat, WrapErr, eyre};
use flate2::read::{DeflateDecoder, GzDecoder};
use iroha_config::parameters::actual::{DaTaikaiAnchor, LaneConfig as ConfigLaneConfig};
use iroha_core::da::{LaneEpoch, ReplayCache, ReplayFingerprint, ReplayInsertOutcome, ReplayKey};
use iroha_crypto::{
    Hash, KeyPair, PublicKey, Signature,
    encryption::{ChaCha20Poly1305, SymmetricEncryptor},
};
use iroha_data_model::{
    account::AccountId,
    da::{
        commitment::{DaCommitmentRecord, DaProofScheme, KzgCommitment, RetentionClass},
        manifest::ChunkRole,
        pin_intent::DaPinIntent,
        prelude::*,
    },
    name::Name,
    nexus::LaneId,
    sorafs::{
        capacity::ProviderId,
        pin_registry::{ManifestDigest, StorageClass},
    },
    taikai::{
        GuardDirectoryId, SegmentDuration, SegmentTimestamp, TaikaiAliasBinding, TaikaiAudioLayout,
        TaikaiAvailabilityClass, TaikaiCarPointer, TaikaiCodec, TaikaiEnvelopeIndexes,
        TaikaiEventId, TaikaiGuardPolicy, TaikaiIngestPointer, TaikaiParseError, TaikaiRenditionId,
        TaikaiRenditionRouteV1, TaikaiResolution, TaikaiRoutingManifestV1, TaikaiSegmentEnvelopeV1,
        TaikaiSegmentSigningManifestV1, TaikaiSegmentWindow, TaikaiStreamId, TaikaiTrackKind,
        TaikaiTrackMetadata,
    },
};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_logger::{debug, error, info, warn};
#[cfg(test)]
use iroha_test_samples::ALICE_ID;
use iroha_torii_shared::da::sampling::{
    build_sampling_plan, compute_sample_window, sampling_plan_to_value,
};
use iroha_zkp_halo2::pallas::{
    Params as IpaCurveParams, Polynomial as IpaPolynomial, Scalar as IpaScalar,
};
use norito::{
    core::NoritoDeserialize,
    decode_from_bytes, from_bytes,
    json::{self, JsonDeserialize, JsonSerialize, Map, Value},
    to_bytes,
};
use reqwest::Client;
use sorafs_car::{ChunkStore, build_plan_from_da_manifest, fetch_plan::chunk_fetch_specs_to_json};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, ChunkingProfileV1,
    deal::XorAmount,
    pdp::{HashAlgorithmV1, PDP_COMMITMENT_VERSION_V1, PdpCommitmentV1},
};
use zstd::stream::decode_all as zstd_decode_all;

use crate::{
    NoritoQuery, SharedAppState,
    routing::MaybeTelemetry,
    sorafs::api::ResponseError,
    utils::{self, ResponseFormat},
};

const CURSOR_FILE_NAME: &str = "replay_cursors.norito.json";
const RECEIPT_FILE_PREFIX: &str = "da-receipt";
const HEADER_SORA_PDP_COMMITMENT: &str = "sora-pdp-commitment";
const RECEIPT_SIGNATURE_PLACEHOLDER: [u8; 64] = [0; 64];
const TAIKAI_SPOOL_SUBDIR: &str = "taikai";
const META_TAIKAI_EVENT_ID: &str = "taikai.event_id";
const META_TAIKAI_STREAM_ID: &str = "taikai.stream_id";
const META_TAIKAI_RENDITION_ID: &str = "taikai.rendition_id";
const META_TAIKAI_TRACK_KIND: &str = "taikai.track.kind";
const META_TAIKAI_TRACK_CODEC: &str = "taikai.track.codec";
const META_TAIKAI_TRACK_BITRATE: &str = "taikai.track.bitrate_kbps";
const META_TAIKAI_TRACK_RESOLUTION: &str = "taikai.track.resolution";
const META_TAIKAI_TRACK_AUDIO_LAYOUT: &str = "taikai.track.audio_layout";
const META_TAIKAI_SEGMENT_SEQUENCE: &str = "taikai.segment.sequence";
const META_TAIKAI_SEGMENT_START: &str = "taikai.segment.start_pts";
const META_TAIKAI_SEGMENT_DURATION: &str = "taikai.segment.duration";
const META_TAIKAI_WALLCLOCK_MS: &str = "taikai.wallclock_unix_ms";
const META_TAIKAI_INGEST_LATENCY_MS: &str = "taikai.instrumentation.ingest_latency_ms";
const META_TAIKAI_LIVE_EDGE_DRIFT_MS: &str = "taikai.instrumentation.live_edge_drift_ms";
const META_TAIKAI_INGEST_NODE_ID: &str = "taikai.instrumentation.ingest_node_id";
const META_TAIKAI_SSM: &str = "taikai.ssm";
const META_TAIKAI_TRM: &str = "taikai.trm";
const META_TAIKAI_AVAILABILITY_CLASS: &str = "taikai.availability_class";
const META_TAIKAI_REPLICATION_REPLICAS: &str = "taikai.replication.replicas";
const META_TAIKAI_REPLICATION_STORAGE: &str = "taikai.replication.storage_class";
const META_TAIKAI_REPLICATION_HOT_SECS: &str = "taikai.replication.hot_retention_secs";
const META_TAIKAI_REPLICATION_COLD_SECS: &str = "taikai.replication.cold_retention_secs";
const META_TAIKAI_CACHE_HINT: &str = "taikai.cache_hint";
const META_DA_PROOF_TIER: &str = "da.proof.tier";
const META_DA_PDP_SAMPLE_WINDOW: &str = "da.proof.pdp.sample_window";
const META_DA_POTR_SAMPLE_WINDOW: &str = "da.proof.potr.sample_window";
const META_DA_REGISTRY_ALIAS: &str = "da.registry.alias";
const META_DA_REGISTRY_OWNER: &str = "da.registry.owner";
const TAIKAI_ANCHOR_SENTINEL_PREFIX: &str = "taikai-anchor-";
const TAIKAI_ANCHOR_SENTINEL_SUFFIX: &str = ".ok";
const TAIKAI_ANCHOR_REQUEST_PREFIX: &str = "taikai-anchor-request-";
const TAIKAI_ANCHOR_REQUEST_SUFFIX: &str = ".json";
const TAIKAI_TRM_LINEAGE_PREFIX: &str = "taikai-trm-state-";
const TAIKAI_TRM_LINEAGE_SUFFIX: &str = ".json";
const TAIKAI_TRM_LOCK_PREFIX: &str = "taikai-trm-lock-";
const TAIKAI_TRM_LOCK_SUFFIX: &str = ".lock";
const TAIKAI_TRM_LOCK_STALE_SECS: u64 = 300;
const TAIKAI_LINEAGE_HINT_PREFIX: &str = "taikai-lineage";
const BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;
const SECS_PER_MONTH: u64 = 30 * 24 * 60 * 60;

mod rs16 {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex, OnceLock},
    };

    const FIELD_ORDER: usize = 1 << 16;
    const FIELD_MASK: u32 = 0x1_0000;
    const FIELD_POLY: u32 = 0x1_100B; // x^16 + x^12 + x^3 + x + 1
    const ORDER_MINUS_ONE: usize = FIELD_ORDER - 1;

    struct FieldTables {
        exp: Vec<u16>,
        log: Vec<u16>,
    }

    fn tables() -> &'static FieldTables {
        static TABLES: OnceLock<FieldTables> = OnceLock::new();
        TABLES.get_or_init(|| {
            let mut exp = vec![0u16; ORDER_MINUS_ONE * 2];
            let mut log = vec![0u16; FIELD_ORDER];

            let mut value: u32 = 1;
            for (idx, slot) in exp.iter_mut().take(ORDER_MINUS_ONE).enumerate() {
                *slot = value as u16;
                log[value as usize] = idx as u16;

                value <<= 1;
                if (value & FIELD_MASK) != 0 {
                    value ^= FIELD_POLY;
                }
                value &= FIELD_MASK - 1;
            }

            let (lower, upper) = exp.split_at_mut(ORDER_MINUS_ONE);
            upper.copy_from_slice(lower);

            FieldTables { exp, log }
        })
    }

    #[inline]
    fn gf_add(a: u16, b: u16) -> u16 {
        a ^ b
    }

    #[inline]
    fn gf_mul(a: u16, b: u16) -> u16 {
        if a == 0 || b == 0 {
            return 0;
        }
        let tables = tables();
        let log_a = tables.log[a as usize] as usize;
        let log_b = tables.log[b as usize] as usize;
        tables.exp[log_a + log_b]
    }

    #[inline]
    fn gf_inv(value: u16) -> Option<u16> {
        if value == 0 {
            return None;
        }
        let tables = tables();
        let log_v = tables.log[value as usize] as usize;
        Some(tables.exp[ORDER_MINUS_ONE - log_v])
    }

    #[inline]
    fn gf_pow(exp: usize) -> u16 {
        let tables = tables();
        tables.exp[exp % ORDER_MINUS_ONE]
    }

    #[allow(clippy::needless_range_loop)]
    fn invert_matrix(mut matrix: Vec<Vec<u16>>) -> Result<Vec<Vec<u16>>, ()> {
        let size = matrix.len();
        if size == 0 {
            return Ok(Vec::new());
        }
        let width = matrix[0].len();
        if size != width {
            return Err(());
        }
        let mut identity = vec![vec![0u16; size]; size];
        for i in 0..size {
            identity[i][i] = 1;
        }

        for col in 0..size {
            let mut pivot_row = None;
            for row in col..size {
                if matrix[row][col] != 0 {
                    pivot_row = Some(row);
                    break;
                }
            }
            let pivot_row = pivot_row.ok_or(())?;
            if pivot_row != col {
                matrix.swap(pivot_row, col);
                identity.swap(pivot_row, col);
            }
            let inv = gf_inv(matrix[col][col]).ok_or(())?;
            for j in 0..size {
                matrix[col][j] = gf_mul(matrix[col][j], inv);
                identity[col][j] = gf_mul(identity[col][j], inv);
            }
            for row in 0..size {
                if row == col {
                    continue;
                }
                let factor = matrix[row][col];
                if factor == 0 {
                    continue;
                }
                for j in 0..size {
                    let term = gf_mul(factor, matrix[col][j]);
                    matrix[row][j] = gf_add(matrix[row][j], term);
                    let term = gf_mul(factor, identity[col][j]);
                    identity[row][j] = gf_add(identity[row][j], term);
                }
            }
        }

        Ok(identity)
    }

    #[derive(Clone)]
    struct ParityMatrix {
        rows: Vec<Vec<u16>>,
    }

    fn parity_matrix(data_shards: usize, parity_shards: usize) -> Result<Arc<ParityMatrix>, ()> {
        static CACHE: OnceLock<Mutex<HashMap<(usize, usize), Arc<ParityMatrix>>>> = OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Some(entry) = cache
            .lock()
            .expect("mutex poisoned")
            .get(&(data_shards, parity_shards))
        {
            return Ok(entry.clone());
        }

        let total = data_shards + parity_shards;
        let mut vandermonde = vec![vec![0u16; data_shards]; total];
        for (row_idx, row) in vandermonde.iter_mut().enumerate() {
            for (col_idx, value) in row.iter_mut().enumerate() {
                *value = if row_idx == 0 || col_idx == 0 {
                    1
                } else {
                    gf_pow(row_idx * col_idx)
                };
            }
        }

        let data_block = invert_matrix(vandermonde[..data_shards].to_vec())?;
        let mut parity_rows = vec![vec![0u16; data_shards]; parity_shards];
        for (parity_idx, parity_row) in parity_rows.iter_mut().enumerate() {
            let source = &vandermonde[data_shards + parity_idx];
            for (col_idx, slot) in parity_row.iter_mut().enumerate() {
                let mut acc = 0u16;
                for (data_idx, src_val) in source.iter().take(data_shards).enumerate() {
                    let term = gf_mul(*src_val, data_block[data_idx][col_idx]);
                    acc = gf_add(acc, term);
                }
                *slot = acc;
            }
        }

        let matrix = Arc::new(ParityMatrix { rows: parity_rows });
        cache
            .lock()
            .expect("mutex poisoned")
            .insert((data_shards, parity_shards), matrix.clone());
        Ok(matrix)
    }

    pub fn encode_parity(
        data_symbols: &[Vec<u16>],
        parity_count: usize,
    ) -> Result<Vec<Vec<u16>>, ()> {
        if data_symbols.is_empty() || parity_count == 0 {
            return Ok(Vec::new());
        }
        let symbol_count = data_symbols[0].len();
        if data_symbols.iter().any(|row| row.len() != symbol_count) {
            return Err(());
        }
        let matrix = parity_matrix(data_symbols.len(), parity_count)?;
        let mut parity = vec![vec![0u16; symbol_count]; parity_count];

        for (row_idx, coeffs) in matrix.rows.iter().enumerate() {
            let row = &mut parity[row_idx];
            for (data_idx, coef) in coeffs.iter().enumerate() {
                if *coef == 0 {
                    continue;
                }
                let data_row = &data_symbols[data_idx];
                for (sym_idx, symbol) in data_row.iter().enumerate() {
                    let term = gf_mul(*coef, *symbol);
                    row[sym_idx] ^= term;
                }
            }
        }

        Ok(parity)
    }
}

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

fn replay_cursor_temp_path(path: &Path) -> PathBuf {
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
struct StoredDaReceipt {
    version: u16,
    sequence: u64,
    receipt: DaIngestReceipt,
}

const STORED_RECEIPT_VERSION: u16 = 1;

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

fn unsigned_receipt_bytes(receipt: &DaIngestReceipt) -> eyre::Result<Vec<u8>> {
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

#[derive(Debug)]
struct CanonicalPayload<'a> {
    bytes: Cow<'a, [u8]>,
}

impl CanonicalPayload<'_> {
    fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    fn len(&self) -> usize {
        self.bytes.len()
    }

    fn into_vec(self) -> Vec<u8> {
        self.bytes.into_owned()
    }
}

/// HTTP handler for `/v1/da/ingest`.
pub async fn handler_post_da_ingest(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    utils::extractors::JsonOnly(request): utils::extractors::JsonOnly<DaIngestRequest>,
) -> Result<Response, ResponseError> {
    let telemetry = app.telemetry_handle();
    let cluster_label = app
        .da_ingest
        .telemetry_cluster_label
        .as_deref()
        .unwrap_or("default");
    let nexus = app.state.nexus_snapshot();
    let format = utils::negotiate_response_format(headers.get(axum::http::header::ACCEPT))
        .map_err(ResponseError::from)?;

    if !nexus.enabled {
        return Err(ResponseError::from(build_error_response(
            StatusCode::BAD_REQUEST,
            "/v1/da/ingest requires nexus.enabled=true; lanes are unavailable in Iroha 2 mode",
            format,
        )));
    }

    let canonical = normalize_payload(&request).map_err(|(status, message)| {
        ResponseError::from(build_error_response(status, &message, format))
    })?;

    validate_request(&request, canonical.len()).map_err(|(status, message)| {
        ResponseError::from(build_error_response(status, message, format))
    })?;

    let proof_scheme =
        lane_proof_scheme(&nexus.lane_config, request.lane_id).map_err(|(status, message)| {
            ResponseError::from(build_error_response(status, &message, format))
        })?;

    let mut metadata = encrypt_governance_metadata(
        &request.metadata,
        app.da_ingest.governance_metadata_key.as_ref(),
        app.da_ingest.governance_metadata_key_label.as_deref(),
    )
    .map_err(|(status, message)| {
        ResponseError::from(build_error_response(status, &message, format))
    })?;

    let mut taikai_ssm_payload =
        taikai_ingest::take_ssm_entry(&mut metadata).map_err(|(status, message)| {
            ResponseError::from(build_error_response(status, &message, format))
        })?;
    let mut taikai_trm_payload =
        taikai_ingest::take_trm_entry(&mut metadata).map_err(|(status, message)| {
            ResponseError::from(build_error_response(status, &message, format))
        })?;

    let taikai_availability = if matches!(request.blob_class, BlobClass::TaikaiSegment) {
        taikai_availability_from_metadata(&request.metadata, taikai_trm_payload.as_deref())
            .map_err(|(status, message)| {
                ResponseError::from(build_error_response(status, &message, format))
            })?
    } else {
        None
    };

    let (expected_retention, retention_mismatch) = app.da_ingest.replication_policy.enforce(
        request.blob_class,
        taikai_availability,
        &request.retention_policy,
    );
    let enforced_retention = expected_retention.clone();
    if retention_mismatch {
        warn!(
            blob_class = ?request.blob_class,
            submitted = ?request.retention_policy,
            expected = ?enforced_retention,
            "overriding DA retention policy to match configured network baseline"
        );
    }

    if matches!(request.blob_class, BlobClass::TaikaiSegment) {
        let payload_digest = BlobDigest::from_hash(blake3_hash(canonical.as_slice()));
        apply_taikai_ingest_tags(
            &mut metadata,
            taikai_availability,
            &enforced_retention,
            payload_digest,
            request.total_size,
        )
        .map_err(|(status, message)| {
            ResponseError::from(build_error_response(status, &message, format))
        })?;
    }

    let chunk_store = build_chunk_store(&request, canonical.as_slice());

    let fingerprint = compute_fingerprint(&request, canonical.as_slice());
    let lane_epoch = LaneEpoch::new(request.lane_id, request.epoch);
    let replay_key = ReplayKey::new(lane_epoch, request.sequence, fingerprint);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);

    let outcome = app.da_replay_cache.insert(replay_key, Instant::now());

    match outcome {
        ReplayInsertOutcome::Fresh { .. } | ReplayInsertOutcome::Duplicate { .. } => {
            if matches!(outcome, ReplayInsertOutcome::Fresh { .. }) {
                if let Err(err) = app.da_replay_store.record(lane_epoch, request.sequence) {
                    warn!(?err, "failed to persist DA replay cursor");
                }
            }
            let duplicate = matches!(outcome, ReplayInsertOutcome::Duplicate { .. });
            let queued_at_secs = now.as_secs();
            let manifest = resolve_manifest(
                &request,
                &chunk_store,
                canonical.as_slice(),
                &metadata,
                &enforced_retention,
                queued_at_secs,
                &fingerprint,
                &app.da_ingest.rent_policy,
            )
            .map_err(|(status, message)| {
                ResponseError::from(build_error_response(status, &message, format))
            })?;
            let (rent_gib, rent_months) =
                rent_usage_from_request(request.total_size, &enforced_retention);
            record_da_rent_quote_metrics(
                &telemetry,
                cluster_label,
                enforced_retention.storage_class,
                rent_gib,
                rent_months,
                &manifest.manifest.rent_quote,
            );

            if let Err(err) = persist_manifest_for_sorafs(
                &app.da_ingest.manifest_store_dir,
                &manifest.encoded,
                request.lane_id,
                request.epoch,
                request.sequence,
                &manifest.storage_ticket,
                &fingerprint,
            ) {
                error!(
                    ?err,
                    spool_dir = ?app.da_ingest.manifest_store_dir,
                    ticket = %hex::encode(manifest.storage_ticket.as_ref()),
                    "failed to enqueue DA manifest for SoraFS orchestration"
                );
            }

            let pdp_commitment = compute_pdp_commitment(
                &manifest.manifest_hash,
                &manifest.manifest,
                &chunk_store,
                queued_at_secs,
            )
            .map_err(|(status, message)| {
                ResponseError::from(build_error_response(status, &message, format))
            })?;
            let pdp_commitment_bytes =
                encode_pdp_commitment_bytes(&pdp_commitment).map_err(|(status, message)| {
                    ResponseError::from(build_error_response(status, &message, format))
                })?;
            let pdp_header_value = pdp_commitment_header_value(&pdp_commitment_bytes).map_err(
                |(status, message)| {
                    ResponseError::from(build_error_response(status, &message, format))
                },
            )?;

            if let Err(err) = persist_pdp_commitment(
                &app.da_ingest.manifest_store_dir,
                &pdp_commitment,
                request.lane_id,
                request.epoch,
                request.sequence,
                &manifest.storage_ticket,
                &fingerprint,
            ) {
                error!(
                    ?err,
                    spool_dir = ?app.da_ingest.manifest_store_dir,
                    ticket = %hex::encode(manifest.storage_ticket.as_ref()),
                    "failed to enqueue PDP commitment for SoraFS orchestration"
                );
            }

            let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
            let receipt = build_receipt(
                &app.da_receipt_signer,
                &request,
                queued_at_secs,
                manifest.blob_hash,
                manifest.chunk_root,
                manifest.manifest_hash,
                manifest.storage_ticket,
                pdp_commitment_bytes.clone(),
                manifest.manifest.rent_quote,
                stripe_layout,
            );
            let commitment_record = build_da_commitment_record(
                &request,
                &manifest,
                &enforced_retention,
                &receipt.operator_signature,
                &pdp_commitment_bytes,
                proof_scheme,
            );
            if let Err(err) = persist_da_commitment_record(
                &app.da_ingest.manifest_store_dir,
                &commitment_record,
                request.lane_id,
                request.epoch,
                request.sequence,
                &manifest.storage_ticket,
                &fingerprint,
            ) {
                error!(
                    ?err,
                    spool_dir = ?app.da_ingest.manifest_store_dir,
                    ticket = %hex::encode(manifest.storage_ticket.as_ref()),
                    "failed to enqueue DA commitment record for bundle ingestion"
                );
            }
            if let Err(err) = persist_da_commitment_schedule_entry(
                &app.da_ingest.manifest_store_dir,
                &commitment_record,
                &pdp_commitment_bytes,
                request.lane_id,
                request.epoch,
                request.sequence,
                &manifest.storage_ticket,
                &fingerprint,
            ) {
                error!(
                    ?err,
                    spool_dir = ?app.da_ingest.manifest_store_dir,
                    ticket = %hex::encode(manifest.storage_ticket.as_ref()),
                    "failed to enqueue DA commitment schedule entry"
                );
            }
            let pin_alias =
                registry_alias_from_metadata(&request.metadata).map_err(|(status, message)| {
                    ResponseError::from(build_error_response(status, &message, format))
                })?;
            let pin_owner =
                registry_owner_from_metadata(&request.metadata).map_err(|(status, message)| {
                    ResponseError::from(build_error_response(status, &message, format))
                })?;
            let mut pin_intent = DaPinIntent::new(
                request.lane_id,
                request.epoch,
                request.sequence,
                manifest.storage_ticket,
                ManifestDigest::new(*manifest.manifest_hash.as_bytes()),
            );
            pin_intent.alias = pin_alias;
            pin_intent.owner = pin_owner;
            if let Err(err) = persist_da_pin_intent(
                &app.da_ingest.manifest_store_dir,
                &pin_intent,
                request.lane_id,
                request.epoch,
                request.sequence,
                &manifest.storage_ticket,
                &fingerprint,
            ) {
                error!(
                    ?err,
                    spool_dir = ?app.da_ingest.manifest_store_dir,
                    ticket = %hex::encode(manifest.storage_ticket.as_ref()),
                    "failed to enqueue DA pin intent for registry ingestion"
                );
            }

            if let Err(err) = persist_da_receipt(
                &app.da_ingest.manifest_store_dir,
                &receipt,
                request.sequence,
                &fingerprint,
            ) {
                error!(
                    ?err,
                    spool_dir = ?app.da_ingest.manifest_store_dir,
                    lane = request.lane_id.as_u32(),
                    epoch = request.epoch,
                    sequence = request.sequence,
                    "failed to enqueue DA receipt for downstream fanout"
                );
            }
            match app.da_receipt_log.append(
                lane_epoch,
                request.sequence,
                receipt.clone(),
                fingerprint,
            ) {
                Ok(outcome) => {
                    record_da_receipt_metrics(&telemetry, lane_epoch, request.sequence, &outcome);
                }
                Err(err) => {
                    warn!(
                        ?err,
                        ?lane_epoch,
                        sequence = request.sequence,
                        "failed to record DA receipt in durable log"
                    );
                    record_da_receipt_error_metrics(&telemetry, lane_epoch, request.sequence);
                }
            }

            if matches!(request.blob_class, BlobClass::TaikaiSegment) {
                let taikai = match taikai_ingest::build_envelope(
                    &request,
                    &manifest,
                    &chunk_store,
                    canonical.as_slice(),
                ) {
                    Ok(value) => value,
                    Err((status, message)) => {
                        let stream_label = stream_label_from_metadata(&request.metadata)
                            .unwrap_or_else(|| taikai_ingest::STREAM_LABEL_FALLBACK.to_string());
                        record_taikai_ingest_error(
                            &telemetry,
                            cluster_label,
                            &stream_label,
                            status,
                        );
                        return Err(ResponseError::from(build_error_response(
                            status, &message, format,
                        )));
                    }
                };

                if let Err(err) = taikai_ingest::persist_envelope(
                    &app.da_ingest.manifest_store_dir,
                    request.lane_id,
                    request.epoch,
                    request.sequence,
                    &manifest.storage_ticket,
                    &fingerprint,
                    &taikai.envelope_bytes,
                ) {
                    error!(
                        ?err,
                        spool_dir = ?app.da_ingest.manifest_store_dir,
                        "failed to enqueue Taikai envelope for anchoring"
                    );
                }

                if let Err(err) = taikai_ingest::persist_indexes(
                    &app.da_ingest.manifest_store_dir,
                    request.lane_id,
                    request.epoch,
                    request.sequence,
                    &manifest.storage_ticket,
                    &fingerprint,
                    &taikai.indexes_json,
                ) {
                    error!(
                        ?err,
                        spool_dir = ?app.da_ingest.manifest_store_dir,
                        "failed to enqueue Taikai index bundle for anchoring"
                    );
                }

                let ssm_bytes = taikai_ssm_payload.take().ok_or_else(|| {
                    build_error_response(
                        StatusCode::BAD_REQUEST,
                        "metadata entry `taikai.ssm` is required for Taikai segments",
                        format,
                    )
                })?;

                let ssm_outcome = validate_taikai_ssm(
                    &ssm_bytes,
                    &manifest.manifest_hash,
                    &taikai.car_digest,
                    &taikai.envelope_bytes,
                    taikai.telemetry.segment_sequence,
                    &app.sorafs_alias_cache_policy,
                    &telemetry,
                )
                .map_err(|(status, message)| {
                    ResponseError::from(build_error_response(status, &message, format))
                })?;

                if let Err(err) = taikai_ingest::persist_ssm(
                    &app.da_ingest.manifest_store_dir,
                    request.lane_id,
                    request.epoch,
                    request.sequence,
                    &manifest.storage_ticket,
                    &fingerprint,
                    &ssm_bytes,
                ) {
                    error!(
                        ?err,
                        spool_dir = ?app.da_ingest.manifest_store_dir,
                        "failed to enqueue Taikai signing manifest for anchoring"
                    );
                }

                iroha_logger::info!(
                    manifest_hash = %hex::encode(manifest.manifest_hash.as_ref()),
                    alias = %ssm_outcome.alias_label,
                    ssm_digest = %hex::encode(ssm_outcome.ssm_digest.as_ref()),
                    "accepted Taikai signing manifest"
                );

                if let Some(trm_bytes) = taikai_trm_payload.take() {
                    let routing_manifest = validate_taikai_trm(&trm_bytes, &taikai).map_err(
                        |(status, message): (StatusCode, String)| {
                            ResponseError::from(build_error_response(status, &message, format))
                        },
                    )?;
                    let manifest_digest_hex = hex::encode(blake3_hash(&trm_bytes).as_bytes());
                    let mut lineage_guard = taikai_ingest::TrmLineageGuard::new(
                        &app.da_ingest.manifest_store_dir,
                        &routing_manifest.alias_binding,
                    )
                    .map_err(|(status, message): (StatusCode, String)| {
                        ResponseError::from(build_error_response(status, &message, format))
                    })?;
                    if let Some(guard) = lineage_guard.as_mut() {
                        guard
                            .validate(&routing_manifest, &manifest_digest_hex)
                            .map_err(|(status, message): (StatusCode, String)| {
                                ResponseError::from(build_error_response(status, &message, format))
                            })?;
                    }

                    match taikai_ingest::persist_trm(
                        &app.da_ingest.manifest_store_dir,
                        request.lane_id,
                        request.epoch,
                        request.sequence,
                        &manifest.storage_ticket,
                        &fingerprint,
                        &trm_bytes,
                    ) {
                        Ok(persisted) => {
                            if persisted.is_some() {
                                if let Some(guard) = lineage_guard.as_mut() {
                                    guard
                                        .persist_lineage_hint(
                                            request.lane_id,
                                            request.epoch,
                                            request.sequence,
                                            &manifest.storage_ticket,
                                            &fingerprint,
                                        )
                                        .map_err(|(status, message): (StatusCode, String)| {
                                            ResponseError::from(build_error_response(
                                                status, &message, format,
                                            ))
                                        })?;
                                    guard
                                        .commit(
                                            routing_manifest.segment_window,
                                            &manifest_digest_hex,
                                        )
                                        .map_err(|(status, message): (StatusCode, String)| {
                                            ResponseError::from(build_error_response(
                                                status, &message, format,
                                            ))
                                        })?;
                                }
                            }
                            record_taikai_alias_rotation_event(
                                &telemetry,
                                cluster_label,
                                &routing_manifest,
                                &manifest_digest_hex,
                            );
                        }
                        Err(err) => {
                            error!(
                                ?err,
                                spool_dir = ?app.da_ingest.manifest_store_dir,
                                "failed to enqueue Taikai routing manifest for anchoring"
                            );
                        }
                    }
                }

                record_taikai_ingest_metrics(&telemetry, cluster_label, &taikai.telemetry);
            }

            let response = DaIngestResponse {
                status: "accepted",
                duplicate,
                receipt: Some(receipt),
            };
            let mut http_response = utils::respond_with_format(response, format);
            http_response.headers_mut().insert(
                HeaderName::from_static(HEADER_SORA_PDP_COMMITMENT),
                pdp_header_value,
            );
            Ok(with_status(http_response, StatusCode::ACCEPTED))
        }
        ReplayInsertOutcome::StaleSequence { highest_observed } => {
            let message = format!(
                "sequence {} is too far behind; highest observed is {}",
                request.sequence, highest_observed
            );
            Ok(build_error_response(StatusCode::CONFLICT, &message, format))
        }
        ReplayInsertOutcome::ConflictingFingerprint { .. } => Ok(build_error_response(
            StatusCode::CONFLICT,
            "sequence already used for a different manifest",
            format,
        )),
    }
}

#[derive(
    Debug, Default, Clone, crate::json_macros::JsonDeserialize, norito::derive::NoritoDeserialize,
)]
pub struct DaManifestQuery {
    block_hash: Option<String>,
}

/// HTTP handler for `/v1/da/manifests/{ticket}`.
pub async fn handler_get_da_manifest(
    State(app): State<SharedAppState>,
    AxumPath(ticket_hex): AxumPath<String>,
    NoritoQuery(params): NoritoQuery<DaManifestQuery>,
    headers: HeaderMap,
) -> Result<Response, ResponseError> {
    let format = utils::negotiate_response_format(headers.get(axum::http::header::ACCEPT))
        .map_err(ResponseError::from)?;

    let nexus_enabled = app.state.nexus_snapshot().enabled;
    if !nexus_enabled {
        return Err(ResponseError::from(build_error_response(
            StatusCode::BAD_REQUEST,
            "/v1/da/manifests requires nexus.enabled=true; lanes are unavailable in Iroha 2 mode",
            format,
        )));
    }

    let ticket_bytes = match parse_storage_ticket_hex(ticket_hex.trim()) {
        Ok(bytes) => bytes,
        Err(message) => {
            return Err(ResponseError::from(build_error_response(
                StatusCode::BAD_REQUEST,
                &message,
                format,
            )));
        }
    };
    let ticket = StorageTicketId::new(ticket_bytes);

    let manifest_bytes = match load_manifest_from_spool(&app.da_ingest.manifest_store_dir, &ticket)
    {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Err(ResponseError::from(build_error_response(
                StatusCode::NOT_FOUND,
                "manifest not found for storage ticket",
                format,
            )));
        }
        Err(err) => {
            return Err(ResponseError::from(build_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("failed to read manifest from spool: {err}"),
                format,
            )));
        }
    };

    let manifest: DaManifestV1 = match decode_from_bytes(&manifest_bytes) {
        Ok(manifest) => manifest,
        Err(err) => {
            return Err(ResponseError::from(build_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("failed to decode stored manifest: {err}"),
                format,
            )));
        }
    };

    let plan = match build_plan_from_da_manifest(&manifest) {
        Ok(plan) => plan,
        Err(err) => {
            return Err(ResponseError::from(build_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("failed to derive chunk plan from manifest: {err}"),
                format,
            )));
        }
    };

    let chunk_plan = chunk_fetch_specs_to_json(&plan);
    let manifest_json = match json::to_value(&manifest) {
        Ok(value) => value,
        Err(err) => {
            return Err(ResponseError::from(build_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("failed to render manifest JSON: {err}"),
                format,
            )));
        }
    };
    let manifest_hash = BlobDigest::from_hash(blake3_hash(&manifest_bytes));

    let sampling_plan = if let Some(block_hex) = params.block_hash.as_deref() {
        let block_hash = match parse_block_hash_hex(block_hex) {
            Ok(hash) => hash,
            Err(message) => {
                return Err(ResponseError::from(build_error_response(
                    StatusCode::BAD_REQUEST,
                    &message,
                    format,
                )));
            }
        };
        Some(build_sampling_plan(&manifest, &block_hash))
    } else {
        None
    };

    let mut body = Map::new();
    body.insert(
        "storage_ticket".into(),
        Value::from(hex::encode(ticket.as_bytes())),
    );
    body.insert(
        "client_blob_id".into(),
        Value::from(hex::encode(manifest.client_blob_id.as_bytes())),
    );
    body.insert(
        "blob_hash".into(),
        Value::from(hex::encode(manifest.blob_hash.as_bytes())),
    );
    body.insert(
        "chunk_root".into(),
        Value::from(hex::encode(manifest.chunk_root.as_bytes())),
    );
    body.insert(
        "manifest_hash".into(),
        Value::from(hex::encode(manifest_hash.as_bytes())),
    );
    body.insert("lane_id".into(), Value::from(manifest.lane_id.as_u32()));
    body.insert("epoch".into(), Value::from(manifest.epoch));
    body.insert("manifest".into(), manifest_json);
    body.insert(
        "manifest_norito".into(),
        Value::from(BASE64.encode(&manifest_bytes)),
    );
    body.insert(
        "manifest_len".into(),
        Value::from(manifest_bytes.len() as u64),
    );
    body.insert("chunk_plan".into(), chunk_plan);
    if let Some(plan) = sampling_plan {
        body.insert("sampling_plan".into(), sampling_plan_to_value(&plan));
    }

    let mut response = utils::respond_value_with_format(Value::Object(body), format);
    match load_pdp_commitment_from_spool(&app.da_ingest.manifest_store_dir, &ticket) {
        Ok(commitment) => match pdp_commitment_header_value(&commitment) {
            Ok(value) => {
                response
                    .headers_mut()
                    .insert(HeaderName::from_static(HEADER_SORA_PDP_COMMITMENT), value);
            }
            Err((_, message)) => {
                warn!(
                    ticket = %hex::encode(ticket.as_bytes()),
                    "failed to encode PDP commitment header: {message}"
                );
            }
        },
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            warn!(
                ?err,
                ticket = %hex::encode(ticket.as_bytes()),
                "failed to load PDP commitment for manifest fetch"
            );
        }
    }
    Ok(response)
}

fn normalize_payload(
    request: &DaIngestRequest,
) -> Result<CanonicalPayload<'_>, (StatusCode, String)> {
    match request.compression {
        Compression::Identity => Ok(CanonicalPayload {
            bytes: Cow::Borrowed(&request.payload),
        }),
        Compression::Gzip | Compression::Deflate | Compression::Zstd => {
            let expected_len = usize::try_from(request.total_size).map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    format!(
                        "total_size {} exceeds this node's supported payload length",
                        request.total_size
                    ),
                )
            })?;
            let decompressed = match request.compression {
                Compression::Identity => unreachable!("handled above"),
                Compression::Gzip => decompress_reader(
                    GzDecoder::new(request.payload.as_slice()),
                    expected_len,
                    "gzip",
                )?,
                Compression::Deflate => decompress_reader(
                    DeflateDecoder::new(request.payload.as_slice()),
                    expected_len,
                    "deflate",
                )?,
                Compression::Zstd => decompress_zstd(request.payload.as_slice(), expected_len)?,
            };
            Ok(CanonicalPayload {
                bytes: Cow::Owned(decompressed),
            })
        }
    }
}

fn parse_block_hash_hex(input: &str) -> Result<Hash, String> {
    let trimmed = input
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if trimmed.is_empty() {
        return Err("block_hash must not be empty when provided".into());
    }
    Hash::from_str(trimmed).map_err(|err| format!("invalid block_hash: {err}"))
}

fn parse_storage_ticket_hex(input: &str) -> Result<[u8; 32], String> {
    if input.is_empty() {
        return Err("storage ticket must be provided".into());
    }
    let trimmed = input.trim_start_matches("0x").trim_start_matches("0X");
    let bytes = hex::decode(trimmed)
        .map_err(|_| "storage ticket must be a 64-character hex string".to_owned())?;
    if bytes.len() != 32 {
        return Err(format!(
            "storage ticket must decode to 32 bytes (got {})",
            bytes.len()
        ));
    }
    let mut array = [0u8; 32];
    array.copy_from_slice(&bytes);
    Ok(array)
}

fn load_manifest_from_spool(
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

fn load_pdp_commitment_from_spool(
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

fn encode_pdp_commitment_bytes(
    commitment: &PdpCommitmentV1,
) -> Result<Vec<u8>, (StatusCode, String)> {
    to_bytes(commitment).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode PDP commitment: {err}"),
        )
    })
}

fn pdp_commitment_header_value(bytes: &[u8]) -> Result<HeaderValue, (StatusCode, String)> {
    let encoded = BASE64.encode(bytes);
    HeaderValue::from_str(&encoded).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode Sora-PDP-Commitment header: {err}"),
        )
    })
}

fn decompress_reader<R>(
    mut reader: R,
    expected_len: usize,
    algorithm: &'static str,
) -> Result<Vec<u8>, (StatusCode, String)>
where
    R: Read,
{
    let mut buffer = Vec::with_capacity(expected_len.min(16 * 1024));
    reader.read_to_end(&mut buffer).map_err(|err| {
        (
            StatusCode::BAD_REQUEST,
            format!("failed to decompress {algorithm} payload: {err}"),
        )
    })?;
    verify_decompressed_len(buffer, expected_len, algorithm)
}

fn decompress_zstd(payload: &[u8], expected_len: usize) -> Result<Vec<u8>, (StatusCode, String)> {
    let bytes = zstd_decode_all(payload).map_err(|err| {
        (
            StatusCode::BAD_REQUEST,
            format!("failed to decompress zstd payload: {err}"),
        )
    })?;
    verify_decompressed_len(bytes, expected_len, "zstd")
}

fn verify_decompressed_len(
    buffer: Vec<u8>,
    expected_len: usize,
    algorithm: &'static str,
) -> Result<Vec<u8>, (StatusCode, String)> {
    if buffer.len() != expected_len {
        Err((
            StatusCode::BAD_REQUEST,
            format!(
                "{algorithm} payload decompressed to {} bytes but total_size advertises {} bytes",
                buffer.len(),
                expected_len
            ),
        ))
    } else {
        Ok(buffer)
    }
}

fn validate_request(
    request: &DaIngestRequest,
    canonical_payload_len: usize,
) -> Result<(), (StatusCode, &'static str)> {
    if request.total_size != canonical_payload_len as u64 {
        return Err((
            StatusCode::BAD_REQUEST,
            "payload length does not match total_size",
        ));
    }

    if request.chunk_size == 0 || !request.chunk_size.is_power_of_two() {
        return Err((
            StatusCode::BAD_REQUEST,
            "chunk_size must be a non-zero power of two",
        ));
    }

    if request.chunk_size < 2 {
        return Err((
            StatusCode::BAD_REQUEST,
            "chunk_size must be at least 2 bytes for parity encoding",
        ));
    }

    const MAX_CHUNK_SIZE: u32 = 2 * 1024 * 1024;
    if request.chunk_size > MAX_CHUNK_SIZE {
        return Err((
            StatusCode::BAD_REQUEST,
            "chunk_size exceeds supported maximum (2 MiB)",
        ));
    }

    if request.erasure_profile.data_shards == 0 && request.erasure_profile.parity_shards == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "erasure profile must include at least one data or parity shard",
        ));
    }

    if request.erasure_profile.parity_shards < 2 {
        return Err((
            StatusCode::BAD_REQUEST,
            "erasure profile requires at least 2 parity shards",
        ));
    }

    Ok(())
}

fn lane_proof_scheme(
    lane_config: &ConfigLaneConfig,
    lane_id: LaneId,
) -> Result<DaProofScheme, (StatusCode, String)> {
    let Some(entry) = lane_config.entry(lane_id) else {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("lane {} not present in lane catalog", lane_id.as_u32()),
        ));
    };

    match entry.proof_scheme {
        DaProofScheme::MerkleSha256 | DaProofScheme::KzgBls12_381 => Ok(entry.proof_scheme),
    }
}

fn compute_fingerprint(request: &DaIngestRequest, canonical_payload: &[u8]) -> ReplayFingerprint {
    if let Some(manifest) = &request.norito_manifest {
        return ReplayFingerprint::from_hash(blake3_hash(manifest));
    }

    let mut hasher = Blake3Hasher::new();
    hasher.update(canonical_payload);
    hasher.update(request.client_blob_id.as_bytes());
    ReplayFingerprint::from_hash(hasher.finalize())
}

#[allow(clippy::too_many_arguments)]
fn build_receipt(
    signer: &KeyPair,
    request: &DaIngestRequest,
    queued_at: u64,
    blob_hash: BlobDigest,
    chunk_root: BlobDigest,
    manifest_hash: BlobDigest,
    storage_ticket: StorageTicketId,
    pdp_commitment: Vec<u8>,
    rent_quote: DaRentQuote,
    stripe_layout: DaStripeLayout,
) -> DaIngestReceipt {
    let mut receipt = DaIngestReceipt {
        client_blob_id: request.client_blob_id.clone(),
        lane_id: request.lane_id,
        epoch: request.epoch,
        blob_hash,
        chunk_root,
        manifest_hash,
        storage_ticket,
        pdp_commitment: Some(pdp_commitment),
        stripe_layout,
        queued_at_unix: queued_at,
        rent_quote,
        operator_signature: Signature::from_bytes(&RECEIPT_SIGNATURE_PLACEHOLDER),
    };
    let unsigned_bytes =
        to_bytes(&receipt).expect("DA receipt is Norito-serializable before signing");
    receipt.operator_signature = Signature::new(signer.private_key(), &unsigned_bytes);
    receipt
}

fn stripe_layout_from_manifest(manifest: &DaManifestV1) -> DaStripeLayout {
    DaStripeLayout {
        total_stripes: manifest.total_stripes,
        shards_per_stripe: manifest.shards_per_stripe,
        row_parity_stripes: manifest.erasure_profile.row_parity_stripes,
    }
}

fn chunk_profile_for_request(chunk_size: u32) -> ChunkProfile {
    let size = usize::try_from(chunk_size.max(1)).unwrap_or(usize::MAX);
    ChunkProfile {
        min_size: size,
        target_size: size,
        max_size: size,
        break_mask: 1,
    }
}

fn build_chunk_store(request: &DaIngestRequest, canonical_payload: &[u8]) -> ChunkStore {
    let mut store = ChunkStore::with_profile(chunk_profile_for_request(request.chunk_size));
    store.ingest_bytes(canonical_payload);
    store
}

fn encrypt_governance_metadata(
    metadata: &ExtraMetadata,
    key: Option<&[u8; 32]>,
    key_label: Option<&str>,
) -> Result<ExtraMetadata, (StatusCode, String)> {
    if metadata.items.is_empty() {
        return Ok(metadata.clone());
    }

    let mut encryptor: Option<SymmetricEncryptor<ChaCha20Poly1305>> = None;
    let mut processed = Vec::with_capacity(metadata.items.len());

    for entry in &metadata.items {
        let mut entry = entry.clone();
        match entry.visibility {
            MetadataVisibility::Public => {
                if entry.encryption != MetadataEncryption::None {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!(
                            "metadata entry `{}` is public but declares encryption {:?}",
                            entry.key, entry.encryption
                        ),
                    ));
                }
            }
            MetadataVisibility::GovernanceOnly => {
                let key_bytes = key.ok_or_else(|| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Torii governance metadata encryption key is not configured".into(),
                    )
                })?;
                let expected_label = key_label;

                if encryptor.is_none() {
                    let enc = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(key_bytes)
                        .map_err(|err| {
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                format!(
                                    "failed to initialise governance metadata encryptor: {err}"
                                ),
                            )
                        })?;
                    encryptor = Some(enc);
                }
                let encryptor = encryptor.as_ref().expect("initialised above");

                match entry.encryption {
                    MetadataEncryption::None => {
                        let ciphertext = encryptor
                            .encrypt_easy(entry.key.as_bytes(), &entry.value)
                            .map_err(|err| {
                                (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    format!(
                                        "failed to encrypt governance metadata entry `{}`: {err}",
                                        entry.key
                                    ),
                                )
                            })?;
                        entry.value = ciphertext;
                        entry.encryption = MetadataEncryption::chacha20poly1305_with_label(
                            expected_label.map(ToOwned::to_owned),
                        );
                    }
                    MetadataEncryption::ChaCha20Poly1305(ref envelope) => {
                        if let Some(label) = expected_label {
                            match envelope.key_label.as_deref() {
                                Some(observed) if observed == label => {}
                                Some(other) => {
                                    return Err((
                                        StatusCode::BAD_REQUEST,
                                        format!(
                                            "governance metadata entry `{}` encrypted with unexpected key `{other}` (expected `{label}`)",
                                            entry.key
                                        ),
                                    ));
                                }
                                None => {
                                    return Err((
                                        StatusCode::BAD_REQUEST,
                                        format!(
                                            "governance metadata entry `{}` missing key label (expected `{label}`)",
                                            entry.key
                                        ),
                                    ));
                                }
                            }
                        }

                        encryptor
                            .decrypt_easy(entry.key.as_bytes(), &entry.value)
                            .map_err(|_| {
                                (
                                    StatusCode::BAD_REQUEST,
                                    format!(
                                        "governance metadata entry `{}` has invalid ciphertext",
                                        entry.key
                                    ),
                                )
                            })?;
                    }
                }
            }
        }
        processed.push(entry);
    }

    Ok(ExtraMetadata { items: processed })
}

fn allocate_chunk_index(counter: &mut u32) -> Result<u32, (StatusCode, String)> {
    let idx = *counter;
    *counter = counter.checked_add(1).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "manifest would exceed supported chunk index space".into(),
        )
    })?;
    Ok(idx)
}

fn symbols_from_chunk(
    chunk_size: usize,
    payload: &[u8],
    offset: usize,
    length: usize,
) -> Result<Vec<u16>, (StatusCode, String)> {
    if length > chunk_size {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("chunk length {length} exceeds configured chunk_size {chunk_size}"),
        ));
    }
    let symbol_count = chunk_size.checked_div(2).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "chunk_size must be at least 2 bytes to support RS(16) parity".into(),
        )
    })?;
    let mut symbols = vec![0u16; symbol_count];
    let mut idx = 0usize;
    let mut cursor = offset;
    while idx < symbol_count && cursor < offset + length {
        let mut pair = [0u8; 2];
        let remaining = 2.min(offset + length - cursor);
        pair[..remaining].copy_from_slice(&payload[cursor..cursor + remaining]);
        symbols[idx] = u16::from_le_bytes(pair);
        cursor += remaining;
        idx += 1;
    }
    Ok(symbols)
}

fn parity_offset(
    total_size: u64,
    stripe_index: usize,
    parity_index: usize,
    parity_shards: usize,
    chunk_size: u32,
) -> Result<u64, (StatusCode, String)> {
    let stride = u64::from(chunk_size);
    let parity_slot = u64::try_from(stripe_index)
        .ok()
        .and_then(|stripe| {
            stripe
                .checked_mul(parity_shards as u64)?
                .checked_add(parity_index as u64)
        })
        .and_then(|slot| slot.checked_mul(stride))
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "parity chunk offset exceeded supported size".into(),
            )
        })?;

    total_size.checked_add(parity_slot).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "parity chunk offset exceeded supported size".into(),
        )
    })
}

fn build_chunk_commitments(
    request: &DaIngestRequest,
    chunk_store: &ChunkStore,
    canonical_payload: &[u8],
) -> Result<Vec<ChunkCommitment>, (StatusCode, String)> {
    build_chunk_commitments_with_parity_observer(
        request,
        chunk_store,
        canonical_payload,
        |_index, _symbols| Ok(()),
    )
}

fn build_chunk_commitments_with_parity_observer<F>(
    request: &DaIngestRequest,
    chunk_store: &ChunkStore,
    canonical_payload: &[u8],
    mut parity_observer: F,
) -> Result<Vec<ChunkCommitment>, (StatusCode, String)>
where
    F: FnMut(u32, &[u16]) -> Result<(), (StatusCode, String)>,
{
    let chunk_size = usize::try_from(request.chunk_size).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "chunk_size exceeds supported host size".into(),
        )
    })?;
    if chunk_size < 2 || chunk_size % 2 != 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "chunk_size must be an even number of bytes for RS(16) parity".into(),
        ));
    }

    let data_shards = usize::from(request.erasure_profile.data_shards);
    let parity_shards = usize::from(request.erasure_profile.parity_shards);
    if data_shards == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "erasure profile must include at least one data shard".to_string(),
        ));
    }

    let symbol_count = chunk_size / 2;
    let chunks = chunk_store.chunks();
    if chunks.is_empty() {
        return Ok(Vec::new());
    }

    let stripes = chunks.len().div_ceil(data_shards);
    let mut commitments = Vec::with_capacity(
        chunks.len()
            + stripes.saturating_mul(parity_shards)
            + stripes
                .saturating_mul(usize::from(request.erasure_profile.row_parity_stripes))
                .saturating_mul(data_shards + parity_shards),
    );
    // For column parity, retain the symbol vectors per stripe/column.
    let mut stripe_symbols_matrix: Vec<Vec<Vec<u16>>> = Vec::with_capacity(stripes);
    let mut next_index: u32 = 0;

    for stripe in 0..stripes {
        let mut stripe_symbols = Vec::with_capacity(data_shards + parity_shards);
        for shard_idx in 0..data_shards {
            let chunk_idx = stripe * data_shards + shard_idx;
            if let Some(chunk) = chunks.get(chunk_idx) {
                let offset = usize::try_from(chunk.offset).map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("chunk {chunk_idx} offset exceeds host limits"),
                    )
                })?;
                let length = usize::try_from(chunk.length).map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("chunk {chunk_idx} length exceeds host limits"),
                    )
                })?;
                let end = offset.checked_add(length).ok_or_else(|| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("chunk {chunk_idx} offset+length overflow"),
                    )
                })?;
                if end > canonical_payload.len() {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!("chunk {chunk_idx} extends past canonical payload"),
                    ));
                }

                let symbols = symbols_from_chunk(chunk_size, canonical_payload, offset, length)?;
                stripe_symbols.push(symbols.clone());

                let index = allocate_chunk_index(&mut next_index)?;
                let stripe_id = u32::try_from(stripe).unwrap_or(u32::MAX);
                commitments.push(ChunkCommitment::new_with_role(
                    index,
                    chunk.offset,
                    chunk.length,
                    ChunkDigest::new(chunk.blake3),
                    ChunkRole::Data,
                    stripe_id,
                ));
            } else {
                stripe_symbols.push(vec![0u16; symbol_count]);
            }
        }

        if parity_shards == 0 {
            stripe_symbols_matrix.push(stripe_symbols);
            continue;
        }

        let parity_symbols = rs16::encode_parity(&stripe_symbols, parity_shards).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to compute parity shards".into(),
            )
        })?;

        for (parity_idx, symbols) in parity_symbols.into_iter().enumerate() {
            let mut hasher = Blake3Hasher::new();
            for symbol in &symbols {
                hasher.update(&symbol.to_le_bytes());
            }
            let digest = hasher.finalize();
            let offset = parity_offset(
                request.total_size,
                stripe,
                parity_idx,
                parity_shards,
                request.chunk_size,
            )?;

            let index = allocate_chunk_index(&mut next_index)?;
            parity_observer(index, &symbols)?;
            let stripe_id = u32::try_from(stripe).unwrap_or(u32::MAX);
            commitments.push(ChunkCommitment::new_with_role(
                index,
                offset,
                request.chunk_size,
                ChunkDigest::new(*digest.as_bytes()),
                ChunkRole::GlobalParity,
                stripe_id,
            ));
            stripe_symbols.push(symbols);
        }

        stripe_symbols_matrix.push(stripe_symbols);
    }

    let row_parity = usize::from(request.erasure_profile.row_parity_stripes);
    if row_parity > 0 {
        let column_count = data_shards + parity_shards;
        let base_offset = request.total_size.saturating_add(
            stripes
                .saturating_mul(parity_shards)
                .saturating_mul(chunk_size) as u64,
        );
        for column in 0..column_count {
            // Collect the column symbols across stripes.
            let mut column_symbols = Vec::with_capacity(stripes);
            for stripe in 0..stripes {
                let stripe_row = stripe_symbols_matrix
                    .get(stripe)
                    .and_then(|row| row.get(column))
                    .cloned()
                    .unwrap_or_else(|| vec![0u16; symbol_count]);
                column_symbols.push(stripe_row);
            }

            let parity_cols = rs16::encode_parity(&column_symbols, row_parity).map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to compute row-parity stripes".into(),
                )
            })?;

            for (row_parity_idx, symbols) in parity_cols.into_iter().enumerate() {
                let mut hasher = Blake3Hasher::new();
                for symbol in &symbols {
                    hasher.update(&symbol.to_le_bytes());
                }
                let digest = hasher.finalize();

                let offset = base_offset.saturating_add(
                    ((row_parity_idx * column_count + column) as u64)
                        .saturating_mul(request.chunk_size as u64),
                );
                let index = allocate_chunk_index(&mut next_index)?;
                parity_observer(index, &symbols)?;
                let column_id = u32::try_from(column).unwrap_or(u32::MAX);
                commitments.push(ChunkCommitment::new_with_role(
                    index,
                    offset,
                    request.chunk_size,
                    ChunkDigest::new(*digest.as_bytes()),
                    ChunkRole::StripeParity,
                    column_id,
                ));
            }
        }
    }

    Ok(commitments)
}

fn role_tag(role: ChunkRole) -> u8 {
    match role {
        ChunkRole::Data => 0,
        ChunkRole::LocalParity => 1,
        ChunkRole::GlobalParity => 2,
        ChunkRole::StripeParity => 3,
    }
}

fn effective_chunk_role(commitment: &ChunkCommitment) -> ChunkRole {
    if commitment.parity && matches!(commitment.role, ChunkRole::Data) {
        ChunkRole::GlobalParity
    } else {
        commitment.role
    }
}

fn ipa_scalar_from_chunk(commitment: &ChunkCommitment) -> IpaScalar {
    let mut hasher = Blake3Hasher::new();
    hasher.update(&commitment.index.to_le_bytes());
    hasher.update(&commitment.offset.to_le_bytes());
    hasher.update(&commitment.length.to_le_bytes());
    hasher.update(commitment.commitment.as_bytes());
    hasher.update(&[commitment.parity as u8, role_tag(commitment.role)]);
    hasher.update(&commitment.group_id.to_le_bytes());
    let mut wide = [0u8; 64];
    hasher.finalize_xof().fill(&mut wide);
    IpaScalar::from_uniform(&wide)
}

pub fn ipa_commitment_from_chunks(
    commitments: &[ChunkCommitment],
) -> Result<BlobDigest, (StatusCode, String)> {
    if commitments.is_empty() {
        return Ok(BlobDigest::default());
    }
    let params_len = commitments.len().next_power_of_two().max(1);
    let params = IpaCurveParams::new(params_len).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to derive IPA parameters: {err}"),
        )
    })?;
    let mut scalars: Vec<IpaScalar> = commitments.iter().map(ipa_scalar_from_chunk).collect();
    while scalars.len() < params_len {
        scalars.push(IpaScalar::zero());
    }
    let poly = IpaPolynomial::from_coeffs(scalars);
    let commitment = poly.commit(&params).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to commit IPA vector: {err}"),
        )
    })?;
    Ok(BlobDigest::new(commitment.to_bytes()))
}

fn compute_tree_height(count: usize) -> u16 {
    if count <= 1 {
        return 1;
    }
    let bits = usize::BITS - (count - 1).leading_zeros();
    bits as u16
}

fn compute_pdp_commitment(
    manifest_digest: &BlobDigest,
    manifest: &DaManifestV1,
    chunk_store: &ChunkStore,
    sealed_at_unix: u64,
) -> Result<PdpCommitmentV1, (StatusCode, String)> {
    let hot_root = chunk_store.pdp_hot_root().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "chunking did not produce PDP hot-leaf commitments".to_owned(),
        )
    })?;
    let segment_root = chunk_store.pdp_segment_root().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "chunking did not produce PDP segment commitments".to_owned(),
        )
    })?;

    let hot_leaf_count = chunk_store.pdp_hot_leaf_count();
    if hot_leaf_count == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "payload produced zero PDP hot leaves".to_owned(),
        ));
    }
    let segment_count = chunk_store.pdp_segment_count();
    if segment_count == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "payload produced zero PDP segments".to_owned(),
        ));
    }

    let commitment = PdpCommitmentV1 {
        version: PDP_COMMITMENT_VERSION_V1,
        manifest_digest: *manifest_digest.as_ref(),
        chunk_profile: ChunkingProfileV1::from_profile(
            chunk_store.profile(),
            BLAKE3_256_MULTIHASH_CODE,
        ),
        commitment_root_hot: hot_root,
        commitment_root_segment: segment_root,
        hash_algorithm: HashAlgorithmV1::Blake3_256,
        hot_tree_height: compute_tree_height(hot_leaf_count),
        segment_tree_height: compute_tree_height(segment_count),
        sample_window: compute_sample_window(manifest.total_size),
        sealed_at: sealed_at_unix,
    };

    commitment
        .validate()
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(commitment)
}

#[derive(Debug)]
struct ManifestArtifacts {
    manifest: DaManifestV1,
    encoded: Vec<u8>,
    manifest_hash: BlobDigest,
    blob_hash: BlobDigest,
    chunk_root: BlobDigest,
    storage_ticket: StorageTicketId,
}

#[allow(clippy::too_many_arguments)]
fn resolve_manifest(
    request: &DaIngestRequest,
    chunk_store: &ChunkStore,
    canonical_payload: &[u8],
    metadata: &ExtraMetadata,
    enforced_retention: &RetentionPolicy,
    queued_at_unix: u64,
    fingerprint: &ReplayFingerprint,
    rent_policy: &DaRentPolicyV1,
) -> Result<ManifestArtifacts, (StatusCode, String)> {
    let blob_hash = BlobDigest::from_hash(*chunk_store.payload_digest());
    let chunk_root = BlobDigest::new(*chunk_store.por_tree().root());
    let storage_ticket = StorageTicketId::new(*fingerprint.as_bytes());
    let total_stripes = (chunk_store.chunks().len() as u32)
        .div_ceil(u32::from(request.erasure_profile.data_shards));
    let shards_per_stripe = u32::from(request.erasure_profile.data_shards)
        .saturating_add(u32::from(request.erasure_profile.parity_shards));
    let total_stripes_full =
        total_stripes.saturating_add(u32::from(request.erasure_profile.row_parity_stripes));

    let chunk_commitments = build_chunk_commitments(request, chunk_store, canonical_payload)?;

    let (rent_gib, rent_months) = rent_usage_from_request(request.total_size, enforced_retention);
    let rent_quote = rent_policy.quote(rent_gib, rent_months).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to compute DA rent quote: {err}"),
        )
    })?;

    let manifest = if let Some(bytes) = &request.norito_manifest {
        let archived = from_bytes::<DaManifestV1>(bytes).map_err(|err| {
            warn!(?err, "failed to decode DA manifest");
            (
                StatusCode::BAD_REQUEST,
                format!("failed to decode DA manifest: {err}"),
            )
        })?;
        let manifest = NoritoDeserialize::try_deserialize(archived).map_err(|err| {
            warn!(?err, "failed to deserialize DA manifest");
            (
                StatusCode::BAD_REQUEST,
                format!("failed to deserialize DA manifest: {err}"),
            )
        })?;
        let expected_ipa = ipa_commitment_from_chunks(&chunk_commitments)?;
        let ipa_commitment = if manifest.ipa_commitment.is_zero() {
            expected_ipa
        } else if manifest.ipa_commitment == expected_ipa {
            manifest.ipa_commitment
        } else {
            return Err((
                StatusCode::BAD_REQUEST,
                "manifest ipa_commitment does not match computed value".into(),
            ));
        };

        verify_manifest_against_request(
            request,
            &manifest,
            enforced_retention,
            metadata,
            &chunk_commitments,
            blob_hash,
            chunk_root,
            &rent_quote,
        )?;

        DaManifestV1 {
            version: manifest.version,
            storage_ticket,
            total_stripes: total_stripes_full,
            shards_per_stripe,
            metadata: metadata.clone(),
            rent_quote,
            ipa_commitment,
            ..manifest
        }
    } else {
        let ipa_commitment = ipa_commitment_from_chunks(&chunk_commitments)?;
        DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: request.client_blob_id.clone(),
            lane_id: request.lane_id,
            epoch: request.epoch,
            blob_class: request.blob_class,
            codec: request.codec.clone(),
            blob_hash,
            chunk_root,
            storage_ticket,
            total_size: request.total_size,
            chunk_size: request.chunk_size,
            total_stripes: total_stripes_full,
            shards_per_stripe,
            erasure_profile: request.erasure_profile,
            retention_policy: enforced_retention.clone(),
            rent_quote,
            chunks: chunk_commitments.clone(),
            ipa_commitment,
            metadata: metadata.clone(),
            issued_at_unix: queued_at_unix,
        }
    };

    let encoded =
        to_bytes(&manifest).map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let manifest_hash = BlobDigest::from_hash(blake3_hash(&encoded));

    Ok(ManifestArtifacts {
        manifest,
        encoded,
        manifest_hash,
        blob_hash,
        chunk_root,
        storage_ticket,
    })
}

fn persist_manifest_for_sorafs(
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

fn persist_pdp_commitment(
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

fn derive_kzg_commitment(
    chunk_root: &BlobDigest,
    storage_ticket: &StorageTicketId,
) -> KzgCommitment {
    let mut hasher = Blake3Hasher::new();
    hasher.update(chunk_root.as_ref());
    hasher.update(storage_ticket.as_ref());

    let mut bytes = [0u8; 48];
    hasher.finalize_xof().fill(&mut bytes);
    KzgCommitment::new(bytes)
}

fn build_da_commitment_record(
    request: &DaIngestRequest,
    manifest: &ManifestArtifacts,
    retention: &RetentionPolicy,
    operator_signature: &Signature,
    pdp_commitment_bytes: &[u8],
    proof_scheme: DaProofScheme,
) -> DaCommitmentRecord {
    let manifest_digest = ManifestDigest::new(*manifest.manifest_hash.as_bytes());
    let chunk_root = Hash::prehashed(*manifest.chunk_root.as_bytes());
    let proof_digest = Hash::new(pdp_commitment_bytes);
    let kzg_commitment = match proof_scheme {
        DaProofScheme::MerkleSha256 => None,
        DaProofScheme::KzgBls12_381 => Some(derive_kzg_commitment(
            &manifest.chunk_root,
            &manifest.storage_ticket,
        )),
    };
    DaCommitmentRecord::new(
        request.lane_id,
        request.epoch,
        request.sequence,
        request.client_blob_id.clone(),
        manifest_digest,
        proof_scheme,
        chunk_root,
        kzg_commitment,
        Some(proof_digest),
        retention.clone(),
        manifest.storage_ticket,
        operator_signature.clone(),
    )
}

fn persist_da_commitment_record(
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
struct DaCommitmentScheduleEntry {
    version: u16,
    record: DaCommitmentRecord,
    pdp_commitment: Vec<u8>,
}

#[allow(clippy::too_many_arguments)]
fn persist_da_commitment_schedule_entry(
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

fn persist_da_pin_intent(
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

fn persist_da_receipt(
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
    .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;

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

fn load_da_receipts(spool_dir: &Path) -> std::io::Result<Vec<StoredDaReceipt>> {
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

#[allow(clippy::redundant_pub_crate)]
mod taikai_ingest {
    use std::{
        io::{self, Write},
        str::FromStr,
    };

    use sorafs_car::{CarBuildPlan, CarWriter};

    use super::*;

    pub(super) const STREAM_LABEL_FALLBACK: &str = "<unknown>";

    pub(super) struct EnvelopeArtifacts {
        pub envelope_bytes: Vec<u8>,
        pub indexes_json: Vec<u8>,
        pub telemetry: TaikaiTelemetrySample,
        pub car_digest: BlobDigest,
    }

    #[derive(Clone)]
    pub(super) struct TaikaiTelemetrySample {
        pub event_id: String,
        pub stream_id: String,
        pub rendition_id: String,
        pub segment_sequence: u64,
        pub wallclock_unix_ms: u64,
        pub ingest_latency_ms: Option<u32>,
        pub live_edge_drift_ms: Option<i32>,
    }

    #[derive(Clone, Debug)]
    struct TrmLineageRecord {
        alias_namespace: String,
        alias_name: String,
        manifest_digest_hex: String,
        window_start_sequence: u64,
        window_end_sequence: u64,
        updated_unix: u64,
    }

    pub(super) struct TrmLineageGuard {
        manifest_store_dir: PathBuf,
        base_dir: PathBuf,
        alias_namespace: String,
        alias_name: String,
        alias_slug: String,
        lock: TrmAliasLock,
        record_path: PathBuf,
        previous: Option<TrmLineageRecord>,
    }

    impl TrmLineageGuard {
        pub fn new(
            spool_dir: &Path,
            alias: &TaikaiAliasBinding,
        ) -> Result<Option<Self>, (StatusCode, String)> {
            if spool_dir.as_os_str().is_empty() {
                return Ok(None);
            }
            let base_dir = spool_dir.join(TAIKAI_SPOOL_SUBDIR);
            fs::create_dir_all(&base_dir).map_err(|err| {
                internal_error(format!(
                    "failed to prepare Taikai spool directory `{}`: {err}",
                    base_dir.display()
                ))
            })?;
            let alias_slug = alias_slug(&alias.namespace, &alias.name);
            let lock = TrmAliasLock::acquire(&base_dir, &alias_slug)?;
            let record_path = lineage_record_path(&base_dir, &alias_slug);
            let previous = read_lineage_record(&record_path).map_err(|err| {
                internal_error(format!(
                    "failed to read Taikai routing manifest lineage `{}`: {err}",
                    record_path.display()
                ))
            })?;
            Ok(Some(Self {
                manifest_store_dir: spool_dir.to_path_buf(),
                base_dir,
                alias_namespace: alias.namespace.clone(),
                alias_name: alias.name.clone(),
                alias_slug,
                lock,
                record_path,
                previous,
            }))
        }

        pub fn validate(
            &self,
            manifest: &TaikaiRoutingManifestV1,
            manifest_digest_hex: &str,
        ) -> Result<(), (StatusCode, String)> {
            if let Some(previous) = &self.previous {
                if previous.manifest_digest_hex == manifest_digest_hex {
                    return Err(bad_request(
                        META_TAIKAI_TRM,
                        format!(
                            "routing manifest digest `{manifest_digest_hex}` already accepted for alias {}.{}",
                            self.alias_name, self.alias_namespace
                        ),
                    ));
                }
                if manifest.segment_window.start_sequence <= previous.window_end_sequence {
                    return Err(bad_request(
                        META_TAIKAI_TRM,
                        format!(
                            "routing manifest window {}–{} overlaps previously accepted window {}–{} for alias {}.{}",
                            manifest.segment_window.start_sequence,
                            manifest.segment_window.end_sequence,
                            previous.window_start_sequence,
                            previous.window_end_sequence,
                            self.alias_name,
                            self.alias_namespace
                        ),
                    ));
                }
            }
            Ok(())
        }

        pub fn persist_lineage_hint(
            &self,
            lane_id: LaneId,
            epoch: u64,
            sequence: u64,
            storage_ticket: &StorageTicketId,
            fingerprint: &ReplayFingerprint,
        ) -> Result<(), (StatusCode, String)> {
            let bytes = build_lineage_hint_bytes(
                &self.alias_namespace,
                &self.alias_name,
                self.previous.as_ref(),
            )
            .map_err(|err| {
                internal_error(format!(
                    "failed to build Taikai routing manifest lineage hint: {err}"
                ))
            })?;
            persist_artifact(
                &self.manifest_store_dir,
                lane_id,
                epoch,
                sequence,
                storage_ticket,
                fingerprint,
                TAIKAI_LINEAGE_HINT_PREFIX,
                "json",
                &bytes,
            )
            .map_err(|err| {
                internal_error(format!(
                    "failed to persist Taikai routing manifest lineage hint in `{}`: {err}",
                    self.manifest_store_dir.display()
                ))
            })?;
            Ok(())
        }

        pub fn commit(
            &mut self,
            window: TaikaiSegmentWindow,
            manifest_digest_hex: &str,
        ) -> Result<(), (StatusCode, String)> {
            let record = TrmLineageRecord {
                alias_namespace: self.alias_namespace.clone(),
                alias_name: self.alias_name.clone(),
                manifest_digest_hex: manifest_digest_hex.to_owned(),
                window_start_sequence: window.start_sequence,
                window_end_sequence: window.end_sequence,
                updated_unix: current_unix_seconds(),
            };
            write_lineage_record(&self.record_path, &record).map_err(|err| {
                internal_error(format!(
                    "failed to persist Taikai routing manifest lineage `{}`: {err}",
                    self.record_path.display()
                ))
            })?;
            self.previous = Some(record);
            Ok(())
        }
    }

    struct TrmAliasLock {
        path: PathBuf,
    }

    impl TrmAliasLock {
        fn acquire(base_dir: &Path, slug: &str) -> Result<Self, (StatusCode, String)> {
            let path = base_dir.join(format!(
                "{TAIKAI_TRM_LOCK_PREFIX}{slug}{TAIKAI_TRM_LOCK_SUFFIX}"
            ));
            for attempt in 0..=1 {
                match OpenOptions::new().write(true).create_new(true).open(&path) {
                    Ok(mut file) => {
                        let now = current_unix_seconds();
                        let _ = writeln!(file, "{now}");
                        return Ok(Self { path });
                    }
                    Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                        if attempt == 0 && lock_is_stale(&path).unwrap_or(false) {
                            let _ = fs::remove_file(&path);
                            continue;
                        }
                        return Err((
                            StatusCode::SERVICE_UNAVAILABLE,
                            format!(
                                "routing manifest lock busy for alias slug `{slug}`; retry later"
                            ),
                        ));
                    }
                    Err(err) => {
                        return Err(internal_error(format!(
                            "failed to create Taikai routing manifest lock `{}`: {err}",
                            path.display()
                        )));
                    }
                }
            }
            unreachable!("lock acquisition attempts exhausted without returning");
        }
    }

    impl Drop for TrmAliasLock {
        fn drop(&mut self) {
            if let Err(err) = fs::remove_file(&self.path) {
                if err.kind() != ErrorKind::NotFound {
                    iroha_logger::warn!(
                        ?err,
                        path = %self.path.display(),
                        "failed to remove Taikai routing manifest lock"
                    );
                }
            }
        }
    }

    fn alias_slug(namespace: &str, name: &str) -> String {
        let mut hasher = Blake3Hasher::new();
        hasher.update(namespace.as_bytes());
        hasher.update(&[0xFF]);
        hasher.update(name.as_bytes());
        let digest = hasher.finalize();
        let digest_hex = hex::encode(&digest.as_bytes()[..6]);
        format!(
            "{}-{}-{digest_hex}",
            sanitize_alias_component(namespace),
            sanitize_alias_component(name)
        )
    }

    fn sanitize_alias_component(component: &str) -> String {
        component
            .chars()
            .map(|ch| match ch {
                'a'..='z' => ch,
                'A'..='Z' => ch.to_ascii_lowercase(),
                '0'..='9' => ch,
                _ => '-',
            })
            .collect()
    }

    fn lineage_record_path(base_dir: &Path, slug: &str) -> PathBuf {
        base_dir.join(format!(
            "{TAIKAI_TRM_LINEAGE_PREFIX}{slug}{TAIKAI_TRM_LINEAGE_SUFFIX}"
        ))
    }

    fn lock_is_stale(path: &Path) -> io::Result<bool> {
        match fs::metadata(path) {
            Ok(metadata) => {
                let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
                let elapsed = SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or_default();
                Ok(elapsed.as_secs() >= TAIKAI_TRM_LOCK_STALE_SECS)
            }
            Err(err) => {
                if err.kind() == ErrorKind::NotFound {
                    Ok(false)
                } else {
                    Err(err)
                }
            }
        }
    }

    fn current_unix_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn read_lineage_record(path: &Path) -> io::Result<Option<TrmLineageRecord>> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        let value: Value = json::from_slice(&bytes)
            .map_err(|err| io::Error::new(ErrorKind::Other, err.to_string()))?;
        let map = value.as_object().ok_or_else(|| {
            io::Error::new(
                ErrorKind::Other,
                "Taikai routing manifest lineage record must be a JSON object",
            )
        })?;
        let alias_namespace = map
            .get("alias_namespace")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                io::Error::new(
                    ErrorKind::Other,
                    "Taikai routing manifest lineage record missing alias_namespace",
                )
            })?
            .to_owned();
        let alias_name = map
            .get("alias_name")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                io::Error::new(
                    ErrorKind::Other,
                    "Taikai routing manifest lineage record missing alias_name",
                )
            })?
            .to_owned();
        let manifest_digest_hex = map
            .get("manifest_digest_hex")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                io::Error::new(
                    ErrorKind::Other,
                    "Taikai routing manifest lineage record missing manifest_digest_hex",
                )
            })?
            .to_owned();
        let window_start_sequence = map
            .get("window_start_sequence")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                io::Error::new(
                    ErrorKind::Other,
                    "Taikai routing manifest lineage record missing window_start_sequence",
                )
            })?;
        let window_end_sequence = map
            .get("window_end_sequence")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                io::Error::new(
                    ErrorKind::Other,
                    "Taikai routing manifest lineage record missing window_end_sequence",
                )
            })?;
        let updated_unix = map
            .get("updated_unix")
            .and_then(Value::as_u64)
            .unwrap_or_else(current_unix_seconds);
        Ok(Some(TrmLineageRecord {
            alias_namespace,
            alias_name,
            manifest_digest_hex,
            window_start_sequence,
            window_end_sequence,
            updated_unix,
        }))
    }

    fn write_lineage_record(path: &Path, record: &TrmLineageRecord) -> io::Result<()> {
        let mut map = Map::new();
        map.insert("version".into(), Value::from(1));
        map.insert(
            "alias_namespace".into(),
            Value::from(record.alias_namespace.clone()),
        );
        map.insert("alias_name".into(), Value::from(record.alias_name.clone()));
        map.insert(
            "manifest_digest_hex".into(),
            Value::from(record.manifest_digest_hex.clone()),
        );
        map.insert(
            "window_start_sequence".into(),
            Value::from(record.window_start_sequence),
        );
        map.insert(
            "window_end_sequence".into(),
            Value::from(record.window_end_sequence),
        );
        map.insert("updated_unix".into(), Value::from(record.updated_unix));
        let rendered = json::to_json_pretty(&Value::Object(map))
            .map_err(|err| io::Error::new(ErrorKind::Other, err.to_string()))?;
        let tmp_path = path.with_extension(format!("tmp-{}", std::process::id()));
        match fs::write(&tmp_path, rendered.as_bytes()) {
            Ok(()) => {}
            Err(err) => {
                let _ = fs::remove_file(&tmp_path);
                return Err(err);
            }
        }
        if let Err(err) = fs::rename(&tmp_path, path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
        Ok(())
    }

    fn build_lineage_hint_bytes(
        alias_namespace: &str,
        alias_name: &str,
        previous: Option<&TrmLineageRecord>,
    ) -> Result<Vec<u8>, io::Error> {
        let mut map = Map::new();
        map.insert("version".into(), Value::from(1));
        map.insert(
            "alias_namespace".into(),
            Value::from(alias_namespace.to_owned()),
        );
        map.insert("alias_name".into(), Value::from(alias_name.to_owned()));
        if let Some(previous) = previous {
            map.insert(
                "previous_manifest_digest_hex".into(),
                Value::from(previous.manifest_digest_hex.clone()),
            );
            map.insert(
                "previous_window_start_sequence".into(),
                Value::from(previous.window_start_sequence),
            );
            map.insert(
                "previous_window_end_sequence".into(),
                Value::from(previous.window_end_sequence),
            );
            map.insert(
                "previous_updated_unix".into(),
                Value::from(previous.updated_unix),
            );
        } else {
            map.insert("previous_manifest_digest_hex".into(), Value::Null);
            map.insert("previous_window_start_sequence".into(), Value::Null);
            map.insert("previous_window_end_sequence".into(), Value::Null);
            map.insert("previous_updated_unix".into(), Value::Null);
        }
        let rendered = json::to_json_pretty(&Value::Object(map))
            .map_err(|err| io::Error::new(ErrorKind::Other, err.to_string()))?;
        Ok(rendered.into_bytes())
    }

    pub(super) fn build_envelope(
        _request: &DaIngestRequest,
        manifest: &ManifestArtifacts,
        chunk_store: &ChunkStore,
        canonical_payload: &[u8],
    ) -> Result<EnvelopeArtifacts, (StatusCode, String)> {
        let metadata = &manifest.manifest.metadata;

        let event_id = TaikaiEventId::new(parse_name(metadata, META_TAIKAI_EVENT_ID)?);
        let stream_id = TaikaiStreamId::new(parse_name(metadata, META_TAIKAI_STREAM_ID)?);
        let rendition_id = TaikaiRenditionId::new(parse_name(metadata, META_TAIKAI_RENDITION_ID)?);

        let track_kind = TaikaiTrackKind::from_str(require_utf8(metadata, META_TAIKAI_TRACK_KIND)?)
            .map_err(|err| parse_error(META_TAIKAI_TRACK_KIND, err))?;
        let codec = TaikaiCodec::from_str(require_utf8(metadata, META_TAIKAI_TRACK_CODEC)?)
            .map_err(|err| parse_error(META_TAIKAI_TRACK_CODEC, err))?;
        let bitrate = parse_u32(
            require_utf8(metadata, META_TAIKAI_TRACK_BITRATE)?,
            META_TAIKAI_TRACK_BITRATE,
        )?;

        let track = match track_kind {
            TaikaiTrackKind::Video => {
                let resolution_str = require_utf8(metadata, META_TAIKAI_TRACK_RESOLUTION)?;
                let resolution = TaikaiResolution::from_str(resolution_str)
                    .map_err(|err| parse_error(META_TAIKAI_TRACK_RESOLUTION, err))?;
                if !matches!(
                    codec,
                    TaikaiCodec::AvcHigh
                        | TaikaiCodec::HevcMain10
                        | TaikaiCodec::Av1Main
                        | TaikaiCodec::Custom(_)
                ) {
                    return Err(bad_request(
                        META_TAIKAI_TRACK_CODEC,
                        "codec is not valid for a video track; expected AV1/AVC/HEVC or custom",
                    ));
                }
                TaikaiTrackMetadata::video(codec, bitrate, resolution)
            }
            TaikaiTrackKind::Audio => {
                let layout_str = require_utf8(metadata, META_TAIKAI_TRACK_AUDIO_LAYOUT)?;
                let layout = TaikaiAudioLayout::from_str(layout_str)
                    .map_err(|err| parse_error(META_TAIKAI_TRACK_AUDIO_LAYOUT, err))?;
                if !matches!(
                    codec,
                    TaikaiCodec::AacLc | TaikaiCodec::Opus | TaikaiCodec::Custom(_)
                ) {
                    return Err(bad_request(
                        META_TAIKAI_TRACK_CODEC,
                        "codec is not valid for an audio track; expected AAC/Opus or custom",
                    ));
                }
                TaikaiTrackMetadata::audio(codec, bitrate, layout)
            }
            TaikaiTrackKind::Data => TaikaiTrackMetadata::data(codec, bitrate),
        };

        if matches!(track.kind, TaikaiTrackKind::Video) && track.resolution.is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "metadata entry `{META_TAIKAI_TRACK_RESOLUTION}` is required for video tracks"
                ),
            ));
        }

        if matches!(track.kind, TaikaiTrackKind::Audio) && track.audio_layout.is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "metadata entry `{META_TAIKAI_TRACK_AUDIO_LAYOUT}` is required for audio tracks"
                ),
            ));
        }

        let segment_sequence = parse_u64(
            require_utf8(metadata, META_TAIKAI_SEGMENT_SEQUENCE)?,
            META_TAIKAI_SEGMENT_SEQUENCE,
        )?;
        let segment_start_pts = parse_u64(
            require_utf8(metadata, META_TAIKAI_SEGMENT_START)?,
            META_TAIKAI_SEGMENT_START,
        )?;
        let segment_duration = parse_u32(
            require_utf8(metadata, META_TAIKAI_SEGMENT_DURATION)?,
            META_TAIKAI_SEGMENT_DURATION,
        )?;
        let wallclock_unix_ms = parse_u64(
            require_utf8(metadata, META_TAIKAI_WALLCLOCK_MS)?,
            META_TAIKAI_WALLCLOCK_MS,
        )?;

        let chunk_count: u32 = chunk_store
            .chunks()
            .len()
            .try_into()
            .map_err(|_| internal_error("chunk count exceeds supported range".into()))?;

        let plan = CarBuildPlan::single_file(canonical_payload)
            .map_err(|err| internal_error(format!("failed to derive CAR plan: {err}")))?;
        let mut sink = io::sink();
        let stats = CarWriter::new(&plan, canonical_payload)
            .map_err(|err| internal_error(format!("failed to initialise CAR writer: {err}")))?
            .write_to(&mut sink)
            .map_err(|err| internal_error(format!("failed to compute CAR digests: {err}")))?;

        let car_digest = BlobDigest::from_hash(stats.car_archive_digest);
        let car_pointer = TaikaiCarPointer::new(
            format!("b{}", encode_base32_lower(&stats.car_cid)),
            car_digest,
            stats.car_size,
        );

        let ingest_pointer = TaikaiIngestPointer::new(
            manifest.manifest_hash,
            manifest.storage_ticket,
            manifest.chunk_root,
            chunk_count,
            car_pointer,
        );

        let event_label = event_id.as_name().as_ref().to_owned();
        let stream_label = stream_id.as_name().as_ref().to_owned();
        let rendition_label = rendition_id.as_name().as_ref().to_owned();

        let mut envelope = TaikaiSegmentEnvelopeV1::new(
            event_id,
            stream_id,
            rendition_id,
            track,
            segment_sequence,
            SegmentTimestamp::new(segment_start_pts),
            SegmentDuration::new(segment_duration),
            wallclock_unix_ms,
            ingest_pointer,
        );

        if let Some(latency_str) = optional_utf8(metadata, META_TAIKAI_INGEST_LATENCY_MS)? {
            let latency = parse_u32(latency_str, META_TAIKAI_INGEST_LATENCY_MS)?;
            envelope.instrumentation.encoder_to_ingest_latency_ms = Some(latency);
        }
        let live_edge_drift_ms =
            if let Some(drift_str) = optional_utf8(metadata, META_TAIKAI_LIVE_EDGE_DRIFT_MS)? {
                Some(parse_i32(drift_str, META_TAIKAI_LIVE_EDGE_DRIFT_MS)?)
            } else {
                None
            };
        if let Some(drift) = live_edge_drift_ms {
            envelope.instrumentation.live_edge_drift_ms = Some(drift);
        }
        if let Some(node_id) = optional_utf8(metadata, META_TAIKAI_INGEST_NODE_ID)? {
            if !node_id.trim().is_empty() {
                envelope.instrumentation.ingest_node_id = Some(node_id.trim().to_owned());
            }
        }

        let indexes = envelope.indexes();
        let envelope_bytes = envelope.encode();
        let indexes_json = norito::json::to_json_pretty(&indexes)
            .map_err(|err| internal_error(format!("failed to render Taikai indexes: {err}")))?
            .into_bytes();

        let telemetry = TaikaiTelemetrySample {
            event_id: event_label,
            stream_id: stream_label,
            rendition_id: rendition_label,
            segment_sequence,
            wallclock_unix_ms,
            ingest_latency_ms: envelope.instrumentation.encoder_to_ingest_latency_ms,
            live_edge_drift_ms,
        };

        Ok(EnvelopeArtifacts {
            envelope_bytes,
            indexes_json,
            telemetry,
            car_digest,
        })
    }

    pub(super) fn persist_envelope(
        spool_dir: &Path,
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        storage_ticket: &StorageTicketId,
        fingerprint: &ReplayFingerprint,
        bytes: &[u8],
    ) -> io::Result<Option<PathBuf>> {
        persist_artifact(
            spool_dir,
            lane_id,
            epoch,
            sequence,
            storage_ticket,
            fingerprint,
            "taikai-envelope",
            "norito",
            bytes,
        )
    }

    pub(super) fn persist_indexes(
        spool_dir: &Path,
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        storage_ticket: &StorageTicketId,
        fingerprint: &ReplayFingerprint,
        bytes: &[u8],
    ) -> io::Result<Option<PathBuf>> {
        persist_artifact(
            spool_dir,
            lane_id,
            epoch,
            sequence,
            storage_ticket,
            fingerprint,
            "taikai-indexes",
            "json",
            bytes,
        )
    }

    pub(super) fn persist_ssm(
        spool_dir: &Path,
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        storage_ticket: &StorageTicketId,
        fingerprint: &ReplayFingerprint,
        bytes: &[u8],
    ) -> io::Result<Option<PathBuf>> {
        persist_artifact(
            spool_dir,
            lane_id,
            epoch,
            sequence,
            storage_ticket,
            fingerprint,
            "taikai-ssm",
            "norito",
            bytes,
        )
    }

    pub(super) fn persist_trm(
        spool_dir: &Path,
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        storage_ticket: &StorageTicketId,
        fingerprint: &ReplayFingerprint,
        bytes: &[u8],
    ) -> io::Result<Option<PathBuf>> {
        persist_artifact(
            spool_dir,
            lane_id,
            epoch,
            sequence,
            storage_ticket,
            fingerprint,
            "taikai-trm",
            "norito",
            bytes,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn persist_artifact(
        spool_dir: &Path,
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        storage_ticket: &StorageTicketId,
        fingerprint: &ReplayFingerprint,
        prefix: &str,
        extension: &str,
        bytes: &[u8],
    ) -> io::Result<Option<PathBuf>> {
        if spool_dir.as_os_str().is_empty() {
            return Ok(None);
        }

        let base_dir = spool_dir.join(TAIKAI_SPOOL_SUBDIR);
        fs::create_dir_all(&base_dir)?;

        let lane = lane_id.as_u32();
        let ticket_hex = hex::encode(storage_ticket.as_ref());
        let fingerprint_hex = hex::encode(fingerprint.as_bytes());
        let file_name = format!(
            "{prefix}-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.{extension}"
        );
        let target_path = base_dir.join(&file_name);
        if target_path.exists() {
            return Ok(Some(target_path));
        }

        let tmp_name = format!(
            ".{prefix}-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.tmp-{}",
            std::process::id()
        );
        let tmp_path = base_dir.join(tmp_name);

        match fs::write(&tmp_path, bytes) {
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
            kind = prefix,
            "queued Taikai artefact for anchoring"
        );

        Ok(Some(target_path))
    }

    fn parse_u64(value: &str, key: &str) -> Result<u64, (StatusCode, String)> {
        value
            .trim()
            .parse::<u64>()
            .map_err(|err| bad_request(key, format!("invalid integer `{value}`: {err}")))
    }

    fn parse_u32(value: &str, key: &str) -> Result<u32, (StatusCode, String)> {
        value
            .trim()
            .parse::<u32>()
            .map_err(|err| bad_request(key, format!("invalid integer `{value}`: {err}")))
    }

    fn parse_i32(value: &str, key: &str) -> Result<i32, (StatusCode, String)> {
        value
            .trim()
            .parse::<i32>()
            .map_err(|err| bad_request(key, format!("invalid integer `{value}`: {err}")))
    }

    pub(super) fn parse_name(
        metadata: &ExtraMetadata,
        key: &str,
    ) -> Result<Name, (StatusCode, String)> {
        let value = require_utf8(metadata, key)?;
        Name::from_str(value.trim())
            .map_err(|err| bad_request(key, format!("invalid Name `{value}`: {err}")))
    }

    fn require_utf8<'a>(
        metadata: &'a ExtraMetadata,
        key: &str,
    ) -> Result<&'a str, (StatusCode, String)> {
        let entry = metadata_entry(metadata, key)?;
        std::str::from_utf8(&entry.value).map_err(|_| bad_request(key, "value must be valid UTF-8"))
    }

    fn optional_utf8<'a>(
        metadata: &'a ExtraMetadata,
        key: &str,
    ) -> Result<Option<&'a str>, (StatusCode, String)> {
        let Some(entry) = metadata.items.iter().find(|entry| entry.key == key) else {
            return Ok(None);
        };
        validate_metadata_entry(entry).map_err(|message| bad_request(key, message))?;
        let value = std::str::from_utf8(&entry.value)
            .map_err(|_| bad_request(key, "value must be valid UTF-8"))?;
        Ok(Some(value))
    }

    pub(super) fn take_ssm_entry(
        metadata: &mut ExtraMetadata,
    ) -> Result<Option<Vec<u8>>, (StatusCode, String)> {
        if let Some(index) = metadata
            .items
            .iter()
            .position(|entry| entry.key == META_TAIKAI_SSM)
        {
            let entry = metadata.items.remove(index);
            validate_metadata_entry(&entry)
                .map_err(|message| bad_request(META_TAIKAI_SSM, message))?;
            return Ok(Some(entry.value));
        }
        Ok(None)
    }

    pub(super) fn take_trm_entry(
        metadata: &mut ExtraMetadata,
    ) -> Result<Option<Vec<u8>>, (StatusCode, String)> {
        if let Some(index) = metadata
            .items
            .iter()
            .position(|entry| entry.key == META_TAIKAI_TRM)
        {
            let entry = metadata.items.remove(index);
            validate_metadata_entry(&entry)
                .map_err(|message| bad_request(META_TAIKAI_TRM, message))?;
            return Ok(Some(entry.value));
        }
        Ok(None)
    }

    fn metadata_entry<'a>(
        metadata: &'a ExtraMetadata,
        key: &str,
    ) -> Result<&'a MetadataEntry, (StatusCode, String)> {
        let entry = metadata
            .items
            .iter()
            .find(|entry| entry.key == key)
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("metadata entry `{key}` is required for Taikai segments"),
                )
            })?;
        validate_metadata_entry(entry).map_err(|message| bad_request(key, message))?;
        Ok(entry)
    }

    fn validate_metadata_entry(entry: &MetadataEntry) -> Result<(), String> {
        if entry.visibility != MetadataVisibility::Public {
            return Err("must use public visibility".into());
        }
        if !matches!(entry.encryption, MetadataEncryption::None) {
            return Err("must not be encrypted".into());
        }
        Ok(())
    }

    pub(super) fn bad_request(key: &str, message: impl Into<String>) -> (StatusCode, String) {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid Taikai metadata `{key}`: {}", message.into()),
        )
    }

    fn parse_error(key: &str, err: TaikaiParseError) -> (StatusCode, String) {
        bad_request(key, err.to_string())
    }

    pub(super) fn internal_error(message: String) -> (StatusCode, String) {
        (StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    fn encode_base32_lower(data: &[u8]) -> String {
        const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
        if data.is_empty() {
            return String::new();
        }
        let mut acc = 0u32;
        let mut bits = 0u32;
        let mut out = Vec::with_capacity((data.len() * 8).div_ceil(5));
        for &byte in data {
            acc = (acc << 8) | u32::from(byte);
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                let index = ((acc >> bits) & 0x1f) as usize;
                out.push(ALPHABET[index]);
            }
        }
        if bits > 0 {
            let index = ((acc << (5 - bits)) & 0x1f) as usize;
            out.push(ALPHABET[index]);
        }
        String::from_utf8(out).expect("alphabet contains valid ASCII")
    }

    pub fn spawn_anchor_worker(
        manifest_store_dir: PathBuf,
        anchor_cfg: DaTaikaiAnchor,
        shutdown: ShutdownSignal,
    ) {
        if anchor_cfg.poll_interval.is_zero() {
            iroha_logger::warn!("Taikai anchor poll interval is zero; using 1 second");
        }
        let poll_interval = if anchor_cfg.poll_interval.is_zero() {
            Duration::from_secs(1)
        } else {
            anchor_cfg.poll_interval
        };

        let spool_dir = manifest_store_dir.join(TAIKAI_SPOOL_SUBDIR);
        let sender = match HttpAnchorSender::new() {
            Ok(sender) => sender,
            Err(err) => {
                iroha_logger::error!(?err, "failed to initialise Taikai anchor HTTP client");
                return;
            }
        };

        tokio::spawn(async move {
            if let Err(err) = async_fs::create_dir_all(&spool_dir).await {
                if err.kind() != ErrorKind::AlreadyExists {
                    iroha_logger::warn!(
                        ?err,
                        ?spool_dir,
                        "failed to prepare Taikai spool directory"
                    );
                }
            }

            run_anchor_worker(spool_dir, anchor_cfg, sender, shutdown, poll_interval).await;
        });
    }

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use iroha_data_model::Encode;
    use tokio::{
        fs as async_fs,
        time::{MissedTickBehavior, interval},
    };

    struct HttpAnchorSender {
        client: Client,
    }

    impl HttpAnchorSender {
        fn new() -> Result<Self, reqwest::Error> {
            Ok(Self {
                client: Client::builder().build()?,
            })
        }
    }

    #[async_trait]
    pub(super) trait AnchorSender: Send + Sync {
        async fn send(
            &self,
            endpoint: &reqwest::Url,
            body: String,
            api_token: Option<&str>,
        ) -> Result<(), reqwest::Error>;
    }

    #[async_trait]
    impl AnchorSender for HttpAnchorSender {
        async fn send(
            &self,
            endpoint: &reqwest::Url,
            body: String,
            api_token: Option<&str>,
        ) -> Result<(), reqwest::Error> {
            let mut request = self
                .client
                .post(endpoint.clone())
                .header("content-type", "application/json");
            if let Some(token) = api_token {
                request = request.header("authorization", token);
            }
            request.body(body).send().await?.error_for_status()?;
            Ok(())
        }
    }

    pub(super) struct PendingUpload {
        base_id: String,
        body: String,
        sentinel_path: PathBuf,
    }

    impl PendingUpload {
        pub(super) fn base_id(&self) -> &str {
            &self.base_id
        }

        pub(super) fn body(&self) -> &str {
            &self.body
        }
    }

    async fn persist_anchor_request_capture(
        spool_dir: &Path,
        base_id: &str,
        body: &str,
    ) -> Result<(), std::io::Error> {
        let request_path = spool_dir.join(format!(
            "{TAIKAI_ANCHOR_REQUEST_PREFIX}{base_id}{TAIKAI_ANCHOR_REQUEST_SUFFIX}"
        ));
        if async_fs::metadata(&request_path).await.is_ok() {
            return Ok(());
        }
        async_fs::write(&request_path, body).await
    }

    async fn run_anchor_worker<S>(
        spool_dir: PathBuf,
        anchor_cfg: DaTaikaiAnchor,
        sender: S,
        shutdown: ShutdownSignal,
        poll_interval: Duration,
    ) where
        S: AnchorSender + 'static,
    {
        let mut ticker = interval(poll_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = shutdown.receive() => break,
                _ = ticker.tick() => {
                    if let Err(err) = process_batch(&spool_dir, &anchor_cfg, &sender).await {
                        iroha_logger::error!(?err, "failed to process Taikai anchor batch");
                    }
                }
            }
        }
    }

    pub(super) async fn process_batch<S>(
        spool_dir: &Path,
        anchor_cfg: &DaTaikaiAnchor,
        sender: &S,
    ) -> Result<(), String>
    where
        S: AnchorSender + ?Sized,
    {
        let uploads = collect_pending_uploads(spool_dir).await?;
        if uploads.is_empty() {
            return Ok(());
        }

        for upload in uploads {
            match sender
                .send(
                    &anchor_cfg.endpoint,
                    upload.body.clone(),
                    anchor_cfg.api_token.as_deref(),
                )
                .await
            {
                Ok(()) => {
                    let marker = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                        .to_string();
                    if let Err(err) = async_fs::write(&upload.sentinel_path, marker).await {
                        iroha_logger::warn!(
                            ?err,
                            path = ?upload.sentinel_path,
                            "failed to write Taikai anchor sentinel"
                        );
                    }
                }
                Err(err) => {
                    iroha_logger::warn!(
                        ?err,
                        base = upload.base_id,
                        "failed to deliver Taikai envelope to anchor service"
                    );
                }
            }
        }

        Ok(())
    }

    pub(super) async fn collect_pending_uploads(
        spool_dir: &Path,
    ) -> Result<Vec<PendingUpload>, String> {
        let mut result = Vec::new();
        let mut dir = match async_fs::read_dir(spool_dir).await {
            Ok(dir) => dir,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(result),
            Err(err) => {
                return Err(format!(
                    "failed to read Taikai spool directory `{}`: {err}",
                    spool_dir.display()
                ));
            }
        };

        while let Some(entry) = dir.next_entry().await.map_err(|err| {
            format!(
                "failed to iterate Taikai spool directory `{}`: {err}",
                spool_dir.display()
            )
        })? {
            let file_name = entry.file_name();
            let file_name = match file_name.to_str() {
                Some(value) => value,
                None => continue,
            };

            if !file_name.starts_with("taikai-envelope-") || !file_name.ends_with(".norito") {
                continue;
            }

            let base_id = &file_name["taikai-envelope-".len()..file_name.len() - ".norito".len()];
            let sentinel_path = spool_dir.join(format!(
                "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
            ));
            if async_fs::metadata(&sentinel_path).await.is_ok() {
                continue;
            }

            let envelope_path = entry.path();
            let indexes_name = format!("taikai-indexes-{base_id}.json");
            let indexes_path = spool_dir.join(&indexes_name);
            if async_fs::metadata(&indexes_path).await.is_err() {
                continue;
            }
            let ssm_name = format!("taikai-ssm-{base_id}.norito");
            let ssm_path = spool_dir.join(&ssm_name);
            if async_fs::metadata(&ssm_path).await.is_err() {
                continue;
            }

            let envelope_bytes = match async_fs::read(&envelope_path).await {
                Ok(bytes) => bytes,
                Err(err) => {
                    iroha_logger::warn!(?err, path = ?envelope_path, "failed to read Taikai envelope");
                    continue;
                }
            };

            let indexes_value: Value = match async_fs::read(&indexes_path).await {
                Ok(bytes) => match json::from_slice(&bytes) {
                    Ok(value) => value,
                    Err(err) => {
                        iroha_logger::warn!(?err, path = ?indexes_path, "failed to parse Taikai indexes JSON");
                        continue;
                    }
                },
                Err(err) => {
                    iroha_logger::warn!(?err, path = ?indexes_path, "failed to read Taikai indexes JSON");
                    continue;
                }
            };

            let ssm_bytes = match async_fs::read(&ssm_path).await {
                Ok(bytes) => bytes,
                Err(err) => {
                    iroha_logger::warn!(?err, path = ?ssm_path, "failed to read Taikai signing manifest");
                    continue;
                }
            };

            let trm_name = format!("taikai-trm-{base_id}.norito");
            let trm_path = spool_dir.join(&trm_name);
            let trm_bytes = match async_fs::read(&trm_path).await {
                Ok(bytes) => Some(bytes),
                Err(err) => {
                    if err.kind() != ErrorKind::NotFound {
                        iroha_logger::warn!(?err, path = ?trm_path, "failed to read Taikai routing manifest");
                    }
                    None
                }
            };
            let lineage_name = format!("{TAIKAI_LINEAGE_HINT_PREFIX}-{base_id}.json");
            let lineage_path = spool_dir.join(&lineage_name);
            let lineage_value = match async_fs::read(&lineage_path).await {
                Ok(bytes) => match json::from_slice(&bytes) {
                    Ok(value) => Some(value),
                    Err(err) => {
                        iroha_logger::warn!(
                            ?err,
                            path = ?lineage_path,
                            "failed to parse Taikai lineage hint JSON"
                        );
                        None
                    }
                },
                Err(err) => {
                    if err.kind() != ErrorKind::NotFound {
                        iroha_logger::warn!(
                            ?err,
                            path = ?lineage_path,
                            "failed to read Taikai lineage hint JSON"
                        );
                    }
                    None
                }
            };

            let mut payload = Map::new();
            let envelope_b64 = BASE64.encode(envelope_bytes);
            payload.insert("envelope_base64".to_string(), Value::String(envelope_b64));
            payload.insert("indexes".to_string(), indexes_value);
            let ssm_b64 = BASE64.encode(ssm_bytes);
            payload.insert("ssm_base64".to_string(), Value::String(ssm_b64));
            if let Some(trm_bytes) = trm_bytes {
                let trm_b64 = BASE64.encode(trm_bytes);
                payload.insert("trm_base64".to_string(), Value::String(trm_b64));
            }
            if let Some(value) = lineage_value {
                payload.insert("lineage_hint".to_string(), value);
            }
            let payload = Value::Object(payload);
            let body = match json::to_string(&payload) {
                Ok(body) => body,
                Err(err) => {
                    iroha_logger::warn!(
                        ?err,
                        base = base_id,
                        "failed to encode Taikai anchor payload"
                    );
                    continue;
                }
            };

            if let Err(err) = persist_anchor_request_capture(spool_dir, base_id, &body).await {
                iroha_logger::warn!(
                    ?err,
                    path = ?spool_dir.join(format!(
                        "{TAIKAI_ANCHOR_REQUEST_PREFIX}{base_id}{TAIKAI_ANCHOR_REQUEST_SUFFIX}"
                    )),
                    "failed to persist Taikai anchor request payload for governance bundle"
                );
            }

            result.push(PendingUpload {
                base_id: base_id.to_string(),
                body,
                sentinel_path,
            });
        }

        Ok(result)
    }
}

fn record_taikai_ingest_metrics(
    telemetry: &MaybeTelemetry,
    cluster_label: &str,
    sample: &taikai_ingest::TaikaiTelemetrySample,
) {
    if !telemetry.is_enabled() {
        return;
    }
    telemetry.with_metrics(|handle| {
        if let Some(latency) = sample.ingest_latency_ms {
            handle.observe_taikai_ingest_latency(cluster_label, sample.stream_id.as_str(), latency);
        }
        if let Some(drift) = sample.live_edge_drift_ms {
            handle.observe_taikai_live_edge_drift(cluster_label, sample.stream_id.as_str(), drift);
        }
    });
}

fn record_taikai_alias_rotation_event(
    telemetry: &MaybeTelemetry,
    cluster_label: &str,
    manifest: &TaikaiRoutingManifestV1,
    manifest_digest_hex: &str,
) {
    if !telemetry.is_enabled() {
        return;
    }
    let event_label = manifest.event_id.as_name().as_ref().to_owned();
    let stream_label = manifest.stream_id.as_name().as_ref().to_owned();
    let alias_namespace = manifest.alias_binding.namespace.clone();
    let alias_name = manifest.alias_binding.name.clone();
    let window = manifest.segment_window;
    telemetry.with_metrics(|handle| {
        handle.record_taikai_alias_rotation(
            cluster_label,
            &event_label,
            &stream_label,
            &alias_namespace,
            &alias_name,
            window.start_sequence,
            window.end_sequence,
            manifest_digest_hex,
        );
    });
}

fn record_taikai_ingest_error(
    telemetry: &MaybeTelemetry,
    cluster_label: &str,
    stream_label: &str,
    status: StatusCode,
) {
    if !telemetry.is_enabled() {
        return;
    }
    let reason = status
        .canonical_reason()
        .map(Cow::Borrowed)
        .unwrap_or_else(|| Cow::Owned(status.as_u16().to_string()));
    telemetry.with_metrics(|handle| {
        handle.inc_taikai_ingest_error(cluster_label, stream_label, &reason);
    });
}

fn record_da_rent_quote_metrics(
    telemetry: &MaybeTelemetry,
    cluster_label: &str,
    storage_class: StorageClass,
    rent_gib: u64,
    rent_months: u32,
    rent_quote: &DaRentQuote,
) {
    if !telemetry.is_enabled() {
        return;
    }
    let months_u64 = u64::from(rent_months);
    let gib_months = rent_gib.saturating_mul(months_u64);
    let storage_label = storage_class_label(storage_class);
    telemetry.with_metrics(|handle| {
        handle.record_da_rent_quote(cluster_label, storage_label, gib_months, rent_quote);
    });
}

fn record_da_receipt_metrics(
    telemetry: &MaybeTelemetry,
    lane_epoch: LaneEpoch,
    sequence: u64,
    outcome: &ReceiptInsertOutcome,
) {
    if !telemetry.is_enabled() {
        return;
    }
    let (outcome_label, cursor_advanced) = match outcome {
        ReceiptInsertOutcome::Stored { cursor_advanced } => ("stored", *cursor_advanced),
        ReceiptInsertOutcome::Duplicate { .. } => ("duplicate", false),
        ReceiptInsertOutcome::ManifestConflict { .. } => ("manifest_conflict", false),
        ReceiptInsertOutcome::StaleSequence { .. } => ("stale_sequence", false),
    };
    telemetry.with_metrics(|handle| {
        handle.record_da_receipt_outcome(
            lane_epoch.lane_id.as_u32(),
            lane_epoch.epoch,
            sequence,
            outcome_label,
            cursor_advanced,
        );
    });
}

fn record_da_receipt_error_metrics(
    telemetry: &MaybeTelemetry,
    lane_epoch: LaneEpoch,
    sequence: u64,
) {
    if !telemetry.is_enabled() {
        return;
    }
    telemetry.with_metrics(|handle| {
        handle.record_da_receipt_outcome(
            lane_epoch.lane_id.as_u32(),
            lane_epoch.epoch,
            sequence,
            "error",
            false,
        );
    });
}

fn storage_class_label(class: StorageClass) -> &'static str {
    match class {
        StorageClass::Hot => "hot",
        StorageClass::Warm => "warm",
        StorageClass::Cold => "cold",
    }
}

fn da_metadata_error(key: &str, message: impl Into<String>) -> (StatusCode, String) {
    (
        StatusCode::BAD_REQUEST,
        format!("invalid DA metadata `{key}`: {}", message.into()),
    )
}

fn validate_public_metadata_entry(
    entry: &MetadataEntry,
    key: &str,
) -> Result<(), (StatusCode, String)> {
    if entry.visibility != MetadataVisibility::Public {
        return Err(da_metadata_error(key, "must use public visibility"));
    }
    if !matches!(entry.encryption, MetadataEncryption::None) {
        return Err(da_metadata_error(key, "must not be encrypted"));
    }
    Ok(())
}

fn registry_alias_from_metadata(
    metadata: &ExtraMetadata,
) -> Result<Option<String>, (StatusCode, String)> {
    let Some(entry) = metadata
        .items
        .iter()
        .find(|entry| entry.key == META_DA_REGISTRY_ALIAS)
    else {
        return Ok(None);
    };
    validate_public_metadata_entry(entry, META_DA_REGISTRY_ALIAS)?;
    let value = std::str::from_utf8(&entry.value)
        .map_err(|_| da_metadata_error(META_DA_REGISTRY_ALIAS, "value must be valid UTF-8"))?
        .trim();
    if value.is_empty() {
        return Err(da_metadata_error(
            META_DA_REGISTRY_ALIAS,
            "alias must not be empty",
        ));
    }
    Ok(Some(value.to_owned()))
}

fn registry_owner_from_metadata(
    metadata: &ExtraMetadata,
) -> Result<Option<AccountId>, (StatusCode, String)> {
    let Some(entry) = metadata
        .items
        .iter()
        .find(|entry| entry.key == META_DA_REGISTRY_OWNER)
    else {
        return Ok(None);
    };
    validate_public_metadata_entry(entry, META_DA_REGISTRY_OWNER)?;
    let value = std::str::from_utf8(&entry.value)
        .map_err(|_| da_metadata_error(META_DA_REGISTRY_OWNER, "value must be valid UTF-8"))?
        .trim();
    if value.is_empty() {
        return Err(da_metadata_error(
            META_DA_REGISTRY_OWNER,
            "owner must not be empty",
        ));
    }
    let owner = AccountId::from_str(value).map_err(|err| {
        da_metadata_error(
            META_DA_REGISTRY_OWNER,
            format!("invalid AccountId `{value}`: {err}"),
        )
    })?;
    Ok(Some(owner))
}

fn stream_label_from_metadata(metadata: &ExtraMetadata) -> Option<String> {
    metadata
        .items
        .iter()
        .find(|entry| entry.key == META_TAIKAI_STREAM_ID)
        .and_then(|entry| std::str::from_utf8(&entry.value).ok())
        .map(|value| value.trim().to_owned())
}

fn validate_da_proof_tier(
    metadata: &ExtraMetadata,
    expected_storage_class: StorageClass,
) -> Result<(), (StatusCode, String)> {
    let entry = metadata
        .items
        .iter()
        .find(|entry| entry.key == META_DA_PROOF_TIER)
        .ok_or_else(|| {
            taikai_ingest::bad_request(
                META_DA_PROOF_TIER,
                "metadata entry is required for Taikai segments",
            )
        })?;
    if entry.visibility != MetadataVisibility::Public
        || !matches!(entry.encryption, MetadataEncryption::None)
    {
        return Err(taikai_ingest::bad_request(
            META_DA_PROOF_TIER,
            "metadata entry must be public and unencrypted",
        ));
    }
    let value = std::str::from_utf8(&entry.value)
        .map_err(|_| taikai_ingest::bad_request(META_DA_PROOF_TIER, "value must be UTF-8"))?
        .trim();
    let expected = storage_class_label(expected_storage_class);
    if value != expected {
        return Err(taikai_ingest::bad_request(
            META_DA_PROOF_TIER,
            format!("tier `{value}` does not match enforced storage class `{expected}`"),
        ));
    }
    Ok(())
}

fn validate_taikai_cache_hint(
    metadata: &ExtraMetadata,
    payload_digest: &BlobDigest,
    expected_payload_len: u64,
) -> Result<(), (StatusCode, String)> {
    let entry = metadata
        .items
        .iter()
        .find(|entry| entry.key == META_TAIKAI_CACHE_HINT)
        .ok_or_else(|| {
            taikai_ingest::bad_request(
                META_TAIKAI_CACHE_HINT,
                "metadata entry is required for Taikai segments",
            )
        })?;
    if entry.visibility != MetadataVisibility::Public
        || !matches!(entry.encryption, MetadataEncryption::None)
    {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_CACHE_HINT,
            "metadata entry must be public and unencrypted",
        ));
    }
    let hint_value: Value = json::from_slice(&entry.value).map_err(|err| {
        taikai_ingest::bad_request(
            META_TAIKAI_CACHE_HINT,
            format!("failed to parse cache hint JSON: {err}"),
        )
    })?;
    let hint = hint_value.as_object().ok_or_else(|| {
        taikai_ingest::bad_request(META_TAIKAI_CACHE_HINT, "cache hint must be a JSON object")
    })?;

    let event = taikai_ingest::parse_name(metadata, META_TAIKAI_EVENT_ID)?;
    let stream = taikai_ingest::parse_name(metadata, META_TAIKAI_STREAM_ID)?;
    let rendition = taikai_ingest::parse_name(metadata, META_TAIKAI_RENDITION_ID)?;
    let sequence_entry = metadata
        .items
        .iter()
        .find(|entry| entry.key == META_TAIKAI_SEGMENT_SEQUENCE)
        .ok_or_else(|| {
            taikai_ingest::bad_request(
                META_TAIKAI_CACHE_HINT,
                "cache hint validation requires taikai.segment.sequence metadata",
            )
        })?;
    if sequence_entry.visibility != MetadataVisibility::Public
        || !matches!(sequence_entry.encryption, MetadataEncryption::None)
    {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SEGMENT_SEQUENCE,
            "metadata entry must be public and unencrypted",
        ));
    }
    let sequence_str = std::str::from_utf8(&sequence_entry.value).map_err(|_| {
        taikai_ingest::bad_request(META_TAIKAI_SEGMENT_SEQUENCE, "value must be UTF-8")
    })?;
    let sequence = sequence_str.parse::<u64>().map_err(|err| {
        taikai_ingest::bad_request(
            META_TAIKAI_SEGMENT_SEQUENCE,
            format!("invalid integer `{sequence_str}`: {err}"),
        )
    })?;

    let expect_str = |key: &str| -> Result<&str, (StatusCode, String)> {
        hint.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                taikai_ingest::bad_request(
                    META_TAIKAI_CACHE_HINT,
                    format!("{key} is required in cache hint"),
                )
            })
    };
    let expect_u64 = |key: &str| -> Result<u64, (StatusCode, String)> {
        hint.get(key).and_then(Value::as_u64).ok_or_else(|| {
            taikai_ingest::bad_request(
                META_TAIKAI_CACHE_HINT,
                format!("{key} is required in cache hint"),
            )
        })
    };

    let event_value = expect_str("event")?;
    let stream_value = expect_str("stream")?;
    let rendition_value = expect_str("rendition")?;
    if event_value != event.as_ref()
        || stream_value != stream.as_ref()
        || rendition_value != rendition.as_ref()
    {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_CACHE_HINT,
            "cache hint identifiers must match segment metadata",
        ));
    }

    let sequence_value = expect_u64("sequence")?;
    if sequence_value != sequence {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_CACHE_HINT,
            "cache hint sequence must match segment metadata",
        ));
    }

    let payload_len = expect_u64("payload_len")?;
    if payload_len != expected_payload_len {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_CACHE_HINT,
            "cache hint payload_len must match canonical payload size",
        ));
    }

    let digest_hex = expect_str("payload_blake3_hex")?;
    let digest_bytes = hex::decode(digest_hex).map_err(|err| {
        taikai_ingest::bad_request(
            META_TAIKAI_CACHE_HINT,
            format!("payload_blake3_hex must be valid hex: {err}"),
        )
    })?;
    if digest_bytes.as_slice() != payload_digest.as_ref() {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_CACHE_HINT,
            "cache hint payload digest does not match canonical payload",
        ));
    }

    Ok(())
}

#[derive(Debug)]
struct TaikaiSsmOutcome {
    alias_label: String,
    ssm_digest: BlobDigest,
    evaluation: crate::sorafs::AliasProofEvaluation,
}

fn validate_taikai_ssm(
    ssm_bytes: &[u8],
    manifest_hash: &BlobDigest,
    car_digest: &BlobDigest,
    envelope_bytes: &[u8],
    expected_sequence: u64,
    alias_policy: &crate::sorafs::AliasCachePolicy,
    telemetry: &MaybeTelemetry,
) -> Result<TaikaiSsmOutcome, (StatusCode, String)> {
    let signing_manifest: TaikaiSegmentSigningManifestV1 =
        decode_from_bytes(ssm_bytes).map_err(|err| {
            taikai_ingest::bad_request(
                META_TAIKAI_SSM,
                format!("failed to decode signing manifest: {err}"),
            )
        })?;

    signing_manifest
        .signature
        .verify(&signing_manifest.body.publisher_key, &signing_manifest.body)
        .map_err(|err| {
            taikai_ingest::bad_request(
                META_TAIKAI_SSM,
                format!("publisher signature verification failed: {err}"),
            )
        })?;

    if &signing_manifest.body.manifest_hash != manifest_hash {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            "manifest hash mismatch between SSM and ingest artefact",
        ));
    }
    if &signing_manifest.body.car_digest != car_digest {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            "CAR digest mismatch between SSM and ingest artefact",
        ));
    }

    let envelope_hash = BlobDigest::from_hash(blake3_hash(envelope_bytes));
    if signing_manifest.body.segment_envelope_hash != envelope_hash {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            "envelope hash mismatch between SSM and ingest artefact",
        ));
    }
    if signing_manifest.body.segment_sequence != expected_sequence {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            "segment sequence mismatch between SSM and ingest metadata",
        ));
    }

    let alias_binding = &signing_manifest.body.alias_binding;
    if alias_binding.proof.is_empty() {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            "alias proof payload must not be empty",
        ));
    }

    let alias_label = format!("{}/{}", alias_binding.namespace, alias_binding.name);
    let alias_proof = crate::sorafs::decode_alias_proof(&alias_binding.proof).map_err(|err| {
        taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            format!("alias proof failed validation for `{alias_label}`: {err}"),
        )
    })?;
    if alias_proof.binding.alias != alias_label {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            format!(
                "alias proof binding `{}` does not match signing manifest alias `{alias_label}`",
                alias_proof.binding.alias
            ),
        ));
    }

    let now_secs = crate::sorafs::unix_now_secs();
    let evaluation = alias_policy.evaluate(&alias_proof, now_secs);
    let status_label = evaluation.status_label();
    let result = if evaluation.state.is_servable() {
        "success"
    } else {
        "error"
    };
    telemetry.with_metrics(|metrics| {
        metrics.record_sorafs_alias_cache(result, status_label, evaluation.age.as_secs_f64());
    });

    if !evaluation.state.is_servable() {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            format!("alias proof for `{alias_label}` expired ({status_label})"),
        ));
    }

    let ssm_digest = BlobDigest::from_hash(blake3_hash(ssm_bytes));
    Ok(TaikaiSsmOutcome {
        alias_label,
        ssm_digest,
        evaluation,
    })
}

fn taikai_availability_from_metadata(
    metadata: &ExtraMetadata,
    trm_payload: Option<&[u8]>,
) -> Result<Option<TaikaiAvailabilityClass>, (StatusCode, String)> {
    let Some(bytes) = trm_payload else {
        return Ok(None);
    };
    let manifest: TaikaiRoutingManifestV1 = decode_from_bytes(bytes).map_err(|err| {
        taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!("failed to decode routing manifest: {err}"),
        )
    })?;
    let event_id = taikai_ingest::parse_name(metadata, META_TAIKAI_EVENT_ID)?;
    if manifest.event_id.as_name() != &event_id {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!(
                "manifest event_id `{}` does not match segment metadata `{}`",
                manifest.event_id.as_name(),
                event_id.as_ref()
            ),
        ));
    }
    let stream_id = taikai_ingest::parse_name(metadata, META_TAIKAI_STREAM_ID)?;
    if manifest.stream_id.as_name() != &stream_id {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!(
                "manifest stream_id `{}` does not match segment metadata `{}`",
                manifest.stream_id.as_name(),
                stream_id.as_ref()
            ),
        ));
    }
    let rendition_name = taikai_ingest::parse_name(metadata, META_TAIKAI_RENDITION_ID)?;
    let Some(route) = manifest
        .renditions
        .iter()
        .find(|route| route.rendition_id.as_name() == &rendition_name)
    else {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!(
                "manifest missing rendition `{}` required by this segment",
                rendition_name.as_ref()
            ),
        ));
    };
    Ok(Some(route.availability_class))
}

fn apply_taikai_ingest_tags(
    metadata: &mut ExtraMetadata,
    availability: Option<TaikaiAvailabilityClass>,
    retention: &RetentionPolicy,
    payload_digest: BlobDigest,
    payload_len: u64,
) -> Result<(), (StatusCode, String)> {
    let availability_class =
        availability.unwrap_or_else(|| TaikaiAvailabilityClass::from(retention.storage_class));
    upsert_metadata(
        metadata,
        META_TAIKAI_AVAILABILITY_CLASS,
        availability_label(availability_class),
    );
    upsert_metadata(
        metadata,
        META_DA_PROOF_TIER,
        storage_class_label(retention.storage_class),
    );
    upsert_metadata(
        metadata,
        META_TAIKAI_REPLICATION_REPLICAS,
        retention.required_replicas.to_string(),
    );
    upsert_metadata(
        metadata,
        META_TAIKAI_REPLICATION_STORAGE,
        storage_class_label(retention.storage_class),
    );
    upsert_metadata(
        metadata,
        META_TAIKAI_REPLICATION_HOT_SECS,
        retention.hot_retention_secs.to_string(),
    );
    upsert_metadata(
        metadata,
        META_TAIKAI_REPLICATION_COLD_SECS,
        retention.cold_retention_secs.to_string(),
    );

    let sample_window = compute_sample_window(payload_len);
    upsert_metadata(
        metadata,
        META_DA_PDP_SAMPLE_WINDOW,
        sample_window.to_string(),
    );
    upsert_metadata(
        metadata,
        META_DA_POTR_SAMPLE_WINDOW,
        sample_window.to_string(),
    );

    let event = taikai_ingest::parse_name(metadata, META_TAIKAI_EVENT_ID)?;
    let stream = taikai_ingest::parse_name(metadata, META_TAIKAI_STREAM_ID)?;
    let rendition = taikai_ingest::parse_name(metadata, META_TAIKAI_RENDITION_ID)?;
    let sequence_entry = metadata
        .items
        .iter()
        .find(|entry| entry.key == META_TAIKAI_SEGMENT_SEQUENCE)
        .ok_or_else(|| {
            taikai_ingest::bad_request(
                META_TAIKAI_SEGMENT_SEQUENCE,
                "metadata entry `taikai.segment.sequence` is required for Taikai segments",
            )
        })?;
    if sequence_entry.visibility != MetadataVisibility::Public {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SEGMENT_SEQUENCE,
            "metadata visibility must be public for Taikai segments",
        ));
    }
    if sequence_entry.encryption != MetadataEncryption::None {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SEGMENT_SEQUENCE,
            "metadata encryption is not supported for Taikai sequence fields",
        ));
    }
    let sequence_raw = std::str::from_utf8(&sequence_entry.value).map_err(|_| {
        taikai_ingest::bad_request(META_TAIKAI_SEGMENT_SEQUENCE, "value must be UTF-8")
    })?;
    let sequence = sequence_raw.parse::<u64>().map_err(|err| {
        taikai_ingest::bad_request(
            META_TAIKAI_SEGMENT_SEQUENCE,
            format!("invalid u64 `{sequence_raw}`: {err}"),
        )
    })?;

    let mut hint = Map::new();
    hint.insert("event".into(), Value::from(event.as_ref()));
    hint.insert("stream".into(), Value::from(stream.as_ref()));
    hint.insert("rendition".into(), Value::from(rendition.as_ref()));
    hint.insert("sequence".into(), Value::from(sequence));
    hint.insert("payload_len".into(), Value::from(payload_len));
    hint.insert(
        "payload_blake3_hex".into(),
        Value::from(hex::encode(payload_digest.as_ref())),
    );
    let rendered_hint = json::to_vec(&Value::Object(hint)).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode Taikai cache hint: {err}"),
        )
    })?;
    upsert_metadata(metadata, META_TAIKAI_CACHE_HINT, rendered_hint);

    Ok(())
}

/// Compute the Taikai ingest metadata enrichment applied by Torii.
///
/// # Errors
///
/// Returns a `(StatusCode, String)` when metadata serialization or tag
/// computation fails for the supplied payload.
pub fn compute_taikai_ingest_tags(
    mut metadata: ExtraMetadata,
    availability: Option<TaikaiAvailabilityClass>,
    retention: &RetentionPolicy,
    payload_digest: BlobDigest,
    payload_len: u64,
) -> Result<ExtraMetadata, (StatusCode, String)> {
    apply_taikai_ingest_tags(
        &mut metadata,
        availability,
        retention,
        payload_digest,
        payload_len,
    )?;
    Ok(metadata)
}

fn upsert_metadata(metadata: &mut ExtraMetadata, key: &str, value: impl Into<Vec<u8>>) {
    if let Some(index) = metadata.items.iter().position(|entry| entry.key == key) {
        metadata.items.remove(index);
    }
    metadata.items.push(MetadataEntry::new(
        key,
        value.into(),
        MetadataVisibility::Public,
    ));
}

fn availability_label(class: TaikaiAvailabilityClass) -> &'static str {
    match class {
        TaikaiAvailabilityClass::Hot => "hot",
        TaikaiAvailabilityClass::Warm => "warm",
        TaikaiAvailabilityClass::Cold => "cold",
    }
}

fn validate_taikai_trm(
    trm_bytes: &[u8],
    envelope: &taikai_ingest::EnvelopeArtifacts,
) -> Result<TaikaiRoutingManifestV1, (StatusCode, String)> {
    let manifest: TaikaiRoutingManifestV1 = decode_from_bytes(trm_bytes).map_err(|err| {
        taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!("failed to decode routing manifest: {err}"),
        )
    })?;

    if manifest.version != TaikaiRoutingManifestV1::VERSION {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!(
                "unsupported manifest version {}; expected {}",
                manifest.version,
                TaikaiRoutingManifestV1::VERSION
            ),
        ));
    }

    if let Err(err) = manifest.validate() {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!("invalid routing manifest: {err}"),
        ));
    }

    if manifest.event_id.as_name().as_ref() != envelope.telemetry.event_id {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!(
                "manifest event_id `{}` does not match segment metadata `{}`",
                manifest.event_id.as_name(),
                envelope.telemetry.event_id
            ),
        ));
    }

    if manifest.stream_id.as_name().as_ref() != envelope.telemetry.stream_id {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!(
                "manifest stream_id `{}` does not match segment metadata `{}`",
                manifest.stream_id.as_name(),
                envelope.telemetry.stream_id
            ),
        ));
    }

    let expected_rendition = envelope.telemetry.rendition_id.as_str();
    if !manifest
        .renditions
        .iter()
        .any(|route| route.rendition_id.as_name().as_ref() == expected_rendition)
    {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!("manifest missing rendition `{expected_rendition}` required by this segment"),
        ));
    }

    if !manifest.covers_sequence(envelope.telemetry.segment_sequence) {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            "manifest window does not cover this segment sequence",
        ));
    }

    Ok(manifest)
}

#[allow(clippy::too_many_arguments)]
fn verify_manifest_against_request(
    request: &DaIngestRequest,
    manifest: &DaManifestV1,
    expected_retention: &RetentionPolicy,
    expected_metadata: &ExtraMetadata,
    computed_chunks: &[ChunkCommitment],
    blob_hash: BlobDigest,
    chunk_root: BlobDigest,
    expected_rent: &DaRentQuote,
) -> Result<(), (StatusCode, String)> {
    if manifest.version != DaManifestV1::VERSION {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "unsupported manifest version {}; expected {}",
                manifest.version,
                DaManifestV1::VERSION
            ),
        ));
    }
    if manifest.client_blob_id != request.client_blob_id {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest client_blob_id does not match ingest request".into(),
        ));
    }
    if manifest.lane_id != request.lane_id || manifest.epoch != request.epoch {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest lane/epoch do not match ingest request".into(),
        ));
    }
    if manifest.blob_class != request.blob_class || manifest.codec != request.codec {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest blob classification does not match ingest request".into(),
        ));
    }
    if manifest.total_size != request.total_size || manifest.chunk_size != request.chunk_size {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest total_size or chunk_size does not match ingest request".into(),
        ));
    }
    if manifest.erasure_profile != request.erasure_profile {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest erasure profile does not match ingest request".into(),
        ));
    }
    if manifest.retention_policy != *expected_retention {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest retention policy does not match ingest request".into(),
        ));
    }
    if manifest.metadata != *expected_metadata {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest metadata does not match ingest request".into(),
        ));
    }
    if manifest.rent_quote != *expected_rent {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest rent quote does not match configured policy".into(),
        ));
    }
    if manifest.blob_hash != blob_hash {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest blob_hash does not match canonical payload digest".into(),
        ));
    }
    if manifest.chunk_root != chunk_root {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest chunk_root does not match recomputed chunk root".into(),
        ));
    }
    if manifest.chunks.len() != computed_chunks.len() {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest chunk count does not match chunker output".into(),
        ));
    }

    if manifest.blob_class == BlobClass::TaikaiSegment {
        validate_taikai_cache_hint(expected_metadata, &blob_hash, manifest.total_size)?;
        validate_da_proof_tier(expected_metadata, manifest.retention_policy.storage_class)?;
    }

    for (expected, actual) in computed_chunks.iter().zip(manifest.chunks.iter()) {
        if expected.index != actual.index
            || expected.offset != actual.offset
            || expected.length != actual.length
            || expected.commitment != actual.commitment
        {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "manifest chunk commitment mismatch at index {}",
                    expected.index
                ),
            ));
        }
        if expected.parity != actual.parity {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("manifest parity flag mismatch at index {}", expected.index),
            ));
        }
        if effective_chunk_role(expected) != effective_chunk_role(actual) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("manifest role mismatch at index {}", expected.index),
            ));
        }
        if actual.group_id != 0 && expected.group_id != actual.group_id {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("manifest group_id mismatch at index {}", expected.index),
            ));
        }
    }

    Ok(())
}

fn build_error_response(status: StatusCode, message: &str, format: ResponseFormat) -> Response {
    let mut map = Map::new();
    map.insert("status".into(), Value::from(status.as_str()));
    map.insert("error".into(), Value::from(message));
    let body = Value::Object(map);
    let mut response = utils::respond_value_with_format(body, format);
    *response.status_mut() = status;
    response
}

fn ceil_div_u64(value: u64, divisor: u64) -> u64 {
    if divisor == 0 {
        return 0;
    }
    if value == 0 {
        return 0;
    }
    value.div_ceil(divisor)
}

fn rent_usage_from_request(total_size: u64, retention: &RetentionPolicy) -> (u64, u32) {
    let adjusted_size = total_size.max(1);
    let gib = ceil_div_u64(adjusted_size, BYTES_PER_GIB).max(1);
    let retention_secs = retention
        .hot_retention_secs
        .max(retention.cold_retention_secs)
        .max(1);
    let months_u64 = ceil_div_u64(retention_secs, SECS_PER_MONTH).max(1);
    let months_u32 = u32::try_from(months_u64).unwrap_or(u32::MAX);
    (gib, months_u32)
}

#[allow(clippy::redundant_pub_crate)]
pub use taikai_ingest::spawn_anchor_worker;

fn with_status(mut response: Response, status: StatusCode) -> Response {
    *response.status_mut() = status;
    response
}

#[derive(JsonSerialize, norito::derive::NoritoSerialize)]
struct DaIngestResponse {
    status: &'static str,
    duplicate: bool,
    receipt: Option<DaIngestReceipt>,
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use core::convert::TryInto;
    use std::{
        fs,
        io::Write,
        num::NonZeroU32,
        path::{Path, PathBuf},
        str::FromStr,
        sync::{Arc, LazyLock},
        time::Duration,
    };

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use flate2::{Compression as FlateCompression, write::GzEncoder};
    use iroha_config::parameters::actual::{
        DaTaikaiAnchor, LaneConfig as ConfigLaneConfig, TelemetryProfile,
    };
    use iroha_core::{da::LaneEpoch, telemetry::Telemetry};
    use iroha_crypto::{Algorithm, KeyPair, PrivateKey, SignatureOf};
    use iroha_data_model::{
        Encode,
        account::AccountId,
        da::{
            commitment::{DaCommitmentBundle, KzgCommitment},
            ingest::DaStripeLayout,
            types::{BlobDigest, DaRentQuote, StorageTicketId},
        },
        nexus::{DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId},
        sorafs::{
            capacity::ProviderId,
            pin_registry::{ManifestAliasBinding, ManifestDigest},
        },
        taikai::{
            GuardDirectoryId, TaikaiAliasBinding, TaikaiAvailabilityClass, TaikaiCarPointer,
            TaikaiCidIndexKey, TaikaiEventId, TaikaiRenditionId, TaikaiRenditionRouteV1,
            TaikaiRoutingManifestV1, TaikaiSegmentSigningBodyV1, TaikaiSegmentSigningManifestV1,
            TaikaiSegmentWindow, TaikaiStreamId, TaikaiTimeIndexKey,
        },
    };
    use iroha_telemetry::metrics::Metrics;
    use norito::{
        codec::Decode,
        from_bytes,
        json::{self, Value},
        to_bytes,
    };
    use reqwest::Url;
    use sorafs_car::{CarBuildPlan, PersistedChunkRecord};
    use sorafs_manifest::{
        BLAKE3_256_MULTIHASH_CODE, ChunkingProfileV1, CouncilSignature, ProfileId,
        pdp::{HashAlgorithmV1, PDP_COMMITMENT_VERSION_V1, PdpCommitmentV1},
        pin_registry::{
            AliasBindingV1, AliasProofBundleV1, alias_merkle_root, alias_proof_signature_digest,
        },
    };
    use tempfile::tempdir;
    use tokio::{fs as async_fs, sync::Mutex as AsyncMutex};

    use super::*;
    use crate::da::taikai_ingest::{AnchorSender, collect_pending_uploads, process_batch};

    #[test]
    fn replay_cursor_temp_path_keeps_suffixes() {
        let base = Path::new("/var/lib/iroha/replay_cursors.norito.json");
        let tmp = replay_cursor_temp_path(base);
        assert_eq!(
            tmp,
            Path::new("/var/lib/iroha/replay_cursors.norito.json.tmp")
        );
    }

    #[test]
    fn parse_storage_ticket_hex_validates_variants() {
        let valid = format!("0x{}", "aa".repeat(32));
        let parsed = parse_storage_ticket_hex(&valid).expect("valid ticket");
        assert_eq!(parsed.len(), 32);
        assert!(parse_storage_ticket_hex("").is_err());
        assert!(parse_storage_ticket_hex("zz").is_err());
        assert!(parse_storage_ticket_hex("ab").is_err(), "too short");
    }

    #[test]
    fn load_manifest_from_spool_locates_ticket() {
        let dir = tempdir().expect("dir");
        let ticket = StorageTicketId::new([0x77; 32]);
        let ticket_hex = hex::encode(ticket.as_bytes());
        let file = format!(
            "manifest-00000001-0000000000000001-0000000000000002-{ticket_hex}-deadbeef.norito"
        );
        let path = dir.path().join(file);
        fs::write(&path, b"manifest-bytes").expect("manifest file");

        let bytes = load_manifest_from_spool(dir.path(), &ticket).expect("manifest bytes");
        assert_eq!(bytes, b"manifest-bytes");

        let missing = StorageTicketId::new([0x55; 32]);
        let err = load_manifest_from_spool(dir.path(), &missing).expect_err("missing ticket");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn load_pdp_commitment_from_spool_locates_ticket() {
        let dir = tempdir().expect("dir");
        let ticket = StorageTicketId::new([0x99; 32]);
        let ticket_hex = hex::encode(ticket.as_bytes());
        let file = format!(
            "pdp-commitment-00000001-0000000000000001-0000000000000002-{ticket_hex}-feedface.norito"
        );
        let path = dir.path().join(file);
        let commitment = sample_pdp_commitment_for_tests();
        let bytes = encode_pdp_commitment_bytes(&commitment).expect("encode commitment");
        fs::write(&path, &bytes).expect("commitment file");

        let loaded = load_pdp_commitment_from_spool(dir.path(), &ticket).expect("commitment");
        assert_eq!(loaded, bytes);

        let missing = StorageTicketId::new([0x55; 32]);
        let err =
            load_pdp_commitment_from_spool(dir.path(), &missing).expect_err("missing commitment");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn pdp_commitment_header_value_matches_base64_payload() {
        let commitment = sample_pdp_commitment_for_tests();
        let bytes = encode_pdp_commitment_bytes(&commitment).expect("encode commitment");
        let header_value = pdp_commitment_header_value(&bytes).expect("header value");
        let expected = BASE64.encode(bytes);
        assert_eq!(header_value.to_str().expect("utf8 header"), expected);
    }

    fn taikai_metadata() -> ExtraMetadata {
        ExtraMetadata {
            items: vec![
                MetadataEntry::new(
                    META_TAIKAI_EVENT_ID,
                    b"global-keynote".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    META_TAIKAI_STREAM_ID,
                    b"stage-a".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    META_TAIKAI_RENDITION_ID,
                    b"1080p".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    META_TAIKAI_TRACK_KIND,
                    b"video".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    META_TAIKAI_TRACK_CODEC,
                    b"av1-main".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    META_TAIKAI_TRACK_BITRATE,
                    b"8000".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    META_TAIKAI_TRACK_RESOLUTION,
                    b"1920x1080".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    META_TAIKAI_SEGMENT_SEQUENCE,
                    b"42".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    META_TAIKAI_SEGMENT_START,
                    b"3600000".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    META_TAIKAI_SEGMENT_DURATION,
                    b"2000000".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    META_TAIKAI_WALLCLOCK_MS,
                    b"1702560000000".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    META_TAIKAI_INGEST_LATENCY_MS,
                    b"120".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    META_TAIKAI_INGEST_NODE_ID,
                    b"ingest-node-1".to_vec(),
                    MetadataVisibility::Public,
                ),
            ],
        }
    }

    #[test]
    fn taikai_availability_defaults_without_trm() {
        let metadata = taikai_metadata();
        let availability =
            super::taikai_availability_from_metadata(&metadata, None).expect("derive");
        assert!(availability.is_none());
    }

    #[test]
    fn taikai_availability_uses_trm_payload() {
        let metadata = taikai_metadata();
        let mut manifest = sample_trm_manifest();
        manifest.renditions[0].availability_class = TaikaiAvailabilityClass::Warm;
        let bytes = to_bytes(&manifest).expect("encode trm");
        let availability = super::taikai_availability_from_metadata(&metadata, Some(&bytes))
            .expect("derive")
            .expect("class");
        assert_eq!(availability, TaikaiAvailabilityClass::Warm);
    }

    #[test]
    fn taikai_ingest_tags_include_availability_and_cache_hint() {
        let mut metadata = taikai_metadata();
        let retention = RetentionPolicy {
            hot_retention_secs: 3_600,
            cold_retention_secs: 12 * 60 * 60,
            required_replicas: 4,
            storage_class: StorageClass::Warm,
            governance_tag: GovernanceTag::new("da.taikai.test"),
        };
        let payload_digest = BlobDigest::from_hash(blake3_hash(b"taikai payload bytes"));
        super::apply_taikai_ingest_tags(
            &mut metadata,
            Some(TaikaiAvailabilityClass::Cold),
            &retention,
            payload_digest,
            1024,
        )
        .expect("tagging succeeds");

        fn value_for(metadata: &ExtraMetadata, key: &str) -> String {
            let entry = metadata
                .items
                .iter()
                .find(|entry| entry.key == key)
                .unwrap_or_else(|| panic!("missing metadata entry `{key}`"));
            String::from_utf8(entry.value.clone()).expect("utf8 value")
        }

        assert_eq!(value_for(&metadata, META_TAIKAI_AVAILABILITY_CLASS), "cold");
        assert_eq!(value_for(&metadata, META_DA_PROOF_TIER), "warm");
        assert_eq!(value_for(&metadata, META_TAIKAI_REPLICATION_REPLICAS), "4");
        assert_eq!(
            value_for(&metadata, META_TAIKAI_REPLICATION_STORAGE),
            "warm"
        );
        assert_eq!(
            value_for(&metadata, META_TAIKAI_REPLICATION_HOT_SECS),
            "3600"
        );
        assert_eq!(
            value_for(&metadata, META_TAIKAI_REPLICATION_COLD_SECS),
            "43200"
        );
        assert_eq!(value_for(&metadata, META_DA_PDP_SAMPLE_WINDOW), "32");
        assert_eq!(value_for(&metadata, META_DA_POTR_SAMPLE_WINDOW), "32");

        let cache_hint_entry = metadata
            .items
            .iter()
            .find(|entry| entry.key == META_TAIKAI_CACHE_HINT)
            .expect("cache hint entry");
        let cache_hint: Value = json::from_slice(&cache_hint_entry.value).expect("cache hint json");
        let hint = cache_hint.as_object().expect("cache hint object");
        assert_eq!(
            hint.get("event").and_then(Value::as_str).expect("event id"),
            "global-keynote"
        );
        assert_eq!(
            hint.get("stream")
                .and_then(Value::as_str)
                .expect("stream id"),
            "stage-a"
        );
        assert_eq!(
            hint.get("rendition")
                .and_then(Value::as_str)
                .expect("rendition id"),
            "1080p"
        );
        assert_eq!(
            hint.get("sequence")
                .and_then(Value::as_u64)
                .expect("sequence"),
            42
        );
        assert_eq!(
            hint.get("payload_len")
                .and_then(Value::as_u64)
                .expect("payload_len"),
            1024
        );
        assert_eq!(
            hint.get("payload_blake3_hex")
                .and_then(Value::as_str)
                .expect("digest"),
            hex::encode(payload_digest.as_ref())
        );
    }

    fn taikai_manifest_fixture() -> (DaIngestRequest, ManifestArtifacts) {
        let mut request = sample_request();
        request.metadata = taikai_metadata();

        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let payload_digest = BlobDigest::from_hash(blake3_hash(canonical.as_slice()));

        let mut metadata = request.metadata.clone();
        apply_taikai_ingest_tags(
            &mut metadata,
            Some(TaikaiAvailabilityClass::Hot),
            &request.retention_policy,
            payload_digest,
            request.total_size,
        )
        .expect("tagging succeeds");

        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            0,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");
        (request, manifest)
    }

    #[test]
    fn verify_manifest_rejects_cache_hint_mismatch() {
        let (request, manifest) = taikai_manifest_fixture();
        let mut tampered = manifest.manifest.clone();

        // Replace the cache hint digest with a mismatched value.
        let hint_entry = tampered
            .metadata
            .items
            .iter_mut()
            .find(|entry| entry.key == META_TAIKAI_CACHE_HINT)
            .expect("cache hint entry");
        let mut hint: Value = json::from_slice(&hint_entry.value).expect("decode cache hint");
        if let Value::Object(map) = &mut hint {
            map.insert(
                "payload_blake3_hex".into(),
                Value::from(hex::encode([0xCD; 32])),
            );
        } else {
            panic!("cache hint must be a JSON object");
        }
        hint_entry.value = json::to_vec(&hint).expect("encode cache hint");

        let err = verify_manifest_against_request(
            &request,
            &tampered,
            &request.retention_policy,
            &tampered.metadata,
            &tampered.chunks,
            manifest.blob_hash,
            manifest.chunk_root,
            &manifest.manifest.rent_quote,
        )
        .expect_err("cache hint digest mismatch must be rejected");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn verify_manifest_rejects_missing_proof_tier() {
        let (request, manifest) = taikai_manifest_fixture();
        let mut tampered = manifest.manifest.clone();
        tampered
            .metadata
            .items
            .retain(|entry| entry.key != META_DA_PROOF_TIER);

        let err = verify_manifest_against_request(
            &request,
            &tampered,
            &request.retention_policy,
            &tampered.metadata,
            &tampered.chunks,
            manifest.blob_hash,
            manifest.chunk_root,
            &manifest.manifest.rent_quote,
        )
        .expect_err("missing proof tier must be rejected");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    fn sample_pdp_commitment_for_tests() -> PdpCommitmentV1 {
        PdpCommitmentV1 {
            version: PDP_COMMITMENT_VERSION_V1,
            manifest_digest: [0x11; 32],
            chunk_profile: ChunkingProfileV1 {
                profile_id: ProfileId(0xAB),
                namespace: "inline".to_owned(),
                name: "inline".to_owned(),
                semver: "1.0.0".to_owned(),
                min_size: 64 * 1024,
                target_size: 64 * 1024,
                max_size: 64 * 1024,
                break_mask: 1,
                multihash_code: BLAKE3_256_MULTIHASH_CODE,
                aliases: vec!["inline.inline@1.0.0".to_owned()],
            },
            commitment_root_hot: [0x22; 32],
            commitment_root_segment: [0x33; 32],
            hash_algorithm: HashAlgorithmV1::Blake3_256,
            hot_tree_height: 6,
            segment_tree_height: 4,
            sample_window: 32,
            sealed_at: 1_707_300_000,
        }
    }

    fn encode_alias_proof_bytes(
        alias_namespace: &str,
        alias_name: &str,
        manifest_cid: &[u8],
        bound_epoch: u64,
        expiry_epoch: u64,
        generated_at_unix: u64,
        expires_at_hint: u64,
    ) -> Vec<u8> {
        let binding = AliasBindingV1 {
            alias: format!("{alias_namespace}/{alias_name}"),
            manifest_cid: manifest_cid.to_vec(),
            bound_at: bound_epoch,
            expiry_epoch,
        };
        let mut bundle = AliasProofBundleV1 {
            binding,
            registry_root: [0u8; 32],
            registry_height: 1,
            generated_at_unix,
            expires_at_unix: expires_at_hint.max(generated_at_unix + 1),
            merkle_path: Vec::new(),
            council_signatures: Vec::new(),
        };
        bundle.registry_root = alias_merkle_root(&bundle.binding, &bundle.merkle_path)
            .expect("compute alias proof root");
        let digest = alias_proof_signature_digest(&bundle);
        let council_key =
            PrivateKey::from_bytes(Algorithm::Ed25519, &[0x33; 32]).expect("seeded council key");
        let keypair = KeyPair::from_private_key(council_key).expect("derive council keypair");
        let signature = iroha_crypto::Signature::new(keypair.private_key(), digest.as_ref());
        let (_, signer_bytes) = keypair.public_key().to_bytes();
        let signer: [u8; 32] = signer_bytes.try_into().expect("ed25519 pk length");
        bundle.council_signatures.push(CouncilSignature {
            signer,
            signature: signature.payload().to_vec(),
        });
        to_bytes(&bundle).expect("encode alias proof")
    }

    fn build_ssm_bytes(
        manifest_hash: BlobDigest,
        car_digest: BlobDigest,
        envelope_hash: BlobDigest,
        segment_sequence: u64,
        generated_at_unix: u64,
        expires_at_hint: u64,
    ) -> Vec<u8> {
        let alias_proof = encode_alias_proof_bytes(
            "sora",
            "docs",
            b"cid-placeholder",
            1,
            32,
            generated_at_unix,
            expires_at_hint,
        );
        let alias_binding = ManifestAliasBinding {
            name: "docs".into(),
            namespace: "sora".into(),
            proof: alias_proof,
        };
        let publisher = KeyPair::random();
        let publisher_account = ALICE_ID.clone();
        let body = TaikaiSegmentSigningBodyV1::new(
            1,
            envelope_hash,
            manifest_hash,
            car_digest,
            segment_sequence,
            publisher_account,
            publisher.public_key().clone(),
            generated_at_unix * 1_000,
            alias_binding,
            ExtraMetadata::default(),
        );
        let signature = SignatureOf::new(publisher.private_key(), &body);
        let manifest = TaikaiSegmentSigningManifestV1::new(body, signature);
        to_bytes(&manifest).expect("encode signing manifest")
    }

    fn sample_trm_manifest() -> TaikaiRoutingManifestV1 {
        let event_id = TaikaiEventId::new(Name::from_str("global-keynote").unwrap());
        let stream_id = TaikaiStreamId::new(Name::from_str("stage-a").unwrap());
        let rendition_id = TaikaiRenditionId::new(Name::from_str("1080p").unwrap());
        let route = TaikaiRenditionRouteV1 {
            rendition_id: rendition_id.clone(),
            latest_manifest_hash: BlobDigest::from_hash(blake3_hash(b"manifest")),
            latest_car: TaikaiCarPointer::new(
                "zbafyqra",
                BlobDigest::from_hash(blake3_hash(b"car")),
                131_072,
            ),
            availability_class: TaikaiAvailabilityClass::Hot,
            replication_targets: vec![ProviderId::new([0x22; 32])],
            soranet_circuit: GuardDirectoryId::new("soranet/demo"),
            ssm_range: TaikaiSegmentWindow::new(40, 64),
        };
        TaikaiRoutingManifestV1 {
            version: TaikaiRoutingManifestV1::VERSION,
            event_id,
            stream_id,
            segment_window: TaikaiSegmentWindow::new(40, 64),
            renditions: vec![route],
            alias_binding: TaikaiAliasBinding {
                name: "docs".to_owned(),
                namespace: "sora".to_owned(),
                proof: vec![0xAB, 0xCD],
            },
            guard_policy: TaikaiGuardPolicy::new(
                GuardDirectoryId::new("soranet/demo"),
                1,
                3,
                vec!["lane-a".to_owned()],
            ),
            metadata: ExtraMetadata::default(),
        }
    }

    fn sample_trm_bytes() -> Vec<u8> {
        to_bytes(&sample_trm_manifest()).expect("encode trm")
    }

    fn taikai_envelope_fixture() -> taikai_ingest::EnvelopeArtifacts {
        let mut request = sample_request();
        request.metadata = taikai_metadata();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let metadata =
            encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");
        taikai_ingest::build_envelope(&request, &manifest, &chunk_store, canonical.as_slice())
            .expect("envelope")
    }

    fn sampling_manifest(stripes: u32) -> DaManifestV1 {
        let mut request = sample_request();
        request.chunk_size = 512;
        request.erasure_profile = ErasureProfile {
            data_shards: 4,
            parity_shards: 2,
            row_parity_stripes: 0,
            chunk_alignment: 2,
            fec_scheme: FecScheme::Rs12_10,
        };
        request.total_size = u64::from(request.chunk_size)
            .saturating_mul(u64::from(request.erasure_profile.data_shards))
            .saturating_mul(u64::from(stripes));
        request.payload = vec![0xA5; request.total_size as usize];

        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let rent_policy = DaRentPolicyV1::default();
        resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_800_000,
            &fingerprint,
            &rent_policy,
        )
        .expect("resolve manifest for sampling")
        .manifest
    }

    fn sample_request() -> DaIngestRequest {
        let keypair = KeyPair::random();
        let payload = b"example".to_vec();

        DaIngestRequest {
            client_blob_id: BlobDigest::from_hash(blake3::hash(b"blob-id")),
            lane_id: LaneId::new(1),
            epoch: 5,
            sequence: 7,
            blob_class: BlobClass::TaikaiSegment,
            codec: BlobCodec::new("cmaf"),
            erasure_profile: ErasureProfile {
                data_shards: 8,
                parity_shards: 4,
                row_parity_stripes: 0,
                chunk_alignment: 2,
                fec_scheme: FecScheme::Rs12_10,
            },
            retention_policy: RetentionPolicy {
                hot_retention_secs: 3600,
                cold_retention_secs: 10 * 3600,
                required_replicas: 3,
                storage_class: StorageClass::Hot,
                governance_tag: GovernanceTag::new("baseline"),
            },
            chunk_size: 1 << 10,
            total_size: payload.len() as u64,
            compression: Compression::Identity,
            norito_manifest: None,
            payload,
            metadata: ExtraMetadata {
                items: vec![MetadataEntry::new(
                    "content-type",
                    b"video/cmaf".to_vec(),
                    MetadataVisibility::Public,
                )],
            },
            submitter: keypair.public_key().clone(),
            signature: Signature::from_bytes(&[0u8; 64]),
        }
    }

    fn lane_config_with_scheme(lane_id: LaneId, scheme: DaProofScheme) -> ConfigLaneConfig {
        let metadata = ModelLaneConfig {
            id: lane_id,
            dataspace_id: DataSpaceId::new(u64::from(lane_id.as_u32())),
            alias: format!("lane-{}", lane_id.as_u32()),
            proof_scheme: scheme,
            ..ModelLaneConfig::default()
        };

        let catalog = LaneCatalog::new(
            NonZeroU32::new(lane_id.as_u32().saturating_add(1)).expect("lane count"),
            vec![metadata],
        )
        .expect("lane catalog");
        ConfigLaneConfig::from_catalog(&catalog)
    }

    #[test]
    fn validate_request_accepts_well_formed_payload() {
        let request = sample_request();
        let canonical = normalize_payload(&request).expect("normalize payload");
        assert!(validate_request(&request, canonical.len()).is_ok());
    }

    #[test]
    fn validate_request_rejects_non_power_two_chunks() {
        let mut request = sample_request();
        request.chunk_size = 1_500;
        let canonical = normalize_payload(&request).expect("normalize payload");
        let err = match validate_request(&request, canonical.len()) {
            Ok(_) => panic!("expected validation to reject non power-of-two chunk size"),
            Err(err) => err,
        };
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn compute_fingerprint_uses_payload_and_client_id() {
        let request = sample_request();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let mut other = request.clone();
        other.client_blob_id = BlobDigest::from_hash(blake3::hash(b"different"));
        let other_canonical = normalize_payload(&other).expect("normalize payload");
        assert_ne!(
            compute_fingerprint(&request, canonical.as_slice()),
            compute_fingerprint(&other, other_canonical.as_slice())
        );
    }

    #[test]
    fn compute_fingerprint_prefers_manifest_when_present() {
        let mut request = sample_request();
        let baseline = {
            let canonical = normalize_payload(&request).expect("normalize payload");
            compute_fingerprint(&request, canonical.as_slice())
        };
        request.norito_manifest = Some(vec![1, 2, 3, 4]);
        let canonical = normalize_payload(&request).expect("normalize payload with manifest");
        let manifest = compute_fingerprint(&request, canonical.as_slice());
        assert_ne!(baseline, manifest);
    }

    #[test]
    fn lane_proof_scheme_accepts_kzg_policy() {
        let lane_id = LaneId::new(3);
        let config = lane_config_with_scheme(lane_id, DaProofScheme::KzgBls12_381);

        let scheme = lane_proof_scheme(&config, lane_id).expect("kzg lane should resolve");
        assert_eq!(scheme, DaProofScheme::KzgBls12_381);
    }

    #[test]
    fn taikai_envelope_generation_requires_metadata() {
        let request = sample_request();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let metadata = request.metadata.clone();
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            0,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");

        let err = match taikai_ingest::build_envelope(
            &request,
            &manifest,
            &chunk_store,
            canonical.as_slice(),
        ) {
            Ok(_) => panic!("missing metadata must error"),
            Err(err) => err,
        };
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn taikai_envelope_generation_computes_pointers() {
        let mut request = sample_request();
        request.metadata = taikai_metadata();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let metadata = request.metadata.clone();
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");

        let artifacts =
            taikai_ingest::build_envelope(&request, &manifest, &chunk_store, canonical.as_slice())
                .expect("taikai envelope");

        let mut cursor = std::io::Cursor::new(&artifacts.envelope_bytes);
        let envelope: TaikaiSegmentEnvelopeV1 =
            Decode::decode(&mut cursor).expect("decode envelope");
        assert_eq!(
            envelope.event_id.as_name(),
            &Name::from_str("global-keynote").unwrap()
        );
        assert_eq!(envelope.segment_sequence, 42);
        assert_eq!(
            envelope.ingest.chunk_count,
            chunk_store.chunks().len() as u32
        );
        assert!(envelope.ingest.car.cid_multibase.starts_with('b'));

        let indexes: TaikaiEnvelopeIndexes =
            norito::json::from_slice(&artifacts.indexes_json).expect("decode indexes");
        assert_eq!(indexes.time_key.event_id, envelope.event_id);
        assert_eq!(
            indexes.cid_key.cid_multibase,
            envelope.ingest.car.cid_multibase
        );
    }

    #[test]
    fn taikai_artifacts_persist_idempotent() {
        let dir = tempdir().expect("tempdir");
        let lane_id = LaneId::new(3);
        let epoch = 7;
        let sequence = 11;
        let storage_ticket = StorageTicketId::new([0x11; 32]);
        let fingerprint = ReplayFingerprint::from_hash(blake3::hash(b"fingerprint"));

        let envelope_path = taikai_ingest::persist_envelope(
            dir.path(),
            lane_id,
            epoch,
            sequence,
            &storage_ticket,
            &fingerprint,
            b"envelope",
        )
        .expect("persist envelope")
        .expect("path");
        assert!(envelope_path.exists());

        let index_path = taikai_ingest::persist_indexes(
            dir.path(),
            lane_id,
            epoch,
            sequence,
            &storage_ticket,
            &fingerprint,
            b"indexes",
        )
        .expect("persist indexes")
        .expect("path");
        assert!(index_path.exists());

        let trm_path = taikai_ingest::persist_trm(
            dir.path(),
            lane_id,
            epoch,
            sequence,
            &storage_ticket,
            &fingerprint,
            b"trm",
        )
        .expect("persist trm")
        .expect("path");
        assert!(trm_path.exists());

        let envelope_second = taikai_ingest::persist_envelope(
            dir.path(),
            lane_id,
            epoch,
            sequence,
            &storage_ticket,
            &fingerprint,
            b"other",
        )
        .expect("persist envelope second")
        .expect("path");
        assert_eq!(envelope_path, envelope_second);

        let trm_second = taikai_ingest::persist_trm(
            dir.path(),
            lane_id,
            epoch,
            sequence,
            &storage_ticket,
            &fingerprint,
            b"other-trm",
        )
        .expect("persist trm second")
        .expect("path");
        assert_eq!(trm_path, trm_second);
    }

    #[derive(Default)]
    struct MockAnchorSender {
        calls: AsyncMutex<Vec<(Url, String, Option<String>)>>,
    }

    #[async_trait]
    impl AnchorSender for MockAnchorSender {
        async fn send(
            &self,
            endpoint: &Url,
            body: String,
            api_token: Option<&str>,
        ) -> Result<(), reqwest::Error> {
            self.calls
                .lock()
                .await
                .push((endpoint.clone(), body, api_token.map(str::to_owned)));
            Ok(())
        }
    }

    #[tokio::test]
    async fn taikai_anchor_processing_generates_payload_and_sentinel() {
        let dir = tempdir().expect("tempdir");
        let spool_dir = dir.path().join(TAIKAI_SPOOL_SUBDIR);
        async_fs::create_dir_all(&spool_dir)
            .await
            .expect("create spool");

        let base_id = "00000001-0000000000000002-0000000000000003-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let envelope_path = spool_dir.join(format!("taikai-envelope-{base_id}.norito"));
        async_fs::write(&envelope_path, b"envelope-bytes")
            .await
            .expect("write envelope");

        let indexes = TaikaiEnvelopeIndexes {
            time_key: TaikaiTimeIndexKey {
                event_id: TaikaiEventId::new(Name::from_str("global-keynote").unwrap()),
                stream_id: TaikaiStreamId::new(Name::from_str("stage-a").unwrap()),
                rendition_id: TaikaiRenditionId::new(Name::from_str("1080p").unwrap()),
                segment_start_pts: SegmentTimestamp::new(3_600_000),
            },
            cid_key: TaikaiCidIndexKey {
                event_id: TaikaiEventId::new(Name::from_str("global-keynote").unwrap()),
                stream_id: TaikaiStreamId::new(Name::from_str("stage-a").unwrap()),
                rendition_id: TaikaiRenditionId::new(Name::from_str("1080p").unwrap()),
                cid_multibase: "zbafyqra".to_string(),
            },
        };
        let indexes_json = norito::json::to_json_pretty(&indexes).expect("indexes json");
        let indexes_path = spool_dir.join(format!("taikai-indexes-{base_id}.json"));
        async_fs::write(&indexes_path, indexes_json.as_bytes())
            .await
            .expect("write indexes");

        let ssm_path = spool_dir.join(format!("taikai-ssm-{base_id}.norito"));
        async_fs::write(&ssm_path, b"ssm-bytes")
            .await
            .expect("write ssm");
        let trm_bytes = sample_trm_bytes();
        let trm_path = spool_dir.join(format!("taikai-trm-{base_id}.norito"));
        async_fs::write(&trm_path, &trm_bytes)
            .await
            .expect("write trm");
        let mut lineage_hint = Map::new();
        lineage_hint.insert("version".into(), Value::from(1));
        lineage_hint.insert("alias_namespace".into(), Value::from("sora"));
        lineage_hint.insert("alias_name".into(), Value::from("docs"));
        lineage_hint.insert(
            "previous_manifest_digest_hex".into(),
            Value::from("cafebabe"),
        );
        lineage_hint.insert("previous_window_start_sequence".into(), Value::from(1));
        lineage_hint.insert("previous_window_end_sequence".into(), Value::from(120));
        lineage_hint.insert("previous_updated_unix".into(), Value::from(1_234_567));
        let lineage_value = Value::Object(lineage_hint.clone());
        let lineage_path = spool_dir.join(format!("taikai-lineage-{base_id}.json"));
        async_fs::write(
            &lineage_path,
            json::to_string(&lineage_value)
                .expect("lineage json")
                .as_bytes(),
        )
        .await
        .expect("write lineage hint");

        let anchor_cfg = DaTaikaiAnchor {
            endpoint: Url::parse("http://localhost/anchor").unwrap(),
            api_token: Some("secret-token".to_string()),
            poll_interval: Duration::from_secs(5),
        };

        let pending = collect_pending_uploads(&spool_dir)
            .await
            .expect("collect pending");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].base_id(), base_id);
        let payload: Value = norito::json::from_str(pending[0].body()).expect("payload json");
        assert_eq!(
            payload.get("envelope_base64").and_then(Value::as_str),
            Some(BASE64.encode(b"envelope-bytes")).as_deref()
        );
        assert_eq!(
            payload.get("ssm_base64").and_then(Value::as_str),
            Some(BASE64.encode(b"ssm-bytes")).as_deref()
        );
        assert_eq!(
            payload.get("trm_base64").and_then(Value::as_str),
            Some(BASE64.encode(&trm_bytes)).as_deref()
        );
        assert_eq!(payload.get("lineage_hint"), Some(&lineage_value));

        let sender = MockAnchorSender::default();
        process_batch(&spool_dir, &anchor_cfg, &sender)
            .await
            .expect("process batch");

        let calls = sender.calls.lock().await.clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, anchor_cfg.endpoint);
        assert_eq!(calls[0].2.as_deref(), anchor_cfg.api_token.as_deref());
        assert_eq!(calls[0].1, pending[0].body());

        let sentinel = spool_dir.join(format!(
            "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
        ));
        assert!(async_fs::metadata(&sentinel).await.is_ok());
        let request_capture = spool_dir.join(format!(
            "{TAIKAI_ANCHOR_REQUEST_PREFIX}{base_id}{TAIKAI_ANCHOR_REQUEST_SUFFIX}"
        ));
        let capture_contents = async_fs::read_to_string(&request_capture)
            .await
            .expect("request capture");
        assert_eq!(capture_contents, pending[0].body());

        let pending_after = collect_pending_uploads(&spool_dir)
            .await
            .expect("collect after upload");
        assert!(pending_after.is_empty());
    }

    #[test]
    fn taikai_trm_lineage_guard_rejects_overlapping_windows() {
        let dir = tempdir().expect("tempdir");
        let spool_dir = dir.path();
        let mut manifest = sample_trm_manifest();
        manifest.segment_window = TaikaiSegmentWindow::new(0, 15);
        let alias = manifest.alias_binding.clone();
        {
            let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
                .expect("guard")
                .expect("enabled");
            guard.validate(&manifest, "deadbeef").expect("valid");
            guard
                .commit(manifest.segment_window, "deadbeef")
                .expect("commit");
        }

        let mut overlap = manifest.clone();
        overlap.segment_window = TaikaiSegmentWindow::new(10, 20);
        let guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
            .expect("guard")
            .expect("enabled");
        guard
            .validate(&overlap, "feedbead")
            .expect_err("must reject overlapping manifest windows");
    }

    #[test]
    fn taikai_trm_lineage_hint_contains_previous_digest() {
        let dir = tempdir().expect("tempdir");
        let spool_dir = dir.path();
        let mut manifest = sample_trm_manifest();
        manifest.segment_window = TaikaiSegmentWindow::new(0, 8);
        let alias = manifest.alias_binding.clone();
        let lane_id = LaneId::new(7);
        let epoch = 42;
        let storage_ticket = StorageTicketId::new([0xAA; 32]);
        let fingerprint = ReplayFingerprint::from_hash(blake3_hash(b"lineage-hint"));
        {
            let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
                .expect("guard")
                .expect("enabled");
            guard.validate(&manifest, "aaaa1111").expect("valid");
            guard
                .persist_lineage_hint(lane_id, epoch, 1, &storage_ticket, &fingerprint)
                .expect("persist hint");
            let base_id = format_base_id(lane_id, epoch, 1, &storage_ticket, &fingerprint);
            let hint_path = spool_dir
                .join(TAIKAI_SPOOL_SUBDIR)
                .join(format!("taikai-lineage-{base_id}.json"));
            let contents = fs::read_to_string(&hint_path).expect("lineage hint contents");
            let value: Value = json::from_str(&contents).expect("lineage value");
            assert!(
                value
                    .get("previous_manifest_digest_hex")
                    .is_some_and(Value::is_null)
            );
            guard
                .commit(manifest.segment_window, "aaaa1111")
                .expect("commit");
        }

        let mut next_manifest = manifest.clone();
        next_manifest.segment_window = TaikaiSegmentWindow::new(9, 16);
        {
            let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
                .expect("guard")
                .expect("enabled");
            guard.validate(&next_manifest, "bbbb2222").expect("valid");
            guard
                .persist_lineage_hint(lane_id, epoch, 2, &storage_ticket, &fingerprint)
                .expect("persist hint");
            let base_id = format_base_id(lane_id, epoch, 2, &storage_ticket, &fingerprint);
            let hint_path = spool_dir
                .join(TAIKAI_SPOOL_SUBDIR)
                .join(format!("taikai-lineage-{base_id}.json"));
            let contents = fs::read_to_string(&hint_path).expect("lineage hint contents");
            let value: Value = json::from_str(&contents).expect("lineage value");
            assert_eq!(
                value
                    .get("previous_manifest_digest_hex")
                    .and_then(Value::as_str),
                Some("aaaa1111")
            );
            guard
                .commit(next_manifest.segment_window, "bbbb2222")
                .expect("commit");
        }
    }

    #[test]
    fn take_ssm_entry_returns_payload_and_strips_metadata() {
        let mut metadata = taikai_metadata();
        metadata.items.push(MetadataEntry::new(
            META_TAIKAI_SSM,
            vec![1, 2, 3],
            MetadataVisibility::Public,
        ));
        let payload = taikai_ingest::take_ssm_entry(&mut metadata)
            .expect("extract ssm")
            .expect("payload present");
        assert_eq!(payload, vec![1, 2, 3]);
        assert!(
            metadata
                .items
                .iter()
                .all(|entry| entry.key != META_TAIKAI_SSM)
        );
    }

    #[test]
    fn take_trm_entry_returns_payload_and_strips_metadata() {
        let mut metadata = taikai_metadata();
        metadata.items.push(MetadataEntry::new(
            META_TAIKAI_TRM,
            vec![9, 8, 7],
            MetadataVisibility::Public,
        ));
        let payload = taikai_ingest::take_trm_entry(&mut metadata)
            .expect("extract trm")
            .expect("payload present");
        assert_eq!(payload, vec![9, 8, 7]);
        assert!(
            metadata
                .items
                .iter()
                .all(|entry| entry.key != META_TAIKAI_TRM)
        );
    }

    #[test]
    fn validate_taikai_ssm_accepts_matching_payload() {
        let mut request = sample_request();
        request.metadata = taikai_metadata();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let metadata =
            encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");
        let taikai =
            taikai_ingest::build_envelope(&request, &manifest, &chunk_store, canonical.as_slice())
                .expect("envelope");
        let now_secs = crate::sorafs::unix_now_secs();
        let ssm_bytes = build_ssm_bytes(
            manifest.manifest_hash,
            taikai.car_digest,
            BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
            taikai.telemetry.segment_sequence,
            now_secs,
            now_secs + 600,
        );
        let alias_policy = crate::sorafs::AliasCachePolicy::new(
            Duration::from_secs(600),
            Duration::from_secs(60),
            Duration::from_secs(1_200),
            Duration::from_secs(60),
            Duration::from_secs(120),
            Duration::from_secs(10_000),
            Duration::from_secs(60),
            Duration::from_secs(60),
        );
        let (_, telemetry) = telemetry_handle_for_tests();
        let outcome = validate_taikai_ssm(
            &ssm_bytes,
            &manifest.manifest_hash,
            &taikai.car_digest,
            &taikai.envelope_bytes,
            taikai.telemetry.segment_sequence,
            &alias_policy,
            &telemetry,
        )
        .expect("ssm valid");
        assert_eq!(outcome.alias_label, "sora/docs");
    }

    #[test]
    fn validate_taikai_ssm_rejects_manifest_mismatch() {
        let mut request = sample_request();
        request.metadata = taikai_metadata();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let metadata =
            encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");
        let taikai =
            taikai_ingest::build_envelope(&request, &manifest, &chunk_store, canonical.as_slice())
                .expect("envelope");
        let now_secs = crate::sorafs::unix_now_secs();
        let bad_ssm = build_ssm_bytes(
            BlobDigest::from_hash(blake3_hash(b"other-manifest")),
            taikai.car_digest,
            BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
            taikai.telemetry.segment_sequence,
            now_secs,
            now_secs + 600,
        );
        let alias_policy = crate::sorafs::AliasCachePolicy::new(
            Duration::from_secs(600),
            Duration::from_secs(60),
            Duration::from_secs(1_200),
            Duration::from_secs(60),
            Duration::from_secs(120),
            Duration::from_secs(10_000),
            Duration::from_secs(60),
            Duration::from_secs(60),
        );
        let (_, telemetry) = telemetry_handle_for_tests();
        let err = validate_taikai_ssm(
            &bad_ssm,
            &manifest.manifest_hash,
            &taikai.car_digest,
            &taikai.envelope_bytes,
            taikai.telemetry.segment_sequence,
            &alias_policy,
            &telemetry,
        )
        .expect_err("manifest mismatch must fail");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validate_taikai_ssm_rejects_tampered_signature() {
        let mut request = sample_request();
        request.metadata = taikai_metadata();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let metadata =
            encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");
        let taikai =
            taikai_ingest::build_envelope(&request, &manifest, &chunk_store, canonical.as_slice())
                .expect("envelope");
        let now_secs = crate::sorafs::unix_now_secs();
        let mut ssm_bytes = build_ssm_bytes(
            manifest.manifest_hash,
            taikai.car_digest,
            BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
            taikai.telemetry.segment_sequence,
            now_secs,
            now_secs + 600,
        );
        // Flip a byte in the signature payload to break verification.
        if let Some(last) = ssm_bytes.last_mut() {
            *last ^= 0xFF;
        }
        let alias_policy = crate::sorafs::AliasCachePolicy::new(
            Duration::from_secs(600),
            Duration::from_secs(60),
            Duration::from_secs(1_200),
            Duration::from_secs(60),
            Duration::from_secs(120),
            Duration::from_secs(10_000),
            Duration::from_secs(60),
            Duration::from_secs(60),
        );
        let (_, telemetry) = telemetry_handle_for_tests();
        let err = validate_taikai_ssm(
            &ssm_bytes,
            &manifest.manifest_hash,
            &taikai.car_digest,
            &taikai.envelope_bytes,
            taikai.telemetry.segment_sequence,
            &alias_policy,
            &telemetry,
        )
        .expect_err("tampered signature must fail");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validate_taikai_trm_accepts_matching_manifest() {
        let mut request = sample_request();
        request.metadata = taikai_metadata();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let metadata =
            encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");
        let taikai =
            taikai_ingest::build_envelope(&request, &manifest, &chunk_store, canonical.as_slice())
                .expect("envelope");
        let routing_manifest =
            validate_taikai_trm(&sample_trm_bytes(), &taikai).expect("trm valid");
        assert_eq!(
            routing_manifest.alias_binding.name.as_str(),
            "docs",
            "alias binding should match the stream metadata"
        );
        assert_eq!(
            routing_manifest.segment_window.start_sequence, 40,
            "validated manifest should expose the expected window"
        );
    }

    #[test]
    fn validate_taikai_trm_rejects_mismatched_event() {
        let mut request = sample_request();
        request.metadata = taikai_metadata();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let metadata =
            encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");
        let taikai =
            taikai_ingest::build_envelope(&request, &manifest, &chunk_store, canonical.as_slice())
                .expect("envelope");
        let mut trm = sample_trm_manifest();
        trm.event_id = TaikaiEventId::new(Name::from_str("other-event").unwrap());
        let trm_bytes = to_bytes(&trm).expect("encode trm");
        let err = validate_taikai_trm(&trm_bytes, &taikai).expect_err("validation must fail");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validate_taikai_trm_rejects_invalid_version() {
        let taikai = taikai_envelope_fixture();
        let mut trm = sample_trm_manifest();
        trm.version = TaikaiRoutingManifestV1::VERSION + 1;
        let trm_bytes = to_bytes(&trm).expect("encode trm");
        let err = validate_taikai_trm(&trm_bytes, &taikai).expect_err("validation must fail");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert!(
            err.1.contains("unsupported manifest version"),
            "unexpected error message: {}",
            err.1
        );
    }

    #[test]
    fn validate_taikai_trm_rejects_invalid_window() {
        let taikai = taikai_envelope_fixture();
        let mut trm = sample_trm_manifest();
        trm.segment_window = TaikaiSegmentWindow::new(50, 40);
        let trm_bytes = to_bytes(&trm).expect("encode trm");
        let err = validate_taikai_trm(&trm_bytes, &taikai).expect_err("validation must fail");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert!(
            err.1.contains("invalid routing manifest"),
            "unexpected error message: {}",
            err.1
        );
    }

    #[test]
    fn normalize_payload_handles_gzip() {
        let mut request = sample_request();
        let canonical = request.payload.clone();
        let mut encoder = GzEncoder::new(Vec::new(), FlateCompression::default());
        encoder.write_all(&canonical).expect("write gzip payload");
        let compressed = encoder.finish().expect("finish gzip payload");
        request.payload = compressed;
        request.compression = Compression::Gzip;
        request.total_size = canonical.len() as u64;

        let normalized = normalize_payload(&request).expect("normalize gzip payload");
        assert_eq!(normalized.as_slice(), canonical.as_slice());
    }

    #[test]
    fn normalize_payload_rejects_size_mismatch() {
        let mut request = sample_request();
        let canonical = request.payload.clone();
        let mut encoder = GzEncoder::new(Vec::new(), FlateCompression::default());
        encoder.write_all(&canonical).expect("write gzip payload");
        let compressed = encoder.finish().expect("finish gzip payload");
        request.payload = compressed;
        request.compression = Compression::Gzip;
        request.total_size = (canonical.len() as u64) + 1;

        let err = match normalize_payload(&request) {
            Ok(_) => panic!("expected normalization to reject mismatched size"),
            Err(err) => err,
        };
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn build_receipt_includes_pdp_commitment() {
        let request = sample_request();
        let signer = KeyPair::random();
        let pdp_commitment = sample_pdp_commitment_for_tests();
        let encoded = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
        let rent_quote = DaRentQuote {
            base_rent: XorAmount::from_micro(111),
            protocol_reserve: XorAmount::from_micro(222),
            provider_reward: XorAmount::from_micro(333),
            pdp_bonus: XorAmount::from_micro(444),
            potr_bonus: XorAmount::from_micro(555),
            egress_credit_per_gib: XorAmount::from_micro(666),
        };
        let receipt = build_receipt(
            &signer,
            &request,
            123,
            BlobDigest::from_hash(blake3_hash(b"blob-hash")),
            BlobDigest::from_hash(blake3_hash(b"chunk-root")),
            BlobDigest::from_hash(blake3_hash(b"manifest-hash")),
            StorageTicketId::new([0x44; 32]),
            encoded.clone(),
            rent_quote,
            DaStripeLayout::default(),
        );
        assert_eq!(receipt.pdp_commitment, Some(encoded));
        assert_eq!(receipt.rent_quote, rent_quote);
    }

    #[test]
    fn build_receipt_signs_with_operator_key() {
        let request = sample_request();
        let signer = KeyPair::random();
        let receipt = build_receipt(
            &signer,
            &request,
            999,
            BlobDigest::from_hash(blake3_hash(b"blob-hash")),
            BlobDigest::from_hash(blake3_hash(b"chunk-root")),
            BlobDigest::from_hash(blake3_hash(b"manifest-hash")),
            StorageTicketId::new([0xAA; 32]),
            Vec::new(),
            DaRentQuote::default(),
            DaStripeLayout::default(),
        );
        let mut unsigned = receipt.clone();
        unsigned.operator_signature = Signature::from_bytes(&RECEIPT_SIGNATURE_PLACEHOLDER);
        let unsigned_bytes = to_bytes(&unsigned).expect("encode unsigned receipt");
        receipt
            .operator_signature
            .verify(signer.public_key(), &unsigned_bytes)
            .expect("signature verifies");
    }

    #[test]
    fn build_receipt_computes_chunk_root_from_payload() {
        let request = sample_request();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_000_000,
            &fingerprint,
            &rent_policy,
        )
        .expect("resolve manifest");
        let pdp_commitment = compute_pdp_commitment(
            &manifest.manifest_hash,
            &manifest.manifest,
            &chunk_store,
            1_701_000_000,
        )
        .expect("pdp commitment");
        let encoded_commitment =
            encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
        let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
        let signer = KeyPair::random();
        let receipt = build_receipt(
            &signer,
            &request,
            1_701_000_000,
            manifest.blob_hash,
            manifest.chunk_root,
            manifest.manifest_hash,
            manifest.storage_ticket,
            encoded_commitment,
            manifest.manifest.rent_quote,
            stripe_layout,
        );
        assert_eq!(receipt.chunk_root, manifest.chunk_root);
        assert_eq!(
            manifest.chunk_root,
            BlobDigest::new(*chunk_store.por_tree().root())
        );
    }

    #[test]
    fn build_receipt_prefers_chunk_root_from_manifest() {
        let mut request = sample_request();
        // Seed Taikai metadata so manifest validation passes gateway checks.
        request.metadata = taikai_metadata();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let canonical_bytes = canonical.as_slice().to_vec();
        drop(canonical);
        let payload_hash = BlobDigest::from_hash(blake3_hash(&canonical_bytes));
        let chunk_store = build_chunk_store(&request, canonical_bytes.as_slice());
        apply_taikai_ingest_tags(
            &mut request.metadata,
            None,
            &request.retention_policy,
            payload_hash.clone(),
            request.total_size,
        )
        .expect("apply taikai tags to metadata");
        let manifest_chunk_root = BlobDigest::new(*chunk_store.por_tree().root());
        let chunk_commitments =
            build_chunk_commitments(&request, &chunk_store, canonical_bytes.as_slice())
                .expect("expected chunk commitments");
        let ipa_commitment =
            ipa_commitment_from_chunks(&chunk_commitments).expect("ipa commitment from chunks");
        let total_stripes = (chunk_store.chunks().len() as u32)
            .div_ceil(u32::from(request.erasure_profile.data_shards));
        let shards_per_stripe = u32::from(request.erasure_profile.data_shards)
            .saturating_add(u32::from(request.erasure_profile.parity_shards));
        let total_stripes_full =
            total_stripes.saturating_add(u32::from(request.erasure_profile.row_parity_stripes));
        let rent_policy = DaRentPolicyV1::default();
        let (rent_gib, rent_months) =
            rent_usage_from_request(request.total_size, &request.retention_policy);
        let rent_quote = rent_policy
            .quote(rent_gib, rent_months)
            .expect("compute rent quote for manifest");
        let manifest = DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: request.client_blob_id.clone(),
            lane_id: request.lane_id,
            epoch: request.epoch,
            blob_class: request.blob_class,
            codec: request.codec.clone(),
            blob_hash: payload_hash,
            chunk_root: manifest_chunk_root.clone(),
            storage_ticket: StorageTicketId::new([0x55; 32]),
            total_size: request.total_size,
            chunk_size: request.chunk_size,
            total_stripes: total_stripes_full,
            shards_per_stripe,
            erasure_profile: request.erasure_profile,
            retention_policy: request.retention_policy.clone(),
            rent_quote,
            chunks: chunk_commitments,
            ipa_commitment,
            metadata: request.metadata.clone(),
            issued_at_unix: 42,
        };
        request.norito_manifest = Some(to_bytes(&manifest).expect("encode manifest"));

        let canonical = normalize_payload(&request).expect("normalize payload with manifest");
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_000_123,
            &fingerprint,
            &rent_policy,
        )
        .expect("resolve provided manifest");
        let pdp_commitment = compute_pdp_commitment(
            &manifest.manifest_hash,
            &manifest.manifest,
            &chunk_store,
            1_701_000_123,
        )
        .expect("pdp commitment");
        let encoded_commitment =
            encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
        let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
        let signer = KeyPair::random();
        let receipt = build_receipt(
            &signer,
            &request,
            1_701_000_123,
            manifest.blob_hash,
            manifest.chunk_root,
            manifest.manifest_hash,
            manifest.storage_ticket,
            encoded_commitment,
            manifest.manifest.rent_quote,
            stripe_layout,
        );
        assert_eq!(receipt.chunk_root, manifest_chunk_root);
    }

    #[test]
    fn persist_manifest_for_sorafs_writes_and_is_idempotent() {
        let temp_dir = tempdir().expect("temp dir");
        let manifest_dir = temp_dir.path();
        let request = sample_request();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_000_555,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");

        let first_path = persist_manifest_for_sorafs(
            manifest_dir,
            &manifest.encoded,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &fingerprint,
        )
        .expect("persist manifest")
        .expect("spool path");
        let bytes = fs::read(&first_path).expect("read manifest file");
        assert_eq!(bytes, manifest.encoded);

        let second_path = persist_manifest_for_sorafs(
            manifest_dir,
            &manifest.encoded,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &fingerprint,
        )
        .expect("persist manifest idempotent")
        .expect("spool path");
        assert_eq!(first_path, second_path);
    }

    #[test]
    fn persist_pdp_commitment_writes_and_is_idempotent() {
        let temp_dir = tempdir().expect("temp dir");
        let manifest_dir = temp_dir.path();
        let request = sample_request();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_000_777,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");

        let commitment = compute_pdp_commitment(
            &manifest.manifest_hash,
            &manifest.manifest,
            &chunk_store,
            1_701_000_777,
        )
        .expect("commitment");

        let first_path = persist_pdp_commitment(
            manifest_dir,
            &commitment,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &fingerprint,
        )
        .expect("persist commitment")
        .expect("spool path");
        let bytes = fs::read(&first_path).expect("read commitment file");
        let archived = from_bytes::<PdpCommitmentV1>(&bytes).expect("decode commitment");
        let decoded = PdpCommitmentV1::deserialize(archived);
        assert_eq!(decoded, commitment);

        let second_path = persist_pdp_commitment(
            manifest_dir,
            &commitment,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &fingerprint,
        )
        .expect("persist commitment idempotent")
        .expect("spool path");
        assert_eq!(first_path, second_path);
    }

    #[test]
    fn build_da_commitment_record_reflects_artifacts() {
        let request = sample_request();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_500_000,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");
        let pdp_commitment = sample_pdp_commitment_for_tests();
        let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
        let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
        let receipt = build_receipt(
            &KeyPair::random(),
            &request,
            1_701_500_000,
            manifest.blob_hash,
            manifest.chunk_root,
            manifest.manifest_hash,
            manifest.storage_ticket,
            pdp_bytes.clone(),
            manifest.manifest.rent_quote,
            stripe_layout,
        );
        let record = build_da_commitment_record(
            &request,
            &manifest,
            &request.retention_policy,
            &receipt.operator_signature,
            &pdp_bytes,
            DaProofScheme::MerkleSha256,
        );
        assert_eq!(record.lane_id, request.lane_id);
        assert_eq!(record.epoch, request.epoch);
        assert_eq!(record.sequence, request.sequence);
        assert_eq!(record.client_blob_id, request.client_blob_id);
        assert_eq!(
            record.manifest_hash.as_bytes(),
            manifest.manifest_hash.as_bytes()
        );
        assert_eq!(record.retention_class, request.retention_policy);
        assert_eq!(record.storage_ticket, manifest.storage_ticket);
        assert!(record.proof_digest.is_some(), "expected proof digest");
        assert_eq!(record.proof_scheme, DaProofScheme::MerkleSha256);
        assert!(
            record.kzg_commitment.is_none(),
            "merkle lanes must not include KZG commitments"
        );
    }

    #[test]
    fn build_da_commitment_record_sets_kzg_commitment_for_kzg_lane() {
        let request = sample_request();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_500_000,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");
        let pdp_commitment = sample_pdp_commitment_for_tests();
        let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
        let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
        let receipt = build_receipt(
            &KeyPair::random(),
            &request,
            1_701_500_000,
            manifest.blob_hash,
            manifest.chunk_root,
            manifest.manifest_hash,
            manifest.storage_ticket,
            pdp_bytes.clone(),
            manifest.manifest.rent_quote,
            stripe_layout,
        );
        let record = build_da_commitment_record(
            &request,
            &manifest,
            &request.retention_policy,
            &receipt.operator_signature,
            &pdp_bytes,
            DaProofScheme::KzgBls12_381,
        );
        let expected_kzg = derive_kzg_commitment(&manifest.chunk_root, &manifest.storage_ticket);
        assert_eq!(record.proof_scheme, DaProofScheme::KzgBls12_381);
        assert_eq!(record.kzg_commitment, Some(expected_kzg));
        assert!(record.proof_digest.is_some(), "expected proof digest");
    }

    #[test]
    fn persist_da_commitment_record_writes_and_is_idempotent() {
        let temp_dir = tempdir().expect("temp dir");
        let manifest_dir = temp_dir.path();
        let request = sample_request();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_600_000,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");
        let pdp_commitment = sample_pdp_commitment_for_tests();
        let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
        let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
        let receipt = build_receipt(
            &KeyPair::random(),
            &request,
            1_701_600_000,
            manifest.blob_hash,
            manifest.chunk_root,
            manifest.manifest_hash,
            manifest.storage_ticket,
            pdp_bytes.clone(),
            manifest.manifest.rent_quote,
            stripe_layout,
        );
        let record = build_da_commitment_record(
            &request,
            &manifest,
            &request.retention_policy,
            &receipt.operator_signature,
            &pdp_bytes,
            DaProofScheme::MerkleSha256,
        );
        let first_path = persist_da_commitment_record(
            manifest_dir,
            &record,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &fingerprint,
        )
        .expect("persist record")
        .expect("spool path");
        let bytes = fs::read(&first_path).expect("read record file");
        let archived = from_bytes::<DaCommitmentRecord>(&bytes).expect("decode record");
        let decoded = DaCommitmentRecord::deserialize(archived);
        assert_eq!(decoded, record);

        let second_path = persist_da_commitment_record(
            manifest_dir,
            &record,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &fingerprint,
        )
        .expect("persist record idempotent")
        .expect("spool path");
        assert_eq!(first_path, second_path);
    }

    #[test]
    fn persist_da_commitment_schedule_entry_writes_bundle() {
        let temp_dir = tempdir().expect("temp dir");
        let manifest_dir = temp_dir.path();
        let request = sample_request();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_600_000,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");
        let pdp_commitment = sample_pdp_commitment_for_tests();
        let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
        let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
        let receipt = build_receipt(
            &KeyPair::random(),
            &request,
            1_701_600_000,
            manifest.blob_hash,
            manifest.chunk_root,
            manifest.manifest_hash,
            manifest.storage_ticket,
            pdp_bytes.clone(),
            manifest.manifest.rent_quote,
            stripe_layout,
        );
        let record = build_da_commitment_record(
            &request,
            &manifest,
            &request.retention_policy,
            &receipt.operator_signature,
            &pdp_bytes,
            DaProofScheme::MerkleSha256,
        );
        let schedule_path = persist_da_commitment_schedule_entry(
            manifest_dir,
            &record,
            &pdp_bytes,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &fingerprint,
        )
        .expect("persist schedule entry")
        .expect("schedule path");
        let bytes = fs::read(&schedule_path).expect("read schedule entry");
        let archived =
            from_bytes::<DaCommitmentScheduleEntry>(&bytes).expect("decode schedule entry");
        let decoded = DaCommitmentScheduleEntry::deserialize(archived);
        assert_eq!(decoded.record, record);
        assert_eq!(decoded.pdp_commitment, pdp_bytes);
    }

    #[test]
    fn persist_da_pin_intent_writes_file() {
        let temp_dir = tempdir().expect("temp dir");
        let manifest_dir = temp_dir.path();
        let mut request = sample_request();
        request.sequence = 42;
        request.metadata.items.push(MetadataEntry::new(
            META_DA_REGISTRY_ALIAS,
            b"sora/docs".to_vec(),
            MetadataVisibility::Public,
        ));
        request.metadata.items.push(MetadataEntry::new(
            META_DA_REGISTRY_OWNER,
            ALICE_ID.to_string().into_bytes(),
            MetadataVisibility::Public,
        ));
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_700_123,
            &fingerprint,
            &rent_policy,
        )
        .expect("manifest");
        let alias =
            registry_alias_from_metadata(&request.metadata).expect("alias metadata should parse");
        let owner =
            registry_owner_from_metadata(&request.metadata).expect("owner metadata should parse");
        let mut intent = DaPinIntent::new(
            request.lane_id,
            request.epoch,
            request.sequence,
            manifest.storage_ticket,
            ManifestDigest::new(*manifest.manifest_hash.as_bytes()),
        );
        intent.alias = alias;
        intent.owner = owner;
        let path = persist_da_pin_intent(
            manifest_dir,
            &intent,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &fingerprint,
        )
        .expect("persist pin")
        .expect("path");
        let bytes = fs::read(&path).expect("read pin intent");
        let archived = from_bytes::<DaPinIntent>(&bytes).expect("decode pin intent");
        let decoded: DaPinIntent =
            NoritoDeserialize::try_deserialize(archived).expect("deserialize pin intent");
        assert_eq!(decoded, intent);
        assert_eq!(decoded.alias, Some("sora/docs".to_owned()));
        assert_eq!(decoded.owner, Some(ALICE_ID.clone()));
    }

    fn test_receipt(signer: &KeyPair, lane_id: LaneId, epoch: u64, seed: u8) -> DaIngestReceipt {
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
        let unsigned = unsigned_receipt_bytes(&receipt).expect("test receipt encodes");
        receipt.operator_signature = Signature::new(signer.private_key(), &unsigned);
        receipt
    }

    fn test_fingerprint(seed: u8) -> ReplayFingerprint {
        ReplayFingerprint::from_hash_bytes(&[seed; blake3::OUT_LEN])
    }

    #[test]
    fn persist_da_receipt_writes_and_is_idempotent() {
        let temp_dir = tempdir().expect("temp dir");
        let manifest_dir = temp_dir.path();
        let signer = KeyPair::random();
        let lane_id = LaneId::new(3);
        let receipt = test_receipt(&signer, lane_id, 5, 0xAA);
        let fingerprint = test_fingerprint(0xCC);

        let first_path =
            persist_da_receipt(manifest_dir, &receipt, 7, &fingerprint).expect("persist receipt");
        let first_path = first_path.expect("receipt path");
        let bytes = fs::read(&first_path).expect("read receipt file");
        let decoded = decode_from_bytes::<StoredDaReceipt>(&bytes).expect("decode stored receipt");
        assert_eq!(decoded.version, STORED_RECEIPT_VERSION);
        assert_eq!(decoded.sequence, 7);
        assert_eq!(decoded.receipt.manifest_hash, receipt.manifest_hash);
        let loaded = load_da_receipts(manifest_dir).expect("load receipts");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].sequence, 7);
        assert_eq!(loaded[0].receipt.manifest_hash, receipt.manifest_hash);

        let second_path =
            persist_da_receipt(manifest_dir, &receipt, 7, &fingerprint).expect("persist again");
        let second_path = second_path.expect("receipt path");
        assert_eq!(first_path, second_path);
    }

    #[test]
    fn load_da_receipts_skips_unsupported_versions() {
        let temp_dir = tempdir().expect("temp dir");
        let manifest_dir = temp_dir.path();
        let signer = KeyPair::random();
        let lane_id = LaneId::new(3);
        let receipt = test_receipt(&signer, lane_id, 5, 0xAB);
        let stored = StoredDaReceipt {
            version: STORED_RECEIPT_VERSION + 1,
            sequence: 7,
            receipt,
        };
        let bytes = to_bytes(&stored).expect("encode receipt");
        let path =
            manifest_dir.join("da-receipt-00000003-0000000000000005-0000000000000007-bad.norito");
        fs::write(&path, bytes).expect("write receipt");

        let loaded = load_da_receipts(manifest_dir).expect("load receipts");
        assert!(loaded.is_empty());
    }

    #[test]
    fn da_receipt_log_enforces_ordering_and_dedupe() {
        let temp_dir = tempdir().expect("temp dir");
        let lane_epoch = LaneEpoch::new(LaneId::new(4), 9);
        let cursor_store = Arc::new(ReplayCursorStore::in_memory());
        let signer = KeyPair::random();
        let log = DaReceiptLog::open(
            temp_dir.path().to_path_buf(),
            Arc::clone(&cursor_store),
            signer.public_key().clone(),
        )
        .unwrap();

        let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1);
        assert!(matches!(
            log.append(lane_epoch, 1, receipt.clone(), test_fingerprint(1))
                .unwrap(),
            ReceiptInsertOutcome::Stored { .. }
        ));

        assert!(matches!(
            log.append(lane_epoch, 1, receipt.clone(), test_fingerprint(1))
                .unwrap(),
            ReceiptInsertOutcome::Duplicate { .. }
        ));

        let conflict = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 2);
        assert!(matches!(
            log.append(lane_epoch, 1, conflict, test_fingerprint(2))
                .unwrap(),
            ReceiptInsertOutcome::ManifestConflict { .. }
        ));

        let stale = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 3);
        assert!(matches!(
            log.append(lane_epoch, 0, stale, test_fingerprint(3))
                .unwrap(),
            ReceiptInsertOutcome::StaleSequence { highest: 1 }
        ));
    }

    #[test]
    fn da_receipt_log_rejects_invalid_signature() {
        let temp_dir = tempdir().expect("temp dir");
        let lane_epoch = LaneEpoch::new(LaneId::new(5), 7);
        let cursor_store = Arc::new(ReplayCursorStore::in_memory());
        let signer = KeyPair::random();
        let log = DaReceiptLog::open(
            temp_dir.path().to_path_buf(),
            Arc::clone(&cursor_store),
            signer.public_key().clone(),
        )
        .expect("open log");

        let mut receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 4);
        let unsigned = unsigned_receipt_bytes(&receipt).expect("unsigned bytes");
        let wrong_signer = KeyPair::random();
        receipt.operator_signature = Signature::new(wrong_signer.private_key(), &unsigned);

        let outcome = log.append(lane_epoch, 1, receipt, test_fingerprint(4));
        assert!(
            outcome.is_err(),
            "receipt with mismatched signature must be rejected"
        );
    }

    #[test]
    fn da_receipt_log_reloads_from_disk() {
        let temp_dir = tempdir().expect("temp dir");
        let lane_epoch = LaneEpoch::new(LaneId::new(5), 11);
        let cursor_store = Arc::new(ReplayCursorStore::in_memory());
        let signer = KeyPair::random();
        {
            let log = DaReceiptLog::open(
                temp_dir.path().to_path_buf(),
                Arc::clone(&cursor_store),
                signer.public_key().clone(),
            )
            .unwrap();
            let first = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 9);
            let second = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 10);
            log.append(lane_epoch, 1, first.clone(), test_fingerprint(9))
                .unwrap();
            log.append(lane_epoch, 2, second.clone(), test_fingerprint(10))
                .unwrap();
        }

        let cursor_store = Arc::new(ReplayCursorStore::in_memory());
        let reopened = DaReceiptLog::open(
            temp_dir.path().to_path_buf(),
            Arc::clone(&cursor_store),
            signer.public_key().clone(),
        )
        .unwrap();
        let entries = reopened.receipts_for(lane_epoch);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sequence, 1);
        assert_eq!(entries[1].sequence, 2);
        assert_eq!(
            entries[1].manifest_hash,
            BlobDigest::new([10u8.wrapping_add(3); 32])
        );
        assert!(
            cursor_store
                .highest_sequences()
                .iter()
                .any(|(key, seq)| *key == lane_epoch && *seq == 2),
            "cursor store should be seeded from disk"
        );
    }

    #[test]
    fn da_receipt_log_skips_invalid_entries_on_open() {
        let temp_dir = tempdir().expect("temp dir");
        let bad_path = temp_dir
            .path()
            .join("da-receipt-00000001-0000000000000001-0000000000000001-bad.norito");
        fs::write(&bad_path, b"corrupt").expect("write corrupt receipt");

        let cursor_store = Arc::new(ReplayCursorStore::in_memory());
        let signer = KeyPair::random();
        let log = DaReceiptLog::open(
            temp_dir.path().to_path_buf(),
            Arc::clone(&cursor_store),
            signer.public_key().clone(),
        )
        .expect("open log");

        let lane_epoch = LaneEpoch::new(LaneId::new(1), 1);
        assert!(log.receipts_for(lane_epoch).is_empty());
    }

    #[test]
    fn replay_cursor_store_persists_sequences() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().to_path_buf();
        let store = ReplayCursorStore::open(path.clone()).expect("open store");
        let lane_epoch = LaneEpoch::new(LaneId::new(2), 9);
        store.record(lane_epoch, 42).expect("record");
        drop(store);

        let reopened = ReplayCursorStore::open(path).expect("reopen store");
        let mut entries = reopened.highest_sequences();
        assert_eq!(entries.len(), 1);
        entries.sort_by_key(|(lane_epoch, _)| lane_epoch.lane_id.as_u32());
        assert_eq!(entries[0], (lane_epoch, 42));
    }

    #[test]
    fn resolve_manifest_emits_parity_chunks() {
        let request = sample_request();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let rent_policy = DaRentPolicyV1::default();
        let artifacts = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_000_111,
            &fingerprint,
            &rent_policy,
        )
        .expect("resolve manifest with parity");

        let expected = build_chunk_commitments(&request, &chunk_store, canonical.as_slice())
            .expect("expected chunk commitments");
        assert_eq!(artifacts.manifest.chunks, expected);

        let parity_chunks: Vec<_> = artifacts
            .manifest
            .chunks
            .iter()
            .filter(|chunk| chunk.parity)
            .collect();
        assert_eq!(
            parity_chunks.len(),
            usize::from(request.erasure_profile.parity_shards)
        );

        for (idx, chunk) in parity_chunks.into_iter().enumerate() {
            let expected_offset = request
                .total_size
                .checked_add(
                    u64::try_from(idx)
                        .expect("parity index fits into u64")
                        .checked_mul(u64::from(request.chunk_size))
                        .expect("offset within test bounds"),
                )
                .expect("parity offset within test bounds");
            assert_eq!(chunk.offset, expected_offset);
            assert_eq!(chunk.length, request.chunk_size);
            assert!(chunk.parity);
        }
    }

    #[test]
    fn resolve_manifest_uses_provided_rent_policy() {
        let request = sample_request();
        let canonical = normalize_payload(&request).expect("normalize payload");
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let fingerprint = compute_fingerprint(&request, canonical.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let rent_policy = DaRentPolicyV1::from_components(750_000, 1_500, 250, 125, 2_000);
        let artifacts = resolve_manifest(
            &request,
            &chunk_store,
            canonical.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_001_000,
            &fingerprint,
            &rent_policy,
        )
        .expect("resolve manifest with custom rent policy");

        let (gib, months) = rent_usage_from_request(request.total_size, &request.retention_policy);
        let expected_quote = rent_policy
            .quote(gib, months)
            .expect("rent quote should compute for test inputs");
        assert_eq!(artifacts.manifest.rent_quote, expected_quote);
    }

    #[test]
    fn resolve_manifest_applies_enforced_retention_policy() {
        let request = sample_request();
        let canonical_bytes = normalize_payload(&request)
            .expect("normalize payload")
            .into_vec();
        let chunk_store = build_chunk_store(&request, canonical_bytes.as_slice());
        let fingerprint = compute_fingerprint(&request, canonical_bytes.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let enforced = RetentionPolicy {
            hot_retention_secs: 99,
            cold_retention_secs: 199,
            required_replicas: 9,
            storage_class: StorageClass::Cold,
            governance_tag: GovernanceTag::new("da.test"),
        };

        let rent_policy = DaRentPolicyV1::default();
        let artifacts = resolve_manifest(
            &request,
            &chunk_store,
            canonical_bytes.as_slice(),
            &metadata,
            &enforced,
            1_701_000_555,
            &fingerprint,
            &rent_policy,
        )
        .expect("resolve manifest with enforced retention");

        assert_eq!(artifacts.manifest.retention_policy, enforced);
    }

    #[test]
    fn sampling_plan_clamps_to_sample_window_and_chunk_count() {
        use std::collections::HashSet;

        let manifest = sampling_manifest(20);
        let block_hash = Hash::new(b"block-assign-clamp");
        let plan = build_sampling_plan(&manifest, &block_hash);
        let expected_window = compute_sample_window(manifest.total_size);
        let expected_len = usize::min(manifest.chunks.len(), expected_window as usize);

        assert_eq!(plan.sample_window, expected_window);
        assert_eq!(plan.samples.len(), expected_len);
        let mut seen = HashSet::new();
        for sample in &plan.samples {
            assert!(
                seen.insert(sample.chunk_index),
                "duplicate sample {} detected",
                sample.chunk_index
            );
        }
    }

    #[test]
    fn sampling_plan_is_deterministic_for_seed() {
        let manifest = sampling_manifest(6);
        let block_hash = Hash::new(b"block-deterministic");
        let first = build_sampling_plan(&manifest, &block_hash);
        let second = build_sampling_plan(&manifest, &block_hash);

        assert_eq!(first.assignment_hash, second.assignment_hash);
        assert_eq!(first.samples, second.samples);
    }

    #[test]
    fn sampling_plan_changes_with_block_hash() {
        let manifest = sampling_manifest(6);
        let first = build_sampling_plan(&manifest, &Hash::new(b"block-a"));
        let second = build_sampling_plan(&manifest, &Hash::new(b"block-b"));

        assert_ne!(first.assignment_hash, second.assignment_hash);
        assert_ne!(first.samples, second.samples);
    }

    #[test]
    fn provided_manifest_must_match_enforced_retention_policy() {
        let mut request = sample_request();
        let canonical_bytes = normalize_payload(&request)
            .expect("normalize payload")
            .into_vec();
        let chunk_store = build_chunk_store(&request, canonical_bytes.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let fingerprint = compute_fingerprint(&request, canonical_bytes.as_slice());
        let rent_policy = DaRentPolicyV1::default();
        let artifacts = resolve_manifest(
            &request,
            &chunk_store,
            canonical_bytes.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_000_600,
            &fingerprint,
            &rent_policy,
        )
        .expect("resolve manifest");
        request.norito_manifest = Some(to_bytes(&artifacts.manifest).expect("encode manifest"));

        let strict_policy = RetentionPolicy {
            hot_retention_secs: request.retention_policy.hot_retention_secs + 1,
            cold_retention_secs: request.retention_policy.cold_retention_secs,
            required_replicas: request.retention_policy.required_replicas,
            storage_class: request.retention_policy.storage_class,
            governance_tag: GovernanceTag::new("da.strict"),
        };
        let err = resolve_manifest(
            &request,
            &chunk_store,
            canonical_bytes.as_slice(),
            &metadata,
            &strict_policy,
            1_701_000_601,
            &fingerprint,
            &rent_policy,
        )
        .expect_err("mismatched retention policy must be rejected");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn provided_manifest_with_wrong_parity_is_rejected() {
        let mut request = sample_request();
        let canonical_bytes = normalize_payload(&request)
            .expect("normalize payload")
            .into_vec();
        let chunk_store = build_chunk_store(&request, canonical_bytes.as_slice());
        let fingerprint = compute_fingerprint(&request, canonical_bytes.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let rent_policy = DaRentPolicyV1::default();
        let artifacts = resolve_manifest(
            &request,
            &chunk_store,
            canonical_bytes.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_000_222,
            &fingerprint,
            &rent_policy,
        )
        .expect("resolve manifest");

        let mut tampered = artifacts.manifest.clone();
        let first_parity = tampered
            .chunks
            .iter_mut()
            .find(|chunk| chunk.parity)
            .expect("expected parity chunk to mutate");
        first_parity.parity = false;

        request.norito_manifest = Some(to_bytes(&tampered).expect("encode tampered manifest"));
        let fingerprint = compute_fingerprint(&request, canonical_bytes.as_slice());
        let err = match resolve_manifest(
            &request,
            &chunk_store,
            canonical_bytes.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_000_333,
            &fingerprint,
            &rent_policy,
        ) {
            Ok(_) => panic!("manifest with mismatched parity flag must be rejected"),
            Err(err) => err,
        };
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn provided_manifest_with_wrong_ipa_commitment_is_rejected() {
        let mut request = sample_request();
        let canonical_bytes = normalize_payload(&request)
            .expect("normalize payload")
            .into_vec();
        let chunk_store = build_chunk_store(&request, canonical_bytes.as_slice());
        let fingerprint = compute_fingerprint(&request, canonical_bytes.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        let rent_policy = DaRentPolicyV1::default();
        let artifacts = resolve_manifest(
            &request,
            &chunk_store,
            canonical_bytes.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_000_920,
            &fingerprint,
            &rent_policy,
        )
        .expect("resolve manifest");

        let mut tampered = artifacts.manifest.clone();
        tampered.ipa_commitment = BlobDigest::new([0xAB; 32]);
        request.norito_manifest = Some(to_bytes(&tampered).expect("encode tampered manifest"));
        let fingerprint = compute_fingerprint(&request, canonical_bytes.as_slice());
        let err = resolve_manifest(
            &request,
            &chunk_store,
            canonical_bytes.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_000_921,
            &fingerprint,
            &rent_policy,
        )
        .expect_err("manifest with mismatched ipa commitment must be rejected");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn governance_metadata_is_encrypted_with_configured_key() {
        let mut request = sample_request();
        let secret = b"confidential-notes".to_vec();
        request.metadata.items.push(MetadataEntry::new(
            "gov-notes",
            secret.clone(),
            MetadataVisibility::GovernanceOnly,
        ));
        let key = [0x11u8; 32];

        let encrypted = encrypt_governance_metadata(&request.metadata, Some(&key), Some("primary"))
            .expect("encryption");
        let entry = encrypted
            .items
            .iter()
            .find(|item| item.key == "gov-notes")
            .expect("entry present");
        assert_eq!(
            entry.encryption,
            MetadataEncryption::chacha20poly1305_with_label(Some("primary"))
        );
        assert_ne!(entry.value, secret);

        let decryptor =
            SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&key).expect("decryptor");
        let plaintext = decryptor
            .decrypt_easy(entry.key.as_bytes(), &entry.value)
            .expect("decrypt");
        assert_eq!(plaintext, secret);
    }

    #[test]
    fn governance_metadata_without_key_is_rejected() {
        let metadata = ExtraMetadata {
            items: vec![MetadataEntry::new(
                "gov-only",
                b"secret".to_vec(),
                MetadataVisibility::GovernanceOnly,
            )],
        };
        let err = match encrypt_governance_metadata(&metadata, None, None) {
            Ok(_) => panic!("expected governance-only metadata to require encryption key"),
            Err(err) => err,
        };
        assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn public_metadata_cannot_declare_encryption() {
        let metadata = ExtraMetadata {
            items: vec![MetadataEntry::with_encryption(
                "public",
                b"plain".to_vec(),
                MetadataVisibility::Public,
                MetadataEncryption::chacha20poly1305_with_label(Some("public")),
            )],
        };
        let err = match encrypt_governance_metadata(&metadata, Some(&[0u8; 32]), Some("primary")) {
            Ok(_) => panic!("expected public metadata to reject encryption hints"),
            Err(err) => err,
        };
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn governance_metadata_rejects_label_mismatch() {
        let key = [0x22u8; 32];
        let encryptor =
            SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&key).expect("encryptor");
        let ciphertext = encryptor
            .encrypt_easy(b"gov-notes".as_ref(), b"payload".as_ref())
            .expect("encrypt payload");
        let metadata = ExtraMetadata {
            items: vec![MetadataEntry::with_encryption(
                "gov-notes",
                ciphertext,
                MetadataVisibility::GovernanceOnly,
                MetadataEncryption::chacha20poly1305_with_label(Some("secondary")),
            )],
        };
        let err = match encrypt_governance_metadata(&metadata, Some(&key), Some("primary")) {
            Ok(_) => panic!("expected label mismatch to be rejected"),
            Err(err) => err,
        };
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn governance_metadata_requires_label_when_expected() {
        let key = [0x33u8; 32];
        let encryptor =
            SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&key).expect("encryptor");
        let ciphertext = encryptor
            .encrypt_easy(b"gov-notes".as_ref(), b"payload".as_ref())
            .expect("encrypt payload");
        let metadata = ExtraMetadata {
            items: vec![MetadataEntry::with_encryption(
                "gov-notes",
                ciphertext,
                MetadataVisibility::GovernanceOnly,
                MetadataEncryption::chacha20poly1305_with_label(None::<String>),
            )],
        };
        let err = match encrypt_governance_metadata(&metadata, Some(&key), Some("primary")) {
            Ok(_) => panic!("expected missing label to be rejected"),
            Err(err) => err,
        };
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn governance_metadata_accepts_matching_label_ciphertext() {
        let key = [0x44u8; 32];
        let encryptor =
            SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&key).expect("encryptor");
        let ciphertext = encryptor
            .encrypt_easy(b"gov-notes".as_ref(), b"payload".as_ref())
            .expect("encrypt payload");
        let metadata = ExtraMetadata {
            items: vec![MetadataEntry::with_encryption(
                "gov-notes",
                ciphertext.clone(),
                MetadataVisibility::GovernanceOnly,
                MetadataEncryption::chacha20poly1305_with_label(Some("primary")),
            )],
        };
        let processed =
            encrypt_governance_metadata(&metadata, Some(&key), Some("primary")).expect("process");
        let entry = processed
            .items
            .iter()
            .find(|item| item.key == "gov-notes")
            .expect("entry");
        assert_eq!(entry.value, ciphertext);
        assert_eq!(
            entry.encryption,
            MetadataEncryption::chacha20poly1305_with_label(Some("primary"))
        );
    }

    #[test]
    fn streaming_chunk_ingest_matches_fixture() {
        let (request, canonical_payload) = sample_request_with_payload();
        let chunk_profile = chunk_profile_for_request(request.chunk_size);
        let plan = CarBuildPlan::single_file_with_profile(&canonical_payload, chunk_profile)
            .expect("plan derivation succeeds");
        let mut streaming_store = ChunkStore::with_profile(chunk_profile);
        let chunk_dir = tempdir().expect("chunk dir");
        let mut payload_cursor: &[u8] = canonical_payload.as_slice();
        let stream_output = streaming_store
            .ingest_plan_stream_to_directory(&plan, &mut payload_cursor, chunk_dir.path())
            .expect("streaming ingest succeeds");
        assert_eq!(
            stream_output.total_bytes, request.total_size,
            "persisted byte count should match total_size"
        );

        let direct_store = build_chunk_store(&request, canonical_payload.as_slice());
        assert_eq!(
            streaming_store.profile(),
            direct_store.profile(),
            "chunk profiles must match"
        );
        assert_eq!(
            streaming_store.payload_digest(),
            direct_store.payload_digest(),
            "payload digests must match"
        );
        assert_eq!(
            streaming_store.payload_len(),
            direct_store.payload_len(),
            "payload lengths must match"
        );
        assert_eq!(
            streaming_store.chunks(),
            direct_store.chunks(),
            "chunk metadata mismatch between streaming/non-streaming ingestion"
        );

        let expected_records = load_chunk_record_fixture("sample_chunk_records.txt");
        assert_eq!(
            stream_output.records.len(),
            expected_records.len(),
            "chunk record count drifted; regenerate fixtures"
        );
        for (actual, expected) in stream_output.records.iter().zip(expected_records.iter()) {
            assert_eq!(actual.file_name, expected.file_name);
            assert_eq!(actual.offset, expected.offset);
            assert_eq!(actual.length, expected.length);
            assert_eq!(hex::encode(actual.digest), expected.digest_hex);
        }
    }

    #[test]
    fn manifest_persistence_matches_fixture() {
        let context = sample_manifest_context_for(BlobClass::TaikaiSegment);
        let spool_dir = tempdir().expect("spool dir");
        let manifest_path = persist_manifest_for_sorafs(
            spool_dir.path(),
            &context.artifacts.encoded,
            context.request.lane_id,
            context.request.epoch,
            context.request.sequence,
            &context.artifacts.storage_ticket,
            &context.fingerprint,
        )
        .expect("persist manifest")
        .expect("spool path");
        let actual_bytes = fs::read(manifest_path).expect("read manifest");
        let expected_bytes = load_manifest_fixture("manifests/taikai_segment/manifest.norito.hex");
        assert_eq!(
            actual_bytes, expected_bytes,
            "DA manifest drifted; rerun regenerate_da_ingest_fixtures"
        );
    }

    #[test]
    fn manifest_fixtures_cover_all_blob_classes() {
        for case in &MANIFEST_FIXTURE_CASES {
            let context = sample_manifest_context_for(case.blob_class);
            let expected_bytes =
                load_manifest_fixture(&format!("manifests/{}/manifest.norito.hex", case.slug));
            assert_eq!(
                context.artifacts.encoded, expected_bytes,
                "manifest fixture hex drifted for {}; rerun regenerate_da_ingest_fixtures",
                case.slug
            );

            let expected_json =
                load_manifest_json_fixture(&format!("manifests/{}/manifest.json", case.slug));
            let actual_json =
                json::to_value(&context.artifacts.manifest).expect("serialize manifest to JSON");
            assert_eq!(
                actual_json, expected_json,
                "manifest JSON fixture drifted for {}; rerun regenerate_da_ingest_fixtures",
                case.slug
            );
        }
    }

    #[test]
    #[ignore = "regenerates DA ingest fixtures on disk"]
    fn regenerate_da_ingest_fixtures() {
        for case in &MANIFEST_FIXTURE_CASES {
            let context = sample_manifest_context_for(case.blob_class);
            write_manifest_fixture_bundle(case, &context).expect("write manifest fixture bundle");
        }
        println!(
            "Regenerated manifest fixtures for {} blob classes under {}/manifests",
            MANIFEST_FIXTURE_CASES.len(),
            fixtures_dir().display()
        );

        let (request, canonical_payload) = sample_request_with_payload();
        let chunk_profile = chunk_profile_for_request(request.chunk_size);
        let plan = CarBuildPlan::single_file_with_profile(&canonical_payload, chunk_profile)
            .expect("plan derivation succeeds");
        let mut streaming_store = ChunkStore::with_profile(chunk_profile);
        let chunk_dir = tempdir().expect("chunk dir");
        let mut payload_cursor: &[u8] = canonical_payload.as_slice();
        let stream_output = streaming_store
            .ingest_plan_stream_to_directory(&plan, &mut payload_cursor, chunk_dir.path())
            .expect("streaming ingest succeeds");
        let metadata =
            encrypt_governance_metadata(&request.metadata, None, None).expect("encrypt metadata");
        let fingerprint = compute_fingerprint(&request, canonical_payload.as_slice());
        let rent_policy = DaRentPolicyV1::default();
        let manifest = resolve_manifest(
            &request,
            &streaming_store,
            canonical_payload.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_000_999,
            &fingerprint,
            &rent_policy,
        )
        .expect("resolve manifest");
        let chunk_fixture_path = fixtures_dir().join("sample_chunk_records.txt");
        write_chunk_record_fixture(
            &chunk_fixture_path,
            &stream_output.records,
            stream_output.total_bytes,
        )
        .expect("write chunk fixture");
        println!(
            "Regenerated chunk fixtures at {} (total bytes = {})",
            chunk_fixture_path.display(),
            stream_output.total_bytes
        );
        println!(
            "Manifest hex for reference (taikai segment): {}",
            hex::encode(&manifest.encoded)
        );
    }

    fn sample_request_with_payload() -> (DaIngestRequest, Vec<u8>) {
        let request = sample_request();
        let canonical_vec = {
            let canonical = normalize_payload(&request).expect("normalize payload");
            canonical.into_vec()
        };
        (request, canonical_vec)
    }

    #[derive(Clone, Copy)]
    struct ManifestFixtureCase {
        slug: &'static str,
        blob_class: BlobClass,
    }

    const MANIFEST_FIXTURE_CASES: [ManifestFixtureCase; 4] = [
        ManifestFixtureCase {
            slug: "taikai_segment",
            blob_class: BlobClass::TaikaiSegment,
        },
        ManifestFixtureCase {
            slug: "nexus_lane_sidecar",
            blob_class: BlobClass::NexusLaneSidecar,
        },
        ManifestFixtureCase {
            slug: "governance_artifact",
            blob_class: BlobClass::GovernanceArtifact,
        },
        ManifestFixtureCase {
            slug: "custom_0042",
            blob_class: BlobClass::Custom(0x0042),
        },
    ];

    const fn manifest_fixture_variant_guard(class: BlobClass) {
        match class {
            BlobClass::TaikaiSegment
            | BlobClass::NexusLaneSidecar
            | BlobClass::GovernanceArtifact
            | BlobClass::Custom(_) => {}
        }
    }

    const _: fn(BlobClass) = manifest_fixture_variant_guard;

    struct ManifestFixtureContext {
        request: DaIngestRequest,
        fingerprint: ReplayFingerprint,
        artifacts: ManifestArtifacts,
    }

    fn sample_manifest_context_for(blob_class: BlobClass) -> ManifestFixtureContext {
        let (mut request, canonical_payload) = sample_request_with_payload();
        request.blob_class = blob_class;
        let chunk_store = build_chunk_store(&request, canonical_payload.as_slice());
        let metadata =
            encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
        let fingerprint = compute_fingerprint(&request, canonical_payload.as_slice());
        let rent_policy = DaRentPolicyV1::default();
        let artifacts = resolve_manifest(
            &request,
            &chunk_store,
            canonical_payload.as_slice(),
            &metadata,
            &request.retention_policy,
            1_701_000_999,
            &fingerprint,
            &rent_policy,
        )
        .expect("resolve manifest");

        ManifestFixtureContext {
            request,
            fingerprint,
            artifacts,
        }
    }
    const METRIC_ASSERT_EPSILON: f64 = 1e-6;

    #[test]
    fn record_taikai_ingest_metrics_updates_histograms() {
        let (metrics, telemetry) = telemetry_handle_for_tests();
        let sample = taikai_ingest::TaikaiTelemetrySample {
            event_id: "event".into(),
            stream_id: "stream-main".into(),
            rendition_id: "1080p".into(),
            segment_sequence: 5,
            wallclock_unix_ms: 1_702_560_000_000,
            ingest_latency_ms: Some(150),
            live_edge_drift_ms: Some(-37),
        };
        record_taikai_ingest_metrics(&telemetry, "cluster-a", &sample);

        let dump = metrics.try_to_string().expect("metrics text");
        let latency_line = find_metric_line(
            &dump,
            "taikai_ingest_segment_latency_ms_sum{cluster=\"cluster-a\"",
        );
        assert!(latency_line.contains(r#"stream="stream-main""#));
        let latency = parse_metric_value(latency_line);
        assert!(
            (latency - 150.0).abs() < METRIC_ASSERT_EPSILON,
            "expected ingest latency sum to equal 150.0, got {latency}"
        );

        let drift_line = find_metric_line(
            &dump,
            "taikai_ingest_live_edge_drift_ms_sum{cluster=\"cluster-a\"",
        );
        assert!(drift_line.contains(r#"stream="stream-main""#));
        let drift = parse_metric_value(drift_line);
        assert!(
            (drift - 37.0).abs() < METRIC_ASSERT_EPSILON,
            "expected live-edge drift sum to equal 37.0, got {drift}"
        );

        let signed_drift_line = find_metric_line(
            &dump,
            "taikai_ingest_live_edge_drift_signed_ms{cluster=\"cluster-a\"",
        );
        assert!(signed_drift_line.contains(r#"stream="stream-main""#));
        let signed_drift = parse_metric_value(signed_drift_line);
        assert!(
            (signed_drift + 37.0).abs() < METRIC_ASSERT_EPSILON,
            "expected signed live-edge drift gauge to equal -37.0, got {signed_drift}"
        );
    }

    #[test]
    fn record_taikai_ingest_error_counts_by_status() {
        let (metrics, telemetry) = telemetry_handle_for_tests();
        record_taikai_ingest_error(
            &telemetry,
            "cluster-a",
            "stream-main",
            StatusCode::BAD_REQUEST,
        );

        let dump = metrics.try_to_string().expect("metrics text");
        let error_line =
            find_metric_line(&dump, "taikai_ingest_errors_total{cluster=\"cluster-a\"");
        assert!(error_line.contains(r#"stream="stream-main""#));
        assert!(error_line.contains(r#"reason="Bad Request""#));
        let errors = parse_metric_value(error_line);
        assert!(
            (errors - 1.0).abs() < METRIC_ASSERT_EPSILON,
            "expected error counter to equal 1.0, got {errors}"
        );
    }

    #[test]
    fn record_taikai_alias_rotation_event_updates_metrics() {
        let (metrics, telemetry) = telemetry_handle_for_tests();
        let manifest = sample_trm_manifest();
        record_taikai_alias_rotation_event(&telemetry, "cluster-a", &manifest, "deadbeef");

        let dump = metrics.try_to_string().expect("metrics text");
        let metric_line = find_metric_line(
            &dump,
            "taikai_trm_alias_rotations_total{alias_name=\"docs\",alias_namespace=\"sora\"",
        );
        assert!(
            metric_line.contains("cluster=\"cluster-a\"")
                && metric_line.contains("event=\"global-keynote\"")
                && metric_line.contains("stream=\"stage-a\""),
            "metric labels should reflect cluster/event/stream"
        );
        let value = parse_metric_value(metric_line);
        assert!(
            (value - 1.0).abs() < METRIC_ASSERT_EPSILON,
            "expected alias rotation counter to increment"
        );

        let snapshots = metrics.taikai_alias_rotation_status();
        assert_eq!(snapshots.len(), 1);
        let snapshot = &snapshots[0];
        assert_eq!(snapshot.cluster, "cluster-a");
        assert_eq!(snapshot.event, "global-keynote");
        assert_eq!(snapshot.stream, "stage-a");
        assert_eq!(snapshot.alias_namespace, "sora");
        assert_eq!(snapshot.alias_name, "docs");
        assert_eq!(snapshot.window_start_sequence, 40);
        assert_eq!(snapshot.window_end_sequence, 64);
        assert_eq!(snapshot.manifest_digest_hex, "deadbeef");
        assert_eq!(snapshot.rotations_total, 1);
        assert!(snapshot.last_updated_unix > 0);
    }

    #[test]
    fn record_da_rent_quote_metrics_accumulates_values() {
        let (metrics, telemetry) = telemetry_handle_for_tests();
        let quote = DaRentQuote {
            base_rent: XorAmount::from_micro(1_000_000),
            protocol_reserve: XorAmount::from_micro(250_000),
            provider_reward: XorAmount::from_micro(750_000),
            pdp_bonus: XorAmount::from_micro(50_000),
            potr_bonus: XorAmount::from_micro(25_000),
            egress_credit_per_gib: XorAmount::from_micro(1_500),
        };

        record_da_rent_quote_metrics(&telemetry, "cluster-a", StorageClass::Warm, 4, 3, &quote);

        let dump = metrics.try_to_string().expect("metrics text");
        let gib_line = find_metric_line(
            &dump,
            "torii_da_rent_gib_months_total{cluster=\"cluster-a\"",
        );
        assert!(gib_line.contains(r#"storage_class="warm""#));
        let gib_months = parse_metric_value(gib_line);
        assert!(
            (gib_months - 12.0).abs() < METRIC_ASSERT_EPSILON,
            "expected 12 GiB-months recorded"
        );

        for (metric, expected) in [
            ("torii_da_rent_base_micro_total", 1_000_000.0),
            ("torii_da_protocol_reserve_micro_total", 250_000.0),
            ("torii_da_provider_reward_micro_total", 750_000.0),
            ("torii_da_pdp_bonus_micro_total", 50_000.0),
            ("torii_da_potr_bonus_micro_total", 25_000.0),
        ] {
            let line = find_metric_line(
                &dump,
                &format!("{metric}{{cluster=\"cluster-a\",storage_class=\"warm\""),
            );
            let value = parse_metric_value(line);
            assert!(
                (value - expected).abs() < METRIC_ASSERT_EPSILON,
                "metric {metric} expected {expected}, got {value}"
            );
        }
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn da_rent_metrics_exposed_via_metrics_handler_snapshot() {
        let (metrics, telemetry) =
            telemetry_handle_for_tests_with_profile(TelemetryProfile::Extended);
        let quote = DaRentQuote {
            base_rent: XorAmount::from_micro(1_000_000),
            protocol_reserve: XorAmount::from_micro(250_000),
            provider_reward: XorAmount::from_micro(750_000),
            pdp_bonus: XorAmount::from_micro(50_000),
            potr_bonus: XorAmount::from_micro(25_000),
            egress_credit_per_gib: XorAmount::from_micro(1_500),
        };

        record_da_rent_quote_metrics(&telemetry, "cluster-a", StorageClass::Warm, 4, 3, &quote);

        let prometheus = crate::handle_metrics(&telemetry, true)
            .await
            .expect("prometheus snapshot");
        let snapshot = da_rent_metric_lines(&prometheus);
        assert_eq!(
            snapshot,
            vec![
                "# HELP torii_da_pdp_bonus_micro_total Aggregate PDP bonuses (micro XOR) quoted by DA ingest grouped by cluster and storage class",
                "# HELP torii_da_potr_bonus_micro_total Aggregate PoTR bonuses (micro XOR) quoted by DA ingest grouped by cluster and storage class",
                "# HELP torii_da_protocol_reserve_micro_total Aggregate protocol reserve (micro XOR) quoted by DA ingest grouped by cluster and storage class",
                "# HELP torii_da_provider_reward_micro_total Aggregate provider rewards (micro XOR) quoted by DA ingest grouped by cluster and storage class",
                "# HELP torii_da_rent_base_micro_total Aggregate base rent (micro XOR) quoted by DA ingest grouped by cluster and storage class",
                "# HELP torii_da_rent_gib_months_total Aggregate GiB-month usage quoted by DA ingest grouped by cluster and storage class",
                "# TYPE torii_da_pdp_bonus_micro_total counter",
                "# TYPE torii_da_potr_bonus_micro_total counter",
                "# TYPE torii_da_protocol_reserve_micro_total counter",
                "# TYPE torii_da_provider_reward_micro_total counter",
                "# TYPE torii_da_rent_base_micro_total counter",
                "# TYPE torii_da_rent_gib_months_total counter",
                "torii_da_pdp_bonus_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 50000",
                "torii_da_potr_bonus_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 25000",
                "torii_da_protocol_reserve_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 250000",
                "torii_da_provider_reward_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 750000",
                "torii_da_rent_base_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 1000000",
                "torii_da_rent_gib_months_total{cluster=\"cluster-a\",storage_class=\"warm\"} 12"
            ],
            "DA rent Prometheus payload drifted"
        );

        let dump = metrics.try_to_string().expect("metrics text");
        for line in snapshot {
            assert!(
                dump.contains(&line),
                "metrics text missing `{line}`\n{dump}"
            );
        }
    }

    #[test]
    fn record_da_receipt_metrics_tracks_outcomes_and_cursor() {
        let (metrics, telemetry) = telemetry_handle_for_tests();
        let lane_epoch = LaneEpoch::new(LaneId::new(7), 3);

        record_da_receipt_metrics(
            &telemetry,
            lane_epoch,
            5,
            &ReceiptInsertOutcome::Stored {
                cursor_advanced: true,
            },
        );
        record_da_receipt_metrics(
            &telemetry,
            lane_epoch,
            5,
            &ReceiptInsertOutcome::Duplicate {
                path: std::path::PathBuf::new(),
            },
        );

        let stored = metrics
            .torii_da_receipts_total
            .with_label_values(&["stored", "7", "3"])
            .get();
        assert_eq!(stored, 1, "stored counter should increment");

        let duplicate = metrics
            .torii_da_receipts_total
            .with_label_values(&["duplicate", "7", "3"])
            .get();
        assert_eq!(duplicate, 1, "duplicate counter should increment");

        let cursor = metrics
            .torii_da_receipt_highest_sequence
            .with_label_values(&["7", "3"])
            .get();
        assert_eq!(cursor, 5, "cursor gauge should reflect stored sequence");
    }

    fn telemetry_handle_for_tests_with_profile(
        profile: TelemetryProfile,
    ) -> (Arc<Metrics>, MaybeTelemetry) {
        let metrics = test_metrics();
        let telemetry = Telemetry::new(metrics.clone(), true);
        let handle = MaybeTelemetry::from_profile(Some(telemetry), profile);
        (metrics, handle)
    }

    fn telemetry_handle_for_tests() -> (Arc<Metrics>, MaybeTelemetry) {
        telemetry_handle_for_tests_with_profile(TelemetryProfile::Operator)
    }

    fn test_metrics() -> Arc<Metrics> {
        enable_duplicate_metric_panic();
        Arc::new(Metrics::default())
    }

    fn enable_duplicate_metric_panic() {
        static INIT: LazyLock<()> = LazyLock::new(|| {
            #[allow(unsafe_code)]
            unsafe {
                std::env::set_var("IROHA_METRICS_PANIC_ON_DUPLICATE", "1");
            }
        });
        LazyLock::force(&INIT);
    }

    fn find_metric_line<'a>(dump: &'a str, prefix: &str) -> &'a str {
        dump.lines()
            .find(|line| line.starts_with(prefix))
            .unwrap_or_else(|| panic!("metric `{prefix}` not found\n{dump}"))
    }

    fn da_rent_metric_lines(dump: &str) -> Vec<String> {
        let mut lines: Vec<String> = dump
            .lines()
            .filter(|line| {
                line.starts_with("# HELP torii_da_")
                    || line.starts_with("# TYPE torii_da_")
                    || line.starts_with("torii_da_")
            })
            .filter(|line| {
                line.contains("_rent_")
                    || line.contains("protocol_reserve_micro_total")
                    || line.contains("provider_reward_micro_total")
                    || line.contains("_pdp_bonus_micro_total")
                    || line.contains("_potr_bonus_micro_total")
            })
            .map(str::to_owned)
            .collect();
        lines.sort();
        lines
    }

    fn parse_metric_value(line: &str) -> f64 {
        line.split_whitespace()
            .last()
            .unwrap_or_default()
            .parse::<f64>()
            .expect("metric value")
    }

    struct ChunkRecordFixture {
        file_name: String,
        offset: u64,
        length: u32,
        digest_hex: String,
    }

    fn load_chunk_record_fixture(name: &str) -> Vec<ChunkRecordFixture> {
        let path = fixtures_dir().join(name);
        let contents = fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!("failed to read chunk fixture {}: {err}", path.display());
        });
        contents
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    return None;
                }
                let mut parts = line.split_whitespace();
                let file_name = parts.next()?.to_string();
                let offset = parts
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or_else(|| panic!("missing offset in fixture line `{line}`"));
                let length = parts
                    .next()
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or_else(|| panic!("missing length in fixture line `{line}`"));
                let digest_hex = parts
                    .next()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| panic!("missing digest in fixture line `{line}`"));
                Some(ChunkRecordFixture {
                    file_name,
                    offset,
                    length,
                    digest_hex,
                })
            })
            .collect()
    }

    fn load_manifest_fixture(name: &str) -> Vec<u8> {
        let path = fixtures_dir().join(name);
        let contents = fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!("failed to read manifest fixture {}: {err}", path.display());
        });
        hex::decode(contents.trim()).expect("fixture must be valid hex")
    }

    fn load_manifest_json_fixture(name: &str) -> Value {
        let path = fixtures_dir().join(name);
        let contents = fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!(
                "failed to read manifest JSON fixture {}: {err}",
                path.display()
            );
        });
        json::from_str(&contents).expect("fixture must be valid Norito JSON")
    }

    fn write_manifest_fixture_bundle(
        case: &ManifestFixtureCase,
        context: &ManifestFixtureContext,
    ) -> std::io::Result<()> {
        let manifest_dir = fixtures_dir().join("manifests").join(case.slug);
        fs::create_dir_all(&manifest_dir)?;
        let hex_path = manifest_dir.join("manifest.norito.hex");
        let hex_text = format!("{}\n", hex::encode(&context.artifacts.encoded));
        fs::write(hex_path, hex_text)?;
        let manifest_value =
            json::to_value(&context.artifacts.manifest).expect("serialize manifest as JSON value");
        let json_text =
            json::to_string_pretty(&manifest_value).expect("render manifest JSON fixture");
        fs::write(manifest_dir.join("manifest.json"), format!("{json_text}\n"))?;
        Ok(())
    }

    fn write_chunk_record_fixture(
        path: &Path,
        records: &[PersistedChunkRecord],
        total_bytes: u64,
    ) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(path)?;
        writeln!(file, "# file_name offset length digest_hex")?;
        for record in records {
            writeln!(
                file,
                "{} {} {} {}",
                record.file_name,
                record.offset,
                record.length,
                hex::encode(record.digest)
            )?;
        }
        writeln!(file, "# total_bytes {total_bytes}")?;
        Ok(())
    }

    fn format_base_id(
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        ticket: &StorageTicketId,
        fingerprint: &ReplayFingerprint,
    ) -> String {
        let lane_hex = format!("{:08x}", lane_id.as_u32());
        let epoch_hex = format!("{:016x}", epoch);
        let sequence_hex = format!("{:016x}", sequence);
        let ticket_hex = hex::encode(ticket.as_ref());
        let fingerprint_hex = hex::encode(fingerprint.as_bytes());
        format!("{lane_hex}-{epoch_hex}-{sequence_hex}-{ticket_hex}-{fingerprint_hex}")
    }

    fn fixtures_dir() -> PathBuf {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/da/ingest");
        base.canonicalize()
            .expect("fixtures/da/ingest directory must exist")
    }
}
const DA_COMMITMENT_SCHEDULE_ENTRY_VERSION: u16 = 1;
