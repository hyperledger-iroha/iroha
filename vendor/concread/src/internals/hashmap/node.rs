use super::cursor::Datum;
use super::simd::*;
use super::states::*;
use crate::utils::*;
use crossbeam_utils::CachePadded;
use std::borrow::Borrow;
use std::fmt::{self, Debug, Error};
use std::hash::Hash;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::mem::MaybeUninit;
use std::ptr;

use smallvec::SmallVec;

#[cfg(feature = "simd_support")]
use core_simd::u64x8;

#[cfg(test)]
use std::collections::BTreeSet;
#[cfg(all(test, not(miri)))]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(all(test, not(miri)))]
use std::sync::Mutex;

pub(crate) const TXID_MASK: u64 = 0x0fff_ffff_ffff_fff0;
const FLAG_MASK: u64 = 0xf000_0000_0000_0000;
const COUNT_MASK: u64 = 0x0000_0000_0000_000f;
pub(crate) const TXID_SHF: usize = 4;
// const FLAG__BRANCH: u64 = 0x1000_0000_0000_0000;
// const FLAG__LEAF: u64 = 0x2000_0000_0000_0000;
const FLAG_HASH_LEAF: u64 = 0x4000_0000_0000_0000;
const FLAG_HASH_BRANCH: u64 = 0x8000_0000_0000_0000;
const FLAG_DROPPED: u64 = 0xeeee_ffff_aaaa_bbbb;
#[cfg(all(test, not(miri)))]
const FLAG_POISON: u64 = 0xabcd_abcd_abcd_abcd;

pub(crate) const H_CAPACITY: usize = 7;
const H_CAPACITY_N1: usize = H_CAPACITY - 1;
pub(crate) const HBV_CAPACITY: usize = H_CAPACITY + 1;

const DEFAULT_BUCKET_ALLOC: usize = 1;

#[cfg(not(feature = "simd_support"))]
#[allow(non_camel_case_types)]
pub struct u64x8 {
    _data: [u64; 8],
}

#[cfg(not(feature = "simd_support"))]
impl u64x8 {
    #[inline(always)]
    fn from_array(data: [u64; 8]) -> Self {
        Self { _data: data }
    }
}

#[cfg(all(test, not(miri)))]
thread_local!(static NODE_COUNTER: AtomicUsize = const { AtomicUsize::new(1) });
#[cfg(all(test, not(miri)))]
thread_local!(static ALLOC_LIST: Mutex<BTreeSet<usize>> = const { Mutex::new(BTreeSet::new()) });

#[cfg(all(test, not(miri)))]
fn alloc_nid() -> usize {
    let nid: usize = NODE_COUNTER.with(|nc| nc.fetch_add(1, Ordering::AcqRel));
    #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
    {
        ALLOC_LIST.with(|llist| llist.lock().unwrap().insert(nid));
    }
    nid
}

#[cfg(all(test, not(miri)))]
fn release_nid(nid: usize) {
    #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
    {
        let r = ALLOC_LIST.with(|llist| llist.lock().unwrap().remove(&nid));
        assert!(r);
    }
    #[cfg(all(test, not(miri), feature = "dhat-heap"))]
    {
        dbg!(nid);
    }
}

#[cfg(test)]
pub(crate) fn assert_released() {
    #[cfg(all(test, not(miri), not(feature = "dhat-heap")))]
    {
        let is_empt = ALLOC_LIST.with(|llist| {
            let x = llist.lock().unwrap();
            // println!("Remaining -> {:?}", x);
            x.is_empty()
        });
        assert!(is_empt);
    }
}

#[repr(C)]
pub(crate) struct Meta(u64);

pub(crate) union Ctrl {
    pub a: ManuallyDrop<(Meta, [u64; H_CAPACITY])>,
    pub simd: ManuallyDrop<u64x8>,
}

#[repr(C, align(64))]
pub(crate) struct Branch<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    pub ctrl: Ctrl,
    #[cfg(all(test, not(miri)))]
    poison: u64,
    nodes: [*mut Node<K, V>; HBV_CAPACITY],
    #[cfg(all(test, not(miri)))]
    pub(crate) nid: usize,
}

type Bucket<K, V> = SmallVec<[Datum<K, V>; DEFAULT_BUCKET_ALLOC]>;

#[repr(C, align(64))]
pub(crate) struct Leaf<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    pub ctrl: Ctrl,
    #[cfg(all(test, not(miri)))]
    poison: u64,
    pub values: [MaybeUninit<Bucket<K, V>>; H_CAPACITY],
    #[cfg(all(test, not(miri)))]
    pub nid: usize,
}

#[repr(C, align(64))]
pub(crate) struct Node<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    pub(crate) ctrl: Ctrl,
    k: PhantomData<K>,
    v: PhantomData<V>,
}

unsafe impl<K: Clone + Hash + Eq + Debug + Send + 'static, V: Clone + Send + 'static> Send
    for Node<K, V>
{
}
unsafe impl<K: Clone + Hash + Eq + Debug + Send + 'static, V: Clone + Sync + Send + 'static> Sync
    for Node<K, V>
{
}

impl<K: Clone + Eq + Hash + Debug, V: Clone> Node<K, V> {
    pub(crate) fn new_leaf(txid: u64) -> *mut Leaf<K, V> {
        // println!("Req new hash leaf");
        debug_assert!(txid < (TXID_MASK >> TXID_SHF));
        let x: Box<CachePadded<Leaf<K, V>>> = Box::new(CachePadded::new(Leaf {
            ctrl: Ctrl {
                simd: ManuallyDrop::new(u64x8::from_array([
                    (txid << TXID_SHF) | FLAG_HASH_LEAF,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                ])),
            },
            #[cfg(all(test, not(miri)))]
            poison: FLAG_POISON,
            values: unsafe { MaybeUninit::uninit().assume_init() },
            #[cfg(all(test, not(miri)))]
            nid: alloc_nid(),
        }));
        Box::into_raw(x) as *mut Leaf<K, V>
    }

    fn new_leaf_bk(flags: u64, h: u64, bk: Bucket<K, V>) -> *mut Leaf<K, V> {
        // println!("Req new hash leaf ins");
        // debug_assert!(false);
        debug_assert!((flags & FLAG_MASK) == FLAG_HASH_LEAF);
        let x: Box<CachePadded<Leaf<K, V>>> = Box::new(CachePadded::new(Leaf {
            // Let the flag, txid and the slots of value 1 through.
            ctrl: Ctrl {
                simd: ManuallyDrop::new(u64x8::from_array([
                    flags & (TXID_MASK | FLAG_MASK | 1),
                    h,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                ])),
            },
            #[cfg(all(test, not(miri)))]
            poison: FLAG_POISON,
            values: [
                MaybeUninit::new(bk),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
            ],
            #[cfg(all(test, not(miri)))]
            nid: alloc_nid(),
        }));
        Box::into_raw(x) as *mut Leaf<K, V>
    }

    fn new_leaf_ins(flags: u64, h: u64, k: K, v: V) -> *mut Leaf<K, V> {
        Self::new_leaf_bk(flags, h, smallvec![Datum { k, v }])
    }

    pub(crate) fn new_branch(
        txid: u64,
        l: *mut Node<K, V>,
        r: *mut Node<K, V>,
    ) -> *mut Branch<K, V> {
        // println!("Req new branch");
        debug_assert!(!l.is_null());
        debug_assert!(!r.is_null());
        debug_assert!(unsafe { (*l).verify() });
        debug_assert!(unsafe { (*r).verify() });
        debug_assert!(txid < (TXID_MASK >> TXID_SHF));
        let pivot = unsafe { (*r).min() };
        let x: Box<CachePadded<Branch<K, V>>> = Box::new(CachePadded::new(Branch {
            // This sets the default (key) slots to 1, since we take an l/r
            ctrl: Ctrl {
                simd: ManuallyDrop::new(u64x8::from_array([
                    (txid << TXID_SHF) | FLAG_HASH_BRANCH | 1,
                    pivot,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                ])),
            },
            #[cfg(all(test, not(miri)))]
            poison: FLAG_POISON,
            nodes: [
                l,
                r,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            ],
            #[cfg(all(test, not(miri)))]
            nid: alloc_nid(),
        }));
        let b = Box::into_raw(x) as *mut Branch<K, V>;
        debug_assert!(unsafe { (*b).verify() });
        b
    }

    #[inline(always)]
    #[cfg(test)]
    pub(crate) fn get_txid(&self) -> u64 {
        unsafe { self.ctrl.a.0.get_txid() }
    }

    #[inline(always)]
    pub(crate) fn is_leaf(&self) -> bool {
        unsafe { self.ctrl.a.0.is_leaf() }
    }

    #[inline(always)]
    #[allow(unused)]
    pub(crate) fn is_branch(&self) -> bool {
        unsafe { self.ctrl.a.0.is_branch() }
    }

    #[cfg(test)]
    pub(crate) fn tree_density(&self) -> (usize, usize, usize) {
        match unsafe { self.ctrl.a.0 .0 } & FLAG_MASK {
            FLAG_HASH_LEAF => {
                let lref = unsafe { &*(self as *const _ as *const Leaf<K, V>) };
                (lref.count(), lref.slots(), H_CAPACITY)
            }
            FLAG_HASH_BRANCH => {
                let bref = unsafe { &*(self as *const _ as *const Branch<K, V>) };
                let mut lcount = 0; // leaf count
                let mut lslots = 0; // leaf populated slots
                let mut mslots = 0; // leaf max possible
                for idx in 0..(bref.slots() + 1) {
                    let n = bref.nodes[idx];
                    let (c, l, m) = unsafe { (*n).tree_density() };
                    lcount += c;
                    lslots += l;
                    mslots += m;
                }
                (lcount, lslots, mslots)
            }
            _ => unreachable!(),
        }
    }

    pub(crate) fn leaf_count(&self) -> usize {
        match unsafe { self.ctrl.a.0 .0 } & FLAG_MASK {
            FLAG_HASH_LEAF => 1,
            FLAG_HASH_BRANCH => {
                let bref = unsafe { &*(self as *const _ as *const Branch<K, V>) };
                let mut lcount = 0; // leaf count
                for idx in 0..(bref.slots() + 1) {
                    let n = bref.nodes[idx];
                    lcount += unsafe { (*n).leaf_count() };
                }
                lcount
            }
            _ => unreachable!(),
        }
    }

    #[cfg(test)]
    #[inline(always)]
    pub(crate) fn get_ref<Q: ?Sized + Eq>(&self, h: u64, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        match unsafe { self.ctrl.a.0 .0 } & FLAG_MASK {
            FLAG_HASH_LEAF => {
                let lref = unsafe { &*(self as *const _ as *const Leaf<K, V>) };
                lref.get_ref(h, k)
            }
            FLAG_HASH_BRANCH => {
                let bref = unsafe { &*(self as *const _ as *const Branch<K, V>) };
                bref.get_ref(h, k)
            }
            _ => {
                // println!("FLAGS: {:x}", self.meta.0);
                unreachable!()
            }
        }
    }

    #[inline(always)]
    pub(crate) fn min(&self) -> u64 {
        match unsafe { self.ctrl.a.0 .0 } & FLAG_MASK {
            FLAG_HASH_LEAF => {
                let lref = unsafe { &*(self as *const _ as *const Leaf<K, V>) };
                lref.min()
            }
            FLAG_HASH_BRANCH => {
                let bref = unsafe { &*(self as *const _ as *const Branch<K, V>) };
                bref.min()
            }
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    pub(crate) fn max(&self) -> u64 {
        match unsafe { self.ctrl.a.0 .0 } & FLAG_MASK {
            FLAG_HASH_LEAF => {
                let lref = unsafe { &*(self as *const _ as *const Leaf<K, V>) };
                lref.max()
            }
            FLAG_HASH_BRANCH => {
                let bref = unsafe { &*(self as *const _ as *const Branch<K, V>) };
                bref.max()
            }
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    pub(crate) fn verify(&self) -> bool {
        match unsafe { self.ctrl.a.0 .0 } & FLAG_MASK {
            FLAG_HASH_LEAF => {
                let lref = unsafe { &*(self as *const _ as *const Leaf<K, V>) };
                lref.verify()
            }
            FLAG_HASH_BRANCH => {
                let bref = unsafe { &*(self as *const _ as *const Branch<K, V>) };
                bref.verify()
            }
            _ => unreachable!(),
        }
    }

    #[cfg(test)]
    fn no_cycles_inner(&self, track: &mut BTreeSet<*const Self>) -> bool {
        match unsafe { self.ctrl.a.0 .0 } & FLAG_MASK {
            FLAG_HASH_LEAF => {
                // check if we are in the set?
                track.insert(self as *const Self)
            }
            FLAG_HASH_BRANCH => {
                if track.insert(self as *const Self) {
                    // check
                    let bref = unsafe { &*(self as *const _ as *const Branch<K, V>) };
                    for i in 0..(bref.slots() + 1) {
                        let n = bref.nodes[i];
                        let r = unsafe { (*n).no_cycles_inner(track) };
                        if !r {
                            // panic!();
                            return false;
                        }
                    }
                    true
                } else {
                    // panic!();
                    false
                }
            }
            _ => {
                // println!("FLAGS: {:x}", self.meta.0);
                unreachable!()
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn no_cycles(&self) -> bool {
        let mut track = BTreeSet::new();
        self.no_cycles_inner(&mut track)
    }

    pub(crate) fn sblock_collect(&mut self, alloc: &mut Vec<*mut Node<K, V>>) {
        // Reset our txid.
        // self.meta.0 &= FLAG_MASK | COUNT_MASK;
        // self.meta.0 |= txid << TXID_SHF;

        if (unsafe { self.ctrl.a.0 .0 } & FLAG_MASK) == FLAG_HASH_BRANCH {
            let bref = unsafe { &*(self as *const _ as *const Branch<K, V>) };
            for idx in 0..(bref.slots() + 1) {
                alloc.push(bref.nodes[idx]);
                let n = bref.nodes[idx];
                unsafe { (*n).sblock_collect(alloc) };
            }
        }
    }

    pub(crate) fn free(node: *mut Node<K, V>) {
        let self_meta = self_meta!(node);
        match self_meta.0 & FLAG_MASK {
            FLAG_HASH_LEAF => Leaf::free(node as *mut Leaf<K, V>),
            FLAG_HASH_BRANCH => Branch::free(node as *mut Branch<K, V>),
            _ => unreachable!(),
        }
    }
}

impl Meta {
    #[inline(always)]
    fn set_slots(&mut self, c: usize) {
        debug_assert!(c < 16);
        // Zero the bits in the flag from the slots.
        self.0 &= FLAG_MASK | TXID_MASK;
        // Assign them.
        self.0 |= c as u8 as u64;
    }

    #[inline(always)]
    pub(crate) fn slots(&self) -> usize {
        (self.0 & COUNT_MASK) as usize
    }

    #[inline(always)]
    fn add_slots(&mut self, x: usize) {
        self.set_slots(self.slots() + x);
    }

    #[inline(always)]
    fn inc_slots(&mut self) {
        debug_assert!(self.slots() < 15);
        // Since slots is the lowest bits, we can just inc
        // dec this as normal.
        self.0 += 1;
    }

    #[inline(always)]
    fn dec_slots(&mut self) {
        debug_assert!(self.slots() > 0);
        self.0 -= 1;
    }

    #[inline(always)]
    pub(crate) fn get_txid(&self) -> u64 {
        (self.0 & TXID_MASK) >> TXID_SHF
    }

    #[inline(always)]
    pub(crate) fn is_leaf(&self) -> bool {
        (self.0 & FLAG_MASK) == FLAG_HASH_LEAF
    }

    #[inline(always)]
    pub(crate) fn is_branch(&self) -> bool {
        (self.0 & FLAG_MASK) == FLAG_HASH_BRANCH
    }
}

impl<K: Hash + Eq + Clone + Debug, V: Clone> Leaf<K, V> {
    #[inline(always)]
    #[cfg(test)]
    fn set_slots(&mut self, c: usize) {
        debug_assert_leaf!(self);
        unsafe { (*self.ctrl.a).0.set_slots(c) }
    }

    #[inline(always)]
    pub(crate) fn slots(&self) -> usize {
        debug_assert_leaf!(self);
        unsafe { self.ctrl.a.0.slots() }
    }

    #[inline(always)]
    fn inc_slots(&mut self) {
        debug_assert_leaf!(self);
        unsafe { (*self.ctrl.a).0.inc_slots() }
    }

    #[inline(always)]
    fn dec_slots(&mut self) {
        debug_assert_leaf!(self);
        unsafe { (*self.ctrl.a).0.dec_slots() }
    }

    #[inline(always)]
    pub(crate) fn get_txid(&self) -> u64 {
        debug_assert_leaf!(self);
        unsafe { self.ctrl.a.0.get_txid() }
    }

    pub(crate) fn count(&self) -> usize {
        let mut c = 0;
        for slot_idx in 0..self.slots() {
            c += unsafe { (*self.values[slot_idx].as_ptr()).len() };
        }
        c
    }

    pub(crate) fn get_ref<Q>(&self, h: u64, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + ?Sized,
    {
        debug_assert_leaf!(self);
        leaf_simd_search(self, h, k)
            .ok()
            .map(|(slot_idx, bk_idx)| unsafe {
                let bucket = (*self.values[slot_idx].as_ptr()).as_slice();
                &(bucket.get_unchecked(bk_idx).v)
            })
    }

    pub(crate) fn get_mut_ref<Q>(&mut self, h: u64, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Eq + ?Sized,
    {
        debug_assert_leaf!(self);
        leaf_simd_search(self, h, k)
            .ok()
            .map(|(slot_idx, bk_idx)| unsafe {
                let bucket = (*self.values[slot_idx].as_mut_ptr()).as_mut_slice();
                &mut (bucket.get_unchecked_mut(bk_idx).v)
            })
    }

    pub(crate) fn get_slot_mut_ref<Q>(&mut self, h: u64) -> Option<&mut [Datum<K, V>]>
    where
        K: Borrow<Q>,
        Q: Eq + ?Sized,
    {
        debug_assert_leaf!(self);
        unsafe {
            leaf_simd_get_slot(self, h)
                .map(|slot_idx| (*self.values[slot_idx].as_mut_ptr()).as_mut_slice())
        }
    }

    #[inline(always)]
    pub(crate) fn get_kv_idx_checked(&self, slot_idx: usize, bk_idx: usize) -> Option<(&K, &V)> {
        debug_assert_leaf!(self);
        if slot_idx < self.slots() {
            let bucket = unsafe { (*self.values[slot_idx].as_ptr()).as_slice() };
            bucket.get(bk_idx).map(|d| (&d.k, &d.v))
        } else {
            None
        }
    }

    pub(crate) fn min(&self) -> u64 {
        debug_assert!(self.slots() > 0);
        unsafe { self.ctrl.a.1[0] }
    }

    pub(crate) fn max(&self) -> u64 {
        debug_assert!(self.slots() > 0);
        unsafe { self.ctrl.a.1[self.slots() - 1] }
    }

    pub(crate) fn req_clone(&self, txid: u64) -> Option<*mut Node<K, V>> {
        debug_assert_leaf!(self);
        debug_assert!(txid < (TXID_MASK >> TXID_SHF));
        if self.get_txid() == txid {
            // Same txn, no action needed.
            None
        } else {
            // println!("Req clone leaf");
            // debug_assert!(false);
            // Diff txn, must clone.
            let new_txid =
                (unsafe { self.ctrl.a.0 .0 } & (FLAG_MASK | COUNT_MASK)) | (txid << TXID_SHF);
            let x: Box<CachePadded<Leaf<K, V>>> = Box::new(CachePadded::new(Leaf {
                ctrl: Ctrl {
                    simd: ManuallyDrop::new(u64x8::from_array([
                        new_txid,
                        u64::MAX,
                        u64::MAX,
                        u64::MAX,
                        u64::MAX,
                        u64::MAX,
                        u64::MAX,
                        u64::MAX,
                    ])),
                },
                #[cfg(all(test, not(miri)))]
                poison: FLAG_POISON,
                values: unsafe { MaybeUninit::uninit().assume_init() },
                #[cfg(all(test, not(miri)))]
                nid: alloc_nid(),
            }));

            let x = Box::into_raw(x);
            let xr = x as *mut Leaf<K, V>;
            // Dup the keys
            unsafe {
                ptr::copy_nonoverlapping(
                    &self.ctrl.a.1 as *const u64,
                    (*(*xr).ctrl.a).1.as_mut_ptr(),
                    H_CAPACITY,
                )
            }

            // Copy in the values to the correct location.
            for idx in 0..self.slots() {
                unsafe {
                    let lvalue: Bucket<K, V> = (*self.values[idx].as_ptr()).clone();
                    (&mut (*x)).values[idx].as_mut_ptr().write(lvalue);
                }
            }

            Some(x as *mut Node<K, V>)
        }
    }

    pub(crate) fn insert_or_update(&mut self, h: u64, k: K, mut v: V) -> LeafInsertState<K, V> {
        debug_assert_leaf!(self);
        // Find the location we need to update
        let r = leaf_simd_search(self, h, &k);
        match r {
            KeyLoc::Ok(slot_idx, bk_idx) => {
                // It exists at idx, replace the value.
                let bucket = unsafe { &mut (*self.values[slot_idx].as_mut_ptr()) };
                let prev = unsafe { bucket.as_mut_slice().get_unchecked_mut(bk_idx) };
                std::mem::swap(&mut prev.v, &mut v);
                // Prev now contains the original value, return it!
                LeafInsertState::Ok(Some(v))
            }
            KeyLoc::Collision(slot_idx) => {
                // The hash collided, but that's okay! We just append to the slice.
                let bucket = unsafe { &mut (*self.values[slot_idx].as_mut_ptr()) };
                bucket.push(Datum { k, v });
                LeafInsertState::Ok(None)
            }
            KeyLoc::Missing(idx) => {
                if self.slots() >= H_CAPACITY {
                    // Overflow to a new node
                    if idx >= self.slots() {
                        // Greater than all else, split right
                        let rnode = Node::new_leaf_ins(unsafe { self.ctrl.a.0 .0 }, h, k, v);
                        LeafInsertState::Split(rnode)
                    } else if idx == 0 {
                        // Lower than all else, split left.
                        // let lnode = ...;
                        let lnode = Node::new_leaf_ins(unsafe { self.ctrl.a.0 .0 }, h, k, v);
                        LeafInsertState::RevSplit(lnode)
                    } else {
                        // Within our range, pop max, insert, and split right.
                        // This is not a bucket add, it's a new bucket!
                        let pk = unsafe { slice_remove(&mut (*self.ctrl.a).1, H_CAPACITY - 1) };
                        let pbk =
                            unsafe { slice_remove(&mut self.values, H_CAPACITY - 1).assume_init() };

                        let rnode = Node::new_leaf_bk(unsafe { self.ctrl.a.0 .0 }, pk, pbk);

                        unsafe {
                            slice_insert(&mut (*self.ctrl.a).1, h, idx);
                            slice_insert(
                                &mut self.values,
                                MaybeUninit::new(smallvec![Datum { k, v }]),
                                idx,
                            );
                        }

                        #[cfg(all(test, not(miri)))]
                        debug_assert!(self.poison == FLAG_POISON);

                        LeafInsertState::Split(rnode)
                    }
                } else {
                    // We have space.
                    unsafe {
                        // self.key[idx] = h;
                        slice_insert(&mut (*self.ctrl.a).1, h, idx);
                        slice_insert(
                            &mut self.values,
                            MaybeUninit::new(smallvec![Datum { k, v }]),
                            idx,
                        );
                    }
                    #[cfg(all(test, not(miri)))]
                    debug_assert!(self.poison == FLAG_POISON);
                    self.inc_slots();
                    LeafInsertState::Ok(None)
                }
            }
        }
    }

    pub(crate) fn remove<Q>(&mut self, h: u64, k: &Q) -> LeafRemoveState<V>
    where
        K: Borrow<Q>,
        Q: Eq + ?Sized,
    {
        debug_assert_leaf!(self);
        if self.slots() == 0 {
            return LeafRemoveState::Shrink(None);
        }
        // We must have a value - where are you ....
        match leaf_simd_search(self, h, k).ok() {
            // Count still greater than 0, so Ok and None,
            None => LeafRemoveState::Ok(None),
            Some((slot_idx, bk_idx)) => {
                // pop from the bucket.
                let Datum { k: _pk, v: pv } =
                    unsafe { (*self.values[slot_idx].as_mut_ptr()).remove(bk_idx) };

                // How much remains?
                if unsafe { (*self.values[slot_idx].as_ptr()).is_empty() } {
                    unsafe {
                        // Get the kv out
                        let _ = slice_remove(&mut (*self.ctrl.a).1, slot_idx);
                        // AFTER the remove, set the top value to u64::MAX
                        (*self.ctrl.a).1[H_CAPACITY - 1] = u64::MAX;

                        // Remove the bucket.
                        let _ = slice_remove(&mut self.values, slot_idx).assume_init();
                    }

                    self.dec_slots();
                    if self.slots() == 0 {
                        LeafRemoveState::Shrink(Some(pv))
                    } else {
                        LeafRemoveState::Ok(Some(pv))
                    }
                } else {
                    // The bucket lives!
                    LeafRemoveState::Ok(Some(pv))
                }
            }
        }
    }

    pub(crate) fn take_from_l_to_r(&mut self, right: &mut Self) {
        debug_assert!(right.slots() == 0);
        let slots = self.slots() / 2;
        let start_idx = self.slots() - slots;

        //move key and values
        unsafe {
            slice_move(
                &mut (*right.ctrl.a).1,
                0,
                &mut (*self.ctrl.a).1,
                start_idx,
                slots,
            );
            slice_move(&mut right.values, 0, &mut self.values, start_idx, slots);
            // Update the left keys to be valid.
            // so we took from:
            //  [ a, b, c, d, e, f, g ]
            //                ^     ^
            //                |     slots
            //                start
            //  so we need to fill from start_idx + slots
            let tgt_ptr = (*self.ctrl.a).1.as_mut_ptr().add(start_idx);
            // https://doc.rust-lang.org/std/ptr/fn.write_bytes.html
            // Sets count * size_of::<T>() bytes of memory starting at dst to val.
            ptr::write_bytes::<u64>(tgt_ptr, 0xff, slots);
            #[cfg(all(test, not(miri)))]
            debug_assert!(self.poison == FLAG_POISON);
        }

        // update the slotss
        unsafe {
            (*self.ctrl.a).0.set_slots(start_idx);
            (*right.ctrl.a).0.set_slots(slots);
        }
    }

    pub(crate) fn take_from_r_to_l(&mut self, right: &mut Self) {
        debug_assert!(self.slots() == 0);
        let slots = right.slots() / 2;
        let start_idx = right.slots() - slots;

        // Move values from right to left.
        unsafe {
            slice_move(&mut (*self.ctrl.a).1, 0, &mut (*right.ctrl.a).1, 0, slots);
            slice_move(&mut self.values, 0, &mut right.values, 0, slots);
        }
        // Shift the values in right down.
        unsafe {
            ptr::copy(
                (*right.ctrl.a).1.as_ptr().add(slots),
                (*right.ctrl.a).1.as_mut_ptr(),
                start_idx,
            );
            ptr::copy(
                right.values.as_ptr().add(slots),
                right.values.as_mut_ptr(),
                start_idx,
            );
        }

        // Fix the slotss.
        unsafe {
            (*self.ctrl.a).0.set_slots(slots);
            (*right.ctrl.a).0.set_slots(start_idx);
        }
        // Update the upper keys in right
        unsafe {
            let tgt_ptr = (*right.ctrl.a).1.as_mut_ptr().add(start_idx);
            // https://doc.rust-lang.org/std/ptr/fn.write_bytes.html
            // Sets count * size_of::<T>() bytes of memory starting at dst to val.
            ptr::write_bytes::<u64>(tgt_ptr, 0xff, H_CAPACITY - start_idx);
            #[cfg(all(test, not(miri)))]
            debug_assert!(right.poison == FLAG_POISON);
        }
    }

    /*
    pub(crate) fn remove_lt<Q: ?Sized>(&mut self, k: &Q) -> LeafPruneState<V>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        unimplemented!();
    }
    */

    #[inline(always)]
    pub(crate) fn merge(&mut self, right: &mut Self) {
        debug_assert_leaf!(self);
        debug_assert_leaf!(right);
        let sc = self.slots();
        let rc = right.slots();
        unsafe {
            slice_merge(&mut (*self.ctrl.a).1, sc, &mut (*right.ctrl.a).1, rc);
            slice_merge(&mut self.values, sc, &mut right.values, rc);
        }
        #[cfg(all(test, not(miri)))]
        debug_assert!(self.poison == FLAG_POISON);
        unsafe {
            (*self.ctrl.a).0.add_slots(right.count());
            (*right.ctrl.a).0.set_slots(0);
        }
        debug_assert!(self.verify());
    }

    pub(crate) fn verify(&self) -> bool {
        debug_assert_leaf!(self);
        #[cfg(all(test, not(miri)))]
        debug_assert!(self.poison == FLAG_POISON);
        // println!("verify leaf -> {:?}", self);
        if unsafe { self.ctrl.a.0.slots() } == 0 {
            return true;
        }
        // Check everything above slots is u64::max
        for work_idx in unsafe { (*self.ctrl.a).0.slots() }..H_CAPACITY {
            debug_assert!(unsafe { (*self.ctrl.a).1[work_idx] } == u64::MAX);
        }
        // Check key sorting
        let mut lk: &u64 = unsafe { &(*self.ctrl.a).1[0] };
        for work_idx in 1..unsafe { self.ctrl.a.0.slots() } {
            let rk: &u64 = unsafe { &(*self.ctrl.a).1[work_idx] };
            // Eq not ok as we have buckets.
            if lk >= rk {
                // println!("{:?}", self);
                if cfg!(test) {
                    return false;
                } else {
                    debug_assert!(false);
                }
            }
            lk = rk;
        }
        true
    }

    fn free(node: *mut Self) {
        unsafe {
            let _x: Box<CachePadded<Leaf<K, V>>> =
                Box::from_raw(node as *mut CachePadded<Leaf<K, V>>);
        }
    }
}

impl<K: Hash + Eq + Clone + Debug, V: Clone> Debug for Leaf<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), Error> {
        debug_assert_leaf!(self);
        write!(f, "Leaf -> {}", self.slots())?;
        #[cfg(all(test, not(miri)))]
        write!(f, " nid: {}", self.nid)?;
        write!(f, "  \\-> [ ")?;
        for idx in 0..self.slots() {
            write!(f, "{:?}, ", unsafe { self.ctrl.a.1[idx] })?;
            write!(f, "[")?;
            for d in unsafe { (*self.values[idx].as_ptr()).as_slice().iter() } {
                write!(f, "{:?}, ", d.k)?;
            }
            write!(f, "], ")?;
        }
        write!(f, " ]")
    }
}

impl<K: Hash + Eq + Clone + Debug, V: Clone> Drop for Leaf<K, V> {
    fn drop(&mut self) {
        debug_assert_leaf!(self);
        #[cfg(all(test, not(miri)))]
        release_nid(self.nid);
        // Due to the use of maybe uninit we have to drop any contained values.
        unsafe {
            for idx in 0..self.slots() {
                ptr::drop_in_place(self.values[idx].as_mut_ptr());
            }
        }
        // Done
        unsafe {
            (*self.ctrl.a).0 .0 = FLAG_DROPPED;
        }
        debug_assert!(unsafe { self.ctrl.a.0 .0 } & FLAG_MASK != FLAG_HASH_LEAF);
        // #[cfg(test)]
        // println!("set leaf {:?} to {:x}", self.nid, self.meta.0);
    }
}

impl<K: Hash + Eq + Clone + Debug, V: Clone> Branch<K, V> {
    #[inline(always)]
    #[allow(unused)]
    fn set_slots(&mut self, c: usize) {
        debug_assert_branch!(self);
        unsafe { (*self.ctrl.a).0.set_slots(c) }
    }

    #[inline(always)]
    pub(crate) fn slots(&self) -> usize {
        debug_assert_branch!(self);
        unsafe { self.ctrl.a.0.slots() }
    }

    #[inline(always)]
    fn inc_slots(&mut self) {
        debug_assert_branch!(self);
        unsafe { (*self.ctrl.a).0.inc_slots() }
    }

    #[inline(always)]
    fn dec_slots(&mut self) {
        debug_assert_branch!(self);
        unsafe { (*self.ctrl.a).0.dec_slots() }
    }

    #[inline(always)]
    pub(crate) fn get_txid(&self) -> u64 {
        debug_assert_branch!(self);
        unsafe { self.ctrl.a.0.get_txid() }
    }

    // Can't inline as this is recursive!
    pub(crate) fn min(&self) -> u64 {
        debug_assert_branch!(self);
        unsafe { (*self.nodes[0]).min() }
    }

    // Can't inline as this is recursive!
    pub(crate) fn max(&self) -> u64 {
        debug_assert_branch!(self);
        // Remember, self.slots() is + 1 offset, so this gets
        // the max node
        unsafe { (*self.nodes[self.slots()]).max() }
    }

    pub(crate) fn req_clone(&self, txid: u64) -> Option<*mut Node<K, V>> {
        debug_assert_branch!(self);
        if self.get_txid() == txid {
            // Same txn, no action needed.
            None
        } else {
            // println!("Req clone branch");
            // Diff txn, must clone.
            let new_txid =
                (unsafe { self.ctrl.a.0 .0 } & (FLAG_MASK | COUNT_MASK)) | (txid << TXID_SHF);

            let x: Box<CachePadded<Branch<K, V>>> = Box::new(CachePadded::new(Branch {
                // This sets the default (key) slots to 1, since we take an l/r
                ctrl: Ctrl {
                    simd: ManuallyDrop::new(u64x8::from_array([
                        new_txid,
                        u64::MAX,
                        u64::MAX,
                        u64::MAX,
                        u64::MAX,
                        u64::MAX,
                        u64::MAX,
                        u64::MAX,
                    ])),
                },
                #[cfg(all(test, not(miri)))]
                poison: FLAG_POISON,
                // Can clone the node pointers.
                nodes: self.nodes,
                #[cfg(all(test, not(miri)))]
                nid: alloc_nid(),
            }));

            let x = Box::into_raw(x);
            let xr = x as *mut Branch<K, V>;
            // Dup the keys
            unsafe {
                ptr::copy_nonoverlapping(
                    &self.ctrl.a.1 as *const u64,
                    (*(*xr).ctrl.a).1.as_mut_ptr(),
                    H_CAPACITY,
                )
            }

            Some(x as *mut Node<K, V>)
        }
    }

    #[inline(always)]
    pub(crate) fn locate_node(&self, h: u64) -> usize {
        debug_assert_branch!(self);
        // If the value is Ok(idx), then that means
        // we were located to the right node. This is because we
        // exactly hit and located on the key.
        //
        // If the value is Err(idx), then we have the exact index already.
        // as branches is of-by-one.
        match branch_simd_search::<K, V>(self, h) {
            Err(idx) => idx,
            Ok(idx) => idx + 1,
        }
    }

    #[inline(always)]
    pub(crate) fn get_idx_unchecked(&self, idx: usize) -> *mut Node<K, V> {
        debug_assert_branch!(self);
        debug_assert!(idx <= self.slots());
        debug_assert!(!self.nodes[idx].is_null());
        self.nodes[idx]
    }

    #[inline(always)]
    pub(crate) fn get_idx_checked(&self, idx: usize) -> Option<*mut Node<K, V>> {
        debug_assert_branch!(self);
        // Remember, that nodes can have +1 to slots which is why <= here, not <.
        if idx <= self.slots() {
            debug_assert!(!self.nodes[idx].is_null());
            Some(self.nodes[idx])
        } else {
            None
        }
    }

    #[cfg(test)]
    pub(crate) fn get_ref<Q: ?Sized + Eq>(&self, h: u64, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        debug_assert_branch!(self);
        let idx = self.locate_node(h);
        unsafe { (*self.nodes[idx]).get_ref(h, k) }
    }

    pub(crate) fn add_node(&mut self, node: *mut Node<K, V>) -> BranchInsertState<K, V> {
        debug_assert_branch!(self);
        // do we have space?
        if self.slots() == H_CAPACITY {
            // if no space ->
            //    split and send two nodes back for new branch
            // There are three possible states that this causes.
            // 1 * The inserted node is the greater than all current values, causing l(max, node)
            //     to be returned.
            // 2 * The inserted node is between max - 1 and max, causing l(node, max) to be returned.
            // 3 * The inserted node is a low/middle value, causing max and max -1 to be returned.
            //
            let kr: u64 = unsafe { (*node).min() };
            let r = branch_simd_search(self, kr);
            let ins_idx = r.unwrap_err();
            // Everything will pop max.
            let max = unsafe { *(self.nodes.get_unchecked(HBV_CAPACITY - 1)) };
            let res = match ins_idx {
                // Case 1
                H_CAPACITY => {
                    // println!("case 1");
                    // Greater than all current values, so we'll just return max and node.
                    unsafe {
                        (*self.ctrl.a).1[H_CAPACITY - 1] = u64::MAX;
                    } // Now setup the ret val NOTICE compared to case 2 that we swap node and max?
                    BranchInsertState::Split(max, node)
                }
                // Case 2
                H_CAPACITY_N1 => {
                    // println!("case 2");
                    // Greater than all but max, so we return max and node in the correct order.
                    // Drop the key between them.
                    unsafe {
                        (*self.ctrl.a).1[H_CAPACITY - 1] = u64::MAX;
                    }
                    // Now setup the ret val NOTICE compared to case 1 that we swap node and max?
                    BranchInsertState::Split(node, max)
                }
                // Case 3
                ins_idx => {
                    // Get the max - 1 and max nodes out.
                    let maxn1 = unsafe { *(self.nodes.get_unchecked(HBV_CAPACITY - 2)) };
                    unsafe {
                        // Drop the key between them.
                        (*self.ctrl.a).1[H_CAPACITY - 1] = u64::MAX;
                        // Drop the key before us that we are about to replace.
                        (*self.ctrl.a).1[H_CAPACITY - 2] = u64::MAX;
                    }
                    // Add node and it's key to the correct location.
                    let leaf_ins_idx = ins_idx + 1;
                    unsafe {
                        slice_insert(&mut (*self.ctrl.a).1, kr, ins_idx);
                        slice_insert(&mut self.nodes, node, leaf_ins_idx);
                    }
                    #[cfg(all(test, not(miri)))]
                    debug_assert!(self.poison == FLAG_POISON);

                    BranchInsertState::Split(maxn1, max)
                }
            };
            // Dec slots as we always reduce branch by one as we split return
            // two.
            self.dec_slots();
            res
        } else {
            // if space ->
            // Get the nodes min-key - we clone it because we'll certainly be inserting it!
            let k: u64 = unsafe { (*node).min() };
            // bst and find when min-key < key[idx]
            let r = branch_simd_search(self, k);
            // if r is ever found, I think this is a bug, because we should never be able to
            // add a node with an existing min.
            //
            //       [ 5 ]
            //        / \
            //    [0,]   [5,]
            //
            // So if we added here to [0, ], and it had to overflow to split, then everything
            // must be < 5. Why? Because to get to [0,] as your insert target, you must be < 5.
            // if we added to [5,] then a split must be greater than, or the insert would replace 5.
            //
            // if we consider
            //
            //       [ 5 ]
            //        / \
            //    [0,]   [7,]
            //
            // Now we insert 5, and 7, splits. 5 would remain in the tree and we'd split 7 to the right
            //
            // As a result, any "Ok(idx)" must represent a corruption of the tree.
            // debug_assert!(r.is_err());
            let ins_idx = r.unwrap_err();
            let leaf_ins_idx = ins_idx + 1;
            // So why do we only need to insert right? Because the left-most
            // leaf when it grows, it splits to the right. That importantly
            // means that we only need to insert to replace the min and it's
            // right leaf, or anything higher. As a result, we are always
            // targeting ins_idx and leaf_ins_idx = ins_idx + 1.
            //
            // We have a situation like:
            //
            //   [1, 3, 9, 18]
            //
            // and ins_idx is 2. IE:
            //
            //   [1, 3, 9, 18]
            //          ^-- k=6
            //
            // So this we need to shift those r-> and insert.
            //
            //   [1, 3, x, 9, 18]
            //          ^-- k=6
            //
            //   [1, 3, 6, 9, 18]
            //
            // Now we need to consider the leaves too:
            //
            //   [1, 3, 9, 18]
            //   | |  |  |   |
            //   v v  v  v   v
            //   0 1  3  9   18
            //
            // So that means we need to move leaf_ins_idx = (ins_idx + 1)
            // right also
            //
            //   [1, 3, x, 9, 18]
            //   | |  |  |  |   |
            //   v v  v  v  v   v
            //   0 1  3  x  9   18
            //           ^-- leaf for k=6 will go here.
            //
            // Now to talk about the right expand issue - lets say 0 conducted
            // a split, it returns the new right node - which would push
            // 3 to the right to insert a new right hand side as required. So we
            // really never need to consider the left most leaf to have to be
            // replaced in any conditions.
            //
            // Magic!
            unsafe {
                slice_insert(&mut (*self.ctrl.a).1, k, ins_idx);
                slice_insert(&mut self.nodes, node, leaf_ins_idx);
            }

            #[cfg(all(test, not(miri)))]
            debug_assert!(self.poison == FLAG_POISON);
            // finally update the slots
            self.inc_slots();
            // Return that we are okay to go!
            BranchInsertState::Ok
        }
    }

    pub(crate) fn add_node_left(
        &mut self,
        lnode: *mut Node<K, V>,
        sibidx: usize,
    ) -> BranchInsertState<K, V> {
        debug_assert_branch!(self);
        if self.slots() == H_CAPACITY {
            if sibidx == self.slots() {
                // If sibidx == self.slots, then we must be going into max - 1.
                //    [   k1, k2, k3, k4, k5, k6   ]
                //    [ v1, v2, v3, v4, v5, v6, v7 ]
                //                            ^ ^-- sibidx
                //                             \---- where left should go
                //
                //    [   k1, k2, k3, k4, k5, xx   ]
                //    [ v1, v2, v3, v4, v5, v6, xx ]
                //
                //    [   k1, k2, k3, k4, k5, xx   ]    [   k6   ]
                //    [ v1, v2, v3, v4, v5, v6, xx ] -> [ ln, v7 ]
                //
                // So in this case we drop k6, and return a split.
                let max = self.nodes[HBV_CAPACITY - 1];
                unsafe {
                    (*self.ctrl.a).1[H_CAPACITY - 1] = u64::MAX;
                }
                self.dec_slots();
                BranchInsertState::Split(lnode, max)
            } else if sibidx == (self.slots() - 1) {
                // If sibidx == (self.slots - 1), then we must be going into max - 2
                //    [   k1, k2, k3, k4, k5, k6   ]
                //    [ v1, v2, v3, v4, v5, v6, v7 ]
                //                         ^ ^-- sibidx
                //                          \---- where left should go
                //
                //    [   k1, k2, k3, k4, dd, xx   ]
                //    [ v1, v2, v3, v4, v5, xx, xx ]
                //
                //
                // This means that we need to return v6,v7 in a split, and
                // just append node after v5.
                let maxn1 = self.nodes[HBV_CAPACITY - 2];
                let max = self.nodes[HBV_CAPACITY - 1];
                unsafe {
                    (*self.ctrl.a).1[H_CAPACITY - 1] = u64::MAX;
                    (*self.ctrl.a).1[H_CAPACITY - 2] = u64::MAX;
                }
                self.dec_slots();
                self.dec_slots();
                //    [   k1, k2, k3, k4, dd, xx   ]    [   k6   ]
                //    [ v1, v2, v3, v4, v5, xx, xx ] -> [ v6, v7 ]
                let h: u64 = unsafe { (*lnode).min() };

                unsafe {
                    slice_insert(&mut (*self.ctrl.a).1, h, sibidx - 1);
                    slice_insert(&mut self.nodes, lnode, sibidx);
                    // slice_insert(&mut self.node, MaybeUninit::new(node), sibidx);
                }
                #[cfg(all(test, not(miri)))]
                debug_assert!(self.poison == FLAG_POISON);
                self.inc_slots();
                //
                //    [   k1, k2, k3, k4, nk, xx   ]    [   k6   ]
                //    [ v1, v2, v3, v4, v5, ln, xx ] -> [ v6, v7 ]

                BranchInsertState::Split(maxn1, max)
            } else {
                // All other cases;
                //    [   k1, k2, k3, k4, k5, k6   ]
                //    [ v1, v2, v3, v4, v5, v6, v7 ]
                //                 ^ ^-- sibidx
                //                  \---- where left should go
                //
                //    [   k1, k2, k3, k4, dd, xx   ]
                //    [ v1, v2, v3, v4, v5, xx, xx ]
                //
                //    [   k1, k2, k3, nk, k4, dd   ]    [   k6   ]
                //    [ v1, v2, v3, ln, v4, v5, xx ] -> [ v6, v7 ]
                //
                // This means that we need to return v6,v7 in a split,, drop k5,
                // then insert

                // Setup the nodes we intend to split away.
                let maxn1 = self.nodes[HBV_CAPACITY - 2];
                let max = self.nodes[HBV_CAPACITY - 1];
                unsafe {
                    (*self.ctrl.a).1[H_CAPACITY - 1] = u64::MAX;
                    (*self.ctrl.a).1[H_CAPACITY - 2] = u64::MAX;
                }
                self.dec_slots();
                self.dec_slots();

                // println!("pre-fixup -> {:?}", self);

                let sibnode = self.nodes[sibidx];
                let nkey: u64 = unsafe { (*sibnode).min() };

                unsafe {
                    slice_insert(&mut (*self.ctrl.a).1, nkey, sibidx);
                    slice_insert(&mut self.nodes, lnode, sibidx);
                }
                #[cfg(all(test, not(miri)))]
                debug_assert!(self.poison == FLAG_POISON);

                self.inc_slots();
                // println!("post fixup -> {:?}", self);

                BranchInsertState::Split(maxn1, max)
            }
        } else {
            // We have space, so just put it in!
            //    [   k1, k2, k3, k4, xx, xx   ]
            //    [ v1, v2, v3, v4, v5, xx, xx ]
            //                 ^ ^-- sibidx
            //                  \---- where left should go
            //
            //    [   k1, k2, k3, k4, xx, xx   ]
            //    [ v1, v2, v3, ln, v4, v5, xx ]
            //
            //    [   k1, k2, k3, nk, k4, xx   ]
            //    [ v1, v2, v3, ln, v4, v5, xx ]
            //

            let sibnode = self.nodes[sibidx];
            let nkey: u64 = unsafe { (*sibnode).min() };

            unsafe {
                slice_insert(&mut self.nodes, lnode, sibidx);
                slice_insert(&mut (*self.ctrl.a).1, nkey, sibidx);
            }
            #[cfg(all(test, not(miri)))]
            debug_assert!(self.poison == FLAG_POISON);

            self.inc_slots();
            // println!("post fixup -> {:?}", self);
            BranchInsertState::Ok
        }
    }

    fn remove_by_idx(&mut self, idx: usize) -> *mut Node<K, V> {
        debug_assert_branch!(self);
        debug_assert!(idx <= self.slots());
        debug_assert!(idx > 0);
        // remove by idx.
        let _pk = unsafe { slice_remove(&mut (*self.ctrl.a).1, idx - 1) };
        // AFTER the remove, set the top value to u64::MAX
        unsafe {
            (*self.ctrl.a).1[H_CAPACITY - 1] = u64::MAX;
        }
        let pn = unsafe { slice_remove(&mut self.nodes, idx) };
        self.dec_slots();
        pn
    }

    pub(crate) fn shrink_decision(&mut self, ridx: usize) -> BranchShrinkState<K, V> {
        // Given two nodes, we need to decide what to do with them!
        //
        // Remember, this isn't happening in a vacuum. This is really a manipulation of
        // the following structure:
        //
        //      parent (self)
        //     /     \
        //   left    right
        //
        //   We also need to consider the following situation too:
        //
        //          root
        //         /    \
        //    lbranch   rbranch
        //    /    \    /     \
        //   l1    l2  r1     r2
        //
        //  Imagine we have exhausted r2, so we need to merge:
        //
        //          root
        //         /    \
        //    lbranch   rbranch
        //    /    \    /     \
        //   l1    l2  r1 <<-- r2
        //
        // This leaves us with a partial state of
        //
        //          root
        //         /    \
        //    lbranch   rbranch (invalid!)
        //    /    \    /
        //   l1    l2  r1
        //
        // This means rbranch issues a cloneshrink to root. clone shrink must contain the remaineer
        // so that it can be reparented:
        //
        //          root
        //         /
        //    lbranch --
        //    /    \    \
        //   l1    l2   r1
        //
        // Now root has to shrink too.
        //
        //     root  --
        //    /    \    \
        //   l1    l2   r1
        //
        // So, we have to analyse the situation.
        //  * Have left or right been emptied? (how to handle when branches)
        //  * Is left or right below a reasonable threshold?
        //  * Does the opposite have capacity to remain valid?

        debug_assert_branch!(self);
        debug_assert!(ridx > 0 && ridx <= self.slots());
        let left = self.nodes[ridx - 1];
        let right = self.nodes[ridx];
        debug_assert!(!left.is_null());
        debug_assert!(!right.is_null());

        #[allow(clippy::needless_borrow)]
        match unsafe { (&(*left).ctrl.a).0 .0 & FLAG_MASK } {
            FLAG_HASH_LEAF => {
                let lmut = leaf_ref!(left, K, V);
                let rmut = leaf_ref!(right, K, V);

                if lmut.slots() + rmut.slots() <= H_CAPACITY {
                    lmut.merge(rmut);
                    // remove the right node from parent
                    let dnode = self.remove_by_idx(ridx);
                    debug_assert!(dnode == right);
                    if self.slots() == 0 {
                        // We now need to be merged across as we only contain a single
                        // value now.
                        BranchShrinkState::Shrink(dnode)
                    } else {
                        // We are complete!
                        // #[cfg(test)]
                        // println!("🔥 {:?}", rmut.nid);
                        BranchShrinkState::Merge(dnode)
                    }
                } else if rmut.slots() > (H_CAPACITY / 2) {
                    lmut.take_from_r_to_l(rmut);
                    self.rekey_by_idx(ridx);
                    BranchShrinkState::Balanced
                } else if lmut.slots() > (H_CAPACITY / 2) {
                    lmut.take_from_l_to_r(rmut);
                    self.rekey_by_idx(ridx);
                    BranchShrinkState::Balanced
                } else {
                    // Do nothing
                    BranchShrinkState::Balanced
                }
            }
            FLAG_HASH_BRANCH => {
                // right or left is now in a "corrupt" state with a single value that we need to relocate
                // to left - or we need to borrow from left and fix it!
                let lmut = branch_ref!(left, K, V);
                let rmut = branch_ref!(right, K, V);

                debug_assert!(rmut.slots() == 0 || lmut.slots() == 0);
                debug_assert!(rmut.slots() <= H_CAPACITY || lmut.slots() <= H_CAPACITY);
                // println!("branch {:?} {:?}", lmut.slots(), rmut.slots());
                if lmut.slots() == H_CAPACITY {
                    // println!("branch take_from_l_to_r ");
                    lmut.take_from_l_to_r(rmut);
                    self.rekey_by_idx(ridx);
                    BranchShrinkState::Balanced
                } else if rmut.slots() == H_CAPACITY {
                    // println!("branch take_from_r_to_l ");
                    lmut.take_from_r_to_l(rmut);
                    self.rekey_by_idx(ridx);
                    BranchShrinkState::Balanced
                } else {
                    // println!("branch merge");
                    // merge the right to tail of left.
                    // println!("BL {:?}", lmut);
                    // println!("BR {:?}", rmut);
                    lmut.merge(rmut);
                    // println!("AL {:?}", lmut);
                    // println!("AR {:?}", rmut);
                    // Reduce our slots
                    let dnode = self.remove_by_idx(ridx);
                    debug_assert!(dnode == right);
                    if self.slots() == 0 {
                        // We now need to be merged across as we also only contain a single
                        // value now.
                        BranchShrinkState::Shrink(dnode)
                    } else {
                        // We are complete!
                        // #[cfg(test)]
                        // println!("🚨 {:?}", rmut.nid);
                        BranchShrinkState::Merge(dnode)
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    pub(crate) fn extract_last_node(&self) -> *mut Node<K, V> {
        debug_assert_branch!(self);
        self.nodes[0]
    }

    pub(crate) fn rekey_by_idx(&mut self, idx: usize) {
        debug_assert_branch!(self);
        debug_assert!(idx <= self.slots());
        debug_assert!(idx > 0);
        // For the node listed, rekey it.
        let nref = self.nodes[idx];
        unsafe { (*self.ctrl.a).1[idx - 1] = (*nref).min() };
    }

    #[inline(always)]
    pub(crate) fn merge(&mut self, right: &mut Self) {
        debug_assert_branch!(self);
        debug_assert_branch!(right);
        let sc = self.slots();
        let rc = right.slots();
        if rc == 0 {
            let node = right.nodes[0];
            debug_assert!(!node.is_null());
            let h: u64 = unsafe { (*node).min() };
            let ins_idx = self.slots();
            let leaf_ins_idx = ins_idx + 1;
            unsafe {
                slice_insert(&mut (*self.ctrl.a).1, h, ins_idx);
                slice_insert(&mut self.nodes, node, leaf_ins_idx);
            }
            #[cfg(all(test, not(miri)))]
            debug_assert!(self.poison == FLAG_POISON);
            self.inc_slots();
        } else {
            debug_assert!(sc == 0);
            unsafe {
                // Move all the nodes from right.
                slice_merge(&mut self.nodes, 1, &mut right.nodes, rc + 1);
                // Move the related keys.
                slice_merge(&mut (*self.ctrl.a).1, 1, &mut (*right.ctrl.a).1, rc);
            }
            #[cfg(all(test, not(miri)))]
            debug_assert!(self.poison == FLAG_POISON);
            // Set our slots correctly.
            unsafe {
                (*self.ctrl.a).0.set_slots(rc + 1);
            }
            // Set right len to 0
            unsafe {
                (*right.ctrl.a).0.set_slots(0);
            }
            // rekey the lowest pointer.
            unsafe {
                let nptr = self.nodes[1];
                let h: u64 = (*nptr).min();
                (*self.ctrl.a).1[0] = h;
            }
            // done!
        }
    }

    pub(crate) fn take_from_l_to_r(&mut self, right: &mut Self) {
        debug_assert_branch!(self);
        debug_assert_branch!(right);

        debug_assert!(self.slots() > right.slots());
        // Starting index of where we move from. We work normally from a branch
        // with only zero (but the base) branch item, but we do the math anyway
        // to be sure in case we change later.
        //
        // So, self.len must be larger, so let's give a few examples here.
        //  4 = 7 - (7 + 0) / 2 (will move 4, 5, 6)
        //  3 = 6 - (6 + 0) / 2 (will move 3, 4, 5)
        //  3 = 5 - (5 + 0) / 2 (will move 3, 4)
        //  2 = 4 ....          (will move 2, 3)
        //
        let slots = (self.slots() + right.slots()) / 2;
        let start_idx = self.slots() - slots;
        // Move the remaining element from r to the correct location.
        //
        //    [   k1, k2, k3, k4, k5, k6   ]
        //    [ v1, v2, v3, v4, v5, v6, v7 ] -> [ v8, ------- ]
        //
        // To:
        //
        //    [   k1, k2, k3, k4, k5, k6   ]    [   --, --, --, --, ...
        //    [ v1, v2, v3, v4, v5, v6, v7 ] -> [ --, --, --, v8, --, ...
        //
        unsafe {
            ptr::swap(
                right.nodes.get_unchecked_mut(0),
                right.nodes.get_unchecked_mut(slots),
            )
        }
        // Move our values from the tail.
        // We would move 3 now to:
        //
        //    [   k1, k2, k3, k4, k5, k6   ]    [   --, --, --, --, ...
        //    [ v1, v2, v3, v4, --, --, -- ] -> [ v5, v6, v7, v8, --, ...
        //
        unsafe {
            slice_move(&mut right.nodes, 0, &mut self.nodes, start_idx + 1, slots);
        }
        // Remove the keys from left.
        // So we need to remove the corresponding keys. so that we get.
        //
        //    [   k1, k2, k3, --, --, --   ]    [   --, --, --, --, ...
        //    [ v1, v2, v3, v4, --, --, -- ] -> [ v5, v6, v7, v8, --, ...
        //
        // This means it's start_idx - 1 up to BK cap
        unsafe {
            let tgt_ptr = (*self.ctrl.a).1.as_mut_ptr().add(start_idx);
            // https://doc.rust-lang.org/std/ptr/fn.write_bytes.html
            // Sets count * size_of::<T>() bytes of memory starting at dst to val.
            ptr::write_bytes::<u64>(tgt_ptr, 0xff, H_CAPACITY - start_idx);
        }

        // Adjust both slotss - we do this before rekey to ensure that the safety
        // checks hold in debugging.
        unsafe {
            (*right.ctrl.a).0.set_slots(slots);
        }
        unsafe {
            (*self.ctrl.a).0.set_slots(start_idx);
        }
        // Rekey right
        for kidx in 1..(slots + 1) {
            right.rekey_by_idx(kidx);
        }
        #[cfg(all(test, not(miri)))]
        debug_assert!(self.poison == FLAG_POISON);
        #[cfg(all(test, not(miri)))]
        debug_assert!(right.poison == FLAG_POISON);
        // Done!
        debug_assert!(self.verify());
        debug_assert!(right.verify());
    }

    pub(crate) fn take_from_r_to_l(&mut self, right: &mut Self) {
        debug_assert_branch!(self);
        debug_assert_branch!(right);
        debug_assert!(right.slots() >= self.slots());

        let slots = (self.slots() + right.slots()) / 2;
        let start_idx = right.slots() - slots;

        // We move slots from right to left.
        unsafe {
            slice_move(&mut self.nodes, 1, &mut right.nodes, 0, slots);
        }

        // move keys down in right
        unsafe {
            ptr::copy(
                right.ctrl.a.1.as_ptr().add(slots),
                (*right.ctrl.a).1.as_mut_ptr(),
                start_idx,
            );
        }

        // Fix up the upper keys
        /*
        for idx in start_idx..H_CAPACITY {
            right.key[idx] = u64::MAX;
        }
        */
        unsafe {
            let tgt_ptr = (*right.ctrl.a).1.as_mut_ptr().add(start_idx);
            // https://doc.rust-lang.org/std/ptr/fn.write_bytes.html
            // Sets count * size_of::<T>() bytes of memory starting at dst to val.
            ptr::write_bytes::<u64>(tgt_ptr, 0xff, H_CAPACITY - start_idx);
        }
        #[cfg(all(test, not(miri)))]
        debug_assert!(right.poison == FLAG_POISON);

        // move nodes down in right
        unsafe {
            ptr::copy(
                right.nodes.as_ptr().add(slots),
                right.nodes.as_mut_ptr(),
                start_idx + 1,
            );
        }

        // update slotss
        unsafe {
            (*right.ctrl.a).0.set_slots(start_idx);
        }
        unsafe {
            (*self.ctrl.a).0.set_slots(slots);
        }
        // Rekey left
        for kidx in 1..(slots + 1) {
            self.rekey_by_idx(kidx);
        }
        debug_assert!(self.verify());
        debug_assert!(right.verify());
        // Done!
    }

    #[inline(always)]
    pub(crate) fn replace_by_idx(&mut self, idx: usize, node: *mut Node<K, V>) {
        debug_assert_branch!(self);
        debug_assert!(idx <= self.slots());
        debug_assert!(!self.nodes[idx].is_null());
        self.nodes[idx] = node;
    }

    pub(crate) fn clone_sibling_idx(
        &mut self,
        txid: u64,
        idx: usize,
        last_seen: &mut Vec<*mut Node<K, V>>,
        first_seen: &mut Vec<*mut Node<K, V>>,
    ) -> usize {
        debug_assert_branch!(self);
        // if we clone, return Some new ptr. if not, None.
        let (ridx, idx) = if idx == 0 {
            // println!("clone_sibling_idx clone right");
            // If we are 0 we clone our right sibling,
            // and return the right idx as 1.
            (1, 1)
        } else {
            // println!("clone_sibling_idx clone left");
            // Else we clone the left, and leave our index unchanged
            // as we are the right node.
            (idx, idx - 1)
        };
        // Now clone the item at idx.
        debug_assert!(idx <= self.slots());
        let sib_ptr = self.nodes[idx];
        debug_assert!(!sib_ptr.is_null());
        // Do we need to clone?
        #[allow(clippy::needless_borrow)]
        let res = match unsafe { (&(*sib_ptr).ctrl.a).0 .0 } & FLAG_MASK {
            FLAG_HASH_LEAF => {
                let lref = unsafe { &*(sib_ptr as *const _ as *const Leaf<K, V>) };
                lref.req_clone(txid)
            }
            FLAG_HASH_BRANCH => {
                let bref = unsafe { &*(sib_ptr as *const _ as *const Branch<K, V>) };
                bref.req_clone(txid)
            }
            _ => unreachable!(),
        };

        // If it did clone, it's a some, so we map that to have the from and new ptrs for
        // the memory management.
        if let Some(n_ptr) = res {
            // println!("ls push 101 {:?}", sib_ptr);
            first_seen.push(n_ptr);
            last_seen.push(sib_ptr);
            // Put the pointer in place.
            self.nodes[idx] = n_ptr;
        };
        // Now return the right index
        ridx
    }

    /*
    pub(crate) fn trim_lt_key<Q: ?Sized>(
        &mut self,
        k: &Q,
        last_seen: &mut Vec<*mut Node<K, V>>,
        first_seen: &mut Vec<*mut Node<K, V>>,
    ) -> BranchTrimState<K, V>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        debug_assert_branch!(self);
        // The possible states of a branch are
        //
        // [  0,  4,  8,  12  ]
        // [n1, n2, n3, n4, n5]
        //
        let r = key_search!(self, k);

        let sc = self.slots();

        match r {
            Ok(idx) => {
                debug_assert!(idx < sc);
                // * A key matches exactly a value. IE k is 4. This means we can remove
                //   n1 and n2 because we know 4 must be in n3 as the min.

                // NEED MM
                debug_assert!(false);

                unsafe {
                    slice_slide_and_drop(&mut self.key, idx, sc - (idx + 1));
                    slice_slide(&mut self.nodes.as_mut(), idx, sc - idx);
                }
                self.meta.set_slots(sc - (idx + 1));

                if self.slots() == 0 {
                    let rnode = self.extract_last_node();
                    BranchTrimState::Promote(rnode)
                } else {
                    BranchTrimState::Complete
                }
            }
            Err(idx) => {
                if idx == 0 {
                    // * The key is less than min. IE it wants to remove the lowest value.
                    // Check the "max" value of the subtree to know if we can proceed.
                    let tnode: *mut Node<K, V> = self.nodes[0];
                    let branch_k: &K = unsafe { (*tnode).max() };
                    if branch_k.borrow() < k {
                        // Everything is smaller, let's remove it that subtree.
                        // NEED MM
                        debug_assert!(false);
                        let _pk = unsafe { slice_remove(&mut self.key, 0).assume_init() };
                        let _pn = unsafe { slice_remove(self.nodes.as_mut(), 0) };
                        self.dec_slots();
                        BranchTrimState::Complete
                    } else {
                        BranchTrimState::Complete
                    }
                } else if idx >= self.slots() {
                    // remove everything except max.
                    unsafe {
                        // NEED MM
                        debug_assert!(false);
                        // We just drop all the keys.
                        for kidx in 0..self.slots() {
                            ptr::drop_in_place(self.key[kidx].as_mut_ptr());
                            // ptr::drop_in_place(self.nodes[kidx].as_mut_ptr());
                        }
                        // Move the last node to the bottom.
                        self.nodes[0] = self.nodes[sc];
                    }
                    self.meta.set_slots(0);
                    let rnode = self.extract_last_node();
                    // Something may still be valid, hand it on.
                    BranchTrimState::Promote(rnode)
                } else {
                    // * A key is between two values. We can remove everything less, but not
                    //   the associated. For example, remove 6 would cause n1, n2 to be removed, but
                    //   the prune/walk will have to examine n3 to know about further changes.
                    debug_assert!(idx > 0);

                    let tnode: *mut Node<K, V> = self.nodes[0];
                    let branch_k: &K = unsafe { (*tnode).max() };

                    if branch_k.borrow() < k {
                        // NEED MM
                        debug_assert!(false);
                        // Remove including idx.
                        unsafe {
                            slice_slide_and_drop(&mut self.key, idx, sc - (idx + 1));
                            slice_slide(self.nodes.as_mut(), idx, sc - idx);
                        }
                        self.meta.set_slots(sc - (idx + 1));
                    } else {
                        // NEED MM
                        debug_assert!(false);
                        unsafe {
                            slice_slide_and_drop(&mut self.key, idx - 1, sc - idx);
                            slice_slide(self.nodes.as_mut(), idx - 1, sc - (idx - 1));
                        }
                        self.meta.set_slots(sc - idx);
                    }

                    if self.slots() == 0 {
                        // NEED MM
                        debug_assert!(false);
                        let rnode = self.extract_last_node();
                        BranchTrimState::Promote(rnode)
                    } else {
                        BranchTrimState::Complete
                    }
                }
            }
        }
    }
    */

    pub(crate) fn verify(&self) -> bool {
        debug_assert_branch!(self);
        #[cfg(all(test, not(miri)))]
        debug_assert!(self.poison == FLAG_POISON);
        if self.slots() == 0 {
            // Not possible to be valid!
            debug_assert!(false);
            return false;
        }
        // println!("verify branch -> {:?}", self);
        // Check everything above slots is u64::max
        for work_idx in unsafe { self.ctrl.a.0.slots() }..H_CAPACITY {
            if unsafe { self.ctrl.a.1[work_idx] } != u64::MAX {
                eprintln!("FAILED ARRAY -> {:?}", unsafe { self.ctrl.a.1 });
                debug_assert!(false);
            }
        }
        // Check we are sorted.
        let mut lk: u64 = unsafe { self.ctrl.a.1[0] };
        for work_idx in 1..self.slots() {
            let rk: u64 = unsafe { self.ctrl.a.1[work_idx] };
            // println!("{:?} >= {:?}", lk, rk);
            if lk >= rk {
                debug_assert!(false);
                return false;
            }
            lk = rk;
        }
        // Recursively call verify
        for work_idx in 0..self.slots() {
            let node = unsafe { &*self.nodes[work_idx] };
            if !node.verify() {
                for work_idx in 0..(self.slots() + 1) {
                    let nref = unsafe { &*self.nodes[work_idx] };
                    if !nref.verify() {
                        // println!("Failed children");
                        debug_assert!(false);
                        return false;
                    }
                }
            }
        }
        // Check descendants are validly ordered.
        //                 V-- remember, there are slots + 1 nodes.
        for work_idx in 0..self.slots() {
            // get left max and right min
            let lnode = unsafe { &*self.nodes[work_idx] };
            let rnode = unsafe { &*self.nodes[work_idx + 1] };

            let pkey = unsafe { self.ctrl.a.1[work_idx] };
            let lkey: u64 = lnode.max();
            let rkey: u64 = rnode.min();
            if lkey >= pkey || pkey > rkey {
                /*
                println!("++++++");
                println!("{:?} >= {:?}, {:?} > {:?}", lkey, pkey, pkey, rkey);
                println!("out of order key found {}", work_idx);
                // println!("left --> {:?}", lnode);
                // println!("right -> {:?}", rnode);
                println!("prnt  -> {:?}", self);
                */
                debug_assert!(false);
                return false;
            }
        }
        // All good!
        true
    }

    #[allow(clippy::cast_ptr_alignment)]
    fn free(node: *mut Self) {
        unsafe {
            let mut _x: Box<CachePadded<Branch<K, V>>> =
                Box::from_raw(node as *mut CachePadded<Branch<K, V>>);
        }
    }
}

impl<K: Hash + Eq + Clone + Debug, V: Clone> Debug for Branch<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), Error> {
        debug_assert_branch!(self);
        write!(f, "Branch -> {}", self.slots())?;
        #[cfg(all(test, not(miri)))]
        write!(f, " nid: {}", self.nid)?;
        write!(f, "  \\-> [ ")?;
        for idx in 0..self.slots() {
            write!(f, "{:?}, ", unsafe { self.ctrl.a.1[idx] })?;
        }
        write!(f, " ]")
    }
}

impl<K: Hash + Eq + Clone + Debug, V: Clone> Drop for Branch<K, V> {
    fn drop(&mut self) {
        debug_assert_branch!(self);
        #[cfg(all(test, not(miri)))]
        release_nid(self.nid);
        // Done
        unsafe {
            (*self.ctrl.a).0 .0 = FLAG_DROPPED;
        }
        debug_assert!(unsafe { self.ctrl.a.0 .0 } & FLAG_MASK != FLAG_HASH_BRANCH);
        // println!("set branch {:?} to {:x}", self.nid, self.meta.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /*
    #[test]
    fn test_hashmap2_node_cache_size() {
        let ls = std::mem::size_of::<Leaf<u64, u64>>() - std::mem::size_of::<usize>();
        let bs = std::mem::size_of::<Branch<u64, u64>>() - std::mem::size_of::<usize>();
        println!("ls {:?}, bs {:?}", ls, bs);
        assert!(ls <= 128);
        assert!(bs <= 128);
    }
    */

    #[test]
    fn test_hashmap2_node_test_weird_basics() {
        let leaf: *mut Leaf<u64, u64> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf };

        assert!(leaf.get_txid() == 1);
        // println!("{:?}", leaf);

        leaf.set_slots(1);
        assert!(leaf.slots() == 1);
        leaf.set_slots(0);
        assert!(leaf.slots() == 0);

        leaf.inc_slots();
        leaf.inc_slots();
        leaf.inc_slots();
        assert!(leaf.slots() == 3);
        leaf.dec_slots();
        leaf.dec_slots();
        leaf.dec_slots();
        assert!(leaf.slots() == 0);

        /*
        let branch: *mut Branch<u64, u64> = Node::new_branch(1, ptr::null_mut(), ptr::null_mut());
        let branch = unsafe { &mut *branch };
        assert!(branch.get_txid() == 1);
        // println!("{:?}", branch);

        branch.set_slots(3);
        assert!(branch.slots() == 3);
        branch.set_slots(0);
        assert!(branch.slots() == 0);
        Branch::free(branch as *mut _);
        */

        Leaf::free(leaf as *mut _);
        assert_released();
    }

    #[test]
    fn test_hashmap2_node_leaf_in_order() {
        let leaf: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf };
        assert!(leaf.get_txid() == 1);
        // Check insert to capacity
        for kv in 0..H_CAPACITY {
            let r = leaf.insert_or_update(kv as u64, kv, kv);
            if let LeafInsertState::Ok(None) = r {
                assert!(leaf.get_ref(kv as u64, &kv) == Some(&kv));
            } else {
                panic!("Invalid leaf insert state");
            }
        }
        assert!(leaf.verify());
        // Check update to capacity
        for kv in 0..H_CAPACITY {
            let r = leaf.insert_or_update(kv as u64, kv, kv);
            if let LeafInsertState::Ok(Some(pkv)) = r {
                assert!(pkv == kv);
                assert!(leaf.get_ref(kv as u64, &kv) == Some(&kv));
            } else {
                panic!("Invalid leaf insert state");
            }
        }
        assert!(leaf.verify());
        Leaf::free(leaf as *mut _);
        assert_released();
    }

    #[test]
    fn test_hashmap2_node_leaf_collision_in_order() {
        let leaf: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf };
        let hash: u64 = 1;
        assert!(leaf.get_txid() == 1);
        // Check insert to capacity
        for kv in 0..H_CAPACITY {
            let r = leaf.insert_or_update(hash, kv, kv);
            if let LeafInsertState::Ok(None) = r {
                assert!(leaf.get_ref(hash, &kv) == Some(&kv));
            } else {
                panic!("Invalid leaf insert state");
            }
        }
        assert!(leaf.verify());
        // Check update to capacity
        for kv in 0..H_CAPACITY {
            let r = leaf.insert_or_update(hash, kv, kv);
            if let LeafInsertState::Ok(Some(pkv)) = r {
                assert!(pkv == kv);
                assert!(leaf.get_ref(hash, &kv) == Some(&kv));
            } else {
                panic!("Invalid leaf insert state");
            }
        }
        assert!(leaf.verify());
        Leaf::free(leaf as *mut _);
        assert_released();
    }

    #[test]
    fn test_hashmap2_node_leaf_out_of_order() {
        let leaf: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf };

        let kvs = [7, 5, 1, 6, 2, 3, 0, 8];
        assert!(leaf.get_txid() == 1);
        // Check insert to capacity
        for kv in kvs.iter().take(H_CAPACITY) {
            let r = leaf.insert_or_update(*kv as u64, *kv, *kv);
            if let LeafInsertState::Ok(None) = r {
                assert!(leaf.get_ref(*kv as u64, kv) == Some(kv));
            } else {
                panic!("Invalid leaf insert state");
            }
        }
        assert!(leaf.verify());
        assert!(leaf.slots() == H_CAPACITY);
        // Check update to capacity
        for kv in kvs.iter().take(H_CAPACITY) {
            let r = leaf.insert_or_update(*kv as u64, *kv, *kv);
            if let LeafInsertState::Ok(Some(pkv)) = r {
                assert!(pkv == *kv);
                assert!(leaf.get_ref(*kv as u64, kv) == Some(kv));
            } else {
                panic!("Invalid leaf insert state");
            }
        }
        assert!(leaf.verify());
        assert!(leaf.slots() == H_CAPACITY);
        Leaf::free(leaf as *mut _);
        assert_released();
    }

    #[test]
    fn test_hashmap2_node_leaf_collision_out_of_order() {
        let leaf: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf };
        let hash: u64 = 1;

        let kvs = [7, 5, 1, 6, 2, 3, 0, 8];
        assert!(leaf.get_txid() == 1);
        // Check insert to capacity
        for kv in kvs.iter().take(H_CAPACITY) {
            let r = leaf.insert_or_update(hash, *kv, *kv);
            if let LeafInsertState::Ok(None) = r {
                assert!(leaf.get_ref(hash, kv) == Some(kv));
            } else {
                panic!("Invalid leaf insert state");
            }
        }
        assert!(leaf.verify());
        assert!(leaf.count() == H_CAPACITY);
        assert!(leaf.slots() == 1);
        // Check update to capacity
        for kv in kvs.iter().take(H_CAPACITY) {
            let r = leaf.insert_or_update(hash, *kv, *kv);
            if let LeafInsertState::Ok(Some(pkv)) = r {
                assert!(pkv == *kv);
                assert!(leaf.get_ref(hash, kv) == Some(kv));
            } else {
                panic!("Invalid leaf insert state");
            }
        }
        assert!(leaf.verify());
        assert!(leaf.count() == H_CAPACITY);
        assert!(leaf.slots() == 1);
        Leaf::free(leaf as *mut _);
        assert_released();
    }

    #[test]
    fn test_hashmap2_node_leaf_min() {
        let leaf: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf };

        let kvs = [3, 2, 6, 4, 5, 1, 9, 0];
        let min: [u64; 8] = [3, 2, 2, 2, 2, 1, 1, 0];

        for idx in 0..H_CAPACITY {
            let kv = kvs[idx];
            let r = leaf.insert_or_update(kv as u64, kv, kv);
            if let LeafInsertState::Ok(None) = r {
                assert!(leaf.get_ref(kv as u64, &kv) == Some(&kv));
                assert!(leaf.min() == min[idx]);
            } else {
                panic!("Invalid leaf insert state");
            }
        }
        assert!(leaf.verify());
        assert!(leaf.slots() == H_CAPACITY);
        Leaf::free(leaf as *mut _);
        assert_released();
    }

    #[test]
    fn test_hashmap2_node_leaf_max() {
        let leaf: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf };

        let kvs = [1, 3, 2, 6, 4, 5, 9, 0];
        let max: [u64; 8] = [1, 3, 3, 6, 6, 6, 9, 9];

        for idx in 0..H_CAPACITY {
            let kv = kvs[idx];
            let r = leaf.insert_or_update(kv as u64, kv, kv);
            if let LeafInsertState::Ok(None) = r {
                assert!(leaf.get_ref(kv as u64, &kv) == Some(&kv));
                assert!(leaf.max() == max[idx]);
            } else {
                panic!("Invalid leaf insert state");
            }
        }
        assert!(leaf.verify());
        assert!(leaf.slots() == H_CAPACITY);
        Leaf::free(leaf as *mut _);
        assert_released();
    }

    #[test]
    fn test_hashmap2_node_leaf_remove_order() {
        let leaf: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf };
        for kv in 0..H_CAPACITY {
            leaf.insert_or_update(kv as u64, kv, kv);
        }
        // Remove all but one.
        for kv in 0..(H_CAPACITY - 1) {
            let r = leaf.remove(kv as u64, &kv);
            if let LeafRemoveState::Ok(Some(rkv)) = r {
                assert!(rkv == kv);
            } else {
                panic!("Invalid leaf remove state");
            }
        }
        assert!(leaf.slots() == 1);
        assert!(leaf.max() == (H_CAPACITY - 1) as u64);
        // Remove a non-existent value.
        let r = leaf.remove((H_CAPACITY + 20) as u64, &(H_CAPACITY + 20));
        if let LeafRemoveState::Ok(None) = r {
            // Ok!
        } else {
            panic!("Invalid leaf remove state");
        }
        // Finally clear the node, should request a shrink.
        let kv = H_CAPACITY - 1;
        let r = leaf.remove(kv as u64, &kv);
        if let LeafRemoveState::Shrink(Some(rkv)) = r {
            assert!(rkv == kv);
        } else {
            panic!("Invalid leaf remove state");
        }
        assert!(leaf.slots() == 0);
        // Remove non-existent post shrink. Should never happen
        // but safety first!
        let r = leaf.remove(0, &0);
        if let LeafRemoveState::Shrink(None) = r {
            // Ok!
        } else {
            panic!("Invalid leaf remove state");
        }

        assert!(leaf.slots() == 0);
        assert!(leaf.verify());
        Leaf::free(leaf as *mut _);
        assert_released();
    }

    #[test]
    fn test_hashmap2_node_leaf_remove_out_of_order() {
        let leaf: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf };
        for kv in 0..H_CAPACITY {
            leaf.insert_or_update(kv as u64, kv, kv);
        }
        let mid = H_CAPACITY / 2;
        // This test removes all BUT one node to keep the states simple.
        for kv in mid..(H_CAPACITY - 1) {
            let r = leaf.remove(kv as u64, &kv);
            match r {
                LeafRemoveState::Ok(_) => {}
                _ => panic!(),
            }
        }

        for kv in 0..(H_CAPACITY / 2) {
            let r = leaf.remove(kv as u64, &kv);
            match r {
                LeafRemoveState::Ok(_) => {}
                _ => panic!(),
            }
        }

        assert!(leaf.slots() == 1);
        assert!(leaf.verify());
        Leaf::free(leaf as *mut _);
        assert_released();
    }

    #[test]
    fn test_hashmap2_node_leaf_remove_collision_in_order() {
        let leaf: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf };
        let hash: u64 = 1;
        assert!(leaf.get_txid() == 1);
        // Check insert to capacity
        for kv in 0..H_CAPACITY {
            let r = leaf.insert_or_update(hash, kv, kv);
            if let LeafInsertState::Ok(None) = r {
                assert!(leaf.get_ref(hash, &kv) == Some(&kv));
            } else {
                panic!("Invalid leaf insert state");
            }
        }
        assert!(leaf.verify());
        assert!(leaf.count() == H_CAPACITY);
        assert!(leaf.slots() == 1);
        // Check remove to cap - 1
        for kv in 1..H_CAPACITY {
            let r = leaf.remove(hash, &kv);
            match r {
                LeafRemoveState::Ok(_) => {}
                _ => panic!(),
            }
        }
        assert!(leaf.count() == 1);
        assert!(leaf.slots() == 1);
        assert!(leaf.verify());
        Leaf::free(leaf as *mut _);
        assert_released();
    }

    #[test]
    fn test_hashmap2_node_leaf_insert_split() {
        let leaf: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf };
        for kv in 0..H_CAPACITY {
            let x = kv + 10;
            leaf.insert_or_update(x as u64, x, x);
        }

        // Split right
        let y = H_CAPACITY + 10;
        let r = leaf.insert_or_update(y as u64, y, y);
        if let LeafInsertState::Split(rleaf) = r {
            unsafe {
                assert!((*rleaf).slots() == 1);
            }
            Leaf::free(rleaf);
        } else {
            panic!();
        }

        // Split left
        let r = leaf.insert_or_update(0, 0, 0);
        if let LeafInsertState::RevSplit(lleaf) = r {
            unsafe {
                assert!((*lleaf).slots() == 1);
            }
            Leaf::free(lleaf);
        } else {
            panic!();
        }

        assert!(leaf.slots() == H_CAPACITY);
        assert!(leaf.verify());
        Leaf::free(leaf as *mut _);
        assert_released();
    }

    /*
    #[test]
    fn test_bptree_leaf_remove_lt() {
        // This is used in split off.
        // Remove none
        let leaf1: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf1 = unsafe { &mut *leaf };
        for kv in 0..H_CAPACITY {
            let _ = leaf1.insert_or_update(kv + 10, kv);
        }
        leaf1.remove_lt(&5);
        assert!(leaf1.slots() == H_CAPACITY);
        Leaf::free(leaf1 as *mut _);

        // Remove all
        let leaf2: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf2 = unsafe { &mut *leaf };
        for kv in 0..H_CAPACITY {
            let _ = leaf2.insert_or_update(kv + 10, kv);
        }
        leaf2.remove_lt(&(H_CAPACITY + 10));
        assert!(leaf2.slots() == 0);
        Leaf::free(leaf2 as *mut _);

        // Remove from middle
        let leaf3: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf3 = unsafe { &mut *leaf };
        for kv in 0..H_CAPACITY {
            let _ = leaf3.insert_or_update(kv + 10, kv);
        }
        leaf3.remove_lt(&((H_CAPACITY / 2) + 10));
        assert!(leaf3.slots() == (H_CAPACITY / 2));
        Leaf::free(leaf3 as *mut _);

        // Remove less than not in leaf.
        let leaf4: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf4 = unsafe { &mut *leaf };
        let _ = leaf4.insert_or_update(5, 5);
        let _ = leaf4.insert_or_update(15, 15);
        leaf4.remove_lt(&10);
        assert!(leaf4.slots() == 1);

        //  Add another and remove all.
        let _ = leaf4.insert_or_update(20, 20);
        leaf4.remove_lt(&25);
        assert!(leaf4.slots() == 0);
        Leaf::free(leaf4 as *mut _);
        // Done!
        assert_released();
    }
    */

    /* ============================================ */
    // Branch tests here!

    #[test]
    fn test_hashmap2_node_branch_new() {
        // Create a new branch, and test it.
        let left: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let left_ref = unsafe { &mut *left };
        let right: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let right_ref = unsafe { &mut *right };

        // add kvs to l and r
        for kv in 0..H_CAPACITY {
            let lkv = kv + 10;
            let rkv = kv + 20;
            left_ref.insert_or_update(lkv as u64, lkv, lkv);
            right_ref.insert_or_update(rkv as u64, rkv, rkv);
        }
        // create branch
        let branch: *mut Branch<usize, usize> = Node::new_branch(
            1,
            left as *mut Node<usize, usize>,
            right as *mut Node<usize, usize>,
        );
        let branch_ref = unsafe { &mut *branch };
        // verify
        assert!(branch_ref.verify());
        // Test .min works on our descendants
        assert!(branch_ref.min() == 10);
        // Test .max works on our descendants.
        assert!(branch_ref.max() == (20 + H_CAPACITY - 1) as u64);
        // Get some k within the leaves.
        assert!(branch_ref.get_ref(11, &11) == Some(&11));
        assert!(branch_ref.get_ref(21, &21) == Some(&21));
        // get some k that is out of bounds.
        assert!(branch_ref.get_ref(1, &1).is_none());
        assert!(branch_ref.get_ref(100, &100).is_none());

        Leaf::free(left as *mut _);
        Leaf::free(right as *mut _);
        Branch::free(branch as *mut _);
        assert_released();
    }

    // Helpers
    macro_rules! test_3_leaf {
        ($fun:expr) => {{
            let a: *mut Leaf<usize, usize> = Node::new_leaf(1);
            let b: *mut Leaf<usize, usize> = Node::new_leaf(1);
            let c: *mut Leaf<usize, usize> = Node::new_leaf(1);

            unsafe {
                (*a).insert_or_update(10, 10, 10);
                (*b).insert_or_update(20, 20, 20);
                (*c).insert_or_update(30, 30, 30);
            }

            $fun(a, b, c);

            Leaf::free(a as *mut _);
            Leaf::free(b as *mut _);
            Leaf::free(c as *mut _);
            assert_released();
        }};
    }

    #[test]
    fn test_hashmap2_node_branch_add_min() {
        // This pattern occurs with "revsplit" to help with reverse
        // ordered inserts.
        test_3_leaf!(|a, b, c| {
            // Add the max two to the branch
            let branch: *mut Branch<usize, usize> = Node::new_branch(
                1,
                b as *mut Node<usize, usize>,
                c as *mut Node<usize, usize>,
            );
            let branch_ref = unsafe { &mut *branch };
            // verify
            assert!(branch_ref.verify());
            // Now min node (uses a diff function!)
            let r = branch_ref.add_node_left(a as *mut Node<usize, usize>, 0);
            match r {
                BranchInsertState::Ok => {}
                _ => debug_assert!(false),
            };
            // Assert okay + verify
            assert!(branch_ref.verify());
            Branch::free(branch as *mut _);
        })
    }

    #[test]
    fn test_hashmap2_node_branch_add_mid() {
        test_3_leaf!(|a, b, c| {
            // Add the outer two to the branch
            let branch: *mut Branch<usize, usize> = Node::new_branch(
                1,
                a as *mut Node<usize, usize>,
                c as *mut Node<usize, usize>,
            );
            let branch_ref = unsafe { &mut *branch };
            // verify
            assert!(branch_ref.verify());
            let r = branch_ref.add_node(b as *mut Node<usize, usize>);
            match r {
                BranchInsertState::Ok => {}
                _ => debug_assert!(false),
            };
            // Assert okay + verify
            assert!(branch_ref.verify());
            Branch::free(branch as *mut _);
        })
    }

    #[test]
    fn test_hashmap2_node_branch_add_max() {
        test_3_leaf!(|a, b, c| {
            // add the bottom two
            let branch: *mut Branch<usize, usize> = Node::new_branch(
                1,
                a as *mut Node<usize, usize>,
                b as *mut Node<usize, usize>,
            );
            let branch_ref = unsafe { &mut *branch };
            // verify
            assert!(branch_ref.verify());
            let r = branch_ref.add_node(c as *mut Node<usize, usize>);
            match r {
                BranchInsertState::Ok => {}
                _ => debug_assert!(false),
            };
            // Assert okay + verify
            assert!(branch_ref.verify());
            Branch::free(branch as *mut _);
        })
    }

    // Helpers
    macro_rules! test_max_leaf {
        ($fun:expr) => {{
            let a: *mut Leaf<usize, usize> = Node::new_leaf(1);
            let b: *mut Leaf<usize, usize> = Node::new_leaf(1);
            let c: *mut Leaf<usize, usize> = Node::new_leaf(1);
            let d: *mut Leaf<usize, usize> = Node::new_leaf(1);
            let e: *mut Leaf<usize, usize> = Node::new_leaf(1);
            let f: *mut Leaf<usize, usize> = Node::new_leaf(1);
            let g: *mut Leaf<usize, usize> = Node::new_leaf(1);
            let h: *mut Leaf<usize, usize> = Node::new_leaf(1);

            unsafe {
                (*a).insert_or_update(10, 10, 10);
                (*b).insert_or_update(20, 20, 20);
                (*c).insert_or_update(30, 30, 30);
                (*d).insert_or_update(40, 40, 40);
                (*e).insert_or_update(50, 50, 50);
                (*f).insert_or_update(60, 60, 60);
                (*g).insert_or_update(70, 70, 70);
                (*h).insert_or_update(80, 80, 80);
            }

            let branch: *mut Branch<usize, usize> = Node::new_branch(
                1,
                a as *mut Node<usize, usize>,
                b as *mut Node<usize, usize>,
            );
            let branch_ref = unsafe { &mut *branch };
            branch_ref.add_node(c as *mut Node<usize, usize>);
            branch_ref.add_node(d as *mut Node<usize, usize>);
            branch_ref.add_node(e as *mut Node<usize, usize>);
            branch_ref.add_node(f as *mut Node<usize, usize>);
            branch_ref.add_node(g as *mut Node<usize, usize>);
            branch_ref.add_node(h as *mut Node<usize, usize>);

            assert!(branch_ref.slots() == H_CAPACITY);

            $fun(branch_ref, 80);

            // MUST NOT verify here, as it's a use after free of the tests inserted node!
            Branch::free(branch as *mut _);
            Leaf::free(a as *mut _);
            Leaf::free(b as *mut _);
            Leaf::free(c as *mut _);
            Leaf::free(d as *mut _);
            Leaf::free(e as *mut _);
            Leaf::free(f as *mut _);
            Leaf::free(g as *mut _);
            Leaf::free(h as *mut _);
            assert_released();
        }};
    }

    #[test]
    fn test_hashmap2_node_branch_add_split_min() {
        // Used in rev split
    }

    #[test]
    fn test_hashmap2_node_branch_add_split_mid() {
        test_max_leaf!(|branch_ref: &mut Branch<usize, usize>, max: u64| {
            let node: *mut Leaf<usize, usize> = Node::new_leaf(1);
            // Branch already has up to H_CAPACITY, incs of 10
            unsafe {
                (*node).insert_or_update(15, 15, 15);
            };

            // Add in the middle
            let r = branch_ref.add_node(node as *mut _);
            match r {
                BranchInsertState::Split(x, y) => {
                    unsafe {
                        assert!((*x).min() == max - 10);
                        assert!((*y).min() == max);
                    }
                    // X, Y will be freed by the macro caller.
                }
                _ => debug_assert!(false),
            };
            assert!(branch_ref.verify());
            // Free node.
            Leaf::free(node as *mut _);
        })
    }

    #[test]
    fn test_hashmap2_node_branch_add_split_max() {
        test_max_leaf!(|branch_ref: &mut Branch<usize, usize>, max: u64| {
            let node: *mut Leaf<usize, usize> = Node::new_leaf(1);
            // Branch already has up to H_CAPACITY, incs of 10
            unsafe {
                (*node).insert_or_update(200, 200, 200);
            };

            // Add in at the end.
            let r = branch_ref.add_node(node as *mut _);
            match r {
                BranchInsertState::Split(y, mynode) => {
                    unsafe {
                        // println!("{:?}", (*y).min());
                        // println!("{:?}", (*mynode).min());
                        assert!((*y).min() == max);
                        assert!((*mynode).min() == 200);
                    }
                    // Y will be freed by the macro caller.
                }
                _ => debug_assert!(false),
            };
            assert!(branch_ref.verify());
            // Free node.
            Leaf::free(node as *mut _);
        })
    }

    #[test]
    fn test_hashmap2_node_branch_add_split_n1max() {
        // Add one before the end!
        test_max_leaf!(|branch_ref: &mut Branch<usize, usize>, max: u64| {
            let node: *mut Leaf<usize, usize> = Node::new_leaf(1);
            // Branch already has up to H_CAPACITY, incs of 10
            let x = (max - 5) as usize;
            unsafe {
                (*node).insert_or_update(max - 5, x, x);
            };

            // Add in one before the end.
            let r = branch_ref.add_node(node as *mut _);
            match r {
                BranchInsertState::Split(mynode, y) => {
                    unsafe {
                        assert!((*mynode).min() == max - 5);
                        assert!((*y).min() == max);
                    }
                    // Y will be freed by the macro caller.
                }
                _ => debug_assert!(false),
            };
            assert!(branch_ref.verify());
            // Free node.
            Leaf::free(node as *mut _);
        })
    }
}
