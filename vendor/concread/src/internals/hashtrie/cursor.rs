//! The cursor is what actually knits a trie together from the parts
//! we have, and has an important role to keep the system consistent.
//!
//! Additionally, the cursor also is responsible for general movement
//! throughout the structure and how to handle that effectively

use crate::internals::lincowcell::LinCowCellCapable;

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::{BTreeSet, VecDeque};
use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::ptr;
use std::sync::Mutex;

use smallvec::SmallVec;

use super::iter::*;

#[cfg(feature = "ahash")]
use ahash::RandomState;

#[cfg(feature = "foldhash")]
use foldhash::fast::RandomState;

#[cfg(all(not(feature = "ahash"), not(feature = "foldhash")))]
use std::collections::hash_map::RandomState;

use std::hash::{BuildHasher, Hash, Hasher};

// This defines the max height of our tree. Gives 16777216.0 entries
// This only consumes 16KB if fully populated
#[cfg(feature = "hashtrie_skinny")]
pub(crate) const MAX_HEIGHT: u64 = 8;
// The true absolute max height
#[cfg(all(feature = "hashtrie_skinny", any(test, debug_assertions)))]
const ABS_MAX_HEIGHT: u64 = 21;
#[cfg(feature = "hashtrie_skinny")]
pub(crate) const HT_CAPACITY: usize = 8;
#[cfg(feature = "hashtrie_skinny")]
const HASH_MASK: u64 =
    0b0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0111;
#[cfg(feature = "hashtrie_skinny")]
const SHIFT: u64 = 3;

// This defines the max height of our tree. Gives 16777216.0 entries
#[cfg(not(feature = "hashtrie_skinny"))]
pub(crate) const MAX_HEIGHT: u64 = 6;
#[cfg(all(not(feature = "hashtrie_skinny"), any(test, debug_assertions)))]
const ABS_MAX_HEIGHT: u64 = 16;
#[cfg(not(feature = "hashtrie_skinny"))]
pub(crate) const HT_CAPACITY: usize = 16;
#[cfg(not(feature = "hashtrie_skinny"))]
const HASH_MASK: u64 =
    0b0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_1111;
#[cfg(not(feature = "hashtrie_skinny"))]
const SHIFT: u64 = 4;

const TAG: usize = 0b0011;
const UNTAG: usize = usize::MAX - TAG;

// const FLAG_CLEAN: usize = 0b00;
const FLAG_DIRTY: usize = 0b01;
const MARK_CLEAN: usize = usize::MAX - FLAG_DIRTY;
const FLAG_BRANCH: usize = 0b00;
const FLAG_BUCKET: usize = 0b10;

const DEFAULT_BUCKET_ALLOC: usize = 1;

macro_rules! hash_key {
    ($self:expr, $k:expr) => {{
        let mut hasher = $self.build_hasher.build_hasher();
        $k.hash(&mut hasher);
        hasher.finish()
    }};
}

#[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
thread_local!(static ALLOC_LIST: Mutex<BTreeSet<Ptr>> = const { Mutex::new(BTreeSet::new()) });

#[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
thread_local!(static WRITE_LIST: Mutex<BTreeSet<Ptr>> = const { Mutex::new(BTreeSet::new()) });

#[cfg(test)]
fn assert_released() {
    #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
    {
        let is_empty = ALLOC_LIST.with(|llist| {
            let x = llist.lock().unwrap();
            println!("Remaining -> {:?}", x);
            x.is_empty()
        });
        assert!(is_empty);
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Ptr {
    // We essential are using this as a void pointer for provenance reasons.
    p: *mut i32,
}

impl PartialEq for Ptr {
    fn eq(&self, other: &Self) -> bool {
        let s = self.p.map_addr(|a| a & UNTAG);
        let o = other.p.map_addr(|a| a & UNTAG);
        s == o
    }
}

impl Eq for Ptr {}

impl PartialOrd for Ptr {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ptr {
    fn cmp(&self, other: &Self) -> Ordering {
        let s = self.p.map_addr(|a| a & UNTAG);
        let o = other.p.map_addr(|a| a & UNTAG);
        s.cmp(&o)
    }
}

impl Debug for Ptr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Ptr")
            .field("p", &self.p)
            .field("untagged", &self.p.map_addr(|a| a & UNTAG))
            .field("bucket", &self.is_bucket())
            .field("dirty", &self.is_dirty())
            .field("null", &self.is_null())
            .finish()
    }
}

impl Ptr {
    fn null_mut() -> Self {
        debug_assert!(std::mem::size_of::<Ptr>() == std::mem::size_of::<*mut Branch<u64, u64>>());
        debug_assert!(std::mem::size_of::<Ptr>() == std::mem::size_of::<*mut Bucket<u64, u64>>());
        Ptr { p: ptr::null_mut() }
    }

    #[inline(always)]
    pub(crate) fn is_null(&self) -> bool {
        self.p.is_null()
    }

    #[inline(always)]
    pub(crate) fn is_bucket(&self) -> bool {
        self.p.addr() & FLAG_BUCKET == FLAG_BUCKET
    }

    #[inline(always)]
    pub(crate) fn is_branch(&self) -> bool {
        self.p.addr() & FLAG_BUCKET != FLAG_BUCKET
    }

    #[inline(always)]
    fn is_dirty(&self) -> bool {
        self.p.addr() & FLAG_DIRTY == FLAG_DIRTY
    }

    #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
    fn untagged(&self) -> Self {
        let p = self.p.map_addr(|a| a & UNTAG);
        Ptr { p }
    }

    #[inline(always)]
    fn mark_dirty(&mut self) {
        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        WRITE_LIST.with(|llist| assert!(llist.lock().unwrap().insert(self.untagged())));
        self.p = self.p.map_addr(|a| a | FLAG_DIRTY)
    }

    #[inline(always)]
    fn mark_clean(&mut self) {
        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        WRITE_LIST.with(|llist| assert!(llist.lock().unwrap().remove(&(self.untagged()))));
        self.p = self.p.map_addr(|a| a & MARK_CLEAN)
    }

    #[inline(always)]
    pub(crate) fn as_bucket<K: Hash + Eq + Clone + Debug, V: Clone>(&self) -> &Bucket<K, V> {
        debug_assert!(self.is_bucket());
        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        {
            let contains =
                ALLOC_LIST.with(|llist| llist.lock().unwrap().contains(&self.untagged()));
            assert!(contains, "as_bucket -> {:?} MISSING", self);
        }
        unsafe { &*(self.p.map_addr(|a| a & UNTAG) as *const Bucket<K, V>) }
    }

    #[inline(always)]
    fn as_bucket_raw<K: Hash + Eq + Clone + Debug, V: Clone>(&self) -> *mut Bucket<K, V> {
        debug_assert!(self.is_bucket());
        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        ALLOC_LIST.with(|llist| assert!(llist.lock().unwrap().contains(&self.untagged())));
        self.p.map_addr(|a| a & UNTAG) as *mut Bucket<K, V>
    }

    #[inline(always)]
    #[allow(clippy::mut_from_ref)]
    pub(crate) fn as_bucket_mut<K: Hash + Eq + Clone + Debug, V: Clone>(
        &self,
    ) -> &mut Bucket<K, V> {
        debug_assert!(self.is_bucket());
        debug_assert!(self.is_dirty());
        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        WRITE_LIST.with(|llist| {
            let wlist_guard = llist.lock().unwrap();
            assert!(wlist_guard.contains(&self.untagged()))
        });
        unsafe { &mut *(self.p.map_addr(|a| a & UNTAG) as *mut Bucket<K, V>) }
    }

    #[inline(always)]
    pub(crate) fn as_branch<K: Hash + Eq + Clone + Debug, V: Clone>(&self) -> &Branch<K, V> {
        debug_assert!(self.is_branch());
        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        {
            let contains =
                ALLOC_LIST.with(|llist| llist.lock().unwrap().contains(&self.untagged()));
            assert!(contains, "as_branch -> {:?} MISSING", self);
        }
        unsafe { &*(self.p.map_addr(|a| a & UNTAG) as *const Branch<K, V>) }
    }

    #[inline(always)]
    fn as_branch_raw<K: Hash + Eq + Clone + Debug, V: Clone>(&self) -> *mut Branch<K, V> {
        debug_assert!(self.is_branch());
        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        ALLOC_LIST.with(|llist| assert!(llist.lock().unwrap().contains(&self.untagged())));
        self.p.map_addr(|a| a & UNTAG) as *mut Branch<K, V>
    }

    #[inline(always)]
    #[allow(clippy::mut_from_ref)]
    pub(crate) fn as_branch_mut<K: Hash + Eq + Clone + Debug, V: Clone>(
        &self,
    ) -> &mut Branch<K, V> {
        debug_assert!(self.is_branch());
        debug_assert!(self.is_dirty());
        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        WRITE_LIST.with(|llist| {
            let wlist_guard = llist.lock().unwrap();
            assert!(wlist_guard.contains(&self.untagged()))
        });
        unsafe { &mut *(self.p.map_addr(|a| a & UNTAG) as *mut Branch<K, V>) }
    }

    #[inline(always)]
    #[allow(clippy::mut_from_ref)]
    unsafe fn as_branch_mut_nock<K: Hash + Eq + Clone + Debug, V: Clone>(
        &self,
    ) -> &mut Branch<K, V> {
        debug_assert!(self.is_branch());
        debug_assert!(self.is_dirty());
        // This is the same as above, but bypasses the wlist check.
        &mut *(self.p.map_addr(|a| a & UNTAG) as *mut Branch<K, V>)
    }

    fn free<K: Hash + Eq + Clone + Debug, V: Clone>(&self) {
        // eprintln!("free      -> {:?}", self);
        // We MUST have allocated this, else it's a double free
        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        ALLOC_LIST.with(|llist| assert!(llist.lock().unwrap().contains(&self.untagged())));

        // It's getting freeeeeedddd
        unsafe {
            if self.is_bucket() {
                let _ = Box::from_raw(self.as_bucket_raw::<K, V>());
            } else {
                let _ = Box::from_raw(self.as_branch_raw::<K, V>());
            }
        }

        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        if self.is_dirty() {
            WRITE_LIST.with(|llist| assert!(llist.lock().unwrap().remove(&(self.untagged()))))
        };

        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        ALLOC_LIST.with(|llist| assert!(llist.lock().unwrap().remove(&self.untagged())));
    }
}

impl<K: Hash + Eq + Clone + Debug, V: Clone> From<Box<Branch<K, V>>> for Ptr {
    fn from(b: Box<Branch<K, V>>) -> Self {
        let rptr: *mut Branch<K, V> = Box::into_raw(b);

        // Assert that the tag bits are not present
        assert_eq!(
            rptr,
            rptr.map_addr(|a| { a & UNTAG }),
            "Impossible State! rptr tag bits are set incorrectly!"
        );

        #[allow(clippy::let_and_return)]
        let r = Self {
            p: rptr.map_addr(|a| a | FLAG_BRANCH) as *mut i32,
        };

        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        ALLOC_LIST.with(|llist| llist.lock().unwrap().insert(r.untagged()));
        r
    }
}

impl<K: Hash + Eq + Clone + Debug, V: Clone> From<Box<Bucket<K, V>>> for Ptr {
    fn from(b: Box<Bucket<K, V>>) -> Self {
        let rptr: *mut Bucket<K, V> = Box::into_raw(b);

        // Assert that the tag bits are not present
        assert_eq!(
            rptr,
            rptr.map_addr(|a| { a & UNTAG }),
            "Impossible State! rptr tag bits are set incorrectly!"
        );

        #[allow(clippy::let_and_return)]
        let r = Self {
            p: rptr.map_addr(|a| a | FLAG_BUCKET) as *mut i32,
        };

        #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
        ALLOC_LIST.with(|llist| llist.lock().unwrap().insert(r.untagged()));
        r
    }
}

/// A stored K/V in the hash bucket.
#[derive(Clone)]
pub struct Datum<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    /// The hash of K.
    pub h: u64,
    /// The K in K:V.
    pub k: K,
    /// The V in K:V.
    pub v: V,
}

type Bucket<K, V> = SmallVec<[Datum<K, V>; DEFAULT_BUCKET_ALLOC]>;

fn new_dirty_bucket_ptr<K: Hash + Eq + Clone + Debug, V: Clone>() -> Ptr {
    let bkt: Box<Bucket<K, V>> = Box::new(SmallVec::new());

    let mut p = Ptr::from(bkt);
    p.mark_dirty();
    p
}

#[repr(align(64))]
pub(crate) struct Branch<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    // Pointer to either a Branch, or a Bucket.
    pub nodes: [Ptr; HT_CAPACITY],
    k: PhantomData<K>,
    v: PhantomData<V>,
}

fn new_dirty_branch_ptr<K: Hash + Eq + Clone + Debug, V: Clone>() -> Ptr {
    let brch: Box<Branch<K, V>> = Branch::new();

    let mut p = Ptr::from(brch);
    p.mark_dirty();
    debug_assert!(p.is_dirty());
    debug_assert!(p.is_branch());
    p
}

impl<K: Hash + Eq + Clone + Debug, V: Clone> Branch<K, V> {
    fn new() -> Box<Self> {
        debug_assert!(std::mem::size_of::<usize>() == std::mem::size_of::<*mut Branch<K, V>>());
        Box::new(Branch {
            nodes: [
                Ptr::null_mut(),
                Ptr::null_mut(),
                Ptr::null_mut(),
                Ptr::null_mut(),
                Ptr::null_mut(),
                Ptr::null_mut(),
                Ptr::null_mut(),
                Ptr::null_mut(),
                #[cfg(not(feature = "hashtrie_skinny"))]
                Ptr::null_mut(),
                #[cfg(not(feature = "hashtrie_skinny"))]
                Ptr::null_mut(),
                #[cfg(not(feature = "hashtrie_skinny"))]
                Ptr::null_mut(),
                #[cfg(not(feature = "hashtrie_skinny"))]
                Ptr::null_mut(),
                #[cfg(not(feature = "hashtrie_skinny"))]
                Ptr::null_mut(),
                #[cfg(not(feature = "hashtrie_skinny"))]
                Ptr::null_mut(),
                #[cfg(not(feature = "hashtrie_skinny"))]
                Ptr::null_mut(),
                #[cfg(not(feature = "hashtrie_skinny"))]
                Ptr::null_mut(),
            ],
            k: PhantomData,
            v: PhantomData,
        })
    }

    fn clone_dirty(&self) -> Ptr {
        let bc: Box<Branch<K, V>> = Box::new(Branch {
            nodes: self.nodes,
            k: PhantomData,
            v: PhantomData,
        });
        let mut p = Ptr::from(bc);
        p.mark_dirty();
        p
    }
}

#[derive(Debug)]
pub(crate) struct SuperBlock<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    root: Ptr,
    length: usize,
    txid: u64,
    build_hasher: RandomState,
    k: PhantomData<K>,
    v: PhantomData<V>,
}

impl<K: Hash + Eq + Clone + Debug, V: Clone> SuperBlock<K, V> {
    /// 🔥 🔥 🔥
    pub unsafe fn new() -> Self {
        #[allow(clippy::assertions_on_constants)]
        {
            #[cfg(any(test, debug_assertions))]
            assert!(MAX_HEIGHT <= ABS_MAX_HEIGHT);
        }

        let b: Box<Branch<K, V>> = Branch::new();
        let root = Ptr::from(b);
        SuperBlock {
            root,
            length: 0,
            txid: 1,
            build_hasher: RandomState::default(),
            k: PhantomData,
            v: PhantomData,
        }
    }
}

impl<K: Hash + Eq + Clone + Debug, V: Clone> LinCowCellCapable<CursorRead<K, V>, CursorWrite<K, V>>
    for SuperBlock<K, V>
{
    fn create_reader(&self) -> CursorRead<K, V> {
        CursorRead::new(self)
    }

    fn create_writer(&self) -> CursorWrite<K, V> {
        CursorWrite::new(self)
    }

    fn pre_commit(
        &mut self,
        mut new: CursorWrite<K, V>,
        prev: &CursorRead<K, V>,
    ) -> CursorRead<K, V> {
        let mut prev_last_seen = prev.last_seen.lock().unwrap();
        debug_assert!((*prev_last_seen).is_empty());

        let new_last_seen = &mut new.last_seen;
        std::mem::swap(&mut (*prev_last_seen), &mut (*new_last_seen));
        debug_assert!((*new_last_seen).is_empty());

        // Mark anything in the tree that is dirty as clean.
        new.mark_clean();

        // Now when the lock is dropped, both sides see the correct info and garbage for drops.
        // Clear first seen, we won't be dropping them from here.
        new.first_seen.clear();

        self.root = new.root;
        self.length = new.length;
        self.txid = new.txid;

        debug_assert!(!self.root.is_dirty());
        debug_assert!(self.root.is_branch());

        // Create the new reader.
        CursorRead::new(self)
    }
}

impl<K: Clone + Hash + Eq + Debug, V: Clone> Drop for SuperBlock<K, V> {
    fn drop(&mut self) {
        // eprintln!("Releasing SuperBlock ...");
        // We must be the last SB and no txns exist. Drop the tree now.
        // TODO: Calc this based on size.
        let mut first_seen = Vec::with_capacity(16);

        let mut stack = VecDeque::new();
        stack.push_back(self.root);

        while let Some(tgt_ptr) = stack.pop_front() {
            first_seen.push(tgt_ptr);
            if tgt_ptr.is_branch() {
                for n in tgt_ptr.as_branch::<K, V>().nodes.iter() {
                    if !n.is_null() {
                        stack.push_back(*n);
                    }
                }
            }
        }

        first_seen.iter().for_each(|p| p.free::<K, V>());
    }
}

pub(crate) trait CursorReadOps<K: Clone + Hash + Eq + Debug, V: Clone> {
    fn get_root_ptr(&self) -> Ptr;

    fn len(&self) -> usize;

    fn get_txid(&self) -> u64;

    fn hash_key<Q>(&self, k: &Q) -> u64
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized;

    fn search<Q>(&self, h: u64, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let mut node = self.get_root_ptr();
        for d in 0..MAX_HEIGHT {
            let bref: &Branch<K, V> = node.as_branch();

            let idx = ((h & (HASH_MASK << (d * SHIFT))) >> (d * SHIFT)) as usize;
            debug_assert!(idx < HT_CAPACITY);

            let tgt_ptr = bref.nodes[idx];

            // If null
            if tgt_ptr.is_null() {
                // Not found
                return None;
            } else if tgt_ptr.is_branch() {
                node = tgt_ptr;
            } else {
                for datum in tgt_ptr.as_bucket::<K, V>().iter() {
                    if datum.h == h && k.eq(datum.k.borrow()) {
                        // Must be it!
                        let x = &datum.v as *const V;
                        return Some(unsafe { &*x as &V });
                    }
                }
                // Not found.
                return None;
            }
        }
        unreachable!();
    }

    fn kv_iter(&self) -> Iter<'_, K, V> {
        Iter::new(self.get_root_ptr(), self.len())
    }

    fn k_iter(&self) -> KeyIter<'_, K, V> {
        KeyIter::new(self.get_root_ptr(), self.len())
    }

    fn v_iter(&self) -> ValueIter<'_, K, V> {
        ValueIter::new(self.get_root_ptr(), self.len())
    }

    #[allow(unused)]
    fn verify_inner(&self, expect_clean: bool) {
        let root = self.get_root_ptr();
        assert!(root.is_branch());

        let mut stack = VecDeque::new();
        let mut ptr_map = BTreeSet::new();
        let mut length = 0;

        stack.push_back(root);

        while let Some(tgt_ptr) = stack.pop_front() {
            // Is true if not present. Every ptr should be unique!
            assert!(ptr_map.insert(tgt_ptr));

            if expect_clean {
                assert!(!tgt_ptr.is_dirty());
            }

            if tgt_ptr.is_branch() {
                // For all nodes
                for n in tgt_ptr.as_branch::<K, V>().nodes.iter() {
                    if !n.is_null() {
                        stack.push_back(*n);
                    }
                }
            } else {
                assert!(tgt_ptr.is_bucket());
                // How long?
                length += tgt_ptr.as_bucket::<K, V>().len();
            }
        }
        // Is our element tracker correct?
        assert!(length == self.len());
    }

    #[allow(unused)]
    fn verify(&self);
}

#[derive(Debug)]
pub(crate) struct CursorWrite<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    txid: u64,
    length: usize,
    root: Ptr,
    last_seen: Vec<Ptr>,
    first_seen: Vec<Ptr>,
    build_hasher: RandomState,
    k: PhantomData<K>,
    v: PhantomData<V>,
}

impl<K: Clone + Hash + Eq + Debug, V: Clone> CursorWrite<K, V> {
    pub(crate) fn new(sblock: &SuperBlock<K, V>) -> Self {
        let txid = sblock.txid + 1;
        let length = sblock.length;
        let root = sblock.root;
        let last_seen = Vec::with_capacity(16);
        let first_seen = Vec::with_capacity(16);

        CursorWrite {
            txid,
            length,
            root,
            last_seen,
            first_seen,
            build_hasher: sblock.build_hasher.to_owned(),
            k: PhantomData,
            v: PhantomData,
        }
    }

    fn dirty_root(&mut self) {
        // If needed, clone root and mark dirty. Swap back into the
        // cursor.
        debug_assert!(self.root.is_branch());
        if !self.root.is_dirty() {
            let clean_bref: &Branch<K, V> = self.root.as_branch();
            self.last_seen.push(self.root);
            // Over-write the ptr with our new ptr.
            self.root = clean_bref.clone_dirty();
            self.first_seen.push(self.root);
        }
    }

    fn mark_clean(&mut self) {
        let root = self.get_root_ptr();
        assert!(root.is_branch());

        let mut stack: VecDeque<Ptr> = VecDeque::new();
        if root.is_dirty() {
            stack.push_back(root);
            self.root.mark_clean();
        }

        while let Some(tgt_ptr) = stack.pop_front() {
            // For all nodes
            for n in unsafe { tgt_ptr.as_branch_mut_nock::<K, V>().nodes.iter_mut() } {
                if !n.is_null() && n.is_dirty() {
                    if n.is_branch() {
                        stack.push_back(*n);
                    }
                    n.mark_clean();
                }
            }

            if cfg!(debug_assertions) {
                for n in tgt_ptr.as_branch::<K, V>().nodes.iter() {
                    assert!(n.is_null() || !n.is_dirty());
                }
            }
        }
    }

    pub(crate) fn insert(&mut self, h: u64, k: K, mut v: V) -> Option<V> {
        self.dirty_root();
        let mut node = self.root;

        for d in 0..MAX_HEIGHT {
            // In the current node
            let bref: &mut Branch<K, V> = node.as_branch_mut();

            // Get our idx from the node.
            let shift = d * SHIFT;
            let idx = ((h & (HASH_MASK << shift)) >> shift) as usize;
            debug_assert!(idx < HT_CAPACITY);

            let tgt_ptr = bref.nodes[idx];

            // If null
            if tgt_ptr.is_null() {
                let dbkt_ptr = new_dirty_bucket_ptr::<K, V>();
                self.first_seen.push(dbkt_ptr);
                // Insert the item.
                dbkt_ptr.as_bucket_mut().push(Datum { h, k, v });
                // Place the dbkt_ptr into the branch
                bref.nodes[idx] = dbkt_ptr;
                // Correct the size.
                self.length += 1;
                // Done!
                return None;
            } else if tgt_ptr.is_branch() {
                if tgt_ptr.is_dirty() {
                    node = tgt_ptr;
                } else {
                    self.last_seen.push(tgt_ptr);
                    let from_bref: &Branch<K, V> = tgt_ptr.as_branch();
                    // Over-write the ptr with our new ptr.
                    let nbrch_ptr = from_bref.clone_dirty();
                    self.first_seen.push(nbrch_ptr);
                    bref.nodes[idx] = nbrch_ptr;
                    // next ptr is our new branch, we let the loop continue.
                    node = nbrch_ptr;
                }
            } else {
                // if bucket
                if d == (MAX_HEIGHT - 1) {
                    let bkt_ptr = if tgt_ptr.is_dirty() {
                        // If the bkt is dirty, we can just append.
                        tgt_ptr
                    } else {
                        // Bucket is clean
                        self.last_seen.push(tgt_ptr);
                        let dbkt_ptr = new_dirty_bucket_ptr::<K, V>();
                        self.first_seen.push(dbkt_ptr);

                        let dbkt = dbkt_ptr.as_bucket_mut::<K, V>();
                        tgt_ptr.as_bucket().iter().for_each(|datum| {
                            dbkt.push(datum.clone());
                        });
                        bref.nodes[idx] = dbkt_ptr;
                        dbkt_ptr
                    };
                    // Handle duplicate K?
                    let bkt = bkt_ptr.as_bucket_mut::<K, V>();
                    for datum in bkt.iter_mut() {
                        if datum.h == h && k.eq(datum.k.borrow()) {
                            // Collision, swap and replace.
                            std::mem::swap(&mut datum.v, &mut v);
                            return Some(v);
                        }
                    }
                    // Wasn't found, append.
                    self.length += 1;
                    bkt.push(Datum { h, k, v });
                    return None;
                } else {
                    let bkt_ptr = if tgt_ptr.is_dirty() {
                        // If the bkt is dirty, we can just re-locate it
                        tgt_ptr
                    } else {
                        // The logic for if a bucket can be n>1
                        // isn't added!
                        debug_assert!(tgt_ptr.as_bucket::<K, V>().len() == 1);
                        // If it's clean, we need to duplicate it.
                        self.last_seen.push(tgt_ptr);
                        let dbkt_ptr = new_dirty_bucket_ptr::<K, V>();
                        self.first_seen.push(dbkt_ptr);

                        let dbkt = dbkt_ptr.as_bucket_mut::<K, V>();
                        tgt_ptr.as_bucket().iter().for_each(|datum| {
                            dbkt.push(datum.clone());
                        });
                        bref.nodes[idx] = dbkt_ptr;
                        dbkt_ptr
                    };

                    // create new branch, and insert the bucket.
                    let nbrch_ptr = new_dirty_branch_ptr::<K, V>();
                    self.first_seen.push(nbrch_ptr);
                    // Locate where in the new branch we need to relocate
                    // our bucket.
                    let bh = bkt_ptr.as_bucket_mut::<K, V>()[0].h;
                    let shift = (d + 1) * SHIFT;
                    let bidx = ((bh & (HASH_MASK << shift)) >> shift) as usize;
                    debug_assert!(bidx < HT_CAPACITY);
                    nbrch_ptr.as_branch_mut::<K, V>().nodes[bidx] = bkt_ptr;
                    bref.nodes[idx] = nbrch_ptr;
                    // next ptr is our new branch, we let the loop continue.
                    node = nbrch_ptr;
                }
            }
        }
        unreachable!();
    }

    pub(crate) fn remove(&mut self, h: u64, k: &K) -> Option<V> {
        self.dirty_root();
        let mut node = self.root;

        for d in 0..MAX_HEIGHT {
            debug_assert!(node.is_dirty());
            debug_assert!(node.is_branch());
            // In the current node
            let bref: &mut Branch<K, V> = node.as_branch_mut();

            // Get our idx from the node.
            let shift = d * SHIFT;
            let idx = ((h & (HASH_MASK << shift)) >> shift) as usize;
            debug_assert!(idx < HT_CAPACITY);

            let tgt_ptr = bref.nodes[idx];

            // If null
            if tgt_ptr.is_null() {
                // Done!
                return None;
            } else if tgt_ptr.is_branch() {
                if tgt_ptr.is_dirty() {
                    node = tgt_ptr;
                } else {
                    self.last_seen.push(tgt_ptr);
                    let from_bref: &Branch<K, V> = tgt_ptr.as_branch();
                    // Over-write the ptr with our new ptr.
                    let nbrch_ptr = from_bref.clone_dirty();
                    self.first_seen.push(nbrch_ptr);
                    bref.nodes[idx] = nbrch_ptr;
                    // next ptr is our new branch, we let the loop continue.
                    node = nbrch_ptr;
                }
            } else {
                // if bucket
                // Fast path - if the tgt is len 1, we can just remove it.
                debug_assert!(!tgt_ptr.as_bucket::<K, V>().is_empty());
                if tgt_ptr.as_bucket::<K, V>().len() == 1 {
                    let tgt_bkt = tgt_ptr.as_bucket::<K, V>();
                    let datum = &tgt_bkt[0];
                    if datum.h == h && k.eq(datum.k.borrow()) {
                        bref.nodes[idx] = Ptr::null_mut();

                        // There is a bit of a difficult case here. If a pointer
                        // is dirty, then it must also have been allocated in
                        // this txn. It's possible we risk leaking the memory
                        // here since the node will be in first_seen, and if we
                        // commit after a insert + remove + insert then the node
                        // risks orphaning. However, this also leads to a possible
                        // double free since if we free here AND a rollback occurs
                        // then we haven't dropped the node.
                        //
                        // So this is a sticky situation - As a result we have to
                        // walk the first_seen and remove this element before we
                        // do this out-of-band free.
                        let v = if tgt_ptr.is_dirty() {
                            let tgt_bkt_mut = tgt_ptr.as_bucket_mut::<K, V>();
                            let Datum { v, .. } = tgt_bkt_mut.remove(0);
                            // Keep any pointer that ISN'T the one we are oob freeing.
                            self.first_seen.retain(|e| *e != tgt_ptr);
                            tgt_ptr.free::<K, V>();
                            v
                        } else {
                            self.last_seen.push(tgt_ptr);
                            datum.v.clone()
                        };

                        self.length -= 1;
                        return Some(v);
                    } else {
                        return None;
                    }
                } else {
                    let bkt_ptr = if tgt_ptr.is_dirty() {
                        // If the bkt is dirty, we can just manipulate it.
                        tgt_ptr
                    } else {
                        self.last_seen.push(tgt_ptr);
                        let dbkt_ptr = new_dirty_bucket_ptr::<K, V>();
                        self.first_seen.push(dbkt_ptr);

                        let dbkt = dbkt_ptr.as_bucket_mut::<K, V>();
                        tgt_ptr.as_bucket().iter().for_each(|datum| {
                            dbkt.push(datum.clone());
                        });
                        bref.nodes[idx] = dbkt_ptr;
                        dbkt_ptr
                    };

                    // Handle duplicate K?
                    let bkt = bkt_ptr.as_bucket_mut::<K, V>();
                    for (i, datum) in bkt.iter().enumerate() {
                        if datum.h == h && k.eq(datum.k.borrow()) {
                            // Found, remove.
                            let Datum { v, .. } = bkt.remove(i);
                            self.length -= 1;
                            return Some(v);
                        }
                    }
                    return None;
                }
            }
        }
        unreachable!();
    }

    pub(crate) unsafe fn get_slot_mut_ref(&mut self, h: u64) -> Option<&mut [Datum<K, V>]> {
        self.dirty_root();
        let mut node = self.root;

        for d in 0..MAX_HEIGHT {
            debug_assert!(node.is_dirty());
            debug_assert!(node.is_branch());
            // In the current node
            let bref: &mut Branch<K, V> = node.as_branch_mut();

            // Get our idx from the node.
            let shift = d * SHIFT;
            let idx = ((h & (HASH_MASK << shift)) >> shift) as usize;
            debug_assert!(idx < HT_CAPACITY);

            let tgt_ptr = bref.nodes[idx];

            // If null
            if tgt_ptr.is_null() {
                // Done!
                return None;
            } else if tgt_ptr.is_branch() {
                if tgt_ptr.is_dirty() {
                    node = tgt_ptr;
                } else {
                    self.last_seen.push(tgt_ptr);
                    let from_bref: &Branch<K, V> = tgt_ptr.as_branch();
                    // Over-write the ptr with our new ptr.
                    let nbrch_ptr = from_bref.clone_dirty();
                    self.first_seen.push(nbrch_ptr);
                    bref.nodes[idx] = nbrch_ptr;
                    // next ptr is our new branch, we let the loop continue.
                    node = nbrch_ptr;
                }
            } else {
                debug_assert!(!tgt_ptr.as_bucket::<K, V>().is_empty());
                let bkt_ptr = if tgt_ptr.is_dirty() {
                    // If the bkt is dirty, we can just manipulate it.
                    tgt_ptr
                } else {
                    self.last_seen.push(tgt_ptr);
                    let dbkt_ptr = new_dirty_bucket_ptr::<K, V>();
                    self.first_seen.push(dbkt_ptr);

                    let dbkt = dbkt_ptr.as_bucket_mut::<K, V>();
                    tgt_ptr.as_bucket().iter().for_each(|datum| {
                        dbkt.push(datum.clone());
                    });
                    bref.nodes[idx] = dbkt_ptr;
                    dbkt_ptr
                };

                // Handle duplicate K?
                let bkt = bkt_ptr.as_bucket_mut::<K, V>();
                let x = bkt.as_mut_slice() as *mut [Datum<K, V>];
                return Some(&mut *x as &mut [Datum<K, V>]);
            }
        }
        unreachable!();
    }

    pub(crate) fn get_mut_ref(&mut self, h: u64, k: &K) -> Option<&mut V> {
        unsafe { self.get_slot_mut_ref(h) }.and_then(|bkt| {
            bkt.iter_mut()
                .filter_map(|datum| {
                    if datum.h == h && k.eq(datum.k.borrow()) {
                        let x = &mut datum.v as *mut V;
                        Some(unsafe { &mut *x as &mut V })
                    } else {
                        None
                    }
                })
                .next()
        })
    }

    pub(crate) fn clear(&mut self) {
        // First clear last_seen, since we can't have any duplicates.
        // self.last_seen.clear();
        // self.first_seen.clear();

        let mut stack = VecDeque::new();
        stack.push_back(self.root);

        while let Some(tgt_ptr) = stack.pop_front() {
            self.last_seen.push(tgt_ptr);
            if tgt_ptr.is_branch() {
                for n in tgt_ptr.as_branch::<K, V>().nodes.iter() {
                    if !n.is_null() {
                        self.last_seen.push(*n);
                    }
                }
            }
        }

        // Now make a new root.
        let b: Box<Branch<K, V>> = Branch::new();
        self.root = Ptr::from(b);
        self.root.mark_dirty();
        self.length = 0;
    }
}

impl<K: Clone + Hash + Eq + Debug, V: Clone> Extend<(K, V)> for CursorWrite<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        iter.into_iter().for_each(|(k, v)| {
            let h = self.hash_key(&k);
            let _ = self.insert(h, k, v);
        });
    }
}

impl<K: Clone + Hash + Eq + Debug, V: Clone> Drop for CursorWrite<K, V> {
    fn drop(&mut self) {
        self.first_seen.iter().for_each(|p| p.free::<K, V>())
    }
}

impl<K: Clone + Hash + Eq + Debug, V: Clone> CursorReadOps<K, V> for CursorWrite<K, V> {
    fn get_root_ptr(&self) -> Ptr {
        self.root
    }

    fn len(&self) -> usize {
        self.length
    }

    fn get_txid(&self) -> u64 {
        self.txid
    }

    fn hash_key<Q>(&self, k: &Q) -> u64
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        hash_key!(self, k)
    }

    fn verify(&self) {
        self.verify_inner(false);
    }
}

#[derive(Debug)]
pub(crate) struct CursorRead<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    txid: u64,
    length: usize,
    root: Ptr,
    last_seen: Mutex<Vec<Ptr>>,
    build_hasher: RandomState,
    k: PhantomData<K>,
    v: PhantomData<V>,
}

impl<K: Clone + Hash + Eq + Debug, V: Clone> CursorRead<K, V> {
    pub(crate) fn new(sblock: &SuperBlock<K, V>) -> Self {
        CursorRead {
            txid: sblock.txid,
            length: sblock.length,
            root: sblock.root,
            last_seen: Mutex::new(Vec::with_capacity(0)),
            build_hasher: sblock.build_hasher.to_owned(),
            k: PhantomData,
            v: PhantomData,
        }
    }
}

impl<K: Clone + Hash + Eq + Debug, V: Clone> Drop for CursorRead<K, V> {
    fn drop(&mut self) {
        let last_seen_guard = self
            .last_seen
            .try_lock()
            .expect("Unable to lock, something is horridly wrong!");
        last_seen_guard.iter().for_each(|p| p.free::<K, V>());
        std::mem::drop(last_seen_guard);
    }
}

impl<K: Clone + Hash + Eq + Debug, V: Clone> CursorReadOps<K, V> for CursorRead<K, V> {
    fn get_root_ptr(&self) -> Ptr {
        self.root
    }

    fn len(&self) -> usize {
        self.length
    }

    fn get_txid(&self) -> u64 {
        self.txid
    }

    fn hash_key<Q>(&self, k: &Q) -> u64
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        hash_key!(self, k)
    }

    fn verify(&self) {
        self.verify_inner(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hashtrie_cursor_basic() {
        let sb: SuperBlock<u64, u64> = unsafe { SuperBlock::new() };

        let mut wr = sb.create_writer();

        assert!(wr.len() == 0);
        assert!(wr.search(0, &0).is_none());
        assert!(wr.insert(0, 0, 0).is_none());
        assert!(wr.len() == 1);
        assert!(wr.search(0, &0).is_some());

        assert!(wr.search(1, &1).is_none());
        assert!(wr.insert(1, 1, 0).is_none());
        assert!(wr.search(1, &1).is_some());
        assert!(wr.len() == 2);

        std::mem::drop(wr);
        std::mem::drop(sb);
        assert_released();
    }

    #[test]
    fn test_hashtrie_cursor_insert_max_depth() {
        let mut sb: SuperBlock<u64, u64> = unsafe { SuperBlock::new() };
        let rdr = sb.create_reader();
        let mut wr = sb.create_writer();

        assert!(wr.len() == 0);
        for i in 0..(ABS_MAX_HEIGHT * 2) {
            // This pretty much stresses every (dirty) insert case.
            assert!(wr.insert(0, i, i).is_none());
            wr.verify();
        }
        assert!(wr.len() == (ABS_MAX_HEIGHT * 2) as usize);

        for i in 0..(ABS_MAX_HEIGHT * 2) {
            assert!(wr.search(0, &i).is_some());
        }

        for i in 0..(ABS_MAX_HEIGHT * 2) {
            assert!(wr.remove(0, &i).is_some());
            wr.verify();
        }
        assert!(wr.len() == 0);

        let rdr2 = sb.pre_commit(wr, &rdr);

        rdr2.verify();
        rdr.verify();

        std::mem::drop(rdr);
        rdr2.verify();
        std::mem::drop(rdr2);
        std::mem::drop(sb);
        assert_released();
    }

    #[test]
    fn test_hashtrie_cursor_insert_broad() {
        let mut sb: SuperBlock<u64, u64> = unsafe { SuperBlock::new() };
        let rdr = sb.create_reader();
        let mut wr = sb.create_writer();

        assert!(wr.len() == 0);
        for i in 0..(ABS_MAX_HEIGHT * ABS_MAX_HEIGHT) {
            assert!(wr.insert(i, i, i).is_none());
            wr.verify();
        }

        assert!(wr.len() == (ABS_MAX_HEIGHT * ABS_MAX_HEIGHT) as usize);
        for i in 0..(ABS_MAX_HEIGHT * ABS_MAX_HEIGHT) {
            assert!(wr.search(i, &i).is_some());
        }

        for i in 0..(ABS_MAX_HEIGHT * ABS_MAX_HEIGHT) {
            assert!(wr.remove(i, &i).is_some());
            wr.verify();
        }
        assert!(wr.len() == 0);

        let rdr2 = sb.pre_commit(wr, &rdr);

        rdr2.verify();
        rdr.verify();

        std::mem::drop(rdr);
        rdr2.verify();
        std::mem::drop(rdr2);
        std::mem::drop(sb);
        assert_released();
    }

    #[test]
    fn test_hashtrie_cursor_insert_multiple_txns() {
        let mut sb: SuperBlock<u64, u64> = unsafe { SuperBlock::new() };
        let mut rdr = sb.create_reader();

        // Do thing
        assert!(rdr.len() == 0);

        for i in 0..(ABS_MAX_HEIGHT * ABS_MAX_HEIGHT) {
            let mut wr = sb.create_writer();
            assert!(wr.insert(i, i, i).is_none());
            wr.verify();
            rdr = sb.pre_commit(wr, &rdr);
        }

        {
            let rdr2 = sb.create_reader();
            assert!(rdr2.len() == (ABS_MAX_HEIGHT * ABS_MAX_HEIGHT) as usize);
            for i in 0..(ABS_MAX_HEIGHT * ABS_MAX_HEIGHT) {
                assert!(rdr2.search(i, &i).is_some());
            }
        }

        for i in 0..(ABS_MAX_HEIGHT * ABS_MAX_HEIGHT) {
            let mut wr = sb.create_writer();
            assert!(wr.remove(i, &i).is_some());
            wr.verify();
            rdr = sb.pre_commit(wr, &rdr);
        }

        assert!(rdr.len() == 0);

        rdr.verify();
        std::mem::drop(rdr);
        std::mem::drop(sb);
        assert_released();
    }
}
