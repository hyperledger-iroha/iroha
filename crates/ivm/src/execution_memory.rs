//! Prepaid execution allocation plans and parent-funded backing owners.
//!
//! A parent admits a checked sum of real allocation layouts once. Child leases
//! partition its original credit without another pool admission or any wait.
//! Each backing allocation keeps its charge until its actual deallocation,
//! including when the allocation is retained by an idle cache.
//!
//! TODO: Route snapshots, nested checkout and host scratch through complete
//! plans before enforcing this pool in production. Existing infallible cloning
//! is not a funded allocation path.

use std::alloc::Layout;

use iroha_allocation::{
    AllocationBudget, AllocationCharge, AllocationRefusal, AllocationReservation, ChargedBuffer,
    InsufficientReservation, PrepaidBufferError,
};

/// Allocation-free checked demand for explicitly enumerated backing layouts.
/// Referenced payloads and scratch must be enumerated separately by their owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExecutionMemoryPlan {
    bytes: usize,
}

impl ExecutionMemoryPlan {
    /// Include one concrete backing layout; overflow leaves this plan unchanged.
    pub fn include(&mut self, layout: Layout) -> Result<(), AllocationRefusal> {
        self.bytes = self
            .bytes
            .checked_add(layout.size())
            .ok_or(AllocationRefusal::DemandOverflow)?;
        Ok(())
    }

    /// Include a complete child component plan without allocating a layout list.
    pub fn include_child(&mut self, child: Self) -> Result<(), AllocationRefusal> {
        self.bytes = self
            .bytes
            .checked_add(child.bytes)
            .ok_or(AllocationRefusal::DemandOverflow)?;
        Ok(())
    }

    /// Plan one fixed array's exact backing allocation.
    pub fn array<T>(capacity: usize) -> Result<Self, AllocationRefusal> {
        let layout = Layout::array::<T>(capacity).map_err(|_| AllocationRefusal::DemandOverflow)?;
        Ok(Self {
            bytes: layout.size(),
        })
    }

    /// Checked sum of requested backing bytes, not RSS or allocator overhead.
    pub const fn requested_bytes(self) -> usize {
        self.bytes
    }
}

/// Move-only prepaid credit, partitioned before starting a nested component.
#[derive(Debug)]
pub struct ExecutionMemoryLease {
    reservation: AllocationReservation,
}

impl ExecutionMemoryLease {
    /// Admit a complete root demand before constructing any covered allocation.
    pub fn reserve(
        budget: &AllocationBudget,
        plan: ExecutionMemoryPlan,
    ) -> Result<Self, AllocationRefusal> {
        budget
            .try_reserve_bytes(plan.bytes)
            .map(|reservation| Self { reservation })
    }

    /// Fund a child entirely from this parent's original reservation.
    /// Refusal leaves the parent's remaining credit unchanged and never waits.
    pub fn partition(
        &mut self,
        plan: ExecutionMemoryPlan,
    ) -> Result<Self, InsufficientReservation> {
        self.reservation
            .try_partition_bytes(plan.bytes)
            .map(|reservation| Self { reservation })
    }

    /// Attach one exact original layout to a component that owns its backing.
    /// The component must drop the backing before this charge, including on unwind.
    pub(crate) fn split_allocation(
        &mut self,
        layout: Layout,
    ) -> Result<AllocationCharge, InsufficientReservation> {
        self.reservation.try_split(layout)
    }

    /// Unspent original credit; this diagnostic never grants a new allocation.
    pub fn remaining_bytes(&self) -> usize {
        self.reservation.remaining_bytes()
    }

    /// Check original pool identity before an internal owner accepts parent credit.
    pub(crate) fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.reservation.belongs_to(budget)
    }
}

/// Fixed backing storage holding execution and retention charges together.
///
/// Referenced allocations inside elements need their own funding. This owner
/// cannot be cloned or separated from its backing allocation. Borrowers may
/// share an outer owner;
/// cache eviction then leaves both charges live until the final owner drops.
pub struct ExecutionBuffer<T> {
    storage: ChargedBuffer<T>,
    retention: crate::cache_memory::MemoryReservation,
}

impl<T> ExecutionBuffer<T> {
    /// Allocate an exact fixed backing from original parent credit.
    pub fn new(
        capacity: usize,
        lease: &mut ExecutionMemoryLease,
    ) -> Result<Self, PrepaidBufferError> {
        let layout = Layout::array::<T>(capacity).map_err(|_| {
            PrepaidBufferError::Allocation(iroha_allocation::ChargedBufferError::Admission(
                AllocationRefusal::DemandOverflow,
            ))
        })?;
        // Claim both owners before the physical allocation. On refusal, the
        // active accounting owner drops without retaining or publishing bytes.
        let retention = crate::cache_memory::MemoryReservation::active(layout.size());
        let storage = ChargedBuffer::from_reservation(capacity, &mut lease.reservation)?;
        Ok(Self { storage, retention })
    }

    /// Append within prepaid capacity; growing the backing is not supported.
    pub fn append(&mut self, values: &[T]) -> std::io::Result<()>
    where
        T: Copy,
    {
        self.storage.append(values)
    }

    /// Move one element into previously prepaid capacity without allocating.
    pub fn push_reserved(&mut self, value: T) {
        self.storage.push_reserved(value);
    }

    /// Remove the last initialized element while keeping the backing charge.
    pub fn pop(&mut self) -> Option<T> {
        self.storage.pop()
    }

    /// Move all initialized elements without releasing the backing charge.
    pub fn drain_all(&mut self) -> std::vec::Drain<'_, T> {
        self.storage.drain_all()
    }

    /// Shorten the initialized prefix while retaining the prepaid backing charge.
    pub fn truncate(&mut self, len: usize) {
        self.storage.truncate(len);
    }

    /// Borrow the initialized prefix while retaining both allocation charges.
    pub fn as_slice(&self) -> &[T] {
        self.storage.as_slice()
    }

    /// Mutate initialized elements without replacing the backing allocation.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.storage.as_mut_slice()
    }

    /// Number of elements admitted in this fixed backing.
    pub fn capacity(&self) -> usize {
        self.storage.capacity()
    }

    /// Admit this same backing allocation to cache retention without refunding
    /// its original execution credit. Refusal requires cold destruction.
    pub fn try_retain(&self) -> bool {
        self.retention.try_retain()
    }

    /// Return to active accounting while preserving the original execution charge.
    pub fn activate(&self) {
        self.retention.make_active();
    }

    pub(crate) fn mark_unmeasured(&mut self) {
        self.retention.mark_unmeasured();
    }

    /// Restore the exact measure of this fixed backing after active work.
    pub(crate) fn remeasure_fixed(&mut self) {
        self.retention.remeasure_fixed();
    }
}

impl ExecutionBuffer<u8> {
    /// Fill the fixed, prepaid backing without allocating a temporary image.
    pub(crate) fn zeroed(
        capacity: usize,
        lease: &mut ExecutionMemoryLease,
    ) -> Result<Self, PrepaidBufferError> {
        let mut buffer = Self::new(capacity, lease)?;
        let zeros = [0_u8; 4096];
        while buffer.storage.as_slice().len() < capacity {
            let remaining = capacity - buffer.storage.as_slice().len();
            buffer
                .storage
                .append(&zeros[..remaining.min(zeros.len())])
                .expect("fixed prepaid backing has sufficient capacity");
        }
        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct MoveOnly(u64);

    #[test]
    fn move_only_backing_uses_original_credit_until_final_borrower_drops() {
        let bytes = 2 * std::mem::size_of::<MoveOnly>();
        let budget = AllocationBudget::new(bytes);
        let mut lease = ExecutionMemoryLease::reserve(
            &budget,
            ExecutionMemoryPlan::array::<MoveOnly>(2).unwrap(),
        )
        .unwrap();
        let mut buffer = ExecutionBuffer::<MoveOnly>::new(2, &mut lease).unwrap();
        buffer.push_reserved(MoveOnly(7));
        buffer.push_reserved(MoveOnly(9));
        assert_eq!(buffer.capacity(), 2);
        assert_eq!(buffer.pop().unwrap().0, 9);
        assert_eq!(buffer.drain_all().next().unwrap().0, 7);
        assert!(buffer.as_slice().is_empty());
        let owner = Arc::new(buffer);
        let borrower = Arc::clone(&owner);
        drop(lease);
        drop(owner);
        assert_eq!(budget.reserved_bytes(), bytes);
        drop(borrower);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn plans_check_layout_overflow_without_losing_existing_demand() {
        let mut plan = ExecutionMemoryPlan::array::<u64>(4).unwrap();
        assert_eq!(plan.requested_bytes(), 32);
        let previous = plan;
        assert!(ExecutionMemoryPlan::array::<u64>(usize::MAX).is_err());
        plan.bytes = usize::MAX;
        assert_eq!(
            plan.include_child(previous),
            Err(AllocationRefusal::DemandOverflow)
        );
        assert_eq!(plan.requested_bytes(), usize::MAX);
    }

    #[test]
    fn nested_allocations_use_prepaid_parent_credit_without_reacquisition() {
        let budget = AllocationBudget::new(32);
        let plan = ExecutionMemoryPlan::array::<u64>(4).unwrap();
        let mut parent = ExecutionMemoryLease::reserve(&budget, plan).unwrap();
        assert!(matches!(
            budget.try_reserve_bytes(1),
            Err(AllocationRefusal::Capacity { .. })
        ));
        let mut child = parent
            .partition(ExecutionMemoryPlan::array::<u64>(3).unwrap())
            .unwrap();
        let mut buffer = ExecutionBuffer::new(3, &mut child).unwrap();
        buffer.append(&[3_u64, 5, 8]).unwrap();
        buffer.as_mut_slice()[1] = 13;
        assert_eq!(buffer.as_slice(), &[3, 13, 8]);
        assert!(buffer.append(&[21]).is_err());
        assert_eq!(parent.remaining_bytes(), 8);
        assert_eq!(child.remaining_bytes(), 0);
        assert_eq!(budget.reserved_bytes(), 32);
        drop(parent);
        drop(child);
        assert_eq!(budget.reserved_bytes(), 24);
        drop(buffer);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn truncating_a_buffer_keeps_its_prepaid_charge_until_drop() {
        let budget = AllocationBudget::new(16);
        let mut lease =
            ExecutionMemoryLease::reserve(&budget, ExecutionMemoryPlan::array::<u64>(2).unwrap())
                .unwrap();
        let mut buffer = ExecutionBuffer::<u64>::new(2, &mut lease).unwrap();
        buffer.append(&[3, 5]).unwrap();
        buffer.truncate(1);
        assert_eq!(buffer.as_slice(), &[3]);
        assert_eq!(budget.reserved_bytes(), 16);
        drop(lease);
        drop(buffer);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn eviction_and_unwind_leave_the_original_charge_with_shared_backing() {
        let budget = AllocationBudget::new(16);
        let mut lease =
            ExecutionMemoryLease::reserve(&budget, ExecutionMemoryPlan::array::<u64>(2).unwrap())
                .unwrap();
        let retained = Arc::new(ExecutionBuffer::<u64>::new(2, &mut lease).unwrap());
        drop(lease);
        let borrower = Arc::clone(&retained);
        drop(retained);
        assert_eq!(budget.reserved_bytes(), 16);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _borrower = borrower;
            panic!("abandon active execution");
        }));
        assert!(unwind.is_err());
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn child_refusal_preserves_parent_and_zero_never_means_unlimited() {
        let budget = AllocationBudget::new(8);
        let mut parent =
            ExecutionMemoryLease::reserve(&budget, ExecutionMemoryPlan::array::<u64>(1).unwrap())
                .unwrap();
        assert!(
            parent
                .partition(ExecutionMemoryPlan::array::<u64>(2).unwrap())
                .is_err()
        );
        assert!(ExecutionBuffer::<u64>::new(2, &mut parent).is_err());
        assert!(ExecutionBuffer::<u64>::new(usize::MAX, &mut parent).is_err());
        assert_eq!(parent.remaining_bytes(), 8);
        assert_eq!(budget.reserved_bytes(), 8);
        let zero = AllocationBudget::new(0);
        assert!(
            ExecutionMemoryLease::reserve(&zero, ExecutionMemoryPlan::array::<u8>(1).unwrap())
                .is_err()
        );
        let mut empty =
            ExecutionMemoryLease::reserve(&zero, ExecutionMemoryPlan::default()).unwrap();
        let buffer = ExecutionBuffer::<u8>::new(0, &mut empty).unwrap();
        assert!(buffer.as_slice().is_empty());
        assert_eq!(zero.reserved_bytes(), 0);
    }

    #[test]
    fn zeroed_fixed_backing_keeps_original_charge_until_owner_drop() {
        let budget = AllocationBudget::new(8193);
        let mut lease =
            ExecutionMemoryLease::reserve(&budget, ExecutionMemoryPlan::array::<u8>(8193).unwrap())
                .unwrap();
        let mut image = ExecutionBuffer::<u8>::zeroed(8193, &mut lease).unwrap();
        assert_eq!(image.as_slice(), &[0; 8193]);
        image.as_mut_slice()[8192] = 7;
        drop(lease);
        assert_eq!(budget.reserved_bytes(), 8193);
        assert_eq!(image.as_slice()[8192], 7);
        drop(image);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
