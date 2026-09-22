//! Owned capture of all ten actual trigger journals after one retention admission.
//!
//! SetBlock has no event queue, execution frame, or prepared-code cache. Its four
//! active-ID maps are real MV owners and are retained alongside action maps,
//! IDs and bytecode/reference counts. Transaction-local postings and lifecycle
//! generations end with the child apply/drop; its mutable borrow precludes
//! detachment while that child exists. World/State event and callback owners are
//! separate and must remain owned by the aggregate carrier handoff.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TODO: connect retained journals to the consuming State publisher"
    )
)]

use super::*;
use mv::storage::Detached as DetachedStorage;

#[path = "set_capture.rs"]
mod capture;
pub(crate) use capture::SetBlockCapture;

#[path = "set_publication.rs"]
mod publication;
pub(crate) use publication::{
    AbortedSet, DetachedSetPublicationSlot, PreparedSet, PublishedSet, SetPublicationError,
};

/// Capture refusal leaves every original trigger writer unpublished and released.
#[derive(Debug, thiserror::Error)]
pub(crate) enum DetachError<E> {
    /// Original component acquisition modes are inconsistent.
    #[error("trigger journal {field} mode {actual:?} differs from {expected:?}")]
    InconsistentMode {
        /// Original component that differs from the common acquisition mode.
        field: &'static str,
        /// Mode of the first original component.
        expected: mv::BlockMode,
        /// Actual mode of the inconsistent component.
        actual: mv::BlockMode,
    },
    /// The caller could not admit retention of the actual original journals.
    #[error("trigger journal retention admission failed")]
    Admission(E),
}

/// Move-only, lifetime-free journals of the actual original trigger block.
///
/// Every component retains its exact current/undo identity, including untouched
/// stores and discarded-tip-only changes in replacement mode. These deltas are
/// not a full SetReadOnly view and grant no publication authority. The original
/// caller's resource guard drops after all retained payloads.
///
/// TODO: compose with the remaining World/State journals, resource admission and
/// finality under one consuming publisher. Prepared component publication alone
/// is not aggregate State authority.
pub(crate) struct DetachedSet<Admission> {
    mode: mv::BlockMode,
    data_triggers: DetachedStorage<TriggerId, LoadedAction<DataEventFilter>, ()>,
    pipeline_triggers: DetachedStorage<TriggerId, LoadedAction<PipelineEventFilterBox>, ()>,
    time_triggers: DetachedStorage<TriggerId, LoadedAction<TimeEventFilter>, ()>,
    by_call_triggers: DetachedStorage<TriggerId, LoadedAction<ExecuteTriggerEventFilter>, ()>,
    ids: DetachedStorage<TriggerId, TriggeringEventType, ()>,
    active_data_trigger_ids: DetachedStorage<TriggerId, (), ()>,
    active_pipeline_trigger_ids: DetachedStorage<TriggerId, (), ()>,
    active_time_trigger_ids: DetachedStorage<TriggerId, (), ()>,
    active_by_call_trigger_ids: DetachedStorage<TriggerId, (), ()>,
    contracts: DetachedStorage<HashOf<IvmBytecode>, IvmBytecodeEntry, ()>,
    admission: Admission,
}

// Preserve all actual source fields explicitly: adding a SetBlock component
// makes this destructuring fail until its owned capture is supplied.
impl SetBlock<'_> {
    /// Check the acquisition mode of all ten original owners without copying values.
    pub(crate) fn capture_mode(
        &self,
    ) -> Result<mv::BlockMode, DetachError<core::convert::Infallible>> {
        let mode = self.data_triggers.mode();
        macro_rules! check_mode {
            ($($field:ident),+ $(,)?) => {$(
                if self.$field.mode() != mode {
                    return Err(DetachError::InconsistentMode {
                        field: stringify!($field),
                        expected: mode,
                        actual: self.$field.mode(),
                    });
                }
            )+};
        }
        check_mode!(
            data_triggers,
            pipeline_triggers,
            time_triggers,
            by_call_triggers,
            ids,
            active_data_trigger_ids,
            active_pipeline_trigger_ids,
            active_time_trigger_ids,
            active_by_call_trigger_ids,
            contracts,
        );
        Ok(mode)
    }

    /// Admit retained values once, capture all original stores, and release writers.
    ///
    /// The callback runs on the immutable complete block before any capture
    /// allocates a delta vector or copies touched final values. It must admit
    /// actual retained allocations, overlap and future installation resources;
    /// this layer invents neither a byte estimator nor a publication permit.
    /// Refusal drops the entire original block without publishing any component.
    pub(crate) fn try_detach<Admission, E>(
        self,
        admit: impl FnOnce(&Self) -> Result<Admission, E>,
    ) -> Result<DetachedSet<Admission>, DetachError<E>> {
        let mut pending = self.capture_slot();
        pending.try_capture(admit)?;
        let (journal, cleanup) = pending.into_detached();
        drop(cleanup);
        Ok(journal)
    }
}

macro_rules! component_accessors {
    ($($field:ident: ($key:ty, $value:ty)),+ $(,)?) => {$(
        #[doc = concat!("Borrow the exact captured `", stringify!($field), "` journal.")]
        pub(crate) fn $field(&self) -> &DetachedStorage<$key, $value, ()> {
            &self.$field
        }
    )+};
}

impl<Admission> DetachedSet<Admission> {
    /// Common actual acquisition mode of all ten original components.
    pub(crate) fn mode(&self) -> mv::BlockMode {
        self.mode
    }

    /// Borrow the retention reservation without releasing it.
    pub(crate) fn admission(&self) -> &Admission {
        &self.admission
    }

    component_accessors! {
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

    /// Advisory comparison against an explicit target's ten exact MV owners.
    ///
    /// These observations are not an atomic Set snapshot or publication lease.
    /// A complete State owner must reacquire and jointly check its components
    /// before any publication; local staleness alone is no consensus verdict.
    pub(crate) fn matches_current(&self, target: &Set) -> bool {
        self.data_triggers.matches_current(&target.data_triggers)
            && self
                .pipeline_triggers
                .matches_current(&target.pipeline_triggers)
            && self.time_triggers.matches_current(&target.time_triggers)
            && self
                .by_call_triggers
                .matches_current(&target.by_call_triggers)
            && self.ids.matches_current(&target.ids)
            && self
                .active_data_trigger_ids
                .matches_current(&target.active_data_trigger_ids)
            && self
                .active_pipeline_trigger_ids
                .matches_current(&target.active_pipeline_trigger_ids)
            && self
                .active_time_trigger_ids
                .matches_current(&target.active_time_trigger_ids)
            && self
                .active_by_call_trigger_ids
                .matches_current(&target.active_by_call_trigger_ids)
            && self.contracts.matches_current(&target.contracts)
    }

    /// Compare all actual acquisition identities and modes of an owned block.
    /// This retains no target reference and grants no publication capability.
    pub(crate) fn matches_block_predecessor(&self, target: &SetBlock<'_>) -> bool {
        self.data_triggers
            .matches_block_predecessor(&target.data_triggers)
            && self
                .pipeline_triggers
                .matches_block_predecessor(&target.pipeline_triggers)
            && self
                .time_triggers
                .matches_block_predecessor(&target.time_triggers)
            && self
                .by_call_triggers
                .matches_block_predecessor(&target.by_call_triggers)
            && self.ids.matches_block_predecessor(&target.ids)
            && self
                .active_data_trigger_ids
                .matches_block_predecessor(&target.active_data_trigger_ids)
            && self
                .active_pipeline_trigger_ids
                .matches_block_predecessor(&target.active_pipeline_trigger_ids)
            && self
                .active_time_trigger_ids
                .matches_block_predecessor(&target.active_time_trigger_ids)
            && self
                .active_by_call_trigger_ids
                .matches_block_predecessor(&target.active_by_call_trigger_ids)
            && self.contracts.matches_block_predecessor(&target.contracts)
    }
}

#[cfg(test)]
#[path = "set_detachment_tests.rs"]
mod tests;
