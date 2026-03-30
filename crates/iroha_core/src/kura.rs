//! Translates to warehouse. File-system and persistence-related
//! logic.  [`Kura`] is the main entity which should be used to store
//! new [`Block`](iroha_data_model::block::SignedBlock)s on the
//! blockchain.
use std::{
    collections::VecDeque,
    fmt::Debug,
    io::{BufWriter, ErrorKind, Read, Seek, SeekFrom, Write},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, RecvTimeoutError},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::telemetry::StateTelemetry;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_config::{
    kura::{FsyncMode, InitMode},
    parameters::{
        actual::{Kura as Config, LaneConfig, LaneConfigEntry},
        defaults::kura::{
            BLOCK_SYNC_ROSTER_RETENTION, BLOCKS_IN_MEMORY, FSYNC_INTERVAL, MAX_DISK_USAGE_BYTES,
            MERGE_LEDGER_CACHE_CAPACITY, ROSTER_SIDECAR_RETENTION,
        },
    },
};
use iroha_crypto::{Hash, HashOf};
#[cfg(test)]
use iroha_data_model::block::decode_versioned_signed_block;
use iroha_data_model::{
    block::{BlockHeader, SignedBlock, decode_framed_signed_block},
    consensus::{Qc, ValidatorSetCheckpoint},
    merge::MergeLedgerEntry,
    peer::PeerId,
    transaction::signed::TransactionEntrypoint,
};
use iroha_file_mmap::ReadOnlyMmap;
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal, spawn_os_thread_as_future};
use iroha_logger::prelude::*;
#[cfg(test)]
use iroha_primitives::time::TimeSource;
#[cfg(test)]
use norito::core::{Header, MAGIC};
use norito::{
    codec::{Decode, Encode},
    json::Value as JsonValue,
};
use parking_lot::Mutex;

#[cfg(test)]
use crate::merge::reduce_merge_hint_roots;
use crate::sumeragi::stake_snapshot::CommitStakeSnapshot;
use crate::{block::CommittedBlock, commit_roster_journal::CommitRosterJournal};

impl From<CommittedBlock> for Arc<SignedBlock> {
    fn from(value: CommittedBlock) -> Self {
        Arc::new(value.into())
    }
}

const INDEX_FILE_NAME: &str = "blocks.index";
const DATA_FILE_NAME: &str = "blocks.data";
const HASHES_FILE_NAME: &str = "blocks.hashes";
const COUNT_FILE_NAME: &str = "blocks.count.norito";
const PIPELINE_DIR_NAME: &str = "pipeline";
const DA_BLOCKS_DIR_NAME: &str = "da_blocks";
const PIPELINE_SIDECARS_DATA_FILE: &str = "sidecars.norito";
const PIPELINE_SIDECARS_INDEX_FILE: &str = "sidecars.index";
const ROSTER_SIDECARS_DATA_FILE: &str = "roster_sidecars.norito";
const ROSTER_SIDECARS_INDEX_FILE: &str = "roster_sidecars.index";
const PIPELINE_INDEX_ENTRY_SIZE: usize = core::mem::size_of::<u64>() * 2;
const PIPELINE_INDEX_ENTRY_SIZE_U64: u64 = PIPELINE_INDEX_ENTRY_SIZE as u64;
const DISK_USAGE_TOTAL_REFRESH_INTERVAL: Duration = Duration::from_secs(60 * 60);

const SIZE_OF_BLOCK_HASH: u64 = Hash::LENGTH as u64;
pub(crate) const STRICT_INIT_MAX_BLOCK_BYTES: u64 = 256 * 1024 * 1024;
const EVICTED_BLOCK_START: u64 = u64::MAX;
/// Upper bound for merge-ledger entry payloads to avoid unbounded allocations on recovery.
const MERGE_LEDGER_MAX_ENTRY_BYTES: usize = 16 * 1024 * 1024;

/// The interface of Kura subsystem.
///
/// Merge-ledger persistence requirements are tracked in
/// `docs/source/merge_ledger.md`; follow that plan when wiring
/// global state checkpoints into storage.
#[derive(Debug)]
pub struct Kura {
    /// The block storage
    block_store: Mutex<BlockStore>,
    /// The array of block hashes and a slot for an arc of the block. This is normally recovered from the index file.
    block_data: Mutex<BlockData>,
    /// Channel for waking the writer thread when new blocks arrive or shutdown is signalled.
    block_notify_tx: mpsc::Sender<BlockNotify>,
    block_notify_rx: Mutex<Option<mpsc::Receiver<BlockNotify>>>,
    /// Path to newline-delimited JSON (JSONL) block dump.
    block_plain_text_path: Mutex<Option<PathBuf>>,
    /// Serialize sidecar writes to avoid index/data races.
    sidecar_lock: Mutex<()>,
    /// Queue of pipeline sidecar writes flushed by the Kura writer thread.
    pipeline_sidecar_queue: Mutex<VecDeque<PipelineRecoverySidecar>>,
    /// Root directory where Kura stores lane segments.
    store_root: PathBuf,
    /// Active block directory for the primary lane.
    active_blocks_dir: Mutex<PathBuf>,
    /// Active merge-ledger file path for the primary lane.
    active_merge_path: Mutex<PathBuf>,
    /// Maximum on-disk footprint for Kura block storage (0 = unlimited).
    max_disk_usage_bytes: u64,
    /// Cached disk usage for budget enforcement (excludes DA payloads).
    disk_usage: AtomicU64,
    /// Cached total disk usage including DA payloads.
    disk_usage_total: AtomicU64,
    /// Cached sum of budgeted bytes for blocks queued but not yet durably indexed.
    pending_budget_bytes: AtomicU64,
    /// Marks whether `pending_budget_bytes` currently reflects in-memory queue state.
    pending_budget_bytes_valid: AtomicBool,
    /// Indicates whether the budget usage cache was initialized successfully.
    disk_usage_initialized: AtomicBool,
    /// Indicates whether the total usage cache was initialized successfully.
    disk_usage_total_initialized: AtomicBool,
    /// Last successful total-usage refresh time (seconds since UNIX epoch).
    disk_usage_total_last_refresh: AtomicU64,
    /// Number of most recent non-genesis blocks stored in memory.
    /// The genesis block is always retained for metrics and replay.
    blocks_in_memory: NonZeroUsize,
    /// Number of recent commit-roster snapshots retained for block sync.
    block_sync_roster_retention: NonZeroUsize,
    /// Number of recent roster sidecars retained alongside the block store.
    roster_sidecar_retention: NonZeroUsize,
    /// Amount of blocks loaded during initialization
    init_block_count: usize,
    /// On-disk merge-ledger log and in-memory cache.
    merge_log: Mutex<MergeLedgerLog>,
    /// Durably persisted commit rosters for block-sync consumers.
    #[allow(dead_code)]
    roster_log: Mutex<CommitRosterJournal>,
    /// Optional telemetry sink for storage budget reporting.
    telemetry: OnceLock<StateTelemetry>,
    /// Last fatal writer fault observed by the background persistence loop.
    writer_fault: Mutex<Option<String>>,
}

type BlockData = Vec<(HashOf<BlockHeader>, Option<Arc<SignedBlock>>)>;

#[derive(Clone, Copy, Debug)]
enum FsyncTarget {
    Data,
    Index,
    Hashes,
}

impl FsyncTarget {
    #[cfg(feature = "telemetry")]
    fn label(self) -> &'static str {
        match self {
            Self::Data => "blocks.data",
            Self::Index => "blocks.index",
            Self::Hashes => "blocks.hashes",
        }
    }
}

#[derive(Debug, Clone)]
struct FsyncState {
    mode: FsyncMode,
    interval: Duration,
    pending_since: Option<Instant>,
}

impl FsyncState {
    fn new(mode: FsyncMode, interval: Duration) -> Self {
        Self {
            mode,
            interval,
            pending_since: None,
        }
    }

    fn record_write(&mut self, now: Instant) {
        if matches!(self.mode, FsyncMode::Off) {
            self.pending_since = None;
            return;
        }

        self.pending_since.get_or_insert(now);
    }

    fn clear(&mut self) {
        self.pending_since = None;
    }

    fn deadline(&self) -> Option<Instant> {
        match (self.mode, self.pending_since) {
            (FsyncMode::Off, _) | (_, None) => None,
            (FsyncMode::On, Some(ts)) => Some(ts),
            (FsyncMode::Batched, Some(ts)) => Some(ts + self.interval),
        }
    }

    fn is_due(&self, now: Instant, force: bool) -> bool {
        match self.mode {
            FsyncMode::Off => false,
            FsyncMode::On => self.pending_since.is_some(),
            FsyncMode::Batched => self.pending_since.is_some_and(|pending| {
                force
                    || self.interval == Duration::ZERO
                    || now.saturating_duration_since(pending) >= self.interval
            }),
        }
    }
}

#[derive(Clone, Default, Debug)]
struct FsyncTelemetry;

impl FsyncTelemetry {
    fn new(mode: FsyncMode) -> Self {
        let telemetry = Self;
        telemetry.update_mode(mode);
        telemetry
    }

    fn update_mode(&self, mode: FsyncMode) {
        let _ = self;
        #[cfg(feature = "telemetry")]
        if let Some(metrics) = iroha_telemetry::metrics::global() {
            metrics.set_kura_fsync_mode(mode);
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = mode;
    }

    fn record_success(&self, target: FsyncTarget, duration: Duration) {
        let _ = self;
        #[cfg(feature = "telemetry")]
        if let Some(metrics) = iroha_telemetry::metrics::global() {
            metrics.record_kura_fsync_latency(target.label(), duration);
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = (target, duration);
    }

    fn record_failure(&self, target: FsyncTarget, duration: Option<Duration>) {
        let _ = self;
        #[cfg(feature = "telemetry")]
        if let Some(metrics) = iroha_telemetry::metrics::global() {
            metrics.inc_kura_fsync_failure(target.label());
            if let Some(duration) = duration {
                metrics.record_kura_fsync_latency(target.label(), duration);
            }
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = (target, duration);
    }
}

#[derive(Debug)]
struct ChainValidation {
    hashes: Vec<HashOf<BlockHeader>>,
    truncated: bool,
    hash_mismatch: bool,
}

#[derive(Debug)]
struct MergeLedgerLog {
    file: Option<FileWrap>,
    entries: Vec<MergeLedgerEntry>,
    cache_capacity: usize,
    total_entries: usize,
    #[cfg(test)]
    fail_next_append: bool,
}

fn sanitize_merge_cache_capacity(capacity: usize) -> usize {
    if capacity == 0 {
        MERGE_LEDGER_CACHE_CAPACITY
    } else {
        capacity
    }
}

impl MergeLedgerLog {
    fn open_at(path: &Path, cache_capacity: usize) -> Result<Self> {
        let cache_capacity = sanitize_merge_cache_capacity(cache_capacity);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| Error::MkDir(err, parent.to_path_buf()))?;
        }
        let mut file = FileWrap::open_with(path.to_path_buf(), |opts| {
            opts.read(true).create(true).append(true);
        })?;
        let (entries, total_entries) = Self::load_entries(&mut file, cache_capacity)?;
        file.try_io(|f| f.seek(SeekFrom::End(0)))?;
        Ok(Self {
            file: Some(file),
            entries,
            cache_capacity,
            total_entries,
            #[cfg(test)]
            fail_next_append: false,
        })
    }

    fn in_memory(cache_capacity: usize) -> Self {
        let cache_capacity = sanitize_merge_cache_capacity(cache_capacity);
        Self {
            file: None,
            entries: Vec::new(),
            cache_capacity,
            total_entries: 0,
            #[cfg(test)]
            fail_next_append: false,
        }
    }

    #[cfg(test)]
    fn consume_fail_next_append(&mut self) -> bool {
        if self.fail_next_append {
            self.fail_next_append = false;
            true
        } else {
            false
        }
    }

    fn append(&mut self, entry: &MergeLedgerEntry) -> Result<()> {
        let encoded = Encode::encode(entry);
        if encoded.len() > MERGE_LEDGER_MAX_ENTRY_BYTES {
            return Err(Error::NoritoFrame(norito::core::Error::Message(format!(
                "merge ledger entry exceeds {MERGE_LEDGER_MAX_ENTRY_BYTES} bytes"
            ))));
        }
        let len: u32 = encoded.len().try_into().map_err(|_| {
            Error::NoritoFrame(norito::core::Error::Message(
                "merge ledger entry exceeds 4 GiB".into(),
            ))
        })?;
        #[cfg(test)]
        if self.fail_next_append {
            self.fail_next_append = false;
            return Err(Error::IO(
                std::io::Error::other("merge-ledger append failed for test injection"),
                PathBuf::from("merge_log_test_fail"),
            ));
        }

        if let Some(file) = self.file.as_mut() {
            file.try_io(|f| f.write_all(&len.to_le_bytes()))?;
            file.try_io(|f| f.write_all(&encoded))?;
            file.try_io(|f| f.sync_data())?;
        }

        self.total_entries = self.total_entries.saturating_add(1);
        self.entries.push(entry.clone());
        self.trim_cache();
        Ok(())
    }

    fn snapshot(&self) -> Vec<MergeLedgerEntry> {
        self.entries.clone()
    }

    fn load_entries(
        file: &mut FileWrap,
        cache_capacity: usize,
    ) -> Result<(Vec<MergeLedgerEntry>, usize)> {
        file.try_io(|f| f.seek(SeekFrom::Start(0)))?;
        let file_len = file.try_io(|f| f.metadata().map(|meta| meta.len()))?;
        let mut entries = Vec::new();
        let mut total_entries = 0usize;
        let mut valid_len = 0u64;
        let truncated = loop {
            let mut len_buf = [0u8; 4];
            match file.try_io(|f| f.read_exact(&mut len_buf)) {
                Ok(()) => {
                    let len = u32::from_le_bytes(len_buf) as usize;
                    if len == 0 {
                        warn!("merge ledger entry length is zero; truncating tail");
                        break true;
                    }
                    if len > MERGE_LEDGER_MAX_ENTRY_BYTES {
                        warn!(
                            len,
                            limit = MERGE_LEDGER_MAX_ENTRY_BYTES,
                            "merge ledger entry length exceeds maximum; truncating tail"
                        );
                        break true;
                    }
                    let mut buf = vec![0u8; len];
                    match file.try_io(|f| f.read_exact(&mut buf)) {
                        Ok(()) => {}
                        Err(Error::IO(err, _)) if err.kind() == ErrorKind::UnexpectedEof => {
                            break true;
                        }
                        Err(err) => return Err(err),
                    }
                    let entry =
                        MergeLedgerEntry::decode(&mut &buf[..]).map_err(Error::NoritoFrame)?;
                    total_entries = total_entries.saturating_add(1);
                    entries.push(entry);
                    valid_len = valid_len.saturating_add(4 + len as u64);
                    if entries.len() > cache_capacity {
                        let overflow = entries.len() - cache_capacity;
                        entries.drain(0..overflow);
                    }
                }
                Err(Error::IO(err, _)) if err.kind() == ErrorKind::UnexpectedEof => {
                    break file_len > valid_len;
                }
                Err(err) => return Err(err),
            }
        };
        if truncated && file_len > valid_len {
            warn!(
                path = %file.path.display(),
                file_len,
                valid_len,
                "truncating merge ledger log after partial entry"
            );
            file.try_io(|f| f.set_len(valid_len))?;
            file.try_io(|f| f.sync_data())?;
        }
        Ok((entries, total_entries))
    }

    fn truncate_to_len(&mut self, keep: usize) -> Result<()> {
        if keep >= self.total_entries {
            return Ok(());
        }

        if let Some(file) = self.file.as_mut() {
            file.try_io(|f| f.seek(SeekFrom::Start(0)))?;
            let mut entries = Vec::new();
            let mut new_len = 0u64;
            for _ in 0..keep {
                let mut len_buf = [0u8; 4];
                file.try_io(|f| f.read_exact(&mut len_buf))?;
                let len = u32::from_le_bytes(len_buf);
                let len_usize: usize = len as usize;
                new_len = new_len
                    .checked_add(4 + u64::from(len))
                    .expect("merge-ledger log length overflow");

                let mut buf = vec![0u8; len_usize];
                file.try_io(|f| f.read_exact(&mut buf))?;
                let entry = MergeLedgerEntry::decode(&mut &buf[..]).map_err(Error::NoritoFrame)?;
                entries.push(entry);
                if entries.len() > self.cache_capacity {
                    let overflow = entries.len() - self.cache_capacity;
                    entries.drain(0..overflow);
                }
            }

            file.try_io(|f| f.set_len(new_len))?;
            file.try_io(|f| f.sync_data())?;
            file.try_io(|f| f.seek(SeekFrom::End(0)))?;
            self.entries = entries;
        } else {
            self.entries.truncate(keep);
        }

        self.total_entries = keep;
        self.trim_cache();
        Ok(())
    }

    fn trim_cache(&mut self) {
        if self.entries.len() > self.cache_capacity {
            let overflow = self.entries.len() - self.cache_capacity;
            self.entries.drain(0..overflow);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BlockNotify {
    NewBlock,
    Shutdown,
}

impl Kura {
    /// Return `true` when the block payload is available locally (in memory, on disk, or in the
    /// DA payload store).
    pub(crate) fn block_payload_available_by_hash(&self, hash: HashOf<BlockHeader>) -> bool {
        let Some(height) = self.get_block_height_by_hash(hash) else {
            return false;
        };
        self.block_payload_available_by_height(height)
    }

    fn block_payload_available_by_height(&self, block_height: NonZeroUsize) -> bool {
        let (block_index, has_cached) = {
            let data = self.block_data.lock();
            let idx = block_height.get().saturating_sub(1);
            if data.len() <= idx {
                return false;
            }
            (idx, data[idx].1.is_some())
        };
        if has_cached {
            return true;
        }

        let (index, da_path) = {
            let mut store = self.block_store.lock();
            let index = match store.read_block_index(block_index as u64) {
                Ok(index) => index,
                Err(err) => {
                    warn!(
                        ?err,
                        block_index,
                        "failed to read block index while checking payload availability"
                    );
                    return false;
                }
            };
            let da_path = store.da_block_path(block_height.get() as u64);
            (index, da_path)
        };

        if index.length == 0 {
            return false;
        }
        if !index.is_evicted() {
            return true;
        }

        let Ok(metadata) = std::fs::metadata(&da_path) else {
            return false;
        };
        metadata.len() == index.length
    }

    /// Initialize Kura.
    ///
    /// This does _not_ start the thread which receives and stores new blocks, see [`Self::start`].
    ///
    /// # Errors
    /// Fails if there are filesystem errors when trying
    /// to access the block store indicated by the provided
    /// path.
    pub fn new(config: &Config, lane_config: &LaneConfig) -> Result<(Arc<Self>, BlockCount)> {
        let store_dir = config.store_dir.resolve_relative_path();
        let store_root = store_dir.clone();
        let primary_lane = lane_config.primary();
        let roster_retention = config.block_sync_roster_retention;
        let roster_sidecar_retention = config.roster_sidecar_retention;

        let blocks_root = Self::select_block_store_root(&store_dir, primary_lane);
        let mut block_store =
            BlockStore::with_fsync(&blocks_root, config.fsync_mode, config.fsync_interval);
        block_store.create_files_if_they_do_not_exist()?;

        let (block_notify_tx, block_notify_rx) = mpsc::channel();

        let block_plain_text_path = config
            .debug_output_new_blocks
            .then(|| blocks_root.join("blocks.jsonl"));

        let (block_data, chain_validation) = Kura::init(&mut block_store, config.init_mode)?;
        let block_count = block_data.len();
        info!(mode=?config.init_mode, block_count, "Kura init complete");

        let merge_log_path = Self::select_merge_log_path(&store_dir, primary_lane);
        let merge_cache_capacity =
            sanitize_merge_cache_capacity(config.merge_ledger_cache_capacity);
        let mut merge_log = MergeLedgerLog::open_at(&merge_log_path, merge_cache_capacity)?;
        let roster_log_path = Self::roster_log_path(&store_root);
        let roster_log = match CommitRosterJournal::load(roster_log_path.clone(), roster_retention)
        {
            Ok(log) => log,
            Err(err) => {
                warn!(
                    ?err,
                    path = %roster_log_path.display(),
                    "failed to load roster journal; starting empty"
                );
                CommitRosterJournal::new(roster_log_path, roster_retention)
            }
        };

        Self::ensure_lane_directories(&store_dir, lane_config, &blocks_root, &merge_log_path)?;

        if merge_log.total_entries > block_count {
            let trimmed = merge_log.total_entries - block_count;
            if chain_validation.truncated {
                info!(
                    trimmed,
                    block_count, "Pruning merge-ledger entries to match truncated block store"
                );
            } else {
                warn!(
                    trimmed,
                    block_count, "Merge-ledger log longer than block store; truncating tail"
                );
            }
            merge_log.truncate_to_len(block_count)?;
        }

        let kura = Arc::new(Self {
            block_store: Mutex::new(block_store),
            block_data: Mutex::new(block_data),
            block_notify_tx,
            block_notify_rx: Mutex::new(Some(block_notify_rx)),
            block_plain_text_path: Mutex::new(block_plain_text_path),
            sidecar_lock: Mutex::new(()),
            pipeline_sidecar_queue: Mutex::new(VecDeque::new()),
            store_root,
            active_blocks_dir: Mutex::new(blocks_root.clone()),
            active_merge_path: Mutex::new(merge_log_path.clone()),
            max_disk_usage_bytes: config.max_disk_usage_bytes.get(),
            disk_usage: AtomicU64::new(0),
            disk_usage_total: AtomicU64::new(0),
            pending_budget_bytes: AtomicU64::new(0),
            pending_budget_bytes_valid: AtomicBool::new(false),
            disk_usage_initialized: AtomicBool::new(false),
            disk_usage_total_initialized: AtomicBool::new(false),
            disk_usage_total_last_refresh: AtomicU64::new(0),
            blocks_in_memory: config.blocks_in_memory,
            block_sync_roster_retention: roster_retention,
            roster_sidecar_retention,
            init_block_count: block_count,
            merge_log: Mutex::new(merge_log),
            roster_log: Mutex::new(roster_log),
            telemetry: OnceLock::new(),
            writer_fault: Mutex::new(None),
        });

        match kura.kura_disk_usage_bytes() {
            Ok(bytes) => {
                kura.disk_usage.store(bytes, Ordering::Relaxed);
                kura.disk_usage_initialized.store(true, Ordering::Relaxed);
            }
            Err(err) => warn!(
                ?err,
                path = %kura.store_root.display(),
                "failed to measure initial Kura disk usage"
            ),
        }
        match kura.kura_total_disk_usage_bytes() {
            Ok(bytes) => {
                kura.disk_usage_total.store(bytes, Ordering::Relaxed);
                kura.disk_usage_total_initialized
                    .store(true, Ordering::Relaxed);
                kura.disk_usage_total_last_refresh
                    .store(Self::now_unix_secs(), Ordering::Relaxed);
            }
            Err(err) => warn!(
                ?err,
                path = %kura.store_root.display(),
                "failed to measure initial total Kura disk usage"
            ),
        }

        Ok((kura, BlockCount(block_count)))
    }

    /// Create a kura instance that doesn't write to disk. Instead it serves as a handler
    /// for in-memory blocks only.
    pub fn blank_kura_for_testing() -> Arc<Kura> {
        let (block_notify_tx, block_notify_rx) = mpsc::channel();
        Arc::new(Self {
            block_store: Mutex::new(BlockStore::with_fsync(
                PathBuf::new(),
                FsyncMode::Off,
                FSYNC_INTERVAL,
            )),
            block_data: Mutex::new(Vec::new()),
            block_notify_tx,
            block_notify_rx: Mutex::new(Some(block_notify_rx)),
            block_plain_text_path: Mutex::new(None),
            sidecar_lock: Mutex::new(()),
            pipeline_sidecar_queue: Mutex::new(VecDeque::new()),
            store_root: PathBuf::new(),
            active_blocks_dir: Mutex::new(PathBuf::new()),
            active_merge_path: Mutex::new(PathBuf::new()),
            max_disk_usage_bytes: MAX_DISK_USAGE_BYTES.get(),
            disk_usage: AtomicU64::new(0),
            disk_usage_total: AtomicU64::new(0),
            pending_budget_bytes: AtomicU64::new(0),
            pending_budget_bytes_valid: AtomicBool::new(false),
            disk_usage_initialized: AtomicBool::new(true),
            disk_usage_total_initialized: AtomicBool::new(true),
            disk_usage_total_last_refresh: AtomicU64::new(0),
            blocks_in_memory: BLOCKS_IN_MEMORY,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
            init_block_count: 0,
            merge_log: Mutex::new(MergeLedgerLog::in_memory(MERGE_LEDGER_CACHE_CAPACITY)),
            roster_log: Mutex::new(CommitRosterJournal::new(
                PathBuf::new(),
                BLOCK_SYNC_ROSTER_RETENTION,
            )),
            telemetry: OnceLock::new(),
            writer_fault: Mutex::new(None),
        })
    }

    /// Attach a telemetry sink for storage budget reporting.
    pub fn attach_telemetry(&self, telemetry: StateTelemetry) {
        let _ = self.telemetry.set(telemetry);
    }

    /// Root directory used by this Kura instance.
    #[must_use]
    pub fn store_root(&self) -> PathBuf {
        self.store_root.clone()
    }

    /// Return cached total on-disk bytes used by Kura (active + retired segments).
    ///
    /// This includes DA-backed payloads, which are excluded from budget accounting.
    /// Use [`Self::refresh_disk_usage_bytes`] to resync budget usage, and
    /// [`Self::refresh_total_disk_usage_bytes`] for the full on-disk view.
    pub(crate) fn disk_usage_bytes(&self) -> Result<u64> {
        self.maybe_refresh_total_disk_usage_bytes()?;
        Ok(self.disk_usage_total.load(Ordering::Relaxed))
    }

    /// Recompute on-disk bytes used by Kura and refresh the cached value.
    pub(crate) fn refresh_disk_usage_bytes(&self) -> Result<u64> {
        let usage = self.kura_disk_usage_bytes()?;
        self.disk_usage.store(usage, Ordering::Relaxed);
        self.disk_usage_initialized.store(true, Ordering::Relaxed);
        let _ = self.refresh_total_disk_usage_bytes();
        Ok(usage)
    }

    /// Recompute total on-disk bytes (including DA payloads) and refresh the cached value.
    pub(crate) fn refresh_total_disk_usage_bytes(&self) -> Result<u64> {
        let usage = self.kura_total_disk_usage_bytes()?;
        self.disk_usage_total.store(usage, Ordering::Relaxed);
        self.disk_usage_total_initialized
            .store(true, Ordering::Relaxed);
        self.disk_usage_total_last_refresh
            .store(Self::now_unix_secs(), Ordering::Relaxed);
        Ok(usage)
    }

    fn maybe_refresh_total_disk_usage_bytes(&self) -> Result<()> {
        if !self.disk_usage_total_initialized.load(Ordering::Relaxed) {
            let _ = self.refresh_total_disk_usage_bytes()?;
            return Ok(());
        }
        let last_refresh = self.disk_usage_total_last_refresh.load(Ordering::Relaxed);
        let now = Self::now_unix_secs();
        if now.saturating_sub(last_refresh) >= DISK_USAGE_TOTAL_REFRESH_INTERVAL.as_secs() {
            let _ = self.refresh_total_disk_usage_bytes()?;
        }
        Ok(())
    }

    fn ensure_disk_usage_initialized(&self) -> Result<()> {
        if self.disk_usage_initialized.load(Ordering::Relaxed) {
            return Ok(());
        }
        let _ = self.refresh_disk_usage_bytes()?;
        Ok(())
    }

    /// Update the cached disk usage by applying a before/after delta.
    pub(crate) fn update_disk_usage_delta(&self, before: u64, after: u64) {
        if before == after {
            return;
        }
        if after > before {
            self.add_disk_usage_bytes(after - before);
        } else {
            self.sub_disk_usage_bytes(before - after);
        }
    }

    fn update_total_disk_usage_delta(&self, before: u64, after: u64) {
        if before == after {
            return;
        }
        if after > before {
            self.add_total_disk_usage_bytes(after - before);
        } else {
            self.sub_total_disk_usage_bytes(before - after);
        }
    }

    fn add_disk_usage_bytes(&self, delta: u64) {
        if delta == 0 {
            return;
        }
        let _ = self
            .disk_usage
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_add(delta))
            });
        self.add_total_disk_usage_bytes(delta);
    }

    fn sub_disk_usage_bytes(&self, delta: u64) {
        if delta == 0 {
            return;
        }
        let _ = self
            .disk_usage
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_sub(delta))
            });
        self.sub_total_disk_usage_bytes(delta);
    }

    fn add_total_disk_usage_bytes(&self, delta: u64) {
        if delta == 0 || !self.disk_usage_total_initialized.load(Ordering::Relaxed) {
            return;
        }
        let _ =
            self.disk_usage_total
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    Some(current.saturating_add(delta))
                });
    }

    fn sub_total_disk_usage_bytes(&self, delta: u64) {
        if delta == 0 || !self.disk_usage_total_initialized.load(Ordering::Relaxed) {
            return;
        }
        let _ =
            self.disk_usage_total
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    Some(current.saturating_sub(delta))
                });
    }

    /// Attempt to purge retired Kura segments to reclaim disk budget.
    pub(crate) fn purge_retired_segments(&self) -> bool {
        self.purge_retired_storage()
    }

    fn invalidate_pending_budget_cache(&self) {
        self.pending_budget_bytes_valid
            .store(false, Ordering::Relaxed);
    }

    fn add_pending_budget_bytes(&self, delta: u64) {
        if delta == 0 || !self.pending_budget_bytes_valid.load(Ordering::Relaxed) {
            return;
        }
        let _ = self.pending_budget_bytes.fetch_update(
            Ordering::Relaxed,
            Ordering::Relaxed,
            |current| Some(current.saturating_add(delta)),
        );
    }

    fn sub_pending_budget_bytes(&self, delta: u64) {
        if delta == 0 || !self.pending_budget_bytes_valid.load(Ordering::Relaxed) {
            return;
        }
        let _ = self.pending_budget_bytes.fetch_update(
            Ordering::Relaxed,
            Ordering::Relaxed,
            |current| Some(current.saturating_sub(delta)),
        );
    }

    fn record_writer_fault(&self, context: &'static str, error: &Error) {
        {
            let mut fault = self.writer_fault.lock();
            if fault.is_none() {
                *fault = Some(format!("{context}: {error}"));
            }
        }
        if let Some(telemetry) = self.telemetry.get() {
            telemetry.inc_storage_budget_exceeded("kura_writer_fault");
        }
    }

    fn ensure_writer_healthy(&self) -> Result<()> {
        let fault = self.writer_fault.lock();
        if let Some(reason) = fault.as_ref() {
            return Err(Error::BlockWriterFaulted(reason.clone()));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn mark_writer_fault_for_tests(&self, reason: impl Into<String>) {
        *self.writer_fault.lock() = Some(reason.into());
    }

    /// Evict persisted block bodies into DA-backed storage to reclaim disk budget.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn evict_block_bodies(&self, bytes_needed: u64) -> Result<u64> {
        if bytes_needed == 0 || self.store_root.as_os_str().is_empty() {
            return Ok(0);
        }

        let mut block_store = self.block_store.lock();
        if let Err(err) = block_store.flush_pending_fsync(true) {
            warn!(?err, "failed to flush pending Kura writes before eviction");
        }

        let persisted = usize::try_from(block_store.read_durable_index_count()?)?;
        if persisted <= 1 {
            return Ok(0);
        }
        let retain_tail = self.blocks_in_memory.get().max(1);
        let evict_limit = persisted.saturating_sub(retain_tail);
        if evict_limit <= 1 {
            return Ok(0);
        }

        let mut indices = vec![BlockIndex::default(); persisted];
        block_store.read_block_indices(0, &mut indices)?;

        let mut evict_mask = vec![false; persisted];
        let mut freed = 0u64;
        for idx in 1..evict_limit {
            let entry = indices[idx];
            if entry.is_evicted() {
                continue;
            }
            freed = freed.saturating_add(entry.length);
            evict_mask[idx] = true;
            if freed >= bytes_needed {
                break;
            }
        }

        if freed == 0 {
            return Ok(0);
        }

        let before_bytes = Self::block_store_tracked_bytes(&mut block_store)?;
        let mut da_added = 0u64;

        block_store.ensure_da_blocks_dir()?;
        let mut buffer = Vec::new();
        for idx in 1..evict_limit {
            if !evict_mask[idx] {
                continue;
            }
            let entry = indices[idx];
            if entry.is_evicted() {
                continue;
            }
            let height = idx.saturating_add(1) as u64;
            let path = block_store.da_block_path(height);
            if path.exists() {
                continue;
            }
            let length: usize = entry.length.try_into()?;
            buffer.resize(length, 0);
            block_store.read_block_data(entry.start, &mut buffer)?;
            let tmp_path = path.with_extension("norito.tmp");
            let mut tmp_file = FileWrap::open_with(tmp_path.clone(), |opts| {
                opts.write(true).create(true).truncate(true);
            })?;
            tmp_file.try_io(|file| {
                file.write_all(&buffer)?;
                file.flush()?;
                file.sync_data()
            })?;
            std::fs::rename(&tmp_path, &path).map_err(|err| Error::IO(err, path.clone()))?;
            if let Some(parent) = path.parent() {
                sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
            }
            da_added = da_added.saturating_add(entry.length);
        }

        let data_path = block_store.path_to_blockchain.join(DATA_FILE_NAME);
        let data_tmp = block_store
            .path_to_blockchain
            .join(format!("{DATA_FILE_NAME}.tmp"));
        let index_path = block_store.path_to_blockchain.join(INDEX_FILE_NAME);
        let index_tmp = block_store
            .path_to_blockchain
            .join(format!("{INDEX_FILE_NAME}.tmp"));

        let mut new_indices = indices.clone();
        let mut cursor = 0u64;
        let mut data_tmp_file = FileWrap::open_with(data_tmp.clone(), |opts| {
            opts.write(true).create(true).truncate(true);
        })?;
        for (idx, entry) in indices.iter().enumerate() {
            if entry.is_evicted() || evict_mask[idx] {
                new_indices[idx].start = EVICTED_BLOCK_START;
                continue;
            }
            let length: usize = entry.length.try_into()?;
            buffer.resize(length, 0);
            block_store.read_block_data(entry.start, &mut buffer)?;
            data_tmp_file.try_io(|file| {
                file.seek(SeekFrom::Start(cursor))?;
                file.write_all(&buffer)
            })?;
            new_indices[idx].start = cursor;
            cursor = cursor.saturating_add(entry.length);
        }
        data_tmp_file.try_io(|file| {
            file.flush()?;
            file.set_len(cursor)?;
            file.sync_data()
        })?;

        let mut index_tmp_file = FileWrap::open_with(index_tmp.clone(), |opts| {
            opts.write(true).create(true).truncate(true);
        })?;
        let index_len = u64::try_from(persisted)?.saturating_mul(BlockIndex::SIZE);
        index_tmp_file.try_io(|file| {
            for entry in &new_indices {
                let bytes = (*entry).encode();
                file.write_all(&bytes)?;
            }
            file.flush()?;
            file.set_len(index_len)?;
            file.sync_data()
        })?;

        std::fs::rename(&data_tmp, &data_path).map_err(|err| Error::IO(err, data_path.clone()))?;
        std::fs::rename(&index_tmp, &index_path)
            .map_err(|err| Error::IO(err, index_path.clone()))?;
        if let Some(parent) = data_path.parent() {
            sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
        }

        block_store.fsync.clear();
        block_store.drop_cached_handles();
        let after_bytes = Self::block_store_tracked_bytes(&mut block_store)?;
        self.update_disk_usage_delta(before_bytes, after_bytes);
        self.add_total_disk_usage_bytes(da_added);

        if freed > 0 {
            if let Some(telemetry) = self.telemetry.get() {
                telemetry.add_storage_da_churn_bytes("kura", "evicted", freed);
            }
        }

        Ok(freed)
    }

    /// Retention window for commit-roster snapshots.
    #[must_use]
    pub fn block_sync_roster_retention(&self) -> NonZeroUsize {
        self.block_sync_roster_retention
    }

    /// Retention window for roster sidecars stored alongside blocks.
    #[must_use]
    pub fn roster_sidecar_retention(&self) -> NonZeroUsize {
        self.roster_sidecar_retention
    }

    fn select_block_store_root(store_dir: &Path, lane: &LaneConfigEntry) -> PathBuf {
        lane.blocks_dir(store_dir)
    }

    fn roster_log_path(store_root: &Path) -> PathBuf {
        CommitRosterJournal::journal_path(store_root)
    }

    fn select_merge_log_path(store_dir: &Path, lane: &LaneConfigEntry) -> PathBuf {
        lane.merge_log_path(store_dir)
    }

    fn ensure_lane_directories(
        store_dir: &Path,
        lane_config: &LaneConfig,
        active_blocks_dir: &Path,
        active_merge_path: &Path,
    ) -> Result<()> {
        for entry in lane_config.entries() {
            let blocks_dir = entry.blocks_dir(store_dir);
            if blocks_dir.as_path() != active_blocks_dir {
                std::fs::create_dir_all(&blocks_dir)
                    .map_err(|err| Error::MkDir(err, blocks_dir.clone()))?;
            }

            let merge_path = entry.merge_log_path(store_dir);
            if merge_path.as_path() != active_merge_path {
                if let Some(parent) = merge_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|err| Error::MkDir(err, parent.to_path_buf()))?;
                }
                if !merge_path.exists() {
                    FileWrap::open_with(merge_path.clone(), |opts| {
                        opts.read(true).write(true).create(true).append(true);
                    })?;
                }
            }
        }

        Ok(())
    }

    /// Reconcile lane storage topology, provisioning directories for new lanes and
    /// archiving storage for retired lanes.
    ///
    /// # Errors
    /// Returns an [`Error`] if preparing or retiring storage directories fails for any entry.
    pub fn reconcile_lane_segments(
        &self,
        added: &[&LaneConfigEntry],
        retired: &[&LaneConfigEntry],
    ) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        let mut changed = false;
        for entry in added {
            self.prepare_lane_storage(entry)?;
            changed = true;
        }

        for entry in retired {
            self.retire_lane_storage(entry)?;
            changed = true;
        }

        if changed {
            if let Err(err) = self.refresh_disk_usage_bytes() {
                warn!(
                    ?err,
                    "failed to refresh disk usage after lane reconciliation"
                );
            }
        }

        Ok(())
    }

    /// Rename lane storage directories when lane aliases change.
    ///
    /// The migrations slice pairs the previous and current lane entries; each
    /// pair triggers a best-effort `std::fs::rename` so block data continues to
    /// live under the new alias without re-syncing from peers. Missing or
    /// conflicting directories are logged and skipped so existing data is not
    /// destroyed.
    /// Move per-lane block directories when a lane's configuration alias changes.
    ///
    /// Each tuple in `migrations` pairs the previous configuration entry with the
    /// new one. When the underlying storage path differs we rename the directory,
    /// creating the new parent path if needed.
    ///
    /// # Errors
    /// Returns an error if preparing lane storage, creating directories, renaming,
    /// or retargeting the block store fails.
    pub fn relabel_lane_segments(
        &self,
        migrations: &[(&LaneConfigEntry, &LaneConfigEntry)],
    ) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        for (previous, current) in migrations {
            if previous.kura_segment == current.kura_segment {
                continue;
            }
            let old_dir = previous.blocks_dir(&self.store_root);
            let new_dir = current.blocks_dir(&self.store_root);
            if old_dir == new_dir {
                continue;
            }
            if !old_dir.exists() {
                self.prepare_lane_storage(current)?;
                continue;
            }
            if let Some(parent) = new_dir.parent() {
                create_dir_all_with_context(parent)?;
            }
            if new_dir.exists() {
                iroha_logger::warn!(
                    old = %old_dir.display(),
                    new = %new_dir.display(),
                    "lane storage relabel skipped because target directory already exists"
                );
                continue;
            }
            std::fs::rename(&old_dir, &new_dir).map_err(|err| Error::IO(err, old_dir.clone()))?;
            let new_parent = new_dir.parent();
            let old_parent = old_dir.parent();
            if let Some(parent) = new_parent {
                sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
            }
            if let Some(parent) = old_parent {
                if Some(parent) != new_parent {
                    sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
                }
            }

            {
                let mut plain_text = self.block_plain_text_path.lock();
                if let Some(path) = plain_text.as_mut() {
                    if let Ok(suffix) = path.strip_prefix(&old_dir) {
                        *path = new_dir.join(suffix);
                    }
                }
            }

            {
                let mut active_dir = self.active_blocks_dir.lock();
                if *active_dir == old_dir {
                    active_dir.clone_from(&new_dir);
                    self.block_store.lock().retarget_path(new_dir.clone())?;
                }
            }

            self.relabel_merge_log(previous, current)?;

            iroha_logger::info!(
                lane = %current.lane_id.as_u32(),
                alias_before = previous.alias,
                alias_after = current.alias,
                source = %old_dir.display(),
                target = %new_dir.display(),
                "lane storage relabelled to match updated alias"
            );
        }

        Ok(())
    }

    fn relabel_merge_log(
        &self,
        previous: &LaneConfigEntry,
        current: &LaneConfigEntry,
    ) -> Result<()> {
        let old_path = previous.merge_log_path(&self.store_root);
        let new_path = current.merge_log_path(&self.store_root);
        if old_path == new_path {
            return Ok(());
        }

        if !old_path.exists() {
            if let Some(parent) = new_path.parent() {
                create_dir_all_with_context(parent)?;
            }
            if !new_path.exists() {
                FileWrap::open_with(new_path.clone(), |opts| {
                    opts.read(true).write(true).create(true).append(true);
                })?;
            }
            return Ok(());
        }

        if let Some(parent) = new_path.parent() {
            create_dir_all_with_context(parent)?;
        }
        if new_path.exists() {
            let retired_root = self.store_root.join("retired").join("merge_ledger");
            create_dir_all_with_context(&retired_root)?;
            let dest = unique_retired_path(&retired_root, &current.merge_segment, Some("log"));
            std::fs::rename(&new_path, &dest).map_err(|err| Error::IO(err, new_path.clone()))?;
            let dest_parent = dest.parent();
            let new_parent = new_path.parent();
            if let Some(parent) = dest_parent {
                sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
            }
            if let Some(parent) = new_parent {
                if Some(parent) != dest_parent {
                    sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
                }
            }
            iroha_logger::info!(
                lane = %current.lane_id.as_u32(),
                alias = current.alias,
                source = %new_path.display(),
                target = %dest.display(),
                "archived conflicting merge-ledger log before relabel"
            );
        }

        std::fs::rename(&old_path, &new_path).map_err(|err| Error::IO(err, old_path.clone()))?;
        let new_parent = new_path.parent();
        let old_parent = old_path.parent();
        if let Some(parent) = new_parent {
            sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
        }
        if let Some(parent) = old_parent {
            if Some(parent) != new_parent {
                sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
            }
        }

        {
            let mut active_merge = self.active_merge_path.lock();
            if *active_merge == old_path {
                active_merge.clone_from(&new_path);
            }
        }

        iroha_logger::info!(
            lane = %current.lane_id.as_u32(),
            alias_before = previous.alias,
            alias_after = current.alias,
            source = %old_path.display(),
            target = %new_path.display(),
            "lane merge-ledger relabelled"
        );

        Ok(())
    }

    fn prepare_lane_storage(&self, entry: &LaneConfigEntry) -> Result<()> {
        let blocks_dir = entry.blocks_dir(&self.store_root);
        let mut block_store = BlockStore::new(&blocks_dir);
        block_store.create_files_if_they_do_not_exist()?;

        let merge_path = entry.merge_log_path(&self.store_root);
        if let Some(parent) = merge_path.parent() {
            create_dir_all_with_context(parent)?;
        }
        if !merge_path.exists() {
            FileWrap::open_with(merge_path.clone(), |opts| {
                opts.read(true).write(true).create(true).append(true);
            })?;
        }

        iroha_logger::info!(
            lane = %entry.lane_id.as_u32(),
            alias = entry.alias,
            blocks = %blocks_dir.display(),
            merge = %merge_path.display(),
            "lane storage provisioned"
        );
        Ok(())
    }

    fn retire_lane_storage(&self, entry: &LaneConfigEntry) -> Result<()> {
        let blocks_dir = entry.blocks_dir(&self.store_root);
        {
            let active_dir = self.active_blocks_dir.lock();
            if blocks_dir == *active_dir {
                iroha_logger::warn!(
                    lane = %entry.lane_id.as_u32(),
                    alias = entry.alias,
                    "skipping retirement of active lane block directory"
                );
                return Ok(());
            }
        }
        let merge_path = entry.merge_log_path(&self.store_root);
        if merge_path == *self.active_merge_path.lock() {
            iroha_logger::warn!(
                lane = %entry.lane_id.as_u32(),
                alias = entry.alias,
                "skipping retirement of active lane merge log"
            );
            return Ok(());
        }

        let retired_root = self.store_root.join("retired");
        let retired_blocks_root = retired_root.join("blocks");
        let retired_merge_root = retired_root.join("merge_ledger");
        create_dir_all_with_context(&retired_blocks_root)?;
        create_dir_all_with_context(&retired_merge_root)?;

        if blocks_dir.exists() {
            let dest = unique_retired_path(&retired_blocks_root, &entry.kura_segment, None::<&str>);
            std::fs::rename(&blocks_dir, &dest)
                .map_err(|err| Error::IO(err, blocks_dir.clone()))?;
            let dest_parent = dest.parent();
            let blocks_parent = blocks_dir.parent();
            if let Some(parent) = dest_parent {
                sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
            }
            if let Some(parent) = blocks_parent {
                if Some(parent) != dest_parent {
                    sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
                }
            }
            iroha_logger::info!(
                lane = %entry.lane_id.as_u32(),
                alias = entry.alias,
                source = %blocks_dir.display(),
                target = %dest.display(),
                "retired lane block directory"
            );
        }

        if merge_path.exists() {
            let dest = unique_retired_path(&retired_merge_root, &entry.merge_segment, Some("log"));
            std::fs::rename(&merge_path, &dest)
                .map_err(|err| Error::IO(err, merge_path.clone()))?;
            let dest_parent = dest.parent();
            let merge_parent = merge_path.parent();
            if let Some(parent) = dest_parent {
                sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
            }
            if let Some(parent) = merge_parent {
                if Some(parent) != dest_parent {
                    sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
                }
            }
            iroha_logger::info!(
                lane = %entry.lane_id.as_u32(),
                alias = entry.alias,
                source = %merge_path.display(),
                target = %dest.display(),
                "retired lane merge log"
            );
        }

        Ok(())
    }

    /// Append a merge-ledger entry to the persistent log and in-memory cache.
    ///
    /// # Errors
    /// Returns an error if the merge-ledger entry cannot be persisted.
    pub fn append_merge_entry(&self, entry: &MergeLedgerEntry) -> Result<()> {
        self.merge_log.lock().append(entry)?;
        if !self.store_root.as_os_str().is_empty() {
            let bytes = Self::merge_entry_bytes(entry)?;
            self.add_disk_usage_bytes(bytes);
        }
        Ok(())
    }

    /// Snapshot merge-ledger entries retained in the in-memory cache.
    pub fn merge_ledger_snapshot(&self) -> Vec<MergeLedgerEntry> {
        self.merge_log.lock().snapshot()
    }

    fn truncate_merge_log_to_len(&self, keep: usize) -> Result<()> {
        let before = self.merge_log_tracked_bytes()?;
        self.merge_log.lock().truncate_to_len(keep)?;
        let after = self.merge_log_tracked_bytes()?;
        self.update_disk_usage_delta(before, after);
        Ok(())
    }

    /// Start a thread that receives and stores new blocks
    pub fn start(kura: Arc<Self>, shutdown_signal: ShutdownSignal) -> Child {
        let shutdown_notify_tx = kura.block_notify_tx.clone();
        let shutdown_signal_clone = shutdown_signal.clone();
        tokio::spawn(async move {
            shutdown_signal_clone.receive().await;
            let _ = shutdown_notify_tx.send(BlockNotify::Shutdown);
        });

        Child::new(
            tokio::task::spawn(spawn_os_thread_as_future(
                std::thread::Builder::new().name("kura".to_owned()),
                move || {
                    kura.receive_blocks_loop(&shutdown_signal);
                },
            )),
            OnShutdown::Wait(Duration::from_secs(5)),
        )
    }

    /// Initialize [`Kura`] after its construction to be able to work with it.
    ///
    /// # Errors
    /// Fails if:
    /// - file storage is unavailable
    /// - data in file storage is invalid or corrupted
    #[iroha_logger::log(skip_all, name = "kura_init")]
    fn init(block_store: &mut BlockStore, mode: InitMode) -> Result<(BlockData, ChainValidation)> {
        let block_index_count: usize = block_store
            .read_durable_index_count()?
            .try_into()
            .expect("INTERNAL BUG: block index count exceeds usize::MAX");

        let chain_validation = match mode {
            InitMode::Fast => {
                Kura::init_fast_mode(block_store, block_index_count).or_else(|error| {
                    warn!(%error, "Hashes file is broken. Falling back to strict init mode.");
                    Kura::init_strict_mode(block_store, block_index_count)
                })
            }
            InitMode::Strict => Kura::init_strict_mode(block_store, block_index_count),
        }?;

        if chain_validation.truncated {
            warn!(
                validated_blocks = chain_validation.hashes.len(),
                "Kura detected corrupted storage during init and pruned to the last valid block"
            );
        }
        if chain_validation.hash_mismatch {
            warn!("Kura rewrote hashes file after detecting mismatches with on-disk blocks");
        }

        // The none value is set in order to indicate that the blocks exist on disk but are not yet loaded.
        let block_data = chain_validation
            .hashes
            .iter()
            .copied()
            .map(|hash| (hash, None))
            .collect();
        Ok((block_data, chain_validation))
    }

    fn init_fast_mode(
        block_store: &mut BlockStore,
        block_index_count: usize,
    ) -> Result<ChainValidation, Error> {
        let mut block_hashes_count: usize = block_store
            .read_hashes_count()?
            .try_into()
            .expect("INTERNAL BUG: block hashes count exceeds usize::MAX");
        if block_hashes_count > block_index_count {
            warn!(
                hashes_count = block_hashes_count,
                index_count = block_index_count,
                "hashes file longer than index; truncating to durable count"
            );
            block_store.truncate_hashes_to_count(block_index_count as u64)?;
            block_hashes_count = block_index_count;
        }
        if block_hashes_count == block_index_count {
            let mut block_indices = vec![BlockIndex::default(); block_index_count];
            block_store.read_block_indices(0, &mut block_indices)?;
            let expected_hashes = block_store.read_block_hashes(0, block_hashes_count)?;
            let validation =
                Self::validate_block_chain(block_store, &block_indices, Some(&expected_hashes))?;
            if validation.truncated || validation.hash_mismatch {
                block_store.overwrite_block_hashes(&validation.hashes)?;
            }
            Ok(validation)
        } else {
            Err(Error::HashesFileHeightMismatch)
        }
    }

    fn init_strict_mode(
        block_store: &mut BlockStore,
        block_index_count: usize,
    ) -> Result<ChainValidation, Error> {
        let mut block_indices = vec![BlockIndex::default(); block_index_count];
        block_store.read_block_indices(0, &mut block_indices)?;

        let validation = Self::validate_block_chain(block_store, &block_indices, None)?;
        block_store.overwrite_block_hashes(&validation.hashes)?;

        Ok(validation)
    }

    #[allow(clippy::too_many_lines)]
    fn validate_block_chain(
        block_store: &mut BlockStore,
        block_indices: &[BlockIndex],
        expected_hashes: Option<&[HashOf<BlockHeader>]>,
    ) -> Result<ChainValidation, Error> {
        if let Some(expected) = expected_hashes {
            if expected.len() != block_indices.len() {
                return Err(Error::HashesFileHeightMismatch);
            }
        }

        let mut block_hashes = Vec::with_capacity(block_indices.len());
        let mut block_data_buffer = Vec::new();
        let mut prev_block_hash = None;
        let data_file_len = block_store.data_file_len()?;
        let mut truncated = None;
        let mut hash_mismatch = false;

        for (idx, block) in block_indices.iter().enumerate() {
            let height = idx.saturating_add(1) as u64;
            if block.length == 0 {
                truncated = Some(true);
                error!(
                    length = block.length,
                    limit = STRICT_INIT_MAX_BLOCK_BYTES,
                    "Encountered zero-length block entry; pruning to last valid block"
                );
                break;
            }
            if block.length > STRICT_INIT_MAX_BLOCK_BYTES {
                truncated = Some(true);
                error!(
                    length = block.length,
                    limit = STRICT_INIT_MAX_BLOCK_BYTES,
                    "Encountered oversized block entry; pruning to last valid block"
                );
                break;
            }
            let decoded_block =
                if block.is_evicted() {
                    let payload = match block_store.read_da_block_bytes(height, block.length) {
                        Ok(payload) => payload,
                        Err(error) => {
                            truncated = Some(true);
                            error!(
                                ?error,
                                block_index = idx,
                                height,
                                "Failed to read evicted block payload; pruning to last valid block"
                            );
                            break;
                        }
                    };
                    match decode_framed_signed_block(&payload) {
                        Ok(decoded_block) => decoded_block,
                        Err(error) => {
                            truncated = Some(true);
                            error!(
                                ?error,
                                block_index = idx,
                                height,
                                "Malformed evicted block payload; pruning to last valid block"
                            );
                            break;
                        }
                    }
                } else {
                    let end = block.start.checked_add(block.length).ok_or(
                        Error::CorruptedBlockRange {
                            start: block.start,
                            length: block.length,
                            data_len: data_file_len,
                        },
                    )?;
                    if end > data_file_len {
                        truncated = Some(true);
                        error!(
                            start = block.start,
                            length = block.length,
                            data_len = data_file_len,
                            "Block index points past data file; pruning to last valid block"
                        );
                        break;
                    }
                    let length: usize = block.length.try_into()?;
                    let additional = length.saturating_sub(block_data_buffer.len());
                    if additional > 0 {
                        block_data_buffer.try_reserve(additional)?;
                    }
                    block_data_buffer.resize(length, 0);

                    match block_store.read_block_data(block.start, &mut block_data_buffer) {
                        Ok(()) => match decode_framed_signed_block(&block_data_buffer) {
                            Ok(decoded_block) => decoded_block,
                            Err(error) => {
                                truncated = Some(true);
                                error!(
                                    ?error,
                                    block_index = idx,
                                    "Malformed block payload; pruning to last valid block"
                                );
                                break;
                            }
                        },
                        Err(error) => {
                            truncated = Some(true);
                            error!(
                                ?error,
                                block_index = idx,
                                "Failed to read block payload; pruning to last valid block"
                            );
                            break;
                        }
                    }
                };

            if prev_block_hash != decoded_block.header().prev_block_hash() {
                truncated = Some(true);
                error!(
                    expected = ?prev_block_hash,
                    actual = ?decoded_block.header().prev_block_hash(),
                    block_index = idx,
                    "Previous block hash mismatch; pruning to last valid block"
                );
                break;
            }

            let decoded_block_hash = decoded_block.hash();
            if let Some(expected_hashes) = expected_hashes {
                let expected = expected_hashes[idx];
                if expected != decoded_block_hash {
                    hash_mismatch = true;
                    warn!(
                        expected = ?expected,
                        actual = ?decoded_block_hash,
                        block_index = idx,
                        "Block hash file entry mismatched decoded block; rewriting hashes file"
                    );
                }
            }

            prev_block_hash = Some(decoded_block_hash);
            block_hashes.push(decoded_block_hash);
        }

        let truncated = truncated.unwrap_or(false);
        let validated_height = block_hashes.len() as u64;
        if truncated {
            block_store.prune(validated_height)?;
            info!(
                validated_height,
                "Pruned Kura storage to last validated block after detecting corruption"
            );
        }

        Ok(ChainValidation {
            hashes: block_hashes,
            truncated,
            hash_mismatch,
        })
    }

    #[iroha_logger::log(skip_all)]
    #[allow(clippy::too_many_lines)]
    fn receive_blocks_loop(&self, shutdown_signal: &ShutdownSignal) {
        let kura = self;
        let mut written_block_count = kura.init_block_count;
        let mut latest_written_block_hash = {
            let block_data = kura.block_data.lock();
            written_block_count
                .checked_sub(1)
                .map(|idx| block_data[idx].0)
        };

        let block_rx = kura
            .block_notify_rx
            .lock()
            .take()
            .expect("Kura writer thread already started");

        let mut should_exit = false;
        loop {
            // If kura receive shutdown then close block channel and write remaining blocks to the storage
            if shutdown_signal.is_sent() {
                info!("Kura block thread is being shut down. Writing remaining blocks to store.");
                should_exit = true;
            }

            kura.flush_pipeline_sidecars();

            let block_data = kura.block_data.lock();
            let in_memory_len = block_data.len();

            if written_block_count > in_memory_len {
                warn!(
                    written_block_count,
                    in_memory_len,
                    "kura writer detected in-memory chain shrink; rewinding write cursor"
                );
                written_block_count = in_memory_len;
                kura.invalidate_pending_budget_cache();
                latest_written_block_hash = written_block_count
                    .checked_sub(1)
                    .and_then(|idx| block_data.get(idx).map(|entry| entry.0));
            } else {
                let new_latest_written_block_hash = written_block_count
                    .checked_sub(1)
                    .and_then(|idx| block_data.get(idx).map(|entry| entry.0));
                if new_latest_written_block_hash != latest_written_block_hash {
                    written_block_count = written_block_count.saturating_sub(1); // soft-fork rewrite
                    kura.invalidate_pending_budget_cache();
                }
                latest_written_block_hash = written_block_count
                    .checked_sub(1)
                    .and_then(|idx| block_data.get(idx).map(|entry| entry.0));
            }

            if written_block_count >= block_data.len() {
                if should_exit {
                    if let Err(error) = kura.block_store.lock().flush_pending_fsync(true) {
                        error!(?error, "Failed to fsync pending blocks on shutdown");
                        kura.record_writer_fault("shutdown fsync", &error);
                        return;
                    }
                    info!("Kura has written remaining blocks to disk and is shutting down.");
                    return;
                }

                written_block_count = block_data.len();
                drop(block_data);
                let wait_for_fsync = {
                    let guard = kura.block_store.lock();
                    guard.next_fsync_wait()
                };
                match wait_for_fsync {
                    Some(wait) => match block_rx.recv_timeout(wait) {
                        Ok(BlockNotify::NewBlock) => {
                            debug!(written_block_count, "kura writer received new block signal");
                            continue;
                        }
                        Ok(BlockNotify::Shutdown) => {
                            should_exit = true;
                            debug!("kura writer received shutdown signal");
                            continue;
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            let mut store = kura.block_store.lock();
                            if let Err(error) = store.flush_pending_fsync(false) {
                                error!(?error, "Failed to fsync pending batch");
                                kura.record_writer_fault("periodic fsync", &error);
                                return;
                            }
                            continue;
                        }
                        Err(RecvTimeoutError::Disconnected) => {
                            info!("Block writer channel closed; exiting thread.");
                            return;
                        }
                    },
                    None => match block_rx.recv() {
                        Ok(BlockNotify::NewBlock) => {
                            debug!(written_block_count, "kura writer received new block signal");
                            continue;
                        }
                        Ok(BlockNotify::Shutdown) => {
                            should_exit = true;
                            debug!("kura writer received shutdown signal");
                            continue;
                        }
                        Err(error) => {
                            info!(?error, "Block writer channel closed; exiting thread.");
                            return;
                        }
                    },
                }
            }

            // If we get here there are blocks to be written.
            let start_height = written_block_count;
            let mut blocks_to_be_written = Vec::new();
            while written_block_count < block_data.len() {
                let block_ref = block_data[written_block_count].1.as_ref().expect(
                    "INTERNAL BUG: The block to be written is None. Check store_block function.",
                );
                blocks_to_be_written.push(Arc::clone(block_ref));
                written_block_count += 1;
            }

            // We don't want to hold up other threads so we drop the lock on the block data.
            drop(block_data);

            let start_height_u64 = u64::try_from(start_height).expect("start height fits in u64");

            if let Some(path) = kura.block_plain_text_path.lock().clone() {
                let debug_before = match Self::file_len_or_zero(&path) {
                    Ok(bytes) => Some(bytes),
                    Err(err) => {
                        warn!(
                            ?err,
                            path = %path.display(),
                            "failed to measure debug block dump before append"
                        );
                        None
                    }
                };
                if let Err(error) = Self::append_blocks_jsonl(&path, &blocks_to_be_written) {
                    warn!(
                        ?error,
                        path = %path.display(),
                        "Failed to append debug block dump"
                    );
                }
                if let Some(debug_before) = debug_before {
                    match Self::file_len_or_zero(&path) {
                        Ok(debug_after) => {
                            kura.update_disk_usage_delta(debug_before, debug_after);
                        }
                        Err(err) => warn!(
                            ?err,
                            path = %path.display(),
                            "failed to measure debug block dump after append"
                        ),
                    }
                }
            }

            debug!(
                start_height,
                batch_len = blocks_to_be_written.len(),
                in_memory_len,
                "kura writer processing block batch"
            );
            let lock_start = Instant::now();
            let mut block_store_guard = kura.block_store.lock();
            let block_store_before = match Self::block_store_tracked_bytes(&mut block_store_guard) {
                Ok(bytes) => Some(bytes),
                Err(err) => {
                    warn!(?err, "failed to measure block store bytes before append");
                    None
                }
            };
            let da_before = if kura.disk_usage_total_initialized.load(Ordering::Relaxed) {
                match Self::da_payload_bytes_for_range(
                    &block_store_guard,
                    start_height_u64,
                    blocks_to_be_written.len(),
                ) {
                    Ok(bytes) => Some(bytes),
                    Err(err) => {
                        warn!(?err, "failed to measure DA payload bytes before append");
                        None
                    }
                }
            } else {
                None
            };
            let lock_ms = u64::try_from(lock_start.elapsed().as_millis()).unwrap_or(u64::MAX);
            debug!(
                start_height,
                batch_len = blocks_to_be_written.len(),
                lock_ms,
                "kura writer acquired block_store for batch"
            );
            let end_height = start_height + blocks_to_be_written.len();
            let append_start = Instant::now();
            if let Err(error) = block_store_guard.append_block_batch_at(
                start_height_u64,
                &blocks_to_be_written,
                kura.max_disk_usage_bytes,
            ) {
                error!(?error, "Failed to store block batch");
                kura.record_writer_fault("append block batch", &error);
                return;
            }
            let append_ms = u64::try_from(append_start.elapsed().as_millis()).unwrap_or(u64::MAX);
            let index_start = Instant::now();
            if let Err(error) = block_store_guard.write_index_count(end_height as u64) {
                error!(
                    ?error,
                    "Failed to update index count after persisting block batch"
                );
                kura.record_writer_fault("write index count", &error);
                return;
            }
            let index_ms = u64::try_from(index_start.elapsed().as_millis()).unwrap_or(u64::MAX);
            let fsync_start = Instant::now();
            if let Err(error) = block_store_guard.flush_pending_fsync(false) {
                error!(?error, "Failed to fsync persisted block batch");
                kura.record_writer_fault("batch fsync", &error);
                return;
            }
            match blocks_to_be_written.iter().try_fold(0u64, |acc, block| {
                Self::block_required_bytes_for_budget(block.as_ref(), kura.max_disk_usage_bytes)
                    .map(|required| acc.saturating_add(required))
            }) {
                Ok(persisted_pending_bytes) => {
                    kura.sub_pending_budget_bytes(persisted_pending_bytes)
                }
                Err(err) => {
                    warn!(?err, "failed to update pending budget bytes after persist");
                    kura.invalidate_pending_budget_cache();
                }
            }
            let fsync_ms = u64::try_from(fsync_start.elapsed().as_millis()).unwrap_or(u64::MAX);
            debug!(
                start_height,
                end_height,
                batch_len = blocks_to_be_written.len(),
                lock_ms,
                append_ms,
                index_ms,
                fsync_ms,
                "persisted block batch to disk"
            );
            if let Some(block_store_before) = block_store_before {
                match Self::block_store_tracked_bytes(&mut block_store_guard) {
                    Ok(after_bytes) => {
                        kura.update_disk_usage_delta(block_store_before, after_bytes)
                    }
                    Err(err) => warn!(?err, "failed to measure block store bytes after append"),
                }
            }
            if let Some(da_before) = da_before {
                match Self::da_payload_bytes_for_range(
                    &block_store_guard,
                    start_height_u64,
                    blocks_to_be_written.len(),
                ) {
                    Ok(da_after) => kura.update_total_disk_usage_delta(da_before, da_after),
                    Err(err) => warn!(?err, "failed to measure DA payload bytes after append"),
                }
            }
            latest_written_block_hash = blocks_to_be_written.last().map(|block| block.hash());
            {
                let mut block_data = kura.block_data.lock();
                Self::drop_persisted_blocks(
                    &mut block_data,
                    end_height,
                    kura.blocks_in_memory.get(),
                );
            }
        }
    }

    /// Get the hash of the block at the provided height.
    pub fn get_block_hash(&self, block_height: NonZeroUsize) -> Option<HashOf<BlockHeader>> {
        let hash_data_guard = self.block_data.lock();

        let block_height = block_height.get();
        if hash_data_guard.len() < block_height {
            return None;
        }

        let block_index = block_height - 1;
        Some(hash_data_guard[block_index].0)
    }

    /// Search through blocks for the height of the block with the given hash.
    pub fn get_block_height_by_hash(&self, hash: HashOf<BlockHeader>) -> Option<NonZeroUsize> {
        self.block_data
            .lock()
            .iter()
            .position(|(block_hash, _block_arc)| *block_hash == hash)
            .and_then(|idx| idx.checked_add(1))
            .and_then(NonZeroUsize::new)
    }

    /// Get a reference to block by height, loading it from disk if needed.
    pub fn get_block(&self, block_height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
        let (block_index, expected_hash, should_cache) = {
            let data = self.block_data.lock();
            if data.len() < block_height.get() {
                return None;
            }

            let idx = block_height.get() - 1;
            if let Some(block_arc) = data[idx].1.as_ref() {
                return Some(Arc::clone(block_arc));
            }

            let expected_hash = data[idx].0;
            let should_cache = idx + self.blocks_in_memory.get() >= data.len();
            (idx, expected_hash, should_cache)
        };

        let block = {
            let mut block_store = self.block_store.lock();
            let index = match block_store.read_block_index(block_index as u64) {
                Ok(index) => index,
                Err(error) => {
                    error!(?error, block_index, "Failed to read block index from disk");
                    return None;
                }
            };
            let BlockIndex { start, length } = index;
            let is_evicted = index.is_evicted();

            if length == 0 {
                error!(block_index, "Encountered zero-length block entry");
                return None;
            }

            if let Some(telemetry) = self.telemetry.get() {
                let outcome = if is_evicted { "miss" } else { "hit" };
                telemetry.inc_storage_da_cache("kura", outcome);
            }

            if is_evicted {
                let height = block_index.saturating_add(1) as u64;
                let bytes = match block_store.read_da_block_bytes(height, length) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        error!(
                            ?error,
                            block_index, height, "Failed to read evicted block payload"
                        );
                        return None;
                    }
                };
                if let Some(telemetry) = self.telemetry.get() {
                    let actual_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
                    telemetry.add_storage_da_churn_bytes("kura", "rehydrated", actual_len);
                }
                match decode_framed_signed_block(&bytes) {
                    Ok(decoded) => decoded,
                    Err(error) => {
                        error!(
                            ?error,
                            block_index, height, "Failed to decode evicted block payload"
                        );
                        return None;
                    }
                }
            } else {
                let bytes = match block_store.block_bytes(start, length) {
                    Ok(slice) => slice,
                    Err(error) => {
                        error!(?error, block_index, "Failed to borrow block data slice");
                        return None;
                    }
                };

                match decode_framed_signed_block(bytes) {
                    Ok(decoded) => decoded,
                    Err(error) => {
                        error!(?error, block_index, "Failed to decode block from disk");
                        return None;
                    }
                }
            }
        };

        if block.hash() != expected_hash {
            error!(
                expected = ?expected_hash,
                actual = ?block.hash(),
                block_index,
                "Loaded block hash mismatched the index entry"
            );
            return None;
        }

        let block_arc = Arc::new(block);

        if should_cache {
            let mut data = self.block_data.lock();
            if block_index < data.len() && data[block_index].1.is_none() {
                data[block_index].1 = Some(Arc::clone(&block_arc));
            }
        }

        Some(block_arc)
    }

    fn enqueue_block(
        &self,
        block: &Arc<SignedBlock>,
        merge_entry: Option<&MergeLedgerEntry>,
    ) -> Result<()> {
        self.ensure_writer_healthy()?;
        #[cfg(test)]
        if merge_entry.is_none() {
            let mut merge_log = self.merge_log.lock();
            if merge_log.consume_fail_next_append() {
                return Err(Error::IO(
                    std::io::Error::other("kura store_block injected failure"),
                    PathBuf::from("merge_log_test_fail"),
                ));
            }
        }
        let block_hash = block.hash();
        let has_merge_entry = merge_entry.is_some();
        self.check_storage_budget(block, merge_entry)?;
        let block_budget_bytes =
            Self::block_required_bytes_for_budget(block.as_ref(), self.max_disk_usage_bytes)?;
        let mut block_data = self.block_data.lock();
        block_data.push((block_hash, Some(Arc::clone(block))));
        debug!(
            new_len = block_data.len(),
            ?block_hash,
            "enqueued block for persistence"
        );

        if let Some(entry) = merge_entry
            && let Err(err) = self.append_merge_entry(entry)
        {
            block_data.pop();
            error!(
                ?err,
                ?block_hash,
                entry_epoch = entry.epoch_id,
                "Failed to append merge-ledger entry while storing block"
            );
            return Err(err);
        }

        if let Err(err) = self.block_notify_tx.send(BlockNotify::NewBlock) {
            error!(
                ?err,
                "Failed to notify block writer about new block; persistence thread unavailable"
            );
            if has_merge_entry {
                let keep = {
                    let merge_log = self.merge_log.lock();
                    merge_log.total_entries.saturating_sub(1)
                };
                if let Err(rollback_err) = self.truncate_merge_log_to_len(keep) {
                    error!(
                        ?rollback_err,
                        ?block_hash,
                        "Failed to rollback merge-ledger entry after notify failure"
                    );
                }
            }
            block_data.pop();
            return Err(Error::BlockWriterUnavailable);
        }
        self.add_pending_budget_bytes(block_budget_bytes);

        Ok(())
    }

    fn file_len_or_zero(path: &Path) -> Result<u64> {
        if path.as_os_str().is_empty() {
            return Ok(0);
        }
        match std::fs::metadata(path) {
            Ok(meta) => Ok(meta.len()),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(0),
            Err(err) => Err(Error::IO(err, path.to_path_buf())),
        }
    }

    fn block_store_tracked_bytes(block_store: &mut BlockStore) -> Result<u64> {
        if block_store.path_to_blockchain.as_os_str().is_empty() {
            return Ok(0);
        }
        let data_len = block_store.data_file_len()?;
        let index_len = block_store.index_file_len()?;
        let hashes_len = block_store.hashes_file_len()?;
        let marker_path = block_store.commit_marker_path();
        let marker_len = Self::file_len_or_zero(&marker_path)?;
        let marker_tmp_len = Self::file_len_or_zero(&marker_path.with_extension("norito.tmp"))?;
        let data_tmp_len = Self::file_len_or_zero(
            &block_store
                .path_to_blockchain
                .join(format!("{DATA_FILE_NAME}.tmp")),
        )?;
        let index_tmp_len = Self::file_len_or_zero(
            &block_store
                .path_to_blockchain
                .join(format!("{INDEX_FILE_NAME}.tmp")),
        )?;
        let hashes_tmp_len = Self::file_len_or_zero(
            &block_store
                .path_to_blockchain
                .join(format!("{HASHES_FILE_NAME}.tmp")),
        )?;
        Ok(data_len
            .saturating_add(index_len)
            .saturating_add(hashes_len)
            .saturating_add(marker_len)
            .saturating_add(marker_tmp_len)
            .saturating_add(data_tmp_len)
            .saturating_add(index_tmp_len)
            .saturating_add(hashes_tmp_len))
    }

    fn da_payload_bytes_for_range(
        block_store: &BlockStore,
        start_height: u64,
        count: usize,
    ) -> Result<u64> {
        if block_store.da_blocks_dir.as_os_str().is_empty() || count == 0 {
            return Ok(0);
        }
        let mut total = 0u64;
        for offset in 0..count {
            let height = start_height.saturating_add(offset as u64).saturating_add(1);
            let path = block_store.da_block_path(height);
            total = total.saturating_add(Self::file_len_or_zero(&path)?);
        }
        Ok(total)
    }

    fn merge_log_tracked_bytes(&self) -> Result<u64> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(0);
        }
        let path = self.active_merge_path.lock().clone();
        Self::file_len_or_zero(&path)
    }

    fn roster_journal_tracked_bytes(&self) -> Result<u64> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(0);
        }
        let path = CommitRosterJournal::journal_path(&self.store_root);
        let tmp_path = path.with_extension("norito.tmp");
        Ok(Self::file_len_or_zero(&path)?.saturating_add(Self::file_len_or_zero(&tmp_path)?))
    }

    fn sidecar_tracked_bytes(
        data_path: &Path,
        index_path: &Path,
        json_path: Option<&Path>,
    ) -> Result<u64> {
        let data_tmp = data_path.with_extension("norito.tmp");
        let index_tmp = index_path.with_extension("index.tmp");
        let mut total = Self::file_len_or_zero(data_path)?
            .saturating_add(Self::file_len_or_zero(index_path)?)
            .saturating_add(Self::file_len_or_zero(&data_tmp)?)
            .saturating_add(Self::file_len_or_zero(&index_tmp)?);
        if let Some(path) = json_path {
            total = total.saturating_add(Self::file_len_or_zero(path)?);
        }
        Ok(total)
    }

    fn block_required_bytes(block: &SignedBlock) -> Result<u64> {
        let wire = block.canonical_wire()?;
        let (frame, _) = wire.into_parts();
        let frame_len = u64::try_from(frame.len())?;
        Ok(frame_len
            .saturating_add(BlockIndex::SIZE)
            .saturating_add(SIZE_OF_BLOCK_HASH))
    }

    fn merge_entry_bytes(entry: &MergeLedgerEntry) -> Result<u64> {
        let encoded = Encode::encode(entry);
        let encoded_len = u64::try_from(encoded.len())?;
        Ok(encoded_len.saturating_add(std::mem::size_of::<u32>() as u64))
    }

    fn evicted_block_required_bytes() -> u64 {
        BlockIndex::SIZE.saturating_add(SIZE_OF_BLOCK_HASH)
    }

    // Budget accounting treats oversized blocks as evicted-on-write.
    fn block_required_bytes_for_budget(block: &SignedBlock, limit: u64) -> Result<u64> {
        let required = Self::block_required_bytes(block)?;
        if limit > 0 && required > limit {
            Ok(Self::evicted_block_required_bytes())
        } else {
            Ok(required)
        }
    }

    fn sidecar_bytes(store_dir: &Path) -> Result<u64> {
        if store_dir.as_os_str().is_empty() {
            return Ok(0);
        }
        let dir = store_dir.join(PIPELINE_DIR_NAME);
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(Error::IO(err, dir)),
        };
        let mut total = 0u64;
        for entry in entries {
            let entry = entry.map_err(|err| Error::IO(err, dir.clone()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|err| Error::IO(err, path.clone()))?;
            if file_type.is_file() {
                let len = entry
                    .metadata()
                    .map_err(|err| Error::IO(err, path.clone()))?
                    .len();
                total = total.saturating_add(len);
            }
        }
        Ok(total)
    }

    fn block_store_bytes(blocks_dir: &Path) -> Result<u64> {
        if blocks_dir.as_os_str().is_empty() {
            return Ok(0);
        }
        let entries = match std::fs::read_dir(blocks_dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(Error::IO(err, blocks_dir.to_path_buf())),
        };
        let mut files = 0u64;
        for entry in entries {
            let entry = entry.map_err(|err| Error::IO(err, blocks_dir.to_path_buf()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|err| Error::IO(err, path.clone()))?;
            if file_type.is_file() {
                let len = entry
                    .metadata()
                    .map_err(|err| Error::IO(err, path.clone()))?
                    .len();
                files = files.saturating_add(len);
            }
        }
        let sidecars = Self::sidecar_bytes(blocks_dir)?;
        Ok(files.saturating_add(sidecars))
    }

    fn blocks_root_bytes(root: &Path) -> Result<u64> {
        if root.as_os_str().is_empty() {
            return Ok(0);
        }
        let entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(Error::IO(err, root.to_path_buf())),
        };
        let mut total = 0u64;
        for entry in entries {
            let entry = entry.map_err(|err| Error::IO(err, root.to_path_buf()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|err| Error::IO(err, path.clone()))?;
            if file_type.is_dir() {
                total = total.saturating_add(Self::block_store_bytes(&path)?);
            }
        }
        Ok(total)
    }

    fn dir_file_bytes(dir: &Path) -> Result<u64> {
        if dir.as_os_str().is_empty() {
            return Ok(0);
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(Error::IO(err, dir.to_path_buf())),
        };
        let mut total = 0u64;
        for entry in entries {
            let entry = entry.map_err(|err| Error::IO(err, dir.to_path_buf()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|err| Error::IO(err, path.clone()))?;
            if file_type.is_file() {
                let len = entry
                    .metadata()
                    .map_err(|err| Error::IO(err, path.clone()))?
                    .len();
                total = total.saturating_add(len);
            }
        }
        Ok(total)
    }

    fn block_store_total_bytes(blocks_dir: &Path) -> Result<u64> {
        let mut total = Self::block_store_bytes(blocks_dir)?;
        let da_dir = blocks_dir.join(DA_BLOCKS_DIR_NAME);
        total = total.saturating_add(Self::dir_file_bytes(&da_dir)?);
        Ok(total)
    }

    fn blocks_root_total_bytes(root: &Path) -> Result<u64> {
        if root.as_os_str().is_empty() {
            return Ok(0);
        }
        let entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(Error::IO(err, root.to_path_buf())),
        };
        let mut total = 0u64;
        for entry in entries {
            let entry = entry.map_err(|err| Error::IO(err, root.to_path_buf()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|err| Error::IO(err, path.clone()))?;
            if file_type.is_dir() {
                total = total.saturating_add(Self::block_store_total_bytes(&path)?);
            }
        }
        Ok(total)
    }

    fn merge_root_bytes(root: &Path) -> Result<u64> {
        if root.as_os_str().is_empty() {
            return Ok(0);
        }
        let entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(Error::IO(err, root.to_path_buf())),
        };
        let mut total = 0u64;
        for entry in entries {
            let entry = entry.map_err(|err| Error::IO(err, root.to_path_buf()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|err| Error::IO(err, path.clone()))?;
            if file_type.is_file() {
                total = total.saturating_add(Self::file_len_or_zero(&path)?);
            }
        }
        Ok(total)
    }

    fn kura_disk_usage_bytes(&self) -> Result<u64> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(0);
        }
        let blocks_root = self.store_root.join("blocks");
        let merge_root = self.store_root.join("merge_ledger");
        let retired_root = self.store_root.join("retired");
        let retired_blocks_root = retired_root.join("blocks");
        let retired_merge_root = retired_root.join("merge_ledger");

        let mut used = 0u64;
        used = used.saturating_add(Self::blocks_root_bytes(&blocks_root)?);
        used = used.saturating_add(Self::merge_root_bytes(&merge_root)?);
        used = used.saturating_add(Self::blocks_root_bytes(&retired_blocks_root)?);
        used = used.saturating_add(Self::merge_root_bytes(&retired_merge_root)?);
        let roster_journal = CommitRosterJournal::journal_path(&self.store_root);
        used = used.saturating_add(Self::file_len_or_zero(&roster_journal)?);
        used = used.saturating_add(Self::file_len_or_zero(
            &roster_journal.with_extension("norito.tmp"),
        )?);
        Ok(used)
    }

    fn kura_total_disk_usage_bytes(&self) -> Result<u64> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(0);
        }
        let blocks_root = self.store_root.join("blocks");
        let merge_root = self.store_root.join("merge_ledger");
        let retired_root = self.store_root.join("retired");
        let retired_blocks_root = retired_root.join("blocks");
        let retired_merge_root = retired_root.join("merge_ledger");

        let mut used = 0u64;
        used = used.saturating_add(Self::blocks_root_total_bytes(&blocks_root)?);
        used = used.saturating_add(Self::merge_root_bytes(&merge_root)?);
        used = used.saturating_add(Self::blocks_root_total_bytes(&retired_blocks_root)?);
        used = used.saturating_add(Self::merge_root_bytes(&retired_merge_root)?);
        let roster_journal = CommitRosterJournal::journal_path(&self.store_root);
        used = used.saturating_add(Self::file_len_or_zero(&roster_journal)?);
        used = used.saturating_add(Self::file_len_or_zero(
            &roster_journal.with_extension("norito.tmp"),
        )?);
        Ok(used)
    }

    fn purge_retired_storage(&self) -> bool {
        if self.store_root.as_os_str().is_empty() {
            return false;
        }
        let retired_root = self.store_root.join("retired");
        if !retired_root.exists() {
            return false;
        }
        let retired_blocks_root = retired_root.join("blocks");
        let retired_merge_root = retired_root.join("merge_ledger");
        let mut sizing_failed = false;
        let retired_blocks_budget = match Self::blocks_root_bytes(&retired_blocks_root) {
            Ok(bytes) => bytes,
            Err(err) => {
                warn!(
                    ?err,
                    path = %retired_blocks_root.display(),
                    "failed to size retired block segments"
                );
                sizing_failed = true;
                0
            }
        };
        let retired_blocks_total = match Self::blocks_root_total_bytes(&retired_blocks_root) {
            Ok(bytes) => bytes,
            Err(err) => {
                warn!(
                    ?err,
                    path = %retired_blocks_root.display(),
                    "failed to size retired block segments including DA payloads"
                );
                sizing_failed = true;
                0
            }
        };
        let retired_merge_bytes = match Self::merge_root_bytes(&retired_merge_root) {
            Ok(bytes) => bytes,
            Err(err) => {
                warn!(
                    ?err,
                    path = %retired_merge_root.display(),
                    "failed to size retired merge-ledger segments"
                );
                sizing_failed = true;
                0
            }
        };
        let retired_budget_bytes = retired_blocks_budget.saturating_add(retired_merge_bytes);
        let retired_total_bytes = retired_blocks_total.saturating_add(retired_merge_bytes);
        match std::fs::remove_dir_all(&retired_root) {
            Ok(()) => {
                info!(
                    path = %retired_root.display(),
                    "purged retired Kura segments to reclaim disk budget"
                );
                if sizing_failed {
                    if let Err(err) = self.refresh_disk_usage_bytes() {
                        warn!(
                            ?err,
                            path = %retired_root.display(),
                            "failed to refresh Kura disk usage after retired purge"
                        );
                    }
                } else {
                    self.sub_disk_usage_bytes(retired_budget_bytes);
                    let extra = retired_total_bytes.saturating_sub(retired_budget_bytes);
                    self.sub_total_disk_usage_bytes(extra);
                }
                true
            }
            Err(err) => {
                warn!(
                    ?err,
                    path = %retired_root.display(),
                    "failed to purge retired Kura segments while reclaiming disk budget"
                );
                false
            }
        }
    }

    fn pending_block_bytes_raw(&self, persisted_count: usize) -> Result<u64> {
        let pending_blocks = {
            let data = self.block_data.lock();
            let start = persisted_count.min(data.len());
            let mut blocks = Vec::with_capacity(data.len().saturating_sub(start));
            for (_, block) in data.iter().skip(start) {
                let block = block
                    .as_ref()
                    .expect("pending block missing from Kura memory queue");
                blocks.push(Arc::clone(block));
            }
            blocks
        };

        let mut pending_bytes = 0u64;
        for block in pending_blocks {
            pending_bytes = pending_bytes.saturating_add(Self::block_required_bytes_for_budget(
                &block,
                self.max_disk_usage_bytes,
            )?);
        }
        Ok(pending_bytes)
    }

    fn pending_block_bytes(&self, persisted_count: usize, unindexed_bytes: u64) -> Result<u64> {
        if self.pending_budget_bytes_valid.load(Ordering::Relaxed) {
            let pending = self.pending_budget_bytes.load(Ordering::Relaxed);
            return Ok(pending.saturating_sub(unindexed_bytes));
        }
        let pending_bytes = self.pending_block_bytes_raw(persisted_count)?;
        self.pending_budget_bytes
            .store(pending_bytes, Ordering::Relaxed);
        self.pending_budget_bytes_valid
            .store(true, Ordering::Relaxed);
        Ok(pending_bytes.saturating_sub(unindexed_bytes))
    }

    fn persisted_count_and_unindexed_bytes(&self) -> Result<(usize, u64)> {
        let mut block_store = self.block_store.lock();
        let persisted = usize::try_from(block_store.read_durable_index_count()?)?;
        let persisted_u64 = persisted as u64;
        let indexed_data_len = if persisted == 0 {
            0
        } else {
            let last = block_store.read_block_index(persisted as u64 - 1)?;
            last.start.saturating_add(last.length)
        };
        let data_file_len = block_store.data_file_len()?;
        let index_file_len = block_store.index_file_len()?;
        let hashes_file_len = block_store.hashes_file_len()?;
        let indexed_index_len = persisted_u64.saturating_mul(BlockIndex::SIZE);
        let indexed_hash_len = persisted_u64.saturating_mul(SIZE_OF_BLOCK_HASH);
        let unindexed_bytes = data_file_len
            .saturating_sub(indexed_data_len)
            .saturating_add(index_file_len.saturating_sub(indexed_index_len))
            .saturating_add(hashes_file_len.saturating_sub(indexed_hash_len));
        Ok((persisted, unindexed_bytes))
    }

    fn check_storage_budget(
        &self,
        block: &SignedBlock,
        merge_entry: Option<&MergeLedgerEntry>,
    ) -> Result<()> {
        if self.max_disk_usage_bytes == 0 || self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        self.ensure_disk_usage_initialized()?;

        let merge_entry_bytes = match merge_entry {
            Some(entry) => Self::merge_entry_bytes(entry)?,
            None => 0,
        };

        let (persisted_count, unindexed_bytes) = self.persisted_count_and_unindexed_bytes()?;

        let limit = self.max_disk_usage_bytes;
        let block_required = Self::block_required_bytes_for_budget(block, limit)?;
        let mut used = self.disk_usage.load(Ordering::Relaxed);
        let pending_bytes = self.pending_block_bytes(persisted_count, unindexed_bytes)?;
        let mut budget_used = used.saturating_add(pending_bytes);
        let mut required = budget_used
            .saturating_add(block_required)
            .saturating_add(merge_entry_bytes);

        if required > limit {
            if self.purge_retired_storage() {
                used = self.disk_usage.load(Ordering::Relaxed);
                budget_used = used.saturating_add(pending_bytes);
                required = used
                    .saturating_add(pending_bytes)
                    .saturating_add(block_required)
                    .saturating_add(merge_entry_bytes);
                if required <= limit {
                    if let Some(telemetry) = self.telemetry.get() {
                        telemetry.record_storage_budget_usage("kura", required, limit);
                    }
                    return Ok(());
                }
            }
            let evict_needed = required.saturating_sub(limit);
            if evict_needed > 0 {
                match self.evict_block_bodies(evict_needed) {
                    Ok(freed) if freed > 0 => {
                        used = self.disk_usage.load(Ordering::Relaxed);
                        budget_used = used.saturating_add(pending_bytes);
                        required = used
                            .saturating_add(pending_bytes)
                            .saturating_add(block_required)
                            .saturating_add(merge_entry_bytes);
                        if required <= limit {
                            if let Some(telemetry) = self.telemetry.get() {
                                telemetry.record_storage_budget_usage("kura", required, limit);
                            }
                            return Ok(());
                        }
                    }
                    Ok(_) => {}
                    Err(err) => {
                        warn!(?err, "failed to evict Kura block bodies for budget");
                    }
                }
            }
            if let Some(telemetry) = self.telemetry.get() {
                telemetry.record_storage_budget_usage("kura", budget_used, limit);
                telemetry.inc_storage_budget_exceeded("kura");
            }
            warn!(
                used,
                required,
                limit,
                path = %self.store_root.display(),
                "Kura storage budget exceeded"
            );
            return Err(Error::StorageBudgetExceeded {
                limit,
                used,
                required,
            });
        }

        if let Some(telemetry) = self.telemetry.get() {
            telemetry.record_storage_budget_usage("kura", required, limit);
        }

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn check_replace_storage_budget(&self, block: &SignedBlock) -> Result<()> {
        if self.max_disk_usage_bytes == 0 || self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        self.ensure_disk_usage_initialized()?;

        let (block_count, old_block) = {
            let data = self.block_data.lock();
            (
                data.len(),
                data.last()
                    .and_then(|(_, block)| block.as_ref().map(Arc::clone)),
            )
        };

        if block_count == 0 {
            return self.check_storage_budget(block, None);
        }
        let Some(old_block) = old_block else {
            return self.check_storage_budget(block, None);
        };

        let limit = self.max_disk_usage_bytes;
        let new_bytes = Self::block_required_bytes_for_budget(block, limit)?;
        let old_bytes = Self::block_required_bytes_for_budget(&old_block, limit)?;
        let (persisted_count, unindexed_bytes) = self.persisted_count_and_unindexed_bytes()?;
        let pending_raw = self.pending_block_bytes_raw(persisted_count)?;
        let top_is_pending = block_count > persisted_count;

        let mut pending_raw_after = pending_raw;
        if top_is_pending {
            pending_raw_after = pending_raw_after.saturating_sub(old_bytes);
        }
        pending_raw_after = pending_raw_after.saturating_add(new_bytes);

        let pending_current = pending_raw.saturating_sub(unindexed_bytes);
        let mut used = self.disk_usage.load(Ordering::Relaxed);
        let mut budget_used = used.saturating_add(pending_current);
        let mut required = {
            let used_after = if top_is_pending {
                used
            } else {
                used.saturating_sub(old_bytes)
            };
            let pending_after = pending_raw_after.saturating_sub(unindexed_bytes);
            used_after.saturating_add(pending_after)
        };

        if required > limit {
            if self.purge_retired_storage() {
                used = self.disk_usage.load(Ordering::Relaxed);
                budget_used = used.saturating_add(pending_current);
                required = {
                    let used_after = if top_is_pending {
                        used
                    } else {
                        used.saturating_sub(old_bytes)
                    };
                    let pending_after = pending_raw_after.saturating_sub(unindexed_bytes);
                    used_after.saturating_add(pending_after)
                };
                if required <= limit {
                    if let Some(telemetry) = self.telemetry.get() {
                        telemetry.record_storage_budget_usage("kura", required, limit);
                    }
                    return Ok(());
                }
            }
            let evict_needed = required.saturating_sub(limit);
            if evict_needed > 0 {
                match self.evict_block_bodies(evict_needed) {
                    Ok(freed) if freed > 0 => {
                        used = self.disk_usage.load(Ordering::Relaxed);
                        budget_used = used.saturating_add(pending_current);
                        required = {
                            let used_after = if top_is_pending {
                                used
                            } else {
                                used.saturating_sub(old_bytes)
                            };
                            let pending_after = pending_raw_after.saturating_sub(unindexed_bytes);
                            used_after.saturating_add(pending_after)
                        };
                        if required <= limit {
                            if let Some(telemetry) = self.telemetry.get() {
                                telemetry.record_storage_budget_usage("kura", required, limit);
                            }
                            return Ok(());
                        }
                    }
                    Ok(_) => {}
                    Err(err) => {
                        warn!(?err, "failed to evict Kura block bodies for budget");
                    }
                }
            }
            if let Some(telemetry) = self.telemetry.get() {
                telemetry.record_storage_budget_usage("kura", budget_used, limit);
                telemetry.inc_storage_budget_exceeded("kura");
            }
            warn!(
                used,
                required,
                limit,
                path = %self.store_root.display(),
                "Kura storage budget exceeded"
            );
            return Err(Error::StorageBudgetExceeded {
                limit,
                used,
                required,
            });
        }

        if let Some(telemetry) = self.telemetry.get() {
            telemetry.record_storage_budget_usage("kura", required, limit);
        }

        Ok(())
    }

    fn append_blocks_jsonl(path: &Path, blocks: &[Arc<SignedBlock>]) -> std::io::Result<()> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        let mut writer = BufWriter::new(file);
        for block in blocks {
            norito::json::to_writer(&mut writer, block.as_ref())
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
            writer.write_all(b"\n")?;
        }
        writer.flush()
    }

    /// Put a block in Kura's in-memory block store.
    ///
    /// # Errors
    /// Returns an error if the block cannot be enqueued for persistence.
    pub fn store_block(&self, block: impl Into<Arc<SignedBlock>>) -> Result<()> {
        let block = block.into();
        self.enqueue_block(&block, None)
    }

    /// Put a block in Kura's in-memory store and persist the merge-ledger entry sealing it.
    ///
    /// # Errors
    /// Returns an error if the block or merge entry cannot be enqueued.
    pub fn store_block_with_merge_entry(
        &self,
        block: impl Into<Arc<SignedBlock>>,
        merge_entry: &MergeLedgerEntry,
    ) -> Result<()> {
        let block = block.into();
        self.enqueue_block(&block, Some(merge_entry))
    }

    /// Replace the block in `Kura`'s in memory block store.
    ///
    /// # Errors
    /// Returns an error if the block cannot be enqueued for persistence or exceeds the
    /// configured storage budget.
    pub fn replace_top_block(&self, block: impl Into<Arc<SignedBlock>>) -> Result<()> {
        self.ensure_writer_healthy()?;
        self.invalidate_pending_budget_cache();
        let block = block.into();
        self.check_replace_storage_budget(block.as_ref())?;
        let mut data = self.block_data.lock();
        let previous = data.pop();
        data.push((block.hash(), Some(block)));
        if let Err(err) = self.block_notify_tx.send(BlockNotify::NewBlock) {
            error!(
                ?err,
                "Failed to notify block writer about top-block replacement; persistence thread unavailable"
            );
            data.pop();
            if let Some(previous) = previous {
                data.push(previous);
            }
            return Err(Error::BlockWriterUnavailable);
        }
        Ok(())
    }

    /// Truncate the canonical chain to the provided height (inclusive).
    ///
    /// This updates the in-memory block list and prunes persisted storage when available.
    /// Heights are 1-based (genesis is height 1).
    ///
    /// # Errors
    ///
    /// Returns an error if height conversion fails, persisted block storage pruning fails, or
    /// truncating the merge log fails.
    pub fn prune_to_height(&self, height: u64) -> Result<()> {
        let keep = usize::try_from(height)?;
        {
            let mut data = self.block_data.lock();
            if keep >= data.len() {
                return Ok(());
            }
            data.truncate(keep);
        }
        self.invalidate_pending_budget_cache();

        if !self.store_root.as_os_str().is_empty() {
            let mut store = self.block_store.lock();
            let before_bytes = match Self::block_store_tracked_bytes(&mut store) {
                Ok(bytes) => Some(bytes),
                Err(err) => {
                    warn!(?err, "failed to measure block store bytes before prune");
                    None
                }
            };
            store.prune(height)?;
            if let Some(before_bytes) = before_bytes {
                match Self::block_store_tracked_bytes(&mut store) {
                    Ok(after_bytes) => self.update_disk_usage_delta(before_bytes, after_bytes),
                    Err(err) => warn!(?err, "failed to measure block store bytes after prune"),
                }
            }
        }

        self.truncate_merge_log_to_len(keep)?;
        let roster_before = match self.roster_journal_tracked_bytes() {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                warn!(?err, "failed to measure commit roster journal before prune");
                None
            }
        };
        if let Err(err) = self.roster_log.lock().truncate_to_height(height) {
            warn!(
                ?err,
                height, "failed to truncate commit roster journal after Kura prune"
            );
        }
        if let Some(roster_before) = roster_before {
            match self.roster_journal_tracked_bytes() {
                Ok(after_bytes) => self.update_disk_usage_delta(roster_before, after_bytes),
                Err(err) => warn!(?err, "failed to measure commit roster journal after prune"),
            }
        }

        Ok(())
    }

    // Drop cached blocks that are already persisted and outside the retention window.
    // Keep the genesis block plus the most recent `blocks_in_memory` persisted blocks.
    fn drop_persisted_blocks(
        block_data: &mut BlockData,
        persisted_count: usize,
        blocks_in_memory: usize,
    ) {
        let drop_before = persisted_count.saturating_sub(blocks_in_memory);
        let limit = drop_before.min(block_data.len());
        if limit <= 1 {
            return;
        }
        // (Genesis block is used in metrics to get genesis timestamp.)
        for entry in block_data.iter_mut().take(limit).skip(1) {
            entry.1 = None;
        }
    }

    /// Returns count of blocks Kura currently holds
    pub fn blocks_count(&self) -> usize {
        self.block_data.lock().len()
    }
}

#[cfg(test)]
impl Kura {
    pub(crate) fn persist_block_immediate_for_tests(&self, block: &Arc<SignedBlock>) {
        let mut store = self.block_store.lock();
        let before_bytes = Self::block_store_tracked_bytes(&mut store)
            .expect("measure block store bytes before test append");
        store
            .append_block_to_chain(block.as_ref())
            .expect("persist block for tests");
        let after_bytes = Self::block_store_tracked_bytes(&mut store)
            .expect("measure block store bytes after test append");
        self.update_disk_usage_delta(before_bytes, after_bytes);
    }

    pub(crate) fn fail_next_store_for_tests(&self) {
        let mut merge_log = self.merge_log.lock();
        merge_log.fail_next_append = true;
    }
}

/// Loaded block count
#[derive(Clone, Copy, Debug)]
pub struct BlockCount(pub usize);

/// An implementation of a block store for `Kura`
/// that uses `std::fs`, the default IO file in Rust.
pub struct BlockStore {
    path_to_blockchain: PathBuf,
    da_blocks_dir: PathBuf,
    data_file: Option<FileWrap>,
    index_file: Option<FileWrap>,
    hashes_file: Option<FileWrap>,
    fsync: FsyncState,
    fsync_telemetry: FsyncTelemetry,
    encode_scratch: Vec<u8>,
    read_scratch: Vec<u8>,
    data_mmap: Option<MemoryMirror>,
    data_mmap_len: u64,
    commit_marker_count: u64,
    commit_marker_pending: Option<u64>,
}

impl Debug for BlockStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockStore")
            .field("path_to_blockchain", &self.path_to_blockchain)
            .field("da_blocks_dir", &self.da_blocks_dir)
            .field("data_file_open", &self.data_file.is_some())
            .field("index_file_open", &self.index_file.is_some())
            .field("hashes_file_open", &self.hashes_file.is_some())
            .field("fsync_mode", &self.fsync.mode)
            .field("fsync_pending", &self.fsync.pending_since.is_some())
            .field("fsync_telemetry", &self.fsync_telemetry)
            .field("encode_scratch_len", &self.encode_scratch.len())
            .field("read_scratch_len", &self.read_scratch.len())
            .field(
                "mirror_kind",
                &self.data_mmap.as_ref().map(MemoryMirror::kind),
            )
            .field("mmap_len", &self.data_mmap_len)
            .field("commit_marker_count", &self.commit_marker_count)
            .field("commit_marker_pending", &self.commit_marker_pending)
            .finish()
    }
}

/// Read-only mirror of the block data file backed either by a memory mapping or a heap copy.
#[derive(Clone)]
enum MemoryMirror {
    /// OS-backed memory map sharing pages with the data file (copy-on-write).
    Mapped(Arc<ReadOnlyMmap>),
    /// Heap-backed copy used as a portable fallback when memory mapping is unavailable.
    Heap(Arc<[u8]>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemoryMirrorKind {
    /// Mirror is backed by an OS memory map.
    MemoryMapped,
    /// Mirror contains a heap-allocated copy of the file contents.
    HeapCopy,
}

impl MemoryMirror {
    fn from_mmap(map: ReadOnlyMmap) -> Self {
        Self::Mapped(Arc::new(map))
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        Self::Heap(Arc::from(bytes.into_boxed_slice()))
    }

    fn len(&self) -> usize {
        match self {
            Self::Mapped(map) => map.len(),
            Self::Heap(bytes) => bytes.len(),
        }
    }

    fn slice(&self, start: usize, end: usize) -> &[u8] {
        match self {
            Self::Mapped(map) => &map[start..end],
            Self::Heap(bytes) => &bytes[start..end],
        }
    }

    fn kind(&self) -> MemoryMirrorKind {
        match self {
            Self::Mapped(_) => MemoryMirrorKind::MemoryMapped,
            Self::Heap(_) => MemoryMirrorKind::HeapCopy,
        }
    }

    fn from_file(file: &mut std::fs::File, len: usize, file_len: u64) -> std::io::Result<Self> {
        match ReadOnlyMmap::copy_read_only_with_file_len(file, len, file_len) {
            Ok(map) => Ok(Self::from_mmap(map)),
            Err(map_err) => {
                iroha_logger::debug!(
                    ?map_err,
                    "failed to memory-map block data; falling back to heap mirror"
                );
                file.seek(SeekFrom::Start(0))?;
                let mut buffer = Vec::with_capacity(len);
                {
                    let mut reader = file.take(len as u64);
                    reader.read_to_end(&mut buffer)?;
                    if buffer.len() != len {
                        return Err(std::io::Error::new(
                            ErrorKind::UnexpectedEof,
                            format!(
                                "expected {len} bytes while mirroring block data, read {} bytes",
                                buffer.len()
                            ),
                        ));
                    }
                }
                Ok(Self::from_bytes(buffer))
            }
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
/// Lightweight wrapper for block indices in the block index file
pub struct BlockIndex {
    /// Start of block in bytes
    pub start: u64,
    /// Length of block section in bytes
    pub length: u64,
}

impl BlockIndex {
    fn is_evicted(&self) -> bool {
        self.start == EVICTED_BLOCK_START
    }
}

impl BlockIndex {
    const SIZE: u64 = core::mem::size_of::<Self>() as u64;
    const SIZE_USIZE: usize = core::mem::size_of::<Self>();

    fn encode(self) -> [u8; Self::SIZE_USIZE] {
        let mut out = [0u8; Self::SIZE_USIZE];
        out[..core::mem::size_of::<u64>()].copy_from_slice(&self.start.to_le_bytes());
        out[core::mem::size_of::<u64>()..].copy_from_slice(&self.length.to_le_bytes());
        out
    }

    fn read(
        file: &mut std::fs::File,
        buff: &mut [u8; core::mem::size_of::<u64>()],
    ) -> std::io::Result<Self> {
        fn read_u64(
            file: &mut std::fs::File,
            buff: &mut [u8; core::mem::size_of::<u64>()],
        ) -> std::io::Result<u64> {
            file.read_exact(buff).map(|()| u64::from_le_bytes(*buff))
        }

        Ok(Self {
            start: read_u64(file, buff)?,
            length: read_u64(file, buff)?,
        })
    }
}

#[derive(Debug, Clone, Encode, Decode)]
struct BlockStoreCommitMarker {
    /// Marker format version (v1).
    version: u32,
    /// Count of blocks that are fully durable on disk.
    count: u64,
}

impl BlockStoreCommitMarker {
    const VERSION: u32 = 1;

    fn new(count: u64) -> Self {
        Self {
            version: Self::VERSION,
            count,
        }
    }
}

/// Norito-encoded pipeline recovery metadata sidecar stored alongside block data.
#[derive(Debug, Clone, Encode, Decode)]
pub struct PipelineRecoverySidecar {
    /// Schema / evolution tag for the pipeline metadata format.
    pub format: PipelineRecoveryFormat,
    /// Block height the metadata belongs to.
    pub height: u64,
    /// Block hash the metadata belongs to.
    pub block_hash: HashOf<BlockHeader>,
    /// Deterministic DAG fingerprint and key count summary.
    pub dag: PipelineDagSnapshot,
    /// Per-transaction access summaries for recovery heuristics.
    pub txs: Vec<PipelineTxSnapshot>,
    /// Optional zero-knowledge proof attachments captured for this block.
    #[norito(default)]
    pub proofs: Vec<PipelineProofSnapshot>,
}

impl PipelineRecoverySidecar {
    const FORMAT_LABEL: &'static str = "pipeline.recovery";

    /// Create a new recovery sidecar payload.
    pub fn new(
        height: u64,
        block_hash: HashOf<BlockHeader>,
        dag: PipelineDagSnapshot,
        txs: Vec<PipelineTxSnapshot>,
    ) -> Self {
        Self {
            format: PipelineRecoveryFormat::Current,
            height,
            block_hash,
            dag,
            txs,
            proofs: Vec::new(),
        }
    }

    /// Return the human-readable format tag describing the recovery payload.
    pub fn format_label(&self) -> &'static str {
        match self.format {
            PipelineRecoveryFormat::Current => Self::FORMAT_LABEL,
        }
    }

    /// Convert the sidecar into a JSON value for operator tooling.
    pub fn to_json_value(&self) -> JsonValue {
        let dag = {
            let mut dag = norito::json::Map::new();
            dag.insert(
                "fingerprint".to_string(),
                norito::json::to_value(&hex::encode(self.dag.fingerprint))
                    .expect("serialize fingerprint"),
            );
            dag.insert(
                "key_count".to_string(),
                norito::json::to_value(&self.dag.key_count).expect("serialize key_count"),
            );
            norito::json::Value::Object(dag)
        };

        let txs = self
            .txs
            .iter()
            .map(|tx| {
                let mut entry = norito::json::Map::new();
                entry.insert(
                    "hash".to_string(),
                    norito::json::to_value(&tx.hash.to_string()).expect("serialize tx hash"),
                );
                entry.insert(
                    "reads".to_string(),
                    norito::json::to_value(&tx.reads).expect("serialize read count"),
                );
                entry.insert(
                    "writes".to_string(),
                    norito::json::to_value(&tx.writes).expect("serialize write count"),
                );
                norito::json::Value::Object(entry)
            })
            .collect::<Vec<_>>();

        let proofs = self
            .proofs
            .iter()
            .map(|proof| {
                let mut entry = norito::json::Map::new();
                entry.insert(
                    "backend".to_string(),
                    norito::json::to_value(&proof.backend).expect("serialize backend"),
                );
                entry.insert(
                    "proof".to_string(),
                    norito::json::to_value(&BASE64_STANDARD.encode(&proof.proof))
                        .expect("serialize proof"),
                );
                entry.insert(
                    "code_hash".to_string(),
                    norito::json::to_value(&hex::encode(proof.code_hash))
                        .expect("serialize code hash"),
                );
                if let Some(tx_hash) = proof.tx_hash {
                    entry.insert(
                        "tx_hash".to_string(),
                        norito::json::to_value(&hex::encode(tx_hash)).expect("serialize tx hash"),
                    );
                }
                norito::json::Value::Object(entry)
            })
            .collect::<Vec<_>>();

        let mut root = norito::json::Map::new();
        root.insert(
            "format".to_string(),
            norito::json::to_value(&self.format_label()).expect("serialize format label"),
        );
        root.insert(
            "height".to_string(),
            norito::json::to_value(&self.height).expect("serialize pipeline height"),
        );
        root.insert(
            "block_hash".to_string(),
            norito::json::to_value(&self.block_hash.to_string())
                .expect("serialize pipeline block hash"),
        );
        root.insert("dag".to_string(), dag);
        root.insert("txs".to_string(), norito::json::Value::Array(txs));
        root.insert("proofs".to_string(), norito::json::Value::Array(proofs));
        norito::json::Value::Object(root)
    }

    /// Encode the sidecar into a framed Norito buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if framing fails (e.g., compression/header mismatch).
    pub fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        norito::to_bytes(self)
    }
}

/// Known metadata format variants for pipeline recovery sidecars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum PipelineRecoveryFormat {
    #[codec(index = 1)]
    /// Sidecars anchored to a specific block hash to avoid reuse across forks.
    Current,
}

/// Deterministic DAG summary embedded in pipeline recovery metadata.
#[derive(Debug, Copy, Clone, Encode, Decode)]
pub struct PipelineDagSnapshot {
    /// Blake2 hash summarising the DAG structure for the block.
    pub fingerprint: [u8; 32],
    /// Number of unique DAG keys observed during block construction.
    pub key_count: u32,
}

/// Transaction access summary persisted for pipeline recovery/replay.
#[derive(Debug, Clone, Encode, Decode)]
pub struct PipelineTxSnapshot {
    /// Transaction hash to correlate with block entries.
    pub hash: HashOf<TransactionEntrypoint>,
    /// Sorted list of state keys read during execution.
    pub reads: Vec<String>,
    /// Sorted list of state keys written during execution.
    pub writes: Vec<String>,
}

/// ZK proof artifacts captured alongside pipeline metadata.
#[derive(Debug, Clone, Encode, Decode)]
pub struct PipelineProofSnapshot {
    /// Backend identifier for the proof format.
    pub backend: String,
    /// Raw proof bytes recorded for the trace.
    pub proof: Vec<u8>,
    /// Code hash of the executed program producing the trace.
    pub code_hash: [u8; 32],
    /// Optional transaction hash associated with the trace.
    #[norito(default)]
    pub tx_hash: Option<[u8; 32]>,
}

/// Known metadata format variants for roster snapshots persisted alongside blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum RosterSidecarFormat {
    #[codec(index = 1)]
    /// Roster snapshot format with stake metadata.
    Current,
}

/// Persisted roster metadata enabling roster reconstruction during block sync.
#[derive(Debug, Clone, Encode, Decode)]
pub struct RosterSidecar {
    /// Schema / evolution tag for the roster metadata format.
    pub format: RosterSidecarFormat,
    /// Block height the roster applies to.
    pub height: u64,
    /// Block hash the roster was validated against.
    pub block_hash: HashOf<BlockHeader>,
    /// Optional commit certificate capturing the validator set.
    #[norito(default)]
    pub commit_qc: Option<Qc>,
    /// Optional validator-set checkpoint capturing the validator set.
    #[norito(default)]
    pub validator_checkpoint: Option<ValidatorSetCheckpoint>,
    /// Optional stake snapshot aligned to the validator set.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub stake_snapshot: Option<CommitStakeSnapshot>,
}

impl RosterSidecar {
    const FORMAT_LABEL: &'static str = "roster.snapshot";

    /// Construct a new roster sidecar payload using the current schema.
    pub fn new(
        height: u64,
        block_hash: HashOf<BlockHeader>,
        commit_qc: Option<Qc>,
        validator_checkpoint: Option<ValidatorSetCheckpoint>,
        stake_snapshot: Option<CommitStakeSnapshot>,
    ) -> Self {
        Self {
            format: RosterSidecarFormat::Current,
            height,
            block_hash,
            commit_qc,
            validator_checkpoint,
            stake_snapshot,
        }
    }

    /// Return the human-readable format tag.
    pub fn format_label(&self) -> &'static str {
        match self.format {
            RosterSidecarFormat::Current => Self::FORMAT_LABEL,
        }
    }

    /// Encode the sidecar into a framed Norito buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if framing fails.
    pub fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        norito::to_bytes(self)
    }

    /// Return the roster snapshot contained in the sidecar, preferring commit certificates over
    /// checkpoints.
    #[must_use]
    pub fn roster_snapshot(&self) -> Option<Vec<PeerId>> {
        self.commit_qc
            .as_ref()
            .map(|cert| cert.validator_set.clone())
            .or_else(|| {
                self.validator_checkpoint
                    .as_ref()
                    .map(|chkpt| chkpt.validator_set.clone())
            })
    }
}

#[derive(Debug, Clone, Copy)]
struct SidecarIndexEntry {
    offset: u64,
    len: u64,
}

impl SidecarIndexEntry {
    fn to_bytes(self) -> [u8; PIPELINE_INDEX_ENTRY_SIZE] {
        let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        buf[..8].copy_from_slice(&self.offset.to_le_bytes());
        buf[8..].copy_from_slice(&self.len.to_le_bytes());
        buf
    }

    fn from_bytes(bytes: [u8; PIPELINE_INDEX_ENTRY_SIZE]) -> Self {
        let offset = u64::from_le_bytes(bytes[..8].try_into().expect("slice length matches"));
        let len = u64::from_le_bytes(bytes[8..].try_into().expect("slice length matches"));
        Self { offset, len }
    }
}

impl Kura {
    fn now_unix_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|dur| dur.as_secs())
            .unwrap_or(0)
    }

    /// Return the path to the block storage directory, if configured.
    /// For in-memory test Kura, returns `None`.
    fn store_dir(&self) -> Option<PathBuf> {
        let path = self.block_store.lock().path_to_blockchain.clone();
        if path.as_os_str().is_empty() {
            None
        } else {
            Some(path)
        }
    }

    fn sidecar_fsync_mode(&self) -> FsyncMode {
        self.block_store.lock().fsync.mode
    }

    /// Enqueue pipeline recovery metadata for asynchronous persistence.
    ///
    /// This avoids consensus-path I/O; the Kura writer thread flushes the queue.
    pub fn enqueue_pipeline_metadata(&self, sidecar: PipelineRecoverySidecar) {
        {
            let mut queue = self.pipeline_sidecar_queue.lock();
            queue.push_back(sidecar);
        }
        if let Err(err) = self.block_notify_tx.send(BlockNotify::NewBlock) {
            iroha_logger::warn!(?err, "failed to notify block writer about pipeline sidecar");
        }
    }

    fn flush_pipeline_sidecars(&self) -> usize {
        let sidecars = {
            let mut queue = self.pipeline_sidecar_queue.lock();
            if queue.is_empty() {
                return 0;
            }
            queue.drain(..).collect::<Vec<_>>()
        };
        let count = sidecars.len();
        for sidecar in sidecars {
            self.write_pipeline_metadata(&sidecar);
        }
        count
    }

    /// Write per-block pipeline recovery metadata sidecar under the store dir. Best-effort: errors
    /// are logged and ignored.
    pub fn write_pipeline_metadata(&self, sidecar: &PipelineRecoverySidecar) {
        if let Some(mut dir) = self.store_dir() {
            let _guard = self.sidecar_lock.lock();
            dir.push(PIPELINE_DIR_NAME);
            if let Err(e) = std::fs::create_dir_all(&dir) {
                iroha_logger::warn!(?e, ?dir, "failed to create pipeline dir");
                return;
            }
            let data_path = dir.join(PIPELINE_SIDECARS_DATA_FILE);
            let index_path = dir.join(PIPELINE_SIDECARS_INDEX_FILE);
            let json_sidecar_path = dir.join(format!("block_{}.json", sidecar.height));
            let before_bytes = match Self::sidecar_tracked_bytes(
                &data_path,
                &index_path,
                Some(&json_sidecar_path),
            ) {
                Ok(bytes) => Some(bytes),
                Err(err) => {
                    iroha_logger::warn!(
                        ?err,
                        ?dir,
                        "failed to measure pipeline sidecar bytes before write"
                    );
                    None
                }
            };
            let fsync_mode = self.sidecar_fsync_mode();
            let wrote_norito = match sidecar.encode_framed() {
                Ok(buf) => Self::append_indexed_sidecar(
                    &data_path,
                    &index_path,
                    sidecar.height,
                    &buf,
                    "pipeline sidecar",
                    fsync_mode,
                    None,
                ),
                Err(err) => {
                    iroha_logger::warn!(
                        ?err,
                        height = sidecar.height,
                        "failed to encode pipeline metadata"
                    );
                    false
                }
            };

            if wrote_norito {
                if json_sidecar_path.exists()
                    && let Err(e) = std::fs::remove_file(&json_sidecar_path)
                {
                    iroha_logger::debug!(
                        ?e,
                        ?json_sidecar_path,
                        "failed to remove JSON pipeline sidecar"
                    );
                }
            }
            if let Some(before_bytes) = before_bytes {
                match Self::sidecar_tracked_bytes(&data_path, &index_path, Some(&json_sidecar_path))
                {
                    Ok(after_bytes) => self.update_disk_usage_delta(before_bytes, after_bytes),
                    Err(err) => iroha_logger::warn!(
                        ?err,
                        ?dir,
                        "failed to measure pipeline sidecar bytes after write"
                    ),
                }
            }
        }
    }

    /// Write per-block roster metadata sidecar alongside the block store. Best-effort: errors are
    /// logged and ignored.
    pub fn write_roster_metadata(&self, sidecar: &RosterSidecar) {
        if let Some(mut dir) = self.store_dir() {
            let _guard = self.sidecar_lock.lock();
            dir.push(PIPELINE_DIR_NAME);
            if let Err(e) = std::fs::create_dir_all(&dir) {
                iroha_logger::warn!(
                    ?e,
                    ?dir,
                    "failed to create pipeline dir for roster sidecars"
                );
                return;
            }
            let data_path = dir.join(ROSTER_SIDECARS_DATA_FILE);
            let index_path = dir.join(ROSTER_SIDECARS_INDEX_FILE);
            let before_bytes = match Self::sidecar_tracked_bytes(&data_path, &index_path, None) {
                Ok(bytes) => Some(bytes),
                Err(err) => {
                    iroha_logger::warn!(
                        ?err,
                        ?dir,
                        "failed to measure roster sidecar bytes before write"
                    );
                    None
                }
            };
            let fsync_mode = self.sidecar_fsync_mode();
            let wrote_norito = match sidecar.encode_framed() {
                Ok(buf) => Self::append_indexed_sidecar(
                    &data_path,
                    &index_path,
                    sidecar.height,
                    &buf,
                    "roster sidecar",
                    fsync_mode,
                    Some(self.roster_sidecar_retention),
                ),
                Err(err) => {
                    iroha_logger::warn!(
                        ?err,
                        height = sidecar.height,
                        "failed to encode roster metadata"
                    );
                    false
                }
            };
            if !wrote_norito {
                iroha_logger::warn!(
                    height = sidecar.height,
                    "failed to persist roster metadata sidecar"
                );
            }
            if let Some(before_bytes) = before_bytes {
                match Self::sidecar_tracked_bytes(&data_path, &index_path, None) {
                    Ok(after_bytes) => self.update_disk_usage_delta(before_bytes, after_bytes),
                    Err(err) => iroha_logger::warn!(
                        ?err,
                        ?dir,
                        "failed to measure roster sidecar bytes after write"
                    ),
                }
            }
        }
    }

    /// Read per-block pipeline recovery metadata if present. Returns `None` on errors.
    pub fn read_pipeline_metadata(&self, height: u64) -> Option<PipelineRecoverySidecar> {
        let _guard = self.sidecar_lock.lock();
        self.read_indexed_sidecar(
            height,
            PIPELINE_SIDECARS_DATA_FILE,
            PIPELINE_SIDECARS_INDEX_FILE,
            norito::decode_from_bytes::<PipelineRecoverySidecar>,
            "pipeline sidecar",
        )
        .and_then(|sidecar| {
            if sidecar.height != height {
                iroha_logger::warn!(
                    height,
                    sidecar_height = sidecar.height,
                    "pipeline sidecar height mismatch"
                );
                None
            } else if let Ok(height_usize) = usize::try_from(height)
                && let Some(expected) =
                    NonZeroUsize::new(height_usize).and_then(|height| self.get_block_hash(height))
                && expected != sidecar.block_hash
            {
                iroha_logger::warn!(
                    height,
                    expected = %expected,
                    actual = %sidecar.block_hash,
                    "pipeline sidecar block hash mismatch"
                );
                None
            } else {
                Some(sidecar)
            }
        })
    }

    /// Read roster metadata sidecar for `height` if present. Returns `None` on errors or missing
    /// entries.
    pub fn read_roster_metadata(&self, height: u64) -> Option<RosterSidecar> {
        let _guard = self.sidecar_lock.lock();
        self.read_indexed_sidecar(
            height,
            ROSTER_SIDECARS_DATA_FILE,
            ROSTER_SIDECARS_INDEX_FILE,
            norito::decode_from_bytes::<RosterSidecar>,
            "roster sidecar",
        )
        .and_then(|sidecar| {
            if sidecar.height != height {
                iroha_logger::warn!(
                    height,
                    sidecar_height = sidecar.height,
                    "roster sidecar height mismatch"
                );
                return None;
            }
            if let Ok(height_usize) = usize::try_from(height)
                && let Some(expected) =
                    NonZeroUsize::new(height_usize).and_then(|height| self.get_block_hash(height))
                && expected != sidecar.block_hash
            {
                iroha_logger::warn!(
                    height,
                    expected = %expected,
                    actual = %sidecar.block_hash,
                    "roster sidecar block hash mismatch"
                );
                return None;
            }
            if let Some(cert) = sidecar.commit_qc.as_ref() {
                let cert_block_hash = cert.subject_block_hash;
                if cert.height != sidecar.height || cert_block_hash != sidecar.block_hash {
                    iroha_logger::warn!(
                        height,
                        sidecar_height = sidecar.height,
                        sidecar_hash = %sidecar.block_hash,
                        cert_height = cert.height,
                        cert_hash = %cert_block_hash,
                        "roster sidecar commit certificate metadata mismatch"
                    );
                    return None;
                }
            }
            if let Some(checkpoint) = sidecar.validator_checkpoint.as_ref() {
                if checkpoint.height != sidecar.height
                    || checkpoint.block_hash != sidecar.block_hash
                {
                    iroha_logger::warn!(
                        height,
                        sidecar_height = sidecar.height,
                        sidecar_hash = %sidecar.block_hash,
                        checkpoint_height = checkpoint.height,
                        checkpoint_hash = %checkpoint.block_hash,
                        "roster sidecar checkpoint metadata mismatch"
                    );
                    return None;
                }
            }
            Some(sidecar)
        })
    }

    fn recover_indexed_sidecar_artifacts(data_path: &Path, index_path: &Path, kind: &str) {
        let temp_data_path = data_path.with_extension("norito.tmp");
        let temp_index_path = index_path.with_extension("index.tmp");
        let temp_index_exists = temp_index_path.exists();
        let temp_data_exists = temp_data_path.exists();
        let index_promoted = if temp_index_exists {
            let data_len = if temp_data_exists {
                std::fs::metadata(&temp_data_path).map(|meta| meta.len())
            } else {
                std::fs::metadata(data_path).map(|meta| meta.len())
            };
            let (data_len, temp_index_sane) = match data_len {
                Ok(data_len) => {
                    let temp_index_sane = Self::sidecar_index_sane_with_label(
                        &temp_index_path,
                        data_len,
                        kind,
                        "temp",
                    );
                    (Some(data_len), temp_index_sane)
                }
                Err(err) => {
                    warn!(
                        ?err,
                        ?temp_index_path,
                        ?data_path,
                        kind,
                        "failed to read sidecar data length for temp index validation"
                    );
                    (None, false)
                }
            };
            data_len.is_some_and(|data_len| {
                if !temp_index_sane {
                    warn!(
                        ?temp_index_path,
                        kind, "refusing to promote invalid sidecar temp index"
                    );
                    return false;
                }
                if temp_data_exists {
                    return Self::promote_sidecar_temp(&temp_index_path, index_path, kind, "index");
                }
                let main_index_sane = if index_path.exists() {
                    Self::sidecar_index_sane_with_label(index_path, data_len, kind, "main")
                } else {
                    false
                };
                if main_index_sane {
                    iroha_logger::debug!(
                        ?temp_index_path,
                        ?index_path,
                        kind,
                        "skipping temp sidecar index promotion because main index is valid"
                    );
                    false
                } else {
                    Self::promote_sidecar_temp(&temp_index_path, index_path, kind, "index")
                }
            })
        } else {
            false
        };
        if temp_data_exists {
            if temp_index_exists {
                if index_promoted {
                    Self::promote_sidecar_temp(&temp_data_path, data_path, kind, "data");
                } else {
                    warn!(
                        ?temp_data_path,
                        kind,
                        "sidecar temp data exists but index promotion failed; leaving temp data"
                    );
                }
            } else {
                warn!(
                    ?temp_data_path,
                    kind, "sidecar temp data exists without temp index; ignoring temp data"
                );
            }
        }
    }

    fn promote_sidecar_temp(temp_path: &Path, main_path: &Path, kind: &str, label: &str) -> bool {
        if !temp_path.exists() {
            return false;
        }
        if let Err(err) = std::fs::rename(temp_path, main_path) {
            if main_path.exists() {
                if let Err(remove_err) = std::fs::remove_file(main_path) {
                    warn!(
                        ?remove_err,
                        ?main_path,
                        kind,
                        label,
                        "failed to remove sidecar file before promoting temp"
                    );
                    return false;
                }
                if let Err(err) = std::fs::rename(temp_path, main_path) {
                    warn!(
                        ?err,
                        ?temp_path,
                        ?main_path,
                        kind,
                        label,
                        "failed to promote sidecar temp file after removal"
                    );
                    return false;
                }
            } else {
                warn!(
                    ?err,
                    ?temp_path,
                    ?main_path,
                    kind,
                    label,
                    "failed to promote sidecar temp file"
                );
                return false;
            }
        }
        if let Some(parent) = main_path.parent() {
            if let Err(err) = sync_dir(parent) {
                warn!(
                    ?err,
                    ?parent,
                    kind,
                    label,
                    "failed to sync sidecar parent after temp promotion"
                );
            }
        }
        true
    }

    fn sidecar_index_sane_with_label(
        index_path: &Path,
        data_len: u64,
        kind: &str,
        label: &str,
    ) -> bool {
        let mut index = match std::fs::File::open(index_path) {
            Ok(file) => file,
            Err(err) => {
                warn!(
                    ?err,
                    ?index_path,
                    kind,
                    label,
                    "failed to open sidecar index"
                );
                return false;
            }
        };
        let index_len = match index.metadata() {
            Ok(meta) => meta.len(),
            Err(err) => {
                warn!(
                    ?err,
                    ?index_path,
                    kind,
                    label,
                    "failed to stat sidecar index"
                );
                return false;
            }
        };
        if index_len == 0 {
            warn!(?index_path, kind, label, "sidecar index is empty");
            return false;
        }
        if index_len % PIPELINE_INDEX_ENTRY_SIZE_U64 != 0 {
            warn!(
                len = index_len,
                ?index_path,
                kind,
                label,
                "sidecar index length misaligned"
            );
            return false;
        }
        let entries = index_len / PIPELINE_INDEX_ENTRY_SIZE_U64;
        let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        for _ in 0..entries {
            if let Err(err) = index.read_exact(&mut buf) {
                warn!(
                    ?err,
                    ?index_path,
                    kind,
                    label,
                    "failed to read sidecar index entry"
                );
                return false;
            }
            let entry = SidecarIndexEntry::from_bytes(buf);
            if entry.len == 0 {
                continue;
            }
            if entry.len > STRICT_INIT_MAX_BLOCK_BYTES {
                warn!(
                    len = entry.len,
                    limit = STRICT_INIT_MAX_BLOCK_BYTES,
                    ?index_path,
                    kind,
                    label,
                    "sidecar index entry length exceeds limit"
                );
                return false;
            }
            let entry_end = if let Some(end) = entry.offset.checked_add(entry.len) {
                end
            } else {
                warn!(
                    offset = entry.offset,
                    len = entry.len,
                    ?index_path,
                    kind,
                    label,
                    "sidecar index entry overflows offset"
                );
                return false;
            };
            if entry_end > data_len {
                warn!(
                    offset = entry.offset,
                    len = entry.len,
                    data_len,
                    ?index_path,
                    kind,
                    label,
                    "sidecar index entry points past data file"
                );
                return false;
            }
        }
        true
    }

    #[allow(clippy::too_many_lines)]
    fn append_indexed_sidecar(
        data_path: &Path,
        index_path: &Path,
        height: u64,
        payload: &[u8],
        kind: &str,
        fsync_mode: FsyncMode,
        retention: Option<NonZeroUsize>,
    ) -> bool {
        // Sidecars are best-effort; only fsync when strict durability is requested.
        let should_sync = matches!(fsync_mode, FsyncMode::On);
        if height == 0 {
            iroha_logger::warn!(height, kind, "refusing to store sidecar for zero height");
            return false;
        }

        Self::recover_indexed_sidecar_artifacts(data_path, index_path, kind);

        let mut index = match std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(index_path)
        {
            Ok(file) => file,
            Err(err) => {
                iroha_logger::warn!(?err, ?index_path, kind, "failed to open sidecar index");
                return false;
            }
        };
        let index_entries = match index.metadata() {
            Ok(meta) => {
                let len = meta.len();
                let remainder = len % PIPELINE_INDEX_ENTRY_SIZE_U64;
                if remainder != 0 {
                    let aligned_len = len - remainder;
                    iroha_logger::warn!(
                        len,
                        aligned_len,
                        ?index_path,
                        kind,
                        "sidecar index length misaligned; truncating trailing bytes"
                    );
                    if let Err(err) = index.set_len(aligned_len) {
                        iroha_logger::warn!(
                            ?err,
                            ?index_path,
                            kind,
                            "failed to truncate misaligned sidecar index"
                        );
                        return false;
                    }
                    aligned_len / PIPELINE_INDEX_ENTRY_SIZE_U64
                } else {
                    len / PIPELINE_INDEX_ENTRY_SIZE_U64
                }
            }
            Err(err) => {
                iroha_logger::warn!(?err, ?index_path, kind, "failed to stat sidecar index");
                return false;
            }
        };

        // We expect entries to be appended strictly in order, starting at height 1.
        let expected_height = index_entries.saturating_add(1);
        if height <= index_entries {
            let entry_pos = (height - 1) * PIPELINE_INDEX_ENTRY_SIZE_U64;
            let mut entry_buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
            if index
                .seek(SeekFrom::Start(entry_pos))
                .and_then(|_| index.read_exact(&mut entry_buf))
                .is_err()
            {
                iroha_logger::warn!(
                    height,
                    ?index_path,
                    kind,
                    "failed to read sidecar index entry for update"
                );
                return false;
            }
            let entry = SidecarIndexEntry::from_bytes(entry_buf);

            let mut data = match std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(data_path)
            {
                Ok(file) => file,
                Err(err) => {
                    iroha_logger::warn!(?err, ?data_path, kind, "failed to open sidecar store");
                    return false;
                }
            };

            let mut matches_existing = false;
            if entry.len > 0 {
                let len_usize = if let Ok(len) = usize::try_from(entry.len) {
                    len
                } else {
                    iroha_logger::warn!(
                        len = entry.len,
                        kind,
                        "sidecar payload length exceeds usize"
                    );
                    return false;
                };
                let data_len = match data.metadata() {
                    Ok(meta) => meta.len(),
                    Err(err) => {
                        iroha_logger::warn!(?err, ?data_path, kind, "failed to stat sidecar store");
                        return false;
                    }
                };
                if entry.offset.saturating_add(entry.len) <= data_len {
                    let mut existing = vec![0u8; len_usize];
                    if data
                        .seek(SeekFrom::Start(entry.offset))
                        .and_then(|_| data.read_exact(&mut existing))
                        .is_ok()
                    {
                        matches_existing = existing == payload;
                    } else {
                        iroha_logger::debug!(
                            height,
                            ?data_path,
                            kind,
                            "failed to read existing sidecar payload; overwriting entry"
                        );
                    }
                } else {
                    iroha_logger::debug!(
                        height,
                        offset = entry.offset,
                        len = entry.len,
                        data_len,
                        ?data_path,
                        kind,
                        "sidecar entry points past data file; overwriting entry"
                    );
                }
            }

            if matches_existing {
                iroha_logger::debug!(
                    height,
                    index_entries,
                    ?index_path,
                    kind,
                    "sidecar already recorded; skipping duplicate append"
                );
                drop(index);
                drop(data);
                if let Some(retention) = retention {
                    if !Self::prune_indexed_sidecars(data_path, index_path, retention, kind) {
                        return false;
                    }
                }
                return true;
            }

            let offset = match data.metadata() {
                Ok(meta) => meta.len(),
                Err(err) => {
                    iroha_logger::warn!(?err, ?data_path, kind, "failed to stat sidecar store");
                    return false;
                }
            };
            let len_u64 = if let Ok(len) = u64::try_from(payload.len()) {
                len
            } else {
                iroha_logger::warn!(
                    len = payload.len(),
                    kind,
                    "sidecar payload length exceeds u64"
                );
                return false;
            };

            if let Err(err) = data
                .seek(SeekFrom::Start(offset))
                .and_then(|_| data.write_all(payload))
            {
                iroha_logger::warn!(?err, ?data_path, kind, "failed to append sidecar payload");
                return false;
            }
            if should_sync {
                if let Err(err) = data.sync_data() {
                    iroha_logger::warn!(?err, ?data_path, kind, "failed to sync sidecar payload");
                    return false;
                }
            }

            let new_entry = SidecarIndexEntry {
                offset,
                len: len_u64,
            };
            if let Err(err) = index
                .seek(SeekFrom::Start(entry_pos))
                .and_then(|_| index.write_all(&new_entry.to_bytes()))
            {
                iroha_logger::warn!(?err, ?index_path, kind, "failed to update sidecar index");
                if let Err(truncate_err) = data.set_len(offset) {
                    iroha_logger::debug!(
                        ?truncate_err,
                        ?data_path,
                        offset,
                        kind,
                        "failed to roll back sidecar payload"
                    );
                }
                if should_sync {
                    let _ = data.sync_data();
                }
                return false;
            }
            if should_sync {
                if let Err(err) = index.sync_data() {
                    iroha_logger::warn!(?err, ?index_path, kind, "failed to sync sidecar index");
                    return false;
                }
            }
            if should_sync {
                if let Some(parent) = data_path.parent() {
                    if let Err(err) = sync_dir(parent) {
                        iroha_logger::warn!(
                            ?err,
                            ?parent,
                            kind,
                            "failed to sync sidecar parent directory after update"
                        );
                        return false;
                    }
                }
            }

            drop(index);
            drop(data);
            if let Some(retention) = retention {
                if !Self::prune_indexed_sidecars(data_path, index_path, retention, kind) {
                    return false;
                }
            }
            return true;
        }
        if height > expected_height {
            let missing = height - expected_height;
            iroha_logger::warn!(
                height,
                missing,
                kind,
                "sidecar gap detected; filling index placeholders"
            );
            let filler = SidecarIndexEntry { offset: 0, len: 0 }.to_bytes();
            for _ in 0..missing {
                if let Err(err) = index
                    .seek(SeekFrom::End(0))
                    .and_then(|_| index.write_all(&filler))
                {
                    iroha_logger::warn!(
                        ?err,
                        ?index_path,
                        kind,
                        "failed to append placeholder sidecar index entries"
                    );
                    return false;
                }
            }
        }

        let mut data = match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(data_path)
        {
            Ok(file) => file,
            Err(err) => {
                iroha_logger::warn!(?err, ?data_path, kind, "failed to open sidecar store");
                return false;
            }
        };

        let offset = match data.metadata() {
            Ok(meta) => meta.len(),
            Err(err) => {
                iroha_logger::warn!(?err, ?data_path, kind, "failed to stat sidecar store");
                return false;
            }
        };
        let len_u64 = if let Ok(len) = u64::try_from(payload.len()) {
            len
        } else {
            iroha_logger::warn!(
                len = payload.len(),
                kind,
                "sidecar payload length exceeds u64"
            );
            return false;
        };

        if let Err(err) = data.write_all(payload) {
            iroha_logger::warn!(?err, ?data_path, kind, "failed to append sidecar payload");
            return false;
        }
        if let Err(err) = data.sync_data() {
            iroha_logger::warn!(?err, ?data_path, kind, "failed to sync sidecar payload");
            return false;
        }

        let entry = SidecarIndexEntry {
            offset,
            len: len_u64,
        };
        if let Err(err) = index
            .seek(SeekFrom::End(0))
            .and_then(|_| index.write_all(&entry.to_bytes()))
        {
            iroha_logger::warn!(?err, ?index_path, kind, "failed to append sidecar index");
            if let Err(truncate_err) = data.set_len(offset) {
                iroha_logger::debug!(
                    ?truncate_err,
                    ?data_path,
                    offset,
                    kind,
                    "failed to roll back sidecar payload"
                );
            }
            return false;
        }
        if let Err(err) = index.sync_data() {
            iroha_logger::warn!(?err, ?index_path, kind, "failed to sync sidecar index");
            return false;
        }
        if let Some(parent) = data_path.parent() {
            if let Err(err) = sync_dir(parent) {
                iroha_logger::warn!(
                    ?err,
                    ?parent,
                    kind,
                    "failed to sync sidecar parent directory after append"
                );
                return false;
            }
        }

        drop(index);
        drop(data);
        if let Some(retention) = retention {
            if !Self::prune_indexed_sidecars(data_path, index_path, retention, kind) {
                return false;
            }
        }

        true
    }

    #[allow(clippy::too_many_lines)]
    fn read_indexed_sidecar<T, F>(
        &self,
        height: u64,
        data_file: &str,
        index_file: &str,
        decoder: F,
        kind: &str,
    ) -> Option<T>
    where
        F: Fn(&[u8]) -> Result<T, norito::Error>,
    {
        if height == 0 {
            return None;
        }

        let mut dir = self.store_dir()?;
        dir.push(PIPELINE_DIR_NAME);
        let data_path = dir.join(data_file);
        let index_path = dir.join(index_file);

        Self::recover_indexed_sidecar_artifacts(&data_path, &index_path, kind);

        let mut index = std::fs::File::open(&index_path).ok()?;
        let index_meta = index.metadata().ok()?;
        let index_len = index_meta.len();
        let remainder = index_len % PIPELINE_INDEX_ENTRY_SIZE_U64;
        if remainder != 0 {
            iroha_logger::warn!(
                len = index_len,
                ?index_path,
                kind,
                "sidecar index length misaligned; ignoring trailing bytes"
            );
        }
        let total_entries = index_len / PIPELINE_INDEX_ENTRY_SIZE_U64;
        if height > total_entries {
            return None;
        }

        let seek_pos = (height - 1) * PIPELINE_INDEX_ENTRY_SIZE_U64;
        let mut entry_buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        if index
            .seek(SeekFrom::Start(seek_pos))
            .and_then(|_| index.read_exact(&mut entry_buf))
            .is_err()
        {
            iroha_logger::warn!(
                height,
                ?index_path,
                kind,
                "failed to read sidecar index entry"
            );
            return None;
        }

        let entry = SidecarIndexEntry::from_bytes(entry_buf);
        if entry.len == 0 {
            iroha_logger::debug!(height, ?index_path, kind, "empty sidecar length; skipping");
            return None;
        }
        if entry.len > STRICT_INIT_MAX_BLOCK_BYTES {
            iroha_logger::warn!(
                height,
                len = entry.len,
                limit = STRICT_INIT_MAX_BLOCK_BYTES,
                ?index_path,
                kind,
                "sidecar length exceeds limit; skipping"
            );
            return None;
        }
        let len_usize = if let Ok(len) = usize::try_from(entry.len) {
            len
        } else {
            iroha_logger::warn!(
                len = entry.len,
                ?index_path,
                kind,
                "sidecar length exceeds usize; skipping"
            );
            return None;
        };

        let mut data = std::fs::File::open(&data_path).ok()?;
        let data_len = data.metadata().ok()?.len();
        let entry_end = entry.offset.saturating_add(entry.len);
        if entry_end > data_len {
            iroha_logger::warn!(
                height,
                offset = entry.offset,
                len = entry.len,
                data_len,
                ?data_path,
                kind,
                "sidecar entry points past data file"
            );
            return None;
        }
        if height > 1 {
            let prev_pos = (height - 2) * PIPELINE_INDEX_ENTRY_SIZE_U64;
            let mut prev_buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
            if index
                .seek(SeekFrom::Start(prev_pos))
                .and_then(|_| index.read_exact(&mut prev_buf))
                .is_err()
            {
                iroha_logger::warn!(
                    height,
                    ?index_path,
                    kind,
                    "failed to read previous sidecar index entry"
                );
                return None;
            }
            let prev = SidecarIndexEntry::from_bytes(prev_buf);
            if prev.len > 0 {
                let prev_end = prev.offset.saturating_add(prev.len);
                if prev_end <= data_len && entry.offset < prev_end && entry_end > prev.offset {
                    iroha_logger::warn!(
                        height,
                        prev_offset = prev.offset,
                        prev_len = prev.len,
                        offset = entry.offset,
                        len = entry.len,
                        ?index_path,
                        kind,
                        "sidecar index entry overlaps previous payload; skipping"
                    );
                    return None;
                }
            }
        }

        let mut payload = vec![0u8; len_usize];
        if data
            .seek(SeekFrom::Start(entry.offset))
            .and_then(|_| data.read_exact(&mut payload))
            .is_err()
        {
            iroha_logger::warn!(height, ?data_path, kind, "failed to read sidecar payload");
            return None;
        }

        match decoder(&payload) {
            Ok(sidecar) => Some(sidecar),
            Err(err) => {
                iroha_logger::warn!(?err, height, ?data_path, kind, "failed to decode sidecar");
                None
            }
        }
    }
}

impl Kura {
    #[allow(clippy::too_many_lines)] // Pruning covers many edge cases in one pass; keep consolidated.
    fn prune_indexed_sidecars(
        data_path: &Path,
        index_path: &Path,
        retention: NonZeroUsize,
        kind: &str,
    ) -> bool {
        let mut index = match std::fs::File::open(index_path) {
            Ok(file) => file,
            Err(err) => {
                iroha_logger::warn!(?err, ?index_path, kind, "failed to open sidecar index");
                return false;
            }
        };
        let index_meta = match index.metadata() {
            Ok(meta) => meta,
            Err(err) => {
                iroha_logger::warn!(?err, ?index_path, kind, "failed to stat sidecar index");
                return false;
            }
        };
        let index_len = index_meta.len();
        let remainder = index_len % PIPELINE_INDEX_ENTRY_SIZE_U64;
        let aligned_len = index_len - remainder;
        if remainder != 0 {
            iroha_logger::warn!(
                len = index_len,
                aligned_len,
                ?index_path,
                kind,
                "sidecar index length misaligned; ignoring trailing bytes"
            );
        }
        let total_entries = aligned_len / PIPELINE_INDEX_ENTRY_SIZE_U64;
        let retention_u64 = retention.get() as u64;
        if total_entries <= retention_u64 {
            return true;
        }
        let keep_from = total_entries.saturating_sub(retention_u64);

        let mut entries = Vec::with_capacity(total_entries as usize);
        let mut entry_buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        for _ in 0..total_entries {
            if let Err(err) = index.read_exact(&mut entry_buf) {
                iroha_logger::warn!(?err, ?index_path, kind, "failed to read sidecar index");
                return false;
            }
            entries.push(SidecarIndexEntry::from_bytes(entry_buf));
        }

        let mut data = match std::fs::File::open(data_path) {
            Ok(file) => file,
            Err(err) => {
                iroha_logger::warn!(?err, ?data_path, kind, "failed to open sidecar store");
                return false;
            }
        };
        let data_len = match data.metadata() {
            Ok(meta) => meta.len(),
            Err(err) => {
                iroha_logger::warn!(?err, ?data_path, kind, "failed to stat sidecar store");
                return false;
            }
        };

        // Keep temp paths distinct: `.with_extension("tmp")` would collapse
        // `*.norito` and `*.index` into the same `*.tmp` filename.
        let temp_data_path = data_path.with_extension("norito.tmp");
        let temp_index_path = index_path.with_extension("index.tmp");
        let mut new_data = match std::fs::File::create(&temp_data_path) {
            Ok(file) => BufWriter::new(file),
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    ?temp_data_path,
                    kind,
                    "failed to create temp sidecar store"
                );
                return false;
            }
        };
        let mut new_index = match std::fs::File::create(&temp_index_path) {
            Ok(file) => BufWriter::new(file),
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    ?temp_index_path,
                    kind,
                    "failed to create temp sidecar index"
                );
                return false;
            }
        };

        let mut new_offset = 0u64;
        let empty_entry = SidecarIndexEntry { offset: 0, len: 0 }.to_bytes();
        for (idx, entry) in entries.iter().enumerate() {
            if (idx as u64) < keep_from || entry.len == 0 {
                if let Err(err) = new_index.write_all(&empty_entry) {
                    iroha_logger::warn!(
                        ?err,
                        ?temp_index_path,
                        kind,
                        "failed to write pruned sidecar index entry"
                    );
                    return false;
                }
                continue;
            }

            if entry.len > STRICT_INIT_MAX_BLOCK_BYTES {
                iroha_logger::warn!(
                    len = entry.len,
                    limit = STRICT_INIT_MAX_BLOCK_BYTES,
                    kind,
                    "sidecar payload length exceeds limit during prune; dropping entry"
                );
                if let Err(err) = new_index.write_all(&empty_entry) {
                    iroha_logger::warn!(
                        ?err,
                        ?temp_index_path,
                        kind,
                        "failed to write pruned sidecar index entry"
                    );
                    return false;
                }
                continue;
            }
            let len = if let Ok(len) = usize::try_from(entry.len) {
                len
            } else {
                iroha_logger::warn!(
                    len = entry.len,
                    kind,
                    "sidecar payload length exceeds usize during prune; dropping entry"
                );
                if let Err(err) = new_index.write_all(&empty_entry) {
                    iroha_logger::warn!(
                        ?err,
                        ?temp_index_path,
                        kind,
                        "failed to write pruned sidecar index entry"
                    );
                    return false;
                }
                continue;
            };
            let entry_end = if let Some(end) = entry.offset.checked_add(entry.len) {
                end
            } else {
                iroha_logger::warn!(
                    offset = entry.offset,
                    len = entry.len,
                    kind,
                    "sidecar payload range overflow during prune; dropping entry"
                );
                if let Err(err) = new_index.write_all(&empty_entry) {
                    iroha_logger::warn!(
                        ?err,
                        ?temp_index_path,
                        kind,
                        "failed to write pruned sidecar index entry"
                    );
                    return false;
                }
                continue;
            };
            if entry_end > data_len {
                iroha_logger::warn!(
                    offset = entry.offset,
                    len = entry.len,
                    data_len,
                    kind,
                    "sidecar payload past data file during prune; dropping entry"
                );
                if let Err(err) = new_index.write_all(&empty_entry) {
                    iroha_logger::warn!(
                        ?err,
                        ?temp_index_path,
                        kind,
                        "failed to write pruned sidecar index entry"
                    );
                    return false;
                }
                continue;
            }
            if let Err(err) = data.seek(SeekFrom::Start(entry.offset)) {
                iroha_logger::warn!(
                    ?err,
                    offset = entry.offset,
                    ?data_path,
                    kind,
                    "failed to seek to pruned sidecar payload"
                );
                return false;
            }
            let mut buf = vec![0u8; len];
            if let Err(err) = data.read_exact(&mut buf) {
                iroha_logger::warn!(
                    ?err,
                    offset = entry.offset,
                    len = entry.len,
                    ?data_path,
                    kind,
                    "failed to read pruned sidecar payload"
                );
                return false;
            }
            if let Err(err) = new_data.write_all(&buf) {
                iroha_logger::warn!(
                    ?err,
                    ?temp_data_path,
                    len = entry.len,
                    kind,
                    "failed to persist pruned sidecar payload"
                );
                return false;
            }

            let new_entry = SidecarIndexEntry {
                offset: new_offset,
                len: entry.len,
            };
            if let Err(err) = new_index.write_all(&new_entry.to_bytes()) {
                iroha_logger::warn!(
                    ?err,
                    ?temp_index_path,
                    kind,
                    "failed to persist pruned sidecar index entry"
                );
                return false;
            }
            new_offset = new_offset.saturating_add(entry.len);
        }

        if let Err(err) = new_data.flush() {
            iroha_logger::warn!(
                ?err,
                ?temp_data_path,
                kind,
                "failed to flush pruned sidecar store"
            );
            return false;
        }
        if let Err(err) = new_index.flush() {
            iroha_logger::warn!(
                ?err,
                ?temp_index_path,
                kind,
                "failed to flush pruned sidecar index"
            );
            return false;
        }
        if let Err(err) = new_data.get_ref().sync_data() {
            iroha_logger::warn!(
                ?err,
                ?temp_data_path,
                kind,
                "failed to sync pruned sidecar store"
            );
            return false;
        }
        if let Err(err) = new_index.get_ref().sync_data() {
            iroha_logger::warn!(
                ?err,
                ?temp_index_path,
                kind,
                "failed to sync pruned sidecar index"
            );
            return false;
        }

        drop(new_data);
        drop(new_index);
        drop(data);
        drop(index);

        if let Err(err) = std::fs::rename(&temp_data_path, data_path) {
            if err.kind() == std::io::ErrorKind::AlreadyExists {
                if let Err(remove_err) = std::fs::remove_file(data_path) {
                    iroha_logger::warn!(
                        ?remove_err,
                        ?data_path,
                        kind,
                        "failed to remove sidecar store before pruned replace"
                    );
                    return false;
                }
                if let Err(rename_err) = std::fs::rename(&temp_data_path, data_path) {
                    iroha_logger::warn!(
                        ?rename_err,
                        ?temp_data_path,
                        ?data_path,
                        kind,
                        "failed to replace sidecar store after removal"
                    );
                    return false;
                }
            } else {
                iroha_logger::warn!(
                    ?err,
                    ?temp_data_path,
                    ?data_path,
                    kind,
                    "failed to replace sidecar store with pruned data"
                );
                let _ = std::fs::remove_file(&temp_data_path);
                let _ = std::fs::remove_file(&temp_index_path);
                return false;
            }
        }
        if let Err(err) = std::fs::rename(&temp_index_path, index_path) {
            if err.kind() == std::io::ErrorKind::AlreadyExists {
                if let Err(remove_err) = std::fs::remove_file(index_path) {
                    iroha_logger::warn!(
                        ?remove_err,
                        ?index_path,
                        kind,
                        "failed to remove sidecar index before pruned replace"
                    );
                    return false;
                }
                if let Err(rename_err) = std::fs::rename(&temp_index_path, index_path) {
                    iroha_logger::warn!(
                        ?rename_err,
                        ?temp_index_path,
                        ?index_path,
                        kind,
                        "failed to replace sidecar index after removal"
                    );
                    iroha_logger::warn!(
                        ?temp_index_path,
                        kind,
                        "leaving temp index for sidecar recovery"
                    );
                    return false;
                }
            } else {
                iroha_logger::warn!(
                    ?err,
                    ?temp_index_path,
                    ?index_path,
                    kind,
                    "failed to replace sidecar index with pruned entries"
                );
                let _ = std::fs::remove_file(&temp_data_path);
                iroha_logger::warn!(
                    ?temp_index_path,
                    kind,
                    "leaving temp index for sidecar recovery"
                );
                return false;
            }
        }
        if let Some(parent) = data_path.parent() {
            if let Err(err) = sync_dir(parent) {
                iroha_logger::warn!(
                    ?err,
                    ?parent,
                    kind,
                    "failed to sync sidecar parent directory after prune"
                );
                return false;
            }
        }
        if let Some(parent) = index_path.parent() {
            if let Err(err) = sync_dir(parent) {
                iroha_logger::warn!(
                    ?err,
                    ?parent,
                    kind,
                    "failed to sync sidecar parent directory after prune"
                );
                return false;
            }
        }

        let pruned = keep_from as usize;
        iroha_logger::debug!(
            kind,
            total_entries,
            retained = retention.get(),
            pruned,
            "pruned sidecar entries past retention"
        );
        true
    }
}

impl BlockStore {
    fn retarget_path(&mut self, path: PathBuf) -> Result<()> {
        self.drop_cached_handles();
        self.path_to_blockchain = path;
        self.da_blocks_dir = self.path_to_blockchain.join(DA_BLOCKS_DIR_NAME);
        self.fsync.clear();
        self.create_files_if_they_do_not_exist()
    }

    /// Create a new block store in `path`.
    pub fn new(store_path: impl AsRef<Path>) -> Self {
        Self::with_fsync(store_path, FsyncMode::On, FSYNC_INTERVAL)
    }

    /// Create a new block store in `path` with an explicit fsync policy.
    pub fn with_fsync(
        store_path: impl AsRef<Path>,
        fsync_mode: FsyncMode,
        fsync_interval: Duration,
    ) -> Self {
        let path_to_blockchain = store_path.as_ref().to_path_buf();
        Self {
            da_blocks_dir: path_to_blockchain.join(DA_BLOCKS_DIR_NAME),
            path_to_blockchain,
            data_file: None,
            index_file: None,
            hashes_file: None,
            fsync: FsyncState::new(fsync_mode, fsync_interval),
            fsync_telemetry: FsyncTelemetry::new(fsync_mode),
            encode_scratch: Vec::new(),
            read_scratch: Vec::new(),
            data_mmap: None,
            data_mmap_len: 0,
            commit_marker_count: 0,
            commit_marker_pending: None,
        }
    }

    fn da_block_path(&self, height: u64) -> PathBuf {
        self.da_blocks_dir.join(format!("{height:020}.norito"))
    }

    fn ensure_da_blocks_dir(&self) -> Result<()> {
        if self.da_blocks_dir.as_os_str().is_empty() {
            return Ok(());
        }
        std::fs::create_dir_all(&self.da_blocks_dir)
            .map_err(|err| Error::MkDir(err, self.da_blocks_dir.clone()))
    }

    fn read_da_block_bytes(&self, height: u64, expected_len: u64) -> Result<Vec<u8>> {
        self.ensure_da_blocks_dir()?;
        let path = self.da_block_path(height);
        let bytes = std::fs::read(&path).map_err(|err| Error::IO(err, path.clone()))?;
        if expected_len > 0 && u64::try_from(bytes.len())? != expected_len {
            warn!(
                height,
                expected_len,
                actual_len = bytes.len(),
                path = %path.display(),
                "DA-backed block payload length mismatched index entry"
            );
        }
        Ok(bytes)
    }

    fn ensure_data_file(&mut self) -> Result<&mut FileWrap> {
        if self.data_file.is_none() {
            let path = self.path_to_blockchain.join(DATA_FILE_NAME);
            self.data_file = Some(FileWrap::open_read_write(path)?);
        }
        Ok(self.data_file.as_mut().expect("handle just initialised"))
    }

    fn ensure_index_file(&mut self) -> Result<&mut FileWrap> {
        if self.index_file.is_none() {
            let path = self.path_to_blockchain.join(INDEX_FILE_NAME);
            self.index_file = Some(FileWrap::open_read_write(path)?);
        }
        Ok(self.index_file.as_mut().expect("handle just initialised"))
    }

    fn ensure_hashes_file(&mut self) -> Result<&mut FileWrap> {
        if self.hashes_file.is_none() {
            let path = self.path_to_blockchain.join(HASHES_FILE_NAME);
            self.hashes_file = Some(FileWrap::open_read_write(path)?);
        }
        Ok(self.hashes_file.as_mut().expect("handle just initialised"))
    }

    fn commit_marker_path(&self) -> PathBuf {
        self.path_to_blockchain.join(COUNT_FILE_NAME)
    }

    fn read_commit_marker(&mut self) -> Result<Option<BlockStoreCommitMarker>> {
        let path = self.commit_marker_path();
        if path.as_os_str().is_empty() {
            return Ok(None);
        }
        let tmp_path = path.with_extension("norito.tmp");
        let mut main_invalid = false;
        match std::fs::read(&path) {
            Ok(bytes) => match norito::decode_from_bytes::<BlockStoreCommitMarker>(&bytes) {
                Ok(marker) => {
                    if marker.version == BlockStoreCommitMarker::VERSION {
                        return Ok(Some(marker));
                    }
                    warn!(
                        version = marker.version,
                        "unsupported block store marker version; ignoring"
                    );
                    main_invalid = true;
                }
                Err(err) => {
                    warn!(
                        ?err,
                        "failed to decode block store marker; ignoring corrupted marker"
                    );
                    main_invalid = true;
                }
            },
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(Error::IO(err, path)),
        }

        if main_invalid {
            if let Err(err) = std::fs::remove_file(&path) {
                warn!(
                    ?err,
                    path = %path.display(),
                    "failed to remove corrupted block store marker"
                );
            }
        }

        match std::fs::read(&tmp_path) {
            Ok(bytes) => match norito::decode_from_bytes::<BlockStoreCommitMarker>(&bytes) {
                Ok(marker) => {
                    if marker.version != BlockStoreCommitMarker::VERSION {
                        warn!(
                            version = marker.version,
                            "unsupported block store temp marker version; ignoring"
                        );
                        return Ok(None);
                    }
                    warn!(
                        path = %tmp_path.display(),
                        "recovered block store marker from temp file"
                    );
                    self.write_commit_marker(marker.count)?;
                    Ok(Some(marker))
                }
                Err(err) => {
                    warn!(
                        ?err,
                        path = %tmp_path.display(),
                        "failed to decode temp block store marker"
                    );
                    let _ = std::fs::remove_file(&tmp_path);
                    Ok(None)
                }
            },
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
            Err(err) => Err(Error::IO(err, tmp_path)),
        }
    }

    fn write_commit_marker(&mut self, count: u64) -> Result<()> {
        let path = self.commit_marker_path();
        if path.as_os_str().is_empty() {
            return Ok(());
        }
        let marker = BlockStoreCommitMarker::new(count);
        let bytes = norito::to_bytes(&marker).map_err(Error::NoritoFrame)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
        }
        let tmp_path = path.with_extension("norito.tmp");
        {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)
                .map_err(|err| Error::IO(err, tmp_path.clone()))?;
            file.write_all(&bytes)
                .and_then(|()| file.flush())
                .and_then(|()| file.sync_data())
                .map_err(|err| Error::IO(err, tmp_path.clone()))?;
        }
        if let Err(err) = std::fs::rename(&tmp_path, &path) {
            if err.kind() == std::io::ErrorKind::AlreadyExists {
                std::fs::remove_file(&path).map_err(|err| Error::IO(err, path.clone()))?;
                std::fs::rename(&tmp_path, &path).map_err(|err| Error::IO(err, path.clone()))?;
            } else {
                return Err(Error::IO(err, path.clone()));
            }
        }
        if let Some(parent) = path.parent() {
            sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
        }
        Ok(())
    }

    fn align_hashes_len(&mut self) -> Result<u64> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(0);
        }
        let hashes_file = self.ensure_hashes_file()?;
        let len = hashes_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        let aligned = len - (len % SIZE_OF_BLOCK_HASH);
        if aligned != len {
            warn!(
                len,
                aligned, "block hashes length misaligned; truncating trailing bytes"
            );
            hashes_file.try_io(|file| file.set_len(aligned))?;
        }
        Ok(aligned / SIZE_OF_BLOCK_HASH)
    }

    fn data_backed_count(&mut self, mut candidate: u64) -> Result<u64> {
        if candidate == 0 {
            return Ok(0);
        }
        let data_len = self.data_file_len()?;
        if data_len == 0 {
            return Ok(0);
        }
        let initial = candidate;
        while candidate > 0 {
            match self.read_block_index(candidate - 1) {
                Ok(index) => {
                    let end = if let Some(end) = index.start.checked_add(index.length) {
                        end
                    } else {
                        candidate = candidate.saturating_sub(1);
                        continue;
                    };
                    if index.length == 0
                        || index.length > STRICT_INIT_MAX_BLOCK_BYTES
                        || end > data_len
                    {
                        candidate = candidate.saturating_sub(1);
                        continue;
                    }
                    break;
                }
                Err(err) => {
                    warn!(
                        ?err,
                        candidate,
                        "failed to read block index while reconciling data length; truncating"
                    );
                    candidate = 0;
                    break;
                }
            }
        }
        if candidate != initial {
            warn!(
                initial,
                candidate,
                data_len,
                "block store data shorter than index; truncating durable count"
            );
        }
        Ok(candidate)
    }

    fn init_commit_marker(&mut self) -> Result<()> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(());
        }

        let logical_count = {
            let index_file = self.ensure_index_file()?;
            let len = index_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
            let aligned = len - (len % BlockIndex::SIZE);
            if aligned != len {
                warn!(
                    len,
                    aligned, "block index length misaligned; truncating trailing bytes"
                );
                index_file.try_io(|file| file.set_len(aligned))?;
            }
            aligned / BlockIndex::SIZE
        };
        self.align_hashes_len()?;
        let data_backed_count = self.data_backed_count(logical_count)?;
        if matches!(self.fsync.mode, FsyncMode::Off) {
            self.write_commit_marker(data_backed_count)?;
            self.truncate_hashes_to_count(data_backed_count)?;
            self.truncate_data_to_index(data_backed_count)?;
            self.commit_marker_count = data_backed_count;
            self.commit_marker_pending = None;
            return Ok(());
        }

        let mut durable_count = if let Some(marker) = self.read_commit_marker()? {
            marker.count
        } else {
            self.write_commit_marker(data_backed_count)?;
            data_backed_count
        };

        if durable_count > data_backed_count {
            warn!(
                durable_count,
                data_backed_count,
                "block store marker exceeds data-backed count; truncating marker"
            );
            durable_count = data_backed_count;
            self.write_commit_marker(durable_count)?;
        }
        if logical_count < durable_count {
            warn!(
                logical_count,
                durable_count, "block store marker exceeds index length; truncating marker"
            );
            durable_count = logical_count;
            self.write_commit_marker(durable_count)?;
        }

        if logical_count > durable_count {
            self.commit_marker_count = durable_count;
            self.commit_marker_pending = None;
            self.prune(durable_count)?;
        }

        self.truncate_hashes_to_count(durable_count)?;
        self.truncate_data_to_index(durable_count)?;

        self.commit_marker_count = durable_count;
        self.commit_marker_pending = None;
        Ok(())
    }

    fn drop_cached_handles(&mut self) {
        self.data_file = None;
        self.index_file = None;
        self.hashes_file = None;
        self.invalidate_data_mmap();
    }

    fn next_fsync_wait(&self) -> Option<Duration> {
        let deadline = self.fsync.deadline()?;
        let now = Instant::now();
        if deadline <= now {
            Some(Duration::ZERO)
        } else {
            deadline.checked_duration_since(now)
        }
    }

    #[cfg(test)]
    fn fsync_pending_for_tests(&self) -> bool {
        !matches!(self.fsync.mode, FsyncMode::Off) && self.fsync.pending_since.is_some()
    }

    fn flush_pending_fsync(&mut self, force: bool) -> Result<()> {
        self.fsync_telemetry.update_mode(self.fsync.mode);
        let now = Instant::now();
        if !self.fsync.is_due(now, force) {
            return Ok(());
        }

        // Sync index last so the commit marker only advances after data/hashes/index are durable.
        self.sync_target(FsyncTarget::Data, Self::ensure_data_file)?;
        self.sync_target(FsyncTarget::Hashes, Self::ensure_hashes_file)?;
        self.sync_target(FsyncTarget::Index, Self::ensure_index_file)?;

        self.commit_pending_marker()?;
        self.fsync.clear();
        Ok(())
    }

    fn commit_pending_marker(&mut self) -> Result<()> {
        let Some(count) = self.commit_marker_pending else {
            return Ok(());
        };
        self.write_commit_marker(count)?;
        self.commit_marker_count = count;
        self.commit_marker_pending = None;
        Ok(())
    }

    fn sync_target(
        &mut self,
        target: FsyncTarget,
        open: impl Fn(&mut Self) -> Result<&mut FileWrap>,
    ) -> Result<()> {
        let start = Instant::now();
        let result = open(self)?.try_io(|inner| inner.sync_data());
        match result {
            Ok(()) => {
                self.fsync_telemetry.record_success(target, start.elapsed());
                Ok(())
            }
            Err(err) => {
                self.fsync_telemetry
                    .record_failure(target, Some(start.elapsed()));
                Err(err)
            }
        }
    }

    fn schedule_fsync_after_write(&mut self) -> Result<()> {
        self.mark_fsync_pending();
        self.flush_pending_fsync(false)
    }

    fn mark_fsync_pending(&mut self) {
        let now = Instant::now();
        self.fsync.record_write(now);
    }

    fn invalidate_data_mmap(&mut self) {
        let _ = self.data_mmap.take();
        self.data_mmap_len = 0;
    }

    fn ensure_data_file_present(&mut self) -> Result<()> {
        let path = self.path_to_blockchain.join(DATA_FILE_NAME);
        match std::fs::metadata(&path) {
            Ok(_) => Ok(()),
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    self.drop_cached_handles();
                }
                Err(Error::IO(err, path))
            }
        }
    }

    fn ensure_data_mmap(&mut self) -> Result<()> {
        self.ensure_data_file_present()?;
        let len = {
            let data_file = self.ensure_data_file()?;
            data_file.try_io(|file| file.metadata().map(|meta| meta.len()))?
        };

        if len == 0 {
            self.invalidate_data_mmap();
            return Ok(());
        }

        if self.data_mmap.as_ref().map(|m| m.len() as u64) == Some(len) {
            self.data_mmap_len = len;
            return Ok(());
        }

        self.invalidate_data_mmap();
        let len_usize: usize = len.try_into()?;
        let mirror = {
            let data_file = self.ensure_data_file()?;
            data_file.try_io(|file| MemoryMirror::from_file(file, len_usize, len))?
        };
        self.data_mmap_len = len;
        self.data_mmap = Some(mirror);
        Ok(())
    }

    /// Read a contiguous range of bytes from the block data file.
    ///
    /// # Errors
    /// Returns an error if the requested range is out of bounds or if the
    /// underlying storage cannot be read.
    pub fn block_bytes(&mut self, start: u64, length: u64) -> Result<&[u8]> {
        if length == 0 {
            self.ensure_data_mmap()?;
            return Ok(&[]);
        }

        self.ensure_data_mmap()?;
        let end = start
            .checked_add(length)
            .ok_or(Error::CorruptedBlockRange {
                start,
                length,
                data_len: self.data_mmap_len,
            })?;
        if end > self.data_mmap_len {
            return Err(Error::CorruptedBlockRange {
                start,
                length,
                data_len: self.data_mmap_len,
            });
        }

        let len_usize: usize = length.try_into()?;
        let start_usize: usize = start.try_into()?;
        let end_usize = start_usize + len_usize;

        if let Some(ref mirror) = self.data_mmap {
            return Ok(mirror.slice(start_usize, end_usize));
        }

        let mut scratch = std::mem::take(&mut self.read_scratch);
        if scratch.len() < len_usize {
            scratch.resize(len_usize, 0);
        }
        self.ensure_data_file()?.try_io(|file| {
            file.seek(SeekFrom::Start(start))?;
            file.read_exact(&mut scratch[..len_usize])
        })?;
        self.read_scratch = scratch;
        Ok(&self.read_scratch[..len_usize])
    }

    /// Read a series of block indices from the block index file and
    /// attempt to fill all of `dest_buffer`.
    ///
    /// # Errors
    /// IO Error.
    pub fn read_block_indices(
        &mut self,
        start_block_height: u64,
        dest_buffer: &mut [BlockIndex],
    ) -> Result<()> {
        let block_count = dest_buffer.len();
        if block_count == 0 {
            return Ok(());
        }

        let start_location = start_block_height * BlockIndex::SIZE;
        let required = BlockIndex::SIZE * block_count as u64;
        let index_file = self.ensure_index_file()?;
        let file_len = index_file.try_io(|f| f.metadata().map(|meta| meta.len()))?;
        if start_location + required > file_len {
            return Err(Error::OutOfBoundsBlockRead {
                start_block_height,
                block_count,
            });
        }

        index_file.try_io(|file| {
            file.seek(SeekFrom::Start(start_location))?;
            let mut buffer = [0; 8];
            for current in dest_buffer.iter_mut() {
                *current = BlockIndex::read(file, &mut buffer)?;
            }
            Ok(())
        })?;

        Ok(())
    }

    /// Call `read_block_indices` with a buffer of one.
    ///
    /// # Errors
    /// IO Error.
    pub fn read_block_index(&mut self, block_height: u64) -> Result<BlockIndex> {
        let mut index = BlockIndex {
            start: 0,
            length: 0,
        };
        self.read_block_indices(block_height, std::slice::from_mut(&mut index))?;
        Ok(index)
    }

    /// Get the number of indices in the index file, which is
    /// calculated as the size of the index file in bytes divided by
    /// `2*size_of(u64)`.
    ///
    /// # Errors
    /// IO Error.
    ///
    /// The most common reason this function fails is
    /// that you did not call `create_files_if_they_do_not_exist`.
    ///
    /// Note that if there is an error, you can be quite sure all
    /// other read and write operations will also fail.
    #[allow(clippy::integer_division)]
    fn read_index_count_from_len(&mut self) -> Result<u64> {
        let index_file = self.ensure_index_file()?;
        let len = index_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        Ok(len / BlockIndex::SIZE)
    }

    /// Return the logical index count based on the index file length.
    ///
    /// # Errors
    /// Returns any underlying IO errors when reading the index file metadata.
    #[allow(clippy::integer_division)]
    pub fn read_index_count(&mut self) -> Result<u64> {
        self.read_index_count_from_len()
    }

    /// Return the durable index count as recorded by the commit marker.
    ///
    /// # Errors
    /// Returns any underlying IO errors when reading the index file metadata.
    pub fn read_durable_index_count(&mut self) -> Result<u64> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(0);
        }
        if matches!(self.fsync.mode, FsyncMode::Off) {
            return self.read_index_count_from_len();
        }
        Ok(self.commit_marker_count)
    }

    /// Read a series of block hashes from the block hashes file
    ///
    /// # Errors
    /// IO Error.
    pub fn read_block_hashes(
        &mut self,
        start_block_height: u64,
        block_count: usize,
    ) -> Result<Vec<HashOf<BlockHeader>>> {
        let hashes_file = self.ensure_hashes_file()?;
        let start_location = start_block_height * SIZE_OF_BLOCK_HASH;

        let required = SIZE_OF_BLOCK_HASH * block_count as u64;
        let file_len = hashes_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;

        if start_location + required > file_len {
            return Err(Error::OutOfBoundsBlockRead {
                start_block_height,
                block_count,
            });
        }

        let mut hashes = Vec::new();
        hashes.try_reserve(block_count)?;
        hashes_file.try_io(|file| {
            file.seek(SeekFrom::Start(start_location))?;
            for _ in 0..block_count {
                let mut buffer = [0; Hash::LENGTH];
                file.read_exact(&mut buffer)?;
                hashes.push(HashOf::from_untyped_unchecked(Hash::prehashed(buffer)));
            }
            Ok(())
        })?;

        Ok(hashes)
    }

    /// Get the number of hashes in the hashes file, which is
    /// calculated as the size of the hashes file in bytes divided by
    /// `size_of(HashOf<BlockHeader>)`.
    ///
    /// # Errors
    /// IO Error.
    ///
    /// The most common reason this function fails is
    /// that you did not call `create_files_if_they_do_not_exist`.
    #[allow(clippy::integer_division)]
    pub fn read_hashes_count(&mut self) -> Result<u64> {
        let hashes_file = self.ensure_hashes_file()?;
        let len = hashes_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        Ok(len / SIZE_OF_BLOCK_HASH)
    }

    fn truncate_hashes_to_count(&mut self, count: u64) -> Result<()> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(());
        }
        let hashes_file = self.ensure_hashes_file()?;
        let new_len = count.saturating_mul(SIZE_OF_BLOCK_HASH);
        let current_len = hashes_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        if new_len < current_len {
            hashes_file.try_io(|file| file.set_len(new_len))?;
        }
        Ok(())
    }

    fn truncate_data_to_index(&mut self, count: u64) -> Result<()> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(());
        }
        if count == 0 {
            self.invalidate_data_mmap();
            let data_file = self.ensure_data_file()?;
            data_file.try_io(|file| file.set_len(0))?;
            return Ok(());
        }
        let last_index = match self.read_block_index(count.saturating_sub(1)) {
            Ok(index) => index,
            Err(err) => {
                warn!(
                    ?err,
                    count, "failed to read last block index while trimming data file"
                );
                return Ok(());
            }
        };
        let target_len = last_index.start.saturating_add(last_index.length);
        let current_len = {
            let data_file = self.ensure_data_file()?;
            data_file.try_io(|file| file.metadata().map(|meta| meta.len()))?
        };
        if current_len > target_len {
            self.invalidate_data_mmap();
            let data_file = self.ensure_data_file()?;
            data_file.try_io(|file| file.set_len(target_len))?;
        }
        Ok(())
    }

    /// Return the current size of the block data file in bytes.
    ///
    /// # Errors
    /// Propagates I/O errors when the metadata cannot be read.
    pub fn data_file_len(&mut self) -> Result<u64> {
        let data_file = self.ensure_data_file()?;
        data_file.try_io(|file| file.metadata().map(|meta| meta.len()))
    }

    /// Return the current size of the block index file in bytes.
    ///
    /// # Errors
    /// Propagates I/O errors when the metadata cannot be read.
    pub fn index_file_len(&mut self) -> Result<u64> {
        let index_file = self.ensure_index_file()?;
        index_file.try_io(|file| file.metadata().map(|meta| meta.len()))
    }

    /// Return the current size of the block hashes file in bytes.
    ///
    /// # Errors
    /// Propagates I/O errors when the metadata cannot be read.
    pub fn hashes_file_len(&mut self) -> Result<u64> {
        let hashes_file = self.ensure_hashes_file()?;
        hashes_file.try_io(|file| file.metadata().map(|meta| meta.len()))
    }

    /// Read block data starting from the
    /// `start_location_in_data_file` in data file in order to fill
    /// `dest_buffer`.
    ///
    /// # Errors
    /// IO Error.
    pub fn read_block_data(
        &mut self,
        start_location_in_data_file: u64,
        dest_buffer: &mut [u8],
    ) -> Result<()> {
        let data_file = self.ensure_data_file()?;
        data_file.try_io(|file| {
            file.seek(SeekFrom::Start(start_location_in_data_file))?;
            file.read_exact(dest_buffer)
        })?;
        Ok(())
    }

    /// Write the index of a single block at the specified `block_height`.
    /// If `block_height` is beyond the end of the index file, attempt to
    /// extend the index file.
    ///
    /// # Errors
    /// IO Error.
    pub fn write_block_index(&mut self, block_height: u64, start: u64, length: u64) -> Result<()> {
        let index_file = self.ensure_index_file()?;
        let start_location = block_height * BlockIndex::SIZE;
        let new_len = start_location + BlockIndex::SIZE;
        let current_len = index_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        if new_len > current_len {
            index_file.try_io(|file| file.set_len(new_len))?;
        }
        index_file.try_io(|file| {
            file.seek(SeekFrom::Start(start_location))?;
            let bytes = BlockIndex { start, length }.encode();
            file.write_all(&bytes)
        })?;
        self.schedule_fsync_after_write()?;
        Ok(())
    }

    /// Change the size of the index file (the value returned by
    /// `read_index_count`).
    ///
    /// # Errors
    /// IO Error.
    ///
    /// The most common reason this function fails is
    /// that you did not call `create_files_if_they_do_not_exist`.
    ///
    /// Note that if there is an error, you can be quite sure all other
    /// read and write operations will also fail.
    pub fn write_index_count(&mut self, new_count: u64) -> Result<()> {
        let index_file = self.ensure_index_file()?;
        let new_byte_size = new_count * BlockIndex::SIZE;
        index_file.try_io(|file| file.set_len(new_byte_size))?;
        Ok(())
    }

    /// Write `block_data` into the data file starting at
    /// `start_location_in_data_file`. Extend the file if
    /// necessary.
    ///
    /// # Errors
    /// IO Error.
    pub fn write_block_data(
        &mut self,
        start_location_in_data_file: u64,
        block_data: &[u8],
    ) -> Result<()> {
        self.invalidate_data_mmap();
        let data_file = self.ensure_data_file()?;
        let end = start_location_in_data_file + block_data.len() as u64;
        let current_len = data_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        if end > current_len {
            data_file.try_io(|file| file.set_len(end))?;
        }
        data_file.try_io(|file| {
            file.seek(SeekFrom::Start(start_location_in_data_file))?;
            file.write_all(block_data)
        })?;
        self.schedule_fsync_after_write()?;
        Ok(())
    }

    /// Write the hash of a single block at the specified `block_height`.
    /// If `block_height` is beyond the end of the index file, attempt to
    /// extend the index file.
    ///
    /// # Errors
    /// IO Error.
    pub fn write_block_hash(&mut self, block_height: u64, hash: HashOf<BlockHeader>) -> Result<()> {
        let hashes_file = self.ensure_hashes_file()?;
        let start_location = block_height * SIZE_OF_BLOCK_HASH;
        let end = start_location + SIZE_OF_BLOCK_HASH;
        let current_len = hashes_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        if end > current_len {
            hashes_file.try_io(|file| file.set_len(end))?;
        }
        hashes_file.try_io(|file| {
            file.seek(SeekFrom::Start(start_location))?;
            file.write_all(hash.as_ref())
        })?;
        self.schedule_fsync_after_write()?;
        Ok(())
    }

    /// Write the hashes to the hashes file overwriting any previous hashes.
    ///
    /// # Errors
    /// IO Error.
    pub fn overwrite_block_hashes(&mut self, hashes: &[HashOf<BlockHeader>]) -> Result<()> {
        let hashes_file = self.ensure_hashes_file()?;
        hashes_file.try_io(|file| {
            file.set_len(0)?;
            file.seek(SeekFrom::Start(0))
        })?;
        hashes_file.try_io(|file| {
            let mut writer = BufWriter::new(&mut *file);
            for hash in hashes {
                writer.write_all(hash.as_ref())?;
            }
            writer.flush()
        })?;
        self.schedule_fsync_after_write()?;
        Ok(())
    }

    /// Create the index and data files if they do not
    /// already exist.
    ///
    /// # Errors
    /// Fails if any of the files don't exist and couldn't be
    /// created.
    pub fn create_files_if_they_do_not_exist(&mut self) -> Result<()> {
        std::fs::create_dir_all(&*self.path_to_blockchain)
            .map_err(|e| Error::MkDir(e, self.path_to_blockchain.clone()))?;
        for name in [INDEX_FILE_NAME, DATA_FILE_NAME, HASHES_FILE_NAME] {
            let path = self.path_to_blockchain.join(name);
            FileWrap::open_with(path, |opts| {
                opts.write(true).truncate(false).create(true);
            })?;
        }
        self.drop_cached_handles();
        self.init_commit_marker()?;
        self.drop_cached_handles();
        Ok(())
    }

    /// Append `block_data` to this block store. First write
    /// the data to the data file and then create a new index
    /// for it in the index file.
    ///
    /// # Errors
    /// Fails if any of the required platform-specific functions
    /// fail.
    pub fn append_block_to_chain(&mut self, block: &SignedBlock) -> Result<()> {
        // Delegate to the batch writer to share fsync/pending logic.
        self.append_block_batch(&[Arc::new(block.clone())])
    }

    /// Append multiple blocks to the chain in a single I/O batch.
    ///
    /// This method mirrors [`Self::append_block_to_chain`] but avoids repeated
    /// `sync_data` calls when persisting a contiguous run of blocks.
    ///
    /// # Errors
    /// Propagates I/O and encoding errors.
    pub fn append_block_batch(&mut self, blocks: &[Arc<SignedBlock>]) -> Result<()> {
        let start_height = self.read_index_count()?;
        self.append_block_batch_at(start_height, blocks, 0)
    }

    #[allow(clippy::too_many_lines)]
    fn append_block_batch_at(
        &mut self,
        start_height: u64,
        blocks: &[Arc<SignedBlock>],
        max_disk_usage_bytes: u64,
    ) -> Result<()> {
        if blocks.is_empty() {
            return Ok(());
        }

        self.invalidate_data_mmap();
        debug!(
            start_height,
            batch_len = blocks.len(),
            "append_block_batch start"
        );
        let start_location_in_data_file = if start_height == 0 {
            0
        } else {
            let mut idx = start_height.saturating_sub(1);
            loop {
                let entry = self.read_block_index(idx)?;
                if !entry.is_evicted() {
                    break entry.start.checked_add(entry.length).ok_or(
                        Error::CorruptedBlockRange {
                            start: entry.start,
                            length: entry.length,
                            data_len: entry.start,
                        },
                    )?;
                }
                if idx == 0 {
                    break 0;
                }
                idx = idx.saturating_sub(1);
            }
        };
        debug!(
            start_height,
            start_location_in_data_file, "append_block_batch computed start location"
        );

        let mut frames = Vec::with_capacity(blocks.len());
        let mut lengths = Vec::with_capacity(blocks.len());
        let mut offsets = Vec::with_capacity(blocks.len());
        let mut hashes = Vec::with_capacity(blocks.len());
        let mut evicted = Vec::with_capacity(blocks.len());

        for (idx, block) in blocks.iter().enumerate() {
            debug!(
                start_height,
                block_idx = idx,
                "append_block_batch encoding block"
            );
            let wire = block.canonical_wire()?;
            let (frame, versioned) = wire.into_parts();
            let frame_len = u64::try_from(frame.len())?;
            let required = frame_len
                .saturating_add(BlockIndex::SIZE)
                .saturating_add(SIZE_OF_BLOCK_HASH);
            let evict = max_disk_usage_bytes > 0 && required > max_disk_usage_bytes;
            frames.push(frame);
            lengths.push(frame_len);
            hashes.push(block.hash());
            evicted.push(evict);
            self.encode_scratch = versioned;
            debug!(
                start_height,
                block_idx = idx,
                frame_len,
                evict,
                "append_block_batch encoded block"
            );
        }

        debug!(
            start_height,
            frames = frames.len(),
            "append_block_batch prepared frames"
        );
        let mut cursor = start_location_in_data_file;
        for (len, evict) in lengths.iter().zip(evicted.iter()) {
            if *evict {
                offsets.push(EVICTED_BLOCK_START);
                continue;
            }
            offsets.push(cursor);
            cursor = cursor.checked_add(*len).ok_or(Error::CorruptedBlockRange {
                start: cursor,
                length: *len,
                data_len: cursor,
            })?;
        }
        let end_pos = cursor;
        debug!(
            start_height,
            start_location = start_location_in_data_file,
            end_pos,
            frames = frames.len(),
            "append_block_batch preparing to write data"
        );

        if evicted.iter().any(|evict| *evict) {
            self.ensure_da_blocks_dir()?;
            for (idx, (frame, evict)) in frames.iter().zip(evicted.iter()).enumerate() {
                if !*evict {
                    continue;
                }
                let height = start_height.saturating_add(idx as u64).saturating_add(1);
                let path = self.da_block_path(height);
                let tmp_path = path.with_extension("norito.tmp");
                if path.exists() {
                    std::fs::remove_file(&path).map_err(|err| Error::IO(err, path.clone()))?;
                }
                let mut tmp_file = FileWrap::open_with(tmp_path.clone(), |opts| {
                    opts.write(true).create(true).truncate(true);
                })?;
                tmp_file.try_io(|file| {
                    file.write_all(frame)?;
                    file.flush()?;
                    file.sync_data()
                })?;
                std::fs::rename(&tmp_path, &path).map_err(|err| Error::IO(err, path.clone()))?;
                if let Some(parent) = path.parent() {
                    sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
                }
            }
        }

        let data_file = self.ensure_data_file()?;
        data_file.try_io(|file| {
            debug!(
                start_height,
                "append_block_batch writing frames to data file"
            );
            file.seek(SeekFrom::Start(start_location_in_data_file))?;
            for (frame, evict) in frames.iter().zip(evicted.iter()) {
                if *evict {
                    continue;
                }
                file.write_all(frame)?;
            }
            debug!(start_height, "append_block_batch flushing data file");
            file.flush()?;
            Ok(())
        })?;
        debug!(
            start_height,
            start_location = start_location_in_data_file,
            end_pos,
            frames = frames.len(),
            "append_block_batch wrote data"
        );
        data_file.try_io(|file| {
            file.seek(SeekFrom::Start(end_pos))?;
            file.set_len(end_pos)
        })?;

        let hashes_file = self.ensure_hashes_file()?;
        let start_location = start_height * SIZE_OF_BLOCK_HASH;
        let new_hashes_len = start_location + SIZE_OF_BLOCK_HASH * u64::try_from(blocks.len())?;
        hashes_file.try_io(|file| file.set_len(new_hashes_len))?;
        hashes_file.try_io(|file| {
            file.seek(SeekFrom::Start(start_location))?;
            for hash in &hashes {
                file.write_all(hash.as_ref())?;
            }
            file.flush()?;
            Ok(())
        })?;
        debug!(
            start_height,
            new_hashes_len, "append_block_batch wrote hashes"
        );

        // Write the index after data + hashes so the commit marker can safely advance.
        let index_file = self.ensure_index_file()?;
        let new_index_len = (start_height + blocks.len() as u64) * BlockIndex::SIZE;
        index_file.try_io(|file| {
            file.seek(SeekFrom::Start(start_height * BlockIndex::SIZE))?;
            for (start, len) in offsets.iter().zip(lengths.iter()) {
                let bytes = BlockIndex {
                    start: *start,
                    length: *len,
                }
                .encode();
                file.write_all(&bytes)?;
            }
            file.flush()?;
            file.set_len(new_index_len)?;
            Ok(())
        })?;
        debug!(
            start_height,
            new_index_len, "append_block_batch wrote index entries"
        );

        let end_height = start_height + blocks.len() as u64;
        self.commit_marker_pending = Some(
            self.commit_marker_pending
                .map_or(end_height, |pending| pending.max(end_height)),
        );
        self.mark_fsync_pending();
        if matches!(self.fsync.mode, FsyncMode::On)
            || (matches!(self.fsync.mode, FsyncMode::Batched)
                && self.fsync.interval == Duration::ZERO)
        {
            self.flush_pending_fsync(false)?;
        }

        debug!(start_height, end_height, "append_block_batch complete");
        Ok(())
    }

    /// Prune the block storage to the given height
    ///
    /// Removes block entries higher than the given height from
    /// the data file, index file, and hashes file.
    ///
    /// This function **does not** fail if the data in files is behind
    /// the given height.
    ///
    /// Note: this function is not used in Iroha (as of writing this), but is
    /// needed for Explorer.
    ///
    /// # Errors
    ///
    /// - If files do not exist (call [`Self::create_files_if_they_do_not_exist`])
    /// - Other IO errors
    pub fn prune(&mut self, height: u64) -> Result<()> {
        self.invalidate_data_mmap();
        let last_block_index: Option<BlockIndex>;
        let pruned_index_count;

        {
            let mut file =
                FileWrap::open_read_write(self.path_to_blockchain.join(INDEX_FILE_NAME))?;
            let len = file.try_io(|f| f.metadata().map(|x| x.len()))?;
            let new_len = (BlockIndex::SIZE * height).min(len);
            file.try_io(|f| f.set_len(new_len))?;

            last_block_index = if new_len > 0 {
                let actual_height = new_len / BlockIndex::SIZE;
                pruned_index_count = actual_height;
                file.try_io(|f| f.seek(SeekFrom::Start((actual_height - 1) * BlockIndex::SIZE)))?;
                let mut buff = [0; 8];
                Some(file.try_io(|f| BlockIndex::read(f, &mut buff))?)
            } else {
                pruned_index_count = 0;
                None
            };
        }

        {
            let mut file =
                FileWrap::open_read_write(self.path_to_blockchain.join(HASHES_FILE_NAME))?;
            let len = file.try_io(|f| f.metadata().map(|x| x.len()))?;
            let new_len = (SIZE_OF_BLOCK_HASH * height).min(len);
            file.try_io(|f| f.set_len(new_len))?;
        }

        {
            let mut file = FileWrap::open_read_write(self.path_to_blockchain.join(DATA_FILE_NAME))?;
            let len = file.try_io(|f| f.metadata().map(|x| x.len()))?;
            let new_len = last_block_index.map_or(0, |x| x.start + x.length).min(len);
            file.try_io(|f| f.set_len(new_len))?;
        }

        self.commit_marker_pending = None;
        self.commit_marker_count = self.commit_marker_count.min(pruned_index_count);
        self.write_commit_marker(self.commit_marker_count)?;

        Ok(())
    }
}

fn create_dir_all_with_context(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).map_err(|err| Error::MkDir(err, path.to_path_buf()))
}

fn sync_dir(path: &Path) -> std::io::Result<()> {
    let file = std::fs::File::open(path)?;
    file.sync_all()
}

fn unique_retired_path(base: &Path, stem: &str, extension: Option<&str>) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_secs())
        .unwrap_or(0);

    let mut counter = 0u32;

    loop {
        let mut name = format!("{stem}_{stamp}");
        if counter > 0 {
            name.push('_');
            name.push_str(&counter.to_string());
        }
        if let Some(ext) = extension {
            name.push('.');
            name.push_str(ext);
        }
        let candidate = base.join(&name);
        if !candidate.exists() {
            return candidate;
        }
        counter = counter.saturating_add(1);
    }
}

/// Helper to reduce boilerplate of file operations while preserving path context.
struct FileWrap {
    path: PathBuf,
    file: std::fs::File,
}

impl std::fmt::Debug for FileWrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileWrap")
            .field("path", &self.path)
            .field("file", &self.file)
            .finish()
    }
}

impl FileWrap {
    fn open_with(path: PathBuf, configure: impl FnOnce(&mut std::fs::OpenOptions)) -> Result<Self> {
        let mut options = std::fs::OpenOptions::new();
        configure(&mut options);
        let file = options.open(path.clone()).add_err_context(&path)?;
        Ok(Self { path, file })
    }

    fn open_read_write(path: PathBuf) -> Result<Self> {
        Self::open_with(path, |opts| {
            opts.write(true).read(true).create(true).truncate(false);
        })
    }

    fn try_io<F, T>(&mut self, f: F) -> Result<T>
    where
        F: FnOnce(&mut std::fs::File) -> std::io::Result<T>,
    {
        let value = f(&mut self.file).add_err_context(&self.path)?;
        Ok(value)
    }
}

type Result<T, E = Error> = std::result::Result<T, E>;
/// Error variants for persistent storage logic
#[derive(thiserror::Error, Debug, displaydoc::Display)]
pub enum Error {
    /// Failed reading/writing {1:?} from disk
    IO(#[source] std::io::Error, PathBuf),
    /// Failed to create the directory {1:?}
    MkDir(#[source] std::io::Error, PathBuf),
    /// Failed to serialize/deserialize versioned payloads
    VersionedCodec(#[from] iroha_version::error::Error),
    /// Failed to frame or deframe Norito payload
    NoritoFrame(#[from] norito::core::Error),
    /// Failed to allocate buffer
    Alloc(#[from] std::collections::TryReserveError),
    /// Tried reading block data out of bounds: start `{start_block_height}`, count `{block_count}`
    OutOfBoundsBlockRead {
        /// The block height from which the read was supposed to start
        start_block_height: u64,
        /// The actual block count
        block_count: usize,
    },
    /// Tried to lock block store by creating a lockfile at {0}, but it already exists
    Locked(PathBuf),
    /// Block writer thread unavailable; persistence notifications cannot be delivered
    BlockWriterUnavailable,
    /// Block writer thread faulted and stopped processing new blocks: {0}
    BlockWriterFaulted(String),
    /// Conversion of wide integer into narrow integer failed. This error cannot be caught at compile time at present
    IntConversion(#[from] std::num::TryFromIntError),
    /// Blocks count differs hashes file and index file
    HashesFileHeightMismatch,
    /// Block index length {length} exceeds strict-init guard {limit} bytes
    CorruptedBlockLength {
        /// Length of the corrupted block index entry in bytes.
        length: u64,
        /// Configured upper bound for permissible block index entries.
        limit: u64,
    },
    /// Block range start {start} + length {length} exceeds data file length `{data_len}` bytes
    CorruptedBlockRange {
        /// Offset in the data file where the range begins.
        start: u64,
        /// Number of bytes that were requested to be read starting at `start`.
        length: u64,
        /// Total number of bytes available in the data file.
        data_len: u64,
    },
    /// Kura storage budget exceeded: limit {limit} bytes, used {used} bytes, required {required} bytes
    StorageBudgetExceeded {
        /// Configured storage cap in bytes.
        limit: u64,
        /// Bytes currently occupied by the block store.
        used: u64,
        /// Bytes required after accepting the next block.
        required: u64,
    },
}

trait AddErrContextExt<T> {
    type Context;

    fn add_err_context(self, context: &Self::Context) -> Result<T, Error>;
}

impl<T> AddErrContextExt<T> for Result<T, std::io::Error> {
    type Context = PathBuf;

    fn add_err_context(self, path: &Self::Context) -> Result<T, Error> {
        self.map_err(|e| Error::IO(e, path.clone()))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        borrow::Cow,
        fs,
        io::{Read, Seek, SeekFrom, Write},
        num::{NonZeroU32, NonZeroUsize},
        str::FromStr,
        sync::Arc,
        thread,
        time::{Duration, Instant},
    };

    use iroha_config::{
        base::WithOrigin,
        kura::{FsyncMode, InitMode},
        parameters::{
            actual::{Kura as KuraConfig, LaneConfig as RuntimeLaneConfig},
            defaults::kura::{
                BLOCK_SYNC_ROSTER_RETENTION, BLOCKS_IN_MEMORY, FSYNC_INTERVAL,
                MERGE_LEDGER_CACHE_CAPACITY, ROSTER_SIDECAR_RETENTION,
            },
        },
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, bls_normal_pop_prove};
    use iroha_data_model::{
        ChainId, Level,
        account::Account,
        consensus::Qc,
        domain::{Domain, DomainId},
        isi::{Log, Upgrade},
        merge::MergeQuorumCertificate,
        nexus::{DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId},
        peer::PeerId,
        prelude::{Executor, IvmBytecode},
        transaction::TransactionBuilder,
    };
    use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry};
    use iroha_telemetry::metrics::Metrics;
    use iroha_test_samples::{
        SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR, gen_account_in,
    };
    use iroha_version::codec::EncodeVersioned;
    use nonzero_ext::nonzero;
    use tempfile::TempDir;

    use super::*;
    use crate::{
        block::{BlockBuilder, ValidBlock},
        prelude::{AcceptedTransaction, StateReadOnly, World},
        query::store::LiveQueryStore,
        smartcontracts::Registrable,
        state::State,
        sumeragi::{
            consensus::{PERMISSIONED_TAG, Phase, QcAggregate},
            network_topology::Topology,
        },
    };

    #[test]
    fn lane_segment_reconciliation_provisions_and_retires_storage() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let store_root = temp_dir.path().join("kura");
        let lane_count = NonZeroU32::new(4).expect("non-zero lane count");

        let lane0 = ModelLaneConfig::default();
        let lane1 = ModelLaneConfig {
            id: LaneId::from(1),
            alias: "beta".to_string(),
            ..ModelLaneConfig::default()
        };
        let initial_catalog =
            LaneCatalog::new(lane_count, vec![lane0.clone(), lane1.clone()]).expect("catalog");
        let initial_lane_config = RuntimeLaneConfig::from_catalog(&initial_catalog);

        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(store_root.clone()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
        };

        let (kura, _) = Kura::new(&kura_cfg, &initial_lane_config).expect("init kura");

        let lane1_entry = initial_lane_config
            .entry(LaneId::from(1))
            .expect("lane 1 entry");
        let lane1_blocks = lane1_entry.blocks_dir(&store_root);
        assert!(
            lane1_blocks.exists(),
            "expected lane 1 blocks directory to be provisioned"
        );

        let lane2 = ModelLaneConfig {
            id: LaneId::from(2),
            alias: "gamma".to_string(),
            ..ModelLaneConfig::default()
        };
        let extended_catalog = LaneCatalog::new(
            lane_count,
            vec![lane0.clone(), lane1.clone(), lane2.clone()],
        )
        .expect("catalog");
        let extended_lane_config = RuntimeLaneConfig::from_catalog(&extended_catalog);
        let lane2_entry = extended_lane_config
            .entry(LaneId::from(2))
            .expect("lane 2 entry");

        kura.reconcile_lane_segments(&[lane2_entry], &[])
            .expect("provision lane 2");

        let lane2_blocks = lane2_entry.blocks_dir(&store_root);
        assert!(
            lane2_blocks.join(INDEX_FILE_NAME).exists(),
            "lane 2 index file missing"
        );
        assert!(
            lane2_blocks.join(DATA_FILE_NAME).exists(),
            "lane 2 data file missing"
        );
        assert!(
            lane2_blocks.join(HASHES_FILE_NAME).exists(),
            "lane 2 hashes file missing"
        );
        assert!(
            lane2_entry.merge_log_path(&store_root).exists(),
            "lane 2 merge ledger missing"
        );

        kura.reconcile_lane_segments(&[], &[lane1_entry])
            .expect("retire lane 1");

        assert!(
            !lane1_blocks.exists(),
            "lane 1 blocks directory should be retired"
        );
        let retired_blocks_root = store_root.join("retired").join("blocks");
        let retired_entries: Vec<_> = std::fs::read_dir(&retired_blocks_root)
            .expect("retired blocks dir")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect retired entries");
        assert!(
            !retired_entries.is_empty(),
            "expected retired lane directory to be archived"
        );
    }

    #[test]
    fn blank_kura_lane_segment_reconciliation_is_noop() {
        static CWD_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
            std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

        struct WorkingDirGuard(std::path::PathBuf);

        impl Drop for WorkingDirGuard {
            fn drop(&mut self) {
                let _ = std::env::set_current_dir(&self.0);
            }
        }

        let _guard = CWD_LOCK.lock().expect("lock cwd");
        let temp_dir = TempDir::new().expect("create temp dir");
        let original_dir = std::env::current_dir().expect("current dir");
        std::env::set_current_dir(temp_dir.path()).expect("set current dir");
        let _restore_dir = WorkingDirGuard(original_dir);

        let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
        let lane0 = ModelLaneConfig::default();
        let lane1 = ModelLaneConfig {
            id: LaneId::from(1),
            alias: "beta".to_string(),
            ..ModelLaneConfig::default()
        };
        let catalog = LaneCatalog::new(lane_count, vec![lane0, lane1]).expect("catalog");
        let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
        let entry = lane_config.entry(LaneId::from(1)).expect("lane entry");

        let kura = Kura::blank_kura_for_testing();
        kura.reconcile_lane_segments(&[entry], &[])
            .expect("no-op reconcile");

        assert!(
            !temp_dir.path().join("blocks").exists(),
            "blank kura must not create lane block directories"
        );
        assert!(
            !temp_dir.path().join("merge_ledger").exists(),
            "blank kura must not create merge-ledger log directories"
        );
    }

    #[test]
    fn lane_segment_reconciliation_propagates_failure() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let store_root = temp_dir.path().join("kura");

        let initial_catalog =
            LaneCatalog::new(nonzero!(1_u32), vec![ModelLaneConfig::default()]).expect("catalog");
        let initial_lane_config = RuntimeLaneConfig::from_catalog(&initial_catalog);

        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(store_root.clone()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
        };

        let (kura, _) = Kura::new(&kura_cfg, &initial_lane_config).expect("init kura");

        let extended_catalog = LaneCatalog::new(
            nonzero!(2_u32),
            vec![
                ModelLaneConfig::default(),
                ModelLaneConfig {
                    id: LaneId::from(1),
                    alias: "conflict".to_string(),
                    ..ModelLaneConfig::default()
                },
            ],
        )
        .expect("catalog");
        let extended_lane_config = RuntimeLaneConfig::from_catalog(&extended_catalog);
        let conflicting_entry = extended_lane_config
            .entry(LaneId::from(1))
            .expect("lane entry");

        let conflict_dir = conflicting_entry.blocks_dir(&store_root);
        if let Some(parent) = conflict_dir.parent() {
            std::fs::create_dir_all(parent).expect("create parent dir");
        }
        std::fs::File::create(&conflict_dir).expect("seed conflicting file");

        let err = kura
            .reconcile_lane_segments(&[conflicting_entry], &[])
            .expect_err("expected lane provisioning to surface error");
        match err {
            Error::MkDir(_, path) => assert_eq!(path, conflict_dir),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn lane_segment_relabel_updates_primary_directory() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let store_root = temp_dir.path().join("kura");

        let initial_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![ModelLaneConfig {
                alias: "Alpha Lane".to_string(),
                ..ModelLaneConfig::default()
            }],
        )
        .expect("initial catalog");
        let initial_lane_config = RuntimeLaneConfig::from_catalog(&initial_catalog);
        let initial_entry = initial_lane_config
            .entry(LaneId::SINGLE)
            .expect("lane entry");

        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(store_root.clone()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
        };

        let (kura, _) = Kura::new(&kura_cfg, &initial_lane_config).expect("init kura");
        let old_dir = initial_entry.blocks_dir(&kura.store_root);
        let old_merge = initial_entry.merge_log_path(&kura.store_root);
        assert!(old_dir.exists(), "expected initial lane directory to exist");
        assert!(old_merge.exists(), "expected initial merge log to exist");

        let updated_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![ModelLaneConfig {
                alias: "Payments Lane".to_string(),
                ..ModelLaneConfig::default()
            }],
        )
        .expect("updated catalog");
        let updated_lane_config = RuntimeLaneConfig::from_catalog(&updated_catalog);
        let updated_entry = updated_lane_config
            .entry(LaneId::SINGLE)
            .expect("lane entry");

        kura.relabel_lane_segments(&[(initial_entry, updated_entry)])
            .expect("relabel lane storage");

        let new_dir = updated_entry.blocks_dir(&kura.store_root);
        let new_merge = updated_entry.merge_log_path(&kura.store_root);
        assert!(
            new_dir.exists(),
            "expected relabelled lane directory to exist"
        );
        assert!(!old_dir.exists(), "expected old lane directory to be moved");
        assert_eq!(
            *kura.active_blocks_dir.lock(),
            new_dir,
            "active lane path should be updated"
        );
        assert_eq!(
            kura.block_store.lock().path_to_blockchain,
            new_dir,
            "block store should retarget to new directory"
        );
        assert!(new_merge.exists(), "expected relabelled merge log to exist");
        assert!(!old_merge.exists(), "expected old merge log to be moved");
        assert_eq!(
            *kura.active_merge_path.lock(),
            new_merge,
            "active merge log path should be updated"
        );
    }

    #[test]
    fn block_bytes_returns_memory_mapped_slice() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let mut store = new_block_store(&temp_dir);
        store
            .create_files_if_they_do_not_exist()
            .expect("initialise store files");

        let payload = b"test block payload";
        store
            .write_block_data(0, payload.as_ref())
            .expect("write payload");

        let (slice_ptr, slice_len) = {
            let slice = store
                .block_bytes(0, payload.len() as u64)
                .expect("read payload");
            assert_eq!(slice, payload);
            (slice.as_ptr(), slice.len())
        };

        let mirror = store
            .data_mmap
            .as_ref()
            .expect("mirror should be initialised after block_bytes()");
        assert_eq!(mirror.kind(), MemoryMirrorKind::MemoryMapped);
        assert_eq!(mirror.len(), payload.len());
        let mirror_slice = mirror.slice(0, mirror.len());
        assert_eq!(mirror_slice, payload);
        assert_eq!(slice_len, payload.len());
        assert_eq!(slice_ptr, mirror_slice.as_ptr());
        assert_eq!(store.data_mmap_len, payload.len() as u64);
    }

    #[test]
    fn memory_mirror_updates_after_appending_data() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let mut store = new_block_store(&temp_dir);
        store
            .create_files_if_they_do_not_exist()
            .expect("initialise store files");

        let initial = b"initial payload";
        store
            .write_block_data(0, initial.as_ref())
            .expect("write initial payload");

        {
            let slice = store
                .block_bytes(0, initial.len() as u64)
                .expect("prime mirror with initial payload");
            assert_eq!(slice, initial);
        }
        let expected_initial_len = initial.len();
        let mirror = store
            .data_mmap
            .as_ref()
            .expect("mirror initialised after first read");
        assert_eq!(mirror.len(), expected_initial_len);
        assert_eq!(mirror.kind(), MemoryMirrorKind::MemoryMapped);

        let appended = b" appended payload";
        store
            .write_block_data(initial.len() as u64, appended.as_ref())
            .expect("append payload");

        let total_len = (initial.len() + appended.len()) as u64;
        let combined = {
            let slice = store
                .block_bytes(0, total_len)
                .expect("read combined payload");
            assert_eq!(slice.len(), initial.len() + appended.len());
            slice.to_vec()
        };
        let mirror = store
            .data_mmap
            .as_ref()
            .expect("mirror should be remapped after append");
        assert_eq!(mirror.kind(), MemoryMirrorKind::MemoryMapped);
        assert_eq!(mirror.len(), initial.len() + appended.len());
        let mut expected = Vec::with_capacity(initial.len() + appended.len());
        expected.extend_from_slice(initial);
        expected.extend_from_slice(appended);
        assert_eq!(mirror.slice(0, mirror.len()), expected.as_slice());
        assert_eq!(combined, expected);
        assert_eq!(store.data_mmap_len, total_len);
    }

    fn indices<const N: usize>(value: [(u64, u64); N]) -> [BlockIndex; N] {
        let mut ret = [BlockIndex {
            start: 0,
            length: 0,
        }; N];
        for idx in 0..value.len() {
            ret[idx] = value[idx].into();
        }
        ret
    }

    fn wait_for_block_hash(kura: &Arc<Kura>, height: usize, expected: HashOf<BlockHeader>) {
        let deadline = Instant::now() + Duration::from_secs(5);
        let target_index = height
            .checked_sub(1)
            .expect("block height should be non-zero");

        loop {
            {
                let mut store = kura.block_store.lock();
                if let Ok(count) = store.read_index_count() {
                    if count > target_index as u64 {
                        if let Ok(hashes) = store.read_block_hashes(target_index as u64, 1) {
                            if hashes.first().copied() == Some(expected) {
                                return;
                            }
                        }
                    }
                }
            }

            let now = Instant::now();
            assert!(
                now < deadline,
                "Timed out waiting for block {height} to persist"
            );

            thread::sleep(Duration::from_millis(10));
        }
    }

    impl PartialEq for BlockIndex {
        fn eq(&self, other: &Self) -> bool {
            self.start == other.start && self.length == other.length
        }
    }

    impl PartialEq<(u64, u64)> for BlockIndex {
        fn eq(&self, other: &(u64, u64)) -> bool {
            self.start == other.0 && self.length == other.1
        }
    }

    impl From<(u64, u64)> for BlockIndex {
        fn from(value: (u64, u64)) -> Self {
            Self {
                start: value.0,
                length: value.1,
            }
        }
    }

    fn primary_blocks_dir(dir: &TempDir) -> PathBuf {
        let lane_cfg = RuntimeLaneConfig::default();
        let blocks_dir = lane_cfg.primary().blocks_dir(dir.path());
        std::fs::create_dir_all(&blocks_dir).unwrap();
        blocks_dir
    }

    fn new_block_store(dir: &TempDir) -> BlockStore {
        let blocks_dir = primary_blocks_dir(dir);
        BlockStore::new(&blocks_dir)
    }

    fn populate_store(dir: &TempDir, count: usize) {
        let blocks_dir = primary_blocks_dir(dir);
        let mut block_store = BlockStore::new(&blocks_dir);
        block_store.create_files_if_they_do_not_exist().unwrap();

        let leader_key = KeyPair::random();
        let mut prev_hash = None;

        for _ in 0..count {
            let block: SignedBlock =
                ValidBlock::new_dummy_and_modify_header(leader_key.private_key(), |header| {
                    header.set_prev_block_hash(prev_hash);
                })
                .into();
            prev_hash = Some(block.hash());
            block_store.append_block_to_chain(&block).unwrap();
        }
    }

    fn sample_merge_entry(epoch: u64) -> MergeLedgerEntry {
        let epoch_u8 = u8::try_from(epoch).expect("test epoch must fit in a u8");
        let epoch_plus_one = epoch_u8
            .checked_add(1)
            .expect("test epoch offset 1 must fit in a u8");
        let epoch_plus_three = epoch_u8
            .checked_add(3)
            .expect("test epoch offset 3 must fit in a u8");
        let lane_snapshots = vec![iroha_data_model::merge::MergeLaneSnapshot {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            lane_block_height: epoch,
            tip_hash: HashOf::from_untyped_unchecked(Hash::new([epoch_u8])),
            merge_hint_root: Hash::new([epoch_plus_one]),
        }];
        let merge_hint_roots: Vec<Hash> = lane_snapshots
            .iter()
            .map(|snapshot| snapshot.merge_hint_root)
            .collect();
        let global_state_root = reduce_merge_hint_roots(&merge_hint_roots);
        MergeLedgerEntry {
            epoch_id: epoch,
            lane_snapshots,
            global_state_root,
            merge_qc: MergeQuorumCertificate::new(
                epoch,
                epoch,
                vec![0x01],
                vec![0xAA, 0xBB],
                Hash::new([epoch_plus_three]),
            ),
        }
    }

    #[test]
    fn read_and_write_to_blockchain_index() {
        let dir = tempfile::tempdir().unwrap();
        let mut block_store = BlockStore::new(dir.path());
        block_store.create_files_if_they_do_not_exist().unwrap();

        block_store.write_block_index(0, 5, 7).unwrap();
        assert_eq!(block_store.read_block_index(0).unwrap(), (5, 7));

        block_store.write_block_index(0, 2, 9).unwrap();
        assert_ne!(block_store.read_block_index(0).unwrap(), (5, 7));

        block_store.write_block_index(3, 1, 2).unwrap();
        block_store.write_block_index(2, 6, 3).unwrap();

        assert_eq!(block_store.read_block_index(0).unwrap(), (2, 9));
        assert_eq!(block_store.read_block_index(2).unwrap(), (6, 3));
        assert_eq!(block_store.read_block_index(3).unwrap(), (1, 2));

        // or equivalent
        {
            let should_be = indices([(2, 9), (0, 0), (6, 3), (1, 2)]);
            let mut is = indices([(0, 0), (0, 0), (0, 0), (0, 0)]);

            block_store.read_block_indices(0, &mut is).unwrap();
            assert_eq!(should_be, is);
        }

        assert_eq!(block_store.read_index_count().unwrap(), 4);
        block_store.write_index_count(0).unwrap();
        assert_eq!(block_store.read_index_count().unwrap(), 0);
        block_store.write_index_count(12).unwrap();
        assert_eq!(block_store.read_index_count().unwrap(), 12);
    }

    #[test]
    fn block_index_encoding_is_fixed_little_endian_layout() {
        let entry = BlockIndex {
            start: 0x0102_0304_0506_0708,
            length: 0x1112_1314_1516_1718,
        };
        let bytes = entry.encode();

        assert_eq!(bytes.len() as u64, BlockIndex::SIZE);
        assert_eq!(
            &bytes[..core::mem::size_of::<u64>()],
            &entry.start.to_le_bytes()
        );
        assert_eq!(
            &bytes[core::mem::size_of::<u64>()..],
            &entry.length.to_le_bytes()
        );
    }

    #[test]
    fn merge_ledger_entries_persist_across_restart() {
        use iroha_config::{
            base::WithOrigin,
            kura::InitMode,
            parameters::{actual::Kura as Config, defaults::kura::MERGE_LEDGER_CACHE_CAPACITY},
        };

        let dir = tempfile::tempdir().expect("tempdir");
        let config = Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(dir.path().to_path_buf()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
        };

        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("init kura");
        let block1: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let block2: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let entry1 = sample_merge_entry(1);
        kura.store_block_with_merge_entry(block1, &entry1)
            .expect("store block+entry1");
        let entry2 = sample_merge_entry(2);
        kura.store_block_with_merge_entry(block2, &entry2)
            .expect("store block+entry2");
        assert_eq!(kura.merge_ledger_snapshot().len(), 2);

        drop(kura);

        let (kura_reloaded, _) =
            Kura::new(&config, &RuntimeLaneConfig::default()).expect("reopen kura");
        let snapshot = kura_reloaded.merge_ledger_snapshot();
        // Without persisting paired blocks, the merge log is trimmed on restart to
        // preserve consistency with the block store.
        assert!(
            snapshot.is_empty(),
            "merge log without matching blocks is pruned on restart"
        );
    }

    #[test]
    fn store_block_with_merge_entry_appends_log() {
        let kura = Kura::blank_kura_for_testing();
        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let entry = sample_merge_entry(7);
        let expected = entry.clone();

        kura.store_block_with_merge_entry(block, &entry)
            .expect("store block with merge entry");

        assert_eq!(kura.blocks_count(), 1);
        assert_eq!(kura.merge_ledger_snapshot(), vec![expected]);
    }

    #[test]
    fn store_block_injected_failure_aborts_enqueue() {
        let kura = Kura::blank_kura_for_testing();
        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();

        kura.fail_next_store_for_tests();
        let result = kura.store_block(block.clone());
        assert!(result.is_err());
        assert_eq!(
            kura.blocks_count(),
            0,
            "failing append should not enqueue blocks in memory"
        );
        assert!(kura.merge_ledger_snapshot().is_empty());
    }

    #[test]
    fn store_block_rejects_when_budget_exceeded() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: iroha_config::base::util::Bytes(1),
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
        };
        let (mut kura, _) =
            Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
        let baseline = kura.kura_disk_usage_bytes().expect("baseline usage");
        Arc::get_mut(&mut kura)
            .expect("exclusive kura handle")
            .max_disk_usage_bytes = baseline.saturating_add(kura_cfg.max_disk_usage_bytes.get());
        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        kura.attach_telemetry(telemetry);

        let err = kura
            .store_block(block)
            .expect_err("budgeted kura should reject new blocks");
        assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
        assert_eq!(kura.blocks_count(), 0);
        let (persisted_count, unindexed_bytes) = kura
            .persisted_count_and_unindexed_bytes()
            .expect("persisted count");
        let pending_bytes = kura
            .pending_block_bytes(persisted_count, unindexed_bytes)
            .expect("pending bytes");
        let used = kura.kura_disk_usage_bytes().expect("kura bytes");
        let expected_used = used.saturating_add(pending_bytes);
        assert_eq!(
            metrics
                .storage_budget_bytes_used
                .with_label_values(&["kura"])
                .get(),
            expected_used
        );
        assert_eq!(
            metrics
                .storage_budget_bytes_limit
                .with_label_values(&["kura"])
                .get(),
            kura.max_disk_usage_bytes
        );
        assert_eq!(
            metrics
                .storage_budget_exceeded_total
                .with_label_values(&["kura"])
                .get(),
            1
        );
    }

    #[test]
    fn store_block_with_merge_entry_counts_budget() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let block_required = {
            let wire = block.canonical_wire().expect("block wire");
            let (frame, _) = wire.into_parts();
            let frame_len = u64::try_from(frame.len()).expect("frame length");
            frame_len
                .saturating_add(BlockIndex::SIZE)
                .saturating_add(SIZE_OF_BLOCK_HASH)
        };
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: iroha_config::base::util::Bytes(block_required),
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
        };
        let (mut kura, _) =
            Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
        let baseline = kura.kura_disk_usage_bytes().expect("baseline usage");
        Arc::get_mut(&mut kura)
            .expect("exclusive kura handle")
            .max_disk_usage_bytes = baseline.saturating_add(block_required);
        kura.store_block(block).expect("budgeted store block");

        let temp_dir = TempDir::new().expect("create temp dir");
        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let block_required = {
            let wire = block.canonical_wire().expect("block wire");
            let (frame, _) = wire.into_parts();
            let frame_len = u64::try_from(frame.len()).expect("frame length");
            frame_len
                .saturating_add(BlockIndex::SIZE)
                .saturating_add(SIZE_OF_BLOCK_HASH)
        };
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: iroha_config::base::util::Bytes(block_required),
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
        };
        let (mut kura, _) =
            Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
        let baseline = kura.kura_disk_usage_bytes().expect("baseline usage");
        Arc::get_mut(&mut kura)
            .expect("exclusive kura handle")
            .max_disk_usage_bytes = baseline.saturating_add(block_required);
        let entry = sample_merge_entry(7);
        let err = kura
            .store_block_with_merge_entry(block, &entry)
            .expect_err("merge entry should exceed budget");
        assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
    }

    #[test]
    fn store_block_rejects_when_pending_blocks_exceed_budget() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let block1: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let block2: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let block1_required = Kura::block_required_bytes(&block1).expect("block1 required bytes");
        let block2_required = Kura::block_required_bytes(&block2).expect("block2 required bytes");
        let budget_limit = block1_required.max(block2_required);
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: iroha_config::base::util::Bytes(budget_limit),
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
        };
        let (mut kura, _) =
            Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
        let baseline = kura.kura_disk_usage_bytes().expect("baseline usage");
        Arc::get_mut(&mut kura)
            .expect("exclusive kura handle")
            .max_disk_usage_bytes = baseline.saturating_add(budget_limit);

        kura.store_block(block1).expect("store first block");

        let err = kura
            .store_block(block2)
            .expect_err("pending bytes should exceed budget");
        assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
    }

    #[test]
    fn store_block_evicts_when_block_exceeds_budget() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: NonZeroUsize::new(1).expect("non-zero"),
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
        };
        let (mut kura, _) =
            Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
        let used = kura.kura_disk_usage_bytes().expect("baseline usage");
        let overhead = BlockIndex::SIZE.saturating_add(SIZE_OF_BLOCK_HASH);
        let budget_limit = used
            .saturating_add(overhead.saturating_mul(2))
            .saturating_add(1);
        Arc::get_mut(&mut kura)
            .expect("exclusive kura handle")
            .max_disk_usage_bytes = budget_limit;
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let _handle = {
            let _rt_guard = rt.enter();
            Kura::start(kura.clone(), ShutdownSignal::new())
        };

        let make_block = |message: &str, prev: Option<&SignedBlock>| -> Arc<SignedBlock> {
            let tx = TransactionBuilder::new(
                ChainId::from("test"),
                SAMPLE_GENESIS_ACCOUNT_ID.to_owned(),
            )
            .with_instructions([Log::new(Level::INFO, message.to_owned())])
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
            let acc = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
            Arc::new(
                BlockBuilder::new(vec![acc])
                    .chain(0, prev)
                    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
                    .unpack(|_| {})
                    .into(),
            )
        };

        let payload = "x".repeat(4096);
        let block1 = make_block(&payload, None);
        let block2 = make_block(&payload, Some(block1.as_ref()));
        let block1_required = Kura::block_required_bytes(&block1).expect("block1 bytes");
        assert!(
            block1_required > budget_limit,
            "expected block to exceed budget"
        );

        kura.store_block(Arc::clone(&block1))
            .expect("store oversized block1");
        kura.store_block(Arc::clone(&block2))
            .expect("store oversized block2");
        wait_for_block_hash(&kura, 2, block2.hash());

        let (index, da_path) = {
            let mut store = kura.block_store.lock();
            (
                store.read_block_index(0).expect("block index"),
                store.da_block_path(1),
            )
        };
        assert!(index.is_evicted());
        assert!(da_path.exists(), "expected DA payload for block1");
        let da_path2 = {
            let store = kura.block_store.lock();
            store.da_block_path(2)
        };
        assert!(da_path2.exists(), "expected DA payload for block2");
        let da_len1 = std::fs::metadata(&da_path)
            .expect("da payload metadata")
            .len();
        let da_len2 = std::fs::metadata(&da_path2)
            .expect("da payload metadata")
            .len();
        let budget_used = kura.kura_disk_usage_bytes().expect("budget usage");
        let total_used = kura.disk_usage_bytes().expect("total usage");
        let expected_total = budget_used.saturating_add(da_len1).saturating_add(da_len2);
        assert_eq!(
            total_used, expected_total,
            "total usage should include DA payloads"
        );

        let height = NonZeroUsize::new(1).expect("non-zero");
        let block = kura.get_block(height).expect("rehydrate block1");
        assert_eq!(block.hash(), block1.hash());
    }

    #[test]
    fn replace_top_block_evicts_when_budget_exceeded() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
        };
        let (mut kura, _) =
            Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");

        let make_block = |message: &str| -> SignedBlock {
            let tx = TransactionBuilder::new(
                ChainId::from("test"),
                SAMPLE_GENESIS_ACCOUNT_ID.to_owned(),
            )
            .with_instructions([Log::new(Level::INFO, message.to_owned())])
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
            let acc = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
            BlockBuilder::new(vec![acc])
                .chain(0, None)
                .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
                .unpack(|_| {})
                .into()
        };

        let small_block = make_block("short");
        let large_block = make_block(&"x".repeat(4096));
        let small_bytes = Kura::block_required_bytes(&small_block).expect("small bytes");
        let large_bytes = Kura::block_required_bytes(&large_block).expect("large bytes");
        assert!(
            large_bytes > small_bytes,
            "expected large block to be larger"
        );

        let used = kura.kura_disk_usage_bytes().expect("baseline usage");
        let limit = used.saturating_add(small_bytes);
        Arc::get_mut(&mut kura)
            .expect("exclusive kura handle")
            .max_disk_usage_bytes = limit;
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let _handle = {
            let _rt_guard = rt.enter();
            Kura::start(kura.clone(), ShutdownSignal::new())
        };

        kura.store_block(small_block).expect("store small block");

        assert!(
            large_bytes > limit,
            "expected replacement block to exceed budget"
        );
        kura.replace_top_block(large_block.clone())
            .expect("replacement should evict oversized block");
        wait_for_block_hash(&kura, 1, large_block.hash());
        let (index, da_path) = {
            let mut store = kura.block_store.lock();
            (
                store.read_block_index(0).expect("block index"),
                store.da_block_path(1),
            )
        };
        assert!(index.is_evicted());
        assert!(da_path.exists(), "expected DA payload for evicted block");
    }

    #[test]
    fn store_block_rejects_when_sidecar_bytes_exceed_budget() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let budget_limit = Kura::block_required_bytes(&block).expect("block bytes");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: iroha_config::base::util::Bytes(budget_limit),
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
        };
        let (kura, _) =
            Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");

        let blocks_dir = RuntimeLaneConfig::default()
            .primary()
            .blocks_dir(temp_dir.path());
        let pipeline_dir = blocks_dir.join(PIPELINE_DIR_NAME);
        std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
        std::fs::write(pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE), [0u8; 1])
            .expect("write sidecar data");
        kura.refresh_disk_usage_bytes()
            .expect("refresh disk usage after sidecar write");

        let err = kura
            .store_block(block)
            .expect_err("sidecar bytes should exceed budget");
        assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
    }

    #[test]
    fn kura_disk_usage_includes_temp_and_debug_files() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: true,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
        };
        let (kura, _) =
            Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");

        let base = kura.disk_usage_bytes().expect("base usage");
        let blocks_dir = RuntimeLaneConfig::default()
            .primary()
            .blocks_dir(temp_dir.path());

        let debug_path = kura
            .block_plain_text_path
            .lock()
            .clone()
            .expect("debug path");
        std::fs::write(&debug_path, [0u8; 7]).expect("write debug blocks");

        let temp_marker = blocks_dir
            .join(COUNT_FILE_NAME)
            .with_extension("norito.tmp");
        std::fs::write(&temp_marker, [0u8; 5]).expect("write temp marker");

        let pipeline_dir = blocks_dir.join(PIPELINE_DIR_NAME);
        std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
        let temp_sidecar = pipeline_dir
            .join(PIPELINE_SIDECARS_DATA_FILE)
            .with_extension("norito.tmp");
        std::fs::write(&temp_sidecar, [0u8; 3]).expect("write temp sidecar");

        let updated = kura.refresh_disk_usage_bytes().expect("usage with extras");
        let extra = 7u64 + 5 + 3;
        assert_eq!(updated, base.saturating_add(extra));
    }

    #[test]
    fn purge_retired_segments_removes_retired_dir() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
        };
        let (kura, _) =
            Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");

        let retired_dir = temp_dir.path().join("retired").join("blocks");
        std::fs::create_dir_all(&retired_dir).expect("create retired dir");
        std::fs::write(retired_dir.join("dummy.norito"), [0u8; 4]).expect("write retired file");

        assert!(
            kura.purge_retired_segments(),
            "purge should remove retired data"
        );
        assert!(
            !temp_dir.path().join("retired").exists(),
            "retired dir should be removed"
        );
    }

    #[test]
    fn store_block_rejects_when_other_lane_storage_exceeds_budget() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let store_root = temp_dir.path().to_path_buf();
        let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
        let lane0 = ModelLaneConfig::default();
        let lane1 = ModelLaneConfig {
            id: LaneId::from(1),
            alias: "beta".to_string(),
            ..ModelLaneConfig::default()
        };
        let catalog = LaneCatalog::new(lane_count, vec![lane0, lane1]).expect("lane catalog");
        let lane_config = RuntimeLaneConfig::from_catalog(&catalog);

        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let budget_limit = Kura::block_required_bytes(&block).expect("block bytes");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(store_root.clone()),
            max_disk_usage_bytes: iroha_config::base::util::Bytes(budget_limit),
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
        };
        let (kura, _) = Kura::new(&kura_cfg, &lane_config).expect("initialize kura");

        let lane1_entry = lane_config.entry(LaneId::from(1)).expect("lane 1 entry");
        let lane1_blocks = lane1_entry.blocks_dir(&store_root);
        std::fs::write(lane1_blocks.join(DATA_FILE_NAME), [0u8; 1]).expect("seed lane1 data");
        kura.refresh_disk_usage_bytes()
            .expect("refresh disk usage after lane1 seed");

        let err = kura
            .store_block(block)
            .expect_err("lane 1 bytes should exceed budget");
        assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
    }

    #[test]
    fn store_block_reclaims_retired_storage_when_budget_exceeded() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let budget_limit = Kura::block_required_bytes(&block).expect("block bytes");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: iroha_config::base::util::Bytes(budget_limit),
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
        };
        let (mut kura, _) =
            Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
        let baseline = kura.kura_disk_usage_bytes().expect("baseline usage");
        Arc::get_mut(&mut kura)
            .expect("exclusive kura handle")
            .max_disk_usage_bytes = baseline.saturating_add(budget_limit);

        let retired_root = temp_dir.path().join("retired");
        let lane_cfg = RuntimeLaneConfig::default();
        let retired_dir = lane_cfg.primary().blocks_dir(&retired_root);
        std::fs::create_dir_all(&retired_dir).expect("create retired dir");
        std::fs::write(retired_dir.join(DATA_FILE_NAME), [0u8; 1]).expect("seed retired file");
        kura.refresh_disk_usage_bytes()
            .expect("refresh disk usage after retired seed");

        kura.store_block(block)
            .expect("store block after retired purge");
        assert!(
            !temp_dir.path().join("retired").exists(),
            "retired storage should be purged"
        );
    }

    #[test]
    fn store_block_with_merge_entry_propagates_append_error() {
        let kura = Kura::blank_kura_for_testing();
        let failing_dir = tempfile::tempdir().expect("tempdir");
        let log_path = failing_dir.path().join("merge.log");
        std::fs::write(&log_path, []).expect("seed merge log file");
        let failing_log = MergeLedgerLog {
            file: Some(
                FileWrap::open_with(log_path.clone(), |opts| {
                    opts.read(true);
                })
                .expect("open read-only merge log"),
            ),
            entries: Vec::new(),
            cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            total_entries: 0,
            fail_next_append: false,
        };
        *kura.merge_log.lock() = failing_log;

        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let entry = sample_merge_entry(11);

        let err = kura
            .store_block_with_merge_entry(block, &entry)
            .expect_err("merge log append should fail");
        assert!(matches!(err, Error::IO(_, _)));
        assert_eq!(kura.blocks_count(), 0);
    }

    #[test]
    fn merge_log_truncated_when_block_store_pruned() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(dir.path().to_path_buf()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
        };
        let lane_cfg = RuntimeLaneConfig::default();
        let merge_path = lane_cfg.primary().merge_log_path(dir.path());
        {
            let mut merge_log = MergeLedgerLog::open_at(&merge_path, MERGE_LEDGER_CACHE_CAPACITY)
                .expect("prepare merge log");
            merge_log
                .append(&sample_merge_entry(1))
                .expect("append first entry");
            merge_log
                .append(&sample_merge_entry(2))
                .expect("append second entry");
        }

        let (kura, block_count) = Kura::new(&config, &lane_cfg).expect("init kura");
        assert_eq!(block_count.0, 0);
        assert_eq!(kura.merge_ledger_snapshot().len(), 0);
        assert_eq!(
            fs::metadata(&merge_path).expect("merge log metadata").len(),
            0,
            "merge log should be truncated alongside empty block store"
        );
    }

    #[test]
    fn merge_log_truncates_partial_tail_on_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log_path = dir.path().join("merge.log");
        let entry1 = sample_merge_entry(1);

        {
            let mut merge_log = MergeLedgerLog::open_at(&log_path, MERGE_LEDGER_CACHE_CAPACITY)
                .expect("open merge log");
            merge_log.append(&entry1).expect("append entry1");
        }

        let entry2 = sample_merge_entry(2);
        let encoded2 = Encode::encode(&entry2);
        let len2 = u32::try_from(encoded2.len()).expect("entry length fits in u32");
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&log_path)
            .expect("open merge log for truncation");
        file.write_all(&len2.to_le_bytes())
            .expect("write partial length");
        let partial_len = encoded2.len() / 2;
        file.write_all(&encoded2[..partial_len])
            .expect("write partial payload");
        file.flush().expect("flush partial payload");

        let expected_len = 4 + Encode::encode(&entry1).len();
        let mut merge_log = MergeLedgerLog::open_at(&log_path, MERGE_LEDGER_CACHE_CAPACITY)
            .expect("reopen merge log");
        let file_len = fs::metadata(&log_path).expect("merge log metadata").len();
        assert_eq!(file_len, expected_len as u64);
        let snapshot = merge_log.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].epoch_id, entry1.epoch_id);

        let entry3 = sample_merge_entry(3);
        merge_log.append(&entry3).expect("append entry3");
        drop(merge_log);

        let merge_log = MergeLedgerLog::open_at(&log_path, MERGE_LEDGER_CACHE_CAPACITY)
            .expect("reopen after append");
        let snapshot = merge_log.snapshot();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].epoch_id, entry1.epoch_id);
        assert_eq!(snapshot[1].epoch_id, entry3.epoch_id);
    }

    #[test]
    fn merge_log_truncates_oversized_entry_on_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log_path = dir.path().join("merge.log");
        let entry1 = sample_merge_entry(1);
        let encoded1 = Encode::encode(&entry1);

        {
            let mut merge_log = MergeLedgerLog::open_at(&log_path, MERGE_LEDGER_CACHE_CAPACITY)
                .expect("open merge log");
            merge_log.append(&entry1).expect("append entry1");
        }

        let oversize_len =
            u32::try_from(MERGE_LEDGER_MAX_ENTRY_BYTES + 1).expect("max entry size fits in u32");
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&log_path)
            .expect("open merge log for oversize");
        file.write_all(&oversize_len.to_le_bytes())
            .expect("write oversize length");
        file.write_all(&[0u8; 8]).expect("write stub payload");
        file.flush().expect("flush oversize");

        let expected_len = 4 + encoded1.len();
        let merge_log = MergeLedgerLog::open_at(&log_path, MERGE_LEDGER_CACHE_CAPACITY)
            .expect("reopen merge log");
        let file_len = fs::metadata(&log_path).expect("merge log metadata").len();
        assert_eq!(file_len, expected_len as u64);
        let snapshot = merge_log.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].epoch_id, entry1.epoch_id);
    }

    #[test]
    fn merge_log_rejects_oversized_entry() {
        let mut merge_log = MergeLedgerLog::in_memory(MERGE_LEDGER_CACHE_CAPACITY);
        let mut entry = sample_merge_entry(1);
        entry.merge_qc.aggregate_signature = vec![0u8; MERGE_LEDGER_MAX_ENTRY_BYTES];

        let err = merge_log
            .append(&entry)
            .expect_err("oversized merge entry should error");
        assert!(matches!(err, Error::NoritoFrame(_)));
    }

    #[test]
    fn merge_log_respects_cache_capacity() {
        let kura = Kura::blank_kura_for_testing();
        *kura.merge_log.lock() = MergeLedgerLog::in_memory(2);

        kura.append_merge_entry(&sample_merge_entry(1))
            .expect("append entry1");
        kura.append_merge_entry(&sample_merge_entry(2))
            .expect("append entry2");
        kura.append_merge_entry(&sample_merge_entry(3))
            .expect("append entry3");

        let snapshot = kura.merge_ledger_snapshot();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].epoch_id, 2);
        assert_eq!(snapshot[1].epoch_id, 3);
    }

    #[test]
    fn store_block_reports_writer_channel_closed() {
        let kura = Kura::blank_kura_for_testing();
        kura.block_notify_rx.lock().take();

        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let err = kura.store_block(block).expect_err("writer channel closed");
        assert!(matches!(err, Error::BlockWriterUnavailable));
        assert_eq!(
            kura.blocks_count(),
            0,
            "failed enqueue should not mutate block cache"
        );
    }

    #[test]
    fn store_block_reports_writer_fault() {
        let kura = Kura::blank_kura_for_testing();
        kura.mark_writer_fault_for_tests("injected writer failure");

        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let err = kura.store_block(block).expect_err("writer faulted");
        assert!(matches!(err, Error::BlockWriterFaulted(_)));
        assert_eq!(
            kura.blocks_count(),
            0,
            "failed enqueue should not mutate block cache"
        );
    }

    #[test]
    fn replace_top_block_reports_writer_channel_closed() {
        let kura = Kura::blank_kura_for_testing();
        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let block_hash = block.hash();
        kura.store_block(block).expect("store block");
        kura.block_notify_rx.lock().take();

        let replacement: SignedBlock =
            ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let err = kura
            .replace_top_block(replacement)
            .expect_err("writer channel closed");
        assert!(matches!(err, Error::BlockWriterUnavailable));
        assert_eq!(kura.blocks_count(), 1);
        let top_hash = kura.block_data.lock().last().map(|(hash, _)| *hash);
        assert_eq!(top_hash, Some(block_hash));
    }

    #[test]
    fn replace_top_block_reports_writer_fault() {
        let kura = Kura::blank_kura_for_testing();
        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let block_hash = block.hash();
        kura.store_block(block).expect("store block");
        kura.mark_writer_fault_for_tests("injected writer failure");

        let replacement: SignedBlock =
            ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let err = kura
            .replace_top_block(replacement)
            .expect_err("writer faulted");
        assert!(matches!(err, Error::BlockWriterFaulted(_)));
        assert_eq!(kura.blocks_count(), 1);
        let top_hash = kura.block_data.lock().last().map(|(hash, _)| *hash);
        assert_eq!(top_hash, Some(block_hash));
    }

    #[test]
    fn store_block_with_merge_entry_reports_writer_channel_closed() {
        let kura = Kura::blank_kura_for_testing();
        kura.block_notify_rx.lock().take();

        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let entry = sample_merge_entry(7);
        let err = kura
            .store_block_with_merge_entry(block, &entry)
            .expect_err("writer channel closed");
        assert!(matches!(err, Error::BlockWriterUnavailable));
        assert_eq!(kura.blocks_count(), 0);
        assert!(kura.merge_ledger_snapshot().is_empty());
    }

    #[test]
    fn read_and_write_to_blockchain_data_store() {
        let dir = tempfile::tempdir().unwrap();
        let mut block_store = BlockStore::new(dir.path());
        block_store.create_files_if_they_do_not_exist().unwrap();

        block_store
            .write_block_data(43, b"This is some data!")
            .unwrap();

        let mut read_buffer = [0_u8; b"This is some data!".len()];
        block_store.read_block_data(43, &mut read_buffer).unwrap();

        assert_eq!(b"This is some data!", &read_buffer);
    }

    #[test]
    fn block_bytes_matches_direct_read() {
        let dir = tempfile::tempdir().unwrap();
        let mut block_store = BlockStore::new(dir.path());
        block_store.create_files_if_they_do_not_exist().unwrap();

        let dummy_block: SignedBlock =
            ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        block_store.append_block_to_chain(&dummy_block).unwrap();

        let BlockIndex { start, length } = block_store.read_block_index(0).unwrap();
        let len: usize = usize::try_from(length).expect("test block length fits in usize");

        let mut direct = vec![0_u8; len];
        block_store.read_block_data(start, &mut direct).unwrap();

        let slice_bytes = {
            let borrowed = block_store.block_bytes(start, length).unwrap();
            borrowed.to_vec()
        };

        assert_eq!(slice_bytes, direct);
    }

    #[test]
    fn fresh_block_store_has_zero_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let mut block_store = BlockStore::new(dir.path());
        block_store.create_files_if_they_do_not_exist().unwrap();

        assert_eq!(0, block_store.read_index_count().unwrap());
    }

    #[test]
    fn append_block_to_chain_increases_block_count() {
        let dir = tempfile::tempdir().unwrap();
        let mut block_store = BlockStore::new(dir.path());
        block_store.create_files_if_they_do_not_exist().unwrap();

        let dummy_block = ValidBlock::new_dummy(KeyPair::random().private_key()).into();

        let append_count: usize = 35;
        for _ in 0..append_count {
            block_store.append_block_to_chain(&dummy_block).unwrap();
        }

        let index_count =
            usize::try_from(block_store.read_index_count().unwrap()).expect("index count fits");
        assert_eq!(append_count, index_count);
    }

    #[test]
    fn append_block_to_chain_increases_hashes_count() {
        let dir = tempfile::tempdir().unwrap();
        let mut block_store = BlockStore::new(dir.path());
        block_store.create_files_if_they_do_not_exist().unwrap();

        let dummy_block = ValidBlock::new_dummy(KeyPair::random().private_key()).into();

        let append_count = 35;
        for _ in 0..append_count {
            block_store.append_block_to_chain(&dummy_block).unwrap();
        }

        assert_eq!(append_count, block_store.read_hashes_count().unwrap());
    }

    #[test]
    fn append_block_to_chain_write_correct_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let mut block_store = BlockStore::new(dir.path());
        block_store.create_files_if_they_do_not_exist().unwrap();

        let dummy_block = ValidBlock::new_dummy(KeyPair::random().private_key()).into();

        let append_count = 35;
        for _ in 0..append_count {
            block_store.append_block_to_chain(&dummy_block).unwrap();
        }

        let block_hashes = block_store.read_block_hashes(0, append_count).unwrap();

        for hash in block_hashes {
            assert_eq!(hash, dummy_block.hash())
        }
    }

    #[test]
    fn append_block_to_chain_places_blocks_correctly_in_data_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut block_store = BlockStore::new(dir.path());
        block_store.create_files_if_they_do_not_exist().unwrap();

        let dummy_block = ValidBlock::new_dummy(KeyPair::random().private_key()).into();

        let append_count: u64 = 35;
        for _ in 0..append_count {
            block_store.append_block_to_chain(&dummy_block).unwrap();
        }

        let block_wire = dummy_block
            .canonical_wire()
            .expect("canonical wire encoding");
        let block_len = block_wire.as_framed().len() as u64;
        for i in 0..append_count {
            let BlockIndex { start, length } = block_store.read_block_index(i).unwrap();
            assert_eq!(i * block_len, start);
            assert_eq!(block_len, length);
        }
    }

    #[test]
    fn append_block_to_chain_roundtrip_decodes() {
        let dir = tempfile::tempdir().unwrap();
        let mut block_store = BlockStore::new(dir.path());
        block_store.create_files_if_they_do_not_exist().unwrap();

        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        block_store.append_block_to_chain(&block).unwrap();

        let BlockIndex { start, length } = block_store.read_block_index(0).unwrap();
        let len: usize = length.try_into().expect("block length fits in usize");
        let mut bytes = vec![0u8; len];
        block_store.read_block_data(start, &mut bytes).unwrap();

        let versioned = block.encode_versioned();
        let mut payload_cursor = std::io::Cursor::new(&versioned[1..]);
        let decoded_inline = SignedBlock::decode(&mut payload_cursor)
            .expect("decode adaptive payload for inline bytes");
        assert_eq!(decoded_inline.hash(), block.hash());
        assert_eq!(bytes[0], versioned[0]);
        assert!(bytes[1..].starts_with(MAGIC.as_slice()));
        assert_eq!(&bytes[1 + Header::SIZE..], &versioned[1..]);

        let decoded = decode_framed_signed_block(&bytes).expect("decode stored block");
        assert_eq!(decoded.hash(), block.hash());
    }

    #[test]
    fn append_block_batch_persists_all_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let mut block_store = BlockStore::new(dir.path());
        block_store.create_files_if_they_do_not_exist().unwrap();

        let leader = KeyPair::random();
        let mut prev_hash = None;
        let mut blocks = Vec::new();
        for _ in 0..3 {
            let block: Arc<SignedBlock> = Arc::new(
                ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
                    header.set_prev_block_hash(prev_hash);
                })
                .into(),
            );
            prev_hash = Some(block.hash());
            blocks.push(block);
        }

        block_store.append_block_batch(&blocks).unwrap();

        assert_eq!(block_store.read_index_count().unwrap(), 3);
        assert_eq!(block_store.read_hashes_count().unwrap(), 3);
        for (idx, block) in blocks.iter().enumerate() {
            let hash = block_store.read_block_hashes(idx as u64, 1).unwrap();
            assert_eq!(hash, vec![block.hash()]);
        }
    }

    #[test]
    fn append_block_batch_at_rewrites_tail() {
        let dir = tempfile::tempdir().unwrap();
        let mut block_store = BlockStore::new(dir.path());
        block_store.create_files_if_they_do_not_exist().unwrap();

        let leader = KeyPair::random();
        let block1: Arc<SignedBlock> = Arc::new(ValidBlock::new_dummy(leader.private_key()).into());
        let block2: Arc<SignedBlock> = Arc::new(
            ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
                header.set_prev_block_hash(Some(block1.hash()));
            })
            .into(),
        );
        block_store
            .append_block_batch(&[block1.clone(), block2.clone()])
            .unwrap();

        let replacement: Arc<SignedBlock> = Arc::new(
            ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
                header.set_prev_block_hash(Some(block1.hash()));
                header.set_view_change_index(header.view_change_index().saturating_add(1));
            })
            .into(),
        );
        assert_ne!(replacement.hash(), block2.hash(), "replacement must differ");

        block_store
            .append_block_batch_at(1, std::slice::from_ref(&replacement), 0)
            .unwrap();

        assert_eq!(block_store.read_index_count().unwrap(), 2);
        assert_eq!(block_store.read_hashes_count().unwrap(), 2);
        let hash = block_store.read_block_hashes(1, 1).unwrap();
        assert_eq!(hash, vec![replacement.hash()]);

        let BlockIndex { start, length } = block_store.read_block_index(1).unwrap();
        let len: usize = length.try_into().expect("block length fits in usize");
        let mut bytes = vec![0_u8; len];
        block_store.read_block_data(start, &mut bytes).unwrap();
        let decoded = decode_framed_signed_block(&bytes).expect("decode replaced block");
        assert_eq!(decoded.hash(), replacement.hash());
    }

    #[test]
    fn strict_init_kura() {
        let temp_dir = TempDir::new().unwrap();
        Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: iroha_config::base::WithOrigin::inline(
                    temp_dir.path().to_str().unwrap().into(),
                ),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();
    }

    #[test]
    fn kura_not_miss_replace_block() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_time()
            .build()
            .unwrap();

        {
            let _rt_guard = rt.enter();
            let _logger = iroha_logger::test_logger();
        }

        // Create kura and write some blocks
        let temp_dir = TempDir::new().unwrap();
        let [block_genesis, _block, block_soft_fork, block_next] =
            create_blocks(&rt, &temp_dir).try_into().unwrap();

        // Reinitialize kura and check that correct blocks are loaded
        {
            let (kura, block_count) = Kura::new(
                &Config {
                    init_mode: InitMode::Strict,
                    store_dir: iroha_config::base::WithOrigin::inline(
                        temp_dir.path().to_str().unwrap().into(),
                    ),
                    max_disk_usage_bytes:
                        iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                    blocks_in_memory: BLOCKS_IN_MEMORY,
                    debug_output_new_blocks: false,
                    merge_ledger_cache_capacity:
                        iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                    fsync_mode: iroha_config::kura::FsyncMode::Batched,
                    fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

                    block_sync_roster_retention:
                        iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                    roster_sidecar_retention:
                        iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
                },
                &RuntimeLaneConfig::default(),
            )
            .unwrap();

            assert_eq!(block_count.0, 3);

            assert_eq!(
                kura.get_block(nonzero!(1_usize)).unwrap().hash(),
                block_genesis.as_ref().hash()
            );
            assert_eq!(
                kura.get_block(nonzero!(2_usize)).unwrap().hash(),
                block_soft_fork.as_ref().hash()
            );
            assert_eq!(
                kura.get_block(nonzero!(3_usize)).unwrap().hash(),
                block_next.as_ref().hash()
            );
        }
    }

    #[test]
    fn get_block_caches_loaded_block() {
        let temp_dir = TempDir::new().unwrap();
        let block_count = 3usize;
        populate_store(&temp_dir, block_count);

        let (kura, _) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: iroha_config::base::WithOrigin::inline(
                    temp_dir.path().to_str().unwrap().into(),
                ),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let height = NonZeroUsize::new(block_count).unwrap();
        assert_eq!(
            kura.block_data.lock().len(),
            block_count,
            "strict init should load all appended blocks"
        );
        let first = kura.get_block(height).expect("block available");
        let second = kura.get_block(height).expect("cached block");
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn drop_persisted_blocks_keeps_genesis_and_recent_blocks() {
        let mut generator = DummyBlocks::new();
        let mut block_data: BlockData = (0..4)
            .map(|_| {
                let block = generator.next();
                (block.hash(), Some(block))
            })
            .collect();

        Kura::drop_persisted_blocks(&mut block_data, 2, 2);
        assert_eq!(
            block_data
                .iter()
                .filter(|(_, block)| block.is_some())
                .count(),
            4,
            "no blocks should be dropped while within retention"
        );

        Kura::drop_persisted_blocks(&mut block_data, 4, 2);
        assert!(block_data[0].1.is_some(), "genesis block stays cached");
        assert!(
            block_data[1].1.is_none(),
            "oldest non-genesis block should be dropped"
        );
        assert!(block_data[2].1.is_some(), "recent block should stay cached");
        assert!(block_data[3].1.is_some(), "latest block should stay cached");
    }

    #[test]
    fn drop_persisted_blocks_keeps_unpersisted_blocks() {
        let mut generator = DummyBlocks::new();
        let mut block_data: BlockData = (0..6)
            .map(|_| {
                let block = generator.next();
                (block.hash(), Some(block))
            })
            .collect();

        Kura::drop_persisted_blocks(&mut block_data, 4, 2);
        assert!(block_data[0].1.is_some(), "genesis block stays cached");
        assert!(
            block_data[1].1.is_none(),
            "oldest persisted block should be dropped"
        );
        assert!(
            block_data[2].1.is_some(),
            "retained persisted block stays cached"
        );
        assert!(
            block_data[3].1.is_some(),
            "latest persisted block stays cached"
        );
        assert!(block_data[4].1.is_some(), "unpersisted block stays cached");
        assert!(block_data[5].1.is_some(), "unpersisted block stays cached");
    }

    #[test]
    fn get_block_returns_none_when_data_missing() {
        let temp_dir = TempDir::new().unwrap();
        populate_store(&temp_dir, 2);

        let (kura, _) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: iroha_config::base::WithOrigin::inline(
                    temp_dir.path().to_str().unwrap().into(),
                ),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let data_path = primary_blocks_dir(&temp_dir).join(DATA_FILE_NAME);
        std::fs::remove_file(&data_path).unwrap();

        assert!(
            kura.get_block(nonzero!(1_usize)).is_none(),
            "expected missing block to yield None"
        );
    }

    #[test]
    fn eviction_flushes_pending_fsync_before_rewrite() {
        let temp_dir = TempDir::new().unwrap();
        let config = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: NonZeroUsize::new(1).expect("non-zero"),
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: Duration::from_secs(3600),
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
        };

        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("kura init");

        let mut blocks = DummyBlocks::new();
        for _ in 0..3 {
            let block = blocks.next();
            kura.block_store
                .lock()
                .append_block_to_chain(block.as_ref())
                .expect("append block");
        }
        {
            let mut store = kura.block_store.lock();
            store
                .flush_pending_fsync(true)
                .expect("flush pending fsync");
        }

        let block = blocks.next();
        kura.block_store
            .lock()
            .append_block_to_chain(block.as_ref())
            .expect("append block");

        let (durable_before, index_before) = {
            let mut store = kura.block_store.lock();
            let durable = store.read_durable_index_count().expect("durable count");
            let index = store.read_index_count().expect("index count");
            (durable, index)
        };
        assert_eq!(durable_before, 3);
        assert_eq!(index_before, 4);

        let evict_len = {
            let mut store = kura.block_store.lock();
            store.read_block_index(1).expect("block index").length
        };
        kura.evict_block_bodies(evict_len)
            .expect("evict block bodies");

        let index_after = {
            let mut store = kura.block_store.lock();
            store
                .read_index_count()
                .expect("index count after eviction")
        };
        assert_eq!(
            index_after, 4,
            "eviction should not truncate pending index entries"
        );
    }

    #[test]
    fn evicted_block_rehydrates_from_da_store() {
        let temp_dir = TempDir::new().unwrap();
        populate_store(&temp_dir, 4);

        let (kura, _) = Kura::new(
            &KuraConfig {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: NonZeroUsize::new(1).expect("non-zero"),
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: FsyncMode::Batched,
                fsync_interval: FSYNC_INTERVAL,
                block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .expect("kura init");

        let evict_len = {
            let mut store = kura.block_store.lock();
            store.read_block_index(1).expect("block index").length
        };
        let freed = kura
            .evict_block_bodies(evict_len)
            .expect("evict block bodies");
        assert!(
            freed >= evict_len,
            "expected eviction to free at least one block"
        );

        let (evicted_index, da_path) = {
            let mut store = kura.block_store.lock();
            (
                store.read_block_index(1).expect("block index"),
                store.da_block_path(2),
            )
        };
        assert!(evicted_index.is_evicted());
        assert!(da_path.exists(), "expected DA block payload to exist");

        let height = NonZeroUsize::new(2).expect("non-zero");
        let expected_hash = kura.get_block_hash(height).expect("hash available");
        let block = kura.get_block(height).expect("rehydrated block");
        assert_eq!(block.hash(), expected_hash);
    }

    #[test]
    fn block_payload_available_by_hash_tracks_evicted_da_payload() {
        let temp_dir = TempDir::new().unwrap();
        populate_store(&temp_dir, 4);

        let (kura, _) = Kura::new(
            &KuraConfig {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: NonZeroUsize::new(1).expect("non-zero"),
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: FsyncMode::Batched,
                fsync_interval: FSYNC_INTERVAL,
                block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .expect("kura init");

        let evict_len = {
            let mut store = kura.block_store.lock();
            store.read_block_index(1).expect("block index").length
        };
        kura.evict_block_bodies(evict_len)
            .expect("evict block bodies");

        let (da_path, block_hash) = {
            let store = kura.block_store.lock();
            (
                store.da_block_path(2),
                kura.get_block_hash(nonzero!(2_usize))
                    .expect("hash available"),
            )
        };

        assert!(
            da_path.exists(),
            "expected DA payload to exist after eviction"
        );
        assert!(
            kura.block_payload_available_by_hash(block_hash),
            "payload should be available when DA payload exists"
        );

        std::fs::remove_file(&da_path).expect("remove DA payload");
        assert!(
            !kura.block_payload_available_by_hash(block_hash),
            "payload should be unavailable after DA payload removal"
        );
    }

    #[test]
    fn evicted_blocks_survive_restart() {
        let temp_dir = TempDir::new().unwrap();
        populate_store(&temp_dir, 4);

        let config = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: NonZeroUsize::new(1).expect("non-zero"),
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
        };

        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("kura init");
        let evict_len = {
            let mut store = kura.block_store.lock();
            store.read_block_index(1).expect("block index").length
        };
        kura.evict_block_bodies(evict_len)
            .expect("evict block bodies");
        drop(kura);

        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("kura reopen");
        let height = NonZeroUsize::new(2).expect("non-zero");
        let expected_hash = kura.get_block_hash(height).expect("hash available");
        let block = kura
            .get_block(height)
            .expect("rehydrated block after restart");
        assert_eq!(block.hash(), expected_hash);
    }

    #[test]
    fn deep_history_get_block_uses_cached_bytes() {
        const BLOCK_COUNT: usize = 192;
        let temp_dir = TempDir::new().unwrap();
        let mut store = new_block_store(&temp_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        let mut expected_hashes = Vec::with_capacity(BLOCK_COUNT);
        for _ in 0..BLOCK_COUNT {
            let block = blocks.next();
            expected_hashes.push(block.hash());
            store.append_block_to_chain(block.as_ref()).unwrap();
        }

        drop(store);

        let (kura, _) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: iroha_config::base::WithOrigin::inline(
                    temp_dir.path().to_str().unwrap().into(),
                ),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: nonzero!(16_usize),
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let heights: Vec<_> = (1..=BLOCK_COUNT)
            .map(|height| NonZeroUsize::new(height).expect("nonzero height"))
            .collect();

        for _ in 0..3 {
            for (idx, height) in heights.iter().enumerate() {
                let block = kura
                    .get_block(*height)
                    .unwrap_or_else(|| panic!("block missing at height {height}"));
                assert_eq!(block.hash(), expected_hashes[idx]);
            }
        }

        let store_guard = kura.block_store.lock();
        let mirror = store_guard
            .data_mmap
            .as_ref()
            .expect("expected data mirror to be primed");
        assert_eq!(
            mirror.kind(),
            MemoryMirrorKind::MemoryMapped,
            "expected data mirror to use a memory-mapped backend"
        );
        let mapped_len = mirror.len();
        assert_eq!(
            u64::try_from(mapped_len).expect("mirror length fits in u64"),
            store_guard.data_mmap_len,
            "data mirror length should match recorded length"
        );
    }

    #[test]
    fn debug_output_new_blocks_writes_jsonl() {
        let temp_dir = TempDir::new().expect("temp dir");
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let (kura, _) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: iroha_config::base::WithOrigin::inline(
                    temp_dir.path().to_str().unwrap().into(),
                ),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: true,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let _handle = {
            let _rt_guard = rt.enter();
            Kura::start(kura.clone(), ShutdownSignal::new())
        };

        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        let block = Arc::new(block);
        kura.store_block(Arc::clone(&block)).expect("store block");
        wait_for_block_hash(&kura, 1, block.hash());

        let blocks_dir = RuntimeLaneConfig::default()
            .primary()
            .blocks_dir(temp_dir.path());
        let dump_path = blocks_dir.join("blocks.jsonl");
        let contents = fs::read_to_string(&dump_path).expect("read debug block dump");
        let mut lines = contents.lines();
        let first = lines.next().expect("first JSON line");
        assert!(lines.next().is_none(), "expected one JSON line");
        let _: norito::json::Value =
            norito::json::from_slice(first.as_bytes()).expect("valid JSON line");
    }

    #[allow(clippy::too_many_lines)]
    fn create_blocks(rt: &tokio::runtime::Runtime, temp_dir: &TempDir) -> Vec<CommittedBlock> {
        let mut blocks = Vec::new();

        let (leader_public_key, leader_private_key) =
            KeyPair::random_with_algorithm(Algorithm::BlsNormal).into_parts();
        let peer_id = PeerId::new(leader_public_key.clone());
        let topology = Topology::new(vec![peer_id]);
        let topology_entries = vec![GenesisTopologyEntry::new(
            PeerId::new(leader_public_key.clone()),
            bls_normal_pop_prove(&leader_private_key).expect("generate BLS PoP"),
        )];

        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        let (genesis_id, genesis_key_pair) = gen_account_in("genesis");
        let genesis_domain_id = DomainId::from_str("genesis").expect("Valid");
        let genesis_domain = Domain::new(genesis_domain_id.clone()).build(&genesis_id);
        let genesis_account =
            Account::new(genesis_id.clone()).build(&genesis_id);
        let (account_id, account_keypair) = gen_account_in("wonderland");
        let domain_id = DomainId::from_str("wonderland").expect("Valid");
        let domain = Domain::new(domain_id.clone()).build(&genesis_id);
        let account = Account::new(account_id.clone()).build(&genesis_id);

        let live_query_store = {
            let _rt_guard = rt.enter();
            LiveQueryStore::start_test()
        };

        let (kura, block_count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: iroha_config::base::WithOrigin::inline(
                    temp_dir.path().to_str().unwrap().into(),
                ),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();
        assert_eq!(block_count.0, 0);

        let _handle = {
            let _rt_guard = rt.enter();
            Kura::start(kura.clone(), ShutdownSignal::new())
        };

        let state = State::new(
            World::with([domain, genesis_domain], [account, genesis_account], []),
            Arc::clone(&kura),
            live_query_store,
        );

        let genesis =
            GenesisBuilder::new_without_executor(chain_id.clone(), "ivm/libs/not/installed")
                .set_topology(topology_entries)
                .build_and_sign(&genesis_key_pair)
                .expect("genesis block should be built");

        {
            let mut state_block = state.block(genesis.0.header());
            let time_source = TimeSource::new_system();
            let block_genesis = ValidBlock::validate_with_events(
                genesis.0.clone(),
                &topology,
                &chain_id,
                &genesis_id,
                &time_source,
                &mut state_block,
                |_| {},
            )
            .unpack(|_| {})
            .unwrap()
            .commit_unchecked()
            .unpack(|_| {});
            let _events =
                state_block.apply_without_execution(&block_genesis, topology.as_ref().to_owned());
            state_block.commit().unwrap();
            blocks.push(block_genesis.clone());
            kura.store_block(block_genesis.clone())
                .expect("store genesis block");
            wait_for_block_hash(&kura, 1, block_genesis.as_ref().hash());
        }

        let (max_clock_drift, tx_limits) = {
            let view = state.view();
            let params = view.world.parameters.get();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let tx1 = TransactionBuilder::new(chain_id.clone(), account_id.clone())
            .with_instructions([Log::new(Level::INFO, "msg1".to_string())])
            .sign(account_keypair.private_key());

        let tx2 = TransactionBuilder::new(chain_id.clone(), account_id)
            .with_instructions([Log::new(Level::INFO, "msg2".to_string())])
            .sign(account_keypair.private_key());
        let crypto_cfg = state.crypto();
        let tx1 = AcceptedTransaction::accept(
            tx1,
            &chain_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )
        .unwrap();
        let tx2 = AcceptedTransaction::accept(
            tx2,
            &chain_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )
        .unwrap();

        {
            let unverified_block = BlockBuilder::new(vec![tx1.clone()])
                .chain(0, state.view().latest_block().as_deref())
                .sign(&leader_private_key)
                .unpack(|_| {});

            let mut state_block = state.block(unverified_block.header());
            let block = unverified_block
                .validate_and_record_transactions(&mut state_block)
                .unpack(|_| {})
                .commit_unchecked()
                .unpack(|_| {});
            let _events = state_block.apply_without_execution(&block, topology.as_ref().to_owned());
            state_block.commit().unwrap();
            let block_hash = block.as_ref().hash();
            blocks.push(block.clone());
            kura.store_block(block).expect("store block");
            wait_for_block_hash(&kura, 2, block_hash);
        }

        {
            let unverified_block_soft_fork = BlockBuilder::new(vec![tx1])
                .chain(1, Some(&genesis.0))
                .sign(&leader_private_key)
                .unpack(|_| {});

            let mut state_block = state.block_and_revert(unverified_block_soft_fork.header());
            let block_soft_fork = unverified_block_soft_fork
                .validate_and_record_transactions(&mut state_block)
                .unpack(|_| {})
                .commit_unchecked()
                .unpack(|_| {});
            let _events =
                state_block.apply_without_execution(&block_soft_fork, topology.as_ref().to_owned());
            state_block.commit().unwrap();
            let soft_fork_hash = block_soft_fork.as_ref().hash();
            blocks.push(block_soft_fork.clone());
            kura.replace_top_block(block_soft_fork)
                .expect("replace top block");
            wait_for_block_hash(&kura, 2, soft_fork_hash);
        }

        {
            let unverified_block_next = BlockBuilder::new(vec![tx2])
                .chain(0, state.view().latest_block().as_deref())
                .sign(&leader_private_key)
                .unpack(|_| {});

            let mut state_block = state.block(unverified_block_next.header());
            let block_next = unverified_block_next
                .validate_and_record_transactions(&mut state_block)
                .unpack(|_| {})
                .commit_unchecked()
                .unpack(|_| {});
            let _events =
                state_block.apply_without_execution(&block_next, topology.as_ref().to_owned());
            state_block.commit().unwrap();
            let next_hash = block_next.as_ref().hash();
            blocks.push(block_next.clone());
            kura.store_block(block_next).expect("store block");
            wait_for_block_hash(&kura, 3, next_hash);
        }

        {
            let expected_count = kura.blocks_count() as u64;
            let mut store = kura.block_store.lock();
            store
                .flush_pending_fsync(true)
                .expect("flush pending block data for strict reload");
            let durable = store
                .read_durable_index_count()
                .expect("read durable block count");
            assert_eq!(
                durable, expected_count,
                "durable block count should match in-memory block count before reload"
            );
        }

        blocks
    }

    struct DummyBlocks {
        blocks: Vec<Arc<SignedBlock>>,
    }

    impl DummyBlocks {
        fn new() -> Self {
            Self {
                blocks: <_>::default(),
            }
        }

        fn next(&mut self) -> Arc<SignedBlock> {
            let tx = {
                let builder = TransactionBuilder::new(
                    ChainId::from("test"),
                    SAMPLE_GENESIS_ACCOUNT_ID.to_owned(),
                );

                let tx = if self.blocks.is_empty() {
                    builder.with_instructions([Upgrade::new(Executor::new(
                        IvmBytecode::from_compiled(vec![]),
                    ))])
                } else {
                    builder.with_instructions([Log::new(Level::INFO, "test".to_owned())])
                }
                .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());

                AcceptedTransaction::new_unchecked(Cow::Owned(tx))
            };

            let prev = self.blocks.last().cloned();
            let block: SignedBlock = BlockBuilder::new(vec![tx])
                .chain(0, prev.as_ref().map(AsRef::as_ref))
                .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
                .unpack(|_| {})
                .into();

            let block = Arc::new(block);
            self.blocks.push(block.clone());
            block
        }

        fn get(&self, i: usize) -> Option<Arc<SignedBlock>> {
            self.blocks.get(i).cloned()
        }
    }

    fn store_dummy_blocks(kura: &Arc<Kura>, count: usize) -> Vec<HashOf<BlockHeader>> {
        let mut blocks = DummyBlocks::new();
        let mut hashes = Vec::with_capacity(count);
        for _ in 0..count {
            let block = blocks.next();
            let hash = block.hash();
            kura.store_block(block).expect("store block");
            hashes.push(hash);
        }
        hashes
    }

    fn read_block(store: &mut BlockStore, index: usize) -> eyre::Result<SignedBlock> {
        let BlockIndex { start, length } = store.read_block_index(index as u64)?;
        let len: usize = length.try_into().unwrap();
        let mut buff = vec![0_u8; len];
        store.read_block_data(start, &mut buff)?;
        let block = decode_versioned_signed_block(&buff).map_err(eyre::Report::new)?;
        Ok(block)
    }

    #[test]
    fn pipeline_sidecar_roundtrip() {
        use iroha_config::base::WithOrigin;
        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let block_hash = store_dummy_blocks(&kura, 1)[0];
        let sidecar = PipelineRecoverySidecar::new(
            1,
            block_hash,
            PipelineDagSnapshot {
                fingerprint: [0u8; 32],
                key_count: 0,
            },
            Vec::new(),
        );
        kura.write_pipeline_metadata(&sidecar);
        let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
        assert_eq!(got.height, 1);
        assert_eq!(got.block_hash, block_hash);
        assert_eq!(got.dag.key_count, 0);
        assert_eq!(got.format_label(), "pipeline.recovery");
    }

    #[test]
    fn pipeline_sidecar_enqueue_flushes() {
        use iroha_config::base::WithOrigin;
        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let block_hash = store_dummy_blocks(&kura, 1)[0];
        let sidecar = PipelineRecoverySidecar::new(
            1,
            block_hash,
            PipelineDagSnapshot {
                fingerprint: [0u8; 32],
                key_count: 0,
            },
            Vec::new(),
        );
        kura.enqueue_pipeline_metadata(sidecar);
        assert!(kura.read_pipeline_metadata(1).is_none());

        kura.flush_pipeline_sidecars();
        let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
        assert_eq!(got.height, 1);
        assert_eq!(got.block_hash, block_hash);
    }

    #[test]
    fn pipeline_sidecar_promotes_temp_index_on_read() {
        use iroha_config::base::WithOrigin;
        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let block_hash = store_dummy_blocks(&kura, 1)[0];
        let sidecar = PipelineRecoverySidecar::new(
            1,
            block_hash,
            PipelineDagSnapshot {
                fingerprint: [0u8; 32],
                key_count: 0,
            },
            Vec::new(),
        );
        let payload = sidecar.encode_framed().expect("encode sidecar");

        let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
        pipeline_dir.push(PIPELINE_DIR_NAME);
        fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
        let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
        fs::write(&data_path, &payload).expect("write sidecar data");
        std::fs::File::create(&index_path).expect("create empty index");

        let temp_index_path = index_path.with_extension("index.tmp");
        let entry = SidecarIndexEntry {
            offset: 0,
            len: payload.len() as u64,
        }
        .to_bytes();
        let mut temp = std::fs::File::create(&temp_index_path).expect("create temp index");
        temp.write_all(&entry).expect("write temp index entry");
        temp.flush().expect("flush temp index");
        temp.sync_data().expect("sync temp index");

        let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
        assert_eq!(got.block_hash, block_hash);
        assert!(!temp_index_path.exists(), "temp index should be promoted");
        let index_len = std::fs::metadata(&index_path)
            .expect("index metadata")
            .len();
        assert_eq!(index_len, PIPELINE_INDEX_ENTRY_SIZE_U64);
    }

    #[test]
    fn pipeline_sidecar_skips_temp_index_when_main_index_valid() {
        use iroha_config::base::WithOrigin;
        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let hashes = store_dummy_blocks(&kura, 2);
        let sidecar = PipelineRecoverySidecar::new(
            1,
            hashes[0],
            PipelineDagSnapshot {
                fingerprint: [0u8; 32],
                key_count: 0,
            },
            Vec::new(),
        );
        let payload = sidecar.encode_framed().expect("encode sidecar");
        let temp_sidecar = PipelineRecoverySidecar::new(
            1,
            hashes[1],
            PipelineDagSnapshot {
                fingerprint: [1u8; 32],
                key_count: 1,
            },
            Vec::new(),
        );
        let temp_payload = temp_sidecar.encode_framed().expect("encode temp sidecar");

        let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
        pipeline_dir.push(PIPELINE_DIR_NAME);
        fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
        let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
        let temp_index_path = index_path.with_extension("index.tmp");

        let mut data_file = std::fs::File::create(&data_path).expect("create sidecar data");
        data_file.write_all(&payload).expect("write sidecar data");
        let temp_offset = payload.len() as u64;
        data_file
            .write_all(&temp_payload)
            .expect("write temp sidecar data");
        data_file.flush().expect("flush sidecar data");
        data_file.sync_data().expect("sync sidecar data");

        let entry = SidecarIndexEntry {
            offset: 0,
            len: payload.len() as u64,
        }
        .to_bytes();
        let mut index = std::fs::File::create(&index_path).expect("create sidecar index");
        index.write_all(&entry).expect("write sidecar index");
        index.flush().expect("flush sidecar index");
        index.sync_data().expect("sync sidecar index");

        let temp_entry = SidecarIndexEntry {
            offset: temp_offset,
            len: temp_payload.len() as u64,
        }
        .to_bytes();
        let mut temp_index = std::fs::File::create(&temp_index_path).expect("create temp index");
        temp_index.write_all(&temp_entry).expect("write temp index");
        temp_index.flush().expect("flush temp index");
        temp_index.sync_data().expect("sync temp index");

        let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
        assert_eq!(got.block_hash, hashes[0]);
        assert!(
            temp_index_path.exists(),
            "temp index should not be promoted when main index is valid"
        );

        let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        let mut index_file = std::fs::File::open(&index_path).expect("open sidecar index");
        index_file.read_exact(&mut buf).expect("read sidecar index");
        let entry = SidecarIndexEntry::from_bytes(buf);
        assert_eq!(entry.offset, 0);
        assert_eq!(entry.len, payload.len() as u64);
    }

    #[test]
    fn pipeline_sidecar_ignores_corrupt_temp_index() {
        use iroha_config::base::WithOrigin;
        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let block_hash = store_dummy_blocks(&kura, 1)[0];
        let sidecar = PipelineRecoverySidecar::new(
            1,
            block_hash,
            PipelineDagSnapshot {
                fingerprint: [0u8; 32],
                key_count: 0,
            },
            Vec::new(),
        );
        kura.write_pipeline_metadata(&sidecar);

        let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
        pipeline_dir.push(PIPELINE_DIR_NAME);
        let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
        let temp_index_path = index_path.with_extension("index.tmp");
        std::fs::write(&temp_index_path, [0u8; 3]).expect("write corrupt temp index");

        let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
        assert_eq!(got.block_hash, block_hash);
        assert!(
            temp_index_path.exists(),
            "corrupt temp index should not be promoted"
        );
    }

    #[test]
    fn pipeline_sidecar_ignores_orphaned_temp_data() {
        use iroha_config::base::WithOrigin;
        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let hashes = store_dummy_blocks(&kura, 2);
        let sidecar = PipelineRecoverySidecar::new(
            1,
            hashes[0],
            PipelineDagSnapshot {
                fingerprint: [0u8; 32],
                key_count: 0,
            },
            Vec::new(),
        );
        kura.write_pipeline_metadata(&sidecar);

        let temp_sidecar = PipelineRecoverySidecar::new(
            1,
            hashes[1],
            PipelineDagSnapshot {
                fingerprint: [1u8; 32],
                key_count: 1,
            },
            Vec::new(),
        );
        let payload = temp_sidecar.encode_framed().expect("encode temp sidecar");

        let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
        pipeline_dir.push(PIPELINE_DIR_NAME);
        let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
        let temp_data_path = data_path.with_extension("norito.tmp");
        fs::write(&temp_data_path, &payload).expect("write temp data");

        let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
        assert_eq!(got.block_hash, hashes[0]);
    }

    #[test]
    fn pipeline_sidecar_rejects_height_mismatch() {
        use iroha_config::base::WithOrigin;
        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
        pipeline_dir.push(PIPELINE_DIR_NAME);
        std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");

        let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xBB; 32]));
        let sidecar = PipelineRecoverySidecar::new(
            2,
            block_hash,
            PipelineDagSnapshot {
                fingerprint: [0x12; 32],
                key_count: 1,
            },
            Vec::new(),
        );
        let payload = sidecar.encode_framed().expect("encode sidecar");
        assert!(
            Kura::append_indexed_sidecar(
                &data_path,
                &index_path,
                1,
                &payload,
                "pipeline sidecar",
                FsyncMode::Off,
                None,
            ),
            "append mismatched sidecar"
        );
        assert!(
            kura.read_pipeline_metadata(1).is_none(),
            "height mismatch should be rejected"
        );
    }

    #[test]
    fn sidecar_fsync_mode_tracks_kura_config() {
        use iroha_config::base::WithOrigin;

        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::On,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        assert_eq!(kura.sidecar_fsync_mode(), FsyncMode::On);
    }

    #[test]
    fn pipeline_sidecar_rejects_block_hash_mismatch() {
        use iroha_config::base::WithOrigin;

        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let mut blocks = DummyBlocks::new();
        let block = blocks.next();
        let expected_hash = block.hash();
        kura.store_block(block).expect("store block");

        let mismatch_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xCC; 32]));
        assert_ne!(expected_hash, mismatch_hash, "mismatch hash must differ");

        let sidecar = PipelineRecoverySidecar::new(
            1,
            mismatch_hash,
            PipelineDagSnapshot {
                fingerprint: [0x34; 32],
                key_count: 7,
            },
            Vec::new(),
        );
        kura.write_pipeline_metadata(&sidecar);

        assert!(
            kura.read_pipeline_metadata(1).is_none(),
            "block hash mismatch should be rejected"
        );
    }

    #[test]
    fn pipeline_sidecars_append_to_single_store() {
        use iroha_config::base::WithOrigin;

        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let hashes = store_dummy_blocks(&kura, 2);
        let dag = PipelineDagSnapshot {
            fingerprint: [1u8; 32],
            key_count: 1,
        };
        let sidecar1 = PipelineRecoverySidecar::new(1, hashes[0], dag, Vec::new());
        let sidecar2 = PipelineRecoverySidecar::new(2, hashes[1], dag, Vec::new());

        kura.write_pipeline_metadata(&sidecar1);
        kura.write_pipeline_metadata(&sidecar2);

        let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
        pipeline_dir.push(PIPELINE_DIR_NAME);
        let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);

        assert!(data_path.is_file(), "pipeline sidecar data file missing");
        assert!(index_path.is_file(), "pipeline sidecar index file missing");
        let index_len = std::fs::metadata(&index_path)
            .expect("index metadata")
            .len();
        assert_eq!(
            index_len,
            2 * PIPELINE_INDEX_ENTRY_SIZE_U64,
            "expected two index entries"
        );
        assert!(
            !pipeline_dir.join("block_1.norito").exists(),
            "per-block sidecar should not be created in aggregated layout"
        );

        let got = kura.read_pipeline_metadata(2).expect("sidecar exists");
        assert_eq!(got.height, 2);
        assert_eq!(got.dag.key_count, 1);
    }

    #[test]
    fn pipeline_sidecar_overwrite_updates_entry() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);

        let payload1 =
            norito::to_bytes(&DummySidecar { height: 1 }).expect("encode dummy sidecar 1");
        assert!(
            Kura::append_indexed_sidecar(
                &data_path,
                &index_path,
                1,
                &payload1,
                "dummy sidecar",
                FsyncMode::Off,
                None,
            ),
            "append height 1 must succeed"
        );

        let payload2 =
            norito::to_bytes(&DummySidecar { height: 2 }).expect("encode dummy sidecar 2");
        assert!(
            Kura::append_indexed_sidecar(
                &data_path,
                &index_path,
                1,
                &payload2,
                "dummy sidecar",
                FsyncMode::Off,
                None,
            ),
            "overwrite height 1 must succeed"
        );

        let index_len = fs::metadata(&index_path).expect("index metadata").len();
        assert_eq!(
            index_len, PIPELINE_INDEX_ENTRY_SIZE_U64,
            "expected single index entry"
        );

        let mut index = std::fs::File::open(&index_path).expect("index exists");
        let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        index.read_exact(&mut buf).expect("read index entry");
        let entry = SidecarIndexEntry::from_bytes(buf);
        assert!(entry.len > 0);

        let mut data = std::fs::File::open(&data_path).expect("data exists");
        let len = usize::try_from(entry.len).expect("len fits in usize");
        let mut payload = vec![0u8; len];
        data.seek(SeekFrom::Start(entry.offset))
            .expect("seek to payload");
        data.read_exact(&mut payload).expect("read payload");
        let decoded: DummySidecar =
            norito::decode_from_bytes(&payload).expect("decode dummy sidecar");
        assert_eq!(decoded.height, 2);
    }

    #[test]
    fn pipeline_sidecar_rejects_overlapping_offsets() {
        use iroha_config::base::WithOrigin;

        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
        pipeline_dir.push(PIPELINE_DIR_NAME);
        std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
        let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);

        let sidecar1 = PipelineRecoverySidecar::new(
            1,
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x11; 32])),
            PipelineDagSnapshot {
                fingerprint: [0x10; 32],
                key_count: 0,
            },
            Vec::new(),
        );
        let sidecar2 = PipelineRecoverySidecar::new(
            2,
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x22; 32])),
            PipelineDagSnapshot {
                fingerprint: [0x20; 32],
                key_count: 0,
            },
            Vec::new(),
        );
        let payload1 = sidecar1.encode_framed().expect("encode sidecar1");
        let payload2 = sidecar2.encode_framed().expect("encode sidecar2");
        assert_eq!(payload1.len(), payload2.len(), "payload lengths must match");

        fs::write(&data_path, &payload2).expect("write payload data");
        let entry1 = SidecarIndexEntry {
            offset: 0,
            len: payload1.len() as u64,
        };
        let entry2 = SidecarIndexEntry {
            offset: 0,
            len: payload2.len() as u64,
        };
        let mut index = std::fs::File::create(&index_path).expect("create index");
        index.write_all(&entry1.to_bytes()).expect("write entry1");
        index.write_all(&entry2.to_bytes()).expect("write entry2");

        assert!(
            kura.read_pipeline_metadata(2).is_none(),
            "overlapping offsets should be rejected"
        );
    }

    #[test]
    fn pipeline_sidecar_allows_out_of_order_offsets() {
        use iroha_config::base::WithOrigin;

        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
        pipeline_dir.push(PIPELINE_DIR_NAME);
        std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
        let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);

        let hashes = store_dummy_blocks(&kura, 2);
        let sidecar1 = PipelineRecoverySidecar::new(
            1,
            hashes[0],
            PipelineDagSnapshot {
                fingerprint: [0x30; 32],
                key_count: 0,
            },
            Vec::new(),
        );
        let sidecar2 = PipelineRecoverySidecar::new(
            2,
            hashes[1],
            PipelineDagSnapshot {
                fingerprint: [0x40; 32],
                key_count: 0,
            },
            Vec::new(),
        );
        let payload1 = sidecar1.encode_framed().expect("encode sidecar1");
        let payload2 = sidecar2.encode_framed().expect("encode sidecar2");

        let mut data = std::fs::File::create(&data_path).expect("create data file");
        data.write_all(&payload2).expect("write payload2");
        data.write_all(&payload1).expect("write payload1");

        let entry1 = SidecarIndexEntry {
            offset: payload2.len() as u64,
            len: payload1.len() as u64,
        };
        let entry2 = SidecarIndexEntry {
            offset: 0,
            len: payload2.len() as u64,
        };
        let mut index = std::fs::File::create(&index_path).expect("create index");
        index.write_all(&entry1.to_bytes()).expect("write entry1");
        index.write_all(&entry2.to_bytes()).expect("write entry2");

        let got = kura.read_pipeline_metadata(2).expect("sidecar exists");
        assert_eq!(got.height, 2);
        assert_eq!(got.block_hash, sidecar2.block_hash);
    }

    #[test]
    fn pipeline_sidecar_allows_misaligned_index() {
        use iroha_config::base::WithOrigin;

        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
        pipeline_dir.push(PIPELINE_DIR_NAME);
        std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
        let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);

        let block_hash = store_dummy_blocks(&kura, 1)[0];
        let sidecar = PipelineRecoverySidecar::new(
            1,
            block_hash,
            PipelineDagSnapshot {
                fingerprint: [0x44; 32],
                key_count: 0,
            },
            Vec::new(),
        );
        let payload = sidecar.encode_framed().expect("encode sidecar");
        fs::write(&data_path, &payload).expect("write payload");

        let entry = SidecarIndexEntry {
            offset: 0,
            len: payload.len() as u64,
        };
        let mut index = std::fs::File::create(&index_path).expect("create index");
        index.write_all(&entry.to_bytes()).expect("write entry");
        index.write_all(&[0u8; 3]).expect("write padding");

        let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
        assert_eq!(got.height, 1);
        assert_eq!(got.block_hash, sidecar.block_hash);
    }

    #[test]
    fn sidecar_reader_rejects_oversized_payloads() {
        let temp_dir = TempDir::new().unwrap();
        let store_root = temp_dir.path().join("kura");
        let kura = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: iroha_config::base::WithOrigin::inline(store_root),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap()
        .0;

        let mut dir = kura.store_dir().expect("store dir");
        dir.push(PIPELINE_DIR_NAME);
        std::fs::create_dir_all(&dir).expect("create pipeline dir");
        let data_path = dir.join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = dir.join(PIPELINE_SIDECARS_INDEX_FILE);

        std::fs::write(&data_path, []).expect("create sidecar data file");
        let entry = SidecarIndexEntry {
            offset: 0,
            len: STRICT_INIT_MAX_BLOCK_BYTES + 1,
        }
        .to_bytes();
        std::fs::write(&index_path, entry).expect("write oversized index entry");

        assert!(kura.read_pipeline_metadata(1).is_none());
    }

    #[test]
    fn pipeline_sidecar_ignores_invalid_prev_entry() {
        use iroha_config::base::WithOrigin;

        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
        pipeline_dir.push(PIPELINE_DIR_NAME);
        std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
        let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);

        let hashes = store_dummy_blocks(&kura, 2);
        let sidecar2 = PipelineRecoverySidecar::new(
            2,
            hashes[1],
            PipelineDagSnapshot {
                fingerprint: [0x66; 32],
                key_count: 0,
            },
            Vec::new(),
        );
        let payload2 = sidecar2.encode_framed().expect("encode sidecar2");
        fs::write(&data_path, &payload2).expect("write payload2");

        let bogus_prev = SidecarIndexEntry {
            offset: 0,
            len: payload2.len() as u64 + 10,
        };
        let entry2 = SidecarIndexEntry {
            offset: 0,
            len: payload2.len() as u64,
        };
        let mut index = std::fs::File::create(&index_path).expect("create index");
        index
            .write_all(&bogus_prev.to_bytes())
            .expect("write bogus entry");
        index.write_all(&entry2.to_bytes()).expect("write entry2");

        let got = kura.read_pipeline_metadata(2).expect("sidecar exists");
        assert_eq!(got.height, 2);
        assert_eq!(got.block_hash, sidecar2.block_hash);
    }

    #[test]
    fn sidecar_append_truncates_misaligned_index() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);

        let payload1 =
            norito::to_bytes(&DummySidecar { height: 1 }).expect("encode dummy sidecar 1");
        let payload2 =
            norito::to_bytes(&DummySidecar { height: 2 }).expect("encode dummy sidecar 2");
        fs::write(&data_path, &payload1).expect("write payload1");

        let entry1 = SidecarIndexEntry {
            offset: 0,
            len: payload1.len() as u64,
        };
        let mut index = std::fs::File::create(&index_path).expect("create index");
        index.write_all(&entry1.to_bytes()).expect("write entry1");
        index.write_all(&[0u8; 3]).expect("write padding");

        assert!(
            Kura::append_indexed_sidecar(
                &data_path,
                &index_path,
                2,
                &payload2,
                "dummy sidecar",
                FsyncMode::Off,
                None,
            ),
            "append should succeed and truncate misaligned index"
        );

        let index_len = fs::metadata(&index_path).expect("index metadata").len();
        assert_eq!(
            index_len,
            2 * PIPELINE_INDEX_ENTRY_SIZE_U64,
            "expected aligned index after append"
        );

        let mut index = std::fs::File::open(&index_path).expect("index exists");
        let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        index.read_exact(&mut buf).expect("read entry1");
        let entry1 = SidecarIndexEntry::from_bytes(buf);
        index.read_exact(&mut buf).expect("read entry2");
        let entry2 = SidecarIndexEntry::from_bytes(buf);
        assert_eq!(entry1.offset, 0);
        assert_eq!(entry1.len, payload1.len() as u64);
        assert_eq!(entry2.offset, payload1.len() as u64);
        assert_eq!(entry2.len, payload2.len() as u64);
    }

    #[test]
    fn sidecar_prune_truncates_misaligned_index() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);
        let retention = NonZeroUsize::new(2).expect("non-zero retention");

        let payloads = (1_u64..=3)
            .map(|height| norito::to_bytes(&DummySidecar { height }).expect("encode dummy sidecar"))
            .collect::<Vec<_>>();
        let mut entries = Vec::new();
        let mut data = std::fs::File::create(&data_path).expect("create data");
        let mut offset = 0u64;
        for payload in &payloads {
            data.write_all(payload).expect("write payload");
            entries.push(SidecarIndexEntry {
                offset,
                len: payload.len() as u64,
            });
            offset = offset.saturating_add(payload.len() as u64);
        }

        let mut index = std::fs::File::create(&index_path).expect("create index");
        for entry in &entries {
            index.write_all(&entry.to_bytes()).expect("write entry");
        }
        index.write_all(&[0u8; 3]).expect("write padding");

        assert!(
            Kura::prune_indexed_sidecars(&data_path, &index_path, retention, "dummy sidecar"),
            "prune should tolerate misaligned index"
        );

        let index_len = fs::metadata(&index_path).expect("index metadata").len();
        assert_eq!(
            index_len,
            3 * PIPELINE_INDEX_ENTRY_SIZE_U64,
            "expected aligned index after prune"
        );

        let mut index = std::fs::File::open(&index_path).expect("index exists");
        let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        let mut pruned_entries = Vec::new();
        for _ in 0..3 {
            index.read_exact(&mut buf).expect("read entry");
            pruned_entries.push(SidecarIndexEntry::from_bytes(buf));
        }

        assert_eq!(pruned_entries[0].len, 0);
        assert!(pruned_entries[1].len > 0);
        assert!(pruned_entries[2].len > 0);
        assert_eq!(pruned_entries[1].offset, 0);
        assert_eq!(pruned_entries[2].offset, pruned_entries[1].len);

        let mut data = std::fs::File::open(&data_path).expect("data exists");
        for (idx, expected_height) in [2_u64, 3_u64].into_iter().enumerate() {
            let entry = &pruned_entries[idx + 1];
            let len = usize::try_from(entry.len).expect("len fits in usize");
            let mut payload = vec![0u8; len];
            data.seek(SeekFrom::Start(entry.offset))
                .expect("seek to payload");
            data.read_exact(&mut payload).expect("read payload");
            let decoded: DummySidecar =
                norito::decode_from_bytes(&payload).expect("decode dummy sidecar");
            assert_eq!(decoded.height, expected_height);
        }
    }

    #[test]
    fn sidecar_prune_skips_entries_past_data_len() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);
        let retention = NonZeroUsize::new(1).expect("non-zero retention");

        let payload1 = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode sidecar");
        fs::write(&data_path, &payload1).expect("write payload");

        let entry1 = SidecarIndexEntry {
            offset: 0,
            len: payload1.len() as u64,
        };
        let entry2 = SidecarIndexEntry {
            offset: payload1.len() as u64 + 8,
            len: 4,
        };
        let mut index = std::fs::File::create(&index_path).expect("create index");
        index.write_all(&entry1.to_bytes()).expect("write entry1");
        index.write_all(&entry2.to_bytes()).expect("write entry2");

        assert!(
            Kura::prune_indexed_sidecars(&data_path, &index_path, retention, "dummy sidecar"),
            "prune should drop invalid entries"
        );

        let mut index = std::fs::File::open(&index_path).expect("index exists");
        let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        index.read_exact(&mut buf).expect("read entry1");
        let entry1 = SidecarIndexEntry::from_bytes(buf);
        index.read_exact(&mut buf).expect("read entry2");
        let entry2 = SidecarIndexEntry::from_bytes(buf);

        assert_eq!(entry1.len, 0);
        assert_eq!(entry2.len, 0);
        assert_eq!(
            fs::metadata(&data_path).expect("data metadata").len(),
            0,
            "invalid kept entry should be dropped from data file"
        );
    }

    #[test]
    fn roster_sidecar_roundtrip() {
        use iroha_config::base::WithOrigin;

        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer = PeerId::new(kp.public_key().clone());
        let roster = vec![peer];
        let block_hash = store_dummy_blocks(&kura, 1)[0];
        let signers_bitmap = vec![0b0000_0001];
        let bls_aggregate_signature = vec![0xAB; 96];
        let cert = Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster.clone(),
            aggregate: QcAggregate {
                signers_bitmap: signers_bitmap.clone(),
                bls_aggregate_signature: bls_aggregate_signature.clone(),
            },
        };
        let sidecar = RosterSidecar::new(1, block_hash, Some(cert.clone()), None, None);

        kura.write_roster_metadata(&sidecar);
        let got = kura.read_roster_metadata(1).expect("sidecar exists");

        assert_eq!(got.height, 1);
        assert_eq!(got.block_hash, block_hash);
        assert_eq!(got.format_label(), "roster.snapshot");
        assert_eq!(
            got.commit_qc.as_ref().map(|c| c.validator_set_hash),
            Some(HashOf::new(&roster))
        );
        assert!(got.stake_snapshot.is_none());
        assert_eq!(got.roster_snapshot(), Some(roster));
    }

    #[test]
    fn roster_sidecar_rejects_height_mismatch() {
        use iroha_config::base::WithOrigin;
        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
        pipeline_dir.push(PIPELINE_DIR_NAME);
        std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");

        let data_path = pipeline_dir.join(ROSTER_SIDECARS_DATA_FILE);
        let index_path = pipeline_dir.join(ROSTER_SIDECARS_INDEX_FILE);
        let block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xBC; Hash::LENGTH]));
        let sidecar = RosterSidecar::new(2, block_hash, None, None, None);
        let payload = sidecar.encode_framed().expect("encode sidecar");
        assert!(
            Kura::append_indexed_sidecar(
                &data_path,
                &index_path,
                1,
                &payload,
                "roster sidecar",
                FsyncMode::Off,
                None,
            ),
            "append mismatched roster sidecar"
        );
        assert!(
            kura.read_roster_metadata(1).is_none(),
            "height mismatch should be rejected"
        );
    }

    #[test]
    fn roster_sidecar_rejects_block_hash_mismatch() {
        use iroha_config::base::WithOrigin;

        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let mut blocks = DummyBlocks::new();
        let block = blocks.next();
        let expected_hash = block.hash();
        kura.store_block(block).expect("store block");

        let mismatch_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xDD; 32]));
        assert_ne!(expected_hash, mismatch_hash, "mismatch hash must differ");

        let sidecar = RosterSidecar::new(1, mismatch_hash, None, None, None);
        kura.write_roster_metadata(&sidecar);

        assert!(
            kura.read_roster_metadata(1).is_none(),
            "block hash mismatch should be rejected"
        );
    }

    #[test]
    fn roster_sidecar_rejects_commit_qc_mismatch() {
        use iroha_config::base::WithOrigin;

        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let mut blocks = DummyBlocks::new();
        let block = blocks.next();
        let block_hash = block.hash();
        kura.store_block(block).expect("store block");

        let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer = PeerId::new(kp.public_key().clone());
        let roster = vec![peer];
        let mismatch_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xEE; Hash::LENGTH]));
        assert_ne!(block_hash, mismatch_hash, "mismatch hash must differ");

        let cert = Qc {
            phase: Phase::Commit,
            subject_block_hash: mismatch_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster,
            aggregate: QcAggregate {
                signers_bitmap: vec![0b0000_0001],
                bls_aggregate_signature: vec![0xAA; 96],
            },
        };
        let sidecar = RosterSidecar::new(1, block_hash, Some(cert), None, None);
        kura.write_roster_metadata(&sidecar);

        assert!(
            kura.read_roster_metadata(1).is_none(),
            "mismatched commit certificate should be rejected"
        );
    }

    #[test]
    fn roster_sidecar_roundtrip_with_stake_snapshot() {
        use iroha_config::base::WithOrigin;

        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer = PeerId::new(kp.public_key().clone());
        let roster = vec![peer];
        let block_hash = store_dummy_blocks(&kura, 1)[0];
        let signers_bitmap = vec![0b0000_0001];
        let bls_aggregate_signature = vec![0xAC; 96];
        let cert = Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster.clone(),
            aggregate: QcAggregate {
                signers_bitmap: signers_bitmap.clone(),
                bls_aggregate_signature: bls_aggregate_signature.clone(),
            },
        };
        let stake_snapshot = crate::sumeragi::stake_snapshot::CommitStakeSnapshot {
            validator_set_hash: HashOf::new(&roster),
            entries: vec![crate::sumeragi::stake_snapshot::CommitStakeSnapshotEntry {
                peer_id: roster[0].clone(),
                stake: iroha_primitives::numeric::Numeric::new(10, 0),
            }],
        };
        let sidecar = RosterSidecar::new(
            1,
            block_hash,
            Some(cert.clone()),
            None,
            Some(stake_snapshot.clone()),
        );

        kura.write_roster_metadata(&sidecar);
        let got = kura.read_roster_metadata(1).expect("sidecar exists");

        assert_eq!(got.height, 1);
        assert_eq!(got.block_hash, block_hash);
        assert_eq!(got.format_label(), "roster.snapshot");
        assert_eq!(got.stake_snapshot, Some(stake_snapshot));
        assert_eq!(got.roster_snapshot(), Some(roster));
    }

    #[derive(Debug, Encode, Decode, PartialEq, Eq)]
    struct DummySidecar {
        height: u64,
    }

    #[test]
    fn sidecar_append_rejects_zero_height() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);
        let payload = norito::to_bytes(&DummySidecar { height: 0 }).expect("encode sidecar");

        assert!(
            !Kura::append_indexed_sidecar(
                &data_path,
                &index_path,
                0,
                &payload,
                "dummy sidecar",
                FsyncMode::Off,
                None,
            ),
            "height 0 should be rejected"
        );
        assert!(!data_path.exists(), "data file should not be created");
        assert!(!index_path.exists(), "index file should not be created");
    }

    #[test]
    fn roster_sidecars_prune_to_retention() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join(ROSTER_SIDECARS_DATA_FILE);
        let index_path = temp_dir.path().join(ROSTER_SIDECARS_INDEX_FILE);
        let retention = NonZeroUsize::new(2).expect("non-zero retention");

        for height in 1..=4 {
            let payload = norito::to_bytes(&DummySidecar { height }).expect("encode dummy sidecar");
            assert!(
                Kura::append_indexed_sidecar(
                    &data_path,
                    &index_path,
                    height,
                    &payload,
                    "dummy sidecar",
                    FsyncMode::Off,
                    Some(retention),
                ),
                "append at height {height} must succeed"
            );
        }

        let mut index = std::fs::File::open(&index_path).expect("index exists");
        let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        let mut entries = Vec::new();
        for _ in 0..4 {
            index.read_exact(&mut buf).expect("read index entry");
            entries.push(SidecarIndexEntry::from_bytes(buf));
        }

        assert_eq!(entries[0].len, 0);
        assert_eq!(entries[1].len, 0);
        assert!(entries[2].len > 0);
        assert!(entries[3].len > 0);
        assert_eq!(entries[2].offset, 0);
        assert_eq!(entries[3].offset, entries[2].len);

        let mut data = std::fs::File::open(&data_path).expect("data exists");
        for (idx, expected_height) in [3_u64, 4_u64].into_iter().enumerate() {
            let entry = &entries[idx + 2];
            let len = usize::try_from(entry.len).expect("len fits in usize");
            let mut payload = vec![0u8; len];
            data.seek(SeekFrom::Start(entry.offset))
                .expect("seek to payload");
            data.read_exact(&mut payload).expect("read payload");
            let decoded: DummySidecar =
                norito::decode_from_bytes(&payload).expect("decode dummy sidecar");
            assert_eq!(decoded.height, expected_height);
        }
    }

    #[test]
    fn hashes_count_math() {
        let dir = TempDir::new().unwrap();
        let mut store = BlockStore::new(dir.path());
        store.create_files_if_they_do_not_exist().unwrap();

        // Fresh store: no hashes
        assert_eq!(store.read_hashes_count().unwrap(), 0);

        // Manually extend the hashes file to 3 full entries
        let path = dir.path().join(HASHES_FILE_NAME);
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(3 * SIZE_OF_BLOCK_HASH).unwrap();
        assert_eq!(store.read_hashes_count().unwrap(), 3);

        // Non-multiple of 32 is truncated by integer division
        file.set_len(2 * SIZE_OF_BLOCK_HASH + 16).unwrap();
        assert_eq!(store.read_hashes_count().unwrap(), 2);
    }

    #[test]
    fn read_block_hashes_out_of_bounds() {
        let dir = TempDir::new().unwrap();
        let mut store = BlockStore::new(dir.path());
        store.create_files_if_they_do_not_exist().unwrap();

        // Prepare exactly 2 hash slots worth of data
        let path = dir.path().join(HASHES_FILE_NAME);
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(2 * SIZE_OF_BLOCK_HASH).unwrap();

        // Attempt to read 3 hashes from the start should be out of bounds
        let err = store.read_block_hashes(0, 3).unwrap_err();
        match err {
            Error::OutOfBoundsBlockRead {
                start_block_height,
                block_count,
            } => {
                assert_eq!(start_block_height, 0);
                assert_eq!(block_count, 3);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn read_block_indices_out_of_bounds() {
        let dir = TempDir::new().unwrap();
        let mut store = BlockStore::new(dir.path());
        store.create_files_if_they_do_not_exist().unwrap();

        // Prepare exactly 2 index entries worth of data
        let path = dir.path().join(INDEX_FILE_NAME);
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(2 * BlockIndex::SIZE).unwrap();

        let mut buf = vec![BlockIndex::default(); 3];
        let err = store.read_block_indices(0, &mut buf).unwrap_err();
        match err {
            Error::OutOfBoundsBlockRead {
                start_block_height,
                block_count,
            } => {
                assert_eq!(start_block_height, 0);
                assert_eq!(block_count, 3);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn strict_init_prunes_oversized_block_length() {
        let dir = TempDir::new().unwrap();
        let mut store = BlockStore::new(dir.path());
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        let block = blocks.next();
        store.append_block_to_chain(&block).unwrap();

        let BlockIndex { start, .. } = store.read_block_index(0).unwrap();
        let huge_len = STRICT_INIT_MAX_BLOCK_BYTES + 1;
        store.write_block_index(0, start, huge_len).unwrap();

        let validation = Kura::init_strict_mode(&mut store, 1).unwrap();
        assert!(validation.truncated);
        assert!(validation.hashes.is_empty());
        assert_eq!(store.read_index_count().unwrap(), 0);
    }

    #[test]
    fn strict_init_prunes_corrupted_index_end_to_end() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = new_block_store(&temp_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let block: SignedBlock = ValidBlock::new_dummy(KeyPair::random().private_key()).into();
        store.append_block_to_chain(&block).unwrap();

        let BlockIndex { start, .. } = store.read_block_index(0).unwrap();
        let huge_len = STRICT_INIT_MAX_BLOCK_BYTES + 1;
        store.write_block_index(0, start, huge_len).unwrap();

        let store_dir = temp_dir.path().to_path_buf();
        drop(store);

        let (kura, BlockCount(count)) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: iroha_config::base::WithOrigin::inline(store_dir),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        assert_eq!(count, 0);
        assert_eq!(kura.blocks_count(), 0);
    }

    #[test]
    fn prune_blocks() -> eyre::Result<()> {
        let temp = TempDir::new()?;
        let mut store = BlockStore::new(temp.path());
        store.create_files_if_they_do_not_exist()?;

        // prune on empty store - should be fine
        store.prune(0)?;

        // prune with height greater than there is - should be fine
        store.prune(10)?;

        // add some blocks
        let mut blocks = DummyBlocks::new();
        for _ in 0..10 {
            store.append_block_to_chain(&blocks.next())?;
        }

        assert_eq!(store.read_index_count()?, 10);
        assert_eq!(store.read_block_hashes(0, 10)?.len(), 10);

        store.prune(5)?;

        assert_eq!(store.read_index_count()?, 5);
        assert_eq!(store.read_block_hashes(0, 5)?.len(), 5);
        assert!(store.read_block_hashes(0, 7).is_err());

        for i in 0..5 {
            let block = read_block(&mut store, i)?;
            assert_eq!(block, *blocks.get(i).unwrap());
        }
        assert!(read_block(&mut store, 5).is_err());

        // prune on non-empty state with height greater than there are blocks - should be fine
        store.prune(7)?;

        // can add blocks again
        for i in 5..10 {
            store.append_block_to_chain(&blocks.get(i).unwrap())?;
        }
        for i in 0..10 {
            let block = read_block(&mut store, i)?;
            assert_eq!(block, *blocks.get(i).unwrap());
        }

        Ok(())
    }

    #[test]
    fn kura_prune_to_height_truncates_in_memory_chain() {
        let kura = Kura::blank_kura_for_testing();
        let mut blocks = DummyBlocks::new();
        let b1 = blocks.next();
        let b2 = blocks.next();
        let b3 = blocks.next();
        kura.store_block(b1).expect("store block");
        kura.store_block(b2).expect("store block");
        kura.store_block(b3).expect("store block");

        assert_eq!(kura.blocks_count(), 3);
        kura.prune_to_height(2).expect("prune to height");
        assert_eq!(kura.blocks_count(), 2);

        let b4 = blocks.next();
        kura.store_block(b4).expect("store block after prune");
        assert_eq!(kura.blocks_count(), 3);
    }

    #[test]
    fn fast_init_rewrites_tampered_hash_file() {
        let temp_dir = TempDir::new().unwrap();
        populate_store(&temp_dir, 3);

        let hash_path = primary_blocks_dir(&temp_dir).join(HASHES_FILE_NAME);
        {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .open(&hash_path)
                .unwrap();
            // Overwrite the second hash with garbage to simulate tampering.
            file.seek(SeekFrom::Start(SIZE_OF_BLOCK_HASH)).unwrap();
            file.write_all(&[0xAA; Hash::LENGTH]).unwrap();
            file.flush().unwrap();
        }

        let (kura, BlockCount(count)) = Kura::new(
            &Config {
                init_mode: InitMode::Fast,
                store_dir: iroha_config::base::WithOrigin::inline(temp_dir.path().to_path_buf()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .expect("re-init kura");

        assert_eq!(count, 3);
        let block_hash = kura.get_block_hash(nonzero!(2_usize)).unwrap();
        let block = kura.get_block(nonzero!(2_usize)).unwrap();
        assert_eq!(block_hash, block.hash());
    }

    #[test]
    fn fast_init_prunes_truncated_block_data() {
        let temp_dir = TempDir::new().unwrap();
        populate_store(&temp_dir, 3);

        let data_path = primary_blocks_dir(&temp_dir).join(DATA_FILE_NAME);
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&data_path)
            .unwrap();
        let len = file.metadata().unwrap().len();
        file.set_len(len.saturating_sub(4)).unwrap();

        let (kura, BlockCount(count)) = Kura::new(
            &Config {
                init_mode: InitMode::Fast,
                store_dir: iroha_config::base::WithOrigin::inline(temp_dir.path().to_path_buf()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &RuntimeLaneConfig::default(),
        )
        .expect("re-init kura");

        assert_eq!(count, 2);
        assert!(kura.get_block(nonzero!(3_usize)).is_none());

        let mut store = new_block_store(&temp_dir);
        assert_eq!(store.read_index_count().unwrap(), 2);
        assert_eq!(store.read_hashes_count().unwrap(), 2);
    }

    #[test]
    fn commit_marker_prunes_excess_entries_on_init() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        for _ in 0..3 {
            store.append_block_to_chain(&blocks.next()).unwrap();
        }

        store.write_commit_marker(1).unwrap();
        drop(store);

        let mut reopened = BlockStore::new(&blocks_dir);
        reopened.create_files_if_they_do_not_exist().unwrap();

        assert_eq!(reopened.read_index_count().unwrap(), 1);
        assert_eq!(reopened.read_hashes_count().unwrap(), 1);
        assert_eq!(reopened.read_durable_index_count().unwrap(), 1);
        let marker = reopened.read_commit_marker().unwrap().expect("marker");
        assert_eq!(marker.count, 1);

        let last = reopened.read_block_index(0).unwrap();
        let data_len = reopened.data_file_len().unwrap();
        assert_eq!(data_len, last.start + last.length);
    }

    #[test]
    fn commit_marker_truncates_hashes_tail_on_init() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        for _ in 0..2 {
            store.append_block_to_chain(&blocks.next()).unwrap();
        }

        let hashes_path = blocks_dir.join(HASHES_FILE_NAME);
        let hashes_file = std::fs::OpenOptions::new()
            .write(true)
            .open(&hashes_path)
            .unwrap();
        hashes_file.set_len(3 * SIZE_OF_BLOCK_HASH).unwrap();
        drop(store);

        let mut reopened = BlockStore::new(&blocks_dir);
        reopened.create_files_if_they_do_not_exist().unwrap();
        assert_eq!(reopened.read_index_count().unwrap(), 2);
        assert_eq!(reopened.read_hashes_count().unwrap(), 2);
    }

    #[test]
    fn commit_marker_overwrites_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        store.write_commit_marker(1).unwrap();
        store.write_commit_marker(2).unwrap();

        let marker = store.read_commit_marker().unwrap().expect("marker");
        assert_eq!(marker.count, 2);
        assert!(blocks_dir.join(COUNT_FILE_NAME).exists());
    }

    #[test]
    fn commit_marker_corruption_falls_back_to_index_count() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        for _ in 0..2 {
            store.append_block_to_chain(&blocks.next()).unwrap();
        }

        let marker_path = blocks_dir.join(COUNT_FILE_NAME);
        std::fs::write(&marker_path, b"corrupt").unwrap();
        drop(store);

        let mut reopened = BlockStore::new(&blocks_dir);
        reopened.create_files_if_they_do_not_exist().unwrap();
        assert_eq!(reopened.read_durable_index_count().unwrap(), 2);
        let marker = reopened.read_commit_marker().unwrap().expect("marker");
        assert_eq!(marker.count, 2);
    }

    #[test]
    fn commit_marker_corruption_falls_back_to_data_backed_count() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        for _ in 0..2 {
            store.append_block_to_chain(&blocks.next()).unwrap();
        }

        let first = store.read_block_index(0).unwrap();
        let first_end = first.start + first.length;
        drop(store);

        let data_path = blocks_dir.join(DATA_FILE_NAME);
        let data_file = std::fs::OpenOptions::new()
            .write(true)
            .open(&data_path)
            .unwrap();
        data_file.set_len(first_end).unwrap();

        let marker_path = blocks_dir.join(COUNT_FILE_NAME);
        std::fs::write(&marker_path, b"corrupt").unwrap();

        let mut reopened = BlockStore::new(&blocks_dir);
        reopened.create_files_if_they_do_not_exist().unwrap();
        assert_eq!(reopened.read_durable_index_count().unwrap(), 1);
        assert_eq!(reopened.read_index_count().unwrap(), 1);
        assert_eq!(reopened.read_hashes_count().unwrap(), 1);

        let last = reopened.read_block_index(0).unwrap();
        let data_len = reopened.data_file_len().unwrap();
        assert_eq!(data_len, last.start + last.length);
    }

    #[test]
    fn index_misalignment_truncates_on_init() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        for _ in 0..2 {
            store.append_block_to_chain(&blocks.next()).unwrap();
        }

        let index_path = blocks_dir.join(INDEX_FILE_NAME);
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&index_path)
            .unwrap();
        file.write_all(&[0u8; 3]).unwrap();
        drop(store);

        let mut reopened = BlockStore::new(&blocks_dir);
        reopened.create_files_if_they_do_not_exist().unwrap();
        let len = reopened.index_file_len().unwrap();
        assert_eq!(len % BlockIndex::SIZE, 0);
        assert_eq!(reopened.read_index_count().unwrap(), 2);
    }

    #[test]
    fn hashes_misalignment_truncates_on_init() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        for _ in 0..2 {
            store.append_block_to_chain(&blocks.next()).unwrap();
        }

        let hashes_path = blocks_dir.join(HASHES_FILE_NAME);
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&hashes_path)
            .unwrap();
        file.write_all(&[0u8; 3]).unwrap();
        drop(store);

        let mut reopened = BlockStore::new(&blocks_dir);
        reopened.create_files_if_they_do_not_exist().unwrap();
        let len = reopened.hashes_file_len().unwrap();
        assert_eq!(len % SIZE_OF_BLOCK_HASH, 0);
        assert_eq!(reopened.read_hashes_count().unwrap(), 2);
    }

    #[test]
    fn prune_does_not_advance_commit_marker() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let block = DummyBlocks::new().next();
        store.append_block_to_chain(block.as_ref()).unwrap();

        let marker = store.read_commit_marker().unwrap().expect("marker");
        assert_eq!(marker.count, 1);

        store.prune(5).unwrap();

        let marker_after = store.read_commit_marker().unwrap().expect("marker");
        assert_eq!(marker_after.count, 1);
        assert_eq!(store.read_index_count().unwrap(), 1);
    }

    #[test]
    fn batched_fsync_waits_until_interval_elapses() {
        let temp_dir = TempDir::new().expect("temp dir");
        let mut store = BlockStore::with_fsync(
            temp_dir.path(),
            FsyncMode::Batched,
            Duration::from_millis(5),
        );
        store.create_files_if_they_do_not_exist().unwrap();

        let block = DummyBlocks::new().next();
        store
            .append_block_to_chain(block.as_ref())
            .expect("append block");

        assert!(
            store.fsync_pending_for_tests(),
            "batched fsync should leave pending work"
        );
        let wait = store.next_fsync_wait().expect("pending fsync deadline");
        assert!(
            wait <= Duration::from_millis(5),
            "expected wait under batching window"
        );

        thread::sleep(Duration::from_millis(6));
        store
            .flush_pending_fsync(false)
            .expect("flush pending fsync succeeds");
        assert!(
            !store.fsync_pending_for_tests(),
            "batched fsync should clear after flush"
        );
    }

    #[test]
    fn fsync_on_flushes_immediately() {
        let temp_dir = TempDir::new().expect("temp dir");
        let mut store = BlockStore::with_fsync(temp_dir.path(), FsyncMode::On, FSYNC_INTERVAL);
        store.create_files_if_they_do_not_exist().unwrap();

        let block = DummyBlocks::new().next();
        store
            .append_block_to_chain(block.as_ref())
            .expect("append block");
        assert!(
            !store.fsync_pending_for_tests(),
            "immediate fsync should clear pending flag"
        );
        assert!(
            store.next_fsync_wait().is_none(),
            "immediate fsync should not schedule a wait"
        );
    }

    #[test]
    fn commit_marker_write_failure_keeps_pending() {
        let temp_dir = TempDir::new().expect("temp dir");
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store =
            BlockStore::with_fsync(&blocks_dir, FsyncMode::Batched, Duration::from_millis(10));
        store.create_files_if_they_do_not_exist().unwrap();

        let block = DummyBlocks::new().next();
        store
            .append_block_to_chain(block.as_ref())
            .expect("append block");

        assert!(
            store.commit_marker_pending.is_some(),
            "expected pending commit marker before flush"
        );
        assert!(
            store.fsync_pending_for_tests(),
            "fsync should be pending before flush"
        );

        let tmp_marker = blocks_dir
            .join(COUNT_FILE_NAME)
            .with_extension("norito.tmp");
        std::fs::create_dir_all(&tmp_marker).expect("create tmp marker directory");

        store
            .flush_pending_fsync(true)
            .expect_err("flush should fail when commit marker temp is a directory");

        assert!(
            store.commit_marker_pending.is_some(),
            "commit marker should remain pending after failure"
        );
        assert!(
            store.fsync_pending_for_tests(),
            "fsync should remain pending after commit marker failure"
        );
    }
}
