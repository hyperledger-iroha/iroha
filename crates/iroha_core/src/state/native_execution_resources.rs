//! Move the original Native source allocation charge with its execution.
//!
//! This accounts the requested outer verified-group vector and proposal Box
//! before source authentication allocates either. Nested source values, State
//! execution, IVM work, journals and publication need separate charges from
//! this same finite pool before this token can authorize production execution.

use super::VerifiedLaneDecisionGroupV1;
use iroha_data_model::block::SignedBlock;
use mv::allocation::{AllocationBudget, AllocationCharge, AllocationRefusal};
use std::{alloc::Layout, ops::Deref};

/// The requested source layouts, checked without allocating either object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NativeSourceStructuralDemand {
    groups: Layout,
    carrier: Layout,
    total_bytes: usize,
}

impl NativeSourceStructuralDemand {
    /// Plan the exact requested outer Vec and Box layouts for this wire count.
    pub(crate) fn plan(group_count: usize) -> Result<Self, AllocationRefusal> {
        let groups = Layout::array::<VerifiedLaneDecisionGroupV1>(group_count)
            .map_err(|_| AllocationRefusal::DemandOverflow)?;
        let carrier = Layout::new::<SignedBlock>();
        let total_bytes = groups
            .size()
            .checked_add(carrier.size())
            .ok_or(AllocationRefusal::DemandOverflow)?;
        Ok(Self {
            groups,
            carrier,
            total_bytes,
        })
    }

    /// Bytes requested from the one original finite pool.
    pub(crate) fn total_bytes(self) -> usize {
        self.total_bytes
    }
}

/// Prepaid source structure and the same pool used by later Native phases.
///
/// The group charge follows the Vec into retained custody. The Box charge
/// follows the original source Box until its allocation is freed just before
/// execution; the SignedBlock value itself moves into the recorder once.
#[must_use = "move the original Native source charge through the retained carrier"]
pub(crate) struct NativeExecutionResourceAdmission {
    budget: AllocationBudget,
    group_count: usize,
    _group_slots: AllocationCharge,
    source_box: Option<AllocationCharge>,
    executions: Option<AllocationCharge>,
}

impl NativeExecutionResourceAdmission {
    /// Reserve both original structural layouts before authenticated group import.
    pub(crate) fn try_reserve_source(
        budget: &AllocationBudget,
        group_count: usize,
    ) -> Result<Self, AllocationRefusal> {
        let demand = NativeSourceStructuralDemand::plan(group_count)?;
        let mut reservation = budget.try_reserve_bytes(demand.total_bytes())?;
        let group_slots = reservation
            .try_split(demand.groups)
            .expect("aggregate Native source reservation includes its group Vec");
        let source_box = reservation
            .try_split(demand.carrier)
            .expect("aggregate Native source reservation includes its proposal Box");
        assert_eq!(reservation.remaining_bytes(), 0);
        Ok(Self {
            budget: budget.clone(),
            group_count,
            _group_slots: group_slots,
            source_box: Some(source_box),
            executions: None,
        })
    }

    /// Require the exact group count bound when this token was first reserved.
    pub(crate) fn matches_group_count(&self, group_count: usize) -> bool {
        self.group_count == group_count
    }

    /// Borrow the identical finite pool for an aggregate output reservation.
    pub(crate) fn budget(&self) -> &AllocationBudget {
        &self.budget
    }

    /// Keep the final executions Vec charge with the original retained custody.
    /// The producer must split this from this admission's exact budget before
    /// allocating the Vec. A second attachment leaves its original owner intact.
    pub(crate) fn hold_executions_charge(
        &mut self,
        charge: AllocationCharge,
    ) -> Result<(), AllocationCharge> {
        if self.executions.is_some() || !charge.belongs_to(&self.budget) {
            return Err(charge);
        }
        self.executions = Some(charge);
        Ok(())
    }

    /// Bind the prepaid Box charge to the sole owned proposal allocation.
    pub(crate) fn fund_source_box(&mut self, carrier: SignedBlock) -> NativeSourceBox {
        let charge = self
            .source_box
            .take()
            .expect("Native source Box charge can be consumed only once");
        NativeSourceBox {
            value: Box::new(carrier),
            charge,
        }
    }

    /// Finite source charge for standalone scratch qualification only.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn for_test(group_count: usize) -> Self {
        let budget = AllocationBudget::new(16 << 20);
        Self::try_reserve_source(&budget, group_count)
            .expect("standalone Native scratch source fits its finite test pool")
    }

    /// Reserve from even a deliberately malformed fixture's advertised count.
    /// The canonical source constructor still checks that carrier afterward.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn for_test_carrier(carrier: &SignedBlock) -> Self {
        let group_count = carrier
            .execution_context()
            .and_then(|context| context.native_lane_decisions.as_deref())
            .map_or(0, |batch| batch.groups.len());
        Self::for_test(group_count)
    }

    #[cfg(test)]
    pub(crate) fn group_layout_for_test(&self) -> Layout {
        self._group_slots.layout()
    }
}

/// The exact owned source Box and its requested-layout charge.
#[must_use = "keep the source Box charge until its backing allocation is freed"]
pub(crate) struct NativeSourceBox {
    value: Box<SignedBlock>,
    charge: AllocationCharge,
}

impl NativeSourceBox {
    /// Move the original value only after freeing its Box backing allocation.
    pub(crate) fn into_inner(self) -> SignedBlock {
        let Self { value, charge } = self;
        let carrier = *value;
        drop(charge);
        carrier
    }
}

impl Deref for NativeSourceBox {
    type Target = SignedBlock;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_layouts_reserve_and_refund_exact_original_bytes() {
        let demand = NativeSourceStructuralDemand::plan(3).unwrap();
        assert_eq!(
            demand.total_bytes(),
            Layout::array::<VerifiedLaneDecisionGroupV1>(3)
                .unwrap()
                .size()
                + Layout::new::<SignedBlock>().size()
        );
        let short = AllocationBudget::new(demand.total_bytes() - 1);
        assert!(matches!(
            NativeExecutionResourceAdmission::try_reserve_source(&short, 3),
            Err(AllocationRefusal::ExceedsLimit { .. })
        ));
        assert_eq!(short.reserved_bytes(), 0);
        let budget = AllocationBudget::new(demand.total_bytes());
        let admission = NativeExecutionResourceAdmission::try_reserve_source(&budget, 3).unwrap();
        assert!(admission.matches_group_count(3));
        assert!(!admission.matches_group_count(2));
        assert_eq!(admission.group_layout_for_test(), demand.groups);
        assert_eq!(budget.reserved_bytes(), demand.total_bytes());
        drop(admission);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn final_execution_charge_remains_with_source_admission() {
        let source = NativeSourceStructuralDemand::plan(1).unwrap();
        let budget = AllocationBudget::new(source.total_bytes() + 8);
        let mut admission =
            NativeExecutionResourceAdmission::try_reserve_source(&budget, 1).unwrap();
        let foreign = AllocationBudget::new(8);
        let mut foreign_reservation = foreign.try_reserve_bytes(8).unwrap();
        let foreign_charge = foreign_reservation
            .try_split(Layout::array::<u8>(8).unwrap())
            .unwrap();
        let foreign_charge = admission
            .hold_executions_charge(foreign_charge)
            .err()
            .expect("a foreign same-size pool cannot authorize executions");
        assert_eq!(foreign.reserved_bytes(), 8);
        drop(foreign_charge);
        assert_eq!(foreign.reserved_bytes(), 0);
        let mut execution_reservation = budget.try_reserve_bytes(8).unwrap();
        let charge = execution_reservation
            .try_split(Layout::array::<u8>(8).unwrap())
            .unwrap();
        admission.hold_executions_charge(charge).unwrap();
        let repeated = execution_reservation
            .try_split(Layout::array::<u8>(0).unwrap())
            .unwrap();
        assert!(admission.hold_executions_charge(repeated).is_err());
        assert_eq!(budget.reserved_bytes(), source.total_bytes() + 8);
        drop(admission);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
