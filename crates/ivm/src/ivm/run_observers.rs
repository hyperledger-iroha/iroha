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
    type Step<T>: ObservationSlot<T>;
    type Batch<T>: ObservationSlot<T>;
    const ENABLED: bool;
}

impl RunObservers for UnobservedRegisters {
    type Step<T> = NoObservation<T>;
    type Batch<T> = NoObservation<T>;
    const ENABLED: bool = false;
}

/// Diagnostic steps, native packets, or PC deltas without a proof register logger.
#[derive(Clone, Copy)]
pub(super) struct LocalObservers;

impl RegisterObserver for LocalObservers {
    #[inline]
    fn observe(self, build: impl FnOnce() -> crate::zk::RegEvent) {
        UnobservedRegisters.observe(build);
    }
}

impl RunObservers for LocalObservers {
    type Step<T> = Option<T>;
    type Batch<T> = NoObservation<T>;
    const ENABLED: bool = true;
}

impl RunObservers for ScopedRegisters {
    type Step<T> = Option<T>;
    type Batch<T> = Option<T>;
    const ENABLED: bool = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_step_and_register_batch_have_no_backing() {
        type Step = super::super::PendingDiagnosticStep;
        type Batch = crate::zk::RegEventBatch;
        assert_eq!(
            size_of::<<UnobservedRegisters as RunObservers>::Step<Step>>(),
            0
        );
        assert_eq!(
            size_of::<<UnobservedRegisters as RunObservers>::Batch<Batch>>(),
            0
        );
        assert!(
            <UnobservedRegisters as RunObservers>::Step::<u64>::default()
                .take()
                .is_none()
        );
        let mut observed = <ScopedRegisters as RunObservers>::Step::<u64>::default();
        observed.insert_observation(8);
        *ObservationSlot::as_mut(&mut observed).unwrap() = 9;
        assert_eq!(observed.take(), Some(9));
    }

    #[test]
    fn local_observation_records_steps_without_register_events_or_batch_backing() {
        let mut step = <LocalObservers as RunObservers>::Step::<u64>::default();
        step.insert_observation(9);
        assert_eq!(step.take(), Some(9));
        type Batch = <LocalObservers as RunObservers>::Batch<crate::zk::RegEventBatch>;
        assert_eq!(size_of::<Batch>(), 0);
        assert!(Batch::default().take().is_none());
        LocalObservers.observe(|| panic!("local observation requested a proof register event"));
    }
}
