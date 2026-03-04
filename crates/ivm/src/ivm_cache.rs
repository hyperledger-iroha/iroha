//! IvmCache: a minimal pre-decode cache for instruction streams.
//!
//! Decodes a raw instruction byte stream (mixed 16/32) into a compact
//! representation using the canonical decoder and caches the result keyed by
//! `(sha256(code_bytes), version_major, version_minor)`.
//!
//! This is a development scaffold to exercise the pipeline path. It is not
//! intended to be a fully optimized LRU; it keeps a simple VecDeque order and
//! a HashMap for lookups. On capacity overflow, it evicts the least-recently
//! used item. Accessing an existing entry marks it as most-recently used.

use std::{
    collections::{HashMap, VecDeque},
    hash::{Hash, Hasher},
    sync::{
        Arc, Mutex, OnceLock, RwLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Instant,
};

use sha2::{Digest, Sha256};

use crate::{decoder, memory::Memory, metadata::ProgramMetadata};

/// A decoded instruction with its byte offset and length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedOp {
    /// Byte offset from the beginning of the code buffer.
    pub pc: u64,
    /// Canonical 32-bit instruction word returned by the decoder.
    pub inst: u32,
    /// Instruction length in bytes (2 or 4).
    pub len: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CacheKey {
    hash: [u8; 32],
    vmaj: u8,
    vmin: u8,
}

impl Hash for CacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.hash);
        state.write_u8(self.vmaj);
        state.write_u8(self.vmin);
    }
}

/// Snapshot of global cache metrics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub decoded_streams: u64,
    pub decoded_ops_total: u64,
    pub decode_failures: u64,
    pub decode_time_ns_total: u64,
}

/// Limits applied to the global IVM pre-decode cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheLimits {
    /// Total number of cached entries (0 disables caching).
    pub capacity: usize,
    /// Approximate byte budget for cached entries (0 = unlimited).
    pub max_bytes: usize,
    /// Maximum decoded ops per cached entry (0 = unlimited).
    pub max_decoded_ops: usize,
}

const DEFAULT_CACHE_MAX_DECODED_OPS: usize = 8_000_000;
const DEFAULT_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;

/// Minimal LRU cache for pre-decoded instruction streams.
pub struct IvmCache {
    cap: usize,
    /// Approximate memory budget (bytes) for all cached entries.
    max_bytes: usize,
    /// Current accounted bytes across all cached entries.
    cur_bytes: usize,
    /// Cached decoded streams by key.
    map: HashMap<CacheKey, Arc<[DecodedOp]>>,
    /// Per-key sizes in bytes for quick accounting on eviction.
    sizes: HashMap<CacheKey, usize>,
    /// LRU order (front = oldest).
    order: VecDeque<CacheKey>,
    hits: u64,
    misses: u64,
    evictions: u64,
}

impl IvmCache {
    /// Create a new cache with the given capacity (number of entries).
    pub fn new(capacity: usize) -> Self {
        Self {
            cap: capacity,
            max_bytes: configured_max_bytes(),
            cur_bytes: 0,
            map: HashMap::new(),
            sizes: HashMap::new(),
            order: VecDeque::new(),
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    /// Compute the cache key for a code buffer and header version.
    fn key_for(code: &[u8], vmaj: u8, vmin: u8) -> CacheKey {
        let mut hasher = Sha256::new();
        hasher.update(code);
        let hash = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&hash);
        CacheKey {
            hash: out,
            vmaj,
            vmin,
        }
    }

    fn touch(&mut self, key: &CacheKey) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
        self.order.push_back(*key);
        self.enforce_limits();
    }

    fn entry_size(decoded: &Arc<[DecodedOp]>) -> usize {
        core::mem::size_of::<DecodedOp>() * decoded.len()
    }

    fn enforce_limits(&mut self) {
        // Evict by count first
        while self.order.len() > self.cap {
            if let Some(old) = self.order.pop_front() {
                if let Some(decoded) = self.map.remove(&old) {
                    let sz = self
                        .sizes
                        .remove(&old)
                        .unwrap_or_else(|| Self::entry_size(&decoded));
                    self.cur_bytes = self.cur_bytes.saturating_sub(sz);
                }
                self.evictions += 1;
            } else {
                break;
            }
        }
        // Then enforce byte budget
        while self.cur_bytes > self.max_bytes {
            if let Some(old) = self.order.pop_front() {
                if let Some(decoded) = self.map.remove(&old) {
                    let sz = self
                        .sizes
                        .remove(&old)
                        .unwrap_or_else(|| Self::entry_size(&decoded));
                    self.cur_bytes = self.cur_bytes.saturating_sub(sz);
                }
                self.evictions += 1;
            } else {
                break;
            }
        }
    }

    /// Decode the code buffer using the canonical decoder. Returns a shared
    /// slice of decoded ops.
    pub fn decode_stream(code: &[u8]) -> Result<Arc<[DecodedOp]>, crate::VMError> {
        if (code.len() as u64) > crate::memory::Memory::HEAP_START {
            return Err(crate::VMError::MemoryOutOfBounds);
        }
        let start = Instant::now();
        let mut mem = Memory::new(code.len() as u64);
        mem.load_code(code);
        let result: Result<Vec<DecodedOp>, crate::VMError> = (|| {
            let mut pc = 0u64;
            let mut out = Vec::new();
            // Hard cap on the number of decoded instructions per entry. This prevents
            // pathological byte streams from consuming excessive memory during
            // pre-decode. Hosts configure the limit via `iroha_config`; tests override
            // through `CacheLimitsGuard`.
            let max_ops = configured_max_decoded_ops();
            while (pc as usize) < code.len() {
                let (inst, len) = decoder::decode(&mem, pc)?;
                out.push(DecodedOp { pc, inst, len });
                pc += len as u64;
                if out.len() >= max_ops {
                    return Err(crate::VMError::DecodeError);
                }
            }
            Ok(out)
        })();
        let elapsed_ns = start.elapsed().as_nanos();
        let elapsed_ns_u64 = if elapsed_ns > u64::MAX as u128 {
            u64::MAX
        } else {
            elapsed_ns as u64
        };
        match result {
            Ok(out) => {
                let decoded: Arc<[DecodedOp]> = Arc::from(out.into_boxed_slice());
                atomic_saturating_add(&DECODED_STREAMS, 1);
                atomic_saturating_add(&DECODED_OPS, decoded.len() as u64);
                atomic_saturating_add(&DECODE_TIME_NS, elapsed_ns_u64);
                Ok(decoded)
            }
            Err(err) => {
                atomic_saturating_add(&DECODE_FAILURES, 1);
                atomic_saturating_add(&DECODE_TIME_NS, elapsed_ns_u64);
                Err(err)
            }
        }
    }

    /// Get a pre-decoded stream from cache or decode and insert.
    pub fn get_or_predecode(
        &mut self,
        code: &[u8],
        vmaj: u8,
        vmin: u8,
    ) -> Result<Arc<[DecodedOp]>, crate::VMError> {
        let key = Self::key_for(code, vmaj, vmin);
        self.get_or_predecode_with_key(key, code)
    }

    /// Decode a full artifact that begins with an IVM header followed by code bytes.
    pub fn decode_artifact(
        artifact: &[u8],
    ) -> Result<(ProgramMetadata, Arc<[DecodedOp]>), crate::VMError> {
        let parsed = ProgramMetadata::parse(artifact)?;
        let header_len = parsed.header_len;
        if artifact.len() < header_len {
            return Err(crate::VMError::InvalidMetadata);
        }
        let code = &artifact[parsed.code_offset..];
        let decoded = Self::decode_stream(code)?;
        Ok((parsed.metadata, decoded))
    }

    /// Get a pre-decoded stream from a full artifact (header + code), using the header
    /// version in the cache key.
    pub fn get_or_predecode_artifact(
        &mut self,
        artifact: &[u8],
    ) -> Result<(ProgramMetadata, Arc<[DecodedOp]>), crate::VMError> {
        let parsed = ProgramMetadata::parse(artifact)?;
        let code = &artifact[parsed.code_offset..];
        let key = Self::key_for(
            code,
            parsed.metadata.version_major,
            parsed.metadata.version_minor,
        );
        let decoded = self.get_or_predecode_with_key(key, code)?;
        Ok((parsed.metadata, decoded))
    }

    /// Get or predecode using explicit metadata (avoids reconstructing artifact bytes).
    pub fn get_or_predecode_with_meta(
        &mut self,
        code: &[u8],
        meta: &ProgramMetadata,
    ) -> Result<Arc<[DecodedOp]>, crate::VMError> {
        let key = Self::key_for(code, meta.version_major, meta.version_minor);
        self.get_or_predecode_with_key(key, code)
    }

    fn get_or_predecode_with_key(
        &mut self,
        key: CacheKey,
        code: &[u8],
    ) -> Result<Arc<[DecodedOp]>, crate::VMError> {
        if self.cap == 0 {
            self.misses += 1;
            return Self::decode_stream(code);
        }
        if let Some(hit) = { self.map.get(&key).cloned() } {
            self.touch(&key);
            self.hits += 1;
            return Ok(hit);
        }
        self.misses += 1;
        let decoded = Self::decode_stream(code)?;
        let sz = Self::entry_size(&decoded);
        self.cur_bytes = self.cur_bytes.saturating_add(sz);
        self.map.insert(key, decoded.clone());
        self.sizes.insert(key, sz);
        self.touch(&key);
        Ok(decoded)
    }

    /// Return current counters (hits, misses, evictions) for diagnostics.
    pub fn counters(&self) -> (u64, u64, u64) {
        (self.hits, self.misses, self.evictions)
    }
}

// Global thread-safe cache and counters (Phase 2)
pub struct ShardedCache {
    shards: RwLock<Vec<Arc<Mutex<IvmCache>>>>,
    total_capacity: AtomicUsize,
}

const SHARD_ACTIVATION_THRESHOLD: usize = 64;

impl ShardedCache {
    fn new(total_capacity: usize) -> Self {
        let shard_vec = Self::build_shards(total_capacity, default_shard_count());
        Self {
            shards: RwLock::new(shard_vec),
            total_capacity: AtomicUsize::new(total_capacity),
        }
    }

    fn build_shards(total_capacity: usize, suggested_shards: usize) -> Vec<Arc<Mutex<IvmCache>>> {
        let desired_shards = if total_capacity <= SHARD_ACTIVATION_THRESHOLD {
            1
        } else {
            suggested_shards
        };
        let shard_count = match total_capacity {
            0 => 1,
            _ => desired_shards.clamp(1, total_capacity),
        };
        let shard_count = shard_count.max(1);
        let base = if shard_count == 0 {
            0
        } else {
            total_capacity / shard_count
        };
        let remainder = if shard_count == 0 {
            0
        } else {
            total_capacity % shard_count
        };
        let mut shards = Vec::with_capacity(shard_count);
        for idx in 0..shard_count {
            let mut cap = base;
            if idx < remainder {
                cap = cap.saturating_add(1);
            }
            shards.push(Arc::new(Mutex::new(IvmCache::new(cap))));
        }
        shards
    }

    fn shard_for_key(&self, key: &CacheKey) -> Arc<Mutex<IvmCache>> {
        let shards = self.shards.read().unwrap();
        let count = shards.len();
        let idx = if count <= 1 {
            0
        } else {
            let mut prefix = [0u8; 8];
            prefix.copy_from_slice(&key.hash[..8]);
            (u64::from_le_bytes(prefix) as usize) % count
        };
        Arc::clone(&shards[idx])
    }

    fn set_capacity(&self, total_capacity: usize) {
        self.total_capacity.store(total_capacity, Ordering::Relaxed);
        let shard_vec = Self::build_shards(total_capacity, default_shard_count());
        let old_shards = {
            let mut guard = self.shards.write().unwrap();
            std::mem::replace(&mut *guard, shard_vec)
        };

        let evicted = old_shards
            .into_iter()
            .map(|shard| {
                let cache = shard.lock().unwrap();
                cache.map.len() as u64
            })
            .sum();
        atomic_saturating_add(&EVICTS, evicted);
    }
}

static CACHE_MAX_BYTES: AtomicUsize = AtomicUsize::new(DEFAULT_CACHE_MAX_BYTES);
static CACHE_MAX_DECODED_OPS: AtomicUsize = AtomicUsize::new(DEFAULT_CACHE_MAX_DECODED_OPS);
static GLOBAL_CACHE: OnceLock<ShardedCache> = OnceLock::new();
static HITS: AtomicU64 = AtomicU64::new(0);
static MISSES: AtomicU64 = AtomicU64::new(0);
static EVICTS: AtomicU64 = AtomicU64::new(0);
static DECODED_STREAMS: AtomicU64 = AtomicU64::new(0);
static DECODED_OPS: AtomicU64 = AtomicU64::new(0);
static DECODE_FAILURES: AtomicU64 = AtomicU64::new(0);
static DECODE_TIME_NS: AtomicU64 = AtomicU64::new(0);

fn global_capacity() -> usize {
    // Default capacity used when hosts do not configure the cache explicitly.
    128
}

fn configured_max_decoded_ops() -> usize {
    let raw = CACHE_MAX_DECODED_OPS.load(Ordering::Relaxed);
    if raw == 0 { usize::MAX } else { raw }
}

fn configured_max_bytes() -> usize {
    let raw = CACHE_MAX_BYTES.load(Ordering::Relaxed);
    if raw == 0 { usize::MAX } else { raw }
}

fn default_shard_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get().clamp(1, 16))
        .unwrap_or(4)
}

fn atomic_saturating_add(cell: &AtomicU64, value: u64) {
    if value == 0 {
        return;
    }
    let mut current = cell.load(Ordering::Relaxed);
    loop {
        let next = current.saturating_add(value);
        match cell.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(actual) => current = actual,
        }
    }
}

pub fn global_cache() -> &'static ShardedCache {
    GLOBAL_CACHE.get_or_init(|| ShardedCache::new(global_capacity()))
}

pub fn global_get_with_meta(
    code: &[u8],
    meta: &ProgramMetadata,
) -> Result<Arc<[DecodedOp]>, crate::VMError> {
    let cache = global_cache();
    let key = IvmCache::key_for(code, meta.version_major, meta.version_minor);
    let shard = cache.shard_for_key(&key);
    let mut guard = shard.lock().unwrap();
    let before = guard.counters();
    let out = guard.get_or_predecode_with_key(key, code);
    let after = guard.counters();
    HITS.fetch_add(after.0 - before.0, Ordering::Relaxed);
    MISSES.fetch_add(after.1 - before.1, Ordering::Relaxed);
    EVICTS.fetch_add(after.2 - before.2, Ordering::Relaxed);
    out
}

pub fn global_stats() -> CacheStats {
    CacheStats {
        hits: HITS.load(Ordering::Relaxed),
        misses: MISSES.load(Ordering::Relaxed),
        evictions: EVICTS.load(Ordering::Relaxed),
        decoded_streams: DECODED_STREAMS.load(Ordering::Relaxed),
        decoded_ops_total: DECODED_OPS.load(Ordering::Relaxed),
        decode_failures: DECODE_FAILURES.load(Ordering::Relaxed),
        decode_time_ns_total: DECODE_TIME_NS.load(Ordering::Relaxed),
    }
}

pub fn global_counters() -> (u64, u64, u64) {
    let stats = global_stats();
    (stats.hits, stats.misses, stats.evictions)
}

/// Initialize the global cache with a specific capacity if it has not been created yet.
/// If the cache already exists, this function is a no‑op.
pub fn init_global_with_capacity(capacity: usize) {
    let _ = GLOBAL_CACHE.set(ShardedCache::new(capacity));
}

/// Update the capacity of the global cache at runtime. If the cache is not yet
/// initialized, it will be created with the requested capacity.
pub fn set_global_capacity(capacity: usize) {
    let cache = global_cache();
    cache.set_capacity(capacity);
}

fn normalize_limits(limits: CacheLimits) -> CacheLimits {
    CacheLimits {
        capacity: limits.capacity,
        max_bytes: if limits.max_bytes == 0 {
            usize::MAX
        } else {
            limits.max_bytes
        },
        max_decoded_ops: if limits.max_decoded_ops == 0 {
            usize::MAX
        } else {
            limits.max_decoded_ops
        },
    }
}

fn denormalize(value: usize) -> usize {
    if value == usize::MAX { 0 } else { value }
}

fn apply_max_bytes_to_shards(max_bytes: usize) {
    if let Some(cache) = GLOBAL_CACHE.get() {
        let shards = cache.shards.read().unwrap();
        for shard in shards.iter() {
            let mut guard = shard.lock().unwrap();
            guard.max_bytes = max_bytes;
            guard.enforce_limits();
        }
    }
}

/// Configure cache limits (capacity, byte budget, decoded-op guard) from host config.
pub fn configure_limits(limits: CacheLimits) {
    let normalized = normalize_limits(limits);
    CACHE_MAX_BYTES.store(normalized.max_bytes, Ordering::Relaxed);
    CACHE_MAX_DECODED_OPS.store(normalized.max_decoded_ops, Ordering::Relaxed);

    if let Some(cache) = GLOBAL_CACHE.get() {
        apply_max_bytes_to_shards(normalized.max_bytes);
        let current = cache.total_capacity.load(Ordering::Relaxed);
        if current != normalized.capacity {
            cache.set_capacity(normalized.capacity);
        }
    } else {
        let _ = GLOBAL_CACHE.set(ShardedCache::new(normalized.capacity));
    }
}

/// Snapshot of the current cache limits (0 denotes unlimited for max fields).
pub fn cache_limits() -> CacheLimits {
    let capacity = GLOBAL_CACHE
        .get()
        .map(|cache| cache.total_capacity.load(Ordering::Relaxed))
        .unwrap_or_else(global_capacity);
    CacheLimits {
        capacity,
        max_bytes: denormalize(configured_max_bytes()),
        max_decoded_ops: denormalize(configured_max_decoded_ops()),
    }
}

/// RAII guard that restores previous cache limits when dropped.
pub struct CacheLimitsGuard {
    previous: CacheLimits,
}

impl CacheLimitsGuard {
    /// Apply new cache limits for the lifetime of the guard.
    pub fn new(limits: CacheLimits) -> Self {
        let previous = cache_limits();
        configure_limits(limits);
        Self { previous }
    }
}

impl Drop for CacheLimitsGuard {
    fn drop(&mut self) {
        configure_limits(self.previous);
    }
}
