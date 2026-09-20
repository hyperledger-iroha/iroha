//! See the documentation for [BptreeMap]

#[cfg(feature = "asynch")]
pub mod asynch;

#[cfg(feature = "serde")]
use serde::{
    de::{Deserialize, Deserializer},
    ser::{Serialize, SerializeMap, Serializer},
};

#[cfg(feature = "serde")]
use crate::utils::MapCollector;

pub use crate::internals::lincowcell::OwnedWriteError;
use crate::internals::lincowcell::{
    LinCowCell, LinCowCellOwned, LinCowCellReadTxn, LinCowCellWriteTxn,
};

mod admission;
mod mode;

pub use crate::internals::bptree::allocation::{NodeCloning, NodeFunding};
pub use crate::internals::bptree::tracking::{FixedTrackingBuffer, TrackingBuffer};
pub use crate::internals::lincowcell::Untracked;
pub use admission::{
    AllocationDemand, BptreeMapCheckpoint, ClonePlanning, InsertAdmissionError, PlanningError,
};
pub use mode::{MapMode, Prepaid};

type MapCell<K, V, M> = LinCowCell<
    SuperBlock<K, V, M>,
    CursorRead<K, V, M>,
    CursorWrite<K, V, M>,
    <M as NodeFunding>::Charge,
>;
type MapRead<'a, K, V, M> = LinCowCellReadTxn<
    'a,
    SuperBlock<K, V, M>,
    CursorRead<K, V, M>,
    CursorWrite<K, V, M>,
    <M as NodeFunding>::Charge,
>;
type MapWrite<'a, K, V, M> = LinCowCellWriteTxn<
    'a,
    SuperBlock<K, V, M>,
    CursorRead<K, V, M>,
    CursorWrite<K, V, M>,
    <M as NodeFunding>::Charge,
>;

include!("impl.rs");

/// The exact unpublished successor of a synchronous [`BptreeMap`].
///
/// This move-only owner retains the original cursor allocation, working nodes,
/// base reader and root, so its immutable snapshot remains valid even if the
/// map is dropped. The cursor is allocated during original writer acquisition
/// and its same allocation moves through detach, refusal and reattachment.
/// Reattachment accepts only that original map and unchanged reader generation.
/// It does not recreate a cursor, copy entries or grant higher-level publication
/// authority. Destruction aborts the unpublished work.
pub struct BptreeMapOwned<K, V, M = Untracked>
where
    K: Ord + Clone + Debug + Sync + Send + 'static,
    V: Clone + Sync + Send + 'static,
    M: MapMode + NodeCloning<K, V>,
{
    inner:
        LinCowCellOwned<SuperBlock<K, V, M>, CursorRead<K, V, M>, CursorWrite<K, V, M>, M::Charge>,
}

impl<K: Clone + Ord + Debug + Sync + Send + 'static, V: Clone + Sync + Send + 'static, M>
    BptreeMapOwned<K, V, M>
where
    M: MapMode + NodeCloning<K, V>,
{
    /// Borrow a value from the original unpublished successor.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.inner.as_ref().search(key)
    }

    /// Borrow original entries in key order without reconstructing the successor.
    pub fn iter(&self) -> Iter<'_, K, V, M::Charge> {
        self.inner.as_ref().kv_iter()
    }

    /// Borrow an immutable snapshot of the retained successor.
    pub fn to_snapshot(&self) -> BptreeMapReadSnapshot<'_, K, V, M> {
        BptreeMapReadSnapshot {
            inner: SnapshotType::W(self.inner.as_ref()),
        }
    }
}

impl<K: Clone + Ord + Debug + Sync + Send + 'static, V: Clone + Sync + Send + 'static, M>
    BptreeMap<K, V, M>
where
    M: MapMode + NodeCloning<K, V>,
{
    /// Initiate a read transaction for the tree, concurrent to any
    /// other readers or writers.
    pub fn read(&self) -> BptreeMapReadTxn<'_, K, V, M> {
        let inner = self.inner.read();
        BptreeMapReadTxn { inner }
    }

    /// Reacquire the original writer without copying or allocating a successor.
    ///
    /// Both the physical map and its exact base reader generation must match.
    /// Every refusal returns the same retained owner. Acquisition never waits
    /// and never takes the reader mutex; `Busy` identifies writer contention.
    pub fn try_write_owned(
        &self,
        owned: BptreeMapOwned<K, V, M>,
    ) -> Result<BptreeMapWriteTxn<'_, K, V, M>, (BptreeMapOwned<K, V, M>, OwnedWriteError)> {
        self.inner
            .try_write_owned(owned.inner)
            .map(|inner| BptreeMapWriteTxn { inner })
            .map_err(|(inner, error)| (BptreeMapOwned { inner }, error))
    }

    /// Whether an unwind poisoned the map's original writer lock.
    pub fn is_poisoned(&self) -> bool {
        self.inner.is_poisoned()
    }
}

impl<K: Clone + Ord + Debug + Sync + Send + 'static, V: Clone + Sync + Send + 'static, M>
    BptreeMapWriteTxn<'_, K, V, M>
where
    M: MapMode + NodeCloning<K, V>,
{
    /// Commit the changes from this write transaction. Readers after this point
    /// will be able to perceive these changes.
    ///
    /// To abort (unstage changes), just do not call this function.
    pub fn commit(self) {
        // Reject a caught admitted-edit panic before consuming the cursor shell
        // or transferring any ownership into the published reader generation.
        self.inner.as_ref().assert_operable();
        self.inner.commit();
    }

    /// Retain the exact unpublished successor and release its original lock.
    ///
    /// This moves the original cursor and preallocated publication shell. It
    /// neither publishes changes nor clones keys or values.
    pub fn detach(self) -> BptreeMapOwned<K, V, M> {
        self.inner.as_ref().assert_operable();
        BptreeMapOwned {
            inner: self.inner.detach(),
        }
    }
}

impl<K: Clone + Ord + Debug + Sync + Send + 'static, V: Clone + Sync + Send + 'static>
    BptreeMap<K, V>
{
    /// Initiate a write transaction for the tree, exclusive to this
    /// writer, and concurrently to all existing reads.
    pub fn write(&self) -> BptreeMapWriteTxn<'_, K, V> {
        let inner = self.inner.write();
        BptreeMapWriteTxn { inner }
    }
}

#[cfg(feature = "serde")]
impl<K, V, M> Serialize for BptreeMapReadTxn<'_, K, V, M>
where
    M: MapMode + NodeCloning<K, V>,
    K: Serialize + Clone + Ord + Debug + Sync + Send + 'static,
    V: Serialize + Clone + Sync + Send + 'static,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_map(Some(self.len()))?;

        for (key, val) in self.iter() {
            state.serialize_entry(key, val)?;
        }

        state.end()
    }
}

#[cfg(feature = "serde")]
impl<K, V, M> Serialize for BptreeMap<K, V, M>
where
    M: MapMode + NodeCloning<K, V>,
    K: Serialize + Clone + Ord + Debug + Sync + Send + 'static,
    V: Serialize + Clone + Sync + Send + 'static,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.read().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, K, V> Deserialize<'de> for BptreeMap<K, V>
where
    K: Deserialize<'de> + Clone + Ord + Debug + Sync + Send + 'static,
    V: Deserialize<'de> + Clone + Sync + Send + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(MapCollector::new())
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Bound;

    use super::BptreeMap;
    use crate::internals::bptree::node::{assert_released, L_CAPACITY};
    // use rand::prelude::*;
    use rand::seq::SliceRandom;

    #[test]
    fn public_map_owners_account_for_original_policy_and_charge_thread_safety() {
        use std::{cell::Cell, marker::PhantomData, rc::Rc};

        struct Policy<C, P>(PhantomData<(C, P)>);
        impl<C, P> super::NodeFunding for Policy<C, P> {
            type Charge = C;

            fn take_node_charge(&mut self, _layout: std::alloc::Layout) -> C {
                unreachable!("type-only capability assertion")
            }
        }
        impl<C, P> super::NodeCloning<usize, usize> for Policy<C, P> {
            fn clone_key(&mut self, key: &usize) -> usize {
                *key
            }

            fn clone_value(&mut self, value: &usize) -> usize {
                *value
            }
        }
        type Mode<C, P> = super::Prepaid<Policy<C, P>>;
        fn send_sync<T: Send + Sync>() {}
        send_sync::<BptreeMap<usize, usize, Mode<usize, ()>>>();
        send_sync::<super::BptreeMapReadTxn<'_, usize, usize, Mode<usize, ()>>>();
        send_sync::<super::BptreeMapOwned<usize, usize, Mode<usize, ()>>>();
        #[cfg(feature = "asynch")]
        send_sync::<super::asynch::BptreeMap<usize, usize, Mode<usize, ()>>>();

        trait AmbiguousIfSend<A> {
            fn probe() {}
        }
        impl<T: ?Sized> AmbiguousIfSend<()> for T {}
        impl<T: ?Sized + Send> AmbiguousIfSend<u8> for T {}
        trait AmbiguousIfSync<A> {
            fn probe() {}
        }
        impl<T: ?Sized> AmbiguousIfSync<()> for T {}
        impl<T: ?Sized + Sync> AmbiguousIfSync<u8> for T {}
        let _ = <BptreeMap<usize, usize, Mode<Rc<()>, ()>> as AmbiguousIfSend<_>>::probe;
        let _ = <BptreeMap<usize, usize, Mode<Cell<usize>, ()>> as AmbiguousIfSend<_>>::probe;
        let _ = <BptreeMap<usize, usize, Mode<Cell<usize>, ()>> as AmbiguousIfSync<_>>::probe;
        let _ = <BptreeMap<usize, usize, Mode<usize, Rc<()>>> as AmbiguousIfSend<_>>::probe;
        let _ = <BptreeMap<usize, usize, Mode<usize, Cell<usize>>> as AmbiguousIfSync<_>>::probe;
        let _ = <super::BptreeMapReadTxn<'_, usize, usize, Mode<Rc<()>, ()>> as AmbiguousIfSend<
            _,
        >>::probe;
        let _ =
            <super::BptreeMapReadTxn<'_, usize, usize, Mode<Cell<usize>, ()>> as AmbiguousIfSync<
                _,
            >>::probe;
        let _ =
            <super::BptreeMapOwned<usize, usize, Mode<Rc<()>, ()>> as AmbiguousIfSend<_>>::probe;
        let _ = <super::BptreeMapOwned<usize, usize, Mode<Cell<usize>, ()>> as AmbiguousIfSync<
            _,
        >>::probe;
        #[cfg(feature = "asynch")]
        {
            let _ = <super::asynch::BptreeMap<usize, usize, Mode<Cell<usize>, ()>> as AmbiguousIfSend<_>>::probe;
            let _ = <super::asynch::BptreeMapReadTxn<'_, usize, usize, Mode<Rc<()>, ()>> as AmbiguousIfSync<_>>::probe;
        }
    }

    #[test]
    fn test_bptree2_map_basic_write() {
        let bptree: BptreeMap<usize, usize> = BptreeMap::new();
        {
            let mut bpwrite = bptree.write();
            // We should be able to insert.
            bpwrite.insert(0, 0);
            bpwrite.insert(1, 1);
            assert!(bpwrite.get(&0) == Some(&0));
            assert!(bpwrite.get(&1) == Some(&1));
            bpwrite.insert(2, 2);
            bpwrite.commit();
            // println!("commit");
        }
        {
            // Do a clear, but roll it back.
            let mut bpwrite = bptree.write();
            bpwrite.clear();
            // DO NOT commit, this triggers the rollback.
            // println!("post clear");
        }
        {
            let bpwrite = bptree.write();
            assert!(bpwrite.get(&0) == Some(&0));
            assert!(bpwrite.get(&1) == Some(&1));
            // println!("fin write");
        }
        std::mem::drop(bptree);
        assert_released();
    }

    #[test]
    fn test_bptree2_map_cursed_get_mut() {
        let bptree: BptreeMap<usize, usize> = BptreeMap::new();
        {
            let mut w = bptree.write();
            w.insert(0, 0);
            w.commit();
        }
        let r1 = bptree.read();
        {
            let mut w = bptree.write();
            let cursed_zone = w.get_mut(&0).unwrap();
            *cursed_zone = 1;
            // Correctly fails to work as it's a second borrow, which isn't
            // possible once w.remove occurs
            // w.remove(&0);
            // *cursed_zone = 2;
            w.commit();
        }
        let r2 = bptree.read();
        assert!(r1.get(&0) == Some(&0));
        assert!(r2.get(&0) == Some(&1));

        /*
        // Correctly fails to compile. PHEW!
        let fail = {
            let mut w = bptree.write();
            w.get_mut(&0).unwrap()
        };
        */
        std::mem::drop(r1);
        std::mem::drop(r2);
        std::mem::drop(bptree);
        assert_released();
    }

    #[test]
    fn test_bptree2_map_from_iter_1() {
        let ins: Vec<usize> = (0..(L_CAPACITY << 4)).collect();

        let map = BptreeMap::from_iter(ins.into_iter().map(|v| (v, v)));

        {
            let w = map.write();
            assert!(w.verify());
            println!("{:?}", w.tree_density());
        }
        // assert!(w.tree_density() == ((L_CAPACITY << 4), (L_CAPACITY << 4)));
        std::mem::drop(map);
        assert_released();
    }

    #[test]
    fn test_bptree2_map_from_iter_2() {
        let mut rng = rand::rng();
        let mut ins: Vec<usize> = (0..(L_CAPACITY << 4)).collect();
        ins.shuffle(&mut rng);

        let map = BptreeMap::from_iter(ins.into_iter().map(|v| (v, v)));

        {
            let w = map.write();
            assert!(w.verify());
            // w.compact_force();
            assert!(w.verify());
            // assert!(w.tree_density() == ((L_CAPACITY << 4), (L_CAPACITY << 4)));
        }

        std::mem::drop(map);
        assert_released();
    }

    fn bptree_map_basic_concurrency(lower: usize, upper: usize) {
        // Create a map
        let map = BptreeMap::new();

        // add values
        {
            let mut w = map.write();
            w.extend((0..lower).map(|v| (v, v)));
            w.commit();
        }

        // read
        let r = map.read();
        assert!(r.len() == lower);
        for i in 0..lower {
            assert!(r.contains_key(&i))
        }

        // Check a second write doesn't interfere
        {
            let mut w = map.write();
            w.extend((lower..upper).map(|v| (v, v)));
            w.commit();
        }

        assert!(r.len() == lower);

        // But a new write can see
        let r2 = map.read();
        assert!(r2.len() == upper);
        for i in 0..upper {
            assert!(r2.contains_key(&i))
        }

        // Now drain the tree, and the reader should be unaffected.
        {
            let mut w = map.write();
            for i in 0..upper {
                assert!(w.remove(&i).is_some())
            }
            w.commit();
        }

        // All consistent!
        assert!(r.len() == lower);
        assert!(r2.len() == upper);
        for i in 0..upper {
            assert!(r2.contains_key(&i))
        }

        let r3 = map.read();
        // println!("{:?}", r3.len());
        assert!(r3.is_empty());

        std::mem::drop(r);
        std::mem::drop(r2);
        std::mem::drop(r3);

        std::mem::drop(map);
        assert_released();
    }

    #[test]
    fn test_bptree2_map_acb_order() {
        // Need to ensure that txns are dropped in order.

        // Add data, enough to cause a split. All data should be *2
        let map = BptreeMap::new();
        // add values
        {
            let mut w = map.write();
            w.extend((0..(L_CAPACITY * 2)).map(|v| (v * 2, v * 2)));
            w.commit();
        }
        let ro_txn_a = map.read();

        // New write, add 1 val
        {
            let mut w = map.write();
            w.insert(1, 1);
            w.commit();
        }

        let ro_txn_b = map.read();
        // ro_txn_b now owns nodes from a

        // New write, update a value
        {
            let mut w = map.write();
            w.insert(1, 10001);
            w.commit();
        }

        let ro_txn_c = map.read();
        // ro_txn_c
        // Drop ro_txn_b
        assert!(ro_txn_b.verify());
        std::mem::drop(ro_txn_b);
        // Are both still valid?
        assert!(ro_txn_a.verify());
        assert!(ro_txn_c.verify());
        // Drop remaining
        std::mem::drop(ro_txn_a);
        std::mem::drop(ro_txn_c);
        std::mem::drop(map);
        assert_released();
    }

    #[test]
    fn test_bptree2_map_weird_txn_behaviour() {
        let map: BptreeMap<usize, usize> = BptreeMap::new();

        let mut wr = map.write();
        let rd = map.read();

        wr.insert(1, 1);
        assert!(rd.get(&1).is_none());
        wr.commit();
        assert!(rd.get(&1).is_none());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_bptree2_map_basic_concurrency_small() {
        bptree_map_basic_concurrency(100, 200)
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_bptree2_map_basic_concurrency_large() {
        bptree_map_basic_concurrency(10_000, 20_000)
    }

    #[test]
    fn test_bptree2_map_rangeiter_1() {
        let ins: Vec<usize> = (0..100).collect();

        let map = BptreeMap::from_iter(ins.into_iter().map(|v| (v, v)));

        {
            let w = map.write();
            assert!(w.range(0..100).count() == 100);
            assert!(w.range(25..100).count() == 75);
            assert!(w.range(0..75).count() == 75);
            assert!(w.range(25..75).count() == 50);
        }
        // assert!(w.tree_density() == ((L_CAPACITY << 4), (L_CAPACITY << 4)));
        std::mem::drop(map);
        assert_released();
    }

    #[test]
    fn test_bptree2_map_rangeiter_2() {
        let map = BptreeMap::from_iter([(3, ()), (4, ()), (0, ())]);

        let r = map.read();
        assert!(r.range(1..=2).count() == 0);
    }

    #[test]
    fn test_bptree2_map_rangeiter_3() {
        let map = BptreeMap::from_iter([0, 1, 2, 3, 4, 5, 6, 8].map(|v| (v, ())));

        let r = map.read();
        assert!(r.range((Bound::Excluded(6), Bound::Included(7))).count() == 0);
        assert!(r.range((Bound::Excluded(6), Bound::Excluded(8))).count() == 0);
    }

    /*
    #[test]
    fn test_bptree2_map_write_compact() {
        let mut rng = rand::thread_rng();
        let insa: Vec<usize> = (0..(L_CAPACITY << 4)).collect();

        let map = BptreeMap::from_iter(insa.into_iter().map(|v| (v, v)));

        let mut w = map.write();
        // Created linearly, should not need compact
        assert!(w.compact() == false);
        assert!(w.verify());
        assert!(w.tree_density() == ((L_CAPACITY << 4), (L_CAPACITY << 4)));

        // Even in reverse, we shouldn't need it ...
        let insb: Vec<usize> = (0..(L_CAPACITY << 4)).collect();
        let bmap = BptreeMap::from_iter(insb.into_iter().rev().map(|v| (v, v)));
        let mut bw = bmap.write();
        assert!(bw.compact() == false);
        assert!(bw.verify());
        // Assert the density is "best"
        assert!(bw.tree_density() == ((L_CAPACITY << 4), (L_CAPACITY << 4)));

        // Random however, may.
        let mut insc: Vec<usize> = (0..(L_CAPACITY << 4)).collect();
        insc.shuffle(&mut rng);
        let cmap = BptreeMap::from_iter(insc.into_iter().map(|v| (v, v)));
        let mut cw = cmap.write();
        let (_n, d1) = cw.tree_density();
        cw.compact_force();
        assert!(cw.verify());
        let (_n, d2) = cw.tree_density();
        assert!(d2 <= d1);
    }
    */

    /*
    use std::sync::atomic::{AtomicUsize, Ordering};
    use crossbeam_utils::thread::scope;
    use rand::Rng;
    const MAX_TARGET: usize = 210_000;

    #[test]
    fn test_bptree2_map_thread_stress() {
        let start = time::now();
        let reader_completions = AtomicUsize::new(0);
        // Setup a tree with some initial data.
        let map: BptreeMap<usize, usize> = BptreeMap::from_iter(
            (0..10_000).map(|v| (v, v))
        );
        // now setup the threads.
        scope(|scope| {
            let mref = &map;
            let rref = &reader_completions;

            let _readers: Vec<_> = (0..7)
                .map(|_| {
                    scope.spawn(move || {
                        println!("Started reader ...");
                        let mut rng = rand::thread_rng();
                        let mut proceed = true;
                        while proceed {
                            let m_read = mref.read();
                            proceed = ! m_read.contains_key(&MAX_TARGET);
                            // Get a random number.
                            // Add 10_000 * random
                            // Remove 10_000 * random
                            let v1 = rng.gen_range(1, 18) * 10_000;
                            let r1 = v1 + 10_000;
                            for i in v1..r1 {
                                m_read.get(&i);
                            }
                            assert!(m_read.verify());
                            rref.fetch_add(1, Ordering::Relaxed);
                        }
                        println!("Closing reader ...");
                    })
                })
                .collect();

            let _writers: Vec<_> = (0..3)
                .map(|_| {
                    scope.spawn(move || {
                        println!("Started writer ...");
                        let mut rng = rand::thread_rng();
                        let mut proceed = true;
                        while proceed {
                            let mut m_write = mref.write();
                            proceed = ! m_write.contains_key(&MAX_TARGET);
                            // Get a random number.
                            // Add 10_000 * random
                            // Remove 10_000 * random
                            let v1 = rng.gen_range(1, 18) * 10_000;
                            let r1 = v1 + 10_000;
                            let v2 = rng.gen_range(1, 19) * 10_000;
                            let r2 = v2 + 10_000;

                            for i in v1..r1 {
                                m_write.insert(i, i);
                            }
                            for i in v2..r2 {
                                m_write.remove(&i);
                            }
                            m_write.commit();
                        }
                        println!("Closing writer ...");
                    })
                })
                .collect();

            let _complete = scope.spawn(move || {
                let mut last_value = 200_000;
                while last_value < MAX_TARGET {
                    let mut m_write = mref.write();
                    last_value += 1;
                    if last_value % 1000 == 0 {
                        println!("{:?}", last_value);
                    }
                    m_write.insert(last_value, last_value);
                    assert!(m_write.verify());
                    m_write.commit();
                }
            });

        });
        let end = time::now();
        print!("BptreeMap MT create :{} reader completions :{}", end - start, reader_completions.load(Ordering::Relaxed));
        // Done!
    }

    #[test]
    fn test_std_mutex_btreemap_thread_stress() {
        use std::collections::BTreeMap;
        use std::sync::Mutex;

        let start = time::now();
        let reader_completions = AtomicUsize::new(0);
        // Setup a tree with some initial data.
        let map: Mutex<BTreeMap<usize, usize>> = Mutex::new(BTreeMap::from_iter(
            (0..10_000).map(|v| (v, v))
        ));
        // now setup the threads.
        scope(|scope| {
            let mref = &map;
            let rref = &reader_completions;

            let _readers: Vec<_> = (0..7)
                .map(|_| {
                    scope.spawn(move || {
                        println!("Started reader ...");
                        let mut rng = rand::thread_rng();
                        let mut proceed = true;
                        while proceed {
                            let m_read = mref.lock().unwrap();
                            proceed = ! m_read.contains_key(&MAX_TARGET);
                            // Get a random number.
                            // Add 10_000 * random
                            // Remove 10_000 * random
                            let v1 = rng.gen_range(1, 18) * 10_000;
                            let r1 = v1 + 10_000;
                            for i in v1..r1 {
                                m_read.get(&i);
                            }
                            rref.fetch_add(1, Ordering::Relaxed);
                        }
                        println!("Closing reader ...");
                    })
                })
                .collect();

            let _writers: Vec<_> = (0..3)
                .map(|_| {
                    scope.spawn(move || {
                        println!("Started writer ...");
                        let mut rng = rand::thread_rng();
                        let mut proceed = true;
                        while proceed {
                            let mut m_write = mref.lock().unwrap();
                            proceed = ! m_write.contains_key(&MAX_TARGET);
                            // Get a random number.
                            // Add 10_000 * random
                            // Remove 10_000 * random
                            let v1 = rng.gen_range(1, 18) * 10_000;
                            let r1 = v1 + 10_000;
                            let v2 = rng.gen_range(1, 19) * 10_000;
                            let r2 = v2 + 10_000;

                            for i in v1..r1 {
                                m_write.insert(i, i);
                            }
                            for i in v2..r2 {
                                m_write.remove(&i);
                            }
                        }
                        println!("Closing writer ...");
                    })
                })
                .collect();

            let _complete = scope.spawn(move || {
                let mut last_value = 200_000;
                while last_value < MAX_TARGET {
                    let mut m_write = mref.lock().unwrap();
                    last_value += 1;
                    if last_value % 1000 == 0 {
                        println!("{:?}", last_value);
                    }
                    m_write.insert(last_value, last_value);
                }
            });

        });
        let end = time::now();
        print!("Mutex<BTreeMap> MT create :{} reader completions :{}", end - start, reader_completions.load(Ordering::Relaxed));
        // Done!
    }

    #[test]
    fn test_std_rwlock_btreemap_thread_stress() {
        use std::collections::BTreeMap;
        use std::sync::RwLock;

        let start = time::now();
        let reader_completions = AtomicUsize::new(0);
        // Setup a tree with some initial data.
        let map: RwLock<BTreeMap<usize, usize>> = RwLock::new(BTreeMap::from_iter(
            (0..10_000).map(|v| (v, v))
        ));
        // now setup the threads.
        scope(|scope| {
            let mref = &map;
            let rref = &reader_completions;

            let _readers: Vec<_> = (0..7)
                .map(|_| {
                    scope.spawn(move || {
                        println!("Started reader ...");
                        let mut rng = rand::thread_rng();
                        let mut proceed = true;
                        while proceed {
                            let m_read = mref.read().unwrap();
                            proceed = ! m_read.contains_key(&MAX_TARGET);
                            // Get a random number.
                            // Add 10_000 * random
                            // Remove 10_000 * random
                            let v1 = rng.gen_range(1, 18) * 10_000;
                            let r1 = v1 + 10_000;
                            for i in v1..r1 {
                                m_read.get(&i);
                            }
                            rref.fetch_add(1, Ordering::Relaxed);
                        }
                        println!("Closing reader ...");
                    })
                })
                .collect();

            let _writers: Vec<_> = (0..3)
                .map(|_| {
                    scope.spawn(move || {
                        println!("Started writer ...");
                        let mut rng = rand::thread_rng();
                        let mut proceed = true;
                        while proceed {
                            let mut m_write = mref.write().unwrap();
                            proceed = ! m_write.contains_key(&MAX_TARGET);
                            // Get a random number.
                            // Add 10_000 * random
                            // Remove 10_000 * random
                            let v1 = rng.gen_range(1, 18) * 10_000;
                            let r1 = v1 + 10_000;
                            let v2 = rng.gen_range(1, 19) * 10_000;
                            let r2 = v2 + 10_000;

                            for i in v1..r1 {
                                m_write.insert(i, i);
                            }
                            for i in v2..r2 {
                                m_write.remove(&i);
                            }
                        }
                        println!("Closing writer ...");
                    })
                })
                .collect();

            let _complete = scope.spawn(move || {
                let mut last_value = 200_000;
                while last_value < MAX_TARGET {
                    let mut m_write = mref.write().unwrap();
                    last_value += 1;
                    if last_value % 1000 == 0 {
                        println!("{:?}", last_value);
                    }
                    m_write.insert(last_value, last_value);
                }
            });

        });
        let end = time::now();
        print!("RwLock<BTreeMap> MT create :{} reader completions :{}", end - start, reader_completions.load(Ordering::Relaxed));
        // Done!
    }
    */

    #[cfg(feature = "serde")]
    #[test]
    fn test_bptreee2_serialize_deserialize() {
        let map: BptreeMap<usize, usize> = vec![(10, 11), (15, 16), (20, 21)].into_iter().collect();

        let value = serde_json::to_value(&map).unwrap();
        assert_eq!(value, serde_json::json!({ "10": 11, "15": 16, "20": 21 }));

        let map: BptreeMap<usize, usize> = serde_json::from_value(value).unwrap();
        let mut vec: Vec<(usize, usize)> = map.read().iter().map(|(k, v)| (*k, *v)).collect();
        vec.sort_unstable();
        assert_eq!(vec, [(10, 11), (15, 16), (20, 21)]);
    }
}
