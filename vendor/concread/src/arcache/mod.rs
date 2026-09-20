//! ARCache - A concurrently readable adaptive replacement cache.
//!
//! An ARCache is used in place of a `RwLock<LruCache>` or `Mutex<LruCache>`.
//! This structure is transactional, meaning that readers have guaranteed
//! point-in-time views of the cache and their items, while allowing writers
//! to proceed with inclusions and cache state management in parallel.
//!
//! This means that unlike a `RwLock` which can have many readers OR one writer
//! this cache is capable of many readers, over multiple data generations AND
//! writers that are serialised. This formally means that this is an ACID
//! compliant Cache.

mod ll;
/// Stats collection for [ARCache]
pub mod stats;

use self::ll::{LLNodeRef, LLWeight, LL};
use self::stats::{ARCacheReadStat, ARCacheWriteStat};

#[cfg(feature = "arcache-is-hashmap")]
use crate::hashmap::{
    HashMap as DataMap, HashMapReadTxn as DataMapReadTxn, HashMapWriteTxn as DataMapWriteTxn,
};

#[cfg(all(feature = "arcache-is-hashtrie", not(feature = "arcache-is-hashmap")))]
use crate::hashtrie::{
    HashTrie as DataMap, HashTrieReadTxn as DataMapReadTxn, HashTrieWriteTxn as DataMapWriteTxn,
};

use crossbeam_queue::ArrayQueue;
use std::collections::HashMap as Map;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::{Mutex, RwLock};

use std::borrow::Borrow;
use std::cell::UnsafeCell;
use std::fmt::Debug;
use std::hash::Hash;
use std::mem;
use std::num::NonZeroUsize;
use std::ops::Deref;
use std::ops::DerefMut;
use std::time::Instant;

use tracing::trace;

const READ_THREAD_CACHE_RATIO: isize = 8;
const WRITE_THREAD_CACHE_RATIO: isize = 4;

const WRITE_THREAD_CHANNEL_SIZE: usize = 64;
const READ_THREAD_CHANNEL_SIZE: usize = 64;

const TXN_LOOKBACK_LIMIT_DEFAULT: u8 = 32;
const TXN_LOOKBACK_LIMIT_ABS_MIN: u8 = 4;

const WATERMARK_DISABLE_MIN: usize = 128;

const WATERMARK_DISABLE_DIVISOR: usize = 20;
const WATERMARK_DISABLE_RATIO: usize = 18;

const HAUNTED_SIZE: usize = 1;

enum ThreadCacheItem<V> {
    Present(V, bool, usize),
    Removed(bool),
}

struct CacheHitEvent {
    t: Instant,
    k_hash: u64,
}

struct CacheIncludeEvent<K, V> {
    t: Instant,
    k: K,
    v: V,
    txid: u64,
    size: usize,
}

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Clone, Debug)]
struct CacheItemInner<K>
where
    K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
{
    k: K,
    txid: u64,
    size: usize,
}

impl<K> LLWeight for CacheItemInner<K>
where
    K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
{
    #[inline]
    fn ll_weight(&self) -> usize {
        self.size
    }
}

#[derive(Clone, Debug)]
enum CacheItem<K, V>
where
    K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
{
    Freq(LLNodeRef<CacheItemInner<K>>, V),
    Rec(LLNodeRef<CacheItemInner<K>>, V),
    GhostFreq(LLNodeRef<CacheItemInner<K>>),
    GhostRec(LLNodeRef<CacheItemInner<K>>),
    Haunted(LLNodeRef<CacheItemInner<K>>),
}

unsafe impl<
        K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
        V: Clone + Debug + Sync + Send + 'static,
    > Send for CacheItem<K, V>
{
}
unsafe impl<
        K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
        V: Clone + Debug + Sync + Send + 'static,
    > Sync for CacheItem<K, V>
{
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CacheState {
    Freq,
    Rec,
    GhostFreq,
    GhostRec,
    Haunted,
    None,
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
pub(crate) struct CStat {
    max: usize,
    cache: usize,
    tlocal: usize,
    freq: usize,
    rec: usize,
    ghost_freq: usize,
    ghost_rec: usize,
    haunted: usize,
    p: usize,
}

struct ArcInner<K, V>
where
    K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
    V: Clone + Debug + Sync + Send + 'static,
{
    /// Weight of items between the two caches.
    p: usize,
    freq: LL<CacheItemInner<K>>,
    rec: LL<CacheItemInner<K>>,
    ghost_freq: LL<CacheItemInner<K>>,
    ghost_rec: LL<CacheItemInner<K>>,
    haunted: LL<CacheItemInner<K>>,
    hit_queue: Arc<ArrayQueue<CacheHitEvent>>,
    inc_queue: Arc<ArrayQueue<CacheIncludeEvent<K, V>>>,
    min_txid: u64,
}

struct ArcShared<K, V>
where
    K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
    V: Clone + Debug + Sync + Send + 'static,
{
    // Max number of elements to cache.
    max: usize,
    // Max number of elements for a reader per thread.
    read_max: usize,
    hit_queue: Arc<ArrayQueue<CacheHitEvent>>,
    inc_queue: Arc<ArrayQueue<CacheIncludeEvent<K, V>>>,
    /// The number of items that are present in the cache before we start to process
    /// the arc sets/lists.
    watermark: usize,
    /// If readers should attempt to quiesce the cache. Default true
    reader_quiesce: bool,
}

/// A concurrently readable adaptive replacement cache. Operations are performed on the
/// cache via read and write operations.
pub struct ARCache<K, V>
where
    K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
    V: Clone + Debug + Sync + Send + 'static,
{
    // Use a unified tree, allows simpler movement of items between the
    // cache types.
    cache: DataMap<K, CacheItem<K, V>>,
    // This is normally only ever taken in "read" mode, so it's effectively
    // an uncontended barrier.
    shared: RwLock<ArcShared<K, V>>,
    // These are only taken during a quiesce
    inner: Mutex<ArcInner<K, V>>,
    // stats: CowCell<CacheStats>,
    above_watermark: AtomicBool,
    look_back_limit: u64,
}

unsafe impl<
        K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
        V: Clone + Debug + Sync + Send + 'static,
    > Send for ARCache<K, V>
{
}
unsafe impl<
        K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
        V: Clone + Debug + Sync + Send + 'static,
    > Sync for ARCache<K, V>
{
}

#[derive(Debug, Clone)]
struct ReadCacheItem<K, V>
where
    K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
    V: Clone + Debug + Sync + Send + 'static,
{
    k: K,
    v: V,
    size: usize,
}

impl<K, V> LLWeight for ReadCacheItem<K, V>
where
    K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
    V: Clone + Debug + Sync + Send + 'static,
{
    #[inline]
    fn ll_weight(&self) -> usize {
        self.size
    }
}

struct ReadCache<K, V>
where
    K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
    V: Clone + Debug + Sync + Send + 'static,
{
    // cache of our missed items to send forward.
    // On drop we drain this to the channel
    set: Map<K, LLNodeRef<ReadCacheItem<K, V>>>,
    read_size: usize,
    tlru: LL<ReadCacheItem<K, V>>,
}

/// An active read transaction over the cache. The data is this cache is guaranteed to be
/// valid at the point in time the read is created. You may include items during a cache
/// miss via the "insert" function.
pub struct ARCacheReadTxn<'a, K, V, S>
where
    K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
    V: Clone + Debug + Sync + Send + 'static,
    S: ARCacheReadStat + Clone,
{
    caller: &'a ARCache<K, V>,
    // ro_txn to cache
    cache: DataMapReadTxn<'a, K, CacheItem<K, V>>,
    tlocal: Option<ReadCache<K, V>>,
    hit_queue: Arc<ArrayQueue<CacheHitEvent>>,
    inc_queue: Arc<ArrayQueue<CacheIncludeEvent<K, V>>>,
    above_watermark: bool,
    reader_quiesce: bool,
    stats: S,
}

unsafe impl<
        K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
        V: Clone + Debug + Sync + Send + 'static,
        S: ARCacheReadStat + Clone + Sync + Send + 'static,
    > Send for ARCacheReadTxn<'_, K, V, S>
{
}
unsafe impl<
        K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
        V: Clone + Debug + Sync + Send + 'static,
        S: ARCacheReadStat + Clone + Sync + Send + 'static,
    > Sync for ARCacheReadTxn<'_, K, V, S>
{
}

/// An active write transaction over the cache. The data in this cache is isolated
/// from readers, and may be rolled-back if an error occurs. Changes only become
/// globally visible once you call "commit". Items may be added to the cache on
/// a miss via "insert", and you can explicitly remove items by calling "remove".
pub struct ARCacheWriteTxn<'a, K, V, S>
where
    K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
    V: Clone + Debug + Sync + Send + 'static,
    S: ARCacheWriteStat<K>,
{
    caller: &'a ARCache<K, V>,
    // wr_txn to cache
    cache: DataMapWriteTxn<'a, K, CacheItem<K, V>>,
    // Cache of missed items (w_ dirty/clean)
    // On COMMIT we drain this to the main cache
    tlocal: Map<K, ThreadCacheItem<V>>,
    hit: UnsafeCell<Vec<u64>>,
    clear: UnsafeCell<bool>,
    above_watermark: bool,
    // read_ops: UnsafeCell<u32>,
    stats: S,
}

impl<
        K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
        V: Clone + Debug + Sync + Send + 'static,
    > CacheItem<K, V>
{
    fn to_vref(&self) -> Option<&V> {
        match &self {
            CacheItem::Freq(_, v) | CacheItem::Rec(_, v) => Some(v),
            _ => None,
        }
    }

    fn to_kvsref(&self) -> Option<(&K, &V, usize)> {
        match &self {
            CacheItem::Freq(lln, v) | CacheItem::Rec(lln, v) => {
                let cii = lln.as_ref();
                Some((&cii.k, v, cii.size))
            }
            _ => None,
        }
    }

    #[cfg(test)]
    fn to_state(&self) -> CacheState {
        match &self {
            CacheItem::Freq(_, _v) => CacheState::Freq,
            CacheItem::Rec(_, _v) => CacheState::Rec,
            CacheItem::GhostFreq(_) => CacheState::GhostFreq,
            CacheItem::GhostRec(_) => CacheState::GhostRec,
            CacheItem::Haunted(_) => CacheState::Haunted,
        }
    }
}

/// A configurable builder to create new concurrent Adaptive Replacement Caches.
pub struct ARCacheBuilder {
    max: Option<usize>,
    read_max: Option<usize>,
    watermark: Option<usize>,
    reader_quiesce: bool,
    look_back_limit: Option<u8>,
}

impl Default for ARCacheBuilder {
    fn default() -> Self {
        ARCacheBuilder {
            max: None,
            read_max: None,
            watermark: None,
            reader_quiesce: true,
            look_back_limit: None,
        }
    }
}

impl ARCacheBuilder {
    /// Create a new ARCache builder that you can configure before creation.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure a new ARCache, that derives its size based on your expected workload.
    ///
    /// The values are:
    /// * Total number of items you want to have in memory (soft upper bound)
    /// * Number of reading threads you expect concurrently (excluding write thread)
    /// * Average expected number of cache misses per read transaction
    /// * Average expected number of writes and/or misses per write transaction
    ///
    /// The cache may still exceed your provided total, and inaccurate tuning numbers
    /// will yield a situation where you may use too-little ram, or too much. This could
    /// be to your read misses exceeding your expected amount causing the queues to have
    /// more items in them at a time, or your writes are larger than expected.
    ///
    /// If you set ex_ro_miss to zero, no read thread local cache will be configured, but
    /// space will still be reserved for channel communication.
    #[must_use]
    pub fn set_expected_workload(
        self,
        total: usize,
        threads: usize,
        ex_ro_miss: usize,
        ex_rw_miss: usize,
        read_cache: bool,
    ) -> Self {
        let total = isize::try_from(total).unwrap();
        let threads = isize::try_from(threads).unwrap();
        let ro_miss = isize::try_from(ex_ro_miss).unwrap();
        let wr_miss = isize::try_from(ex_rw_miss).unwrap();

        // We need to clamp the expected read-miss so that the calculation doesn't end up
        // skewing the cache to all "read cache" allocation.
        //
        // We clamp the read-max to an 8th of total div thread. This is because 1/8th read
        // cache is a pretty standard ratio for a shared to per thread cache.
        let read_max = if read_cache {
            let read_max_limit = total / READ_THREAD_CACHE_RATIO;
            let read_max_thread_limit = read_max_limit / threads;
            ro_miss.clamp(0, read_max_thread_limit)
        } else {
            // No read cache requested
            0
        };

        // We have to clamp rw_miss, even though we could go over-size - this is because
        // we need to ensure that rw_miss is always < total.
        let wr_miss_thread_limit = total / WRITE_THREAD_CACHE_RATIO;
        let wr_miss = wr_miss.clamp(0, wr_miss_thread_limit);

        let max = total - (wr_miss + (read_max * threads));

        // Now make them usize.
        let max = usize::try_from(max).unwrap();
        let read_max = usize::try_from(read_max).unwrap();

        ARCacheBuilder {
            // stats: self.stats,
            max: Some(max),
            read_max: Some(read_max),
            watermark: self.watermark,
            reader_quiesce: self.reader_quiesce,
            look_back_limit: self.look_back_limit,
        }
    }

    /// Configure a new ARCache, with a capacity of `max` main cache items and `read_max`
    /// Note that due to the way the cache operates, the number of items can and
    /// will exceed `max` on a regular basis, so you should consider using `set_expected_workload`
    /// and specifying your expected workload parameters to have a better derived
    /// cache size.
    #[must_use]
    pub fn set_size(mut self, max: usize, read_max: usize) -> Self {
        self.max = Some(max);
        self.read_max = Some(read_max);
        self
    }

    /// See [ARCache::new_size] for more information. This allows manual configuration of the data
    /// tracking watermark. To disable this, set to 0. If watermark is greater than
    /// max, it will be clamped to max.
    #[must_use]
    pub fn set_watermark(mut self, watermark: usize) -> Self {
        self.watermark = Some(watermark);
        self
    }

    /// Set the look back limit for ghost lists. This is a balance between the cache's ability
    /// to have "perfect information" about all past keys to tune the p weight, and memory
    /// consumption of storing the ghost lists. If your dataset consists of small keys/large
    /// values you should not change this value from it's default. If your dataset contains
    /// equally sized keys/values, then you may wish to lower this value. The lowest value is
    /// `4`, defaults to `32`.
    #[must_use]
    pub fn set_look_back_limit(mut self, look_back_limit: u8) -> Self {
        self.look_back_limit = Some(look_back_limit);
        self
    }

    /// Enable or Disable reader cache quiescing. In some cases this can improve
    /// reader performance, at the expense that cache includes or hits may be delayed
    /// before acknowledgement. You must MANUALLY run periodic quiesces if you mark
    /// this as "false" to disable reader quiescing.
    #[must_use]
    pub fn set_reader_quiesce(mut self, reader_quiesce: bool) -> Self {
        self.reader_quiesce = reader_quiesce;
        self
    }

    /// Consume this builder, returning a cache if successful. If configured parameters are
    /// missing or incorrect, a None will be returned.
    pub fn build<K, V>(self) -> Option<ARCache<K, V>>
    where
        K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
        V: Clone + Debug + Sync + Send + 'static,
    {
        let ARCacheBuilder {
            // stats,
            max,
            read_max,
            watermark,
            reader_quiesce,
            look_back_limit,
        } = self;

        let (max, read_max) = max.zip(read_max)?;

        let watermark = watermark.unwrap_or(if max < WATERMARK_DISABLE_MIN {
            0
        } else {
            (max / WATERMARK_DISABLE_DIVISOR) * WATERMARK_DISABLE_RATIO
        });
        let watermark = watermark.clamp(0, max);
        // If the watermark is 0, always track from the start.
        let init_watermark = watermark == 0;

        let look_back_limit = look_back_limit
            .unwrap_or(TXN_LOOKBACK_LIMIT_DEFAULT)
            .clamp(TXN_LOOKBACK_LIMIT_ABS_MIN, u8::MAX) as u64;

        // The hit queue is reasonably cheap, so we can let this grow a bit.
        /*
        let chan_size = max / 20;
        let chan_size = if chan_size < 16 { 16 } else { chan_size };
        let chan_size = chan_size.clamp(0, 128);
        */
        let chan_size = WRITE_THREAD_CHANNEL_SIZE;
        let hit_queue = Arc::new(ArrayQueue::new(chan_size));

        // this can oversize and take a lot of time to drain and manage, so we keep this bounded.
        // let chan_size = chan_size.clamp(0, 64);
        let chan_size = READ_THREAD_CHANNEL_SIZE;
        let inc_queue = Arc::new(ArrayQueue::new(chan_size));

        let shared = RwLock::new(ArcShared {
            max,
            read_max,
            // stat_tx,
            hit_queue: hit_queue.clone(),
            inc_queue: inc_queue.clone(),
            watermark,
            reader_quiesce,
        });
        let inner = Mutex::new(ArcInner {
            // We use p from the former stats.
            p: 0,
            freq: LL::new(),
            rec: LL::new(),
            ghost_freq: LL::new(),
            ghost_rec: LL::new(),
            haunted: LL::new(),
            // stat_rx,
            hit_queue,
            inc_queue,
            min_txid: 0,
        });

        Some(ARCache {
            cache: DataMap::new(),
            shared,
            inner,
            // stats: CowCell::new(stats),
            above_watermark: AtomicBool::new(init_watermark),
            look_back_limit,
        })
    }
}

impl<
        K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
        V: Clone + Debug + Sync + Send + 'static,
    > ARCache<K, V>
{
    /// Use ARCacheBuilder instead
    #[deprecated(since = "0.2.20", note = "please use`ARCacheBuilder` instead")]
    pub fn new(
        total: usize,
        threads: usize,
        ex_ro_miss: usize,
        ex_rw_miss: usize,
        read_cache: bool,
    ) -> Self {
        ARCacheBuilder::default()
            .set_expected_workload(total, threads, ex_ro_miss, ex_rw_miss, read_cache)
            .build()
            .expect("Invalid cache parameters!")
    }

    /// Use ARCacheBuilder instead
    #[deprecated(since = "0.2.20", note = "please use`ARCacheBuilder` instead")]
    pub fn new_size(max: usize, read_max: usize) -> Self {
        ARCacheBuilder::default()
            .set_size(max, read_max)
            .build()
            .expect("Invalid cache parameters!")
    }

    /// Use ARCacheBuilder instead
    #[deprecated(since = "0.2.20", note = "please use`ARCacheBuilder` instead")]
    pub fn new_size_watermark(max: usize, read_max: usize, watermark: usize) -> Self {
        ARCacheBuilder::default()
            .set_size(max, read_max)
            .set_watermark(watermark)
            .build()
            .expect("Invalid cache parameters!")
    }

    /// Begin a read operation on the cache. This reader has a thread-local cache for items
    /// that are localled included via `insert`, and can communicate back to the main cache
    /// to safely include items.
    pub fn read_stats<S>(&self, stats: S) -> ARCacheReadTxn<'_, K, V, S>
    where
        S: ARCacheReadStat + Clone,
    {
        let rshared = self.shared.read().unwrap();
        let tlocal = if rshared.read_max > 0 {
            Some(ReadCache {
                set: Map::new(),
                read_size: rshared.read_max,
                tlru: LL::new(),
            })
        } else {
            None
        };
        let above_watermark = self.above_watermark.load(Ordering::Relaxed);
        ARCacheReadTxn {
            caller: self,
            cache: self.cache.read(),
            tlocal,
            // stat_tx: rshared.stat_tx.clone(),
            hit_queue: rshared.hit_queue.clone(),
            inc_queue: rshared.inc_queue.clone(),
            above_watermark,
            reader_quiesce: rshared.reader_quiesce,
            stats,
        }
    }

    /// Begin a read operation on the cache. This reader has a thread-local cache for items
    /// that are localled included via `insert`, and can communicate back to the main cache
    /// to safely include items.
    pub fn read(&self) -> ARCacheReadTxn<'_, K, V, ()> {
        self.read_stats(())
    }

    /// Begin a write operation on the cache. This writer has a thread-local store
    /// for all items that have been included or dirtied in the transactions, items
    /// may be removed from this cache (ie deleted, invalidated).
    pub fn write(&self) -> ARCacheWriteTxn<'_, K, V, ()> {
        self.write_stats(())
    }

    /// _
    pub fn write_stats<S>(&self, stats: S) -> ARCacheWriteTxn<'_, K, V, S>
    where
        S: ARCacheWriteStat<K>,
    {
        let above_watermark = self.above_watermark.load(Ordering::Relaxed);
        ARCacheWriteTxn {
            caller: self,
            cache: self.cache.write(),
            tlocal: Map::new(),
            hit: UnsafeCell::new(Vec::new()),
            clear: UnsafeCell::new(false),
            above_watermark,
            // read_ops: UnsafeCell::new(0),
            stats,
        }
    }

    fn try_write_stats<S>(&self, stats: S) -> Result<ARCacheWriteTxn<'_, K, V, S>, S>
    where
        S: ARCacheWriteStat<K>,
    {
        match self.cache.try_write() {
            Some(cache) => {
                let above_watermark = self.above_watermark.load(Ordering::Relaxed);
                Ok(ARCacheWriteTxn {
                    caller: self,
                    cache,
                    tlocal: Map::new(),
                    hit: UnsafeCell::new(Vec::new()),
                    clear: UnsafeCell::new(false),
                    above_watermark,
                    // read_ops: UnsafeCell::new(0),
                    stats,
                })
            }
            None => Err(stats),
        }
    }

    /// If the lock is available, attempt to quiesce the cache's async channel states. If the lock
    /// is currently held, no action is taken.
    pub fn try_quiesce_stats<S>(&self, stats: S) -> S
    where
        S: ARCacheWriteStat<K>,
    {
        // It seems like a good idea to skip this when not at wmark, but
        // that can cause low-pressure caches to no submit includes properly.
        // if self.above_watermark.load(Ordering::Relaxed) {
        match self.try_write_stats(stats) {
            Ok(wr_txn) => wr_txn.commit(),
            Err(stats) => stats,
        }
    }

    /// If the lock is available, attempt to quiesce the cache's async channel states. If the lock
    /// is currently held, no action is taken.
    pub fn try_quiesce(&self) {
        self.try_quiesce_stats(())
    }

    fn calc_p_freq(ghost_rec_len: usize, ghost_freq_len: usize, p: &mut usize, size: usize) {
        let delta = if ghost_rec_len > ghost_freq_len {
            ghost_rec_len / ghost_freq_len
        } else {
            1
        } * size;
        let p_was = *p;
        if delta < *p {
            *p -= delta
        } else {
            *p = 0
        }
        tracing::trace!("f {} >>> {}", p_was, *p);
    }

    fn calc_p_rec(
        cap: usize,
        ghost_rec_len: usize,
        ghost_freq_len: usize,
        p: &mut usize,
        size: usize,
    ) {
        let delta = if ghost_freq_len > ghost_rec_len {
            ghost_freq_len / ghost_rec_len
        } else {
            1
        } * size;
        let p_was = *p;
        if delta <= cap - *p {
            *p += delta
        } else {
            *p = cap
        }
        tracing::trace!("r {} >>> {}", p_was, *p);
    }

    fn drain_tlocal_inc<S>(
        &self,
        cache: &mut DataMapWriteTxn<K, CacheItem<K, V>>,
        inner: &mut ArcInner<K, V>,
        shared: &ArcShared<K, V>,
        tlocal: Map<K, ThreadCacheItem<V>>,
        commit_txid: u64,
        stats: &mut S,
    ) where
        S: ARCacheWriteStat<K>,
    {
        // drain tlocal into the main cache.
        tlocal.into_iter().for_each(|(k, tcio)| {
            #[cfg(test)]
            {
                inner.rec.verify();
                inner.freq.verify();
                inner.ghost_rec.verify();
                inner.ghost_freq.verify();
                inner.haunted.verify();
            }

            let r = cache.get_mut(&k);
            match (r, tcio) {
                (None, ThreadCacheItem::Present(tci, clean, size)) => {
                    assert!(clean);
                    let llp = inner.rec.append_k(CacheItemInner {
                        k: k.clone(),
                        txid: commit_txid,
                        size,
                    });
                    // stats.write_includes += 1;
                    stats.include(&k);
                    // The key MUST NOT exist in the cache already.
                    let existing = cache.insert(k, CacheItem::Rec(llp, tci));
                    assert!(
                        existing.is_none(),
                        "Impossible state! Key must not already exist in cache!"
                    );
                }
                (None, ThreadCacheItem::Removed(clean)) => {
                    assert!(clean);
                    // Mark this as haunted
                    let llp = inner.haunted.append_k(CacheItemInner {
                        k: k.clone(),
                        txid: commit_txid,
                        size: HAUNTED_SIZE,
                    });
                    // The key MUST NOT exist in the cache already.
                    let existing = cache.insert(k, CacheItem::Haunted(llp));
                    assert!(
                        existing.is_none(),
                        "Impossible state! Key must not already exist in cache!"
                    );
                    // Must be now in haunted!
                    debug_assert!(inner.haunted.len() > 0);
                }
                (Some(ref mut ci), ThreadCacheItem::Removed(clean)) => {
                    assert!(clean);
                    // From whatever set we were in, pop and move to haunted.
                    let mut next_state = match ci {
                        CacheItem::Freq(llp, _v) => {
                            let mut owned = inner.freq.extract(llp.clone());
                            owned.as_mut().txid = commit_txid;
                            owned.as_mut().size = HAUNTED_SIZE;
                            let pointer = inner.haunted.append_n(owned);
                            debug_assert!(inner.haunted.len() > 0);
                            CacheItem::Haunted(pointer)
                        }
                        CacheItem::Rec(llp, _v) => {
                            // Remove the node and put it into freq.
                            let mut owned = inner.rec.extract(llp.clone());
                            owned.as_mut().txid = commit_txid;
                            owned.as_mut().size = HAUNTED_SIZE;
                            let pointer = inner.haunted.append_n(owned);
                            debug_assert!(inner.haunted.len() > 0);
                            CacheItem::Haunted(pointer)
                        }
                        CacheItem::GhostFreq(llp) => {
                            let mut owned = inner.ghost_freq.extract(llp.clone());
                            owned.as_mut().txid = commit_txid;
                            owned.as_mut().size = HAUNTED_SIZE;
                            let pointer = inner.haunted.append_n(owned);
                            debug_assert!(inner.haunted.len() > 0);
                            CacheItem::Haunted(pointer)
                        }
                        CacheItem::GhostRec(llp) => {
                            let mut owned = inner.ghost_rec.extract(llp.clone());
                            owned.as_mut().txid = commit_txid;
                            owned.as_mut().size = HAUNTED_SIZE;
                            let pointer = inner.haunted.append_n(owned);
                            debug_assert!(inner.haunted.len() > 0);
                            CacheItem::Haunted(pointer)
                        }
                        CacheItem::Haunted(llp) => {
                            unsafe { llp.make_mut().txid = commit_txid };
                            debug_assert!(inner.haunted.len() > 0);
                            CacheItem::Haunted(llp.clone())
                        }
                    };
                    // Now change the state.
                    mem::swap(*ci, &mut next_state);
                }
                // Done! https://github.com/rust-lang/rust/issues/68354 will stabilise
                // in 1.44 so we can prevent a need for a clone.
                (Some(ref mut ci), ThreadCacheItem::Present(tci, clean, size)) => {
                    assert!(clean);
                    //   * as we include each item, what state was it in before?
                    // It's in the cache - what action must we take?
                    let mut next_state = match ci {
                        CacheItem::Freq(llp, _v) => {
                            let mut owned = inner.freq.extract(llp.clone());
                            owned.as_mut().txid = commit_txid;
                            owned.as_mut().size = size;
                            // Move the list item to it's head.
                            stats.modify(&owned.as_ref().k);
                            let pointer = inner.freq.append_n(owned);
                            // Update v.
                            CacheItem::Freq(pointer, tci)
                        }
                        CacheItem::Rec(llp, _v) => {
                            // Remove the node and put it into freq.
                            let mut owned = inner.rec.extract(llp.clone());
                            owned.as_mut().txid = commit_txid;
                            owned.as_mut().size = size;
                            stats.modify(&owned.as_ref().k);
                            let pointer = inner.freq.append_n(owned);
                            CacheItem::Freq(pointer, tci)
                        }
                        CacheItem::GhostFreq(llp) => {
                            // Adjust p
                            Self::calc_p_freq(
                                inner.ghost_rec.len(),
                                inner.ghost_freq.len(),
                                &mut inner.p,
                                size,
                            );
                            let mut owned = inner.ghost_freq.extract(llp.clone());
                            owned.as_mut().txid = commit_txid;
                            owned.as_mut().size = size;
                            stats.ghost_frequent_revive(&owned.as_ref().k);
                            let pointer = inner.freq.append_n(owned);
                            CacheItem::Freq(pointer, tci)
                        }
                        CacheItem::GhostRec(llp) => {
                            // Adjust p
                            Self::calc_p_rec(
                                shared.max,
                                inner.ghost_rec.len(),
                                inner.ghost_freq.len(),
                                &mut inner.p,
                                size,
                            );
                            let mut owned = inner.ghost_rec.extract(llp.clone());
                            owned.as_mut().txid = commit_txid;
                            owned.as_mut().size = size;
                            stats.ghost_recent_revive(&owned.as_ref().k);
                            let pointer = inner.rec.append_n(owned);
                            CacheItem::Rec(pointer, tci)
                        }
                        CacheItem::Haunted(llp) => {
                            // Moving from haunted to recent.
                            // How can an item be Haunted, but then not be in the haunted set?

                            // eprintln!("{:?}", inner.haunted.len());
                            let before_len = inner.haunted.len();
                            debug_assert!(inner.haunted.len() > 0);
                            let mut owned = inner.haunted.extract(llp.clone());
                            debug_assert!(before_len - HAUNTED_SIZE == inner.haunted.len());

                            owned.as_mut().txid = commit_txid;
                            owned.as_mut().size = size;
                            stats.include_haunted(&owned.as_ref().k);
                            let pointer = inner.rec.append_n(owned);
                            CacheItem::Rec(pointer, tci)
                        }
                    };
                    // Now change the state.
                    mem::swap(*ci, &mut next_state);
                }
            }

            #[cfg(test)]
            {
                inner.rec.verify();
                inner.freq.verify();
                inner.ghost_rec.verify();
                inner.ghost_freq.verify();
                inner.haunted.verify();
            }
        });
    }

    fn drain_hit_rx(
        &self,
        cache: &mut DataMapWriteTxn<K, CacheItem<K, V>>,
        inner: &mut ArcInner<K, V>,
        commit_ts: Instant,
    ) {
        // * for each item
        // while let Ok(ce) = inner.rx.try_recv() {
        // TODO: Find a way to remove these clones here!
        while let Some(ce) = inner.hit_queue.pop() {
            let CacheHitEvent { t, k_hash } = ce;
            if let Some(ref mut ci_slots) = unsafe { cache.get_slot_mut(k_hash) } {
                for ref mut ci in ci_slots.iter_mut() {
                    let mut next_state = match &ci.v {
                        CacheItem::Freq(llp, v) => {
                            inner.freq.touch(llp.to_owned());
                            CacheItem::Freq(llp.to_owned(), v.to_owned())
                        }
                        CacheItem::Rec(llp, v) => {
                            let owned = inner.rec.extract(llp.to_owned());
                            let pointer = inner.freq.append_n(owned);
                            CacheItem::Freq(pointer, v.to_owned())
                        }
                        // While we can't add this from nothing, we can
                        // at least keep it in the ghost sets.
                        CacheItem::GhostFreq(llp) => {
                            inner.ghost_freq.touch(llp.to_owned());
                            CacheItem::GhostFreq(llp.to_owned())
                        }
                        CacheItem::GhostRec(llp) => {
                            inner.ghost_rec.touch(llp.to_owned());
                            CacheItem::GhostRec(llp.to_owned())
                        }
                        CacheItem::Haunted(llp) => {
                            // We can't do anything about this ...
                            // Don't touch or rearrange the haunted list, it should be
                            // in commit txid order.
                            debug_assert!(inner.haunted.len() > 0);
                            CacheItem::Haunted(llp.to_owned())
                        }
                    };
                    mem::swap(&mut ci.v, &mut next_state);
                } // for each item in the bucket.
            }
            // Do nothing, it must have been evicted.

            // Stop processing the queue, we are up to "now".
            if t >= commit_ts {
                break;
            }
        }
    }

    fn drain_inc_rx<S>(
        &self,
        cache: &mut DataMapWriteTxn<K, CacheItem<K, V>>,
        inner: &mut ArcInner<K, V>,
        shared: &ArcShared<K, V>,
        commit_ts: Instant,
        stats: &mut S,
    ) where
        S: ARCacheWriteStat<K>,
    {
        while let Some(ce) = inner.inc_queue.pop() {
            // Update if it was inc
            let CacheIncludeEvent {
                t,
                k,
                v: iv,
                txid,
                size,
            } = ce;
            let mut r = cache.get_mut(&k);
            match r {
                Some(ref mut ci) => {
                    let mut next_state = match &ci {
                        CacheItem::Freq(llp, _v) => {
                            if llp.as_ref().txid >= txid || inner.min_txid > txid {
                                // Our cache already has a newer value, keep it.
                                inner.freq.touch(llp.to_owned());
                                None
                            } else {
                                // The value is newer, update.
                                let mut owned = inner.freq.extract(llp.to_owned());
                                owned.as_mut().txid = txid;
                                owned.as_mut().size = size;
                                stats.modify(&owned.as_mut().k);
                                let pointer = inner.freq.append_n(owned);
                                Some(CacheItem::Freq(pointer, iv))
                            }
                        }
                        CacheItem::Rec(llp, v) => {
                            let mut owned = inner.rec.extract(llp.to_owned());
                            if llp.as_ref().txid >= txid || inner.min_txid > txid {
                                let pointer = inner.freq.append_n(owned);
                                Some(CacheItem::Freq(pointer, v.to_owned()))
                            } else {
                                owned.as_mut().txid = txid;
                                owned.as_mut().size = size;
                                stats.modify(&owned.as_mut().k);
                                let pointer = inner.freq.append_n(owned);
                                Some(CacheItem::Freq(pointer, iv))
                            }
                        }
                        CacheItem::GhostFreq(llp) => {
                            // Adjust p
                            if llp.as_ref().txid > txid || inner.min_txid > txid {
                                // The cache version is newer, this is just a hit.
                                let size = llp.as_ref().size;
                                Self::calc_p_freq(
                                    inner.ghost_rec.len(),
                                    inner.ghost_freq.len(),
                                    &mut inner.p,
                                    size,
                                );
                                inner.ghost_freq.touch(llp.to_owned());
                                None
                            } else {
                                // This item is newer, so we can include it.
                                Self::calc_p_freq(
                                    inner.ghost_rec.len(),
                                    inner.ghost_freq.len(),
                                    &mut inner.p,
                                    size,
                                );
                                let mut owned = inner.ghost_freq.extract(llp.to_owned());
                                owned.as_mut().txid = txid;
                                owned.as_mut().size = size;
                                stats.ghost_frequent_revive(&owned.as_mut().k);
                                let pointer = inner.freq.append_n(owned);
                                Some(CacheItem::Freq(pointer, iv))
                            }
                        }
                        CacheItem::GhostRec(llp) => {
                            // Adjust p
                            if llp.as_ref().txid > txid || inner.min_txid > txid {
                                let size = llp.as_ref().size;
                                Self::calc_p_rec(
                                    shared.max,
                                    inner.ghost_rec.len(),
                                    inner.ghost_freq.len(),
                                    &mut inner.p,
                                    size,
                                );
                                inner.ghost_rec.touch(llp.clone());
                                None
                            } else {
                                Self::calc_p_rec(
                                    shared.max,
                                    inner.ghost_rec.len(),
                                    inner.ghost_freq.len(),
                                    &mut inner.p,
                                    size,
                                );
                                let mut owned = inner.ghost_rec.extract(llp.to_owned());
                                owned.as_mut().txid = txid;
                                owned.as_mut().size = size;
                                stats.ghost_recent_revive(&owned.as_ref().k);
                                let pointer = inner.rec.append_n(owned);
                                Some(CacheItem::Rec(pointer, iv))
                            }
                        }
                        CacheItem::Haunted(llp) => {
                            // if
                            //     the haunted item is newer
                            // OR
                            //     inclusion is older than our minimum
                            // then the item is skipped.
                            if llp.as_ref().txid > txid || inner.min_txid > txid {
                                None
                            } else {
                                // ELSE we need to update the txid of the haunted item
                                // to ensure that it's at the list head.
                                debug_assert!(inner.haunted.len() > 0);
                                let before_len = inner.haunted.len();

                                let mut owned = inner.haunted.extract(llp.to_owned());

                                debug_assert!(before_len - HAUNTED_SIZE == inner.haunted.len());

                                owned.as_mut().txid = txid;
                                debug_assert!(owned.as_mut().size == HAUNTED_SIZE);
                                stats.include_haunted(&owned.as_mut().k);
                                let pointer = inner.rec.append_n(owned);
                                Some(CacheItem::Rec(pointer, iv))
                            }
                        }
                    };
                    if let Some(ref mut next_state) = next_state {
                        mem::swap(*ci, next_state);
                    }
                }
                None => {
                    // This key has never been seen before.
                    // It's not present - include it!
                    if txid >= inner.min_txid {
                        let llp = inner.rec.append_k(CacheItemInner {
                            k: k.clone(),
                            txid,
                            size,
                        });
                        stats.include(&k);
                        // The key MUST NOT exist in the cache already.
                        let existing = cache.insert(k, CacheItem::Rec(llp, iv));
                        assert!(
                            existing.is_none(),
                            "Impossible state! Key must not already exist in cache!"
                        );
                    }
                }
            };

            // Stop processing the queue, we are up to "now".
            if t >= commit_ts {
                break;
            }
        }
    }

    fn drain_tlocal_hits(
        &self,
        cache: &mut DataMapWriteTxn<K, CacheItem<K, V>>,
        inner: &mut ArcInner<K, V>,
        // shared: &ArcShared<K, V>,
        commit_txid: u64,
        hit: Vec<u64>,
    ) {
        // Stats updated by caller
        hit.into_iter().for_each(|k_hash| {
            // * everything hit must be in main cache now, so bring these
            //   all to the relevant item heads.
            // * Why do this last? Because the write is the "latest" we want all the fresh
            //   written items in the cache over the "read" hits, it gives us some appximation
            //   of time ordering, but not perfect.

            // Find the item in the cache.
            // * based on it's type, promote it in the correct list, or move it.
            // How does this prevent incorrect promotion from rec to freq? txid?
            let mut r = unsafe { cache.get_slot_mut(k_hash) };
            match r {
                Some(ref mut ci_slots) => {
                    for ref mut ci in ci_slots.iter_mut() {
                        // This differs from above - we skip if we don't touch anything
                        // that was added in this txn. This is to prevent double touching
                        // anything that was included in a write.

                        // TODO: find a way to remove these clones
                        let mut next_state = match &ci.v {
                            CacheItem::Freq(llp, v) => {
                                // To prevent cache hysterisis, we require a hit over
                                // two transactions.
                                if llp.as_ref().txid != commit_txid {
                                    inner.freq.touch(llp.to_owned());
                                    Some(CacheItem::Freq(llp.to_owned(), v.to_owned()))
                                } else {
                                    None
                                }
                            }
                            CacheItem::Rec(llp, v) => {
                                if llp.as_ref().txid != commit_txid {
                                    // println!("hit {:?} Rec -> Freq", k);
                                    let owned = inner.rec.extract(llp.clone());
                                    let pointer = inner.freq.append_n(owned);
                                    Some(CacheItem::Freq(pointer, v.clone()))
                                } else {
                                    None
                                }
                            }
                            _ => {
                                // Ignore hits on items that may have been cleared.
                                None
                            }
                        };
                        // Now change the state.
                        if let Some(ref mut next_state) = next_state {
                            mem::swap(&mut ci.v, next_state);
                        }
                    } // for each ci in slots
                }
                None => {
                    // Impossible state!
                    unreachable!();
                }
            }
        });
    }

    fn evict_to_haunted_len(
        cache: &mut DataMapWriteTxn<K, CacheItem<K, V>>,
        ll: &mut LL<CacheItemInner<K>>,
        to_ll: &mut LL<CacheItemInner<K>>,
        size: usize,
        txid: u64,
    ) {
        let to_ll_before = to_ll.len();
        let ll_before = ll.len();
        let mut added = 0;
        let mut removed = 0;

        while ll.len() > size {
            #[cfg(test)]
            {
                ll.verify();
                to_ll.verify();
            }

            if let Some(mut owned) = ll.pop() {
                debug_assert!(!owned.is_null());

                // Track the sizes.
                removed += owned.as_mut().size;

                assert_eq!(ll.len(), ll_before - removed);

                // Set the item's evict txid.
                owned.as_mut().txid = txid;
                // Trim the haunted size as needed.
                owned.as_mut().size = HAUNTED_SIZE;
                added += HAUNTED_SIZE;

                let pointer = to_ll.append_n(owned);

                assert_eq!(
                    to_ll.len(),
                    to_ll_before + added,
                    "Impossible State! List lengths are no longer consistent!"
                );

                let mut r = cache.get_mut(&pointer.as_ref().k);

                match r {
                    Some(ref mut ci) => {
                        // Now change the state.
                        let mut next_state = CacheItem::Haunted(pointer);
                        mem::swap(*ci, &mut next_state);
                    }
                    None => {
                        // Impossible state!
                        unreachable!();
                    }
                };
            } else {
                // Impossible state!
                unreachable!();
            }

            #[cfg(test)]
            {
                ll.verify();
                to_ll.verify();
            }
        }
    }

    fn evict_to_len<S>(
        cache: &mut DataMapWriteTxn<K, CacheItem<K, V>>,
        ll: &mut LL<CacheItemInner<K>>,
        to_ll: &mut LL<CacheItemInner<K>>,
        size: usize,
        txid: u64,
        stats: &mut S,
    ) where
        S: ARCacheWriteStat<K>,
    {
        debug_assert!(ll.len() >= size);

        while ll.len() > size {
            #[cfg(test)]
            {
                ll.verify();
                to_ll.verify();
            }

            if let Some(mut owned) = ll.pop() {
                debug_assert!(!owned.is_null());
                let mut r = cache.get_mut(&owned.as_ref().k);
                // Set the item's evict txid.
                owned.as_mut().txid = txid;
                match r {
                    Some(ref mut ci) => {
                        let mut next_state = match &ci {
                            CacheItem::Freq(llp, _v) => {
                                // The pointer from any key MUST be unique!
                                assert!(llp == &owned, "Impossible State! Pointer in map does not match the pointer from the list!");
                                // No need to extract, already popped!
                                // $ll.extract(*llp);
                                stats.evict_from_frequent(&owned.as_ref().k);
                                let pointer = to_ll.append_n(owned);
                                CacheItem::GhostFreq(pointer)
                            }
                            CacheItem::Rec(llp, _v) => {
                                // The pointer from any key MUST be unique!
                                assert!(llp == &owned, "Impossible State! Pointer in map does not match the pointer from the list!");
                                // No need to extract, already popped!
                                // $ll.extract(*llp);
                                stats.evict_from_recent(&owned.as_mut().k);
                                let pointer = to_ll.append_n(owned);
                                CacheItem::GhostRec(pointer)
                            }
                            _ => {
                                // Impossible state! All members of the from-ll, must be
                                // in either the frequent or recent state.
                                unreachable!();
                            }
                        };
                        // Now change the state.
                        mem::swap(*ci, &mut next_state);
                    }
                    None => {
                        // Impossible state! This indicates that the key was already
                        // removed. Only one key -> linked-list-pointer should exist at
                        // anytime. If we already removed this, that indicates there were
                        // two llp's with the same key!
                        unreachable!();
                    }
                };
            } else {
                //  Impossible state!
                unreachable!();
            }

            #[cfg(test)]
            {
                ll.verify();
                to_ll.verify();
            }
        } // end while
    }

    #[allow(clippy::cognitive_complexity)]
    fn evict<S>(
        &self,
        cache: &mut DataMapWriteTxn<K, CacheItem<K, V>>,
        inner: &mut ArcInner<K, V>,
        shared: &ArcShared<K, V>,
        commit_txid: u64,
        stats: &mut S,
    ) where
        S: ARCacheWriteStat<K>,
    {
        debug_assert!(inner.p <= shared.max);
        // Convince the compiler copying is okay.
        let p = inner.p;

        if inner.rec.len() + inner.freq.len() > shared.max {
            // println!("Checking cache evict");
            trace!(
                "from -> rec {:?}, freq {:?}",
                inner.rec.len(),
                inner.freq.len()
            );
            let delta = (inner.rec.len() + inner.freq.len()) - shared.max;
            // We have overflowed by delta. As we are not "evicting as we go" we have to work out
            // what we should have evicted up to now.
            //
            // keep removing from rec until == p OR delta == 0, and if delta remains, then remove from freq.
            //
            // Remember P is "the maximum size of recent" or "pressure on recent". If P is max then
            // we are pressuring churning on recent but not freq, so evict in freq.
            //
            // If P is toward 0 that means all our pressure is in frequent and we evicted things we
            // shouldn't have so we want more space in frequent and less in rec.

            // delta here is the number of elements we need to remove to remain below shared.max.

            let rec_to_len = if inner.p == 0 {
                trace!("p == 0 => {:?} - {}", inner.rec.len(), delta);
                if delta < inner.rec.len() {
                    // We are fully weighted to freq, so only remove in recent.
                    inner.rec.len() - delta
                } else {
                    // We need to fully clear rec *and* then from freq as well.
                    0
                }
            } else if inner.rec.len() > inner.p {
                // There is a partial weighting, how much do we need to move?
                let rec_delta = inner.rec.len() - inner.p;
                if rec_delta > delta {
                    /*
                    println!(
                        "p ({:?}) <= rec ({:?}), rec_delta ({:?}) > delta ({:?})",
                        inner.p,
                        inner.rec.len(),
                        rec_delta,
                        delta
                    );
                    */
                    // We will have removed enough through delta alone in rec. Technically
                    // this means we are still over p, but since we already removed delta
                    // number of elements, freq won't change dimensions.
                    inner.rec.len() - delta
                } else {
                    /*
                    println!(
                        "p ({:?}) <= rec ({:?}), rec_delta ({:?}) <= delta ({:?})",
                        inner.p,
                        inner.rec.len(),
                        rec_delta,
                        delta
                    );
                    */
                    // Remove the full recent delta, and excess will be removed from freq.
                    inner.rec.len() - rec_delta
                }
            } else {
                // rec is already below p, therefore we must need to remove in freq, and
                // we need to consider how much is in rec.
                // println!("p ({:?}) > rec ({:?})", inner.p, inner.rec.len());
                inner.rec.len()
            };

            // Now we can get the expected sizes;
            let freq_to_len = shared.max - rec_to_len;
            // println!("move to -> rec {:?}, freq {:?}", rec_to_len, freq_to_len);
            debug_assert!(rec_to_len <= inner.rec.len());
            debug_assert!(freq_to_len <= inner.freq.len());

            // stats.freq_evicts += (inner.freq.len() - freq_to_len) as u64;
            // stats.recent_evicts += (inner.rec.len() - rec_to_len) as u64;
            // stats.frequent_evict_add((inner.freq.len() - freq_to_len) as u64);
            // stats.recent_evict_add((inner.rec.len() - rec_to_len) as u64);

            Self::evict_to_len(
                cache,
                &mut inner.rec,
                &mut inner.ghost_rec,
                rec_to_len,
                commit_txid,
                stats,
            );
            Self::evict_to_len(
                cache,
                &mut inner.freq,
                &mut inner.ghost_freq,
                freq_to_len,
                commit_txid,
                stats,
            );

            // Finally, do an evict of the ghost sets if they are too long - these are weighted
            // inverse to the above sets. Note the freq to len in ghost rec, and rec to len in
            // ghost freq!
            if inner.ghost_rec.len() > (shared.max - p) {
                Self::evict_to_haunted_len(
                    cache,
                    &mut inner.ghost_rec,
                    &mut inner.haunted,
                    freq_to_len,
                    commit_txid,
                );
            }

            if inner.ghost_freq.len() > p {
                Self::evict_to_haunted_len(
                    cache,
                    &mut inner.ghost_freq,
                    &mut inner.haunted,
                    rec_to_len,
                    commit_txid,
                );
            }
        }
    }

    fn drain_ll_to_ghost<S>(
        cache: &mut DataMapWriteTxn<K, CacheItem<K, V>>,
        ll: &mut LL<CacheItemInner<K>>,
        gf: &mut LL<CacheItemInner<K>>,
        gr: &mut LL<CacheItemInner<K>>,
        txid: u64,
        stats: &mut S,
    ) where
        S: ARCacheWriteStat<K>,
    {
        while let Some(mut owned) = ll.pop() {
            #[cfg(test)]
            {
                ll.verify();
                gf.verify();
                gr.verify();
            }

            debug_assert!(!owned.is_null());

            // Set the item's eviction txid.
            owned.as_mut().txid = txid;

            let mut r = cache.get_mut(&owned.as_ref().k);
            match r {
                Some(ref mut ci) => {
                    let mut next_state = match &ci {
                        CacheItem::Freq(n, _) => {
                            debug_assert!(n == &owned);
                            stats.evict_from_frequent(&owned.as_ref().k);
                            let pointer = gf.append_n(owned);
                            CacheItem::GhostFreq(pointer)
                        }
                        CacheItem::Rec(n, _) => {
                            debug_assert!(n == &owned);
                            stats.evict_from_recent(&owned.as_ref().k);
                            let pointer = gr.append_n(owned);
                            CacheItem::GhostRec(pointer)
                        }
                        _ => {
                            // Impossible state!
                            unreachable!();
                        }
                    };
                    // Now change the state.
                    mem::swap(*ci, &mut next_state);
                }
                None => {
                    // Impossible state! This indicates that the key was already
                    // removed. Only one key -> linked-list-pointer should exist at
                    // anytime. If we already removed this, that indicates there were
                    // two llp's with the same key!
                    unreachable!();
                }
            }

            #[cfg(test)]
            {
                ll.verify();
                gf.verify();
                gr.verify();
            }
        } // end while
    }

    fn drain_ll_min_txid(
        cache: &mut DataMapWriteTxn<K, CacheItem<K, V>>,
        ll: &mut LL<CacheItemInner<K>>,
        min_txid: u64,
    ) {
        while let Some(node) = ll.peek_head() {
            #[cfg(test)]
            {
                ll.verify();
            }

            // if the node is older than our min txid.
            if node.txid < min_txid {
                let before_len = ll.len();
                debug_assert!(ll.len() > 0);

                // Need to free from the cache.
                cache.remove(&node.k);

                // Okay, this node can be trimmed.
                ll.drop_head();

                debug_assert!(before_len - HAUNTED_SIZE == ll.len());
            } else {
                // We are done with this loop, everything else
                // is newer.
                break;
            }

            #[cfg(test)]
            {
                ll.verify();
            }
        }
    }

    #[allow(clippy::unnecessary_mut_passed)]
    fn commit<S>(
        &self,
        mut cache: DataMapWriteTxn<K, CacheItem<K, V>>,
        tlocal: Map<K, ThreadCacheItem<V>>,
        hit: Vec<u64>,
        clear: bool,
        init_above_watermark: bool,
        // read_ops: u32,
        mut stats: S,
    ) -> S
    where
        S: ARCacheWriteStat<K>,
    {
        // What is the time?
        let commit_ts = Instant::now();
        let commit_txid = cache.get_txid();
        // Copy p + init cache sizes for adjustment.
        let mut inner = self.inner.lock().unwrap();
        let shared = self.shared.read().unwrap();

        // Did we request to be cleared? If so, we move everything to a ghost set
        // that was live.
        //
        // we also set the min_txid watermark which prevents any inclusion of
        // any item that existed before this point in time.
        if clear {
            // Set the watermark of this txn.
            inner.min_txid = commit_txid;

            // Indicate that we evicted all to ghost/freq
            // stats.frequent_evict_add(inner.freq.len() as u64);
            // stats.recent_evict_add(inner.rec.len() as u64);

            // This weird looking dance is to convince rust that the mutable borrow is safe.
            let m_inner = inner.deref_mut();

            let i_f = &mut m_inner.freq;
            let g_f = &mut m_inner.ghost_freq;
            let i_r = &mut m_inner.rec;
            let g_r = &mut m_inner.ghost_rec;

            // Move everything active into ghost sets.
            Self::drain_ll_to_ghost(&mut cache, i_f, g_f, g_r, commit_txid, &mut stats);
            Self::drain_ll_to_ghost(&mut cache, i_r, g_f, g_r, commit_txid, &mut stats);
        } else {
            // Update the minimum txid we'll include from based on the look back limit
            //
            // If a clear happens, we don't want this setting the limit back lower than an existing
            // limit, so we take the greater of these values.
            let possible_new_limit = commit_txid.saturating_sub(self.look_back_limit);
            inner.min_txid = inner.min_txid.max(possible_new_limit);
        }

        // Why is it okay to drain the rx/tlocal and create the cache in a temporary
        // oversize? Because these values in the queue/tlocal are already in memory
        // and we are moving them to the cache, we are not actually using any more
        // memory (well, not significantly more). By adding everything, then evicting
        // we also get better and more accurate hit patterns over the cache based on what
        // was used. This gives us an advantage over other cache types - we can see
        // patterns based on temporal usage that other caches can't, at the expense that
        // it may take some moments for that cache pattern to sync to the main thread.

        self.drain_tlocal_inc(
            &mut cache,
            inner.deref_mut(),
            shared.deref(),
            tlocal,
            commit_txid,
            &mut stats,
        );

        // drain rx until empty or time >= time.
        self.drain_inc_rx(
            &mut cache,
            inner.deref_mut(),
            shared.deref(),
            commit_ts,
            &mut stats,
        );

        self.drain_hit_rx(&mut cache, inner.deref_mut(), commit_ts);

        // drain the tlocal hits into the main cache.

        // stats.write_hits += hit.len() as u64;
        // stats.write_read_ops += read_ops as u64;

        self.drain_tlocal_hits(&mut cache, inner.deref_mut(), commit_txid, hit);

        // now clean the space for each of the primary caches, evicting into the ghost sets.
        // * It's possible that both caches are now over-sized if rx was empty
        //   but wr inc many items.
        // * p has possibly changed width, causing a balance shift
        // * and ghost items have been included changing ghost list sizes.
        // so we need to do a clean up/balance of all the list lengths.
        self.evict(
            &mut cache,
            inner.deref_mut(),
            shared.deref(),
            commit_txid,
            &mut stats,
        );

        // self.drain_stat_rx(inner.deref_mut(), stats, commit_ts);

        // Now drain from the haunted set if required, this removes anything
        // lower than the min_txid.
        {
            // Tell the compiler copying is okay.
            let min_txid = inner.min_txid;
            Self::drain_ll_min_txid(&mut cache, &mut inner.haunted, min_txid);
        }

        stats.p_weight(inner.p as u64);
        stats.shared_max(shared.max as u64);
        stats.freq(inner.freq.len() as u64);
        stats.recent(inner.rec.len() as u64);
        stats.all_seen_keys(cache.len() as u64);

        // Indicate if we are at/above watermark, so that read/writers begin to indicate their
        // hit events so we can start to setup/order our arc sets correctly.
        //
        // If we drop below this again, they'll go back to just insert/remove content only mode.
        if init_above_watermark {
            // We were above, now we are below the limit, go back to just insert/remove only mode.
            if (inner.freq.len() + inner.rec.len()) < shared.watermark {
                self.above_watermark.store(false, Ordering::Relaxed);
            }
        } else if (inner.freq.len() + inner.rec.len()) >= shared.watermark {
            // we were not above above the watermark but now the cache is large enough
            // to demand that we should be tracking data.
            self.above_watermark.store(true, Ordering::Relaxed);
        }

        // commit on the wr txn.
        cache.commit();
        // done!

        // Return the stats to the caller.
        stats
    }
}

impl<
        K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
        V: Clone + Debug + Sync + Send + 'static,
        S: ARCacheWriteStat<K>,
    > ARCacheWriteTxn<'_, K, V, S>
{
    /// Commit the changes of this writer, making them globally visible. This causes
    /// all items written to this thread's local store to become visible in the main
    /// cache.
    ///
    /// To rollback (abort) and operation, just do not call commit (consider std::mem::drop
    /// on the write transaction)
    pub fn commit(self) -> S {
        self.caller.commit(
            self.cache,
            self.tlocal,
            self.hit.into_inner(),
            self.clear.into_inner(),
            self.above_watermark,
            // self.read_ops.into_inner(),
            self.stats,
        )
    }

    /// Clear all items of the cache. This operation does not take effect until you commit.
    /// After calling "clear", you may then include new items which will be stored thread
    /// locally until you commit.
    pub fn clear(&mut self) {
        // Mark that we have been requested to clear the cache.
        unsafe {
            let clear_ptr = self.clear.get();
            *clear_ptr = true;
        }
        // Dump the hit log.
        unsafe {
            let hit_ptr = self.hit.get();
            (*hit_ptr).clear();
        }

        // Throw away any read ops we did on the old values since they'll
        // mess up stat numbers.
        self.stats.cache_clear();
        /*
        unsafe {
            let op_ptr = self.read_ops.get();
            (*op_ptr) = 0;
        }
        */

        // Dump the thread local state.
        self.tlocal.clear();
        // From this point any get will miss on the main cache.
        // Inserts are accepted.
    }

    /// Attempt to retrieve a k-v pair from the cache. If it is present in the main cache OR
    /// the thread local cache, a `Some` is returned, else you will receive a `None`. On a
    /// `None`, you must then consult the external data source that this structure is acting
    /// as a cache for.
    pub fn get<Q>(&mut self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + Ord + ?Sized,
    {
        let k_hash: u64 = self.cache.prehash(k);

        // Track the attempted read op
        /*
        unsafe {
            let op_ptr = self.read_ops.get();
            (*op_ptr) += 1;
        }
        */
        self.stats.cache_read();

        let r: Option<&V> = if let Some(tci) = self.tlocal.get(k) {
            match tci {
                ThreadCacheItem::Present(v, _clean, _size) => {
                    let v = v as *const _;
                    unsafe { Some(&(*v)) }
                }
                ThreadCacheItem::Removed(_clean) => {
                    return None;
                }
            }
        } else {
            // If we have been requested to clear, the main cache is "empty"
            // but we can't do that until a commit, so we just flag it and avoid.
            let is_cleared = unsafe {
                let clear_ptr = self.clear.get();
                *clear_ptr
            };
            if !is_cleared {
                if let Some(v) = self.cache.get_prehashed(k, k_hash) {
                    (*v).to_vref()
                } else {
                    None
                }
            } else {
                None
            }
        };

        if r.is_some() {
            self.stats.cache_hit();
        }

        // How do we track this was a hit?
        // Remember, we don't track misses - they are *implied* by the fact they'll trigger
        // an inclusion from the external system. Subsequent, any further re-hit on an
        // included value WILL be tracked, allowing arc to adjust appropriately.
        if self.above_watermark && r.is_some() {
            unsafe {
                let hit_ptr = self.hit.get();
                (*hit_ptr).push(k_hash);
            }
        }
        r
    }

    /// If a value is in the thread local cache, retrieve it for mutation. If the value
    /// is not in the thread local cache, it is retrieved and cloned from the main cache. If
    /// the value had been marked for removal, it must first be re-inserted.
    ///
    /// # Safety
    ///
    /// Since you are mutating the state of the value, if you have sized insertions you MAY
    /// break this since you can change the weight of the value to be inconsistent
    pub fn get_mut<Q>(&mut self, k: &Q, make_dirty: bool) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + Ord + ?Sized,
    {
        // If we were requested to clear, we can not copy to the tlocal cache.
        let is_cleared = unsafe {
            let clear_ptr = self.clear.get();
            *clear_ptr
        };

        // If the main cache has NOT been cleared (ie it has items) and our tlocal
        // does NOT contain this key, then we prime it.
        if !is_cleared && !self.tlocal.contains_key(k) {
            // Copy from the core cache into the tlocal.
            let k_hash: u64 = self.cache.prehash(k);
            if let Some(v) = self.cache.get_prehashed(k, k_hash) {
                if let Some((dk, dv, ds)) = v.to_kvsref() {
                    self.tlocal.insert(
                        dk.clone(),
                        ThreadCacheItem::Present(dv.clone(), !make_dirty, ds),
                    );
                }
            }
        };

        // Now return from the tlocal, if present, a mut pointer.

        match self.tlocal.get_mut(k) {
            Some(ThreadCacheItem::Present(v, clean, _size)) => {
                if make_dirty && *clean {
                    *clean = false;
                }
                let v = v as *mut _;
                unsafe { Some(&mut (*v)) }
            }
            _ => None,
        }
    }

    /// Determine if this cache contains the following key.
    pub fn contains_key<Q>(&mut self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + Ord + ?Sized,
    {
        self.get(k).is_some()
    }

    /// Add a value to the cache. This may be because you have had a cache miss and
    /// now wish to include in the thread local storage, or because you have written
    /// a new value and want it to be submitted for caching. This item is marked as
    /// clean, IE you have synced it to whatever associated store exists.
    pub fn insert(&mut self, k: K, v: V) {
        self.tlocal.insert(k, ThreadCacheItem::Present(v, true, 1));
    }

    /// Insert an item to the cache, with an associated weight/size factor. See also `insert`
    pub fn insert_sized(&mut self, k: K, v: V, size: NonZeroUsize) {
        self.tlocal
            .insert(k, ThreadCacheItem::Present(v, true, size.get()));
    }

    /// Remove this value from the thread local cache IE mask from from being
    /// returned until this thread performs an insert. This item is marked as clean
    /// IE you have synced it to whatever associated store exists.
    pub fn remove(&mut self, k: K) {
        self.tlocal.insert(k, ThreadCacheItem::Removed(true));
    }

    /// Add a value to the cache. This may be because you have had a cache miss and
    /// now wish to include in the thread local storage, or because you have written
    /// a new value and want it to be submitted for caching. This item is marked as
    /// dirty, because you have *not* synced it. You MUST call iter_mut_mark_clean before calling
    /// `commit` on this transaction, or a panic will occur.
    pub fn insert_dirty(&mut self, k: K, v: V) {
        self.tlocal.insert(k, ThreadCacheItem::Present(v, false, 1));
    }

    /// Insert a dirty item to the cache, with an associated weight/size factor. See also `insert_dirty`
    pub fn insert_dirty_sized(&mut self, k: K, v: V, size: NonZeroUsize) {
        self.tlocal
            .insert(k, ThreadCacheItem::Present(v, false, size.get()));
    }

    /// Remove this value from the thread local cache IE mask from from being
    /// returned until this thread performs an insert. This item is marked as
    /// dirty, because you have *not* synced it. You MUST call iter_mut_mark_clean before calling
    /// `commit` on this transaction, or a panic will occur.
    pub fn remove_dirty(&mut self, k: K) {
        self.tlocal.insert(k, ThreadCacheItem::Removed(false));
    }

    /// Determines if dirty elements exist in this cache or not.
    pub fn is_dirty(&self) -> bool {
        self.iter_dirty().take(1).next().is_some()
    }

    /// Yields an iterator over all values that are currently dirty. As the iterator
    /// progresses, items will NOT be marked clean. This allows you to examine
    /// any currently dirty items in the cache.
    pub fn iter_dirty(&self) -> impl Iterator<Item = (&K, Option<&V>)> {
        self.tlocal
            .iter()
            .filter(|(_k, v)| match v {
                ThreadCacheItem::Present(_v, c, _size) => !c,
                ThreadCacheItem::Removed(c) => !c,
            })
            .map(|(k, v)| {
                // Get the data.
                let data = match v {
                    ThreadCacheItem::Present(v, _c, _size) => Some(v),
                    ThreadCacheItem::Removed(_c) => None,
                };
                (k, data)
            })
    }

    /// Yields a mutable iterator over all values that are currently dirty. As the iterator
    /// progresses, items will NOT be marked clean. This allows you to modify and
    /// change any currently dirty items as required.
    pub fn iter_mut_dirty(&mut self) -> impl Iterator<Item = (&K, Option<&mut V>)> {
        self.tlocal
            .iter_mut()
            .filter(|(_k, v)| match v {
                ThreadCacheItem::Present(_v, c, _size) => !c,
                ThreadCacheItem::Removed(c) => !c,
            })
            .map(|(k, v)| {
                // Get the data.
                let data = match v {
                    ThreadCacheItem::Present(v, _c, _size) => Some(v),
                    ThreadCacheItem::Removed(_c) => None,
                };
                (k, data)
            })
    }

    /// Yields an iterator over all values that are currently dirty. As the iterator
    /// progresses, items will be marked clean. This is where you should sync dirty
    /// cache content to your associated store. The iterator is `K, Option<V>`, where
    /// the `Option<V>` indicates if the item has been remove (None) or is updated (Some).
    pub fn iter_mut_mark_clean(&mut self) -> impl Iterator<Item = (&K, Option<&mut V>)> {
        self.tlocal
            .iter_mut()
            .filter(|(_k, v)| match v {
                ThreadCacheItem::Present(_v, c, _size) => !c,
                ThreadCacheItem::Removed(c) => !c,
            })
            .map(|(k, v)| {
                // Mark it clean.
                match v {
                    ThreadCacheItem::Present(_v, c, _size) => *c = true,
                    ThreadCacheItem::Removed(c) => *c = true,
                }
                // Get the data.
                let data = match v {
                    ThreadCacheItem::Present(v, _c, _size) => Some(v),
                    ThreadCacheItem::Removed(_c) => None,
                };
                (k, data)
            })
    }

    /// Yield an iterator over all currently live and valid cache items.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.cache.values().filter_map(|ci| match &ci {
            CacheItem::Rec(lln, v) => {
                let cii = lln.as_ref();
                Some((&cii.k, v))
            }
            CacheItem::Freq(lln, v) => {
                let cii = lln.as_ref();
                Some((&cii.k, v))
            }
            _ => None,
        })
    }

    /// Yield an iterator over all currently live and valid items in the
    /// recent access list.
    pub fn iter_rec(&self) -> impl Iterator<Item = &K> {
        self.cache.values().filter_map(|ci| match &ci {
            CacheItem::Rec(lln, _) => {
                let cii = lln.as_ref();
                Some(&cii.k)
            }
            _ => None,
        })
    }

    /// Yield an iterator over all currently live and valid items in the
    /// frequent access list.
    pub fn iter_freq(&self) -> impl Iterator<Item = &K> {
        self.cache.values().filter_map(|ci| match &ci {
            CacheItem::Rec(lln, _) => {
                let cii = lln.as_ref();
                Some(&cii.k)
            }
            _ => None,
        })
    }

    #[cfg(test)]
    pub(crate) fn iter_ghost_rec(&self) -> impl Iterator<Item = &K> {
        self.cache.values().filter_map(|ci| match &ci {
            CacheItem::GhostRec(lln) => {
                let cii = lln.as_ref();
                Some(&cii.k)
            }
            _ => None,
        })
    }

    #[cfg(test)]
    pub(crate) fn iter_ghost_freq(&self) -> impl Iterator<Item = &K> {
        self.cache.values().filter_map(|ci| match &ci {
            CacheItem::GhostFreq(lln) => {
                let cii = lln.as_ref();
                Some(&cii.k)
            }
            _ => None,
        })
    }

    #[cfg(test)]
    pub(crate) fn peek_hit(&self) -> &[u64] {
        let hit_ptr = self.hit.get();
        unsafe { &(*hit_ptr) }
    }

    #[cfg(test)]
    pub(crate) fn peek_cache<Q: ?Sized + Hash + Eq + Ord>(&self, k: &Q) -> CacheState
    where
        K: Borrow<Q>,
    {
        if let Some(v) = self.cache.get(k) {
            (*v).to_state()
        } else {
            CacheState::None
        }
    }

    #[cfg(test)]
    pub(crate) fn peek_stat(&self) -> CStat {
        let inner = self.caller.inner.lock().unwrap();
        let shared = self.caller.shared.read().unwrap();
        CStat {
            max: shared.max,
            cache: self.cache.len(),
            tlocal: self.tlocal.len(),
            freq: inner.freq.len(),
            rec: inner.rec.len(),
            ghost_freq: inner.ghost_freq.len(),
            ghost_rec: inner.ghost_rec.len(),
            haunted: inner.haunted.len(),
            p: inner.p,
        }
    }

    // to_snapshot
}

impl<
        K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
        V: Clone + Debug + Sync + Send + 'static,
        S: ARCacheReadStat + Clone,
    > ARCacheReadTxn<'_, K, V, S>
{
    /// Attempt to retrieve a k-v pair from the cache. If it is present in the main cache OR
    /// the thread local cache, a `Some` is returned, else you will receive a `None`. On a
    /// `None`, you must then consult the external data source that this structure is acting
    /// as a cache for.
    pub fn get<Q>(&mut self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + Ord + ?Sized,
    {
        let k_hash: u64 = self.cache.prehash(k);

        self.stats.cache_read();
        // self.ops += 1;

        let mut hits = false;
        let mut tlocal_hits = false;

        let r: Option<&V> = self
            .tlocal
            .as_ref()
            .and_then(|cache| {
                cache.set.get(k).map(|v| {
                    // Indicate a hit on the tlocal cache.
                    tlocal_hits = true;

                    if self.above_watermark {
                        let _ = self.hit_queue.push(CacheHitEvent {
                            t: Instant::now(),
                            k_hash,
                        });
                    }
                    unsafe {
                        let v = &v.as_ref().v as *const _;
                        // This discards the lifetime and repins it to the lifetime of `self`.
                        &(*v)
                    }
                })
            })
            .or_else(|| {
                self.cache.get_prehashed(k, k_hash).and_then(|v| {
                    (*v).to_vref().map(|vin| {
                        // Indicate a hit on the main cache.
                        hits = true;

                        if self.above_watermark {
                            let _ = self.hit_queue.push(CacheHitEvent {
                                t: Instant::now(),
                                k_hash,
                            });
                        }

                        unsafe {
                            let vin = vin as *const _;
                            &(*vin)
                        }
                    })
                })
            });

        if tlocal_hits {
            self.stats.cache_local_hit()
        } else if hits {
            self.stats.cache_main_hit()
        };

        r
    }

    /// Determine if this cache contains the following key.
    pub fn contains_key<Q>(&mut self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + Ord + ?Sized,
    {
        self.get(k).is_some()
    }

    /// Insert an item to the cache, with an associated weight/size factor. See also `insert`
    pub fn insert_sized(&mut self, k: K, v: V, size: NonZeroUsize) {
        let size = size.get();
        // Send a copy forward through time and space.
        // let _ = self.tx.try_send(
        if self
            .inc_queue
            .push(CacheIncludeEvent {
                t: Instant::now(),
                k: k.clone(),
                v: v.clone(),
                txid: self.cache.get_txid(),
                size,
            })
            .is_ok()
        {
            self.stats.include();
        } else {
            self.stats.failed_include();
        }

        // We have a cache, so lets update it.
        if let Some(ref mut cache) = self.tlocal {
            self.stats.local_include();
            while cache.tlru.len() >= cache.read_size {
                if let Some(owned_inner) = cache.tlru.pop_n_free() {
                    let existing = cache.set.remove(&owned_inner.k);
                    // Must have been present.
                    assert!(
                        existing.is_some(),
                        "Impossible state! Key was NOT present in cache!"
                    );
                } else {
                    // Somehow the list is empty, but we still are oversize?
                    debug_assert!(false);
                    break;
                }
            }

            // Now add it, as we have enough space.
            let n = cache.tlru.append_k(ReadCacheItem {
                k: k.clone(),
                v,
                size,
            });
            let r = cache.set.insert(k, n);
            // There should never be a previous value.
            assert!(r.is_none());
        }
    }

    /// Add a value to the cache. This may be because you have had a cache miss and
    /// now wish to include in the thread local storage.
    ///
    /// Note that is invalid to insert an item who's key already exists in this thread local cache,
    /// and this is asserted IE will panic if you attempt this. It is also invalid for you to insert
    /// a value that does not match the source-of-truth state, IE inserting a different
    /// value than another thread may perceive. This is a *read* thread, so you should only be adding
    /// values that are relevant to this read transaction and this point in time. If you do not
    /// heed this warning, you may alter the fabric of time and space and have some interesting
    /// distortions in your data over time.
    pub fn insert(&mut self, k: K, v: V) {
        self.insert_sized(k, v, unsafe { NonZeroUsize::new_unchecked(1) })
    }

    /// _
    pub fn finish(self) -> S {
        let stats = self.stats.clone();
        drop(self);

        stats
    }
}

impl<
        K: Hash + Eq + Ord + Clone + Debug + Sync + Send + 'static,
        V: Clone + Debug + Sync + Send + 'static,
        S: ARCacheReadStat + Clone,
    > Drop for ARCacheReadTxn<'_, K, V, S>
{
    fn drop(&mut self) {
        // We could make this check the queue sizes rather than blindly quiescing
        if self.reader_quiesce {
            self.caller.try_quiesce();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::stats::{TraceStat, WriteCountStat};
    use super::ARCache;
    use super::ARCacheBuilder;
    use super::CStat;
    use super::CacheState;
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::thread;

    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_cache_arc_basic() {
        let arc: ARCache<usize, usize> = ARCacheBuilder::new()
            .set_size(4, 4)
            .build()
            .expect("Invalid cache parameters!");
        let mut wr_txn = arc.write();

        assert!(wr_txn.get(&1).is_none());
        assert!(wr_txn.peek_hit().is_empty());
        wr_txn.insert(1, 1);
        assert!(wr_txn.get(&1) == Some(&1));
        assert!(wr_txn.peek_hit().len() == 1);

        wr_txn.commit();

        // Now we start the second txn, and see if it's in there.
        let mut wr_txn = arc.write();
        assert!(wr_txn.peek_cache(&1) == CacheState::Rec);
        assert!(wr_txn.get(&1) == Some(&1));
        assert!(wr_txn.peek_hit().len() == 1);
        wr_txn.commit();
        // And now check it's moved to Freq due to the extra
        let wr_txn = arc.write();
        assert!(wr_txn.peek_cache(&1) == CacheState::Freq);
        println!("{:?}", wr_txn.peek_stat());
    }

    #[test]
    fn test_cache_evict() {
        let _ = tracing_subscriber::fmt::try_init();
        println!("== 1");
        let arc: ARCache<usize, usize> = ARCacheBuilder::default()
            .set_size(4, 4)
            .build()
            .expect("Invalid cache parameters!");
        let stats = TraceStat {};

        let mut wr_txn = arc.write_stats(stats);
        assert!(
            CStat {
                max: 4,
                cache: 0,
                tlocal: 0,
                freq: 0,
                rec: 0,
                ghost_freq: 0,
                ghost_rec: 0,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );

        // In the first txn we insert 4 items.
        wr_txn.insert(1, 1);
        wr_txn.insert(2, 2);
        wr_txn.insert(3, 3);
        wr_txn.insert(4, 4);

        assert!(
            CStat {
                max: 4,
                cache: 0,
                tlocal: 4,
                freq: 0,
                rec: 0,
                ghost_freq: 0,
                ghost_rec: 0,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );
        let stats = wr_txn.commit();

        // Now we start the second txn, and check the stats.
        println!("== 2");
        let mut wr_txn = arc.write_stats(stats);
        assert!(
            CStat {
                max: 4,
                cache: 4,
                tlocal: 0,
                freq: 0,
                rec: 4,
                ghost_freq: 0,
                ghost_rec: 0,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );

        // Now touch two items, this promote to the freq set.
        // Remember, a double hit doesn't weight any more than 1 hit.
        assert!(wr_txn.get(&1) == Some(&1));
        assert!(wr_txn.get(&1) == Some(&1));
        assert!(wr_txn.get(&2) == Some(&2));

        let stats = wr_txn.commit();

        // Now we start the third txn, and check the stats.
        println!("== 3");
        let mut wr_txn = arc.write_stats(stats);
        assert!(
            CStat {
                max: 4,
                cache: 4,
                tlocal: 0,
                freq: 2,
                rec: 2,
                ghost_freq: 0,
                ghost_rec: 0,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );
        // Add one more item, this will trigger an evict.
        wr_txn.insert(5, 5);
        let stats = wr_txn.commit();

        // Now we start the fourth txn, and check the stats.
        println!("== 4");
        let mut wr_txn = arc.write_stats(stats);
        println!("stat -> {:?}", wr_txn.peek_stat());
        assert!(
            CStat {
                max: 4,
                cache: 5,
                tlocal: 0,
                freq: 2,
                rec: 2,
                ghost_freq: 0,
                ghost_rec: 1,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );
        // And assert what's in the sets to be sure of what went where.
        // 🚨 Can no longer peek these with hashmap backing as the keys may
        // be evicted out-of-order, but the stats are correct!

        // Now touch the two recent items to bring them also to freq

        let rec_set: Vec<usize> = wr_txn.iter_rec().take(2).copied().collect();
        assert!(wr_txn.get(&rec_set[0]) == Some(&rec_set[0]));
        assert!(wr_txn.get(&rec_set[1]) == Some(&rec_set[1]));

        let stats = wr_txn.commit();

        // Now we start the fifth txn, and check the stats.
        println!("== 5");
        let mut wr_txn = arc.write_stats(stats);
        println!("stat -> {:?}", wr_txn.peek_stat());
        assert!(
            CStat {
                max: 4,
                cache: 5,
                tlocal: 0,
                freq: 4,
                rec: 0,
                ghost_freq: 0,
                ghost_rec: 1,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );
        // And assert what's in the sets to be sure of what went where.
        // 🚨 Can no longer peek these with hashmap backing as the keys may
        // be evicted out-of-order, but the stats are correct!

        // Now touch the one item that's in ghost rec - this will trigger
        // an evict from ghost freq
        let grec: usize = wr_txn.iter_ghost_rec().take(1).copied().next().unwrap();
        wr_txn.insert(grec, grec);
        assert!(wr_txn.get(&grec) == Some(&grec));
        // When we add 3, we are basically issuing a demand that the rec set should be
        // allowed to grow as we had a potential cache miss here.
        let stats = wr_txn.commit();

        // Now we start the sixth txn, and check the stats.
        println!("== 6");
        let mut wr_txn = arc.write_stats(stats);
        println!("stat -> {:?}", wr_txn.peek_stat());
        assert!(
            CStat {
                max: 4,
                cache: 5,
                tlocal: 0,
                freq: 3,
                rec: 1,
                ghost_freq: 1,
                ghost_rec: 0,
                haunted: 0,
                p: 1
            } == wr_txn.peek_stat()
        );
        // And assert what's in the sets to be sure of what went where.
        // 🚨 Can no longer peek these with hashmap backing as the keys may
        // be evicted out-of-order, but the stats are correct!
        assert!(wr_txn.peek_cache(&grec) == CacheState::Rec);

        // Right, seventh txn - we show how a cache scan doesn't cause p shifting or evict.
        // tl;dr - attempt to include a bunch in a scan, and it will be ignored as p is low,
        // and any miss on rec won't shift p unless it's in the ghost rec.
        wr_txn.insert(10, 10);
        wr_txn.insert(11, 11);
        wr_txn.insert(12, 12);
        let stats = wr_txn.commit();

        println!("== 7");
        let mut wr_txn = arc.write_stats(stats);
        println!("stat -> {:?}", wr_txn.peek_stat());
        assert!(
            CStat {
                max: 4,
                cache: 8,
                tlocal: 0,
                freq: 3,
                rec: 1,
                ghost_freq: 1,
                ghost_rec: 3,
                haunted: 0,
                p: 1
            } == wr_txn.peek_stat()
        );
        // 🚨 Can no longer peek these with hashmap backing as the keys may
        // be evicted out-of-order, but the stats are correct!

        // Eight txn - now that we had a demand for items before, we re-demand them - this will trigger
        // a shift in p, causing some more to be in the rec cache.
        let grec_set: Vec<usize> = wr_txn.iter_ghost_rec().take(3).copied().collect();
        println!("{:?}", grec_set);

        grec_set
            .iter()
            .for_each(|i| println!("{:?}", wr_txn.peek_cache(i)));

        grec_set.iter().for_each(|i| wr_txn.insert(*i, *i));

        grec_set
            .iter()
            .for_each(|i| println!("{:?}", wr_txn.peek_cache(i)));
        wr_txn.commit();

        println!("== 8");
        let mut wr_txn = arc.write();
        println!("stat -> {:?}", wr_txn.peek_stat());
        assert!(
            CStat {
                max: 4,
                cache: 8,
                tlocal: 0,
                freq: 0,
                rec: 4,
                ghost_freq: 4,
                ghost_rec: 0,
                haunted: 0,
                p: 4
            } == wr_txn.peek_stat()
        );

        grec_set
            .iter()
            .for_each(|i| println!("{:?}", wr_txn.peek_cache(i)));
        grec_set
            .iter()
            .for_each(|i| assert!(wr_txn.peek_cache(i) == CacheState::Rec));

        // Now lets go back the other way - we want freq items to come back.
        let gfreq_set: Vec<usize> = wr_txn.iter_ghost_freq().take(4).copied().collect();

        gfreq_set.iter().for_each(|i| wr_txn.insert(*i, *i));
        wr_txn.commit();

        println!("== 9");
        let wr_txn = arc.write();
        println!("stat -> {:?}", wr_txn.peek_stat());
        assert!(
            CStat {
                max: 4,
                cache: 8,
                tlocal: 0,
                freq: 4,
                rec: 0,
                ghost_freq: 0,
                ghost_rec: 4,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );
        // 🚨 Can no longer peek these with hashmap backing as the keys may
        // be evicted out-of-order, but the stats are correct!
        gfreq_set
            .iter()
            .for_each(|i| assert!(wr_txn.peek_cache(i) == CacheState::Freq));

        // And done!
        let () = wr_txn.commit();
        // See what stats did
        // let stats = arc.view_stats();
        // println!("{:?}", *stats);
    }

    #[test]
    fn test_cache_concurrent_basic() {
        // Now we want to check some basic interactions of read and write together.
        // Setup the cache.
        let arc: ARCache<usize, usize> = ARCacheBuilder::default()
            .set_size(4, 4)
            .build()
            .expect("Invalid cache parameters!");
        // start a rd
        {
            let mut rd_txn = arc.read();
            // add items to the rd
            rd_txn.insert(1, 1);
            rd_txn.insert(2, 2);
            rd_txn.insert(3, 3);
            rd_txn.insert(4, 4);
            // Should be in the tlocal
            // assert!(rd_txn.get(&1).is_some());
            // assert!(rd_txn.get(&2).is_some());
            // assert!(rd_txn.get(&3).is_some());
            // assert!(rd_txn.get(&4).is_some());
            // end the rd
        }
        arc.try_quiesce();
        // What state is the cache now in?
        println!("== 2");
        let wr_txn = arc.write();
        println!("{:?}", wr_txn.peek_stat());
        assert!(
            CStat {
                max: 4,
                cache: 4,
                tlocal: 0,
                freq: 0,
                rec: 4,
                ghost_freq: 0,
                ghost_rec: 0,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );
        assert!(wr_txn.peek_cache(&1) == CacheState::Rec);
        assert!(wr_txn.peek_cache(&2) == CacheState::Rec);
        assert!(wr_txn.peek_cache(&3) == CacheState::Rec);
        assert!(wr_txn.peek_cache(&4) == CacheState::Rec);
        // Magic! Without a single write op we included items!
        // Lets have the read touch two items, and then add two new.
        // This will trigger evict on 1/2
        {
            let mut rd_txn = arc.read();
            // add items to the rd
            assert!(rd_txn.get(&3) == Some(&3));
            assert!(rd_txn.get(&4) == Some(&4));
            rd_txn.insert(5, 5);
            rd_txn.insert(6, 6);
            // end the rd
        }
        // Now commit and check the state.
        wr_txn.commit();
        println!("== 3");
        let wr_txn = arc.write();
        assert!(
            CStat {
                max: 4,
                cache: 6,
                tlocal: 0,
                freq: 2,
                rec: 2,
                ghost_freq: 0,
                ghost_rec: 2,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );
        assert!(wr_txn.peek_cache(&1) == CacheState::GhostRec);
        assert!(wr_txn.peek_cache(&2) == CacheState::GhostRec);
        assert!(wr_txn.peek_cache(&3) == CacheState::Freq);
        assert!(wr_txn.peek_cache(&4) == CacheState::Freq);
        assert!(wr_txn.peek_cache(&5) == CacheState::Rec);
        assert!(wr_txn.peek_cache(&6) == CacheState::Rec);

        // Now trigger hits on 1/2 which will cause a shift in P.
        {
            let mut rd_txn = arc.read();
            // add items to the rd
            rd_txn.insert(1, 1);
            rd_txn.insert(2, 2);
            // end the rd
        }

        wr_txn.commit();
        println!("== 4");
        let wr_txn = arc.write();
        assert!(
            CStat {
                max: 4,
                cache: 6,
                tlocal: 0,
                freq: 2,
                rec: 2,
                ghost_freq: 0,
                ghost_rec: 2,
                haunted: 0,
                p: 2
            } == wr_txn.peek_stat()
        );
        assert!(wr_txn.peek_cache(&1) == CacheState::Rec);
        assert!(wr_txn.peek_cache(&2) == CacheState::Rec);
        assert!(wr_txn.peek_cache(&3) == CacheState::Freq);
        assert!(wr_txn.peek_cache(&4) == CacheState::Freq);
        assert!(wr_txn.peek_cache(&5) == CacheState::GhostRec);
        assert!(wr_txn.peek_cache(&6) == CacheState::GhostRec);
        // See what stats did
        // let stats = arc.view_stats();
        // println!("stats 1: {:?}", *stats);
        // assert!(stats.reader_hits == 2);
        // assert!(stats.reader_includes == 8);
        // assert!(stats.reader_tlocal_includes == 8);
        // assert!(stats.reader_tlocal_hits == 0);
    }

    // Test edge cases that are horrifying and could destroy peoples lives
    // and sanity.
    #[test]
    fn test_cache_concurrent_cursed_1() {
        // Case 1 - It's possible for a read transaction to last for a long time,
        // and then have a cache include, which may cause an attempt to include
        // an outdated value into the cache. To handle this the haunted set exists
        // so that all keys and their eviction ids are always tracked for all of time
        // to ensure that we never incorrectly include a value that may have been updated
        // more recently.
        let arc: ARCache<usize, usize> = ARCacheBuilder::default()
            .set_size(4, 4)
            .build()
            .expect("Invalid cache parameters!");

        // Start a wr
        let mut wr_txn = arc.write();
        // Start a rd
        let mut rd_txn = arc.read();
        // Add the value 1,1 via the wr.
        wr_txn.insert(1, 1);

        // assert 1 is not in rd.
        assert!(rd_txn.get(&1).is_none());

        // Commit wr
        wr_txn.commit();
        // Even after the commit, it's not in rd.
        assert!(rd_txn.get(&1).is_none());
        // begin wr
        let mut wr_txn = arc.write();
        // We now need to flood the cache, to cause ghost rec eviction.
        wr_txn.insert(10, 1);
        wr_txn.insert(11, 1);
        wr_txn.insert(12, 1);
        wr_txn.insert(13, 1);
        wr_txn.insert(14, 1);
        wr_txn.insert(15, 1);
        wr_txn.insert(16, 1);
        wr_txn.insert(17, 1);
        // commit wr
        wr_txn.commit();

        // begin wr
        let wr_txn = arc.write();
        // assert that 1 is haunted.
        assert!(wr_txn.peek_cache(&1) == CacheState::Haunted);
        // assert 1 is not in rd.
        assert!(rd_txn.get(&1).is_none());
        // now that 1 is hanuted, in rd attempt to insert 1, X
        rd_txn.insert(1, 100);
        // commit wr
        wr_txn.commit();

        // start wr
        let wr_txn = arc.write();
        // assert that 1 is still haunted.
        assert!(wr_txn.peek_cache(&1) == CacheState::Haunted);
        // assert that 1, x is in rd.
        assert!(rd_txn.get(&1) == Some(&100));
        // done!
    }

    #[test]
    fn test_cache_clear() {
        let arc: ARCache<usize, usize> = ARCacheBuilder::default()
            .set_size(4, 4)
            .build()
            .expect("Invalid cache parameters!");

        // Start a wr
        let mut wr_txn = arc.write();
        // Add a bunch of values, and touch some twice.
        wr_txn.insert(10, 10);
        wr_txn.insert(11, 11);
        wr_txn.insert(12, 12);
        wr_txn.insert(13, 13);
        wr_txn.insert(14, 14);
        wr_txn.insert(15, 15);
        wr_txn.insert(16, 16);
        wr_txn.insert(17, 17);
        wr_txn.commit();
        // Begin a new write.
        let mut wr_txn = arc.write();

        // Touch two values that are in the rec set.
        let rec_set: Vec<usize> = wr_txn.iter_rec().take(2).copied().collect();
        println!("{:?}", rec_set);
        assert!(wr_txn.get(&rec_set[0]) == Some(&rec_set[0]));
        assert!(wr_txn.get(&rec_set[1]) == Some(&rec_set[1]));

        // commit wr
        wr_txn.commit();
        // Begin a new write.
        let mut wr_txn = arc.write();
        println!("stat -> {:?}", wr_txn.peek_stat());
        assert!(
            CStat {
                max: 4,
                cache: 8,
                tlocal: 0,
                freq: 2,
                rec: 2,
                ghost_freq: 0,
                ghost_rec: 4,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );

        // Clear
        wr_txn.clear();
        // Now commit
        wr_txn.commit();
        // Now check their states.
        let wr_txn = arc.write();
        // See what stats did
        println!("stat -> {:?}", wr_txn.peek_stat());
        // stat -> CStat { max: 4, cache: 8, tlocal: 0, freq: 0, rec: 0, ghost_freq: 2, ghost_rec: 6, haunted: 0, p: 0 }
        assert!(
            CStat {
                max: 4,
                cache: 8,
                tlocal: 0,
                freq: 0,
                rec: 0,
                ghost_freq: 2,
                ghost_rec: 6,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );
        // let stats = arc.view_stats();
        // println!("{:?}", *stats);
    }

    #[test]
    fn test_cache_clear_rollback() {
        let arc: ARCache<usize, usize> = ARCacheBuilder::default()
            .set_size(4, 4)
            .build()
            .expect("Invalid cache parameters!");

        // Start a wr
        let mut wr_txn = arc.write();
        // Add a bunch of values, and touch some twice.
        wr_txn.insert(10, 10);
        wr_txn.insert(11, 11);
        wr_txn.insert(12, 12);
        wr_txn.insert(13, 13);
        wr_txn.insert(14, 14);
        wr_txn.insert(15, 15);
        wr_txn.insert(16, 16);
        wr_txn.insert(17, 17);
        wr_txn.commit();
        // Begin a new write.
        let mut wr_txn = arc.write();
        let rec_set: Vec<usize> = wr_txn.iter_rec().take(2).copied().collect();
        println!("{:?}", rec_set);
        let r = wr_txn.get(&rec_set[0]);
        println!("{:?}", r);
        assert!(r == Some(&rec_set[0]));
        assert!(wr_txn.get(&rec_set[1]) == Some(&rec_set[1]));

        // commit wr
        wr_txn.commit();
        // Begin a new write.
        let mut wr_txn = arc.write();
        println!("stat -> {:?}", wr_txn.peek_stat());
        assert!(
            CStat {
                max: 4,
                cache: 8,
                tlocal: 0,
                freq: 2,
                rec: 2,
                ghost_freq: 0,
                ghost_rec: 4,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );

        // Clear
        wr_txn.clear();
        // Now abort the clear - should do nothing!
        drop(wr_txn);
        // Check the states, should not have changed
        let wr_txn = arc.write();
        println!("stat -> {:?}", wr_txn.peek_stat());
        assert!(
            CStat {
                max: 4,
                cache: 8,
                tlocal: 0,
                freq: 2,
                rec: 2,
                ghost_freq: 0,
                ghost_rec: 4,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );
    }

    #[test]
    fn test_cache_clear_cursed() {
        let arc: ARCache<usize, usize> = ARCacheBuilder::default()
            .set_size(4, 4)
            .build()
            .expect("Invalid cache parameters!");
        // Setup for the test
        // --
        let mut wr_txn = arc.write();
        wr_txn.insert(10, 1);
        wr_txn.commit();
        // --
        let wr_txn = arc.write();
        assert!(wr_txn.peek_cache(&10) == CacheState::Rec);
        wr_txn.commit();
        // --
        // Okay, now the test starts. First, we begin a read
        let mut rd_txn = arc.read();
        // Then while that read exists, we open a write, and conduct
        // a cache clear.
        let mut wr_txn = arc.write();
        wr_txn.clear();
        // Commit the clear write.
        wr_txn.commit();

        // Now on the read, we perform a touch of an item, and we include
        // something that was not yet in the cache.
        assert!(rd_txn.get(&10) == Some(&1));
        rd_txn.insert(11, 1);
        // Complete the read
        std::mem::drop(rd_txn);
        // Perform a cache quiesce
        arc.try_quiesce();
        // --

        // Assert that the items that we provided were NOT included, and are
        // in the correct states.
        let wr_txn = arc.write();
        assert!(wr_txn.peek_cache(&10) == CacheState::GhostRec);
        println!("--> {:?}", wr_txn.peek_cache(&11));
        assert!(wr_txn.peek_cache(&11) == CacheState::None);
    }

    #[test]
    fn test_cache_p_weight_zero_churn() {
        let arc: ARCache<usize, usize> = ARCacheBuilder::new()
            .set_size(4, 4)
            .set_watermark(0)
            .build()
            .expect("Invalid cache parameters!");

        let mut wr_txn = arc.write();

        // First we need to load up frequent.
        wr_txn.insert(1, 1);
        wr_txn.insert(2, 2);
        wr_txn.insert(3, 3);
        wr_txn.insert(4, 4);
        assert_eq!(wr_txn.get(&1), Some(&1));
        assert_eq!(wr_txn.get(&2), Some(&2));
        assert_eq!(wr_txn.get(&3), Some(&3));
        assert_eq!(wr_txn.get(&4), Some(&4));

        assert_eq!(wr_txn.peek_stat().p, 0);
        wr_txn.commit();

        // Hitting again in a new txn moves to freq
        let mut wr_txn = arc.write();
        assert_eq!(wr_txn.get(&1), Some(&1));
        assert_eq!(wr_txn.get(&2), Some(&2));
        assert_eq!(wr_txn.get(&3), Some(&3));
        assert_eq!(wr_txn.get(&4), Some(&4));

        assert_eq!(wr_txn.peek_stat().p, 0);
        wr_txn.commit();

        // Now include new items. The goal is we want to shift p to at least 1.
        let mut wr_txn = arc.write();
        // Won't fit, moves to ghost recent
        wr_txn.insert(100, 100);
        println!("b {:?}", wr_txn.peek_stat());
        assert_eq!(wr_txn.peek_stat().p, 0);
        wr_txn.commit();

        // Include again, causes evict in freq.
        let mut wr_txn = arc.write();
        assert_eq!(wr_txn.peek_stat().p, 0);
        assert_eq!(wr_txn.peek_stat().ghost_rec, 1);

        // Causes shift in P, stays in recent.
        wr_txn.insert(100, 100);
        wr_txn.commit();

        let wr_txn = arc.write();
        println!("c {:?}", wr_txn.peek_stat());
        assert_eq!(wr_txn.peek_stat().p, 1);
        assert_eq!(wr_txn.peek_stat().ghost_rec, 0);
        assert_eq!(wr_txn.peek_stat().ghost_freq, 1);
        wr_txn.commit();

        // Now, we want to bring back from ghost freq.
        let mut wr_txn = arc.write();
        assert_eq!(wr_txn.get(&1), None);
        // We missed, so re-include.
        wr_txn.insert(1, 1);
        wr_txn.commit();

        let wr_txn = arc.write();
        println!("d {:?}", wr_txn.peek_stat());
        // Weights P back to 0.
        assert_eq!(wr_txn.peek_stat().p, 0);
        assert_eq!(wr_txn.peek_stat().ghost_rec, 1);
        assert_eq!(wr_txn.peek_stat().ghost_freq, 0);
        wr_txn.commit();
    }

    #[test]
    fn test_cache_haunted_bounds() {
        let arc: ARCache<usize, usize> = ARCacheBuilder::new()
            .set_size(4, 4)
            .set_watermark(0)
            .set_look_back_limit(4)
            .build()
            .expect("Invalid cache parameters!");

        let mut wr_txn = arc.write();

        // We want to test that haunted items are removed after 4 generations.

        // Setup a removed key which immediately goes to haunted.
        wr_txn.remove(1);
        wr_txn.commit();

        // Now write and commit 4 times to advance the txid.
        for _i in 0..5 {
            let wr_txn = arc.write();
            println!("l {:?}", wr_txn.peek_stat());
            assert_eq!(wr_txn.peek_stat().haunted, 1);
            assert!(wr_txn.peek_cache(&1) == CacheState::Haunted);
            wr_txn.commit();
        }

        // Now it's removed.
        let wr_txn = arc.write();
        println!("d {:?}", wr_txn.peek_stat());
        assert_eq!(wr_txn.peek_stat().haunted, 0);
        assert_eq!(wr_txn.peek_cache(&1), CacheState::None);
        wr_txn.commit();
    }

    #[test]
    fn test_cache_dirty_write() {
        let arc: ARCache<usize, usize> = ARCacheBuilder::default()
            .set_size(4, 4)
            .build()
            .expect("Invalid cache parameters!");
        let mut wr_txn = arc.write();
        wr_txn.insert_dirty(10, 1);
        wr_txn.iter_mut_mark_clean().for_each(|(_k, _v)| {});
        wr_txn.commit();
    }

    #[test]
    fn test_cache_read_no_tlocal() {
        // Check a cache with no read local thread capacity
        // Setup the cache.
        let arc: ARCache<usize, usize> = ARCacheBuilder::default()
            .set_size(4, 0)
            .build()
            .expect("Invalid cache parameters!");
        // start a rd
        {
            let mut rd_txn = arc.read();
            // add items to the rd
            rd_txn.insert(1, 1);
            rd_txn.insert(2, 2);
            rd_txn.insert(3, 3);
            rd_txn.insert(4, 4);
            // end the rd
            // Everything should be missing frm the tlocal.
            assert!(rd_txn.get(&1).is_none());
            assert!(rd_txn.get(&2).is_none());
            assert!(rd_txn.get(&3).is_none());
            assert!(rd_txn.get(&4).is_none());
        }
        arc.try_quiesce();
        // What state is the cache now in?
        println!("== 2");
        let wr_txn = arc.write();
        assert!(
            CStat {
                max: 4,
                cache: 4,
                tlocal: 0,
                freq: 0,
                rec: 4,
                ghost_freq: 0,
                ghost_rec: 0,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );
        assert!(wr_txn.peek_cache(&1) == CacheState::Rec);
        assert!(wr_txn.peek_cache(&2) == CacheState::Rec);
        assert!(wr_txn.peek_cache(&3) == CacheState::Rec);
        assert!(wr_txn.peek_cache(&4) == CacheState::Rec);
        // let stats = arc.view_stats();
        // println!("stats 1: {:?}", *stats);
        // assert!(stats.reader_includes == 4);
        // assert!(stats.reader_tlocal_includes == 0);
        // assert!(stats.reader_tlocal_hits == 0);
    }

    #[derive(Clone, Debug)]
    struct Weighted {
        _i: u64,
    }

    #[test]
    fn test_cache_weighted() {
        let arc: ARCache<usize, Weighted> = ARCacheBuilder::default()
            .set_size(4, 0)
            .build()
            .expect("Invalid cache parameters!");
        let mut wr_txn = arc.write();

        assert!(
            CStat {
                max: 4,
                cache: 0,
                tlocal: 0,
                freq: 0,
                rec: 0,
                ghost_freq: 0,
                ghost_rec: 0,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );

        // In the first txn we insert 2 weight 2 items.
        wr_txn.insert_sized(1, Weighted { _i: 1 }, NonZeroUsize::new(2).unwrap());
        wr_txn.insert_sized(2, Weighted { _i: 2 }, NonZeroUsize::new(2).unwrap());

        assert!(
            CStat {
                max: 4,
                cache: 0,
                tlocal: 2,
                freq: 0,
                rec: 0,
                ghost_freq: 0,
                ghost_rec: 0,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );
        wr_txn.commit();

        // Now once committed, the proper sizes kick in.

        let wr_txn = arc.write();
        assert!(
            CStat {
                max: 4,
                cache: 2,
                tlocal: 0,
                freq: 0,
                rec: 4,
                ghost_freq: 0,
                ghost_rec: 0,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );
        wr_txn.commit();

        // Check the numbers move properly.
        let mut wr_txn = arc.write();
        wr_txn.get(&1);
        wr_txn.commit();

        let mut wr_txn = arc.write();
        assert!(
            CStat {
                max: 4,
                cache: 2,
                tlocal: 0,
                freq: 2,
                rec: 2,
                ghost_freq: 0,
                ghost_rec: 0,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );

        wr_txn.insert_sized(3, Weighted { _i: 3 }, NonZeroUsize::new(2).unwrap());
        wr_txn.insert_sized(4, Weighted { _i: 4 }, NonZeroUsize::new(2).unwrap());
        wr_txn.commit();

        // Check the evicts
        let wr_txn = arc.write();
        assert!(
            CStat {
                max: 4,
                cache: 4,
                tlocal: 0,
                freq: 2,
                rec: 2,
                ghost_freq: 0,
                ghost_rec: 4,
                haunted: 0,
                p: 0
            } == wr_txn.peek_stat()
        );
        wr_txn.commit();
    }

    #[test]
    fn test_cache_stats_reload() {
        let _ = tracing_subscriber::fmt::try_init();

        // Make a cache
        let arc: ARCache<usize, usize> = ARCacheBuilder::default()
            .set_size(4, 0)
            .build()
            .expect("Invalid cache parameters!");

        let stats = WriteCountStat::default();

        let mut wr_txn = arc.write_stats(stats);
        wr_txn.insert(1, 1);
        let stats = wr_txn.commit();

        tracing::trace!("stats 1: {:?}", stats);
    }

    #[test]
    fn test_cache_mut_inplace() {
        // Make a cache
        let arc: ARCache<usize, usize> = ARCacheBuilder::default()
            .set_size(4, 0)
            .build()
            .expect("Invalid cache parameters!");
        let mut wr_txn = arc.write();

        assert!(wr_txn.get_mut(&1, false).is_none());
        // It was inserted, can mutate. This is the tlocal present state.
        wr_txn.insert(1, 1);
        {
            let mref = wr_txn.get_mut(&1, false).unwrap();
            *mref = 2;
        }
        assert!(wr_txn.get_mut(&1, false) == Some(&mut 2));
        wr_txn.commit();

        // It's in the main cache, can mutate immediately and the tlocal is primed.
        let mut wr_txn = arc.write();
        {
            let mref = wr_txn.get_mut(&1, false).unwrap();
            *mref = 3;
        }
        assert!(wr_txn.get_mut(&1, false) == Some(&mut 3));
        wr_txn.commit();

        // Marked for remove, can not mut.
        let mut wr_txn = arc.write();
        wr_txn.remove(1);
        assert!(wr_txn.get_mut(&1, false).is_none());
        wr_txn.commit();
    }

    #[allow(dead_code)]
    pub static RUNNING: AtomicBool = AtomicBool::new(false);

    #[allow(dead_code)]
    pub static READ_OPERATIONS: u32 = 1024;
    #[allow(dead_code)]
    pub static WRITE_OPERATIONS: u32 = 10240;

    #[allow(dead_code)]
    pub static CACHE_SIZE: u32 = 64;
    pub static VALUE_MAX_RANGE: u32 = CACHE_SIZE * 8;

    #[cfg(test)]
    fn multi_thread_worker(arc: Arc<ARCache<Box<u32>, Box<u32>>>) {
        while RUNNING.load(Ordering::Relaxed) {
            let mut rd_txn = arc.read();

            use rand::Rng;
            let mut rng = rand::rng();

            for _i in 0..VALUE_MAX_RANGE {
                let x = rng.random_range(0..VALUE_MAX_RANGE);

                if rd_txn.get(&x).is_none() {
                    rd_txn.insert(Box::new(x), Box::new(x))
                }
            }
        }
    }

    #[allow(dead_code)]
    #[cfg_attr(miri, ignore)]
    #[cfg_attr(feature = "dhat-heap", test)]
    fn test_cache_stress_1() {
        #[cfg(feature = "dhat-heap")]
        let _profiler = dhat::Profiler::builder().trim_backtraces(None).build();

        use rand::Rng;
        let mut rng = rand::rng();

        let arc: Arc<ARCache<Box<u32>, Box<u32>>> = Arc::new(
            ARCacheBuilder::default()
                .set_size(CACHE_SIZE as usize, 4)
                .build()
                .expect("Invalid cache parameters!"),
        );

        // Do some writes ...
        for _i in 0..WRITE_OPERATIONS {
            let mut wr_txn = arc.write();

            let x = rng.random_range(0..VALUE_MAX_RANGE);

            if wr_txn.get(&x).is_none() {
                wr_txn.insert(Box::new(x), Box::new(x))
            }

            // Can corrupt here no issue.
            wr_txn.commit();
        }

        eprintln!("writes pass");

        let thread_count = 8;

        RUNNING.store(true, Ordering::Relaxed);

        // Now do writes and reads concurrently
        let handles: Vec<_> = (0..thread_count)
            .map(|_| {
                // Build the threads.
                let cache = arc.clone();
                thread::spawn(move || multi_thread_worker(cache))
            })
            .collect();

        std::thread::sleep(std::time::Duration::from_secs(5));

        // Everything is fine until we write.

        for _i in 0..WRITE_OPERATIONS {
            let mut wr_txn = arc.write();

            let x = rng.random_range(0..VALUE_MAX_RANGE);

            if wr_txn.get(&x).is_none() {
                wr_txn.insert(Box::new(x), Box::new(x))
            }

            wr_txn.commit();
        }

        RUNNING.store(false, Ordering::Relaxed);

        for handle in handles {
            if let Err(err) = handle.join() {
                std::panic::resume_unwind(err)
            }
        }
    }

    #[test]
    fn test_set_expected_workload_negative() {
        let _arc: Arc<ARCache<Box<u32>, Box<u32>>> = Arc::new(
            ARCacheBuilder::default()
                .set_expected_workload(256, 31, 8, 16, true)
                .build()
                .expect("Invalid cache parameters!"),
        );
    }
}
