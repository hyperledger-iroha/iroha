//! Disk-backed snapshot of `RBC` session summaries for operator endpoints.
//! Not consensus-critical. Each Sumeragi instance registers its own handle
//! so concurrent actors do not trample one another.

use core::sync::atomic::{AtomicU64, Ordering};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::{BlockHeader, consensus::RbcEncoding};
use iroha_logger::prelude::*;
use norito::codec::{Decode, Encode};
use norito::{decode_from_bytes, to_bytes};

use super::status::{DataspaceRbcSnapshot, LaneRbcSnapshot};
use crate::panic_hook;

/// Active store used by `Torii` endpoints and other global queries.
static ACTIVE_STORE: OnceLock<Mutex<Option<Arc<Store>>>> = OnceLock::new();

fn active_slot() -> &'static Mutex<Option<Arc<Store>>> {
    ACTIVE_STORE.get_or_init(|| Mutex::new(None))
}

fn lock_or_recover<'a, T>(mutex: &'a Mutex<T>, name: &'static str) -> MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn!(
                lock = name,
                "recovering poisoned RBC status mutex; preserving in-memory operator state"
            );
            poisoned.into_inner()
        }
    }
}

#[derive(Default)]
struct Inner {
    map: BTreeMap<(HashOf<BlockHeader>, u64, u64), Entry>,
    disk: Option<DiskPersistenceState>,
    persistence_unavailable: bool,
}

#[derive(Clone)]
struct Entry {
    summary: Summary,
    updated_at: SystemTime,
}

struct Store {
    inner: Mutex<Inner>,
    active_count: AtomicU64,
}

struct DiskPersistenceState {
    store: DiskStore,
    disabled: bool,
    disable_logged: bool,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            active_count: AtomicU64::new(0),
        }
    }
}

impl Store {
    fn lock_inner(&self) -> MutexGuard<'_, Inner> {
        lock_or_recover(&self.inner, "rbc_status_store")
    }

    fn snapshot(&self) -> Vec<Summary> {
        let inner = self.lock_inner();
        inner
            .map
            .values()
            .map(|entry| entry.summary.clone())
            .collect()
    }

    fn sessions_active(&self) -> u64 {
        self.active_count.load(Ordering::Relaxed)
    }
}

impl DiskPersistenceState {
    fn new(store: DiskStore) -> Self {
        Self {
            store,
            disabled: false,
            disable_logged: false,
        }
    }
}

/// Handle bound to a single Sumeragi instance.
#[derive(Clone, Default)]
pub struct Handle {
    store: Arc<Store>,
}

impl Handle {
    /// Create a fresh handle with in-memory state.
    pub fn new() -> Self {
        Self {
            store: Arc::new(Store::default()),
        }
    }

    /// Configure the disk-backed snapshot for this handle.
    /// Passing `None` disables persistence and clears existing state.
    pub fn configure(&self, config: Option<StoreConfig>) {
        let mut inner = self.store.lock_inner();
        inner.map.clear();
        match config {
            Some(cfg) => match DiskStore::new(&cfg) {
                Ok(disk) => {
                    load_into_map(&disk, &mut inner.map);
                    inner.disk = Some(DiskPersistenceState::new(disk));
                    inner.persistence_unavailable = false;
                    set_persistence_disabled_metric(false);
                    persist_if_needed(&mut inner, "configure");
                }
                Err(err) => {
                    warn!(
                        ?err,
                        "failed to initialise RBC session store; persistence unavailable"
                    );
                    inner.disk = None;
                    inner.persistence_unavailable = true;
                    set_persistence_disabled_metric(true);
                }
            },
            None => {
                inner.disk = None;
                inner.persistence_unavailable = false;
                set_persistence_disabled_metric(false);
            }
        }
        self.store
            .active_count
            .store(inner.map.len() as u64, Ordering::Relaxed);
    }

    /// Update or insert a session summary.
    pub fn update(&self, summary: Summary, updated_at: SystemTime) {
        let mut inner = self.store.lock_inner();
        let key = (summary.block_hash, summary.height, summary.view);
        if !session_summary_chunk_shape_valid(&summary)
            || summary_allocation_error(&summary).is_some()
        {
            let removed = inner.map.remove(&key).is_some();
            if removed {
                persist_if_needed(&mut inner, "drop_invalid_update");
            }
            self.store
                .active_count
                .store(inner.map.len() as u64, Ordering::Relaxed);
            return;
        }
        let mut persist_needed = true;
        if let Some(entry) = inner.map.get_mut(&key) {
            if entry.summary == summary {
                entry.updated_at = updated_at;
                persist_needed = false;
            } else {
                entry.summary = summary;
                entry.updated_at = updated_at;
            }
        } else {
            inner.map.insert(
                key,
                Entry {
                    summary,
                    updated_at,
                },
            );
        }
        let disk_config = inner
            .disk
            .as_ref()
            .map(|disk| (disk.store.ttl, disk.store.capacity));
        if let Some((ttl, capacity)) = disk_config {
            let should_persist = persist_needed || ttl > Duration::ZERO || capacity > 0;
            if should_persist {
                enforce_map_limits(&mut inner.map, ttl, capacity);
                persist_if_needed(&mut inner, "update");
            }
        }
        self.store
            .active_count
            .store(inner.map.len() as u64, Ordering::Relaxed);
    }

    /// Fetch the stored summary for `key` if present.
    pub fn get(&self, key: &(HashOf<BlockHeader>, u64, u64)) -> Option<Summary> {
        let inner = self.store.lock_inner();
        inner.map.get(key).map(|entry| entry.summary.clone())
    }

    /// Return session keys whose summaries are older than `ttl`.
    pub(super) fn stale_keys(
        &self,
        ttl: Duration,
        now: SystemTime,
    ) -> Vec<(HashOf<BlockHeader>, u64, u64)> {
        if ttl == Duration::ZERO {
            return Vec::new();
        }
        let inner = self.store.lock_inner();
        inner
            .map
            .iter()
            .filter_map(|(key, entry)| {
                let age = now
                    .duration_since(entry.updated_at)
                    .unwrap_or(Duration::ZERO);
                (age > ttl).then_some(*key)
            })
            .collect()
    }

    /// Return the duration until the next session summary becomes stale.
    pub(super) fn next_stale_due(&self, ttl: Duration, now: SystemTime) -> Option<Duration> {
        if ttl == Duration::ZERO {
            return None;
        }
        let inner = self.store.lock_inner();
        let mut next_due: Option<Duration> = None;
        for entry in inner.map.values() {
            let age = now
                .duration_since(entry.updated_at)
                .unwrap_or(Duration::ZERO);
            let remaining = if age >= ttl {
                Duration::ZERO
            } else {
                ttl.saturating_sub(age)
            };
            if remaining == Duration::ZERO {
                return Some(Duration::ZERO);
            }
            next_due = Some(next_due.map_or(remaining, |prev| prev.min(remaining)));
        }
        next_due
    }

    /// Remove a session summary by key.
    pub fn remove(&self, key: &(HashOf<BlockHeader>, u64, u64)) {
        let mut inner = self.store.lock_inner();
        inner.map.remove(key);
        if let Some(disk) = inner.disk.as_ref() {
            let (ttl, capacity) = (disk.store.ttl, disk.store.capacity);
            enforce_map_limits(&mut inner.map, ttl, capacity);
            persist_if_needed(&mut inner, "remove");
        }
        self.store
            .active_count
            .store(inner.map.len() as u64, Ordering::Relaxed);
    }

    /// Clear all session summaries.
    pub fn clear(&self) {
        let mut inner = self.store.lock_inner();
        inner.map.clear();
        persist_if_needed(&mut inner, "clear");
        self.store.active_count.store(0, Ordering::Relaxed);
    }

    /// Snapshot all session summaries for this handle.
    pub fn snapshot(&self) -> Vec<Summary> {
        self.store.snapshot()
    }

    /// Gauge: number of active sessions for this handle.
    pub fn sessions_active(&self) -> u64 {
        self.store.sessions_active()
    }

    #[cfg(test)]
    fn persistence_unavailable_for_tests(&self) -> bool {
        let inner = self.store.lock_inner();
        inner.persistence_unavailable
            || inner
                .disk
                .as_ref()
                .is_some_and(|disk_state| disk_state.disabled)
    }

    /// Check whether a delivered session exists for the given `(block_hash, height)` pair.
    pub fn is_delivered(&self, block_hash: &HashOf<BlockHeader>, height: u64) -> bool {
        let inner = self.store.lock_inner();
        let start = (*block_hash, height, 0);
        let end = (*block_hash, height, u64::MAX);
        inner
            .map
            .range(start..=end)
            .any(|(_, entry)| valid_delivered_summary(&entry.summary))
    }

    /// Check whether a delivered session with a complete chunk set matches the provided payload.
    pub fn delivered_payload_matches(
        &self,
        block_hash: &HashOf<BlockHeader>,
        height: u64,
        payload_hash: &Hash,
    ) -> bool {
        let inner = self.store.lock_inner();
        let start = (*block_hash, height, 0);
        let end = (*block_hash, height, u64::MAX);
        inner.map.range(start..=end).any(|(_, entry)| {
            let summary = &entry.summary;
            summary.delivered
                && !summary.invalid
                && complete_summary_chunk_shape_valid(summary)
                && matches!(summary.payload_hash, Some(hash) if &hash == payload_hash)
        })
    }

    /// Check whether a specific session key has a complete local chunk set that matches the
    /// provided payload, regardless of whether DELIVER has been observed yet.
    pub fn complete_payload_matches(
        &self,
        block_hash: &HashOf<BlockHeader>,
        height: u64,
        view: u64,
        payload_hash: &Hash,
    ) -> bool {
        let inner = self.store.lock_inner();
        inner
            .map
            .get(&(*block_hash, height, view))
            .is_some_and(|entry| {
                let summary = &entry.summary;
                !summary.invalid
                    && complete_summary_chunk_shape_valid(summary)
                    && matches!(summary.payload_hash, Some(hash) if &hash == payload_hash)
            })
    }

    /// Test-only helper that overwrites the in-memory summary for a given
    /// `(block_hash, height, view)` tuple without touching the persisted store.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub fn update_at(
        &self,
        key: (HashOf<BlockHeader>, u64, u64),
        total_chunks: u32,
        received_chunks: u32,
        ready_count: u64,
        delivered: bool,
        payload_hash: Option<Hash>,
        updated_at: SystemTime,
        recovered_from_disk: bool,
    ) {
        let mut inner = self.store.lock_inner();
        let (block_hash, height, view) = key;
        inner.map.insert(
            key,
            Entry {
                summary: Summary {
                    block_hash,
                    height,
                    view,
                    total_chunks,
                    encoding: RbcEncoding::Plain,
                    data_shards: 0,
                    parity_shards: 0,
                    received_chunks,
                    ready_count,
                    delivered,
                    payload_hash,
                    recovered_from_disk,
                    invalid: false,
                    reconstructed_stripes: 0,
                    reconstructable_stripes: 0,
                    lane_backlog: Vec::new(),
                    dataspace_backlog: Vec::new(),
                },
                updated_at,
            },
        );
        persist_if_needed(&mut inner, "update_at");
        self.store
            .active_count
            .store(inner.map.len() as u64, Ordering::Relaxed);
    }
}

/// Register a fresh handle for a Sumeragi instance.
pub fn register_handle() -> Handle {
    Handle::new()
}

/// Mark the supplied handle as active for global snapshot queries.
pub fn set_active(handle: &Handle) {
    *lock_or_recover(active_slot(), "rbc_status_active_slot") = Some(handle.store.clone());
}

fn active_store() -> Option<Arc<Store>> {
    lock_or_recover(active_slot(), "rbc_status_active_slot").clone()
}

/// Compact summary of an RBC session.
///
/// This carries non-consensus operator-facing state about a single RBC
/// session identified by `(block_hash, height, view)`.
#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub struct Summary {
    /// Block hash for which this RBC session is active.
    pub block_hash: HashOf<BlockHeader>,
    /// Block height corresponding to the session.
    pub height: u64,
    /// View (round) index at which the session is observed.
    pub view: u64,
    /// Total number of chunks expected in the RBC payload.
    pub total_chunks: u32,
    /// Payload encoding used by the session.
    pub encoding: RbcEncoding,
    /// Number of RS16 data shards per stripe (`0` for plain sessions).
    pub data_shards: u16,
    /// Number of RS16 parity shards per stripe (`0` for plain sessions).
    pub parity_shards: u16,
    /// Number of chunks received so far.
    pub received_chunks: u32,
    /// Number of READY messages observed (for threshold heuristics).
    pub ready_count: u64,
    /// Whether the session reached DELIVER state.
    pub delivered: bool,
    /// Optional hash of the payload (when available).
    pub payload_hash: Option<Hash>,
    /// True when the session snapshot originated from disk recovery.
    pub recovered_from_disk: bool,
    /// True when the session detected an integrity failure (chunk-root mismatch, etc.).
    pub invalid: bool,
    /// Number of RS16 stripes fully reconstructed from parity.
    pub reconstructed_stripes: u32,
    /// Number of RS16 stripes that are reconstructable with the currently buffered shards.
    pub reconstructable_stripes: u32,
    /// Aggregated per-lane backlog snapshot for this session.
    pub lane_backlog: Vec<LaneRbcSnapshot>,
    /// Aggregated per-dataspace backlog snapshot for this session.
    pub dataspace_backlog: Vec<DataspaceRbcSnapshot>,
}

/// Persistent store configuration for RBC session metadata.
#[derive(Clone)]
pub struct StoreConfig {
    /// Directory where the persisted snapshot should be placed.
    pub dir: PathBuf,
    /// Session TTL after which entries are considered stale.
    pub ttl: Duration,
    /// Maximum number of session summaries retained on disk.
    pub capacity: usize,
}

/// Snapshot the active store (if any).
pub fn snapshot() -> Vec<Summary> {
    active_store().map_or_else(Vec::new, |store| store.snapshot())
}

/// Gauge: number of active sessions in the active store.
pub fn sessions_active() -> u64 {
    active_store().map_or(0, |store| store.sessions_active())
}

/// Read persisted snapshot directly from disk without touching in-memory state.
pub fn read_persisted_snapshot(dir: impl AsRef<Path>) -> Vec<Summary> {
    let _suppressor = panic_hook::ScopedSuppressor::new();
    let file = dir.as_ref().join(FILE_NAME);
    read_entries_with_fallback(&file)
        .into_iter()
        .map(|stored| stored.summary)
        .collect()
}

const FILE_NAME: &str = "sessions.norito";

#[derive(Clone)]
struct DiskStore {
    file: PathBuf,
    ttl: Duration,
    capacity: usize,
    #[cfg(test)]
    fail_persist_with: Option<io::ErrorKind>,
}

#[derive(Clone, Encode, Decode)]
struct StoredEntry {
    summary: Summary,
    updated_at_ms: u64,
}

impl DiskStore {
    fn new(cfg: &StoreConfig) -> std::io::Result<Self> {
        fs::create_dir_all(&cfg.dir)?;
        Ok(Self {
            file: cfg.dir.join(FILE_NAME),
            ttl: cfg.ttl,
            capacity: cfg.capacity,
            #[cfg(test)]
            fail_persist_with: None,
        })
    }

    fn persist(
        &self,
        map: &BTreeMap<(HashOf<BlockHeader>, u64, u64), Entry>,
    ) -> std::io::Result<()> {
        #[cfg(test)]
        if let Some(kind) = self.fail_persist_with {
            return Err(io::Error::from(kind));
        }
        let mut entries: Vec<StoredEntry> = map
            .values()
            .map(|entry| StoredEntry {
                summary: entry.summary.clone(),
                updated_at_ms: system_time_to_ms(entry.updated_at),
            })
            .collect();
        entries.sort_by_key(|stored| stored.updated_at_ms);
        let encoded = to_bytes(&entries).map_err(io::Error::other)?;
        let tmp = temp_store_path(&self.file);
        {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp)?;
            file.write_all(&encoded)?;
            file.sync_all()?;
        }
        if let Err(err) = fs::rename(&tmp, &self.file) {
            if err.kind() == io::ErrorKind::AlreadyExists {
                fs::remove_file(&self.file)?;
                fs::rename(&tmp, &self.file)?;
            } else {
                return Err(err);
            }
        }
        if let Some(parent) = self.file.parent() {
            if !parent.as_os_str().is_empty() {
                sync_dir(parent)?;
            }
        }
        Ok(())
    }
}

fn is_fatal_persist_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::StorageFull
            | io::ErrorKind::WriteZero
            | io::ErrorKind::OutOfMemory
            | io::ErrorKind::FileTooLarge
            | io::ErrorKind::QuotaExceeded
    )
}

fn set_persistence_disabled_metric(disabled: bool) {
    #[cfg(feature = "telemetry")]
    if let Some(metrics) = iroha_telemetry::metrics::global() {
        metrics
            .sumeragi_rbc_status_persistence_disabled
            .set(u64::from(disabled));
    }
    #[cfg(not(feature = "telemetry"))]
    let _ = disabled;
}

fn record_fatal_persist_failure() {
    #[cfg(feature = "telemetry")]
    if let Some(metrics) = iroha_telemetry::metrics::global() {
        metrics.sumeragi_rbc_status_persist_failures_total.inc();
    }
}

fn persist_if_needed(inner: &mut Inner, context: &'static str) {
    let Some(disk_state) = inner.disk.as_ref() else {
        return;
    };
    if disk_state.disabled {
        return;
    }
    let disk = disk_state.store.clone();

    if let Err(err) = disk.persist(&inner.map) {
        if is_fatal_persist_error(&err) {
            if let Some(disk_state) = inner.disk.as_mut() {
                disk_state.disabled = true;
                inner.persistence_unavailable = true;
                if !disk_state.disable_logged {
                    disk_state.disable_logged = true;
                    warn!(
                        ?err,
                        context = context,
                        "fatal RBC status persist error; disabling disk persistence and keeping in-memory status snapshots active"
                    );
                }
            }
            record_fatal_persist_failure();
            set_persistence_disabled_metric(true);
            return;
        }

        warn!(
            ?err,
            context = context,
            "failed to persist RBC session store"
        );
    }
}

fn temp_store_path(path: &Path) -> PathBuf {
    path.with_added_extension("tmp")
}

fn load_into_map(disk: &DiskStore, map: &mut BTreeMap<(HashOf<BlockHeader>, u64, u64), Entry>) {
    let mut entries = read_entries_with_fallback(&disk.file);
    enforce_limits(&mut entries, disk.ttl, disk.capacity);
    for stored in entries {
        let Some(updated_at) = ms_to_system_time(stored.updated_at_ms) else {
            continue;
        };
        let summary = stored.summary;
        let key = (summary.block_hash, summary.height, summary.view);
        map.insert(
            key,
            Entry {
                summary,
                updated_at,
            },
        );
    }
}

fn read_entries_with_fallback(path: &Path) -> Vec<StoredEntry> {
    let tmp_path = temp_store_path(path);
    let tmp_bytes = match read_store_bytes(&tmp_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            warn!(?err, ?tmp_path, "failed to read RBC session temp store");
            None
        }
    };
    let main_bytes = match read_store_bytes(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            warn!(?err, ?path, "failed to read RBC session store");
            None
        }
    };

    if tmp_bytes.is_none() && main_bytes.is_none() {
        return Vec::new();
    }

    let had_tmp = tmp_bytes.is_some();
    let mut selected = None;
    for (candidate_path, is_temp, bytes) in [
        (path, false, main_bytes),
        (tmp_path.as_path(), true, tmp_bytes),
    ] {
        let Some(bytes) = bytes.as_deref() else {
            continue;
        };
        match decode_entries(bytes) {
            Ok(entries) => {
                let decoded_len = entries.len();
                let entries = retain_valid_entries(entries, candidate_path);
                if entries.is_empty() && decoded_len > 0 {
                    let _ = fs::remove_file(candidate_path);
                    continue;
                }
                let newest_updated_at_ms = entries
                    .iter()
                    .map(|entry| entry.updated_at_ms)
                    .max()
                    .unwrap_or(0);
                let candidate = StoreCandidate {
                    entries,
                    newest_updated_at_ms,
                    is_temp,
                };
                if store_candidate_newer_than_selected(&candidate, selected.as_ref()) {
                    selected = Some(candidate);
                }
            }
            Err(err) => {
                if is_temp {
                    warn!(?err, ?tmp_path, "failed to decode RBC session temp store");
                    let _ = fs::remove_file(&tmp_path);
                } else {
                    warn!(?err, ?path, "failed to decode RBC session store");
                    let _ = fs::remove_file(path);
                }
            }
        }
    }

    if let Some(selected) = selected {
        if selected.is_temp {
            warn!(
                path = %tmp_path.display(),
                "recovered RBC session store from temp file"
            );
            promote_temp_store(&tmp_path, path);
        } else if had_tmp {
            let _ = fs::remove_file(&tmp_path);
        }
        return selected.entries;
    }

    Vec::new()
}

struct StoreCandidate {
    entries: Vec<StoredEntry>,
    newest_updated_at_ms: u64,
    is_temp: bool,
}

fn store_candidate_newer_than_selected(
    candidate: &StoreCandidate,
    selected: Option<&StoreCandidate>,
) -> bool {
    let Some(selected) = selected else {
        return true;
    };
    candidate.newest_updated_at_ms > selected.newest_updated_at_ms
        || (candidate.newest_updated_at_ms == selected.newest_updated_at_ms
            && !candidate.is_temp
            && selected.is_temp)
}

fn read_store_bytes(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn decode_entries(buf: &[u8]) -> Result<Vec<StoredEntry>, norito::Error> {
    let _suppressor = panic_hook::ScopedSuppressor::new();
    decode_from_bytes(buf)
}

fn retain_valid_entries(entries: Vec<StoredEntry>, path: &Path) -> Vec<StoredEntry> {
    let now = SystemTime::now();
    entries
        .into_iter()
        .filter(|stored| {
            valid_entry_timestamp(stored, path, now).is_some()
                && valid_persisted_summary(stored, path)
        })
        .collect()
}

fn valid_persisted_summary(stored: &StoredEntry, path: &Path) -> bool {
    let summary = &stored.summary;
    if !session_summary_chunk_shape_valid(summary) {
        warn!(
            ?path,
            block_hash = ?summary.block_hash,
            height = summary.height,
            view = summary.view,
            total_chunks = summary.total_chunks,
            received_chunks = summary.received_chunks,
            "dropping RBC session status with impossible chunk counters"
        );
        return false;
    }
    if !summary.invalid && summary.delivered && !complete_summary_chunk_shape_valid(summary) {
        warn!(
            ?path,
            block_hash = ?summary.block_hash,
            height = summary.height,
            view = summary.view,
            total_chunks = summary.total_chunks,
            received_chunks = summary.received_chunks,
            "dropping delivered RBC session status without a complete chunk set"
        );
        return false;
    }
    if let Some(reason) = summary_allocation_error(summary) {
        warn!(
            ?path,
            block_hash = ?summary.block_hash,
            height = summary.height,
            view = summary.view,
            reason,
            "dropping RBC session status with inconsistent lane/dataspace allocation metadata"
        );
        return false;
    }
    true
}

fn session_summary_chunk_shape_valid(summary: &Summary) -> bool {
    summary.total_chunks > 0 && summary.received_chunks <= summary.total_chunks
}

pub(super) fn summary_allocations_valid(summary: &Summary) -> bool {
    summary_allocation_error(summary).is_none()
}

fn summary_allocation_error(summary: &Summary) -> Option<&'static str> {
    if summary.lane_backlog.is_empty() && summary.dataspace_backlog.is_empty() {
        return None;
    }
    if summary.total_chunks == 0 {
        return Some("allocation metadata with zero chunks");
    }
    if summary.lane_backlog.is_empty() || summary.dataspace_backlog.is_empty() {
        return Some("incomplete allocation metadata");
    }

    let mut lane_totals: BTreeMap<u32, (u64, u64, u64)> = BTreeMap::new();
    let mut lane_chunk_sum = 0u64;
    for lane in &summary.lane_backlog {
        if lane.tx_count == 0 {
            return Some("zero lane allocation transaction count");
        }
        if lane.pending_chunks > lane.total_chunks {
            return Some("lane allocation pending chunks exceed total chunks");
        }
        if lane_totals
            .insert(
                lane.lane_id,
                (lane.tx_count, lane.total_chunks, lane.rbc_bytes_total),
            )
            .is_some()
        {
            return Some("duplicate lane allocation");
        }
        let Some(updated_chunk_sum) = lane_chunk_sum.checked_add(lane.total_chunks) else {
            return Some("lane allocation chunk sum overflow");
        };
        lane_chunk_sum = updated_chunk_sum;
    }
    if lane_chunk_sum != u64::from(summary.total_chunks) {
        return Some("lane allocation chunk sum mismatch");
    }

    let mut dataspace_seen = BTreeSet::new();
    let mut dataspace_sums: BTreeMap<u32, (u64, u64, u64)> = BTreeMap::new();
    for dataspace in &summary.dataspace_backlog {
        if dataspace.tx_count == 0 {
            return Some("zero dataspace allocation transaction count");
        }
        if dataspace.pending_chunks > dataspace.total_chunks {
            return Some("dataspace allocation pending chunks exceed total chunks");
        }
        if !lane_totals.contains_key(&dataspace.lane_id) {
            return Some("dataspace allocation references unknown lane");
        }
        if !dataspace_seen.insert((dataspace.lane_id, dataspace.dataspace_id)) {
            return Some("duplicate dataspace allocation");
        }
        let entry = dataspace_sums.entry(dataspace.lane_id).or_insert((0, 0, 0));
        let Some(tx_count) = entry.0.checked_add(dataspace.tx_count) else {
            return Some("dataspace allocation transaction sum overflow");
        };
        let Some(total_chunks) = entry.1.checked_add(dataspace.total_chunks) else {
            return Some("dataspace allocation chunk sum overflow");
        };
        let Some(rbc_bytes_total) = entry.2.checked_add(dataspace.rbc_bytes_total) else {
            return Some("dataspace allocation byte sum overflow");
        };
        *entry = (tx_count, total_chunks, rbc_bytes_total);
    }

    for (lane_id, expected) in lane_totals {
        if dataspace_sums.get(&lane_id).copied().unwrap_or_default() != expected {
            return Some("dataspace allocation sum mismatch");
        }
    }

    None
}

fn complete_summary_chunk_shape_valid(summary: &Summary) -> bool {
    session_summary_chunk_shape_valid(summary) && summary.received_chunks == summary.total_chunks
}

fn valid_delivered_summary(summary: &Summary) -> bool {
    summary.delivered && !summary.invalid && complete_summary_chunk_shape_valid(summary)
}

fn valid_entry_timestamp(stored: &StoredEntry, path: &Path, now: SystemTime) -> Option<SystemTime> {
    let Some(updated_at) = ms_to_system_time(stored.updated_at_ms) else {
        warn!(
            ?path,
            updated_at_ms = stored.updated_at_ms,
            "dropping RBC session status with unrepresentable timestamp"
        );
        return None;
    };
    if let Err(err) = now.duration_since(updated_at) {
        warn!(
            ?err,
            ?path,
            updated_at_ms = stored.updated_at_ms,
            "dropping RBC session status with future timestamp"
        );
        return None;
    }
    Some(updated_at)
}

fn promote_temp_store(tmp_path: &Path, main_path: &Path) {
    let promoted = match fs::rename(tmp_path, main_path) {
        Ok(()) => true,
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            if let Err(remove_err) = fs::remove_file(main_path) {
                warn!(
                    ?remove_err,
                    ?main_path,
                    "failed to remove RBC session store before temp promotion"
                );
                false
            } else if let Err(rename_err) = fs::rename(tmp_path, main_path) {
                warn!(
                    ?rename_err,
                    ?tmp_path,
                    "failed to promote RBC session temp store after removal"
                );
                false
            } else {
                true
            }
        }
        Err(err) => {
            warn!(?err, ?tmp_path, "failed to promote RBC session temp store");
            false
        }
    };

    if promoted {
        if let Some(parent) = main_path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(err) = sync_dir(parent) {
                    warn!(?err, ?parent, "failed to sync RBC session store directory");
                }
            }
        }
    }
}

fn sync_dir(path: &Path) -> io::Result<()> {
    let file = fs::File::open(path)?;
    file.sync_all()
}

fn enforce_limits(entries: &mut Vec<StoredEntry>, ttl: Duration, capacity: usize) {
    if ttl > Duration::ZERO {
        let now = SystemTime::now();
        entries.retain(|stored| {
            let Some(updated_at) = ms_to_system_time(stored.updated_at_ms) else {
                return false;
            };
            now.duration_since(updated_at).unwrap_or(Duration::ZERO) <= ttl
        });
    }
    if capacity == 0 {
        entries.clear();
    } else if entries.len() > capacity {
        let keep = entries.len() - capacity;
        entries.drain(..keep);
    }
}

fn enforce_map_limits(
    map: &mut BTreeMap<(HashOf<BlockHeader>, u64, u64), Entry>,
    ttl: Duration,
    capacity: usize,
) {
    if map.is_empty() {
        return;
    }

    let mut entries: Vec<StoredEntry> = map
        .values()
        .map(|entry| StoredEntry {
            summary: entry.summary.clone(),
            updated_at_ms: system_time_to_ms(entry.updated_at),
        })
        .collect();
    entries.sort_by_key(|stored| stored.updated_at_ms);
    enforce_limits(&mut entries, ttl, capacity);
    map.clear();
    for stored in entries {
        let key = (
            stored.summary.block_hash,
            stored.summary.height,
            stored.summary.view,
        );
        let Some(updated_at) = ms_to_system_time(stored.updated_at_ms) else {
            continue;
        };
        map.insert(
            key,
            Entry {
                summary: stored.summary,
                updated_at,
            },
        );
    }
}

fn system_time_to_ms(time: SystemTime) -> u64 {
    let duration = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
        .min(u128::from(u64::MAX));
    u64::try_from(duration).unwrap_or(u64::MAX)
}

fn ms_to_system_time(ms: u64) -> Option<SystemTime> {
    UNIX_EPOCH.checked_add(Duration::from_millis(ms))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;
    use norito::to_bytes;
    use tempfile::tempdir;

    use super::*;

    fn hash(byte: u8) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed([byte; 32]))
    }

    fn summary(
        byte: u8,
        height: u64,
        received_chunks: u32,
        ready_count: u64,
        delivered: bool,
        payload: Option<&[u8]>,
    ) -> Summary {
        Summary {
            block_hash: hash(byte),
            height,
            view: 0,
            total_chunks: 4,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks,
            ready_count,
            delivered,
            payload_hash: payload.map(Hash::new),
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        }
    }

    #[test]
    fn temp_store_path_preserves_extensions() {
        let base = Path::new("/var/lib/iroha/rbc/sessions.norito");
        let tmp = temp_store_path(base);
        assert_eq!(tmp, Path::new("/var/lib/iroha/rbc/sessions.norito.tmp"));
    }

    #[test]
    fn persisted_snapshot_promotes_temp_file() {
        let dir = tempdir().expect("tempdir");
        let summary = Summary {
            block_hash: hash(7),
            height: 7,
            view: 0,
            total_chunks: 3,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 1,
            ready_count: 0,
            delivered: false,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        let entry = StoredEntry {
            summary: summary.clone(),
            updated_at_ms: 42,
        };
        let encoded = to_bytes(&vec![entry]).expect("encode RBC status store");
        let file = dir.path().join(FILE_NAME);
        let tmp = temp_store_path(&file);
        fs::write(&tmp, encoded).expect("write temp store");

        let snapshot = read_persisted_snapshot(dir.path());
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].block_hash, summary.block_hash);
        assert!(file.exists(), "temp store should be promoted");
        assert!(!tmp.exists(), "temp store should be removed");
    }

    #[test]
    fn persisted_snapshot_prefers_main_store_over_temp_file() {
        let dir = tempdir().expect("tempdir");
        let main_summary = Summary {
            block_hash: hash(8),
            height: 8,
            view: 0,
            total_chunks: 4,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 4,
            ready_count: 3,
            delivered: true,
            payload_hash: Some(Hash::new(b"main")),
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        let tmp_summary = Summary {
            block_hash: hash(8),
            height: 8,
            view: 0,
            total_chunks: 4,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 2,
            ready_count: 1,
            delivered: false,
            payload_hash: Some(Hash::new(b"tmp")),
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        let file = dir.path().join(FILE_NAME);
        let tmp = temp_store_path(&file);
        let main_encoded = to_bytes(&vec![StoredEntry {
            summary: main_summary.clone(),
            updated_at_ms: 200,
        }])
        .expect("encode main store");
        let tmp_encoded = to_bytes(&vec![StoredEntry {
            summary: tmp_summary,
            updated_at_ms: 100,
        }])
        .expect("encode temp store");
        fs::write(&file, main_encoded).expect("write main store");
        fs::write(&tmp, tmp_encoded).expect("write temp store");

        let snapshot = read_persisted_snapshot(dir.path());
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0], main_summary);
        assert!(
            !tmp.exists(),
            "older temp store should be removed after selecting the newer main store"
        );
    }

    #[test]
    fn persisted_snapshot_promotes_newer_temp_store_over_main_file() {
        let dir = tempdir().expect("tempdir");
        let main_summary = summary(9, 9, 2, 1, false, Some(b"main"));
        let tmp_summary = summary(9, 9, 4, 3, true, Some(b"tmp"));
        let file = dir.path().join(FILE_NAME);
        let tmp = temp_store_path(&file);
        let main_encoded = to_bytes(&vec![StoredEntry {
            summary: main_summary,
            updated_at_ms: 100,
        }])
        .expect("encode main store");
        let tmp_encoded = to_bytes(&vec![StoredEntry {
            summary: tmp_summary.clone(),
            updated_at_ms: 200,
        }])
        .expect("encode temp store");
        fs::write(&file, main_encoded).expect("write main store");
        fs::write(&tmp, tmp_encoded).expect("write temp store");

        let snapshot = read_persisted_snapshot(dir.path());
        assert_eq!(snapshot, vec![tmp_summary]);
        assert!(file.exists(), "newer temp store should be promoted");
        assert!(!tmp.exists(), "promoted temp store should be removed");
        let promoted = decode_entries(&fs::read(&file).expect("read promoted store"))
            .expect("decode promoted store");
        assert_eq!(promoted[0].updated_at_ms, 200);
    }

    #[test]
    fn persisted_snapshot_rejects_future_timestamp_store() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join(FILE_NAME);
        let future_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .saturating_add(120_000);
        let encoded = to_bytes(&vec![StoredEntry {
            summary: summary(10, 10, 1, 0, false, Some(b"future")),
            updated_at_ms: u64::try_from(future_ms).unwrap_or(u64::MAX),
        }])
        .expect("encode future store");
        fs::write(&file, encoded).expect("write future store");

        let snapshot = read_persisted_snapshot(dir.path());
        assert!(
            snapshot.is_empty(),
            "future-dated status snapshots must not be reported"
        );
        assert!(
            !file.exists(),
            "future-dated status stores should be removed when every entry is invalid"
        );
    }

    #[test]
    fn persisted_snapshot_rejects_max_timestamp_store() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join(FILE_NAME);
        let encoded = to_bytes(&vec![StoredEntry {
            summary: summary(11, 11, 1, 0, false, Some(b"max")),
            updated_at_ms: u64::MAX,
        }])
        .expect("encode max-timestamp store");
        fs::write(&file, encoded).expect("write max-timestamp store");

        let snapshot = read_persisted_snapshot(dir.path());
        assert!(
            snapshot.is_empty(),
            "unrepresentable status timestamps must not be reported"
        );
        assert!(
            !file.exists(),
            "unrepresentable status stores should be removed when every entry is invalid"
        );
    }

    #[test]
    fn persistence_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let handle = register_handle();
        set_active(&handle);
        handle.configure(Some(StoreConfig {
            dir: dir.path().to_path_buf(),
            ttl: Duration::from_secs(60),
            capacity: 8,
        }));
        let summary = Summary {
            block_hash: hash(1),
            height: 1,
            view: 0,
            total_chunks: 4,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 2,
            ready_count: 1,
            delivered: false,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        handle.update(summary, SystemTime::now());
        assert_eq!(handle.snapshot().len(), 1);

        let handle = register_handle();
        set_active(&handle);
        handle.configure(Some(StoreConfig {
            dir: dir.path().to_path_buf(),
            ttl: Duration::from_secs(60),
            capacity: 8,
        }));
        let items = handle.snapshot();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].block_hash, hash(1));
        assert_eq!(items[0].height, 1);
        assert!(!items[0].invalid);
        assert!(items[0].lane_backlog.is_empty());
        assert!(items[0].dataspace_backlog.is_empty());
    }

    #[test]
    fn configure_failure_marks_persistence_unavailable_but_keeps_memory_snapshot() {
        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join("not-a-directory");
        fs::write(&file_path, b"not a directory").expect("write obstacle file");

        let handle = register_handle();
        handle.configure(Some(StoreConfig {
            dir: file_path,
            ttl: Duration::from_secs(60),
            capacity: 8,
        }));

        assert!(
            handle.persistence_unavailable_for_tests(),
            "configured persistence failures should remain visible until reconfigure"
        );

        let key = (hash(10), 10, 0);
        let summary = Summary {
            block_hash: key.0,
            height: key.1,
            view: key.2,
            total_chunks: 3,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 2,
            ready_count: 1,
            delivered: false,
            payload_hash: Some(Hash::new(b"memory-only")),
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        handle.update(summary.clone(), SystemTime::now());

        assert_eq!(
            handle.get(&key),
            Some(summary),
            "operator status should keep in-memory snapshots after persistence setup fails"
        );
        assert_eq!(handle.sessions_active(), 1);
        assert!(
            handle.persistence_unavailable_for_tests(),
            "memory-only updates must not hide the persistence failure"
        );

        handle.configure(None);
        assert!(
            !handle.persistence_unavailable_for_tests(),
            "explicitly disabling persistence should clear failure state"
        );
    }

    #[test]
    fn next_stale_due_picks_earliest_entry() {
        let handle = Handle::new();
        let now = UNIX_EPOCH + Duration::from_secs(100);
        let summary_one = Summary {
            block_hash: hash(1),
            height: 1,
            view: 0,
            total_chunks: 1,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 0,
            ready_count: 0,
            delivered: false,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        let summary_two = Summary {
            block_hash: hash(2),
            height: 2,
            view: 0,
            total_chunks: 1,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 0,
            ready_count: 0,
            delivered: false,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        handle.update(summary_one, now - Duration::from_secs(5));
        handle.update(summary_two, now - Duration::from_secs(2));

        let due = handle
            .next_stale_due(Duration::from_secs(10), now)
            .expect("entries should report a due time");
        assert_eq!(due, Duration::from_secs(5));

        let due = handle
            .next_stale_due(Duration::from_secs(3), now)
            .expect("entries should be stale under shorter TTL");
        assert_eq!(due, Duration::ZERO);
    }

    #[test]
    fn delivered_payload_matches_requires_complete_chunks() {
        let handle = register_handle();
        set_active(&handle);

        let block_hash = hash(9);
        let payload_hash = Hash::new(b"payload");
        let summary = Summary {
            block_hash,
            height: 9,
            view: 0,
            total_chunks: 2,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 1,
            ready_count: 0,
            delivered: true,
            payload_hash: Some(payload_hash),
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        handle.update(summary.clone(), SystemTime::now());
        assert!(
            !handle.delivered_payload_matches(&block_hash, 9, &payload_hash),
            "incomplete chunks should not satisfy delivered payload match"
        );

        let summary = Summary {
            received_chunks: 2,
            ..summary
        };
        handle.update(summary, SystemTime::now());
        assert!(
            handle.delivered_payload_matches(&block_hash, 9, &payload_hash),
            "complete chunks should satisfy delivered payload match"
        );
    }

    #[test]
    fn delivery_predicates_require_valid_complete_chunks() {
        let handle = register_handle();
        set_active(&handle);

        let block_hash = hash(12);
        let payload_hash = Hash::new(b"payload");
        let base_summary = Summary {
            block_hash,
            height: 12,
            view: 0,
            total_chunks: 2,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 1,
            ready_count: 1,
            delivered: true,
            payload_hash: Some(payload_hash),
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };

        handle.update(base_summary.clone(), SystemTime::now());
        assert!(
            !handle.is_delivered(&block_hash, 12),
            "incomplete delivered summaries must not count as delivered"
        );
        assert!(
            !handle.delivered_payload_matches(&block_hash, 12, &payload_hash),
            "incomplete delivered summaries must not match payloads"
        );

        handle.update(
            Summary {
                total_chunks: 2,
                received_chunks: 3,
                ..base_summary.clone()
            },
            SystemTime::now(),
        );
        assert!(
            !handle.is_delivered(&block_hash, 12),
            "over-counted delivered summaries must not count as delivered"
        );
        assert!(
            !handle.delivered_payload_matches(&block_hash, 12, &payload_hash),
            "over-counted delivered summaries must not match delivered payloads"
        );
        assert!(
            !handle.complete_payload_matches(&block_hash, 12, 0, &payload_hash),
            "over-counted summaries must not match complete payloads"
        );

        handle.update(
            Summary {
                total_chunks: 0,
                received_chunks: 0,
                ..base_summary.clone()
            },
            SystemTime::now(),
        );
        assert!(
            !handle.is_delivered(&block_hash, 12),
            "zero-chunk summaries must not count as delivered"
        );
        assert!(
            !handle.complete_payload_matches(&block_hash, 12, 0, &payload_hash),
            "zero-chunk summaries must not match complete payloads"
        );

        handle.update(
            Summary {
                total_chunks: 2,
                received_chunks: 2,
                invalid: true,
                ..base_summary.clone()
            },
            SystemTime::now(),
        );
        assert!(
            !handle.is_delivered(&block_hash, 12),
            "invalid summaries must not count as delivered"
        );
        assert!(
            !handle.delivered_payload_matches(&block_hash, 12, &payload_hash),
            "invalid summaries must not match delivered payloads"
        );

        handle.update(
            Summary {
                total_chunks: 2,
                received_chunks: 2,
                delivered: false,
                ..base_summary.clone()
            },
            SystemTime::now(),
        );
        assert!(
            !handle.is_delivered(&block_hash, 12),
            "complete summaries without DELIVER must not count as delivered"
        );
        assert!(
            handle.complete_payload_matches(&block_hash, 12, 0, &payload_hash),
            "complete valid chunks should match payloads before DELIVER"
        );
        assert!(
            !handle.delivered_payload_matches(&block_hash, 12, &payload_hash),
            "complete valid chunks without DELIVER must not match delivered payloads"
        );

        handle.update(
            Summary {
                total_chunks: 2,
                received_chunks: 2,
                ..base_summary
            },
            SystemTime::now(),
        );
        assert!(
            handle.is_delivered(&block_hash, 12),
            "valid complete delivered summaries should count as delivered"
        );
        assert!(
            handle.delivered_payload_matches(&block_hash, 12, &payload_hash),
            "valid complete delivered summaries should match payloads"
        );
    }

    #[test]
    fn update_drops_impossible_summary_and_clears_stale_entry() {
        let handle = register_handle();
        set_active(&handle);

        let block_hash = hash(13);
        let payload_hash = Hash::new(b"payload");
        let valid = Summary {
            block_hash,
            height: 13,
            view: 0,
            total_chunks: 2,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 2,
            ready_count: 1,
            delivered: true,
            payload_hash: Some(payload_hash),
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        handle.update(valid.clone(), SystemTime::now());
        assert!(handle.delivered_payload_matches(&block_hash, 13, &payload_hash));

        handle.update(
            Summary {
                received_chunks: 3,
                ..valid
            },
            SystemTime::now(),
        );

        assert!(
            handle.get(&(block_hash, 13, 0)).is_none(),
            "impossible updates must clear stale summaries for the same key"
        );
        assert!(
            !handle.delivered_payload_matches(&block_hash, 13, &payload_hash),
            "stale delivered proof must not survive an impossible replacement"
        );

        let allocated_payload_hash = Hash::new(b"allocated-payload");
        let allocated = Summary {
            block_hash,
            height: 13,
            view: 0,
            total_chunks: 2,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 2,
            ready_count: 1,
            delivered: true,
            payload_hash: Some(allocated_payload_hash),
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: vec![LaneRbcSnapshot {
                lane_id: 7,
                tx_count: 1,
                total_chunks: 2,
                pending_chunks: 0,
                rbc_bytes_total: 16,
            }],
            dataspace_backlog: vec![DataspaceRbcSnapshot {
                lane_id: 7,
                dataspace_id: 42,
                tx_count: 1,
                total_chunks: 2,
                pending_chunks: 0,
                rbc_bytes_total: 16,
            }],
        };
        handle.update(allocated.clone(), SystemTime::now());
        assert!(handle.delivered_payload_matches(&block_hash, 13, &allocated_payload_hash));

        handle.update(
            Summary {
                dataspace_backlog: vec![DataspaceRbcSnapshot {
                    lane_id: 7,
                    dataspace_id: 42,
                    tx_count: 1,
                    total_chunks: 1,
                    pending_chunks: 0,
                    rbc_bytes_total: 16,
                }],
                ..allocated
            },
            SystemTime::now(),
        );

        assert!(
            handle.get(&(block_hash, 13, 0)).is_none(),
            "inconsistent allocation updates must clear stale summaries for the same key"
        );
    }

    #[test]
    fn persisted_snapshot_drops_impossible_chunk_shapes() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join(FILE_NAME);
        let now_ms = system_time_to_ms(SystemTime::now());
        let valid_in_progress = summary(13, 13, 1, 0, false, Some(b"in-progress"));
        let valid_delivered = summary(14, 13, 4, 2, true, Some(b"delivered"));
        let invalid_diagnostic = Summary {
            invalid: true,
            delivered: true,
            received_chunks: 3,
            ..summary(15, 13, 3, 0, false, Some(b"invalid"))
        };
        let zero_chunk = Summary {
            total_chunks: 0,
            received_chunks: 0,
            ..summary(16, 13, 0, 0, false, Some(b"zero"))
        };
        let over_counted = Summary {
            total_chunks: 3,
            received_chunks: 4,
            ..summary(17, 13, 4, 0, false, Some(b"over"))
        };
        let delivered_incomplete = Summary {
            total_chunks: 4,
            received_chunks: 3,
            delivered: true,
            ..summary(18, 13, 3, 0, false, Some(b"incomplete"))
        };
        let encoded = to_bytes(&vec![
            StoredEntry {
                summary: valid_in_progress.clone(),
                updated_at_ms: now_ms,
            },
            StoredEntry {
                summary: valid_delivered.clone(),
                updated_at_ms: now_ms,
            },
            StoredEntry {
                summary: invalid_diagnostic.clone(),
                updated_at_ms: now_ms,
            },
            StoredEntry {
                summary: zero_chunk,
                updated_at_ms: now_ms,
            },
            StoredEntry {
                summary: over_counted,
                updated_at_ms: now_ms,
            },
            StoredEntry {
                summary: delivered_incomplete,
                updated_at_ms: now_ms,
            },
        ])
        .expect("encode RBC status store");
        fs::write(&file, encoded).expect("write RBC status store");

        let snapshot = read_persisted_snapshot(dir.path());
        assert_eq!(
            snapshot,
            vec![valid_in_progress, valid_delivered, invalid_diagnostic],
            "persisted recovery should keep valid and invalid diagnostic rows but drop impossible chunk shapes"
        );
    }

    #[test]
    fn persisted_snapshot_drops_inconsistent_allocation_metadata() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join(FILE_NAME);
        let now_ms = system_time_to_ms(SystemTime::now());
        let valid = Summary {
            lane_backlog: vec![LaneRbcSnapshot {
                lane_id: 7,
                tx_count: 2,
                total_chunks: 4,
                pending_chunks: 3,
                rbc_bytes_total: 1024,
            }],
            dataspace_backlog: vec![DataspaceRbcSnapshot {
                lane_id: 7,
                dataspace_id: 42,
                tx_count: 2,
                total_chunks: 4,
                pending_chunks: 3,
                rbc_bytes_total: 1024,
            }],
            ..summary(19, 13, 1, 0, false, Some(b"valid-alloc"))
        };
        let inconsistent = Summary {
            lane_backlog: vec![LaneRbcSnapshot {
                lane_id: 7,
                tx_count: 2,
                total_chunks: 4,
                pending_chunks: 3,
                rbc_bytes_total: 1024,
            }],
            dataspace_backlog: vec![DataspaceRbcSnapshot {
                lane_id: 7,
                dataspace_id: 42,
                tx_count: 2,
                total_chunks: 3,
                pending_chunks: 3,
                rbc_bytes_total: 1024,
            }],
            ..summary(20, 13, 1, 0, false, Some(b"bad-alloc"))
        };
        let over_pending = Summary {
            lane_backlog: vec![LaneRbcSnapshot {
                lane_id: 7,
                tx_count: 2,
                total_chunks: 4,
                pending_chunks: 5,
                rbc_bytes_total: 1024,
            }],
            dataspace_backlog: vec![DataspaceRbcSnapshot {
                lane_id: 7,
                dataspace_id: 42,
                tx_count: 2,
                total_chunks: 4,
                pending_chunks: 4,
                rbc_bytes_total: 1024,
            }],
            ..summary(21, 13, 1, 0, false, Some(b"over-pending"))
        };
        let dataspace_over_pending = Summary {
            lane_backlog: vec![LaneRbcSnapshot {
                lane_id: 7,
                tx_count: 2,
                total_chunks: 4,
                pending_chunks: 4,
                rbc_bytes_total: 1024,
            }],
            dataspace_backlog: vec![DataspaceRbcSnapshot {
                lane_id: 7,
                dataspace_id: 42,
                tx_count: 2,
                total_chunks: 4,
                pending_chunks: 5,
                rbc_bytes_total: 1024,
            }],
            ..summary(22, 13, 1, 0, false, Some(b"dataspace-over-pending"))
        };
        let encoded = to_bytes(&vec![
            StoredEntry {
                summary: valid.clone(),
                updated_at_ms: now_ms,
            },
            StoredEntry {
                summary: inconsistent,
                updated_at_ms: now_ms,
            },
            StoredEntry {
                summary: over_pending,
                updated_at_ms: now_ms,
            },
            StoredEntry {
                summary: dataspace_over_pending,
                updated_at_ms: now_ms,
            },
        ])
        .expect("encode RBC status store");
        fs::write(&file, encoded).expect("write RBC status store");

        assert_eq!(
            read_persisted_snapshot(dir.path()),
            vec![valid],
            "persisted recovery should drop allocation summaries whose dataspace totals do not match the lane"
        );
    }

    #[test]
    fn update_persists_timestamp_when_summary_unchanged() {
        let dir = tempdir().expect("tempdir");
        let handle = register_handle();
        handle.configure(Some(StoreConfig {
            dir: dir.path().to_path_buf(),
            ttl: Duration::from_secs(60),
            capacity: 8,
        }));
        let block_hash = hash(9);
        let height = 9;
        let view = 0;
        let summary = Summary {
            block_hash,
            height,
            view,
            total_chunks: 3,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 2,
            ready_count: 1,
            delivered: false,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        let initial_time = SystemTime::now() - Duration::from_secs(1);
        handle.update(summary.clone(), initial_time);
        let path = dir.path().join(FILE_NAME);
        let before = fs::read(&path).expect("read RBC snapshot");

        let updated_time = initial_time + Duration::from_secs(1);
        handle.update(summary, updated_time);
        let after = fs::read(&path).expect("read RBC snapshot");
        assert_ne!(before, after);

        let stored = decode_entries(&after).expect("decode RBC session store");
        let entry = stored
            .iter()
            .find(|entry| entry.summary.block_hash == block_hash)
            .expect("entry persisted");
        assert_eq!(entry.updated_at_ms, system_time_to_ms(updated_time));

        let key = (block_hash, height, view);
        let inner = handle.store.lock_inner();
        let entry = inner.map.get(&key).expect("entry exists");
        assert_eq!(
            system_time_to_ms(entry.updated_at),
            system_time_to_ms(updated_time)
        );
    }

    #[test]
    fn handle_recovers_from_poisoned_status_lock() {
        let handle = register_handle();
        let store = handle.store.clone();
        {
            let _suppressor = panic_hook::ScopedSuppressor::new();
            let result = std::panic::catch_unwind({
                let store = store.clone();
                move || {
                    let _guard = store.inner.lock().expect("fresh RBC status lock");
                    panic!("poison RBC status lock for recovery test");
                }
            });
            assert!(result.is_err());
        }
        assert!(
            store.inner.is_poisoned(),
            "test precondition should poison the status mutex"
        );

        let observed = SystemTime::now();
        let summary = summary(41, 41, 2, 1, false, Some(b"poisoned-lock"));
        let key = (summary.block_hash, summary.height, summary.view);
        handle.update(summary.clone(), observed);

        assert_eq!(
            handle.get(&key),
            Some(summary.clone()),
            "poisoned status locks should recover and keep accepting updates"
        );
        assert_eq!(handle.snapshot(), vec![summary]);
        assert_eq!(handle.sessions_active(), 1);

        handle.remove(&key);
        assert_eq!(handle.get(&key), None);
        assert_eq!(handle.sessions_active(), 0);
    }

    #[test]
    fn ttl_prunes_on_init() {
        let dir = tempdir().expect("tempdir");
        let handle = register_handle();
        handle.configure(Some(StoreConfig {
            dir: dir.path().to_path_buf(),
            ttl: Duration::from_secs(1),
            capacity: 8,
        }));
        let summary = Summary {
            block_hash: hash(2),
            height: 2,
            view: 0,
            total_chunks: 1,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 1,
            ready_count: 0,
            delivered: true,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        handle.update(summary, SystemTime::now() - Duration::from_secs(10));
        handle.configure(Some(StoreConfig {
            dir: dir.path().to_path_buf(),
            ttl: Duration::from_secs(1),
            capacity: 8,
        }));
        assert!(handle.snapshot().is_empty());
    }

    #[test]
    fn capacity_prunes_oldest_on_init() {
        let dir = tempdir().expect("tempdir");
        let base = SystemTime::now() - Duration::from_secs(3);
        let handle = register_handle();
        handle.configure(Some(StoreConfig {
            dir: dir.path().to_path_buf(),
            ttl: Duration::from_secs(120),
            capacity: 2,
        }));
        let summary1 = Summary {
            block_hash: hash(3),
            height: 1,
            view: 0,
            total_chunks: 1,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 1,
            ready_count: 0,
            delivered: false,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        handle.update(summary1, base);
        let summary2 = Summary {
            block_hash: hash(4),
            height: 2,
            view: 0,
            total_chunks: 1,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 1,
            ready_count: 0,
            delivered: false,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        handle.update(summary2, base + Duration::from_secs(1));
        let summary3 = Summary {
            block_hash: hash(5),
            height: 3,
            view: 0,
            total_chunks: 1,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 1,
            ready_count: 0,
            delivered: false,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        handle.update(summary3, base + Duration::from_secs(2));
        handle.configure(Some(StoreConfig {
            dir: dir.path().to_path_buf(),
            ttl: Duration::from_secs(120),
            capacity: 2,
        }));
        let items = handle.snapshot();
        assert_eq!(items.len(), 2);
        let heights: Vec<u64> = items.iter().map(|s| s.height).collect();
        assert!(heights.contains(&2));
        assert!(heights.contains(&3));
    }

    #[test]
    fn fatal_persist_error_disables_disk_but_keeps_memory_snapshot() {
        let dir = tempdir().expect("tempdir");
        let handle = register_handle();
        handle.configure(Some(StoreConfig {
            dir: dir.path().to_path_buf(),
            ttl: Duration::from_secs(60),
            capacity: 8,
        }));
        let key = (hash(7), 7, 0);
        let summary = Summary {
            block_hash: key.0,
            height: key.1,
            view: key.2,
            total_chunks: 4,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 2,
            ready_count: 1,
            delivered: false,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        let base = SystemTime::now() - Duration::from_secs(1);
        handle.update(summary.clone(), base);
        let path = dir.path().join(FILE_NAME);
        let persisted_before_fault = fs::read(&path).expect("persisted snapshot");

        {
            let mut inner = handle.store.lock_inner();
            inner
                .disk
                .as_mut()
                .expect("disk store configured")
                .store
                .fail_persist_with = Some(io::ErrorKind::StorageFull);
        }

        let updated = Summary {
            received_chunks: 3,
            ..summary
        };
        handle.update(updated.clone(), base + Duration::from_secs(1));

        assert_eq!(handle.get(&key), Some(updated));
        let inner = handle.store.lock_inner();
        assert!(
            inner.disk.as_ref().is_some_and(|disk| disk.disabled),
            "fatal persist errors must disable future disk writes"
        );
        drop(inner);

        let persisted_after_fault = fs::read(&path).expect("persisted snapshot");
        assert_eq!(
            persisted_after_fault, persisted_before_fault,
            "fatal persist errors must not clobber the last successful on-disk snapshot"
        );
    }

    #[test]
    fn disabled_persistence_stops_future_disk_writes_until_reconfigure() {
        let dir = tempdir().expect("tempdir");
        let handle = register_handle();
        handle.configure(Some(StoreConfig {
            dir: dir.path().to_path_buf(),
            ttl: Duration::from_secs(60),
            capacity: 8,
        }));
        let key = (hash(8), 8, 0);
        let summary = Summary {
            block_hash: key.0,
            height: key.1,
            view: key.2,
            total_chunks: 2,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 1,
            ready_count: 0,
            delivered: false,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        let base = SystemTime::now() - Duration::from_secs(4);
        handle.update(summary.clone(), base);
        let path = dir.path().join(FILE_NAME);

        {
            let mut inner = handle.store.lock_inner();
            inner
                .disk
                .as_mut()
                .expect("disk store configured")
                .store
                .fail_persist_with = Some(io::ErrorKind::WriteZero);
        }
        handle.update(
            Summary {
                received_chunks: 2,
                ..summary.clone()
            },
            base + Duration::from_secs(1),
        );
        let persisted_after_disable = fs::read(&path).expect("persisted snapshot");

        handle.update(
            Summary {
                delivered: true,
                ..summary.clone()
            },
            base + Duration::from_secs(2),
        );
        handle.remove(&key);
        assert_eq!(
            fs::read(&path).expect("persisted snapshot"),
            persisted_after_disable,
            "memory-only mode must stop future persist attempts until reconfigured"
        );

        handle.configure(Some(StoreConfig {
            dir: dir.path().to_path_buf(),
            ttl: Duration::from_secs(60),
            capacity: 8,
        }));
        {
            let inner = handle.store.lock_inner();
            assert!(
                inner.disk.as_ref().is_some_and(|disk| !disk.disabled),
                "explicit configure(Some(...)) must re-enable persistence"
            );
        }

        let replacement = Summary {
            block_hash: key.0,
            height: key.1,
            view: key.2,
            total_chunks: 3,
            encoding: RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 3,
            ready_count: 2,
            delivered: true,
            payload_hash: Some(Hash::new(b"re-enabled")),
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        };
        handle.update(replacement.clone(), base + Duration::from_secs(3));

        assert!(
            read_persisted_snapshot(dir.path())
                .iter()
                .any(|entry| entry.block_hash == replacement.block_hash
                    && entry.height == replacement.height
                    && entry.received_chunks == replacement.received_chunks
                    && entry.delivered == replacement.delivered),
            "reconfigured persistence should write fresh snapshots again"
        );
    }
}
