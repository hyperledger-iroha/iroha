use crate::internals::hashtrie::cursor::CursorReadOps;
use crate::internals::hashtrie::cursor::{CursorRead, CursorWrite, SuperBlock};
use crate::internals::hashtrie::iter::*;
use std::borrow::Borrow;

use crate::internals::lincowcell::LinCowCellCapable;

use std::fmt::Debug;
use std::hash::Hash;
use std::iter::FromIterator;

/// A concurrently readable map based on a modified Trie.
///
///
///
/// This is a concurrently readable structure, meaning it has transactional
/// properties. Writers are serialised (one after the other), and readers
/// can exist in parallel with stable views of the structure at a point
/// in time.
///
/// This is achieved through the use of COW or MVCC. As a write occurs
/// subsets of the tree are cloned into the writer thread and then committed
/// later. This may cause memory usage to increase in exchange for a gain
/// in concurrent behaviour.
///
/// Transactions can be rolled-back (aborted) without penalty by dropping
/// the `HashTrieWriteTxn` without calling `commit()`.
pub struct HashTrie<K, V>
where
    K: Hash + Eq + Clone + Debug + Sync + Send + 'static,
    V: Clone + Sync + Send + 'static,
{
    inner: LinCowCell<SuperBlock<K, V>, CursorRead<K, V>, CursorWrite<K, V>>,
}

unsafe impl<K: Hash + Eq + Clone + Debug + Sync + Send + 'static, V: Clone + Sync + Send + 'static>
    Send for HashTrie<K, V>
{
}
unsafe impl<K: Hash + Eq + Clone + Debug + Sync + Send + 'static, V: Clone + Sync + Send + 'static>
    Sync for HashTrie<K, V>
{
}

/// An active read transaction over a `HashTrie`. The data in this tree
/// is guaranteed to not change and will remain consistent for the life
/// of this transaction.
pub struct HashTrieReadTxn<'a, K, V>
where
    K: Hash + Eq + Clone + Debug + Sync + Send + 'static,
    V: Clone + Sync + Send + 'static,
{
    inner: LinCowCellReadTxn<'a, SuperBlock<K, V>, CursorRead<K, V>, CursorWrite<K, V>>,
}

/// An active write transaction for a `HashTrie`. The data in this tree
/// may be modified exclusively through this transaction without affecting
/// readers. The write may be rolledback/aborted by dropping this guard
/// without calling `commit()`. Once `commit()` is called, readers will be
/// able to access and perceive changes in new transactions.
pub struct HashTrieWriteTxn<'a, K, V>
where
    K: Hash + Eq + Clone + Debug + Sync + Send + 'static,
    V: Clone + Sync + Send + 'static,
{
    inner: LinCowCellWriteTxn<'a, SuperBlock<K, V>, CursorRead<K, V>, CursorWrite<K, V>>,
}

enum SnapshotType<'a, K, V>
where
    K: Hash + Eq + Clone + Debug + Sync + Send + 'static,
    V: Clone + Sync + Send + 'static,
{
    R(&'a CursorRead<K, V>),
    W(&'a CursorWrite<K, V>),
}

/// A point-in-time snapshot of the tree from within a read OR write. This is
/// useful for building other transactional types on top of this structure, as
/// you need a way to downcast both HashTrieReadTxn or HashTrieWriteTxn to
/// a singular reader type for a number of get_inner() style patterns.
///
/// This snapshot IS safe within the read thread due to the nature of the
/// implementation borrowing the inner tree to prevent mutations within the
/// same thread while the read snapshot is open.
pub struct HashTrieReadSnapshot<'a, K, V>
where
    K: Hash + Eq + Clone + Debug + Sync + Send + 'static,
    V: Clone + Sync + Send + 'static,
{
    inner: SnapshotType<'a, K, V>,
}

impl<K: Hash + Eq + Clone + Debug + Sync + Send + 'static, V: Clone + Sync + Send + 'static> Default
    for HashTrie<K, V>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Hash + Eq + Clone + Debug + Sync + Send + 'static, V: Clone + Sync + Send + 'static>
    FromIterator<(K, V)> for HashTrie<K, V>
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut new_sblock = unsafe { SuperBlock::new() };
        let prev = new_sblock.create_reader();
        let mut cursor = new_sblock.create_writer();
        cursor.extend(iter);

        let _ = new_sblock.pre_commit(cursor, &prev);

        HashTrie {
            inner: LinCowCell::new(new_sblock),
        }
    }
}

impl<K: Hash + Eq + Clone + Debug + Sync + Send + 'static, V: Clone + Sync + Send + 'static>
    Extend<(K, V)> for HashTrieWriteTxn<'_, K, V>
{
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        self.inner.as_mut().extend(iter);
    }
}

impl<K: Hash + Eq + Clone + Debug + Sync + Send + 'static, V: Clone + Sync + Send + 'static>
    HashTrieWriteTxn<'_, K, V>
{
    /*
    pub(crate) fn prehash<Q>(&self, k: &Q) -> u64
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.as_ref().hash_key(k)
    }
    */

    pub(crate) fn get_prehashed<Q>(&self, k: &Q, k_hash: u64) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.as_ref().search(k_hash, k)
    }

    /// Retrieve a value from the map. If the value exists, a reference is returned
    /// as `Some(&V)`, otherwise if not present `None` is returned.
    pub fn get<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let k_hash = self.inner.as_ref().hash_key(k);
        self.get_prehashed(k, k_hash)
    }

    /// Assert if a key exists in the map.
    pub fn contains_key<Q>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get(k).is_some()
    }

    /// returns the current number of k:v pairs in the tree
    pub fn len(&self) -> usize {
        self.inner.as_ref().len()
    }

    /// Determine if the set is currently empty
    pub fn is_empty(&self) -> bool {
        self.inner.as_ref().len() == 0
    }

    /// Iterator over `(&K, &V)` of the set
    pub fn iter(&self) -> Iter<'_, K, V> {
        self.inner.as_ref().kv_iter()
    }

    /// Iterator over &K
    pub fn values(&self) -> ValueIter<'_, K, V> {
        self.inner.as_ref().v_iter()
    }

    /// Iterator over &V
    pub fn keys(&self) -> KeyIter<'_, K, V> {
        self.inner.as_ref().k_iter()
    }

    /// Reset this map to an empty state. As this is within the transaction this
    /// change only takes effect once committed. Once cleared, you can begin adding
    /// new writes and changes, again, that will only be visible once committed.
    pub fn clear(&mut self) {
        self.inner.as_mut().clear()
    }

    /// Insert or update a value by key. If the value previously existed it is returned
    /// as `Some(V)`. If the value did not previously exist this returns `None`.
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        // Hash the key.
        let k_hash = self.inner.as_ref().hash_key(&k);
        self.inner.as_mut().insert(k_hash, k, v)
    }

    /// Remove a key if it exists in the tree. If the value exists, we return it as `Some(V)`,
    /// and if it did not exist, we return `None`
    pub fn remove(&mut self, k: &K) -> Option<V> {
        let k_hash = self.inner.as_ref().hash_key(k);
        self.inner.as_mut().remove(k_hash, k)
    }

    /// Get a mutable reference to a value in the tree. This is correctly, and
    /// safely cloned before you attempt to mutate the value, isolating it from
    /// other transactions.
    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        let k_hash = self.inner.as_ref().hash_key(k);
        self.inner.as_mut().get_mut_ref(k_hash, k)
    }

    /// Create a read-snapshot of the current map. This does NOT guarantee the map may
    /// not be mutated during the read, so you MUST guarantee that no functions of the
    /// write txn are called while this snapshot is active.
    pub fn to_snapshot(&self) -> HashTrieReadSnapshot<'_, K, V> {
        HashTrieReadSnapshot {
            inner: SnapshotType::W(self.inner.as_ref()),
        }
    }
}

impl<K: Hash + Eq + Clone + Debug + Sync + Send + 'static, V: Clone + Sync + Send + 'static>
    HashTrieReadTxn<'_, K, V>
{
    pub(crate) fn get_prehashed<Q>(&self, k: &Q, k_hash: u64) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.search(k_hash, k)
    }

    /// Retrieve a value from the tree. If the value exists, a reference is returned
    /// as `Some(&V)`, otherwise if not present `None` is returned.
    pub fn get<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let k_hash = self.inner.as_ref().hash_key(k);
        self.get_prehashed(k, k_hash)
    }

    /// Assert if a key exists in the tree.
    pub fn contains_key<Q>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get(k).is_some()
    }

    /// Returns the current number of k:v pairs in the tree
    pub fn len(&self) -> usize {
        self.inner.as_ref().len()
    }

    /// Determine if the set is currently empty
    pub fn is_empty(&self) -> bool {
        self.inner.as_ref().len() == 0
    }

    /// Iterator over `(&K, &V)` of the set
    pub fn iter(&self) -> Iter<'_, K, V> {
        self.inner.as_ref().kv_iter()
    }

    /// Iterator over &K
    pub fn values(&self) -> ValueIter<'_, K, V> {
        self.inner.as_ref().v_iter()
    }

    /// Iterator over &V
    pub fn keys(&self) -> KeyIter<'_, K, V> {
        self.inner.as_ref().k_iter()
    }

    /// Create a read-snapshot of the current tree.
    /// As this is the read variant, it IS safe, and guaranteed the tree will not change.
    pub fn to_snapshot(&self) -> HashTrieReadSnapshot<'_, K, V> {
        HashTrieReadSnapshot {
            inner: SnapshotType::R(self.inner.as_ref()),
        }
    }
}

impl<K: Hash + Eq + Clone + Debug + Sync + Send + 'static, V: Clone + Sync + Send + 'static>
    HashTrieReadSnapshot<'_, K, V>
{
    /// Retrieve a value from the tree. If the value exists, a reference is returned
    /// as `Some(&V)`, otherwise if not present `None` is returned.
    pub fn get<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        match self.inner {
            SnapshotType::R(inner) => {
                let k_hash = inner.hash_key(k);
                inner.search(k_hash, k)
            }
            SnapshotType::W(inner) => {
                let k_hash = inner.hash_key(k);
                inner.search(k_hash, k)
            }
        }
    }

    /// Assert if a key exists in the tree.
    pub fn contains_key<Q>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get(k).is_some()
    }

    /// Returns the current number of k:v pairs in the tree
    pub fn len(&self) -> usize {
        match self.inner {
            SnapshotType::R(inner) => inner.len(),
            SnapshotType::W(inner) => inner.len(),
        }
    }

    /// Determine if the set is currently empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // (adv) range

    /// Iterator over `(&K, &V)` of the set
    pub fn iter(&self) -> Iter<'_, K, V> {
        match self.inner {
            SnapshotType::R(inner) => inner.kv_iter(),
            SnapshotType::W(inner) => inner.kv_iter(),
        }
    }

    /// Iterator over &K
    pub fn values(&self) -> ValueIter<'_, K, V> {
        match self.inner {
            SnapshotType::R(inner) => inner.v_iter(),
            SnapshotType::W(inner) => inner.v_iter(),
        }
    }

    /// Iterator over &V
    pub fn keys(&self) -> KeyIter<'_, K, V> {
        match self.inner {
            SnapshotType::R(inner) => inner.k_iter(),
            SnapshotType::W(inner) => inner.k_iter(),
        }
    }
}
