//! Prepare all original trigger journals before publishing any component.

use super::*;
use mv::{PublicationPreparationError, storage::PreparedPublication};
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

// Each failure consumes earlier prepared writers back into their exact original
// journals. No placeholder journal, mutable SetBlock, or reexecution is needed.
macro_rules! prepare_components {
    ($target:ident, $mode:ident, $admission:ident, $installation:ident;
        [$($done:ident,)*]; [$next:ident, $($rest:ident,)*]) => {{
        let $next = match $next.try_prepare_publication(&$target.$next, |_, _| Ok::<_, Infallible>(())) {
            Ok(prepared) => prepared,
            Err(($next, cause)) => {
                $(let $done = $done.abort();)*
                drop($installation);
                return Err((DetachedSet {
                    mode: $mode, $($done,)* $next, $($rest,)* admission: $admission,
                }, SetPublicationError::Component { field: stringify!($next), cause }));
            }
        };
        prepare_components!($target, $mode, $admission, $installation;
            [$($done,)* $next,]; [$($rest,)*])
    }};
    ($target:ident, $mode:ident, $admission:ident, $installation:ident;
        [$($done:ident,)*]; []) => {
        Ok(PreparedSet { mode: $mode, $($done,)* admission: $admission, installation: $installation })
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
    ) -> Result<PreparedSet<'target, Admission, Installation>, (Self, SetPublicationError<E>)> {
        let installation = match admit(&self, target) {
            Ok(installation) => installation,
            Err(error) => return Err((self, SetPublicationError::Admission(error))),
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
        let PreparedSet { mode, $($field,)* admission, installation } = $original;
        consume_components!(@$operation mode, admission, installation; [$($field,)*])
    }};
    (@abort $mode:ident, $admission:ident, $installation:ident; [$($field:ident,)*]) => {{
        $(let $field = $field.abort();)*
        drop($installation);
        DetachedSet { mode: $mode, $($field,)* admission: $admission }
    }};
    (@publish $mode:ident, $admission:ident, $installation:ident; [$($field:ident,)*]) => {{
        let _ = $mode;
        $($field.publish();)*
        ($admission, $installation)
    }};
}

impl<Admission, Installation> PreparedSet<'_, Admission, Installation> {
    /// Release every writer, returning the original ten journals and retention guard.
    pub(crate) fn abort(self) -> DetachedSet<Admission> {
        consume_components!(self, abort; [
            data_triggers, pipeline_triggers, time_triggers, by_call_triggers, ids,
            active_data_trigger_ids, active_pipeline_trigger_ids, active_time_trigger_ids,
            active_by_call_trigger_ids, contracts,
        ])
    }

    /// Consume each exact prepared component and return both resource guards.
    pub(crate) fn publish(self) -> (Admission, Installation) {
        consume_components!(self, publish; [
            data_triggers, pipeline_triggers, time_triggers, by_call_triggers, ids,
            active_data_trigger_ids, active_pipeline_trigger_ids, active_time_trigger_ids,
            active_by_call_trigger_ids, contracts,
        ])
    }
}
