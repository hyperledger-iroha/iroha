//! Actual frozen Time matching under the common internal invocation owner.

use super::internal::{InternalInvocation, InternalOutputDisposition};
use super::*;
use crate::smartcontracts::isi::triggers::set::{
    SetReadOnly, invocation_identity::time_trigger_use_v1, time_trigger_action_is_due,
};
use iroha_data_model::{
    block::execution_output::TimeInvocationV1,
    events::{EventBox, time::TimeEvent},
    trigger::TriggerId,
};
use mv::storage::StorageReadOnly;

impl ExecutionOutputProducer<'_, '_, '_> {
    /// Consume the actual frozen Time schedule once after prior output phases.
    /// No caller supplies an event, index, descriptor, result or skip count.
    pub(super) fn execute_scheduled_time_outputs(&mut self) -> Result<(), String> {
        let result = (|| {
            if self.failed || self.time_started || self.network_resolved.iter().any(|done| !done) {
                return Err("Time phase is repeated or has unresolved prior work".into());
            }
            self.time_started = true;
            // Even zero-count release verifies prior phases before maintenance.
            self.budget
                .as_mut()
                .ok_or("output budget already consumed")?
                .skip_uninvoked(ExecutionOutputPhase::Time, 0)?;
            let (event, maximum) = self.state.prepare_owned_time_phase(&self.source.header())?;
            let height = self.source.header().height().get();
            let now = u64::try_from(self.source.header().creation_time().as_millis())
                .map_err(|_| "Time timestamp exceeds u64")?;
            let mut matched = Vec::new();
            matched
                .try_reserve_exact(maximum)
                .map_err(|_| "host cannot retain bounded Time matches")?;
            matched.extend(
                self.state
                    .world
                    .triggers
                    .match_time_event(event, height, now, maximum),
            );
            if matched.len() > maximum {
                return Err("Time matcher exceeded its frozen capacity".into());
            }
            let unused = maximum - matched.len();
            self.budget
                .as_mut()
                .ok_or("output budget already consumed")?
                .skip_uninvoked(
                    ExecutionOutputPhase::Time,
                    u32::try_from(unused).map_err(|_| "Time count exceeds u32")?,
                )?;
            let mut suppressed = std::collections::BTreeSet::new();
            for (schedule_index, id) in matched.into_iter().enumerate() {
                if suppressed.contains(&id) {
                    self.skip_uninvoked(ExecutionOutputPhase::Time, 1)?;
                    continue;
                }
                if self.try_apply_time_output(
                    id.clone(),
                    event,
                    u32::try_from(schedule_index)
                        .map_err(|_| "Time schedule position exceeds u32")?,
                )? {
                    suppressed.insert(id);
                }
            }
            Ok(())
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    // True means the existing retry policy suppresses later occurrences this block.
    fn try_apply_time_output(
        &mut self,
        id: TriggerId,
        event: TimeEvent,
        schedule_index: u32,
    ) -> Result<bool, String> {
        let height = self.source.header().height().get();
        let now = u64::try_from(self.source.header().creation_time().as_millis())
            .map_err(|_| "Time timestamp exceeds u64")?;
        let action = self
            .state
            .world
            .triggers
            .time_triggers()
            .get(&id)
            .filter(|action| time_trigger_action_is_due(action, &event, height, now))
            .cloned();
        let Some(action) = action else {
            self.skip_uninvoked(ExecutionOutputPhase::Time, 1)?;
            return Ok(false);
        };
        let invocation = TimeInvocationV1 {
            schedule_index,
            event,
            trigger: time_trigger_use_v1(&self.state.world.triggers, &id, height)?,
        };
        let disposition = self.try_apply_internal(
            InternalInvocation::Time(invocation),
            EventBox::Time(event),
            &action,
        )?;
        Ok(disposition == InternalOutputDisposition::Rejected && action.retry_policy.is_some())
    }
}
