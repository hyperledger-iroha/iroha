//! This module contains implementations of smart-contract traits and
//! instructions for triggers in Iroha.

use iroha_data_model::{
    evaluate::ExpressionEvaluator, isi::error::MathError, prelude::*, query::error::FindError,
};
use iroha_telemetry::metrics;

pub mod set;

/// All instructions related to triggers.
/// - registering a trigger
/// - un-registering a trigger
/// - TODO: technical accounts.
/// - TODO: technical account permissions.
pub mod isi {
    use iroha_data_model::{
        events::Filter, isi::error::InvalidParameterError, trigger::prelude::*,
    };

    use super::{super::prelude::*, *};

    impl Execute for Register<Trigger<FilterBox, Executable>> {
        #[metrics(+"register_trigger")]
        #[allow(clippy::expect_used)]
        fn execute(self, _authority: &AccountId, wsv: &WorldStateView) -> Result<(), Error> {
            let new_trigger = self.object;

            if !new_trigger.action.filter.mintable() {
                match &new_trigger.action.repeats {
                    Repeats::Exactly(action) if action.get() == 1 => (),
                    _ => {
                        return Err(MathError::Overflow.into());
                    }
                }
            }

            wsv.modify_triggers(|triggers| {
                let trigger_id = new_trigger.id().clone();
                let success = match &new_trigger.action.filter {
                    FilterBox::Data(_) => triggers.add_data_trigger(
                        new_trigger
                            .try_into()
                            .map_err(|e: &str| Error::Conversion(e.to_owned()))?,
                    ),
                    FilterBox::Pipeline(_) => triggers.add_pipeline_trigger(
                        new_trigger
                            .try_into()
                            .map_err(|e: &str| Error::Conversion(e.to_owned()))?,
                    ),
                    FilterBox::Time(_) => triggers.add_time_trigger(
                        new_trigger
                            .try_into()
                            .map_err(|e: &str| Error::Conversion(e.to_owned()))?,
                    ),
                    FilterBox::ExecuteTrigger(_) => triggers.add_by_call_trigger(
                        new_trigger
                            .try_into()
                            .map_err(|e: &str| Error::Conversion(e.to_owned()))?,
                    ),
                }
                .map_err(|e| InvalidParameterError::Wasm(e.to_string()))?;
                if success {
                    Ok(TriggerEvent::Created(trigger_id))
                } else {
                    Err(Error::Repetition(
                        InstructionType::Register,
                        trigger_id.into(),
                    ))
                }
            })
        }
    }

    impl Execute for Unregister<Trigger<FilterBox, Executable>> {
        #[metrics(+"unregister_trigger")]
        fn execute(self, _authority: &AccountId, wsv: &WorldStateView) -> Result<(), Error> {
            let trigger_id = self.object_id.clone();

            wsv.modify_triggers(|triggers| {
                if triggers.remove(&trigger_id) {
                    Ok(TriggerEvent::Deleted(self.object_id))
                } else {
                    Err(Error::Repetition(
                        InstructionType::Unregister,
                        trigger_id.into(),
                    ))
                }
            })
        }
    }

    impl Execute for Mint<Trigger<FilterBox, Executable>, u32> {
        #[metrics(+"mint_trigger_repetitions")]
        fn execute(self, _authority: &AccountId, wsv: &WorldStateView) -> Result<(), Error> {
            let id = self.destination_id;

            wsv.modify_triggers(|triggers| {
                triggers
                    .inspect_by_id(&id, |action| -> Result<(), Error> {
                        if action.mintable() {
                            Ok(())
                        } else {
                            Err(MathError::Overflow.into())
                        }
                    })
                    .ok_or_else(|| Error::Find(Box::new(FindError::Trigger(id.clone()))))??;

                triggers.mod_repeats(&id, |n| {
                    n.checked_add(self.object)
                        .ok_or(super::set::RepeatsOverflowError)
                })?;
                Ok(TriggerEvent::Extended(TriggerNumberOfExecutionsChanged {
                    trigger_id: id,
                    by: self.object,
                }))
            })
        }
    }

    impl Execute for Burn<Trigger<FilterBox, Executable>, u32> {
        #[metrics(+"burn_trigger_repetitions")]
        fn execute(self, _authority: &AccountId, wsv: &WorldStateView) -> Result<(), Error> {
            let trigger = self.destination_id;
            wsv.modify_triggers(|triggers| {
                triggers.mod_repeats(&trigger, |n| {
                    n.checked_sub(self.object)
                        .ok_or(super::set::RepeatsOverflowError)
                })?;
                // TODO: Is it okay to remove triggers with 0 repeats count from `TriggerSet` only
                // when they will match some of the events?
                Ok(TriggerEvent::Shortened(TriggerNumberOfExecutionsChanged {
                    trigger_id: trigger,
                    by: self.object,
                }))
            })
        }
    }

    impl Execute for ExecuteTriggerBox {
        #[metrics(+"execute_trigger")]
        fn execute(self, authority: &AccountId, wsv: &WorldStateView) -> Result<(), Error> {
            let id = wsv.evaluate(&self.trigger_id)?;

            wsv.triggers()
                .inspect_by_id(&id, |action| -> Result<(), Error> {
                    let allow_execute =
                        if let FilterBox::ExecuteTrigger(filter) = action.clone_and_box().filter {
                            let event = ExecuteTriggerEvent {
                                trigger_id: id.clone(),
                                authority: authority.clone(),
                            };

                            filter.matches(&event) || action.technical_account() == authority
                        } else {
                            false
                        };
                    if allow_execute {
                        wsv.execute_trigger(id.clone(), authority);
                        Ok(())
                    } else {
                        Err(ValidationError::new("Unauthorized trigger execution").into())
                    }
                })
                .ok_or_else(|| Error::Find(Box::new(FindError::Trigger(id))))?
        }
    }
}

pub mod query {
    //! Queries associated to triggers.
    use iroha_data_model::{
        query::error::QueryExecutionFailure as Error, trigger::OptimizedExecutable,
    };

    use super::*;
    use crate::prelude::*;

    impl ValidQuery for FindAllActiveTriggerIds {
        #[metrics(+"find_all_active_triggers")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            Ok(wsv.triggers().ids())
        }
    }

    impl ValidQuery for FindTriggerById {
        #[metrics(+"find_trigger_by_id")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            let id = wsv
                .evaluate(&self.id)
                .map_err(|e| Error::Evaluate(format!("Failed to evaluate trigger id. {e}")))?;
            iroha_logger::trace!(%id);
            // Can't use just `ActionTrait::clone_and_box` cause this will trigger lifetime mismatch
            #[allow(clippy::redundant_closure_for_method_calls)]
            let Action {
                executable: loaded_executable,
                repeats,
                technical_account,
                filter,
                metadata,
            } = wsv
                .triggers()
                .inspect_by_id(&id, |action| action.clone_and_box())
                .ok_or_else(|| Error::Find(Box::new(FindError::Trigger(id.clone()))))?;

            let action = Action::new(
                OptimizedExecutable::from(loaded_executable),
                repeats,
                technical_account,
                filter,
            )
            .with_metadata(metadata);

            // TODO: Should we redact the metadata if the account is not the technical account/owner?
            Ok(Trigger::new(id, action))
        }
    }

    impl ValidQuery for FindTriggerKeyValueByIdAndKey {
        #[metrics(+"find_trigger_key_value_by_id_and_key")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            let id = wsv
                .evaluate(&self.id)
                .map_err(|e| Error::Evaluate(format!("Failed to evaluate trigger id. {e}")))?;
            let key = wsv
                .evaluate(&self.key)
                .map_err(|e| Error::Evaluate(format!("Failed to evaluate key. {e}")))?;
            iroha_logger::trace!(%id, %key);
            wsv.triggers()
                .inspect_by_id(&id, |action| {
                    action
                        .metadata()
                        .get(&key)
                        .map(Clone::clone)
                        .ok_or_else(|| FindError::MetadataKey(key.clone()).into())
                })
                .ok_or_else(|| Error::Find(Box::new(FindError::Trigger(id))))?
        }
    }

    impl ValidQuery for FindTriggersByDomainId {
        #[metrics(+"find_triggers_by_domain_id")]
        fn execute(&self, wsv: &WorldStateView) -> eyre::Result<Self::Output, Error> {
            let domain_id = wsv
                .evaluate(&self.domain_id)
                .map_err(|e| Error::Evaluate(format!("Failed to evaluate domain id. {e}")))?;

            let triggers = wsv
                .triggers()
                .inspect_by_domain_id(&domain_id, |trigger_id, action| {
                    let Action {
                        executable: loaded_executable,
                        repeats,
                        technical_account,
                        filter,
                        metadata,
                    } = action.clone_and_box();
                    Trigger::new(
                        trigger_id.clone(),
                        Action::new(
                            OptimizedExecutable::from(loaded_executable),
                            repeats,
                            technical_account,
                            filter,
                        )
                        .with_metadata(metadata),
                    )
                });

            Ok(triggers)
        }
    }
}
