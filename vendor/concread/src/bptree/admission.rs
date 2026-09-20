//! Closed insertion admission for the existing synchronous map engine.

use super::*;
use crate::internals::bptree::node::{Branch, Leaf, Node, TXID_MASK, TXID_SHF};
use crate::internals::lincowcell::{InitialCharges, WriterAdmission, WriterCharges, WriterLayouts};
use crossbeam_utils::CachePadded;
use std::alloc::Layout;

/// Checked sum of actual requested allocation layouts, not encoded sizes or RSS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AllocationDemand {
    bytes: usize,
    allocations: usize,
}
impl AllocationDemand {
    /// Empty demand, with no planning allocation.
    pub fn new() -> Self {
        Self::default()
    }
    /// Requested bytes across the planned concrete allocations.
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    /// Upper bound on nonzero allocation calls represented by this demand.
    pub fn allocations(&self) -> usize {
        self.allocations
    }
    /// Add one actual concrete layout with checked arithmetic.
    pub fn add_layout(&mut self, layout: Layout) -> Result<(), PlanningError> {
        self.add(
            Self {
                bytes: layout.size(),
                allocations: usize::from(layout.size() != 0),
            },
            1,
        )
    }
    fn add(&mut self, other: Self, copies: usize) -> Result<(), PlanningError> {
        let bytes = other
            .bytes
            .checked_mul(copies)
            .and_then(|n| self.bytes.checked_add(n))
            .ok_or(PlanningError::Overflow)?;
        let allocations = other
            .allocations
            .checked_mul(copies)
            .and_then(|n| self.allocations.checked_add(n))
            .ok_or(PlanningError::Overflow)?;
        *self = Self { bytes, allocations };
        Ok(())
    }
    fn include_max(&mut self, other: Self) {
        self.bytes = self.bytes.max(other.bytes);
        self.allocations = self.allocations.max(other.allocations);
    }
}

/// Allocation-free description of the nested storage a payload copy may own.
///
/// Implementations enumerate actual layouts into the checked demand. They must
/// not clone, serialize or allocate while planning; key ordering and diagnostic
/// formatting must likewise allocate no unadmitted storage. A key's bound must also cover
/// a copy of that key made by this same policy: insertion can use a freshly
/// cloned leaf key as a separator. Returned copies must preserve key ordering.
/// Values are copied at most once during the closed insertion. Existing incoming
/// payloads already own their storage; the plan funds new copies only.
pub trait ClonePlanning<K, V>: NodeCloning<K, V> {
    /// Add all nested layouts owned by a key copy and copies of that key.
    fn plan_key(key: &K, demand: &mut AllocationDemand) -> Result<(), PlanningError>;
    /// Add all nested layouts owned by a value copy.
    fn plan_value(value: &V, demand: &mut AllocationDemand) -> Result<(), PlanningError>;
}

/// A complete insertion demand cannot be established before allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanningError {
    /// A count, layout, byte sum or generation exceeds its representable limit.
    Overflow,
    /// The payload policy cannot provide a concrete allocation bound.
    UnsupportedPayload,
}

/// Local refusal before constructing any insertion allocation.
#[derive(Debug)]
pub enum InsertAdmissionError<E> {
    /// Another writer currently owns the original map lock.
    Busy,
    /// An earlier unwind poisoned the original writer lock.
    Poisoned,
    /// A retained successor belongs to another map or an obsolete generation.
    Changed,
    /// Planning failed before calling the admission provider.
    Planning(PlanningError),
    /// The original provider refused the complete checked demand.
    Refused(E),
}

struct InsertPlan {
    demand: AllocationDemand,
    tree_demand: AllocationDemand,
    first: usize,
    last: usize,
    first_layout: Layout,
    last_layout: Layout,
}

fn key_demand<K, V, P: ClonePlanning<K, V>>(key: &K) -> Result<AllocationDemand, PlanningError> {
    let mut demand = AllocationDemand::new();
    P::plan_key(key, &mut demand)?;
    Ok(demand)
}

fn plan_insert<K, V, P>(
    source: &SuperBlock<K, V, Prepaid<P>>,
    key: &K,
    shells: WriterLayouts,
) -> Result<InsertPlan, PlanningError>
where
    K: Clone + Ord + Debug,
    V: Clone,
    P: ClonePlanning<K, V>,
{
    source
        .txid
        .checked_add(1)
        .filter(|txid| *txid < (TXID_MASK >> TXID_SHF))
        .ok_or(PlanningError::Overflow)?;
    // SAFETY: the source is retained under its original writer lock.
    let mut plan = unsafe { plan_tree_insert::<K, V, P>(source.root, source.size, key) }?;
    for layout in [shells.cursor, shells.reader] {
        plan.demand.add_layout(layout)?;
    }
    Ok(plan)
}

// SAFETY: root and every reachable node must remain alive and immutable for this
// call, under the original writer lock. No tree pointer escapes the plan.
unsafe fn plan_tree_insert<K, V, P>(
    root: *mut Node<K, V, P::Charge>,
    length: usize,
    key: &K,
) -> Result<InsertPlan, PlanningError>
where
    K: Clone + Ord + Debug,
    V: Clone,
    P: ClonePlanning<K, V>,
{
    length.checked_add(1).ok_or(PlanningError::Overflow)?;
    let mut demand = AllocationDemand::new();
    let mut separator = key_demand::<K, V, P>(key)?;
    let mut branches = 0usize;
    let mut node = root;
    // All references belong to the original published or retained private tree
    // under its writer lock. Planning neither creates a reader nor mutates nodes.
    while !unsafe { &*node }.is_leaf() {
        branches = branches.checked_add(1).ok_or(PlanningError::Overflow)?;
        if branches >= usize::BITS as usize {
            return Err(PlanningError::Overflow);
        }
        let branch = unsafe { &*node.cast::<Branch<K, V, P::Charge>>() };
        for index in 0..branch.count() {
            let cloned = key_demand::<K, V, P>(branch.key_at(index))?;
            demand.add(cloned, 1)?;
            separator.include_max(cloned);
        }
        // A split may detach either of the final children, even when insertion
        // descends elsewhere. Include every original child minimum as a possible
        // new separator; fresh-child minima come from this path or incoming key.
        for index in 0..=branch.count() {
            let minimum = unsafe { &*Node::min_raw(branch.get_idx_unchecked(index)) };
            separator.include_max(key_demand::<K, V, P>(minimum)?);
        }
        node = branch.get_idx_unchecked(branch.locate_node(key));
    }
    let leaf = unsafe { &*node.cast::<Leaf<K, V, P::Charge>>() };
    for index in 0..leaf.count() {
        let (key, value) = leaf
            .get_kv_idx_checked(index)
            .expect("initialized leaf prefix");
        let cloned = key_demand::<K, V, P>(key)?;
        demand.add(cloned, 1)?;
        separator.include_max(cloned);
        P::plan_value(value, &mut demand)?;
    }
    // One leaf clone and singleton split; one clone and sibling per branch;
    // one possible new root. Each branch adds at most one separator and one
    // sibling separator, with one final root separator. These are counts of
    // real engine allocations/copies, not a guessed encoded-size multiplier.
    let branch_nodes = branches
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .ok_or(PlanningError::Overflow)?;
    let first = branch_nodes.checked_add(2).ok_or(PlanningError::Overflow)?;
    let last = branches.checked_add(1).ok_or(PlanningError::Overflow)?;
    let mut leaf_layout = AllocationDemand::new();
    leaf_layout.add_layout(Layout::new::<CachePadded<Leaf<K, V, P::Charge>>>())?;
    demand.add(leaf_layout, 2)?;
    let mut branch_layout = AllocationDemand::new();
    branch_layout.add_layout(Layout::new::<CachePadded<Branch<K, V, P::Charge>>>())?;
    demand.add(branch_layout, branch_nodes)?;
    demand.add(separator, branch_nodes)?;
    type Buffer<K, V, C> = FixedTrackingBuffer<*mut Node<K, V, C>, C>;
    let first_layout =
        Buffer::<K, V, P::Charge>::allocation_layout(first).map_err(|_| PlanningError::Overflow)?;
    let last_layout =
        Buffer::<K, V, P::Charge>::allocation_layout(last).map_err(|_| PlanningError::Overflow)?;
    let tree_demand = demand;
    for layout in [first_layout, last_layout] {
        demand.add_layout(layout)?;
    }
    Ok(InsertPlan {
        demand,
        tree_demand,
        first,
        last,
        first_layout,
        last_layout,
    })
}

struct TrackingGrowth {
    capacity: usize,
    layout: Layout,
}

fn plan_tracking_growth<K, V, P>(
    initialized: usize,
    capacity: usize,
    required: usize,
    demand: &mut AllocationDemand,
) -> Result<Option<TrackingGrowth>, PlanningError>
where
    K: Clone + Ord + Debug,
    V: Clone,
    P: ClonePlanning<K, V>,
{
    let needed = initialized
        .checked_add(required)
        .ok_or(PlanningError::Overflow)?;
    if needed <= capacity {
        return Ok(None);
    }
    // Geometric growth bounds cumulative pointer copies over a multi-edit
    // successor. Both the retained old buffer and its replacement remain paid
    // during construction; no capacity is borrowed from a future refund.
    type Buffer<K, V, C> = FixedTrackingBuffer<*mut Node<K, V, C>, C>;
    let enlarged = capacity.checked_mul(2).map(|n| n.max(needed));
    let (capacity, layout) = enlarged
        .and_then(|n| {
            Buffer::<K, V, P::Charge>::allocation_layout(n)
                .ok()
                .map(|l| (n, l))
        })
        .or_else(|| {
            Buffer::<K, V, P::Charge>::allocation_layout(needed)
                .ok()
                .map(|l| (needed, l))
        })
        .ok_or(PlanningError::Overflow)?;
    demand.add_layout(layout)?;
    Ok(Some(TrackingGrowth { capacity, layout }))
}

fn allocate_tracking<K, V, P>(
    growth: Option<TrackingGrowth>,
    provider: &mut P,
) -> Option<FixedTrackingBuffer<*mut Node<K, V, P::Charge>, P::Charge>>
where
    K: Clone + Ord + Debug,
    V: Clone,
    P: ClonePlanning<K, V>,
{
    growth.map(|growth| {
        let charge = provider.take_node_charge(growth.layout);
        FixedTrackingBuffer::try_new(growth.capacity, charge)
            .unwrap_or_else(|_| unreachable!("planned tracking growth layout"))
    })
}

impl<K, V, P> BptreeMap<K, V, Prepaid<P>>
where
    K: Clone + Ord + Debug + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    P: ClonePlanning<K, V>,
{
    /// Construct an empty map with prepaid node, root and initial reader owners.
    ///
    /// The callback runs before all three charged allocations and supplies the
    /// original provider for their exact layouts. Platform-native mutex storage
    /// remains outside this demand.
    /// TODO: bind native lock/runtime storage before claiming complete map
    /// construction admission. This constructor does not claim that bound.
    pub fn try_new_with_node_custody<E>(
        admit: impl FnOnce(AllocationDemand) -> Result<P, E>,
    ) -> Result<Self, E> {
        let initial = MapCell::<K, V, Prepaid<P>>::initial_allocation_layouts();
        let mut demand = AllocationDemand::new();
        demand
            .add_layout(Layout::new::<CachePadded<Leaf<K, V, P::Charge>>>())
            .expect("three concrete initial layouts fit usize");
        for layout in [initial.root, initial.reader] {
            demand
                .add_layout(layout)
                .expect("three concrete initial layouts fit usize");
        }
        let mut provider = Prepaid(Some(admit(demand)?));
        // The initial node takes its own original charge immediately before
        // allocation. The still-owned SuperBlock reclaims it if setup unwinds.
        let source = unsafe { SuperBlock::new_with_funding(&mut provider) };
        let charges = InitialCharges {
            root: provider.take_node_charge(initial.root),
            reader: provider.take_node_charge(initial.reader),
        };
        Ok(Self {
            inner: LinCowCell::new_charged(source, charges),
        })
    }

    /// Plan and execute one insertion, retaining its exact unpublished successor.
    ///
    /// Contention, poison, overflow or admission refusal returns the same owned
    /// input before any new node, payload, shell or bookkeeping allocation. The
    /// callback receives a complete checked upper bound while the original
    /// writer lock is held. It must reserve once; its returned provider splits
    /// that owner without fresh pool acquisition during execution.
    ///
    /// The caller must enclose this synchronous operation in the original
    /// budget's refund-notification deferral scope. Incoming payloads already
    /// own their storage. A clone panic aborts the private cursor and preserves
    /// the published generation; a partly mutated cursor is never returned for
    /// retry. The returned owner has no unrestricted mutation interface.
    pub fn try_insert_admitted<E>(
        &self,
        key: K,
        value: V,
        admit: impl FnOnce(AllocationDemand) -> Result<P, E>,
    ) -> Result<(BptreeMapOwned<K, V, Prepaid<P>>, Option<V>), ((K, V), InsertAdmissionError<E>)>
    {
        let mut input = Some((key, value));
        let acquired = self.inner.try_write_charged(|source, shells| {
            let plan =
                plan_insert::<K, V, P>(source, &input.as_ref().expect("original input").0, shells)
                    .map_err(InsertAdmissionError::Planning)?;
            let mut provider = Prepaid(Some(
                admit(plan.demand).map_err(InsertAdmissionError::Refused)?,
            ));
            let first_charge = provider.take_node_charge(plan.first_layout);
            let first = FixedTrackingBuffer::try_new(plan.first, first_charge)
                .unwrap_or_else(|_| unreachable!("planned first buffer layout"));
            let last_charge = provider.take_node_charge(plan.last_layout);
            let last = FixedTrackingBuffer::try_new(plan.last, last_charge)
                .unwrap_or_else(|_| unreachable!("planned retirement buffer layout"));
            let charges = WriterCharges {
                cursor: provider.take_node_charge(shells.cursor),
                reader: provider.take_node_charge(shells.reader),
            };
            Ok(WriterAdmission {
                charges,
                input: (provider, first, last),
            })
        });
        let mut writer = match acquired {
            Ok(Some(writer)) => writer,
            Ok(None) => {
                return Err((
                    input.take().expect("original refused input"),
                    if self.inner.is_poisoned() {
                        InsertAdmissionError::Poisoned
                    } else {
                        InsertAdmissionError::Busy
                    },
                ))
            }
            Err(error) => return Err((input.take().expect("original refused input"), error)),
        };
        let (key, value) = input.take().expect("original admitted input");
        writer.as_mut().begin_admitted_edit();
        let previous = writer.as_mut().try_insert(key, value).unwrap_or_else(|_| {
            unreachable!("complete fixed tracking bound planned under original writer")
        });
        // A further closed edit requires its own complete admission. Release the
        // unused remainder now, before potentially long handoff waits;
        // actual allocation charges remain attached to their original owners.
        writer.as_mut().finish_admitted_funding();
        Ok((
            BptreeMapOwned {
                inner: writer.detach(),
            },
            previous,
        ))
    }

    /// Admit another insertion into the same original unpublished successor.
    ///
    /// Root, base generation and writer availability are checked before planning
    /// from the retained private tree. Any refusal returns that exact owner and
    /// input without allocations or mutation. Successful edits retain the same
    /// cursor, publication shell and base; only exhausted bookkeeping buffers
    /// are replaced, with their complete storage prepaid alongside the edit.
    /// No intermediate generation is published.
    ///
    /// Enclose this synchronous operation in the original budget's refund
    /// notification deferral scope. A panic under the writer lock aborts the
    /// entire private successor and poisons the writer; it cannot return a
    /// partly changed successor for retry.
    pub fn try_insert_owned_admitted<E>(
        &self,
        owned: BptreeMapOwned<K, V, Prepaid<P>>,
        key: K,
        value: V,
        admit: impl FnOnce(AllocationDemand) -> Result<P, E>,
    ) -> Result<
        (BptreeMapOwned<K, V, Prepaid<P>>, Option<V>),
        (
            (BptreeMapOwned<K, V, Prepaid<P>>, (K, V)),
            InsertAdmissionError<E>,
        ),
    > {
        let mut writer = match self.inner.try_write_owned(owned.inner) {
            Ok(writer) => writer,
            Err((inner, error)) => {
                let error = match error {
                    OwnedWriteError::Busy => InsertAdmissionError::Busy,
                    OwnedWriteError::Poisoned => InsertAdmissionError::Poisoned,
                    OwnedWriteError::Changed => InsertAdmissionError::Changed,
                };
                return Err(((BptreeMapOwned { inner }, (key, value)), error));
            }
        };
        let previous = match edit_admitted(writer.as_mut(), key, value, admit, None) {
            Ok(previous) => previous,
            Err((input, error)) => {
                return Err((
                    (
                        BptreeMapOwned {
                            inner: writer.detach(),
                        },
                        input,
                    ),
                    error,
                ));
            }
        };
        Ok((
            BptreeMapOwned {
                inner: writer.detach(),
            },
            previous,
        ))
    }
}

fn edit_admitted<K, V, P, E>(
    cursor: &mut CursorWrite<K, V, Prepaid<P>>,
    key: K,
    value: V,
    admit: impl FnOnce(AllocationDemand) -> Result<P, E>,
    saved: Option<&mut crate::internals::bptree::cursor::CheckpointBuffers<K, V, P>>,
) -> Result<Option<V>, ((K, V), InsertAdmissionError<E>)>
where
    K: Clone + Ord + Debug,
    V: Clone,
    P: ClonePlanning<K, V>,
{
    cursor.assert_operable();
    let preparation = (|| {
        // SAFETY: this exclusive original cursor retains its base and root.
        let plan = unsafe { plan_tree_insert::<K, V, P>(cursor.get_root(), cursor.len(), &key) }
            .map_err(InsertAdmissionError::Planning)?;
        let mut demand = plan.tree_demand;
        let [(first_len, first_capacity), (last_len, last_capacity)] = cursor.admitted_tracking();
        let first =
            plan_tracking_growth::<K, V, P>(first_len, first_capacity, plan.first, &mut demand)
                .map_err(InsertAdmissionError::Planning)?;
        let last = plan_tracking_growth::<K, V, P>(last_len, last_capacity, plan.last, &mut demand)
            .map_err(InsertAdmissionError::Planning)?;
        let mut provider = admit(demand).map_err(InsertAdmissionError::Refused)?;
        let first = allocate_tracking::<K, V, P>(first, &mut provider);
        let last = allocate_tracking::<K, V, P>(last, &mut provider);
        Ok((provider, first, last))
    })();
    let (provider, first, last) = match preparation {
        Ok(prepared) => prepared,
        Err(error) => return Err(((key, value), error)),
    };
    cursor.begin_admitted_edit();
    cursor.resume_admitted_funding(provider, first, last, saved);
    let previous = cursor
        .try_insert(key, value)
        .unwrap_or_else(|_| unreachable!("complete tracking bound planned under original writer"));
    cursor.finish_admitted_funding();
    Ok(previous)
}

/// An exclusive transaction-start checkpoint of an original prepaid map writer.
///
/// Dropping this guard aborts its private edits without allocating or obtaining
/// new credit. Applying it keeps those edits private in the same writer. Nested
/// guards resolve in LIFO order through exclusive reborrows. No publication,
/// detachment or unrestricted mutable payload access exists through this guard.
///
/// The entire original writer and checkpoint lifetime must remain inside its
/// budget's synchronous refund-notification deferral scope. An edit or cleanup
/// panic makes the original cursor unusable even if caught before the physical
/// writer guard unwinds; abort that writer instead of publishing it.
pub struct BptreeMapCheckpoint<'a, K, V, P>
where
    K: Clone + Ord + Debug + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    P: ClonePlanning<K, V>,
{
    inner: crate::internals::bptree::cursor::CursorCheckpoint<'a, K, V, P>,
}

impl<K, V, P> BptreeMapWriteTxn<'_, K, V, Prepaid<P>>
where
    K: Clone + Ord + Debug + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    P: ClonePlanning<K, V>,
{
    /// Begin an allocation-free private checkpoint under this original writer.
    ///
    /// Checked generation exhaustion refuses before changing any owner. Keep
    /// the entire writer lifetime in its original budget's refund-deferral scope.
    pub fn checkpoint(&mut self) -> Result<BptreeMapCheckpoint<'_, K, V, P>, PlanningError> {
        self.inner
            .as_mut()
            .checkpoint()
            .map(|inner| BptreeMapCheckpoint { inner })
            .ok_or(PlanningError::Overflow)
    }

    /// Admit one closed insertion while retaining this original physical writer.
    ///
    /// Refusal returns the original input before mutation; successful edits
    /// release unused admission. A panic makes this cursor unpublishable. Keep
    /// this writer's lifetime inside the original refund-notification scope.
    pub fn try_insert_admitted<E>(
        &mut self,
        key: K,
        value: V,
        admit: impl FnOnce(AllocationDemand) -> Result<P, E>,
    ) -> Result<Option<V>, ((K, V), InsertAdmissionError<E>)> {
        edit_admitted(self.inner.as_mut(), key, value, admit, None)
    }
}

impl<K, V, P> BptreeMapCheckpoint<'_, K, V, P>
where
    K: Clone + Ord + Debug + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    P: ClonePlanning<K, V>,
{
    /// Borrow an immutable value from this private checkpoint's current state.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.inner.as_ref().search(key)
    }

    /// Borrow a snapshot whose lifetime cannot escape this exclusive guard.
    pub fn to_snapshot(&self) -> BptreeMapReadSnapshot<'_, K, V, Prepaid<P>> {
        BptreeMapReadSnapshot {
            inner: SnapshotType::W(self.inner.as_ref()),
        }
    }

    /// Begin a nested private checkpoint; overflow leaves the parent unchanged.
    pub fn checkpoint(&mut self) -> Result<BptreeMapCheckpoint<'_, K, V, P>, PlanningError> {
        self.inner
            .checkpoint()
            .map(|inner| BptreeMapCheckpoint { inner })
            .ok_or(PlanningError::Overflow)
    }

    /// Admit one closed edit while retaining exact parent rollback ownership.
    pub fn try_insert_admitted<E>(
        &mut self,
        key: K,
        value: V,
        admit: impl FnOnce(AllocationDemand) -> Result<P, E>,
    ) -> Result<Option<V>, ((K, V), InsertAdmissionError<E>)> {
        let (cursor, buffers) = self.inner.edit_parts();
        edit_admitted(cursor, key, value, admit, Some(buffers))
    }

    /// Keep all child edits private in the original writer; does not publish.
    pub fn apply(self) {
        self.inner.apply();
    }
}

#[cfg(all(test, not(feature = "dhat-heap"), not(miri)))]
#[path = "admission_tests.rs"]
mod tests;
