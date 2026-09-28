//! Typed trigger validation borrows the same ten frozen original journals.
//!
//! No reader is opened from current State, no component is cloned, and this
//! implementation exposes no execution/mutation entrypoint. The existing Set
//! publication slot remains the sole reacquisition kernel for these originals.

use super::*;

macro_rules! frozen_read_accessors {
    ($($field:ident: ($key:ty, $value:ty)),+ $(,)?) => {$(
        fn $field(&self) -> &impl StorageReadOnly<$key, $value> {
            &self.$field
        }
    )+};
}

impl<Admission> SetReadOnly for DetachedSet<Admission> {
    frozen_read_accessors! {
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
}
