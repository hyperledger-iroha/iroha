//! Original-budget ownership for the existing charged Concread map engine.
//!
//! These owners bind node/writer allocations to one actual immutable pool. They
//! expose no raw mutable map import or escape. Complete Storage and construction
//! admission remains unavailable until native control/collector costs are funded.

use std::{alloc::Layout, borrow::Borrow, marker::PhantomData};

use concread::bptree::{
    AllocationDemand, BptreeMap, BptreeMapCheckpoint, BptreeMapOwned, BptreeMapWriteTxn,
    ClonePlanning, InsertAdmissionError, NodeCloning, NodeFunding, OwnedWriteError, PlanningError,
    Prepaid,
};

use super::{AllocationBudget, AllocationCharge, AllocationRefusal, AllocationReservation};
use crate::{Key, Value};

/// Stable nested-copy policy for values stored under an original allocation pool.
///
/// Planning must allocate nothing and remain valid for the entire source borrow.
/// Shared/interior-mutable payloads without a stable bound must refuse. Copying
/// may only split the supplied original reservation before allocation. The copy
/// retains every nested allocation's actual charge until physical reclamation;
/// an unwind must reclaim partial storage before returning its charge. A copied
/// key preserves ordering and the same bound for subsequent separator copies.
/// This is a trusted allocation policy contract, not executable-state authority.
pub trait CopyPolicy<K: Key, V: Value> {
    /// Add the actual nested layouts for one key copy and copies of that key.
    fn plan_key(key: &K, demand: &mut AllocationDemand) -> Result<(), PlanningError>;
    /// Add the actual nested layouts for one value copy.
    fn plan_value(value: &V, demand: &mut AllocationDemand) -> Result<(), PlanningError>;
    /// Copy a key from the supplied original prepaid owner.
    fn copy_key(key: &K, reservation: &mut AllocationReservation) -> K;
    /// Copy a value from the supplied original prepaid owner.
    fn copy_value(value: &V, reservation: &mut AllocationReservation) -> V;
}

// There is no external provider constructor or alternate budget argument. Node
// charges always come from the actual reservation retained in this owner.
pub(super) struct Provider<K, V, P> {
    reservation: AllocationReservation,
    marker: PhantomData<fn(K, V) -> P>,
}

impl<K, V, P> Provider<K, V, P> {
    fn new(reservation: AllocationReservation) -> Self {
        Self {
            reservation,
            marker: PhantomData,
        }
    }
}

impl<K, V, P> NodeFunding for Provider<K, V, P> {
    type Charge = AllocationCharge;
    fn take_node_charge(&mut self, layout: Layout) -> AllocationCharge {
        self.reservation
            .try_split(layout)
            .expect("complete original map demand")
    }
}
impl<K: Key, V: Value, P: CopyPolicy<K, V>> NodeCloning<K, V> for Provider<K, V, P> {
    fn clone_key(&mut self, key: &K) -> K {
        P::copy_key(key, &mut self.reservation)
    }
    fn clone_value(&mut self, value: &V) -> V {
        P::copy_value(value, &mut self.reservation)
    }
}
impl<K: Key, V: Value, P: CopyPolicy<K, V>> ClonePlanning<K, V> for Provider<K, V, P> {
    fn plan_key(key: &K, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        P::plan_key(key, demand)
    }
    fn plan_value(value: &V, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        P::plan_value(value, demand)
    }
}

type Mode<K, V, P> = Prepaid<Provider<K, V, P>>;

/// A physical map retaining the original pool selected before construction.
///
/// No existing unbound map can be imported, and the raw mutable map cannot
/// escape. This precursor exposes ownership/read operations only; funded
/// insertion and complete Storage integration require their aggregate owner.
/// It does not qualify native lock/collector or budget-control allocations.
pub struct BudgetMap<K: Key, V: Value, P: CopyPolicy<K, V>> {
    inner: BptreeMap<K, V, Mode<K, V, P>>,
    budget: AllocationBudget,
}

impl<K: Key, V: Value, P: CopyPolicy<K, V>> BudgetMap<K, V, P> {
    /// Admit initial node/root/reader layouts from the actual selected pool.
    ///
    /// Call acquisition and cleanup inside this pool's synchronous refund scope.
    /// TODO: fund native lock/collector/control storage before activating these
    /// owners as complete production Storage construction admission.
    pub fn try_new_with_node_custody(budget: &AllocationBudget) -> Result<Self, AllocationRefusal> {
        let inner = BptreeMap::try_new_with_node_custody(|demand| {
            budget.try_reserve_bytes(demand.bytes()).map(Provider::new)
        })?;
        Ok(Self {
            inner,
            budget: budget.clone(),
        })
    }

    /// Acquire the original writer using only this map's retained pool.
    ///
    /// No replacement budget/provider is accepted. Keep the complete physical
    /// writer lifetime inside the original pool's refund-notification scope.
    pub fn try_write(
        &self,
    ) -> Result<BudgetWriter<'_, K, V, P>, InsertAdmissionError<AllocationRefusal>> {
        let inner = self.inner.try_write_admitted(|demand| {
            self.budget
                .try_reserve_bytes(demand.bytes())
                .map(Provider::new)
        })?;
        Ok(BudgetWriter {
            inner,
            budget: &self.budget,
        })
    }

    /// Reacquire the same physical successor after checking its original pool.
    ///
    /// A foreign pool refuses before map acquisition. The existing engine then
    /// checks actual map identity/base generation; equal pools do not make
    /// different maps interchangeable. Refusal returns the unchanged owner.
    pub fn try_write_owned(
        &self,
        owned: BudgetOwned<K, V, P>,
    ) -> Result<BudgetWriter<'_, K, V, P>, (BudgetOwned<K, V, P>, OwnerRefusal)> {
        if !self.budget.same_pool(&owned.budget) {
            return Err((owned, OwnerRefusal::ForeignPool));
        }
        match self.inner.try_write_owned(owned.inner) {
            Ok(inner) => Ok(BudgetWriter {
                inner,
                budget: &self.budget,
            }),
            Err((inner, error)) => Err((
                BudgetOwned {
                    inner,
                    budget: owned.budget,
                },
                OwnerRefusal::Map(error),
            )),
        }
    }
}

/// Local owner mismatch or physical acquisition refusal; not transaction validity.
#[derive(Debug)]
pub enum OwnerRefusal {
    /// The retained successor originated from a different actual allocation pool.
    ForeignPool,
    /// The existing map engine refused a foreign/stale/busy/poisoned successor.
    Map(OwnedWriteError),
}

/// The same original physical writer and its immutable pool binding.
pub struct BudgetWriter<'map, K: Key, V: Value, P: CopyPolicy<K, V>> {
    inner: BptreeMapWriteTxn<'map, K, V, Mode<K, V, P>>,
    budget: &'map AllocationBudget,
}
impl<K: Key, V: Value, P: CopyPolicy<K, V>> BudgetWriter<'_, K, V, P> {
    /// Retain the original generation under a nested rollback checkpoint.
    pub fn checkpoint(&mut self) -> Result<BudgetCheckpoint<'_, K, V, P>, PlanningError> {
        Ok(BudgetCheckpoint {
            inner: self.inner.checkpoint()?,
            budget: self.budget,
        })
    }
    /// Borrow a value without copying its payload or exposing a mutable map.
    pub fn get<Q: Ord + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.inner.get(key)
    }
    /// Detach the exact original physical cursor with its original pool.
    pub fn detach(self) -> BudgetOwned<K, V, P> {
        BudgetOwned {
            inner: self.inner.detach(),
            budget: self.budget.clone(),
        }
    }
    /// Publish this operable original writer; never reconstruct another cursor.
    pub fn commit(self) {
        self.inner.commit();
    }
}

/// A private original generation retaining its actual map and allocation pool.
pub struct BudgetCheckpoint<'writer, K: Key, V: Value, P: CopyPolicy<K, V>> {
    inner: BptreeMapCheckpoint<'writer, K, V, Mode<K, V, P>>,
    budget: &'writer AllocationBudget,
}
impl<K: Key, V: Value, P: CopyPolicy<K, V>> BudgetCheckpoint<'_, K, V, P> {
    /// Begin another original rollback generation without allocating.
    pub fn checkpoint(&mut self) -> Result<BudgetCheckpoint<'_, K, V, P>, PlanningError> {
        Ok(BudgetCheckpoint {
            inner: self.inner.checkpoint()?,
            budget: self.budget,
        })
    }
    /// Borrow the current value under this original checkpoint.
    pub fn get<Q: Ord + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.inner.get(key)
    }
    /// Borrow the preimage retained at this checkpoint's original start.
    pub fn get_before<Q: Ord + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.inner.get_before(key)
    }
    /// Keep changes private in the same original parent writer.
    pub fn apply(self) {
        self.inner.apply();
    }
}

/// A detached original successor and the actual pool that admitted it.
///
/// No raw successor can be imported, rebound to another pool or extracted. The
/// existing physical map identity is checked again when its writer is acquired.
pub struct BudgetOwned<K: Key, V: Value, P: CopyPolicy<K, V>> {
    inner: BptreeMapOwned<K, V, Mode<K, V, P>>,
    budget: AllocationBudget,
}
impl<K: Key, V: Value, P: CopyPolicy<K, V>> BudgetOwned<K, V, P> {
    /// Borrow an original retained value without cloning the physical tree.
    pub fn get<Q: Ord + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.inner.get(key)
    }
}

#[cfg(test)]
mod tests;
