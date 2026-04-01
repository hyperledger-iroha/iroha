//! Disk-backed snapshot of `RBC` session summaries for operator endpoints.
//! Not consensus-critical. Each Sumeragi instance registers its own handle
//! so concurrent actors do not trample one another.

use core::sync::atomic::{AtomicU64, Ordering};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
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

#[derive(Default)]
struct Inner {
    map: BTreeMap<(HashOf<BlockHeader>, u64, u64), Entry>,
    disk: Option<DiskPersistenceState>,
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
    fn snapshot(&self) -> Vec<Summary> {
        let inner = self.inner.lock().expect("rbc status lock poisoned");
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
        let mut inner = self.store.inner.lock().expect("rbc status lock poisoned");
        inner.map.clear();
        match config {
            Some(cfg) => match DiskStore::new(&cfg) {
                Ok(disk) => {
                    load_into_map(&disk, &mut inner.map);
                    inner.disk = Some(DiskPersistenceState::new(disk));
                    set_persistence_disabled_metric(false);
                    persist_if_needed(&mut inner, "configure");
                }
                Err(err) => {
                    warn!(
                        ?err,
                        "failed to initialise RBC session store; persistence disabled"
                    );
                    inner.disk = None;
                    set_persistence_disabled_metric(false);
                }
            },
            None => {
                inner.disk = None;
                set_persistence_disabled_metric(false);
            }
        }
        self.store
            .active_count
            .store(inner.map.len() as u64, Ordering::Relaxed);
    }

    /// Update or insert a session summary.
    pub fn update(&self, summary: Summary, updated_at: SystemTime) {
        let mut inner = self.store.inner.lock().expect("rbc status lock poisoned");
        let key = (summary.block_hash, summary.height, summary.view);
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
        let inner = self.store.inner.lock().expect("rbc status lock poisoned");
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
        let inner = self.store.inner.lock().expect("rbc status lock poisoned");
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
        let inner = self.store.inner.lock().expect("rbc status lock poisoned");
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
        let mut inner = self.store.inner.lock().expect("rbc status lock poisoned");
        inner.map.remove(key);
        if inner.disk.is_some() {
            let (ttl, capacity) = {
                let disk = inner
                    .disk
                    .as_ref()
                    .expect("disk store should exist when checked");
                (disk.store.ttl, disk.store.capacity)
            };
            enforce_map_limits(&mut inner.map, ttl, capacity);
            persist_if_needed(&mut inner, "remove");
        }
        self.store
            .active_count
            .store(inner.map.len() as u64, Ordering::Relaxed);
    }

    /// Clear all session summaries.
    pub fn clear(&self) {
        let mut inner = self.store.inner.lock().expect("rbc status lock poisoned");
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

    /// Check whether a delivered session exists for the given `(block_hash, height)` pair.
    pub fn is_delivered(&self, block_hash: &HashOf<BlockHeader>, height: u64) -> bool {
        let inner = self.store.inner.lock().expect("rbc status lock poisoned");
        let start = (*block_hash, height, 0);
        let end = (*block_hash, height, u64::MAX);
        inner
            .map
            .range(start..=end)
            .any(|(_, entry)| entry.summary.delivered)
    }

    /// Check whether a delivered session with a complete chunk set matches the provided payload.
    pub fn delivered_payload_matches(
        &self,
        block_hash: &HashOf<BlockHeader>,
        height: u64,
        payload_hash: &Hash,
    ) -> bool {
        let inner = self.store.inner.lock().expect("rbc status lock poisoned");
        let start = (*block_hash, height, 0);
        let end = (*block_hash, height, u64::MAX);
        inner.map.range(start..=end).any(|(_, entry)| {
            let summary = &entry.summary;
            summary.delivered
                && !summary.invalid
                && summary.received_chunks == summary.total_chunks
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
        let inner = self.store.inner.lock().expect("rbc status lock poisoned");
        inner
            .map
            .get(&(*block_hash, height, view))
            .is_some_and(|entry| {
                let summary = &entry.summary;
                !summary.invalid
                    && summary.received_chunks == summary.total_chunks
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
        let mut inner = self.store.inner.lock().expect("rbc status lock poisoned");
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
    *active_slot().lock().expect("rbc active slot poisoned") = Some(handle.store.clone());
}

fn active_store() -> Option<Arc<Store>> {
    active_slot()
        .lock()
        .expect("rbc active slot poisoned")
        .clone()
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
        let updated_at = ms_to_system_time(stored.updated_at_ms);
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

    let mut selected = None;
    if let Some(bytes) = main_bytes.as_deref() {
        match decode_entries(bytes) {
            Ok(entries) => {
                selected = Some(entries);
            }
            Err(err) => {
                warn!(?err, ?path, "failed to decode RBC session store");
                let _ = fs::remove_file(path);
            }
        }
    }

    let mut used_tmp = false;
    if selected.is_none()
        && let Some(bytes) = tmp_bytes.as_deref()
    {
        match decode_entries(bytes) {
            Ok(entries) => {
                selected = Some(entries);
                used_tmp = true;
            }
            Err(err) => {
                warn!(?err, ?tmp_path, "failed to decode RBC session temp store");
                let _ = fs::remove_file(&tmp_path);
            }
        }
    }

    if let Some(entries) = selected {
        if used_tmp {
            warn!(
                path = %tmp_path.display(),
                "recovered RBC session store from temp file"
            );
            promote_temp_store(&tmp_path, path);
        }
        return entries;
    }

    Vec::new()
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
            let updated_at = ms_to_system_time(stored.updated_at_ms);
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
        map.insert(
            key,
            Entry {
                summary: stored.summary,
                updated_at: ms_to_system_time(stored.updated_at_ms),
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

fn ms_to_system_time(ms: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(ms)
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
            tmp.exists(),
            "valid main store should not require temp promotion"
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
    fn next_stale_due_picks_earliest_entry() {
        let handle = Handle::new();
        let now = UNIX_EPOCH + Duration::from_secs(100);
        let summary_one = Summary {
            block_hash: hash(1),
            height: 1,
            view: 0,
            total_chunks: 0,
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
            total_chunks: 0,
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
        let initial_time = SystemTime::now();
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
        let inner = handle.store.inner.lock().expect("rbc status lock poisoned");
        let entry = inner.map.get(&key).expect("entry exists");
        assert_eq!(
            system_time_to_ms(entry.updated_at),
            system_time_to_ms(updated_time)
        );
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
        let base = SystemTime::now();
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
        handle.update(summary.clone(), SystemTime::now());
        let path = dir.path().join(FILE_NAME);
        let persisted_before_fault = fs::read(&path).expect("persisted snapshot");

        {
            let mut inner = handle.store.inner.lock().expect("rbc status lock poisoned");
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
        handle.update(updated.clone(), SystemTime::now() + Duration::from_secs(1));

        assert_eq!(handle.get(&key), Some(updated));
        let inner = handle.store.inner.lock().expect("rbc status lock poisoned");
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
        handle.update(summary.clone(), SystemTime::now());
        let path = dir.path().join(FILE_NAME);

        {
            let mut inner = handle.store.inner.lock().expect("rbc status lock poisoned");
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
            SystemTime::now() + Duration::from_secs(1),
        );
        let persisted_after_disable = fs::read(&path).expect("persisted snapshot");

        handle.update(
            Summary {
                delivered: true,
                ..summary.clone()
            },
            SystemTime::now() + Duration::from_secs(2),
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
            let inner = handle.store.inner.lock().expect("rbc status lock poisoned");
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
        handle.update(
            replacement.clone(),
            SystemTime::now() + Duration::from_secs(3),
        );

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
