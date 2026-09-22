//! Caller-owned four-cell capture, including refusal and partial unwind.

use super::*;
use mv::{BlockCapture, BlockRetirement};

macro_rules! runtime_capture_fields {
    ($($field:ident: $value:ty),+ $(,)?) => {
        struct AttachedRuntime<'state> {
            $($field: Option<CellBlock<'state, $value>>,)+
        }

        impl<'state> AttachedRuntime<'state> {
            fn inputs(&self) -> RuntimeJournalInputs<'_, 'state> {
                RuntimeJournalInputs {
                    $($field: self.$field.as_ref().expect("original runtime block"),)+
                }
            }

            fn release(&mut self) {
                $(if let Some(field) = self.$field.as_mut() { field.release_writers(); })+
            }

            fn into_slots(mut self) -> CapturingRuntime<'state> {
                // Inert moves only: every original remains in one of the two
                // aggregates until the complete set has transferred.
                CapturingRuntime {
                    $($field: Some(self.$field.take().expect("original runtime block").capture_slot()),)+
                }
            }
        }

        impl Drop for AttachedRuntime<'_> {
            fn drop(&mut self) { self.release(); }
        }

        struct CapturingRuntime<'state> {
            $($field: Option<mv::cell::BlockCaptureSlot<'state, $value, ()>>,)+
        }

        impl CapturingRuntime<'_> {
            fn capture(&mut self) {
                $(match self.$field.as_mut().expect("original runtime capture slot")
                    .try_capture(|_| Ok::<(), core::convert::Infallible>(())) {
                    Ok(()) => {},
                    Err(impossible) => match impossible {},
                })+
            }

            fn release(&mut self) {
                $(if let Some(field) = self.$field.as_mut() { field.release(); })+
            }

            fn into_journals<Admission>(self, admission: Admission) -> RuntimeJournals<Admission> {
                let admission = admission;
                let mut pending = self;
                $(let $field = pending.$field.take().expect("original runtime capture slot").into_detached();)+
                let cleanup = [$($field.1,)+];
                let journals = RuntimeJournals {
                    $($field: $field.0,)+
                    admission,
                };
                drop(cleanup);
                journals
            }
        }

        impl Drop for CapturingRuntime<'_> {
            fn drop(&mut self) { self.release(); }
        }

        impl<'state, Admission> RuntimeCapture<'state, Admission> {
            /// Install all original cells without admission, allocation or release.
            pub(in crate::state::carrier_preparation::journals) fn new(
                $($field: CellBlock<'state, $value>,)+
            ) -> Self {
                Self {
                    phase: RuntimePhase::Attached(AttachedRuntime { $($field: Some($field),)+ }),
                    started: false,
                    complete: false,
                    admission: None,
                }
            }
        }
    };
}

runtime_capture_fields! {
    canonical_runtime: SnapshotNexusRuntime,
    commit_topology: Vec<PeerId>,
    prev_commit_topology: Vec<PeerId>,
    lane_consensus_contexts: LaneConsensusContextsV1,
}

enum RuntimePhase<'state> {
    Empty,
    Attached(AttachedRuntime<'state>),
    Capturing(CapturingRuntime<'state>),
}

/// All four original runtime writers and their deferred retirement.
pub(in crate::state::carrier_preparation::journals) struct RuntimeCapture<'state, Admission> {
    phase: RuntimePhase<'state>,
    started: bool,
    complete: bool,
    // Last: original payloads and notifications precede resource refund.
    admission: Option<Admission>,
}

impl<'state, Admission> RuntimeCapture<'state, Admission> {
    /// Admit and capture while every original remains in its caller's aggregate.
    pub(in crate::state::carrier_preparation::journals) fn try_capture<E>(
        &mut self,
        admit: impl FnOnce(RuntimeJournalInputs<'_, 'state>) -> Result<Admission, E>,
    ) -> Result<(), E> {
        assert!(!self.started, "original runtime capture is one-shot");
        self.started = true;
        let RuntimePhase::Attached(original) = &self.phase else {
            panic!("original attached runtime capture");
        };
        self.admission = Some(admit(original.inputs())?);
        // Only inert moves occur before the caller owns the complete slots.
        let RuntimePhase::Attached(original) =
            std::mem::replace(&mut self.phase, RuntimePhase::Empty)
        else {
            unreachable!("original checked runtime capture");
        };
        self.phase = RuntimePhase::Capturing(original.into_slots());
        let RuntimePhase::Capturing(pending) = &mut self.phase else {
            unreachable!("original runtime capture slots");
        };
        pending.capture();
        self.complete = true;
        Ok(())
    }

    /// Terminally release all physical writers while retaining original cleanup.
    pub(in crate::state::carrier_preparation::journals) fn release(&mut self) {
        self.started = true;
        self.complete = false;
        match &mut self.phase {
            RuntimePhase::Empty => {}
            RuntimePhase::Attached(original) => original.release(),
            RuntimePhase::Capturing(pending) => pending.release(),
        }
    }

    /// Materialize only after this and every enclosing capture has completed.
    pub(in crate::state::carrier_preparation::journals) fn into_journals(
        mut self,
    ) -> RuntimeJournals<Admission> {
        assert!(self.complete, "original runtime capture did not complete");
        let admission = self.admission.take().expect("original runtime admission");
        let RuntimePhase::Capturing(pending) =
            std::mem::replace(&mut self.phase, RuntimePhase::Empty)
        else {
            unreachable!("original completed runtime capture");
        };
        pending.into_journals(admission)
    }
}

impl<Admission> Drop for RuntimeCapture<'_, Admission> {
    fn drop(&mut self) {
        self.release();
    }
}
