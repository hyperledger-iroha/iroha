use super::states::*;
use crate::utils::*;
// use libc::{c_void, mprotect, PROT_READ, PROT_WRITE};
use crossbeam_utils::CachePadded;
use std::borrow::Borrow;
use std::fmt::{self, Debug, Error};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr;
use std::slice;

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
const FLAG_BRANCH: u64 = 0x1000_0000_0000_0000;
const FLAG_LEAF: u64 = 0x2000_0000_0000_0000;
const FLAG_INVALID: u64 = 0x4000_0000_0000_0000;
// const FLAG_HASH: u64 = 0x4000_0000_0000_0000;
// const FLAG_BUCKET: u64 = 0x8000_0000_0000_0000;
const FLAG_DROPPED: u64 = 0xaaaa_bbbb_cccc_dddd;

#[cfg(feature = "skinny")]
pub(crate) const L_CAPACITY: usize = 3;
#[cfg(feature = "skinny")]
pub(crate) const L_CAPACITY_N1: usize = L_CAPACITY - 1;
#[cfg(feature = "skinny")]
pub(crate) const BV_CAPACITY: usize = L_CAPACITY + 1;

#[cfg(not(feature = "skinny"))]
pub(crate) const L_CAPACITY: usize = 7;
#[cfg(not(feature = "skinny"))]
pub(crate) const L_CAPACITY_N1: usize = L_CAPACITY - 1;
#[cfg(not(feature = "skinny"))]
pub(crate) const BV_CAPACITY: usize = L_CAPACITY + 1;

#[cfg(all(test, not(miri)))]
thread_local!(static NODE_COUNTER: AtomicUsize = const { AtomicUsize::new(1) });
#[cfg(all(test, not(miri)))]
thread_local!(static ALLOC_LIST: Mutex<BTreeSet<usize>> = const { Mutex::new(BTreeSet::new()) });

#[cfg(all(test, not(miri)))]
fn alloc_nid() -> usize {
    let nid: usize = NODE_COUNTER.with(|nc| nc.fetch_add(1, Ordering::AcqRel));
    #[cfg(all(test, not(miri)))]
    {
        ALLOC_LIST.with(|llist| llist.lock().unwrap().insert(nid));
    }
    // eprintln!("Allocate -> {:?}", nid);
    nid
}

#[cfg(all(test, not(miri)))]
fn release_nid(nid: usize) {
    // println!("Release -> {:?}", nid);
    // debug_assert!(nid != 3);
    let r = ALLOC_LIST.with(|llist| llist.lock().unwrap().remove(&nid));
    assert!(r);
}

#[cfg(test)]
pub(crate) fn assert_released() {
    #[cfg(not(miri))]
    {
        let is_empt = ALLOC_LIST.with(|llist| {
            let x = llist.lock().unwrap();
            eprintln!("Assert Released - Remaining -> {:?}", x);
            x.is_empty()
        });
        assert!(is_empt);
    }
}

#[repr(C)]
pub(crate) struct Meta(u64);

#[repr(C)]
pub(crate) struct Branch<K, V>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    pub(crate) meta: Meta,
    key: [MaybeUninit<K>; L_CAPACITY],
    nodes: [*mut Node<K, V>; BV_CAPACITY],
    #[cfg(all(test, not(miri)))]
    pub(crate) nid: usize,
}

#[repr(C)]
pub(crate) struct Leaf<K, V>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    pub(crate) meta: Meta,
    key: [MaybeUninit<K>; L_CAPACITY],
    values: [MaybeUninit<V>; L_CAPACITY],
    #[cfg(all(test, not(miri)))]
    pub(crate) nid: usize,
}

#[repr(C)]
pub(crate) struct Node<K, V> {
    pub(crate) meta: Meta,
    k: PhantomData<K>,
    v: PhantomData<V>,
}

unsafe impl<K: Clone + Ord + Debug + Send + 'static, V: Clone + Send + 'static> Send
    for Node<K, V>
{
}
unsafe impl<K: Clone + Ord + Debug + Send + 'static, V: Clone + Sync + Send + 'static> Sync
    for Node<K, V>
{
}

/*
pub(crate) union NodeX<K, V>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    meta: Meta,
    leaf: Leaf<K, V>,
    branch: Branch<K, V>,
}
*/

impl<K: Clone + Ord + Debug, V: Clone> Node<K, V> {
    pub(crate) fn new_leaf(txid: u64) -> *mut Leaf<K, V> {
        // println!("Req new leaf");
        debug_assert!(txid < (TXID_MASK >> TXID_SHF));
        let x: Box<CachePadded<Leaf<K, V>>> = Box::new(CachePadded::new(Leaf {
            meta: Meta((txid << TXID_SHF) | FLAG_LEAF),
            key: unsafe { MaybeUninit::uninit().assume_init() },
            values: unsafe { MaybeUninit::uninit().assume_init() },
            #[cfg(all(test, not(miri)))]
            nid: alloc_nid(),
        }));
        Box::into_raw(x) as *mut Leaf<K, V>
    }

    fn new_leaf_ins(flags: u64, k: K, v: V) -> *mut Leaf<K, V> {
        // println!("Req new leaf ins");
        // debug_assert!(false);
        debug_assert!((flags & FLAG_MASK) == FLAG_LEAF);
        // Let the flag, txid and the count of value 1 through.
        let txid = flags & (TXID_MASK | FLAG_MASK | 1);
        let x: Box<CachePadded<Leaf<K, V>>> = Box::new(CachePadded::new(Leaf {
            meta: Meta(txid),
            #[cfg(feature = "skinny")]
            key: [
                MaybeUninit::new(k),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
            ],
            #[cfg(not(feature = "skinny"))]
            key: [
                MaybeUninit::new(k),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
            ],
            #[cfg(feature = "skinny")]
            values: [
                MaybeUninit::new(v),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
            ],
            #[cfg(not(feature = "skinny"))]
            values: [
                MaybeUninit::new(v),
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

    pub(crate) fn new_branch(
        txid: u64,
        l: *mut Node<K, V>,
        r: *mut Node<K, V>,
    ) -> *mut Branch<K, V> {
        // println!("Req new branch");
        debug_assert!(!l.is_null());
        debug_assert!(!r.is_null());
        debug_assert!(Node::verify_raw(l));
        debug_assert!(Node::verify_raw(r));
        debug_assert!(txid < (TXID_MASK >> TXID_SHF));
        let x: Box<CachePadded<Branch<K, V>>> = Box::new(CachePadded::new(Branch {
            // This sets the default (key) count to 1, since we take an l/r
            meta: Meta((txid << TXID_SHF) | FLAG_BRANCH | 1),
            #[cfg(feature = "skinny")]
            key: [
                MaybeUninit::new(unsafe { &*Node::min_raw(r) }.clone()),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
            ],
            #[cfg(not(feature = "skinny"))]
            key: [
                MaybeUninit::new(unsafe { &*Node::min_raw(r) }.clone()),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
            ],
            #[cfg(feature = "skinny")]
            nodes: [l, r, ptr::null_mut(), ptr::null_mut()],
            #[cfg(not(feature = "skinny"))]
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
        let output = Box::into_raw(x) as *mut Branch<K, V>;
        debug_assert!(Self::verify_raw(output as *const _));
        output
    }

    #[inline(always)]
    pub(crate) fn make_ro_raw(pointer: *const Self) {
        match unsafe { &*pointer }.meta.0 & FLAG_MASK {
            FLAG_LEAF => Leaf::<K, V>::make_ro_raw(pointer as *const _),
            FLAG_BRANCH => Branch::<K, V>::make_ro_raw(pointer as *const _),
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    #[cfg(test)]
    pub(crate) fn get_txid(&self) -> u64 {
        self.meta.get_txid()
    }

    #[inline(always)]
    pub(crate) fn is_leaf(&self) -> bool {
        self.meta.is_leaf()
    }

    #[allow(unused)]
    #[inline(always)]
    pub(crate) fn is_branch(&self) -> bool {
        self.meta.is_branch()
    }

    #[cfg(test)]
    pub(crate) fn tree_density_raw(pointer: *const Self) -> (usize, usize) {
        match unsafe { &*pointer }.meta.0 & FLAG_MASK {
            FLAG_LEAF => {
                let lref = unsafe { &*(pointer as *const Leaf<K, V>) };
                (lref.count(), L_CAPACITY)
            }
            FLAG_BRANCH => {
                let bref = unsafe { &*(pointer as *const Branch<K, V>) };
                let mut lcount = 0; // leaf populated
                let mut mcount = 0; // leaf max possible
                for idx in 0..(bref.count() + 1) {
                    let n = bref.nodes[idx];
                    let (l, m) = Self::tree_density_raw(n);
                    lcount += l;
                    mcount += m;
                }
                (lcount, mcount)
            }
            _ => unreachable!(),
        }
    }

    /*
    pub(crate) fn leaf_count(&self) -> usize {
        match self.meta.0 & FLAG_MASK {
            FLAG_LEAF => 1,
            FLAG_BRANCH => {
                let bref = unsafe { &*(self as *const _ as *const Branch<K, V>) };
                let mut lcount = 0; // leaf count
                for idx in 0..(bref.count() + 1) {
                    let n = bref.nodes[idx] as *mut Node<K, V>;
                    lcount += unsafe { (*n).leaf_count() };
                }
                lcount
            }
            _ => unreachable!(),
        }
    }
    */

    #[cfg(test)]
    #[inline(always)]
    pub(crate) fn get_ref_raw<Q: ?Sized + Ord>(pointer: *const Self, k: &Q) -> Option<*const V>
    where
        K: Borrow<Q>,
    {
        match unsafe { &*pointer }.meta.0 & FLAG_MASK {
            FLAG_LEAF => Leaf::<K, V>::get_ref_raw(pointer as *const _, k),
            FLAG_BRANCH => Branch::<K, V>::get_ref_raw(pointer as *const _, k),
            _ => {
                // println!("FLAGS: {:x}", self.meta.0);
                unreachable!()
            }
        }
    }

    #[inline(always)]
    pub(crate) fn min(&self) -> &K {
        match self.meta.0 & FLAG_MASK {
            FLAG_LEAF => {
                let lref = unsafe { &*(self as *const _ as *const Leaf<K, V>) };
                lref.min()
            }
            FLAG_BRANCH => {
                let bref = unsafe { &*(self as *const _ as *const Branch<K, V>) };
                bref.min()
            }
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    pub(crate) fn min_raw(pointer: *const Self) -> *const K {
        match unsafe { &*pointer }.meta.0 & FLAG_MASK {
            FLAG_LEAF => Leaf::<K, V>::min_raw(pointer as *const _),
            FLAG_BRANCH => Branch::<K, V>::min_raw(pointer as *const _),
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    pub(crate) fn max_raw(pointer: *const Self) -> *const K {
        match unsafe { &*pointer }.meta.0 & FLAG_MASK {
            FLAG_LEAF => Leaf::<K, V>::max_raw(pointer as *const _),
            FLAG_BRANCH => Branch::<K, V>::max_raw(pointer as *const _),
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    pub(crate) fn verify_raw(pointer: *const Self) -> bool {
        match unsafe { &*pointer }.meta.0 & FLAG_MASK {
            FLAG_LEAF => Leaf::<K, V>::verify_raw(pointer as *const _),
            FLAG_BRANCH => Branch::<K, V>::verify_raw(pointer as *const _),
            _ => unreachable!(),
        }
    }

    #[cfg(test)]
    fn no_cycles_inner_raw(pointer: *const Self, track: &mut BTreeSet<*const Self>) -> bool {
        match unsafe { &*pointer }.meta.0 & FLAG_MASK {
            FLAG_LEAF => {
                // check if we are in the set?
                track.insert(pointer)
            }
            FLAG_BRANCH => {
                if track.insert(pointer) {
                    // check
                    let bref = unsafe { &*(pointer as *const Branch<K, V>) };
                    for i in 0..(bref.count() + 1) {
                        let n = bref.nodes[i];
                        let r = Node::no_cycles_inner_raw(n, track);
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
    pub(crate) fn no_cycles_raw(pointer: *const Self) -> bool {
        let mut track = BTreeSet::new();
        Self::no_cycles_inner_raw(pointer, &mut track)
    }

    pub(crate) fn sblock_collect(&mut self, alloc: &mut Vec<*mut Node<K, V>>) {
        // Reset our txid.
        // self.meta.0 &= FLAG_MASK | COUNT_MASK;
        // self.meta.0 |= txid << TXID_SHF;

        if (self.meta.0 & FLAG_MASK) == FLAG_BRANCH {
            let bref = unsafe { &*(self as *const _ as *const Branch<K, V>) };
            for idx in 0..(bref.count() + 1) {
                alloc.push(bref.nodes[idx]);
                let n = bref.nodes[idx];
                unsafe { (*n).sblock_collect(alloc) };
            }
        }
    }

    pub(crate) fn sblock_collect_raw(pointer: *mut Self, alloc: &mut Vec<*mut Node<K, V>>) {
        // Reset our txid.
        // self.meta.0 &= FLAG_MASK | COUNT_MASK;
        // self.meta.0 |= txid << TXID_SHF;

        if (unsafe { &*pointer }.meta.0 & FLAG_MASK) == FLAG_BRANCH {
            let bref = unsafe { &*(pointer as *const Branch<K, V>) };
            for idx in 0..(bref.count() + 1) {
                alloc.push(bref.nodes[idx]);
                let n = bref.nodes[idx];
                Node::sblock_collect_raw(n, alloc);
            }
        }
    }

    pub(crate) fn free(node: *mut Node<K, V>) {
        let self_meta = self_meta!(node);
        match self_meta.0 & FLAG_MASK {
            FLAG_LEAF => Leaf::free(node as *mut Leaf<K, V>),
            FLAG_BRANCH => Branch::free(node as *mut Branch<K, V>),
            _ => unreachable!(),
        }
    }
}

impl Meta {
    #[inline(always)]
    fn set_count(&mut self, c: usize) {
        debug_assert!(c < 16);
        // Zero the bits in the flag from the count.
        self.0 &= FLAG_MASK | TXID_MASK;
        // Assign them.
        self.0 |= c as u8 as u64;
    }

    #[inline(always)]
    pub(crate) fn count(&self) -> usize {
        (self.0 & COUNT_MASK) as usize
    }

    #[inline(always)]
    fn add_count(&mut self, x: usize) {
        self.set_count(self.count() + x);
    }

    #[inline(always)]
    fn inc_count(&mut self) {
        debug_assert!(self.count() < 15);
        // Since count is the lowest bits, we can just inc
        // dec this as normal.
        self.0 += 1;
    }

    #[inline(always)]
    fn dec_count(&mut self) {
        debug_assert!(self.count() > 0);
        self.0 -= 1;
    }

    #[inline(always)]
    pub(crate) fn get_txid(&self) -> u64 {
        (self.0 & TXID_MASK) >> TXID_SHF
    }

    #[inline(always)]
    pub(crate) fn is_leaf(&self) -> bool {
        (self.0 & FLAG_MASK) == FLAG_LEAF
    }

    #[inline(always)]
    pub(crate) fn is_branch(&self) -> bool {
        (self.0 & FLAG_MASK) == FLAG_BRANCH
    }
}

impl<K: Ord + Clone + Debug, V: Clone> Leaf<K, V> {
    #[inline(always)]
    #[cfg(test)]
    fn set_count(&mut self, c: usize) {
        debug_assert_leaf!(self);
        self.meta.set_count(c)
    }

    #[inline(always)]
    pub(crate) fn count(&self) -> usize {
        debug_assert_leaf!(self);
        self.meta.count()
    }

    #[inline(always)]
    fn inc_count(&mut self) {
        debug_assert_leaf!(self);
        self.meta.inc_count()
    }

    #[inline(always)]
    fn dec_count(&mut self) {
        debug_assert_leaf!(self);
        self.meta.dec_count()
    }

    #[inline(always)]
    pub(crate) fn get_txid(&self) -> u64 {
        debug_assert_leaf!(self);
        self.meta.get_txid()
    }

    pub(crate) fn locate<Q>(&self, k: &Q) -> Result<usize, usize>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        debug_assert_leaf!(self);
        key_search!(self, k)
    }

    pub(crate) fn get_ref<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        debug_assert_leaf!(self);
        key_search!(self, k)
            .ok()
            .map(|idx| unsafe { &*self.values[idx].as_ptr() })
    }

    #[cfg(test)]
    pub(crate) fn get_ref_raw<Q>(pointer: *const Self, k: &Q) -> Option<*const V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        let this = unsafe { &*pointer };
        debug_assert_leaf!(this);
        key_search!(this, k)
            .ok()
            .map(|idx| this.values[idx].as_ptr())
    }

    pub(crate) fn get_mut_ref<Q>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        debug_assert_leaf!(self);
        key_search!(self, k)
            .ok()
            .map(|idx| unsafe { &mut *self.values[idx].as_mut_ptr() })
    }

    #[inline(always)]
    pub(crate) fn get_kv_idx_checked(&self, idx: usize) -> Option<(&K, &V)> {
        debug_assert_leaf!(self);
        if idx < self.count() {
            Some((unsafe { &*self.key[idx].as_ptr() }, unsafe {
                &*self.values[idx].as_ptr()
            }))
        } else {
            None
        }
    }

    pub(crate) fn min(&self) -> &K {
        debug_assert!(self.count() > 0);
        unsafe { &*self.key[0].as_ptr() }
    }

    pub(crate) fn min_raw(pointer: *const Self) -> *const K {
        unsafe { &*pointer }.min()
    }

    #[cfg(test)]
    pub(crate) fn max(&self) -> &K {
        debug_assert!(self.count() > 0);
        unsafe { &*self.key[self.count() - 1].as_ptr() }
    }

    pub(crate) fn max_raw(pointer: *const Self) -> *const K {
        let this = unsafe { &*pointer };
        debug_assert!(this.count() > 0);
        unsafe { &*this.key[this.count() - 1].as_ptr() }
    }

    pub(crate) fn min_value(&self) -> Option<(&K, &V)> {
        if self.count() > 0 {
            self.get_kv_idx_checked(0)
        } else {
            None
        }
    }

    pub(crate) fn max_value(&self) -> Option<(&K, &V)> {
        if self.count() > 0 {
            self.get_kv_idx_checked(self.count() - 1)
        } else {
            None
        }
    }

    pub(crate) fn req_clone(&self, txid: u64) -> Option<*mut Node<K, V>> {
        debug_assert_leaf!(self);
        debug_assert!(txid < (TXID_MASK >> TXID_SHF));
        if self.get_txid() == txid {
            // Same txn, no action needed.
            None
        } else {
            debug_assert!(txid > self.get_txid());
            // eprintln!("Req clone leaf");
            // debug_assert!(false);
            // Diff txn, must clone.
            // # https://github.com/kanidm/concread/issues/55
            // We flag the node as unable to drop it's internals.
            let new_txid =
                (self.meta.0 & (FLAG_MASK | COUNT_MASK)) | (txid << TXID_SHF) | FLAG_INVALID;
            let mut x: Box<CachePadded<Leaf<K, V>>> = Box::new(CachePadded::new(Leaf {
                // Need to preserve count.
                meta: Meta(new_txid),
                key: unsafe { MaybeUninit::uninit().assume_init() },
                values: unsafe { MaybeUninit::uninit().assume_init() },
                #[cfg(all(test, not(miri)))]
                nid: alloc_nid(),
            }));

            debug_assert!((x.meta.0 & FLAG_INVALID) != 0);

            // Copy in the values to the correct location.
            for idx in 0..self.count() {
                unsafe {
                    let lkey = (*self.key[idx].as_ptr()).clone();
                    x.key[idx].as_mut_ptr().write(lkey);
                    let lvalue = (*self.values[idx].as_ptr()).clone();
                    x.values[idx].as_mut_ptr().write(lvalue);
                }
            }
            // Finally undo the invalid flag to allow drop to proceed.
            x.meta.0 &= !FLAG_INVALID;

            debug_assert!((x.meta.0 & FLAG_INVALID) == 0);

            Some(Box::into_raw(x) as *mut Node<K, V>)
        }
    }

    pub(crate) fn insert_or_update(&mut self, k: K, v: V) -> LeafInsertState<K, V> {
        debug_assert_leaf!(self);
        // Find the location we need to update
        let r = key_search!(self, &k);
        match r {
            Ok(idx) => {
                // It exists at idx, replace
                let prev = unsafe { self.values[idx].as_mut_ptr().replace(v) };
                // Prev now contains the original value, return it!
                LeafInsertState::Ok(Some(prev))
            }
            Err(idx) => {
                if self.count() >= L_CAPACITY {
                    // Overflow to a new node
                    if idx >= self.count() {
                        // Greater than all else, split right
                        let rnode = Node::new_leaf_ins(self.meta.0, k, v);
                        LeafInsertState::Split(rnode)
                    } else if idx == 0 {
                        // Lower than all else, split left.
                        // let lnode = ...;
                        let lnode = Node::new_leaf_ins(self.meta.0, k, v);
                        LeafInsertState::RevSplit(lnode)
                    } else {
                        // Within our range, pop max, insert, and split
                        // right.
                        let pk =
                            unsafe { slice_remove(&mut self.key, L_CAPACITY - 1).assume_init() };
                        let pv =
                            unsafe { slice_remove(&mut self.values, L_CAPACITY - 1).assume_init() };
                        unsafe {
                            slice_insert(&mut self.key, MaybeUninit::new(k), idx);
                            slice_insert(&mut self.values, MaybeUninit::new(v), idx);
                        }

                        let rnode = Node::new_leaf_ins(self.meta.0, pk, pv);
                        LeafInsertState::Split(rnode)
                    }
                } else {
                    // We have space.
                    unsafe {
                        slice_insert(&mut self.key, MaybeUninit::new(k), idx);
                        slice_insert(&mut self.values, MaybeUninit::new(v), idx);
                    }
                    self.inc_count();
                    LeafInsertState::Ok(None)
                }
            }
        }
    }

    pub(crate) fn remove<Q>(&mut self, k: &Q) -> LeafRemoveState<V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        debug_assert_leaf!(self);
        if self.count() == 0 {
            return LeafRemoveState::Shrink(None);
        }
        // We must have a value - where are you ....
        match key_search!(self, k).ok() {
            // Count still greater than 0, so Ok and None,
            None => LeafRemoveState::Ok(None),
            Some(idx) => {
                // Get the kv out
                let _pk = unsafe { slice_remove(&mut self.key, idx).assume_init() };
                let pv = unsafe { slice_remove(&mut self.values, idx).assume_init() };
                self.dec_count();
                if self.count() == 0 {
                    LeafRemoveState::Shrink(Some(pv))
                } else {
                    LeafRemoveState::Ok(Some(pv))
                }
            }
        }
    }

    pub(crate) fn take_from_l_to_r(&mut self, right: &mut Self) {
        debug_assert!(right.count() == 0);
        let count = self.count() / 2;
        let start_idx = self.count() - count;

        //move key and values
        unsafe {
            slice_move(&mut right.key, 0, &mut self.key, start_idx, count);
            slice_move(&mut right.values, 0, &mut self.values, start_idx, count);
        }

        // update the counts
        self.meta.set_count(start_idx);
        right.meta.set_count(count);
    }

    pub(crate) fn take_from_r_to_l(&mut self, right: &mut Self) {
        debug_assert!(self.count() == 0);
        let count = right.count() / 2;
        let start_idx = right.count() - count;

        // Move values from right to left.
        unsafe {
            slice_move(&mut self.key, 0, &mut right.key, 0, count);
            slice_move(&mut self.values, 0, &mut right.values, 0, count);
        }
        // Shift the values in right down.
        unsafe {
            ptr::copy(
                right.key.as_ptr().add(count),
                right.key.as_mut_ptr(),
                start_idx,
            );
            ptr::copy(
                right.values.as_ptr().add(count),
                right.values.as_mut_ptr(),
                start_idx,
            );
        }

        // Fix the counts.
        self.meta.set_count(count);
        right.meta.set_count(start_idx);
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
    pub(crate) fn make_ro_raw(pointer: *const Self) {
        debug_assert_leaf!(unsafe { &*pointer });
        /*
        let r = unsafe {
            mprotect(
                this as *const Leaf<K, V> as *mut c_void,
                size_of::<Leaf<K, V>>(),
                PROT_READ
            )
        };
        assert!(r == 0);
        */
    }

    #[inline(always)]
    pub(crate) fn merge(&mut self, right: &mut Self) {
        debug_assert_leaf!(self);
        debug_assert_leaf!(right);
        let sc = self.count();
        let rc = right.count();
        unsafe {
            slice_merge(&mut self.key, sc, &mut right.key, rc);
            slice_merge(&mut self.values, sc, &mut right.values, rc);
        }
        self.meta.add_count(right.count());
        right.meta.set_count(0);
    }

    #[cfg(test)]
    pub(crate) fn verify(&self) -> bool {
        debug_assert_leaf!(self);
        // println!("verify leaf -> {:?}", self);
        // Check key sorting
        if self.meta.count() == 0 {
            return true;
        }
        let mut lk: &K = unsafe { &*self.key[0].as_ptr() };
        for work_idx in 1..self.meta.count() {
            let rk: &K = unsafe { &*self.key[work_idx].as_ptr() };
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

    pub(crate) fn verify_raw(pointer: *const Self) -> bool {
        debug_assert_leaf!(unsafe { &*pointer });
        // println!("verify leaf -> {:?}", self);
        // Check key sorting
        let count = unsafe { &*pointer }.meta.count();
        if count == 0 {
            return true;
        }
        let mut lk: &K = unsafe { &*(*pointer).key[0].as_ptr() };
        for work_idx in 1..count {
            let rk: &K = unsafe { &*(*pointer).key[work_idx].as_ptr() };
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

impl<K: Ord + Clone + Debug, V: Clone> Debug for Leaf<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), Error> {
        debug_assert_leaf!(self);
        write!(f, "Leaf -> {}", self.count())?;
        #[cfg(all(test, not(miri)))]
        write!(f, " nid: {}", self.nid)?;
        write!(f, "  \\-> [ ")?;
        for idx in 0..self.count() {
            write!(f, "{:?}, ", unsafe { &*self.key[idx].as_ptr() })?;
        }
        write!(f, " ]")
    }
}

impl<K: Ord + Clone + Debug, V: Clone> Drop for Leaf<K, V> {
    fn drop(&mut self) {
        debug_assert_leaf!(self);
        #[cfg(all(test, not(miri)))]
        release_nid(self.nid);
        // Due to the use of maybe uninit we have to drop any contained values.
        // https://github.com/kanidm/concread/issues/55
        // if we are invalid, do NOT drop our internals as they MAY be inconsistent.
        // this WILL leak memory, but it's better than crashing.
        if self.meta.0 & FLAG_INVALID == 0 {
            unsafe {
                for idx in 0..self.count() {
                    ptr::drop_in_place(self.key[idx].as_mut_ptr());
                    ptr::drop_in_place(self.values[idx].as_mut_ptr());
                }
            }
        }
        // Done
        self.meta.0 = FLAG_DROPPED;
        debug_assert!(self.meta.0 & FLAG_MASK != FLAG_LEAF);
        // #[cfg(test)]
        // println!("set leaf {:?} to {:x}", self.nid, self.meta.0);
    }
}

impl<K: Ord + Clone + Debug, V: Clone> Branch<K, V> {
    #[allow(unused)]
    #[inline(always)]
    fn set_count(&mut self, c: usize) {
        debug_assert_branch!(self);
        self.meta.set_count(c)
    }

    #[inline(always)]
    pub(crate) fn count(&self) -> usize {
        debug_assert_branch!(self);
        self.meta.count()
    }

    #[inline(always)]
    fn inc_count(&mut self) {
        debug_assert_branch!(self);
        self.meta.inc_count()
    }

    #[inline(always)]
    fn dec_count(&mut self) {
        debug_assert_branch!(self);
        self.meta.dec_count()
    }

    #[inline(always)]
    pub(crate) fn get_txid(&self) -> u64 {
        debug_assert_branch!(self);
        self.meta.get_txid()
    }

    // Can't inline as this is recursive!
    pub(crate) fn min(&self) -> &K {
        debug_assert_branch!(self);
        unsafe { (*self.nodes[0]).min() }
    }

    pub(crate) fn min_raw(pointer: *const Self) -> *const K {
        let this = unsafe { &*pointer };
        debug_assert_branch!(this);
        Node::min_raw(this.nodes[0])
    }

    pub(crate) fn max_raw(pointer: *const Self) -> *const K {
        let this = unsafe { &*pointer };
        debug_assert_branch!(this);
        // Remember, self.count() is + 1 offset, so this gets
        // the max node
        Node::max_raw(this.nodes[this.count()])
    }

    pub(crate) fn min_node(&self) -> *mut Node<K, V> {
        self.nodes[0]
    }

    pub(crate) fn max_node(&self) -> *mut Node<K, V> {
        self.nodes[self.count()]
    }

    pub(crate) fn req_clone(&self, txid: u64) -> Option<*mut Node<K, V>> {
        debug_assert_branch!(self);
        if self.get_txid() == txid {
            // Same txn, no action needed.
            None
        } else {
            // println!("Req clone branch");
            // Diff txn, must clone.
            // # https://github.com/kanidm/concread/issues/55
            // We flag the node as unable to drop it's internals.
            let new_txid =
                (self.meta.0 & (FLAG_MASK | COUNT_MASK)) | (txid << TXID_SHF) | FLAG_INVALID;
            let mut x: Box<CachePadded<Branch<K, V>>> = Box::new(CachePadded::new(Branch {
                // Need to preserve count.
                meta: Meta(new_txid),
                key: unsafe { MaybeUninit::uninit().assume_init() },
                // We can simply clone the pointers.
                nodes: self.nodes,
                #[cfg(all(test, not(miri)))]
                nid: alloc_nid(),
            }));

            debug_assert!((x.meta.0 & FLAG_INVALID) != 0);

            // Copy in the keys to the correct location.
            for idx in 0..self.count() {
                unsafe {
                    let lkey = (*self.key[idx].as_ptr()).clone();
                    x.key[idx].as_mut_ptr().write(lkey);
                }
            }
            // Finally undo the invalid flag to allow drop to proceed.
            x.meta.0 &= !FLAG_INVALID;

            debug_assert!((x.meta.0 & FLAG_INVALID) == 0);

            Some(Box::into_raw(x) as *mut Node<K, V>)
        }
    }

    #[inline(always)]
    pub(crate) fn locate_node<Q>(&self, k: &Q) -> usize
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        debug_assert_branch!(self);
        match key_search!(self, k) {
            Err(idx) => idx,
            Ok(idx) => idx + 1,
        }
    }

    #[inline(always)]
    pub(crate) fn get_idx_unchecked(&self, idx: usize) -> *mut Node<K, V> {
        debug_assert_branch!(self);
        debug_assert!(idx <= self.count());
        debug_assert!(!self.nodes[idx].is_null());
        self.nodes[idx]
    }

    #[inline(always)]
    pub(crate) fn get_idx_checked(&self, idx: usize) -> Option<*mut Node<K, V>> {
        debug_assert_branch!(self);
        // Remember, that nodes can have +1 to count which is why <= here, not <.
        if idx <= self.count() {
            debug_assert!(!self.nodes[idx].is_null());
            Some(self.nodes[idx])
        } else {
            None
        }
    }

    #[cfg(test)]
    pub(crate) fn get_ref_raw<Q: ?Sized + Ord>(pointer: *const Self, k: &Q) -> Option<*const V>
    where
        K: Borrow<Q>,
    {
        let this = unsafe { &*pointer };
        debug_assert_branch!(this);
        // If the value is Ok(idx), then that means
        // we were located to the right node. This is because we
        // exactly hit and located on the key.
        //
        // If the value is Err(idx), then we have the exact index already.
        // as branches is of-by-one.
        let idx = this.locate_node(k);
        Node::get_ref_raw(this.nodes[idx], k)
    }

    pub(crate) fn add_node(&mut self, node: *mut Node<K, V>) -> BranchInsertState<K, V> {
        debug_assert_branch!(self);
        // do we have space?
        if self.count() == L_CAPACITY {
            // if no space ->
            //    split and send two nodes back for new branch
            // There are three possible states that this causes.
            // 1 * The inserted node is the greater than all current values, causing l(max, node)
            //     to be returned.
            // 2 * The inserted node is between max - 1 and max, causing l(node, max) to be returned.
            // 3 * The inserted node is a low/middle value, causing max and max -1 to be returned.
            //
            let kr = unsafe { &*Node::min_raw(node) };
            let r = key_search!(self, kr);
            let ins_idx = r.unwrap_err();
            // Everything will pop max.
            let max = unsafe { *(self.nodes.get_unchecked(BV_CAPACITY - 1)) };
            let res = match ins_idx {
                // Case 1
                L_CAPACITY => {
                    // println!("case 1");
                    // Greater than all current values, so we'll just return max and node.
                    let _kdrop =
                        unsafe { ptr::read(self.key.get_unchecked(L_CAPACITY - 1)).assume_init() };
                    // Now setup the ret val NOTICE compared to case 2 that we swap node and max?
                    BranchInsertState::Split(max, node)
                }
                // Case 2
                L_CAPACITY_N1 => {
                    // println!("case 2");
                    // Greater than all but max, so we return max and node in the correct order.
                    // Drop the key between them.
                    let _kdrop =
                        unsafe { ptr::read(self.key.get_unchecked(L_CAPACITY - 1)).assume_init() };
                    // Now setup the ret val NOTICE compared to case 1 that we swap node and max?
                    BranchInsertState::Split(node, max)
                }
                // Case 3
                ins_idx => {
                    // Get the max - 1 and max nodes out.
                    let maxn1 = unsafe { *(self.nodes.get_unchecked(BV_CAPACITY - 2)) };
                    // Drop the key between them.
                    let _kdrop =
                        unsafe { ptr::read(self.key.get_unchecked(L_CAPACITY - 1)).assume_init() };
                    // Drop the key before us that we are about to replace.
                    let _kdrop =
                        unsafe { ptr::read(self.key.get_unchecked(L_CAPACITY - 2)).assume_init() };
                    // Add node and it's key to the correct location.
                    let k: K = kr.clone();
                    let leaf_ins_idx = ins_idx + 1;
                    unsafe {
                        slice_insert(&mut self.key, MaybeUninit::new(k), ins_idx);
                        slice_insert(&mut self.nodes, node, leaf_ins_idx);
                    }

                    BranchInsertState::Split(maxn1, max)
                }
            };
            // Dec count as we always reduce branch by one as we split return
            // two.
            self.dec_count();
            res
        } else {
            // if space ->
            // Get the nodes min-key - we clone it because we'll certainly be inserting it!
            let k: K = unsafe { &*Node::min_raw(node) }.clone();
            // bst and find when min-key < key[idx]
            let r = key_search!(self, &k);
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
                slice_insert(&mut self.key, MaybeUninit::new(k), ins_idx);
                slice_insert(&mut self.nodes, node, leaf_ins_idx);
            }
            // finally update the count
            self.inc_count();
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
        if self.count() == L_CAPACITY {
            if sibidx == self.count() {
                // If sibidx == self.count, then we must be going into max - 1.
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
                let max = self.nodes[BV_CAPACITY - 1];
                let _kdrop =
                    unsafe { ptr::read(self.key.get_unchecked(L_CAPACITY - 1)).assume_init() };
                self.dec_count();
                BranchInsertState::Split(lnode, max)
            } else if sibidx == (self.count() - 1) {
                // If sibidx == (self.count - 1), then we must be going into max - 2
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
                let maxn1 = self.nodes[BV_CAPACITY - 2];
                let max = self.nodes[BV_CAPACITY - 1];
                let _kdrop =
                    unsafe { ptr::read(self.key.get_unchecked(L_CAPACITY - 1)).assume_init() };
                let _kdrop =
                    unsafe { ptr::read(self.key.get_unchecked(L_CAPACITY - 2)).assume_init() };
                self.dec_count();
                self.dec_count();
                //    [   k1, k2, k3, k4, dd, xx   ]    [   k6   ]
                //    [ v1, v2, v3, v4, v5, xx, xx ] -> [ v6, v7 ]
                let k: K = unsafe { (*lnode).min().clone() };

                unsafe {
                    slice_insert(&mut self.key, MaybeUninit::new(k), sibidx - 1);
                    slice_insert(&mut self.nodes, lnode, sibidx);
                    // slice_insert(&mut self.node, MaybeUninit::new(node), sibidx);
                }
                self.inc_count();
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
                let maxn1 = self.nodes[BV_CAPACITY - 2];
                let max = self.nodes[BV_CAPACITY - 1];
                let _kdrop =
                    unsafe { ptr::read(self.key.get_unchecked(L_CAPACITY - 1)).assume_init() };
                let _kdrop =
                    unsafe { ptr::read(self.key.get_unchecked(L_CAPACITY - 2)).assume_init() };
                self.dec_count();
                self.dec_count();

                // println!("pre-fixup -> {:?}", self);

                let sibnode = self.nodes[sibidx];
                let nkey: K = unsafe { &*Node::min_raw(sibnode) }.clone();

                unsafe {
                    slice_insert(&mut self.key, MaybeUninit::new(nkey), sibidx);
                    slice_insert(&mut self.nodes, lnode, sibidx);
                }

                self.inc_count();
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
            let nkey: K = unsafe { &*Node::min_raw(sibnode) }.clone();

            unsafe {
                slice_insert(&mut self.nodes, lnode, sibidx);
                slice_insert(&mut self.key, MaybeUninit::new(nkey), sibidx);
            }

            self.inc_count();
            // println!("post fixup -> {:?}", self);
            BranchInsertState::Ok
        }
    }

    fn remove_by_idx(&mut self, idx: usize) -> *mut Node<K, V> {
        debug_assert_branch!(self);
        debug_assert!(idx <= self.count());
        debug_assert!(idx > 0);
        // remove by idx.
        let _pk = unsafe { slice_remove(&mut self.key, idx - 1).assume_init() };
        let pn = unsafe { slice_remove(&mut self.nodes, idx) };
        self.dec_count();
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
        // This means rbranch issues a cloneshrink to root. clone shrink must contain the remainder
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
        debug_assert!(ridx > 0 && ridx <= self.count());
        let left = self.nodes[ridx - 1];
        let right = self.nodes[ridx];
        debug_assert!(!left.is_null());
        debug_assert!(!right.is_null());

        match unsafe { (*left).meta.0 & FLAG_MASK } {
            FLAG_LEAF => {
                let lmut = leaf_ref!(left, K, V);
                let rmut = leaf_ref!(right, K, V);

                if lmut.count() + rmut.count() <= L_CAPACITY {
                    lmut.merge(rmut);
                    // remove the right node from parent
                    let dnode = self.remove_by_idx(ridx);
                    debug_assert!(dnode == right);
                    if self.count() == 0 {
                        // We now need to be merged across as we only contain a single
                        // value now.
                        BranchShrinkState::Shrink(dnode)
                    } else {
                        // We are complete!
                        // #[cfg(test)]
                        // println!("🔥 {:?}", rmut.nid);
                        BranchShrinkState::Merge(dnode)
                    }
                } else if rmut.count() > (L_CAPACITY / 2) {
                    lmut.take_from_r_to_l(rmut);
                    self.rekey_by_idx(ridx);
                    BranchShrinkState::Balanced
                } else if lmut.count() > (L_CAPACITY / 2) {
                    lmut.take_from_l_to_r(rmut);
                    self.rekey_by_idx(ridx);
                    BranchShrinkState::Balanced
                } else {
                    // Do nothing
                    BranchShrinkState::Balanced
                }
            }
            FLAG_BRANCH => {
                // right or left is now in a "corrupt" state with a single value that we need to relocate
                // to left - or we need to borrow from left and fix it!
                let lmut = branch_ref!(left, K, V);
                let rmut = branch_ref!(right, K, V);

                debug_assert!(rmut.count() == 0 || lmut.count() == 0);
                debug_assert!(rmut.count() <= L_CAPACITY || lmut.count() <= L_CAPACITY);
                // println!("{:?} {:?}", lmut.count(), rmut.count());
                if lmut.count() == L_CAPACITY {
                    lmut.take_from_l_to_r(rmut);
                    self.rekey_by_idx(ridx);
                    BranchShrinkState::Balanced
                } else if rmut.count() == L_CAPACITY {
                    lmut.take_from_r_to_l(rmut);
                    self.rekey_by_idx(ridx);
                    BranchShrinkState::Balanced
                } else {
                    // merge the right to tail of left.
                    // println!("BL {:?}", lmut);
                    // println!("BR {:?}", rmut);
                    lmut.merge(rmut);
                    // println!("AL {:?}", lmut);
                    // println!("AR {:?}", rmut);
                    // Reduce our count
                    let dnode = self.remove_by_idx(ridx);
                    debug_assert!(dnode == right);
                    if self.count() == 0 {
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
        debug_assert!(idx <= self.count());
        debug_assert!(idx > 0);
        // For the node listed, rekey it.
        let nref = self.nodes[idx];
        let nkey = unsafe { &*Node::min_raw(nref) }.clone();
        unsafe {
            self.key[idx - 1].as_mut_ptr().write(nkey);
        }
    }

    #[inline(always)]
    pub(crate) fn merge(&mut self, right: &mut Self) {
        debug_assert_branch!(self);
        debug_assert_branch!(right);
        let sc = self.count();
        let rc = right.count();
        if rc == 0 {
            let node = right.nodes[0];
            debug_assert!(!node.is_null());
            let k: K = unsafe { &*Node::min_raw(node) }.clone();
            let ins_idx = self.count();
            let leaf_ins_idx = ins_idx + 1;
            unsafe {
                slice_insert(&mut self.key, MaybeUninit::new(k), ins_idx);
                slice_insert(&mut self.nodes, node, leaf_ins_idx);
            }
            self.inc_count();
        } else {
            debug_assert!(sc == 0);
            unsafe {
                // Move all the nodes from right.
                slice_merge(&mut self.nodes, 1, &mut right.nodes, rc + 1);
                // Move the related keys.
                slice_merge(&mut self.key, 1, &mut right.key, rc);
            }
            // Set our count correctly.
            self.meta.set_count(rc + 1);
            // Set right len to 0
            right.meta.set_count(0);
            // rekey the lowest pointer.
            unsafe {
                let nptr = self.nodes[1];
                let k: K = { &*Node::min_raw(nptr) }.clone();
                self.key[0].as_mut_ptr().write(k);
            }
            // done!
        }
    }

    pub(crate) fn take_from_l_to_r(&mut self, right: &mut Self) {
        debug_assert_branch!(self);
        debug_assert_branch!(right);
        debug_assert!(self.count() > right.count());
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
        let count = (self.count() + right.count()) / 2;
        let start_idx = self.count() - count;
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
        if count != 0 {
            right.nodes.swap(0, count);
        }
        // Move our values from the tail.
        // We would move 3 now to:
        //
        //    [   k1, k2, k3, k4, k5, k6   ]    [   --, --, --, --, ...
        //    [ v1, v2, v3, v4, --, --, -- ] -> [ v5, v6, v7, v8, --, ...
        //
        unsafe {
            slice_move(&mut right.nodes, 0, &mut self.nodes, start_idx + 1, count);
        }
        // Remove the keys from left.
        // So we need to remove the corresponding keys. so that we get.
        //
        //    [   k1, k2, k3, --, --, --   ]    [   --, --, --, --, ...
        //    [ v1, v2, v3, v4, --, --, -- ] -> [ v5, v6, v7, v8, --, ...
        //
        // This means it's start_idx - 1 up to BK cap

        for kidx in start_idx..L_CAPACITY {
            let _pk = unsafe { ptr::read(self.key.get_unchecked(kidx)).assume_init() };
            // They are dropped now.
        }
        // Adjust both counts - we do this before rekey to ensure that the safety
        // checks hold in debugging.
        right.meta.set_count(count);
        self.meta.set_count(start_idx);
        // Rekey right
        for kidx in 1..(count + 1) {
            right.rekey_by_idx(kidx);
        }
        // Done!
    }

    pub(crate) fn take_from_r_to_l(&mut self, right: &mut Self) {
        debug_assert_branch!(self);
        debug_assert_branch!(right);
        debug_assert!(right.count() >= self.count());

        let count = (self.count() + right.count()) / 2;
        let start_idx = right.count() - count;

        // We move count from right to left.
        unsafe {
            slice_move(&mut self.nodes, 1, &mut right.nodes, 0, count);
        }

        // Pop the excess keys in right
        // So say we had 6/7 in right, and 0/1 in left.
        //
        // We have a start_idx of 4, and count of 3.
        //
        // We moved 3 values from right, leaving 4. That means we need to remove
        // keys 0, 1, 2. The remaining keys are moved down.
        for kidx in 0..count {
            let _pk = unsafe { ptr::read(right.key.get_unchecked(kidx)).assume_init() };
            // They are dropped now.
        }

        // move keys down in right
        unsafe {
            let key = right.key.as_mut_ptr();
            ptr::copy(key.add(count), key, start_idx);
        }
        // move nodes down in right
        unsafe {
            let nodes = right.nodes.as_mut_ptr();
            ptr::copy(nodes.add(count), nodes, start_idx + 1);
        }

        // update counts
        right.meta.set_count(start_idx);
        self.meta.set_count(count);
        // Rekey left
        for kidx in 1..(count + 1) {
            self.rekey_by_idx(kidx);
        }
        // Done!
    }

    #[inline(always)]
    pub(crate) fn replace_by_idx(&mut self, idx: usize, node: *mut Node<K, V>) {
        debug_assert_branch!(self);
        debug_assert!(idx <= self.count());
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
        debug_assert!(idx <= self.count());
        let sib_ptr = self.nodes[idx];
        debug_assert!(!sib_ptr.is_null());
        // Do we need to clone?
        let res = match unsafe { (*sib_ptr).meta.0 & FLAG_MASK } {
            FLAG_LEAF => {
                let lref = unsafe { &*(sib_ptr as *const _ as *const Leaf<K, V>) };
                lref.req_clone(txid)
            }
            FLAG_BRANCH => {
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

        let sc = self.count();

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
                self.meta.set_count(sc - (idx + 1));

                if self.count() == 0 {
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
                        self.dec_count();
                        BranchTrimState::Complete
                    } else {
                        BranchTrimState::Complete
                    }
                } else if idx >= self.count() {
                    // remove everything except max.
                    unsafe {
                        // NEED MM
                        debug_assert!(false);
                        // We just drop all the keys.
                        for kidx in 0..self.count() {
                            ptr::drop_in_place(self.key[kidx].as_mut_ptr());
                            // ptr::drop_in_place(self.nodes[kidx].as_mut_ptr());
                        }
                        // Move the last node to the bottom.
                        self.nodes[0] = self.nodes[sc];
                    }
                    self.meta.set_count(0);
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
                        self.meta.set_count(sc - (idx + 1));
                    } else {
                        // NEED MM
                        debug_assert!(false);
                        unsafe {
                            slice_slide_and_drop(&mut self.key, idx - 1, sc - idx);
                            slice_slide(self.nodes.as_mut(), idx - 1, sc - (idx - 1));
                        }
                        self.meta.set_count(sc - idx);
                    }

                    if self.count() == 0 {
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

    #[inline(always)]
    pub(crate) fn make_ro_raw(pointer: *const Self) {
        debug_assert_branch!(unsafe { &*pointer });
        /*
        let r = unsafe {
            mprotect(
                this as *const Branch<K, V> as *mut c_void,
                size_of::<Branch<K, V>>(),
                PROT_READ
            )
        };
        assert!(r == 0);
        */
    }

    pub(crate) fn verify_raw(pointer: *const Self) -> bool {
        let this = unsafe { &*pointer };
        debug_assert_branch!(this);
        if this.count() == 0 {
            // Not possible to be valid!
            debug_assert!(false);
            return false;
        }
        // println!("verify branch -> {:?}", self);
        // Check we are sorted.
        let mut lk: &K = unsafe { &*this.key[0].as_ptr() };
        for work_idx in 1..this.count() {
            let rk: &K = unsafe { &*this.key[work_idx].as_ptr() };
            // println!("{:?} >= {:?}", lk, rk);
            if lk >= rk {
                debug_assert!(false);
                return false;
            }
            lk = rk;
        }
        // Recursively call verify
        for work_idx in 0..this.count() {
            let node = this.nodes[work_idx];
            if !Node::verify_raw(node) {
                for work_idx in 0..(this.count() + 1) {
                    let nref = this.nodes[work_idx];
                    if !Node::verify_raw(nref) {
                        // println!("Failed children");
                        debug_assert!(false);
                        return false;
                    }
                }
            }
        }
        // Check descendants are validly ordered.
        //                 V-- remember, there are count + 1 nodes.
        for work_idx in 0..this.count() {
            // get left max and right min
            let lnode = this.nodes[work_idx];
            let rnode = this.nodes[work_idx + 1];

            let pkey = unsafe { &*this.key[work_idx].as_ptr() };
            let lkey = unsafe { &*Node::max_raw(lnode as *const _) };
            let rkey = unsafe { &*Node::min_raw(rnode as *const _) };
            if lkey >= pkey || pkey > rkey {
                // println!("++++++");
                // println!("{:?} >= {:?}, {:?} > {:?}", lkey, pkey, pkey, rkey);
                // println!("out of order key found {}", work_idx);
                // println!("left --> {:?}", lnode);
                // println!("right -> {:?}", rnode);
                // println!("prnt  -> {:?}", self);
                debug_assert!(false);
                return false;
            }
        }
        // All good!
        true
    }

    fn free(node: *mut Self) {
        unsafe {
            let mut _x: Box<CachePadded<Branch<K, V>>> =
                Box::from_raw(node as *mut CachePadded<Branch<K, V>>);
        }
    }
}

impl<K: Ord + Clone + Debug, V: Clone> Debug for Branch<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), Error> {
        debug_assert_branch!(self);
        write!(f, "Branch -> {}", self.count())?;
        #[cfg(all(test, not(miri)))]
        write!(f, " nid: {}", self.nid)?;
        write!(f, "  \\-> [ ")?;
        for idx in 0..self.count() {
            write!(f, "{:?}, ", unsafe { &*self.key[idx].as_ptr() })?;
        }
        write!(f, " ]")
    }
}

impl<K: Ord + Clone + Debug, V: Clone> Drop for Branch<K, V> {
    fn drop(&mut self) {
        debug_assert_branch!(self);
        #[cfg(all(test, not(miri)))]
        release_nid(self.nid);
        // Due to the use of maybe uninit we have to drop any contained values.
        // https://github.com/kanidm/concread/issues/55
        // if we are invalid, do NOT drop our internals as they MAY be inconsistent.
        // this WILL leak memory, but it's better than crashing.
        if self.meta.0 & FLAG_INVALID == 0 {
            unsafe {
                for idx in 0..self.count() {
                    ptr::drop_in_place(self.key[idx].as_mut_ptr());
                }
            }
        }
        // Done
        self.meta.0 = FLAG_DROPPED;
        debug_assert!(self.meta.0 & FLAG_MASK != FLAG_BRANCH);
        // println!("set branch {:?} to {:x}", self.nid, self.meta.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bptree2_node_cache_size() {
        let ls = std::mem::size_of::<Leaf<u64, u64>>() - std::mem::size_of::<usize>();
        let bs = std::mem::size_of::<Branch<u64, u64>>() - std::mem::size_of::<usize>();
        #[cfg(feature = "skinny")]
        {
            assert!(ls <= 64);
            assert!(bs <= 64);
        }
        #[cfg(not(feature = "skinny"))]
        {
            assert!(ls <= 128);
            assert!(bs <= 128);
        }
    }

    #[test]
    fn test_bptree2_node_test_weird_basics() {
        let leaf_raw: *mut Leaf<u64, u64> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf_raw };

        assert!(leaf.get_txid() == 1);
        // println!("{:?}", leaf);

        leaf.set_count(1);
        assert!(leaf.count() == 1);
        leaf.set_count(0);
        assert!(leaf.count() == 0);

        leaf.inc_count();
        leaf.inc_count();
        leaf.inc_count();
        assert!(leaf.count() == 3);
        leaf.dec_count();
        leaf.dec_count();
        leaf.dec_count();
        assert!(leaf.count() == 0);

        /*
        let branch: *mut Branch<u64, u64> = Node::new_branch(1, ptr::null_mut(), ptr::null_mut());
        let branch = unsafe { &mut *branch };
        assert!(branch.get_txid() == 1);
        // println!("{:?}", branch);

        branch.set_count(3);
        assert!(branch.count() == 3);
        branch.set_count(0);
        assert!(branch.count() == 0);
        Branch::free(branch as *mut _);
        */

        Leaf::free(leaf_raw as *mut _);
        assert_released();
    }

    #[test]
    fn test_bptree2_node_leaf_in_order() {
        let leaf_raw: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf_raw };
        assert!(leaf.get_txid() == 1);
        // Check insert to capacity
        for kv in 0..L_CAPACITY {
            let r = leaf.insert_or_update(kv, kv);
            if let LeafInsertState::Ok(None) = r {
                assert!(leaf.get_ref(&kv) == Some(&kv));
            } else {
                panic!("Invalid Leaf InsertState");
            }
        }
        assert!(leaf.verify());
        // Check update to capacity
        for kv in 0..L_CAPACITY {
            let r = leaf.insert_or_update(kv, kv);
            if let LeafInsertState::Ok(Some(pkv)) = r {
                assert!(pkv == kv);
                assert!(leaf.get_ref(&kv) == Some(&kv));
            } else {
                panic!("Invalid Leaf InsertState");
            }
        }
        assert!(Leaf::<usize, usize>::verify_raw(leaf_raw));
        Leaf::free(leaf_raw);
        assert_released();
    }

    #[test]
    fn test_bptree2_node_leaf_out_of_order() {
        let leaf_raw: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf_raw };

        let kvs = [7, 5, 1, 6, 2, 3, 0, 8];
        assert!(leaf.get_txid() == 1);
        // Check insert to capacity
        for kv in kvs.iter().take(L_CAPACITY) {
            let r = leaf.insert_or_update(*kv, *kv);
            if let LeafInsertState::Ok(None) = r {
                assert!(leaf.get_ref(kv) == Some(kv));
            } else {
                panic!("Invalid Leaf InsertState");
            }
        }
        assert!(leaf.verify());
        assert!(leaf.count() == L_CAPACITY);
        // Check update to capacity
        for kv in kvs.iter().take(L_CAPACITY) {
            let r = leaf.insert_or_update(*kv, *kv);
            if let LeafInsertState::Ok(Some(pkv)) = r {
                assert!(&pkv == kv);
                assert!(leaf.get_ref(kv) == Some(kv));
            } else {
                panic!("Invalid Leaf InsertState");
            }
        }
        assert!(leaf.verify());
        assert!(leaf.count() == L_CAPACITY);
        Leaf::free(leaf_raw);
        assert_released();
    }

    #[test]
    fn test_bptree2_node_leaf_min() {
        let leaf_raw: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf_raw };

        let kvs = [3, 2, 6, 4, 5, 1, 9, 0];
        let min = [3, 2, 2, 2, 2, 1, 1, 0];

        for idx in 0..L_CAPACITY {
            let kv = kvs[idx];
            let r = leaf.insert_or_update(kv, kv);
            if let LeafInsertState::Ok(None) = r {
                assert!(leaf.get_ref(&kv) == Some(&kv));
                assert!(leaf.min() == &min[idx]);
            } else {
                panic!("Invalid Leaf InsertState");
            }
        }
        assert!(leaf.verify());
        assert!(leaf.count() == L_CAPACITY);
        Leaf::free(leaf_raw);
        assert_released();
    }

    #[test]
    fn test_bptree2_node_leaf_max() {
        let leaf_raw: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf_raw };

        let kvs = [1, 3, 2, 6, 4, 5, 9, 0];
        let max: [usize; 8] = [1, 3, 3, 6, 6, 6, 9, 9];

        for idx in 0..L_CAPACITY {
            let kv = kvs[idx];
            let r = leaf.insert_or_update(kv, kv);
            if let LeafInsertState::Ok(None) = r {
                assert!(leaf.get_ref(&kv) == Some(&kv));
                assert!(leaf.max() == &max[idx]);
            } else {
                panic!("Invalid Leaf InsertState");
            }
        }
        assert!(leaf.verify());
        assert!(leaf.count() == L_CAPACITY);
        Leaf::free(leaf_raw);
        assert_released();
    }

    #[test]
    fn test_bptree2_node_leaf_remove_order() {
        let leaf_raw: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf_raw };
        for kv in 0..L_CAPACITY {
            leaf.insert_or_update(kv, kv);
        }
        // Remove all but one.
        for kv in 0..(L_CAPACITY - 1) {
            let r = leaf.remove(&kv);
            if let LeafRemoveState::Ok(Some(rkv)) = r {
                assert!(rkv == kv);
            } else {
                panic!("Invalid Leaf RemoveState");
            }
        }
        assert!(leaf.count() == 1);
        assert!(leaf.max() == &(L_CAPACITY - 1));
        // Remove a non-existent value.
        let r = leaf.remove(&(L_CAPACITY + 20));
        if let LeafRemoveState::Ok(None) = r {
            // Ok!
        } else {
            panic!("Invalid Leaf RemoveState");
        }
        // Finally clear the node, should request a shrink.
        let kv = L_CAPACITY - 1;
        let r = leaf.remove(&kv);
        if let LeafRemoveState::Shrink(Some(rkv)) = r {
            assert!(rkv == kv);
        } else {
            panic!("Invalid Leaf RemoveState");
        }
        assert!(leaf.count() == 0);
        // Remove non-existent post shrink. Should never happen
        // but safety first!
        let r = leaf.remove(&0);
        if let LeafRemoveState::Shrink(None) = r {
            // Ok!
        } else {
            panic!("Invalid Leaf RemoveState");
        }

        assert!(leaf.count() == 0);
        assert!(leaf.verify());
        Leaf::free(leaf_raw as *mut _);
        assert_released();
    }

    #[test]
    fn test_bptree2_node_leaf_remove_out_of_order() {
        let leaf_raw: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf_raw };
        for kv in 0..L_CAPACITY {
            leaf.insert_or_update(kv, kv);
        }
        let mid = L_CAPACITY / 2;
        // This test removes all BUT one node to keep the states simple.
        for kv in mid..(L_CAPACITY - 1) {
            let r = leaf.remove(&kv);
            match r {
                LeafRemoveState::Ok(_) => {}
                _ => panic!(),
            }
        }

        for kv in 0..(L_CAPACITY / 2) {
            let r = leaf.remove(&kv);
            match r {
                LeafRemoveState::Ok(_) => {}
                _ => panic!(),
            }
        }

        assert!(leaf.count() == 1);
        assert!(leaf.verify());
        Leaf::free(leaf_raw);
        assert_released();
    }

    #[test]
    fn test_bptree2_node_leaf_insert_split() {
        let leaf_raw: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf = unsafe { &mut *leaf_raw };
        for kv in 0..L_CAPACITY {
            leaf.insert_or_update(kv + 10, kv + 10);
        }

        // Split right
        let r = leaf.insert_or_update(L_CAPACITY + 10, L_CAPACITY + 10);
        if let LeafInsertState::Split(rleaf) = r {
            unsafe {
                assert!((*rleaf).count() == 1);
            }
            Leaf::free(rleaf);
        } else {
            panic!();
        }

        // Split left
        let r = leaf.insert_or_update(0, 0);
        if let LeafInsertState::RevSplit(lleaf) = r {
            unsafe {
                assert!((*lleaf).count() == 1);
            }
            Leaf::free(lleaf);
        } else {
            panic!();
        }

        assert!(leaf.count() == L_CAPACITY);
        assert!(leaf.verify());
        Leaf::free(leaf_raw);
        assert_released();
    }

    /*
    #[test]
    fn test_bptree_leaf_remove_lt() {
        // This is used in split off.
        // Remove none
        let leaf1: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf1 = unsafe { &mut *leaf };
        for kv in 0..L_CAPACITY {
            let _ = leaf1.insert_or_update(kv + 10, kv);
        }
        leaf1.remove_lt(&5);
        assert!(leaf1.count() == L_CAPACITY);
        Leaf::free(leaf1 as *mut _);

        // Remove all
        let leaf2: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf2 = unsafe { &mut *leaf };
        for kv in 0..L_CAPACITY {
            let _ = leaf2.insert_or_update(kv + 10, kv);
        }
        leaf2.remove_lt(&(L_CAPACITY + 10));
        assert!(leaf2.count() == 0);
        Leaf::free(leaf2 as *mut _);

        // Remove from middle
        let leaf3: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf3 = unsafe { &mut *leaf };
        for kv in 0..L_CAPACITY {
            let _ = leaf3.insert_or_update(kv + 10, kv);
        }
        leaf3.remove_lt(&((L_CAPACITY / 2) + 10));
        assert!(leaf3.count() == (L_CAPACITY / 2));
        Leaf::free(leaf3 as *mut _);

        // Remove less than not in leaf.
        let leaf4: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let leaf4 = unsafe { &mut *leaf };
        let _ = leaf4.insert_or_update(5, 5);
        let _ = leaf4.insert_or_update(15, 15);
        leaf4.remove_lt(&10);
        assert!(leaf4.count() == 1);

        //  Add another and remove all.
        let _ = leaf4.insert_or_update(20, 20);
        leaf4.remove_lt(&25);
        assert!(leaf4.count() == 0);
        Leaf::free(leaf4 as *mut _);
        // Done!
        assert_released();
    }
    */

    /* ============================================ */
    // Branch tests here!

    #[test]
    fn test_bptree2_node_branch_new() {
        // Create a new branch, and test it.
        let left: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let left_ref = unsafe { &mut *left };
        let right: *mut Leaf<usize, usize> = Node::new_leaf(1);
        let right_ref = unsafe { &mut *right };

        // add kvs to l and r
        for kv in 0..L_CAPACITY {
            left_ref.insert_or_update(kv + 10, kv + 10);
            right_ref.insert_or_update(kv + 20, kv + 20);
        }
        // create branch
        let branch: *mut Branch<usize, usize> = Node::new_branch(
            1,
            left as *mut Node<usize, usize>,
            right as *mut Node<usize, usize>,
        );
        // verify
        assert!(Branch::<usize, usize>::verify_raw(branch));
        // Test .min works on our descendants
        assert!(unsafe { *Branch::<usize, usize>::min_raw(branch) } == 10);
        // Test .max works on our descendants.
        assert!(unsafe { *Branch::<usize, usize>::max_raw(branch) } == (20 + L_CAPACITY - 1));
        // Get some k within the leaves.
        assert!(
            Branch::<usize, usize>::get_ref_raw(branch, &11).map(|val| unsafe { *val }) == Some(11)
        );
        assert!(
            Branch::<usize, usize>::get_ref_raw(branch, &21).map(|val| unsafe { *val }) == Some(21)
        );
        // get some k that is out of bounds.
        assert!(Branch::<usize, usize>::get_ref_raw(branch, &1).is_none());
        assert!(Branch::<usize, usize>::get_ref_raw(branch, &100).is_none());

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
                (*a).insert_or_update(10, 10);
                (*b).insert_or_update(20, 20);
                (*c).insert_or_update(30, 30);
            }

            $fun(a, b, c);

            Leaf::free(a as *mut _);
            Leaf::free(b as *mut _);
            Leaf::free(c as *mut _);
            assert_released();
        }};
    }

    #[test]
    fn test_bptree2_node_branch_add_min() {
        // This pattern occurs with "revsplit" to help with reverse
        // ordered inserts.
        test_3_leaf!(|a, b, c| {
            // Add the max two to the branch
            let branch: *mut Branch<usize, usize> = Node::new_branch(
                1,
                b as *mut Node<usize, usize>,
                c as *mut Node<usize, usize>,
            );
            assert!(Branch::<usize, usize>::verify_raw(branch));
            let branch_ref = unsafe { &mut *branch };
            // verify
            // Now min node (uses a diff function!)
            let r = branch_ref.add_node_left(a as *mut Node<usize, usize>, 0);
            match r {
                BranchInsertState::Ok => {}
                _ => debug_assert!(false),
            };
            // Assert okay + verify
            assert!(Branch::<usize, usize>::verify_raw(branch));
            Branch::free(branch as *mut _);
        })
    }

    #[test]
    fn test_bptree2_node_branch_add_mid() {
        test_3_leaf!(|a, b, c| {
            // Add the outer two to the branch
            let branch: *mut Branch<usize, usize> = Node::new_branch(
                1,
                a as *mut Node<usize, usize>,
                c as *mut Node<usize, usize>,
            );
            assert!(Branch::<usize, usize>::verify_raw(branch));
            let branch_ref = unsafe { &mut *branch };
            // verify
            let r = branch_ref.add_node(b as *mut Node<usize, usize>);
            match r {
                BranchInsertState::Ok => {}
                _ => debug_assert!(false),
            };
            // Assert okay + verify
            assert!(Branch::<usize, usize>::verify_raw(branch));
            Branch::free(branch as *mut _);
        })
    }

    #[test]
    fn test_bptree2_node_branch_add_max() {
        test_3_leaf!(|a, b, c| {
            // add the bottom two
            let branch: *mut Branch<usize, usize> = Node::new_branch(
                1,
                a as *mut Node<usize, usize>,
                b as *mut Node<usize, usize>,
            );
            // verify
            assert!(Branch::<usize, usize>::verify_raw(branch));
            let branch_ref = unsafe { &mut *branch };
            let r = branch_ref.add_node(c as *mut Node<usize, usize>);
            match r {
                BranchInsertState::Ok => {}
                _ => debug_assert!(false),
            };
            // Assert okay + verify
            assert!(Branch::<usize, usize>::verify_raw(branch));
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

            #[cfg(not(feature = "skinny"))]
            let e: *mut Leaf<usize, usize> = Node::new_leaf(1);
            #[cfg(not(feature = "skinny"))]
            let f: *mut Leaf<usize, usize> = Node::new_leaf(1);
            #[cfg(not(feature = "skinny"))]
            let g: *mut Leaf<usize, usize> = Node::new_leaf(1);
            #[cfg(not(feature = "skinny"))]
            let h: *mut Leaf<usize, usize> = Node::new_leaf(1);

            unsafe {
                (*a).insert_or_update(10, 10);
                (*b).insert_or_update(20, 20);
                (*c).insert_or_update(30, 30);
                (*d).insert_or_update(40, 40);
                #[cfg(not(feature = "skinny"))]
                {
                    (*e).insert_or_update(50, 50);
                    (*f).insert_or_update(60, 60);
                    (*g).insert_or_update(70, 70);
                    (*h).insert_or_update(80, 80);
                }
            }

            let branch: *mut Branch<usize, usize> = Node::new_branch(
                1,
                a as *mut Node<usize, usize>,
                b as *mut Node<usize, usize>,
            );
            let branch_ref = unsafe { &mut *branch };
            branch_ref.add_node(c as *mut Node<usize, usize>);
            branch_ref.add_node(d as *mut Node<usize, usize>);

            #[cfg(not(feature = "skinny"))]
            {
                branch_ref.add_node(e as *mut Node<usize, usize>);
                branch_ref.add_node(f as *mut Node<usize, usize>);
                branch_ref.add_node(g as *mut Node<usize, usize>);
                branch_ref.add_node(h as *mut Node<usize, usize>);
            }

            assert!(branch_ref.count() == L_CAPACITY);

            #[cfg(feature = "skinny")]
            $fun(branch_ref, 40);
            #[cfg(not(feature = "skinny"))]
            $fun(branch_ref, 80);

            // MUST NOT verify here, as it's a use after free of the tests inserted node!
            Branch::free(branch as *mut _);
            Leaf::free(a as *mut _);
            Leaf::free(b as *mut _);
            Leaf::free(c as *mut _);
            Leaf::free(d as *mut _);
            #[cfg(not(feature = "skinny"))]
            {
                Leaf::free(e as *mut _);
                Leaf::free(f as *mut _);
                Leaf::free(g as *mut _);
                Leaf::free(h as *mut _);
            }
            assert_released();
        }};
    }

    #[test]
    fn test_bptree2_node_branch_add_split_min() {
        // Used in rev split
    }

    #[test]
    fn test_bptree2_node_branch_add_split_mid() {
        test_max_leaf!(|branch_ref: &mut Branch<usize, usize>, max: usize| {
            let node: *mut Leaf<usize, usize> = Node::new_leaf(1);
            // Branch already has up to L_CAPACITY, incs of 10
            unsafe {
                (*node).insert_or_update(15, 15);
            };

            // Add in the middle
            let r = branch_ref.add_node(node as *mut _);
            match r {
                BranchInsertState::Split(x, y) => {
                    unsafe {
                        assert!({ &*Node::<usize, usize>::min_raw(x as *const _) } == &(max - 10));
                        assert!({ &*Node::<usize, usize>::min_raw(y as *const _) } == &max);
                    }
                    // X, Y will be freed by the macro caller.
                }
                _ => debug_assert!(false),
            };
            assert!(Branch::<usize, usize>::verify_raw(branch_ref as *mut _));
            // Free node.
            Leaf::free(node as *mut _);
        })
    }

    #[test]
    fn test_bptree2_node_branch_add_split_max() {
        test_max_leaf!(|branch_ref: &mut Branch<usize, usize>, max: usize| {
            let node: *mut Leaf<usize, usize> = Node::new_leaf(1);
            // Branch already has up to L_CAPACITY, incs of 10
            unsafe {
                (*node).insert_or_update(200, 200);
            };

            // Add in at the end.
            let r = branch_ref.add_node(node as *mut _);
            match r {
                BranchInsertState::Split(y, mynode) => {
                    unsafe {
                        // println!("{:?}", (*y).min());
                        // println!("{:?}", (*mynode).min());

                        assert!({ &*Node::<usize, usize>::min_raw(y as *const _) } == &max);
                        assert!({ &*Node::<usize, usize>::min_raw(mynode as *const _) } == &200);
                    }
                    // Y will be freed by the macro caller.
                }
                _ => debug_assert!(false),
            };
            assert!(Branch::<usize, usize>::verify_raw(branch_ref as *mut _));
            // Free node.
            Leaf::free(node as *mut _);
        })
    }

    #[test]
    fn test_bptree2_node_branch_add_split_n1max() {
        // Add one before the end!
        test_max_leaf!(|branch_ref: &mut Branch<usize, usize>, max: usize| {
            let node: *mut Leaf<usize, usize> = Node::new_leaf(1);
            // Branch already has up to L_CAPACITY, incs of 10
            unsafe {
                (*node).insert_or_update(max - 5, max - 5);
            };

            // Add in one before the end.
            let r = branch_ref.add_node(node as *mut _);
            match r {
                BranchInsertState::Split(mynode, y) => {
                    unsafe {
                        assert!(
                            { &*Node::<usize, usize>::min_raw(mynode as *const _) } == &(max - 5)
                        );
                        assert!({ &*Node::<usize, usize>::min_raw(y as *const _) } == &max);
                    }
                    // Y will be freed by the macro caller.
                }
                _ => debug_assert!(false),
            };
            assert!(Branch::<usize, usize>::verify_raw(branch_ref as *mut _));
            // Free node.
            Leaf::free(node as *mut _);
        })
    }
}
