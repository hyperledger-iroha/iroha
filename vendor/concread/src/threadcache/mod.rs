//! ThreadCache - A per-thread cache with transactional behaviour.
//!
//! This provides a per-thread cache, which uses a broadcast invalidation
//! queue to manage local content. This is similar to how a CPU cache works
//! in hardware. Generally this is best for small, distinct caches with very
//! few changes / writes.
//!
//! It's worth noting that each thread needs to frequently "read" it's cache.
//! Any idle thread will end up with invalidations building up, that can consume
//! a large volume of memory. This means you need your "readers" to have transactions
//! opened/closed periodically to ensure that invalidations are acknowledged.
//!
//! Generally you should prefer to use `ARCache` over this module unless you really require
//! the properties of this module.

use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard};

use std::fmt::Debug;
use std::hash::Hash;

use lru::LruCache;

struct Inner<K, V>
where
    K: Hash + Eq + Debug + Clone,
{
    tid: usize,
    last_inv: Option<Invalidate<K>>,
    cache: LruCache<K, V>,
}

/// An instance of a threads local cache store.
pub struct ThreadLocal<K, V>
where
    K: Hash + Eq + Debug + Clone,
{
    rx: Receiver<Invalidate<K>>,
    wrlock: Arc<Mutex<Writer<K>>>,
    inv_up_to_txid: Arc<AtomicU64>,
    inner: Mutex<Inner<K, V>>,
}

struct Writer<K>
where
    K: Hash + Eq + Debug + Clone,
{
    txs: Vec<Sender<Invalidate<K>>>,
}

/// A write transaction over this local threads cache. If you hold the write txn, no
/// other thread can be in the write state. Changes to this cache will be broadcast to
/// other threads to ensure they can revalidate their content correctly.
pub struct ThreadLocalWriteTxn<'a, K, V>
where
    K: Hash + Eq + Debug + Clone,
{
    txid: u64,
    // parent: &'a mut ThreadLocal<K, V>,
    parent: MutexGuard<'a, Inner<K, V>>,
    guard: MutexGuard<'a, Writer<K>>,
    rollback: HashSet<K>,
    inv_up_to_txid: Arc<AtomicU64>,
}

/// A read transaction of this cache. During a read, it is guaranteed that the content
/// of this cache will not be updated or invalidated unless by this threads actions.
pub struct ThreadLocalReadTxn<'a, K, V>
where
    K: Hash + Eq + Debug + Clone,
{
    // txid: u64,
    // parent: &'a mut ThreadLocal<K, V>,
    parent: MutexGuard<'a, Inner<K, V>>,
}

#[derive(Clone)]
struct Invalidate<K>
where
    K: Hash + Eq + Debug + Clone,
{
    k: K,
    txid: u64,
}

impl<K, V> ThreadLocal<K, V>
where
    K: Hash + Eq + Debug + Clone,
{
    /// Create a new set of caches. You must specify the number of threads caches to
    /// create, and the per-thread size of the cache in capacity. An array of the
    /// cache instances will be returned that you can then distribute to the threads.
    pub fn new(threads: usize, capacity: usize) -> Vec<Self> {
        assert!(threads > 0);
        let capacity = NonZeroUsize::new(capacity).unwrap();

        let (txs, rxs): (Vec<_>, Vec<_>) = (0..threads).map(|_| channel::<Invalidate<K>>()).unzip();

        // Create an Arc<Mutex<txs>> for the writer.
        let inv_up_to_txid = Arc::new(AtomicU64::new(0));
        let wrlock = Arc::new(Mutex::new(Writer { txs }));

        // Then for each thread, take one rx and a clone of the broadcast tbl.
        // Allocate a threadid (tid).

        rxs.into_iter()
            .enumerate()
            .map(|(tid, rx)| ThreadLocal {
                rx,
                wrlock: wrlock.clone(),
                inv_up_to_txid: inv_up_to_txid.clone(),
                inner: Mutex::new(Inner {
                    tid,
                    last_inv: None,
                    cache: LruCache::new(capacity),
                }),
            })
            .collect()
    }

    /// Begin a read transaction of this thread local cache. In the start of this read
    /// invalidation requests will be acknowledged.
    pub fn read(&mut self) -> ThreadLocalReadTxn<'_, K, V> {
        let txid = self.inv_up_to_txid.load(Ordering::Acquire);

        let parent = self.invalidate(txid);
        ThreadLocalReadTxn { parent }
    }

    /// Begin a write transaction of this thread local cache. Once granted, only this
    /// thread may be in the write state - all other threads will either block on
    /// acquiring the write, or they can proceed to read.
    pub fn write(&mut self) -> ThreadLocalWriteTxn<'_, K, V> {
        // SAFETY this is safe, because while we are duplicating the mutable reference
        // which conflicts with the mutex, we aren't change the wrlock value so the mutex
        // is fine.
        // let parent: &mut Self = unsafe { &mut *(self as *mut _) };
        // We are the only writer!
        let guard = self.wrlock.lock().unwrap();
        let inv_up_to_txid = self.inv_up_to_txid.clone();
        let txid = self.inv_up_to_txid.load(Ordering::Acquire);
        let txid = txid + 1;
        let parent = self.invalidate(txid);
        ThreadLocalWriteTxn {
            txid,
            parent,
            guard,
            rollback: HashSet::new(),
            inv_up_to_txid,
        }
    }
    // This allow is here because the lifetime syntax is mismatched, but correct
    #[allow(mismatched_lifetime_syntaxes)]
    fn invalidate(&self, up_to: u64) -> MutexGuard<Inner<K, V>> {
        let mut inner = self.inner.lock().unwrap();

        if let Some(inv_txid) = inner.last_inv.as_ref().map(|inv| inv.txid) {
            if inv_txid > up_to {
                // We've already invalidated past this point.
                return inner;
            } else {
                let mut inv = None;
                std::mem::swap(&mut inv, &mut inner.last_inv);
                // Must be valid due to being in a SOME loop!
                let inv = inv.unwrap();
                inner.cache.pop(&inv.k);
            }
        }

        // We acted on the stashed invalidation, so lets see if anything else needs work.

        while let Ok(inv) = self.rx.try_recv() {
            if inv.txid > up_to {
                // Stash this for next loop.
                inner.last_inv = Some(inv);
                return inner;
            } else {
                inner.cache.pop(&inv.k);
            }
        }

        inner
    }
}

impl<K, V> ThreadLocalWriteTxn<'_, K, V>
where
    K: Hash + Eq + Debug + Clone,
{
    /// Attempt to retrieve a k-v pain from the cache. If it is not present, `None` is returned.
    pub fn get(&mut self, k: &K) -> Option<&V> {
        self.parent.cache.get(k)
    }

    /// Determine if the key exists in the cache.
    pub fn contains_key(&mut self, k: &K) -> bool {
        self.parent.cache.get(k).is_some()
    }

    /// Insert a new item to this cache for this transaction.
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        // Store the k in our rollback set.
        self.rollback.insert(k.clone());
        self.parent.cache.put(k, v)
    }

    /// Remove an item from the cache for this transaction. IE you are deleting the k-v.
    pub fn remove(&mut self, k: &K) -> Option<V> {
        self.rollback.insert(k.clone());
        self.parent.cache.pop(k)
    }

    /// Commit the changes to this cache so they are visible to others. If you do NOT call
    /// commit, all changes to this cache are rolled back to prevent invalidate states.
    pub fn commit(mut self) {
        // We are committing, so let's get ready.
        // First, anything that we touched in the rollback set will need
        // to be invalidated from other caches. It doesn't matter if we
        // removed or inserted, it has the same effect on them.
        self.guard.txs.iter().enumerate().for_each(|(i, tx)| {
            if i != self.parent.tid {
                self.rollback.iter().for_each(|k| {
                    // Ignore channel failures.
                    let _ = tx.send(Invalidate {
                        k: k.clone(),
                        txid: self.txid,
                    });
                });
            }
        });
        // Now we have issued our invalidations, we can tell people to invalidate up to this txid
        self.inv_up_to_txid.store(self.txid, Ordering::Release);
        // Ensure our rollback set is empty now to avoid the drop handler.
        self.rollback.clear();
        // We're done!
    }
}

impl<K, V> Drop for ThreadLocalWriteTxn<'_, K, V>
where
    K: Hash + Eq + Debug + Clone,
{
    fn drop(&mut self) {
        // Clear anything that's in the rollback.
        for k in self.rollback.iter() {
            self.parent.cache.pop(k);
        }
    }
}

impl<K, V> ThreadLocalReadTxn<'_, K, V>
where
    K: Hash + Eq + Debug + Clone,
{
    /// Attempt to retrieve a k-v pain from the cache. If it is not present, `None` is returned.
    pub fn get(&mut self, k: &K) -> Option<&V> {
        self.parent.cache.get(k)
    }

    /// Determine if the key exists in the cache.
    pub fn contains_key(&mut self, k: &K) -> bool {
        self.parent.cache.get(k).is_some()
    }

    /// Insert a new item to this cache for this transaction.
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.parent.cache.put(k, v)
    }
}

#[cfg(test)]
mod tests {
    use super::ThreadLocal;

    // Temporarily ignored due to a bug in lru.
    #[test]
    // #[cfg_attr(miri, ignore)]
    fn test_basic() {
        let mut cache: Vec<ThreadLocal<u32, u32>> = ThreadLocal::new(2, 8);
        let mut cache_a = cache.pop().unwrap();
        let mut cache_b = cache.pop().unwrap();

        let mut wr_txn = cache_a.write();
        let mut rd_txn = cache_b.read();

        wr_txn.insert(1, 1);
        wr_txn.insert(2, 2);
        assert!(wr_txn.contains_key(&1));
        assert!(wr_txn.contains_key(&2));

        assert!(!rd_txn.contains_key(&1));
        assert!(!rd_txn.contains_key(&2));
        wr_txn.commit();

        drop(rd_txn);

        let mut rd_txn = cache_b.read();
        // Even in a new txn, we don't have this in our cache.
        assert!(!rd_txn.contains_key(&1));
        assert!(!rd_txn.contains_key(&2));
        // But we can insert it to match
        rd_txn.insert(1, 1);
        rd_txn.insert(2, 2);
        drop(rd_txn);

        // Repeat use of rd should still show it.
        let mut rd_txn = cache_b.read();
        assert!(rd_txn.contains_key(&1));
        assert!(rd_txn.contains_key(&2));
        drop(rd_txn);

        // Add new items.
        let mut wr_txn = cache_a.write();
        assert!(wr_txn.contains_key(&1));
        assert!(wr_txn.contains_key(&2));
        wr_txn.insert(3, 3);
        assert!(wr_txn.contains_key(&3));
        drop(wr_txn);

        let mut wr_txn = cache_a.write();
        assert!(wr_txn.contains_key(&1));
        assert!(wr_txn.contains_key(&2));
        // Should have been rolled back.
        assert!(!wr_txn.contains_key(&3));

        // Now invalidate 1/2
        wr_txn.remove(&1);
        wr_txn.remove(&2);
        wr_txn.commit();

        // This sends invalidation reqs, so we should now have removed this in the other cache.
        let mut rd_txn = cache_b.read();
        // Even in a new txn, we don't have this in our cache.
        assert!(!rd_txn.contains_key(&1));
        assert!(!rd_txn.contains_key(&2));
    }
}
