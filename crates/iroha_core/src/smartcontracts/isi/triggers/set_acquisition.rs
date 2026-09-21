//! Joint custody of partial and complete original trigger writers.

use super::*;
use mv::{BlockAcquisition, BlockMode, BlockRetirement};

macro_rules! trigger_acquisition {
    ($($field:ident: ($key:ty, $value:ty)),+ $(,)?) => {
        /// Inert original field slots retained across all fallible initialization.
        pub(crate) struct SetBlockAcquisition<'set> {
            $($field: Option<mv::storage::BlockAcquisitionSlot<'set, $key, $value>>,)+
        }

        impl<'set> SetBlockAcquisition<'set> {
            pub(super) fn new(target: &'set Set) -> Self {
                Self { $($field: Some(target.$field.block_acquisition()),)+ }
            }
        }

        impl<'set> BlockAcquisition for SetBlockAcquisition<'set> {
            type Block = SetBlock<'set>;

            fn initialize(&mut self, mode: BlockMode) {
                $(self.$field.as_mut().expect("original trigger slot").initialize(mode);)+
            }

            fn release(&mut self) {
                $(if let Some(field) = self.$field.as_mut() { field.release(); })+
            }

            fn into_block(mut self) -> Self::Block {
                SetBlock { fields: Some(SetBlockFields {
                    $($field: self.$field.take().expect("original trigger slot").into_block(),)+
                }) }
            }
        }

        impl Drop for SetBlockAcquisition<'_> {
            fn drop(&mut self) {
                self.release();
            }
        }

        impl BlockRetirement for SetBlock<'_> {
            fn release_writers(&mut self) {
                if let Some(fields) = self.fields.as_mut() {
                    $(fields.$field.release_writers();)+
                }
            }
        }
    };
}

trigger_acquisition! {
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

impl<'set> SetBlock<'set> {
    pub(super) fn into_fields(mut self) -> SetBlockFields<'set> {
        // Capture only performs inert moves before its caller owns every slot.
        // TODO: retain aggregate retirement through consuming commit too.
        self.fields.take().expect("original trigger block fields")
    }
}

impl Drop for SetBlock<'_> {
    fn drop(&mut self) {
        self.release_writers();
    }
}
