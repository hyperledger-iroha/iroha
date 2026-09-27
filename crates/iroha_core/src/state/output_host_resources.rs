//! Prepaid host storage for the native output producer's fixed vectors.
//!
//! The requested layouts are reserved together before the producer allocates
//! its first row. Charges move with the row and source vectors; later variable
//! payloads and execution scratch require their own admission from the same pool.

use super::{ExecutionOutputV1, OwnedExecutionSource};
use crate::queue::RoutingDecision;
use mv::allocation::{AllocationBudget, AllocationCharge, AllocationRefusal};
use std::alloc::Layout;

/// Exact requested layouts for the producer's four fixed vectors.
#[derive(Clone, Copy, Debug)]
pub(super) struct NativeOutputProducerDemand {
    row_slots: Layout,
    source_entries: Layout,
    source_routes: Layout,
    network_resolved: Layout,
    total_bytes: usize,
}

impl NativeOutputProducerDemand {
    /// Derive the host layout from the already frozen agreed output envelope.
    pub(super) fn plan(
        maximum_rows: usize,
        network_inputs: usize,
    ) -> Result<Self, AllocationRefusal> {
        let row_slots = Layout::array::<ExecutionOutputV1>(maximum_rows)
            .map_err(|_| AllocationRefusal::DemandOverflow)?;
        let source_entries = Layout::array::<OwnedExecutionSource>(maximum_rows)
            .map_err(|_| AllocationRefusal::DemandOverflow)?;
        let source_routes = Layout::array::<RoutingDecision>(network_inputs)
            .map_err(|_| AllocationRefusal::DemandOverflow)?;
        let network_resolved =
            Layout::array::<bool>(network_inputs).map_err(|_| AllocationRefusal::DemandOverflow)?;
        let total_bytes = [row_slots, source_entries, source_routes, network_resolved]
            .into_iter()
            .try_fold(0usize, |total, layout| total.checked_add(layout.size()))
            .ok_or(AllocationRefusal::DemandOverflow)?;
        Ok(Self {
            row_slots,
            source_entries,
            source_routes,
            network_resolved,
            total_bytes,
        })
    }

    /// Reserve all four requested vectors from the source's exact finite pool.
    pub(super) fn try_reserve(
        self,
        budget: &AllocationBudget,
    ) -> Result<NativeOutputProducerCharges, AllocationRefusal> {
        let mut reservation = budget.try_reserve_bytes(self.total_bytes)?;
        let row_slots = reservation
            .try_split(self.row_slots)
            .expect("aggregate native output reservation includes row slots");
        let source_entries = reservation
            .try_split(self.source_entries)
            .expect("aggregate native output reservation includes source entries");
        let source_routes = reservation
            .try_split(self.source_routes)
            .expect("aggregate native output reservation includes source routes");
        let network_resolved = reservation
            .try_split(self.network_resolved)
            .expect("aggregate native output reservation includes resolution bits");
        assert_eq!(reservation.remaining_bytes(), 0);
        Ok(NativeOutputProducerCharges {
            budget: budget.clone(),
            row_slots: Some(row_slots),
            source_entries: Some(source_entries),
            source_routes: Some(source_routes),
            _network_resolved: network_resolved,
        })
    }

    #[cfg(test)]
    fn total_bytes(self) -> usize {
        self.total_bytes
    }
}

/// Move-only charges retained by the actual output producer and its descendants.
pub(super) struct NativeOutputProducerCharges {
    budget: AllocationBudget,
    pub(super) row_slots: Option<AllocationCharge>,
    pub(super) source_entries: Option<AllocationCharge>,
    pub(super) source_routes: Option<AllocationCharge>,
    pub(super) _network_resolved: AllocationCharge,
}

impl NativeOutputProducerCharges {
    /// Reuse the source's exact pool for later Network scratch admission.
    pub(super) fn budget(&self) -> &AllocationBudget {
        &self.budget
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_output_vectors_are_prepaid_and_refunded_from_one_pool() {
        let demand = NativeOutputProducerDemand::plan(7, 4).unwrap();
        let short = AllocationBudget::new(demand.total_bytes() - 1);
        assert!(matches!(
            demand.try_reserve(&short),
            Err(AllocationRefusal::ExceedsLimit { .. })
        ));
        assert_eq!(short.reserved_bytes(), 0);

        let budget = AllocationBudget::new(demand.total_bytes());
        let mut charges = demand.try_reserve(&budget).unwrap();
        assert_eq!(budget.reserved_bytes(), demand.total_bytes());
        assert_eq!(
            charges.row_slots.as_ref().unwrap().layout(),
            demand.row_slots
        );
        assert_eq!(charges._network_resolved.layout(), demand.network_resolved);
        let retained_rows = charges.row_slots.take().unwrap();
        drop(charges);
        assert_eq!(budget.reserved_bytes(), demand.row_slots.size());
        drop(retained_rows);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
