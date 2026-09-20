//! Closed insertion admission for the existing synchronous map engine.

use super::*;
use crate::internals::bptree::node::{Branch, Leaf, Node, TXID_MASK, TXID_SHF};
use crate::internals::lincowcell::{WriterAdmission, WriterCharges, WriterLayouts};
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
/// not clone, serialize or allocate while planning. A key's bound must also cover
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
    /// Planning failed before calling the admission provider.
    Planning(PlanningError),
    /// The original provider refused the complete checked demand.
    Refused(E),
}

struct InsertPlan {
    demand: AllocationDemand,
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
    source.size.checked_add(1).ok_or(PlanningError::Overflow)?;
    source
        .txid
        .checked_add(1)
        .filter(|txid| *txid < (TXID_MASK >> TXID_SHF))
        .ok_or(PlanningError::Overflow)?;
    let mut demand = AllocationDemand::new();
    let mut separator = key_demand::<K, V, P>(key)?;
    let mut branches = 0usize;
    let mut node = source.root;
    // All references belong to the exact original SuperBlock under its writer
    // lock. Planning neither constructs a reader nor mutates any node.
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
    for layout in [first_layout, last_layout, shells.cursor, shells.reader] {
        demand.add_layout(layout)?;
    }
    Ok(InsertPlan {
        demand,
        first,
        last,
        first_layout,
        last_layout,
    })
}

impl<K, V, P> BptreeMap<K, V, Prepaid<P>>
where
    K: Clone + Ord + Debug + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    P: ClonePlanning<K, V>,
{
    /// Construct an empty map with prepaid node and initial reader ownership.
    ///
    /// The callback runs before either charged allocation. It supplies the
    /// original provider for their exact layouts. The permanent linear-cell
    /// root Arc and native mutex storage remain outside this demand.
    /// TODO: bind those initial control allocations before claiming complete
    /// map construction admission. This constructor does not claim that bound.
    pub fn try_new_with_node_custody<E>(
        admit: impl FnOnce(AllocationDemand) -> Result<P, E>,
    ) -> Result<Self, E> {
        let reader = MapCell::<K, V, Prepaid<P>>::reader_allocation_layout();
        let mut demand = AllocationDemand::new();
        demand
            .add_layout(Layout::new::<CachePadded<Leaf<K, V, P::Charge>>>())
            .expect("two concrete initial layouts fit usize");
        demand
            .add_layout(reader)
            .expect("two concrete initial layouts fit usize");
        let mut provider = Prepaid(Some(admit(demand)?));
        // The initial node takes its own original charge immediately before
        // allocation. The still-owned SuperBlock reclaims it if setup unwinds.
        let source = unsafe { SuperBlock::new_with_funding(&mut provider) };
        let reader_charge = provider.take_node_charge(reader);
        Ok(Self {
            inner: LinCowCell::new_charged(source, reader_charge),
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
        let previous = writer.as_mut().try_insert(key, value).unwrap_or_else(|_| {
            unreachable!("complete fixed tracking bound planned under original writer")
        });
        // No public prepaid successor can mutate again. Release only the
        // unused admission remainder now, before potentially long handoff waits;
        // actual allocation charges remain attached to their original owners.
        writer.as_mut().finish_admitted_funding();
        Ok((
            BptreeMapOwned {
                inner: writer.detach(),
            },
            previous,
        ))
    }
}

#[cfg(all(test, not(feature = "dhat-heap"), not(miri)))]
#[path = "admission_tests.rs"]
mod tests;
