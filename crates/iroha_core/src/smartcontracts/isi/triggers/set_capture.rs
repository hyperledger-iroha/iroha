//! Caller-owned capture of the ten original trigger writers and notifications.

use super::*;
use mv::{BlockCapture, BlockRetirement, CaptureCleanup};

/// Original release owners held through an enclosing World capture.
#[must_use = "retain trigger capture cleanup through enclosing physical writers"]
pub(crate) struct SetCaptureCleanup([Option<CaptureCleanup>; 10]);

impl Default for SetCaptureCleanup {
    fn default() -> Self {
        Self(std::array::from_fn(|_| None))
    }
}

impl Drop for SetCaptureCleanup {
    fn drop(&mut self) {
        for cleanup in &mut self.0 {
            drop(cleanup.take());
        }
    }
}

macro_rules! capture_fields {
    ($($field:ident: ($key:ty, $value:ty)),+ $(,)?) => {
        struct CapturingSet<'set, Admission> {
            $($field: Option<mv::storage::BlockCaptureSlot<'set, $key, $value, ()>>,)+
            admission: Option<Admission>,
        }

        impl<'set, Admission> CapturingSet<'set, Admission> {
            fn new(original: SetBlock<'set>, admission: Admission) -> Self {
                // Only infallible, inert owner moves occur across this extraction.
                let SetBlockFields { $($field,)+ } = original.into_fields();
                Self { $($field: Some($field.capture_slot()),)+ admission: Some(admission) }
            }

            fn capture(&mut self) {
                $(match self.$field.as_mut().expect("original trigger capture slot")
                    .try_capture(|_| Ok::<(), core::convert::Infallible>(() )) {
                    Ok(()) => {},
                    Err(impossible) => match impossible {},
                })+
            }

            fn release(&mut self) {
                $(if let Some(field) = self.$field.as_mut() { field.release(); })+
            }

            fn finish(mut self, mode: mv::BlockMode) -> (DetachedSet<Admission>, SetCaptureCleanup) {
                // All native capture calls completed before any journal transfer.
                $(let $field = self.$field.take().expect("original trigger capture slot").into_detached();)+
                let cleanup = SetCaptureCleanup([$(Some($field.1),)+]);
                let journal = DetachedSet {
                    mode,
                    $($field: $field.0,)+
                    admission: self.admission.take().expect("original trigger admission"),
                };
                (journal, cleanup)
            }
        }

        impl<Admission> Drop for CapturingSet<'_, Admission> {
            fn drop(&mut self) { self.release(); }
        }
    };
}

capture_fields! {
    data_triggers: (TriggerId, LoadedAction<DataEventFilter>),
    pipeline_triggers: (TriggerId, LoadedAction<PipelineEventFilterBox>),
    time_triggers: (TriggerId, LoadedAction<TimeEventFilter>),
    by_call_triggers: (TriggerId, LoadedAction<ExecuteTriggerEventFilter>),
    ids: (TriggerId, TriggeringEventType),
    active_data_trigger_ids: (TriggerId, ()),
    active_pipeline_trigger_ids: (TriggerId, ()),
    active_time_trigger_ids: (TriggerId, ()),
    active_by_call_trigger_ids: (TriggerId, ()),
    contracts: (HashOf<IvmBytecode>, IvmBytecodeEntry),
}

enum CapturePhase<'set, Admission> {
    Empty,
    Attached(SetBlock<'set>),
    Capturing(CapturingSet<'set, Admission>),
    Captured(DetachedSet<Admission>),
}

/// Original trigger capture retained by its enclosing field aggregate.
pub(crate) struct SetBlockCapture<'set, Admission> {
    phase: CapturePhase<'set, Admission>,
    started: bool,
    // Last: retained payloads precede the original release notifications.
    cleanup: SetCaptureCleanup,
}

impl<'set> SetBlock<'set> {
    /// Move all original writers into an inert caller-owned capture slot.
    pub(crate) fn capture_slot<Admission>(self) -> SetBlockCapture<'set, Admission> {
        SetBlockCapture {
            phase: CapturePhase::Attached(self),
            started: false,
            cleanup: SetCaptureCleanup::default(),
        }
    }
}

impl<'set, Admission> SetBlockCapture<'set, Admission> {
    /// Retain all original trigger state through checks, admission and capture.
    pub(crate) fn try_capture<E>(
        &mut self,
        admit: impl FnOnce(&SetBlock<'set>) -> Result<Admission, E>,
    ) -> Result<(), DetachError<E>> {
        assert!(!self.started, "original trigger capture is one-shot");
        self.started = true;
        let CapturePhase::Attached(original) = &self.phase else {
            panic!("original attached trigger capture");
        };
        let mode = original.capture_mode().map_err(|error| match error {
            DetachError::InconsistentMode {
                field,
                expected,
                actual,
            } => DetachError::InconsistentMode {
                field,
                expected,
                actual,
            },
            DetachError::Admission(impossible) => match impossible {},
        })?;
        let admission = admit(original).map_err(DetachError::Admission)?;
        let CapturePhase::Attached(original) =
            std::mem::replace(&mut self.phase, CapturePhase::Empty)
        else {
            unreachable!("original checked trigger block");
        };
        self.phase = CapturePhase::Capturing(CapturingSet::new(original, admission));
        let CapturePhase::Capturing(pending) = &mut self.phase else {
            unreachable!()
        };
        pending.capture();
        let CapturePhase::Capturing(pending) =
            std::mem::replace(&mut self.phase, CapturePhase::Empty)
        else {
            unreachable!("original completed trigger capture");
        };
        let (journal, cleanup) = pending.finish(mode);
        self.phase = CapturePhase::Captured(journal);
        self.cleanup = cleanup;
        Ok(())
    }

    /// Release every remaining writer without destroying its original cleanup.
    pub(crate) fn release(&mut self) {
        self.started = true;
        match &mut self.phase {
            CapturePhase::Attached(original) => original.release_writers(),
            CapturePhase::Capturing(pending) => pending.release(),
            CapturePhase::Captured(_) | CapturePhase::Empty => {}
        }
    }

    /// Transfer only fully captured journals with their original notifications.
    pub(crate) fn into_detached(mut self) -> (DetachedSet<Admission>, SetCaptureCleanup) {
        match std::mem::replace(&mut self.phase, CapturePhase::Empty) {
            CapturePhase::Captured(journal) => (journal, std::mem::take(&mut self.cleanup)),
            original => {
                self.phase = original;
                panic!("original trigger capture did not complete");
            }
        }
    }
}

impl<Admission> Drop for SetBlockCapture<'_, Admission> {
    fn drop(&mut self) {
        self.release();
    }
}
