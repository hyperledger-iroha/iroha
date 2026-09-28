//! Exact original capacity for carrier retention Boxes and World journal shells.
//!
//! This finite structural reservation precedes execution and cannot stand in for
//! concrete World payload, execution scratch, decoding, runtime or archive work.

use super::RetainedCarrierEffects;
use crate::state::world_journals::resources::{
    WorldJournalShellDemand, WorldJournalShellReservation,
};
use mv::allocation::{AllocationBudget, AllocationCharge, AllocationRefusal};
use std::{
    alloc::Layout,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// Fixed retention storage from one original finite pool before execution.
/// The admission type remains bound to the original complete candidate owner.
#[must_use = "move original credits through candidate capture, not into a replacement reservation"]
pub(crate) struct CarrierJournalShellReservation<A> {
    pub(super) world: WorldJournalShellReservation,
    pub(super) effects: AllocationCharge,
    _admission: PhantomData<fn() -> A>,
}

impl<A> CarrierJournalShellReservation<A> {
    pub(super) fn demand() -> Result<(WorldJournalShellDemand, usize), AllocationRefusal> {
        let world = WorldJournalShellDemand::plan()?;
        let total = world
            .total_bytes()
            .checked_add(Layout::new::<RetainedCarrierEffects>().size())
            .ok_or(AllocationRefusal::DemandOverflow)?;
        Ok((world, total))
    }

    /// Reserve all structural overlap atomically before opening candidate writers.
    pub(crate) fn try_reserve(budget: &AllocationBudget) -> Result<Self, AllocationRefusal> {
        let (world_demand, total) = Self::demand()?;
        let mut reservation = budget.try_reserve_bytes(total)?;
        let world = WorldJournalShellReservation::take_reserved(world_demand, &mut reservation)
            .expect("the complete aggregate includes original World shell capacity");
        let effects = reservation
            .try_split(Layout::new::<RetainedCarrierEffects>())
            .expect("the complete aggregate includes the exact effects Box");
        assert_eq!(reservation.remaining_bytes(), 0);
        Ok(Self {
            world,
            effects,
            _admission: PhantomData,
        })
    }

    #[cfg(test)]
    pub(in crate::state) fn for_test() -> Self {
        let (_, total) = Self::demand().expect("fixture journal layouts");
        Self::try_reserve(&AllocationBudget::new(total)).expect("fixture original journal capacity")
    }
}

/// A charge outside its original Box, refunded only after physical deallocation.
/// Neither a naked Box nor its reusable capacity can escape separately.
pub(crate) struct FundedBox<T> {
    value: Box<T>,
    _charge: AllocationCharge,
}

impl<T> FundedBox<T> {
    pub(super) fn new(value: T, charge: AllocationCharge) -> Self {
        assert_eq!(
            charge.layout(),
            Layout::new::<T>(),
            "exact original Box layout"
        );
        Self {
            value: Box::new(value),
            _charge: charge,
        }
    }

    /// Move the payload only after freeing the original backing Box allocation.
    pub(super) fn into_inner(self) -> T {
        let Self {
            value,
            _charge: charge,
        } = self;
        let value = *value;
        drop(charge);
        value
    }
}

impl<T> Deref for FundedBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}
impl<T> DerefMut for FundedBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}
impl<T> AsRef<T> for FundedBox<T> {
    fn as_ref(&self) -> &T {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn carrier_shell_reservation_admits_complete_typed_overlap_once() {
        let (world, bytes) = CarrierJournalShellReservation::<()>::demand().unwrap();
        assert_eq!(
            bytes,
            world.total_bytes() + Layout::new::<RetainedCarrierEffects>().size()
        );
        let too_small = AllocationBudget::new(bytes - 1);
        assert!(matches!(
            CarrierJournalShellReservation::<()>::try_reserve(&too_small),
            Err(AllocationRefusal::ExceedsLimit { .. })
        ));
        assert_eq!(too_small.reserved_bytes(), 0);
        let budget = AllocationBudget::new(bytes);
        let reservation = CarrierJournalShellReservation::<()>::try_reserve(&budget).unwrap();
        assert_eq!(budget.reserved_bytes(), bytes);
        assert_eq!(
            reservation.effects.layout(),
            Layout::new::<RetainedCarrierEffects>()
        );
        assert!(matches!(
            budget.try_reserve_bytes(1),
            Err(AllocationRefusal::Capacity { .. })
        ));
        drop(reservation);
        assert_eq!(budget.reserved_bytes(), 0);
        let (_, larger) = CarrierJournalShellReservation::<[u8; 4096]>::demand().unwrap();
        assert_eq!(
            larger, bytes,
            "admission is retained inline, with no obsolete capture Box"
        );
    }

    #[test]
    fn funded_box_keeps_original_charge_through_payload_drop_and_pointer_moves() {
        struct Probe {
            budget: AllocationBudget,
            observed: Arc<AtomicBool>,
        }
        impl Drop for Probe {
            fn drop(&mut self) {
                self.observed.store(
                    self.budget.reserved_bytes() == Layout::new::<Self>().size(),
                    Ordering::SeqCst,
                );
            }
        }
        let budget = AllocationBudget::new(Layout::new::<Probe>().size());
        let mut reservation = budget.try_reserve(Layout::new::<Probe>()).unwrap();
        let observed = Arc::new(AtomicBool::new(false));
        let value = FundedBox::new(
            Probe {
                budget: budget.clone(),
                observed: Arc::clone(&observed),
            },
            reservation.try_split(Layout::new::<Probe>()).unwrap(),
        );
        let pointer = std::ptr::from_ref(value.as_ref());
        let refused: Result<(), _> = Err(value);
        let value = refused.unwrap_err();
        assert_eq!(std::ptr::from_ref(value.as_ref()), pointer);
        drop(value);
        assert!(observed.load(Ordering::SeqCst));
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn funded_box_into_inner_releases_only_original_backing_storage() {
        let budget = AllocationBudget::new(Layout::new::<Vec<u8>>().size());
        let mut reservation = budget.try_reserve(Layout::new::<Vec<u8>>()).unwrap();
        // The nested Vec is separately owned test data, outside the Box charge.
        let payload = vec![3_u8; 128];
        let pointer = payload.as_ptr();
        let owner = FundedBox::new(
            payload,
            reservation.try_split(Layout::new::<Vec<u8>>()).unwrap(),
        );
        let payload = owner.into_inner();
        assert_eq!(payload.as_ptr(), pointer);
        assert_eq!(payload.len(), 128);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
