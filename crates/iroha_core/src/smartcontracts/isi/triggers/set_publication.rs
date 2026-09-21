//! Prepare all original trigger journals before publishing any component.

use super::*;
use mv::{
    PublicationPreparationError,
    storage::{PreparedPublication, PublishedPublication},
};
use std::convert::Infallible;

/// A local preparation refusal retains the entire original trigger journal.
#[derive(Debug)]
pub(crate) enum SetPublicationError<E> {
    /// Installation capacity was refused before acquiring any component writer.
    Admission(E),
    /// The exact component is busy or no longer has its captured predecessor.
    Component {
        /// Name of the original component that refused publication preparation.
        field: &'static str,
        /// Local writer/cut refusal; component admission is covered by the Set.
        cause: PublicationPreparationError<Infallible>,
    },
}

/// Ten prepared original journals, with every writer retained together.
///
/// This is not a State/finality authorization. The enclosing publisher must
/// prepare all other State components before consuming this owner.
pub(crate) struct PreparedSet<'target, Admission, Installation> {
    original: Option<AcquiredSet<'target, Admission, Installation>>,
}

/// Original physical components consumed only through their aggregate owner.
struct AcquiredSet<'target, Admission, Installation> {
    mode: mv::BlockMode,
    data_triggers: PreparedPublication<'target, TriggerId, LoadedAction<DataEventFilter>, (), ()>,
    pipeline_triggers:
        PreparedPublication<'target, TriggerId, LoadedAction<PipelineEventFilterBox>, (), ()>,
    time_triggers: PreparedPublication<'target, TriggerId, LoadedAction<TimeEventFilter>, (), ()>,
    by_call_triggers:
        PreparedPublication<'target, TriggerId, LoadedAction<ExecuteTriggerEventFilter>, (), ()>,
    ids: PreparedPublication<'target, TriggerId, TriggeringEventType, (), ()>,
    active_data_trigger_ids: PreparedPublication<'target, TriggerId, (), (), ()>,
    active_pipeline_trigger_ids: PreparedPublication<'target, TriggerId, (), (), ()>,
    active_time_trigger_ids: PreparedPublication<'target, TriggerId, (), (), ()>,
    active_by_call_trigger_ids: PreparedPublication<'target, TriggerId, (), (), ()>,
    contracts: PreparedPublication<'target, HashOf<IvmBytecode>, IvmBytecodeEntry, (), ()>,
    admission: Admission,
    installation: Installation,
}

/// Original released trigger participants and their enclosing resource owners.
/// Retain through the entire State publication interval before cleanup.
pub(crate) struct PublishedSet<Admission, Installation> {
    _data_triggers: PublishedPublication<TriggerId, LoadedAction<DataEventFilter>, (), ()>,
    _pipeline_triggers:
        PublishedPublication<TriggerId, LoadedAction<PipelineEventFilterBox>, (), ()>,
    _time_triggers: PublishedPublication<TriggerId, LoadedAction<TimeEventFilter>, (), ()>,
    _by_call_triggers:
        PublishedPublication<TriggerId, LoadedAction<ExecuteTriggerEventFilter>, (), ()>,
    _ids: PublishedPublication<TriggerId, TriggeringEventType, (), ()>,
    _active_data_trigger_ids: PublishedPublication<TriggerId, (), (), ()>,
    _active_pipeline_trigger_ids: PublishedPublication<TriggerId, (), (), ()>,
    _active_time_trigger_ids: PublishedPublication<TriggerId, (), (), ()>,
    _active_by_call_trigger_ids: PublishedPublication<TriggerId, (), (), ()>,
    _contracts: PublishedPublication<HashOf<IvmBytecode>, IvmBytecodeEntry, (), ()>,
    _admission: Admission,
    _installation: Installation,
}

/// Original abort notifications retained until every enclosing writer unlocks.
pub(crate) struct AbortedSet<Installation> {
    _components: [Option<mv::PublicationCleanup<()>>; 10],
    _installation: Option<Installation>,
}

// Each failure consumes earlier prepared writers back into their exact original
// journals. No placeholder journal, mutable SetBlock, or reexecution is needed.
macro_rules! prepare_components {
    ($target:ident, $mode:ident, $admission:ident, $installation:ident;
        [$($done:ident,)*]; [$next:ident, $($rest:ident,)*]) => {{
        let $next = match $next.try_prepare_publication(&$target.$next, |_, _| Ok::<_, Infallible>(())) {
            Ok(prepared) => prepared,
            Err(($next, cause, refused)) => {
                $(let $done = $done.abort();)*
                // Release every acquired component before any original notification.
                let retirement = AbortedSet {
                    _components: [$(Some($done.1),)* Some(refused), $({ let _ = stringify!($rest); None },)*],
                    _installation: Some($installation),
                };
                return Err((DetachedSet {
                    mode: $mode, $($done: $done.0,)* $next, $($rest,)* admission: $admission,
                }, SetPublicationError::Component { field: stringify!($next), cause }, retirement));
            }
        };
        prepare_components!($target, $mode, $admission, $installation;
            [$($done,)* $next,]; [$($rest,)*])
    }};
    ($target:ident, $mode:ident, $admission:ident, $installation:ident;
        [$($done:ident,)*]; []) => {
        Ok(PreparedSet { original: Some(AcquiredSet { mode: $mode, $($done,)* admission: $admission, installation: $installation }) })
    };
}

impl<Admission> DetachedSet<Admission> {
    /// Admit all installation copies and then acquire every original component.
    ///
    /// The callback must cover all ten staging deltas, EBR retention and writer
    /// installation costs before any MV component can copy its values. Refusal
    /// releases all acquired writers and returns the complete original journal.
    pub(crate) fn try_prepare_publication<'target, Installation, E>(
        self,
        target: &'target Set,
        admit: impl FnOnce(&Self, &Set) -> Result<Installation, E>,
    ) -> Result<
        PreparedSet<'target, Admission, Installation>,
        (Self, SetPublicationError<E>, AbortedSet<Installation>),
    > {
        let installation = match admit(&self, target) {
            Ok(installation) => installation,
            Err(error) => {
                return Err((
                    self,
                    SetPublicationError::Admission(error),
                    AbortedSet {
                        _components: std::array::from_fn(|_| None),
                        _installation: None,
                    },
                ));
            }
        };
        let Self {
            mode,
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
            admission,
        } = self;
        prepare_components!(target, mode, admission, installation; []; [
            data_triggers, pipeline_triggers, time_triggers, by_call_triggers, ids,
            active_data_trigger_ids, active_pipeline_trigger_ids, active_time_trigger_ids,
            active_by_call_trigger_ids, contracts,
        ])
    }
}

macro_rules! consume_components {
    ($original:ident, $operation:ident; [$($field:ident,)*]) => {{
        let AcquiredSet { mode, $($field,)* admission, installation } = $original;
        consume_components!(@$operation mode, admission, installation; [$($field,)*])
    }};
    (@abort $mode:ident, $admission:ident, $installation:ident; [$($field:ident,)*]) => {{
        $(let $field = $field.abort();)*
        let retirement = AbortedSet { _components: [$(Some($field.1),)*], _installation: Some($installation) };
        (DetachedSet { mode: $mode, $($field: $field.0,)* admission: $admission }, retirement)
    }};
}

impl<Admission, Installation> AcquiredSet<'_, Admission, Installation> {
    /// Release every writer, returning the original ten journals and retention guard.
    pub(crate) fn abort(self) -> (DetachedSet<Admission>, AbortedSet<Installation>) {
        consume_components!(self, abort; [
            data_triggers, pipeline_triggers, time_triggers, by_call_triggers, ids,
            active_data_trigger_ids, active_pipeline_trigger_ids, active_time_trigger_ids,
            active_by_call_trigger_ids, contracts,
        ])
    }

    /// Consume each exact prepared component and return both resource guards.
    pub(crate) fn publish(self) -> PublishedSet<Admission, Installation> {
        PublishedSet {
            _data_triggers: self.data_triggers.publish(),
            _pipeline_triggers: self.pipeline_triggers.publish(),
            _time_triggers: self.time_triggers.publish(),
            _by_call_triggers: self.by_call_triggers.publish(),
            _ids: self.ids.publish(),
            _active_data_trigger_ids: self.active_data_trigger_ids.publish(),
            _active_pipeline_trigger_ids: self.active_pipeline_trigger_ids.publish(),
            _active_time_trigger_ids: self.active_time_trigger_ids.publish(),
            _active_by_call_trigger_ids: self.active_by_call_trigger_ids.publish(),
            _contracts: self.contracts.publish(),
            _admission: self.admission,
            _installation: self.installation,
        }
    }
}

impl<Admission, Installation> PreparedSet<'_, Admission, Installation> {
    /// Release all original writers before returning journals and deferred cleanup.
    pub(crate) fn abort(mut self) -> (DetachedSet<Admission>, AbortedSet<Installation>) {
        self.original
            .take()
            .expect("original prepared triggers")
            .abort()
    }

    /// Consume the same original components under the enclosing State authority.
    pub(crate) fn publish(mut self) -> PublishedSet<Admission, Installation> {
        self.original
            .take()
            .expect("original prepared triggers")
            .publish()
    }
}

impl<Admission, Installation> Drop for PreparedSet<'_, Admission, Installation> {
    fn drop(&mut self) {
        let Some(original) = self.original.take() else {
            return;
        };
        // Both capacities outlive every original payload and callback, including
        // cleanup unwind. No component may notify while a sibling is still held.
        let admission;
        let installation;
        let AcquiredSet {
            mode: _,
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
            admission: retained_admission,
            installation: retained_installation,
        } = original;
        admission = retained_admission;
        installation = retained_installation;
        let data_triggers = data_triggers.abort();
        let pipeline_triggers = pipeline_triggers.abort();
        let time_triggers = time_triggers.abort();
        let by_call_triggers = by_call_triggers.abort();
        let ids = ids.abort();
        let active_data_trigger_ids = active_data_trigger_ids.abort();
        let active_pipeline_trigger_ids = active_pipeline_trigger_ids.abort();
        let active_time_trigger_ids = active_time_trigger_ids.abort();
        let active_by_call_trigger_ids = active_by_call_trigger_ids.abort();
        let contracts = contracts.abort();
        drop((
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
        ));
        drop((admission, installation));
    }
}
