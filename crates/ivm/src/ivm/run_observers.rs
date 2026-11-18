//! Invocation-selected observation storage for the C10 interpreter.

use std::marker::PhantomData;

use crate::registers::{RegisterObserver, ScopedRegisters, UnobservedRegisters};

/// Optional storage that disappears entirely from an ordinary invocation.
pub(super) trait ObservationSlot<T>: Default {
    fn insert_observation(&mut self, value: T);
    fn as_mut(&mut self) -> Option<&mut T>;
    fn take(&mut self) -> Option<T>;
}

impl<T> ObservationSlot<T> for Option<T> {
    #[inline]
    fn insert_observation(&mut self, value: T) {
        *self = Some(value);
    }

    #[inline]
    fn as_mut(&mut self) -> Option<&mut T> {
        Option::as_mut(self)
    }

    #[inline]
    fn take(&mut self) -> Option<T> {
        Option::take(self)
    }
}

pub(super) struct NoObservation<T>(PhantomData<fn() -> T>);

impl<T> Default for NoObservation<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T> ObservationSlot<T> for NoObservation<T> {
    #[inline]
    fn insert_observation(&mut self, _: T) {
        unreachable!("disabled observers never start an observation")
    }

    #[inline]
    fn as_mut(&mut self) -> Option<&mut T> {
        None
    }

    #[inline]
    fn take(&mut self) -> Option<T> {
        None
    }
}

/// Only the instrumented interpreter needs pending diagnostic snapshots or proof batches.
pub(super) trait RunObservers: RegisterObserver {
    type Pending<T>: ObservationSlot<T>;
    const ENABLED: bool;
}

impl RunObservers for UnobservedRegisters {
    type Pending<T> = NoObservation<T>;
    const ENABLED: bool = false;
}

impl RunObservers for ScopedRegisters {
    type Pending<T> = Option<T>;
    const ENABLED: bool = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_step_and_register_batch_have_no_backing() {
        type Pending<T> = <UnobservedRegisters as RunObservers>::Pending<T>;
        assert_eq!(size_of::<Pending<super::super::PendingDiagnosticStep>>(), 0);
        assert_eq!(size_of::<Pending<crate::zk::RegEventBatch>>(), 0);
        assert!(Pending::<u64>::default().take().is_none());
        let mut observed = <ScopedRegisters as RunObservers>::Pending::<u64>::default();
        observed.insert_observation(8);
        *ObservationSlot::as_mut(&mut observed).unwrap() = 9;
        assert_eq!(observed.take(), Some(9));
    }
}
