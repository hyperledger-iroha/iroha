//! Prepaid test observations using the single production registration engine.

use iroha_allocation::{release::ReleaseRegistration, AllocationBudget};

/// Admit a control from the fixture's original budget before occupying it.
pub(crate) fn registration(budget: &AllocationBudget) -> ReleaseRegistration {
    let mut prepaid = budget
        .try_reserve(ReleaseRegistration::allocation_layout())
        .expect("fixture prepays its exact release registration before work");
    let registration = ReleaseRegistration::from_reservation(&mut prepaid)
        .expect("fixture constructs its prepaid release registration");
    assert!(registration.belongs_to(budget));
    registration
}
