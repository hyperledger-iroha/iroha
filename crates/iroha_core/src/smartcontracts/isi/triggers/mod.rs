//! This module contains implementations of smart-contract traits and
//! instructions for triggers in Iroha.

use std::sync::OnceLock;

use iroha_data_model::{
    ValidationFail,
    isi::error::MathError,
    prelude::*,
    query::error::FindError,
    transaction::error::prelude::TransactionRejectionReason,
    trigger::action, // for constructing adjusted Action
};
use iroha_telemetry::metrics;

pub mod set;
pub mod specialized;

/// Trigger metadata key toggled by `set_trigger_enabled` syscalls/ISIs.
pub(crate) const TRIGGER_ENABLED_METADATA_KEY: &str = "__enabled";

fn trigger_enabled_metadata_key() -> &'static Name {
    static KEY: OnceLock<Name> = OnceLock::new();
    KEY.get_or_init(|| {
        TRIGGER_ENABLED_METADATA_KEY
            .parse()
            .expect("trigger enabled metadata key must be valid")
    })
}

/// Read the trigger enabled flag from metadata, defaulting to `true` when absent or malformed.
pub(crate) fn trigger_is_enabled(metadata: &Metadata) -> bool {
    let Some(value) = metadata.get(trigger_enabled_metadata_key()) else {
        return true;
    };
    if let Ok(flag) = value.clone().try_into_any_norito::<bool>() {
        return flag;
    }
    if let Ok(raw) = value.clone().try_into_any_norito::<u64>() {
        return raw != 0;
    }
    true
}

/// All instructions related to triggers.
/// - registering a trigger and validating the declared authority against
///   `CanRegisterTrigger{authority: ...}` permissions or domain ownership
/// - un-registering a trigger and cleaning up associated metadata
/// - adjusting trigger metadata to reflect registration height and block time
#[allow(clippy::used_underscore_binding)]
pub mod isi {
    use iroha_data_model::{
        events::EventFilter,
        isi::error::{InvalidParameterError, RepetitionError},
        name::Name,
        trigger::prelude::*,
    };

    use super::{super::prelude::*, *};

    const RESERVED_TRIGGER_METADATA_KEYS: [&str; 2] =
        ["__registered_block_height", "__registered_at_ms"];

    pub(super) fn ensure_metadata_key_is_not_reserved(key: &Name) -> Result<(), Error> {
        if RESERVED_TRIGGER_METADATA_KEYS
            .iter()
            .any(|reserved| key.as_ref() == *reserved)
        {
            return Err(Error::InvalidParameter(
                InvalidParameterError::SmartContract(format!(
                    "Metadata key `{key}` is reserved for trigger lifecycle management",
                )),
            ));
        }
        Ok(())
    }

    fn enforce_trigger_metadata_limits(
        metadata: &Metadata,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        for (key, value) in metadata.iter() {
            ensure_metadata_key_is_not_reserved(key)?;
            crate::smartcontracts::limits::enforce_json_size(
                state_transaction,
                value,
                "max_metadata_value_bytes",
                crate::smartcontracts::limits::DEFAULT_JSON_LIMIT,
            )?;
        }
        Ok(())
    }

    fn is_permission_trigger_associated(
        permission: &iroha_data_model::permission::Permission,
        trigger_id: &TriggerId,
    ) -> bool {
        if let Ok(permission) =
            iroha_executor_data_model::permission::trigger::CanUnregisterTrigger::try_from(
                permission,
            )
        {
            return &permission.trigger == trigger_id;
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::trigger::CanModifyTrigger::try_from(permission)
        {
            return &permission.trigger == trigger_id;
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::trigger::CanExecuteTrigger::try_from(permission)
        {
            return &permission.trigger == trigger_id;
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::trigger::CanModifyTriggerMetadata::try_from(
                permission,
            )
        {
            return &permission.trigger == trigger_id;
        }

        false
    }

    pub(crate) fn remove_trigger_associated_permissions(
        state_transaction: &mut StateTransaction<'_, '_>,
        trigger_id: &TriggerId,
    ) {
        let account_ids: Vec<AccountId> = state_transaction
            .world
            .account_permissions
            .iter()
            .map(|(holder, _)| holder.clone())
            .collect();

        for holder in account_ids {
            let should_remove = state_transaction
                .world
                .account_permissions
                .get(&holder)
                .is_some_and(|permissions| {
                    permissions
                        .iter()
                        .any(|permission| is_permission_trigger_associated(permission, trigger_id))
                });
            if !should_remove {
                continue;
            }

            let remove_entry = if let Some(permissions) =
                state_transaction.world.account_permissions.get_mut(&holder)
            {
                permissions
                    .retain(|permission| !is_permission_trigger_associated(permission, trigger_id));
                permissions.is_empty()
            } else {
                false
            };

            if remove_entry {
                state_transaction
                    .world
                    .account_permissions
                    .remove(holder.clone());
            }

            state_transaction.invalidate_permission_cache_for_account(&holder);
        }

        let role_ids: Vec<RoleId> = state_transaction
            .world
            .roles
            .iter()
            .map(|(role_id, _)| role_id.clone())
            .collect();

        for role_id in role_ids {
            let should_remove = state_transaction
                .world
                .roles
                .get(&role_id)
                .is_some_and(|role| {
                    role.permissions()
                        .any(|permission| is_permission_trigger_associated(permission, trigger_id))
                });
            if !should_remove {
                continue;
            }

            let impacted_accounts = state_transaction.accounts_with_role(&role_id);

            if let Some(role) = state_transaction.world.roles.get_mut(&role_id) {
                role.permissions
                    .retain(|permission| !is_permission_trigger_associated(permission, trigger_id));
                role.permission_epochs
                    .retain(|permission, _| role.permissions.contains(permission));
            }

            if !impacted_accounts.is_empty() {
                state_transaction.invalidate_permission_cache_for(impacted_accounts.iter());
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn register_trigger_internal(
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
        trigger: Trigger,
        skip_permission_check: bool,
    ) -> Result<(), Error> {
        let mut new_trigger = trigger;

        if !skip_permission_check {
            // Enforce minimal permission: only genesis block, domain owner of the trigger owner,
            // or an account with CanRegisterTrigger{authority: <owner>} may register the trigger.
            let owner = new_trigger.action().authority().clone();
            let is_genesis = state_transaction._curr_block.is_genesis();
            let mut is_domain_owner = false;
            for domain_id in state_transaction.world.domains_for_subject(&owner) {
                let domain_owner = state_transaction
                    .world
                    .domain(&domain_id)
                    .map(|domain| domain.owned_by().clone())
                    .map_err(Error::Find)?;
                if &domain_owner == authority {
                    is_domain_owner = true;
                    break;
                }
            }
            let has_permission =
                (!is_genesis) && state_transaction.can_register_trigger_for(authority, &owner);
            if !(is_genesis || is_domain_owner || has_permission) {
                return Err(Error::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "Missing CanRegisterTrigger{{authority: {owner}}} permission for {authority}"
                    )),
                ));
            }
        }

        if let EventFilterBox::Time(TimeEventFilter(ExecutionTime::Schedule(schedule))) =
            new_trigger.action().filter()
        {
            if schedule.period_ms.is_some_and(|period| period == 0) {
                return Err(Error::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "Time trigger period must be greater than zero".into(),
                    ),
                ));
            }
        }

        // If a time trigger is scheduled at or before the current block creation time,
        // shift its start strictly after the current block to prevent immediate firing.
        if let EventFilterBox::Time(time_filter) = new_trigger.action().filter()
            && let ExecutionTime::Schedule(mut schedule) = time_filter.0
        {
            let block_time = state_transaction._curr_block.creation_time();
            let start = schedule.start();
            if start <= block_time {
                let adjusted_start = block_time
                    .checked_add(core::time::Duration::from_millis(1))
                    .unwrap_or(block_time);
                schedule.start_ms = adjusted_start
                    .as_millis()
                    .try_into()
                    .expect("INTERNAL BUG: Unix timestamp exceeds u64::MAX");
                // Rebuild the action with the adjusted schedule while preserving other fields and metadata
                let act = new_trigger.action().clone();
                let updated = action::Action {
                    executable: act.executable,
                    repeats: act.repeats,
                    authority: act.authority,
                    filter: EventFilterBox::Time(TimeEventFilter(ExecutionTime::Schedule(
                        schedule,
                    ))),
                    metadata: act.metadata,
                };
                new_trigger = Trigger::new(new_trigger.id().clone(), updated);
            }
        }

        // Mark triggers registered within the current block so they do not fire in the same block.
        // This flag is used as a secondary guard in the execution path.
        {
            use iroha_primitives::json::Json;
            enforce_trigger_metadata_limits(new_trigger.action().metadata(), state_transaction)?;
            let key_height = "__registered_block_height"
                .parse::<Name>()
                .expect("Valid metadata key");
            let key_time = "__registered_at_ms"
                .parse::<Name>()
                .expect("Valid metadata key");
            let height = state_transaction._curr_block.height().get();
            let created_ms = state_transaction._curr_block.creation_time().as_millis();
            let created_ms = u64::try_from(created_ms).unwrap_or(u64::MAX);
            let mut metadata = new_trigger.action().clone().metadata;
            metadata.insert(key_height, Json::from(height));
            metadata.insert(key_time, Json::from(created_ms));
            let action = new_trigger.action().clone().with_metadata(metadata);
            new_trigger = Trigger::new(new_trigger.id().clone(), action);
        }

        if !new_trigger.action().filter().mintable() {
            match new_trigger.action().repeats() {
                Repeats::Exactly(1) => (),
                _ => {
                    return Err(MathError::Overflow.into());
                }
            }
        }

        // If the trigger is already depleted (Exactly(0)) do not register it.
        // This enforces the lifecycle policy that zero-repeat triggers are removed immediately.
        if new_trigger.action().repeats().is_depleted() {
            return Ok(());
        }

        let triggers = &mut state_transaction.world.triggers;
        let trigger_id = new_trigger.id().clone();
        let success = match new_trigger.action().filter() {
            EventFilterBox::Data(_) => triggers.add_data_trigger(
                new_trigger
                    .try_into()
                    .map_err(|e: &str| Error::Conversion(e.to_owned()))?,
            ),
            EventFilterBox::Pipeline(_) => triggers.add_pipeline_trigger(
                new_trigger
                    .try_into()
                    .map_err(|e: &str| Error::Conversion(e.to_owned()))?,
            ),
            EventFilterBox::Time(_time_filter) => triggers.add_time_trigger(
                new_trigger
                    .try_into()
                    .map_err(|e: &str| Error::Conversion(e.to_owned()))?,
            ),
            EventFilterBox::ExecuteTrigger(_) => triggers.add_by_call_trigger(
                new_trigger
                    .try_into()
                    .map_err(|e: &str| Error::Conversion(e.to_owned()))?,
            ),
            EventFilterBox::TriggerCompleted(_) => {
                unreachable!("Disallowed during deserialization");
            }
        }
        .map_err(|e| InvalidParameterError::SmartContract(e.to_string()))?;

        if !success {
            return Err(RepetitionError {
                instruction: InstructionType::Register,
                id: trigger_id.into(),
            }
            .into());
        }

        state_transaction
            .world
            .emit_events(Some(TriggerEvent::Created(trigger_id)));

        Ok(())
    }

    impl Execute for Register<Trigger> {
        #[metrics(+"register_trigger")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            register_trigger_internal(authority, state_transaction, self.object().clone(), false)
        }
    }

    impl Execute for Unregister<Trigger> {
        #[metrics(+"unregister_trigger")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let trigger_id = self.object().clone();

            if state_transaction.world.triggers.remove(&trigger_id) {
                remove_trigger_associated_permissions(state_transaction, &trigger_id);
                state_transaction
                    .world
                    .emit_events(Some(TriggerEvent::Deleted(trigger_id)));
                Ok(())
            } else {
                Err(RepetitionError {
                    instruction: InstructionType::Unregister,
                    id: trigger_id.into(),
                }
                .into())
            }
        }
    }

    impl Execute for Mint<u32, Trigger> {
        #[metrics(+"mint_trigger_repetitions")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let id = self.destination().clone();

            let triggers = &mut state_transaction.world.triggers;
            triggers
                .inspect_by_id(&id, |action| -> Result<(), Error> {
                    if action.mintable() {
                        Ok(())
                    } else {
                        Err(MathError::Overflow.into())
                    }
                })
                .ok_or_else(|| Error::Find(FindError::Trigger(id.clone())))??;

            triggers.mod_repeats(&id, |n| {
                n.checked_add(*self.object())
                    .ok_or(super::set::RepeatsOverflowError)
            })?;

            state_transaction
                .world
                .emit_events(Some(TriggerEvent::Extended(
                    TriggerNumberOfExecutionsChanged {
                        trigger: id,
                        by: *self.object(),
                    },
                )));

            Ok(())
        }
    }

    impl Execute for Burn<u32, Trigger> {
        #[metrics(+"burn_trigger_repetitions")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let trigger = self.destination().clone();
            let mut removed = false;
            {
                let triggers = &mut state_transaction.world.triggers;
                triggers.mod_repeats(&trigger, |n| {
                    n.checked_sub(*self.object())
                        .ok_or(super::set::RepeatsOverflowError)
                })?;
                // Remove triggers that reached zero repeats immediately to avoid latent state
                let should_remove = triggers
                    .inspect_by_id(&trigger, |action| action.repeats().is_depleted())
                    .unwrap_or(false);
                if should_remove {
                    removed = triggers.remove(&trigger);
                }
            }
            if removed {
                remove_trigger_associated_permissions(state_transaction, &trigger);
            }
            state_transaction
                .world
                .emit_events(Some(TriggerEvent::Shortened(
                    TriggerNumberOfExecutionsChanged {
                        trigger,
                        by: *self.object(),
                    },
                )));

            Ok(())
        }
    }

    impl Execute for SetKeyValue<Trigger> {
        #[metrics(+"set_trigger_key_value")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let SetKeyValue {
                object: trigger_id,
                key,
                value,
            } = self;
            ensure_metadata_key_is_not_reserved(&key)?;
            crate::smartcontracts::limits::enforce_json_size(
                state_transaction,
                &value,
                "max_metadata_value_bytes",
                crate::smartcontracts::limits::DEFAULT_JSON_LIMIT,
            )?;

            state_transaction
                .world
                .triggers
                .inspect_by_id_mut(&trigger_id, |action| {
                    action.metadata_mut().insert(key.clone(), value.clone())
                })
                .ok_or(FindError::Trigger(trigger_id.clone()))?;

            state_transaction
                .world
                .emit_events(Some(TriggerEvent::MetadataInserted(MetadataChanged {
                    target: trigger_id,
                    key,
                    value,
                })));

            Ok(())
        }
    }

    impl Execute for RemoveKeyValue<Trigger> {
        #[metrics(+"remove_trigger_key_value")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let trigger_id = self.object().clone();
            ensure_metadata_key_is_not_reserved(self.key())?;

            let value = state_transaction
                .world
                .triggers
                .inspect_by_id_mut(&trigger_id, |action| {
                    action
                        .metadata_mut()
                        .remove(self.key().as_ref())
                        .ok_or_else(|| FindError::MetadataKey(self.key().clone()))
                })
                .ok_or(FindError::Trigger(trigger_id.clone()))??;

            state_transaction
                .world
                .emit_events(Some(TriggerEvent::MetadataRemoved(MetadataChanged {
                    target: trigger_id,
                    key: self.key().clone(),
                    value,
                })));

            Ok(())
        }
    }

    impl Execute for ExecuteTrigger {
        #[metrics(+"execute_trigger")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            // Enforce args payload size
            enforce_payload_limit(state_transaction, self.args())?;
            let id = self.trigger();

            let event = ExecuteTriggerEvent {
                trigger_id: id.clone(),
                authority: authority.clone(),
                args: self.args().clone(),
            };

            // Precompute permission check before borrowing `triggers` view to avoid
            // conflicting borrows and to keep the closure `Fn`-compatible.
            let perm_ok_global = state_transaction.can_execute_trigger_for(authority, id);

            state_transaction
                .world
                .triggers
                .inspect_by_id(id, |action| -> Result<(), Error> {
                    // If the trigger has no remaining executions, treat it as not found.
                    if action.repeats().is_depleted() {
                        return Err(Error::Find(FindError::Trigger(id.clone())));
                    }

                    // Manual execution is only allowed for triggers with ExecuteTrigger filters
                    // and only if (a) the event matches the filter and (b) the caller is authorized.
                    let mut filter_event = event.clone();
                    if perm_ok_global {
                        filter_event.authority = action.authority().clone();
                    }
                    match action.clone_and_box().filter {
                        EventFilterBox::ExecuteTrigger(filter) => {
                            // Check filter match
                            if !filter.matches(&filter_event) {
                                return Err(Error::InvariantViolation(
                                    "Trigger can't be executed manually: filter mismatch".into(),
                                ));
                            }

                            // Authorization: owner of the trigger action OR explicit permission
                            let owner_ok = action.authority() == authority;
                            if owner_ok || perm_ok_global {
                                Ok(())
                            } else {
                                Err(Error::InvariantViolation(
                                    "Trigger can't be executed manually: not permitted".into(),
                                ))
                            }
                        }
                        _ => Err(Error::InvariantViolation(
                            "Trigger can't be executed manually: not a by-call trigger".into(),
                        )),
                    }
                })
                .ok_or_else(|| Error::Find(FindError::Trigger(id.clone())))
                .and_then(core::convert::identity)?;

            // ExecuteTrigger remains supported; step output is intentionally dropped here
            // because trigger completion is already surfaced via events.
            let _step = state_transaction
                .execute_called_trigger(id, &event)
                .map_err(|rej| {
                    iroha_logger::debug!(
                        ?rej,
                        trigger_id = %id,
                        authority = %authority,
                        "execute_called_trigger rejected"
                    );
                    match rej {
                        TransactionRejectionReason::Validation(vf) => {
                            iroha_logger::debug!(
                                ?vf,
                                trigger_id = %id,
                                authority = %authority,
                                "validation failure while executing trigger"
                            );
                            match vf {
                                ValidationFail::InstructionFailed(ie) => {
                                    iroha_logger::debug!(
                                        ?ie,
                                        trigger_id = %id,
                                        authority = %authority,
                                        "trigger execution returned instruction error"
                                    );
                                    ie
                                }
                                ValidationFail::NotPermitted(msg) => {
                                    iroha_logger::debug!(
                                        %msg,
                                        trigger_id = %id,
                                        authority = %authority,
                                        "trigger execution not permitted"
                                    );
                                    Error::InvalidParameter(InvalidParameterError::SmartContract(
                                        msg,
                                    ))
                                }
                                other => {
                                    iroha_logger::debug!(
                                        ?other,
                                        trigger_id = %id,
                                        authority = %authority,
                                        "trigger validation produced unexpected failure"
                                    );
                                    Error::Conversion(format!("Validation failed: {other}"))
                                }
                            }
                        }
                        TransactionRejectionReason::InstructionExecution(fail) => {
                            iroha_logger::debug!(
                                ?fail,
                                trigger_id = %id,
                                authority = %authority,
                                "trigger rejected due to instruction execution fail"
                            );
                            Error::Conversion(format!("Instruction execution: {fail}"))
                        }
                        TransactionRejectionReason::IvmExecution(err) => {
                            iroha_logger::debug!(
                                ?err,
                                trigger_id = %id,
                                authority = %authority,
                                "trigger rejected due to IVM execution error"
                            );
                            Error::Conversion(format!("IVM execution: {err}"))
                        }
                        TransactionRejectionReason::TriggerExecution(err) => {
                            iroha_logger::debug!(
                                ?err,
                                trigger_id = %id,
                                authority = %authority,
                                "trigger rejected while executing nested trigger"
                            );
                            Error::Conversion(format!("Trigger execution: {err:?}"))
                        }
                        TransactionRejectionReason::AccountDoesNotExist(err) => {
                            iroha_logger::debug!(
                                ?err,
                                trigger_id = %id,
                                authority = %authority,
                                "trigger rejected because account was missing"
                            );
                            Error::Find(err)
                        }
                        TransactionRejectionReason::LimitCheck(err) => {
                            iroha_logger::debug!(
                                ?err,
                                trigger_id = %id,
                                authority = %authority,
                                "trigger rejected due to limit check"
                            );
                            Error::Conversion(format!("Limit check: {err}"))
                        }
                    }
                })?;

            Ok(())
        }
    }

    /// Enforce a maximum JSON size for metadata values.
    // centralized in smartcontracts::limits
    /// Enforce a maximum JSON size for instruction payloads.
    fn enforce_payload_limit(
        state_transaction: &mut StateTransaction<'_, '_>,
        value: &Json,
    ) -> Result<(), Error> {
        const DEFAULT_MAX: usize = 1_048_576; // 1 MiB
        const PARAM_NAME: &str = "max_instruction_payload_bytes";

        let params = state_transaction.world.parameters.get();
        let limit = if let Ok(name) = core::str::FromStr::from_str(PARAM_NAME)
            && let Some(custom) = params
                .custom()
                .get(&iroha_data_model::parameter::CustomParameterId(name))
            && let Ok(num) = custom.payload().try_into_any_norito::<u64>()
        {
            usize::try_from(num).unwrap_or(usize::MAX)
        } else {
            DEFAULT_MAX
        };
        if value.as_ref().len() > limit {
            return Err(Error::InvalidParameter(
                InvalidParameterError::SmartContract(format!(
                    "Instruction payload too large: {} > {} bytes",
                    value.as_ref().len(),
                    limit
                )),
            ));
        }
        Ok(())
    }
}

pub mod query {
    //! Queries associated to triggers.
    use iroha_data_model::{
        query::{
            dsl::{CompoundPredicate, EvaluatePredicate},
            error::QueryExecutionFail as Error,
            trigger::FindTriggers,
        },
        trigger::{Trigger, TriggerId},
    };

    use super::*;
    use crate::{
        prelude::*,
        smartcontracts::{ValidQuery, triggers::set::SetReadOnly},
        state::StateReadOnly,
    };

    impl ValidQuery for FindActiveTriggerIds {
        #[metrics(+"find_active_triggers")]
        fn execute(
            self,
            filter: CompoundPredicate<TriggerId>,
            state_ro: &impl StateReadOnly,
        ) -> Result<impl Iterator<Item = TriggerId>, Error> {
            let triggers = state_ro.world().triggers();
            let iter: Box<dyn Iterator<Item = TriggerId> + '_> =
                Box::new(triggers.active_trigger_ids_iter().cloned());
            if filter.json_payload().is_none() {
                return Ok(iter);
            }

            // Only report triggers that are actually active (i.e., have
            // remaining executions). This makes the query resilient to any
            // edge case where a depleted trigger could temporarily remain
            // registered during a transaction before being pruned.
            Ok(Box::new(iter.filter(move |id| filter.applies(id))))
        }
    }

    impl ValidQuery for FindTriggers {
        #[metrics(+"find_triggers")]
        fn execute(
            self,
            filter: CompoundPredicate<Trigger>,
            state_ro: &impl StateReadOnly,
        ) -> Result<impl Iterator<Item = Self::Item>, Error> {
            let triggers = state_ro.world().triggers();

            let iter: Box<dyn Iterator<Item = Trigger> + '_> = Box::new(triggers.triggers_iter());
            if filter.json_payload().is_none() {
                return Ok(iter);
            }

            Ok(Box::new(
                iter.filter(move |trigger| filter.applies(trigger)),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;
    use std::{str::FromStr, time::Duration};

    use iroha_data_model::{
        block::BlockHeader,
        events::time::Schedule,
        isi::error::{InstructionExecutionError, InvalidParameterError},
        name::Name,
        parameter::{CustomParameter, CustomParameterId, Parameter},
        permission::Permission,
        role::{Role, RoleId},
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use mv::storage::StorageReadOnly;

    use super::*;
    use crate::{
        block::ValidBlock,
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::{Error, Execute, ValidQuery},
        state::{State, World},
        sumeragi::network_topology::Topology,
    };

    fn new_dummy_block() -> crate::block::CommittedBlock {
        let (leader_public_key, leader_private_key) = iroha_crypto::KeyPair::random().into_parts();
        let peer_id = crate::PeerId::new(leader_public_key);
        let topology = Topology::new(vec![peer_id]);
        ValidBlock::new_dummy_and_modify_header(&leader_private_key, |h| {
            h.set_height(NonZeroU64::new(1).unwrap());
        })
        .commit(&topology)
        .unpack(|_| {})
        .unwrap()
    }

    /// Fetch active trigger ids using a short-lived [`State::view`] guard so the caller
    /// can immediately take new mutable block scopes without deadlocking on the view lock.
    fn collect_active_trigger_ids(state: &State) -> Vec<TriggerId> {
        use iroha_data_model::query::dsl::CompoundPredicate;

        let view = state.view();
        let ids: Vec<_> = ValidQuery::execute(FindActiveTriggerIds, CompoundPredicate::PASS, &view)
            .expect("active trigger query should succeed")
            .collect();
        drop(view);
        ids
    }

    // No unit test for the cache helpers here; those are exercised indirectly
    // via existing trigger authorization tests.

    #[test]
    fn trigger_is_enabled_reads_metadata_flag() {
        let mut metadata = Metadata::default();
        assert!(
            trigger_is_enabled(&metadata),
            "missing flag defaults to enabled"
        );

        let key = TRIGGER_ENABLED_METADATA_KEY
            .parse::<Name>()
            .expect("valid metadata key");
        metadata.insert(key.clone(), Json::from(false));
        assert!(!trigger_is_enabled(&metadata), "false disables trigger");

        metadata.insert(key.clone(), Json::from(0_u64));
        assert!(!trigger_is_enabled(&metadata), "zero disables trigger");

        metadata.insert(key, Json::from(true));
        assert!(trigger_is_enabled(&metadata), "true enables trigger");
    }

    #[test]
    fn execute_trigger_requires_owner_or_permission() {
        use iroha_data_model::events::execute_trigger::ExecuteTriggerEventFilter;
        use iroha_executor_data_model::permission::trigger::CanExecuteTrigger;
        use iroha_test_samples::{ALICE_ID, BOB_ID};

        // Build state
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);

        // Prepare a block and state transaction
        let mut state_block = state.block(BlockHeader::new(
            NonZeroU64::new(1).unwrap(),
            None,
            None,
            None,
            0,
            0,
        ));
        let mut stx = state_block.transaction();
        stx._curr_block
            .set_height(NonZeroU64::new(2).expect("nonzero"));

        // Create domain and accounts
        let domain_id: DomainId = "wonderland".parse().unwrap();
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        Register::account(Account::new(
            ALICE_ID.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        Register::account(Account::new(
            BOB_ID.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        // Register a by-call trigger owned by Alice
        let trig_id: TriggerId = "bycall_authz_test".parse().unwrap();
        let trig = Trigger::new(
            trig_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Exactly(1),
                ALICE_ID.clone(),
                ExecuteTriggerEventFilter::new().for_trigger(trig_id.clone()),
            ),
        );
        Register::trigger(trig)
            .execute(&ALICE_ID, &mut stx)
            .unwrap();

        // Bob cannot execute without permission
        let exec = ExecuteTrigger::new(trig_id.clone());
        assert!(exec.execute(&BOB_ID, &mut stx).is_err());

        // Grant Bob an explicit permission for this trigger
        let perm: iroha_data_model::permission::Permission = CanExecuteTrigger {
            trigger: trig_id.clone(),
        }
        .into();
        Grant::account_permission(perm, BOB_ID.clone())
            .execute(&ALICE_ID, &mut stx)
            .unwrap();

        // Commit the permission grant and execute in the next block
        stx.apply();
        state_block.commit().unwrap();

        let mut state_block2 = state.block(BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            None,
            None,
            None,
            0,
            0,
        ));
        let mut stx2 = state_block2.transaction();
        let exec = ExecuteTrigger::new(trig_id);
        exec.execute(&BOB_ID, &mut stx2)
            .expect("bob should be permitted to execute trigger");
    }

    #[test]
    fn unregister_trigger_removes_associated_permissions_from_accounts_and_roles() {
        use iroha_data_model::events::execute_trigger::ExecuteTriggerEventFilter;
        use iroha_executor_data_model::permission::trigger::CanExecuteTrigger;

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);

        let mut state_block = state.block(BlockHeader::new(
            NonZeroU64::new(1).expect("nonzero"),
            None,
            None,
            None,
            0,
            0,
        ));
        let mut stx = state_block.transaction();
        stx._curr_block
            .set_height(NonZeroU64::new(2).expect("nonzero"));

        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register domain");
        Register::account(Account::new(
            ALICE_ID.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .expect("register alice");
        Register::account(Account::new(
            BOB_ID.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .expect("register bob");
        let bob_id = (*BOB_ID).clone();

        let trig_id: TriggerId = "perm_cleanup".parse().expect("trigger id");
        let trig = Trigger::new(
            trig_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Exactly(1),
                ALICE_ID.clone(),
                ExecuteTriggerEventFilter::new().for_trigger(trig_id.clone()),
            ),
        );
        Register::trigger(trig)
            .execute(&ALICE_ID, &mut stx)
            .expect("register trigger");

        let permission: Permission = CanExecuteTrigger {
            trigger: trig_id.clone(),
        }
        .into();
        Grant::account_permission(permission.clone(), bob_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("grant account permission");

        let role_id: RoleId = "TRIGGER_CLEANUP".parse().expect("role id");
        Register::role(Role::new(role_id.clone(), bob_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register role");
        Grant::role_permission(permission.clone(), role_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("grant role permission");

        assert!(
            stx.world
                .account_permissions
                .get(&bob_id)
                .is_some_and(|perms| perms.contains(&permission)),
            "bob should have direct permission before unregister"
        );
        let role = stx.world.roles.get(&role_id).expect("role should exist");
        assert!(
            role.permissions().any(|perm| perm == &permission),
            "role should include permission before unregister"
        );

        Unregister::trigger(trig_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("unregister trigger");

        assert!(
            !stx.world
                .account_permissions
                .get(&bob_id)
                .is_some_and(|perms| perms.contains(&permission)),
            "bob direct permission should be removed"
        );
        let role = stx.world.roles.get(&role_id).expect("role should exist");
        assert!(
            !role.permissions().any(|perm| perm == &permission),
            "role permission should be removed"
        );
        assert!(
            !role.permission_epochs().contains_key(&permission),
            "permission epoch should be pruned"
        );
    }

    #[test]
    fn by_call_trigger_is_pruned_after_manual_execution() {
        use iroha_data_model::events::execute_trigger::ExecuteTriggerEventFilter;
        use iroha_logger::Level;

        // Build state
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);

        // Prepare a block and state transaction
        let mut state_block = state.block(BlockHeader::new(
            NonZeroU64::new(1).unwrap(),
            None,
            None,
            None,
            0,
            0,
        ));
        let mut stx = state_block.transaction();
        stx._curr_block
            .set_height(NonZeroU64::new(2).expect("nonzero"));

        // Create domain and account
        let domain_id: DomainId = "wonderland".parse().unwrap();
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        Register::account(Account::new(
            ALICE_ID.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        // Register a by-call trigger that may run exactly once
        let trig_id: TriggerId = "manual_once".parse().unwrap();
        let trig = Trigger::new(
            trig_id.clone(),
            Action::new(
                vec![InstructionBox::from(Log::new(Level::INFO, "ping".into()))],
                Repeats::Exactly(1),
                ALICE_ID.clone(),
                ExecuteTriggerEventFilter::new().for_trigger(trig_id.clone()),
            ),
        );
        Register::trigger(trig)
            .execute(&ALICE_ID, &mut stx)
            .unwrap();

        // First execution should succeed.
        ExecuteTrigger::new(trig_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .unwrap();

        // Subsequent execution should report the trigger as missing/depleted.
        let err = ExecuteTrigger::new(trig_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect_err("depleted by-call trigger must not run twice");
        assert!(matches!(err, Error::Find(FindError::Trigger(id)) if id == trig_id));
    }

    #[test]
    fn register_trigger_without_permission_is_rejected() {
        // Build state
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);

        let block = new_dummy_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx._curr_block
            .set_height(NonZeroU64::new(2).expect("nonzero"));

        let domain_id: DomainId = "wonderland".parse().unwrap();
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        Register::account(Account::new(
            ALICE_ID.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        Register::account(Account::new(
            BOB_ID.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        let trig_id: TriggerId = "reg_trigger_denied".parse().unwrap();
        let trig = Trigger::new(
            trig_id,
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Indefinitely,
                ALICE_ID.clone(),
                ExecuteTriggerEventFilter::new(),
            ),
        );

        let err = Register::trigger(trig)
            .execute(&BOB_ID, &mut stx)
            .expect_err("register should fail without permission");

        match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                msg,
            )) => {
                assert!(msg.contains("CanRegisterTrigger"));
                let expected = ALICE_ID.clone().to_string();
                assert!(msg.contains(&expected));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn register_trigger_with_permission_succeeds() {
        use iroha_data_model::permission::Permission;
        use iroha_primitives::json::Json;

        // Build state
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);

        let block = new_dummy_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let domain_id: DomainId = "wonderland".parse().unwrap();
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        Register::account(Account::new(
            ALICE_ID.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        Register::account(Account::new(
            BOB_ID.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        let trig_id: TriggerId = "reg_trigger_allowed".parse().unwrap();
        let trig = Trigger::new(
            trig_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Indefinitely,
                ALICE_ID.clone(),
                ExecuteTriggerEventFilter::new(),
            ),
        );

        let payload = norito::json::object([("authority", ALICE_ID.to_string())])
            .expect("serialize permission payload");
        let perm = Permission::new("CanRegisterTrigger".into(), Json::new(payload));
        Grant::account_permission(perm, BOB_ID.clone())
            .execute(&ALICE_ID, &mut stx)
            .unwrap();

        Register::trigger(trig)
            .execute(&BOB_ID, &mut stx)
            .expect("register should succeed with permission");
    }

    #[test]
    fn active_trigger_ids_excludes_depleted_after_burn() {
        // Build state
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);

        // Bootstrap world with a domain and Alice account
        let mut state_block = state.block(BlockHeader::new(
            NonZeroU64::new(1).unwrap(),
            None,
            None,
            None,
            0,
            0,
        ));
        let mut stx = state_block.transaction();
        let domain_id: DomainId = "wonderland".parse().unwrap();
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        Register::account(Account::new(
            ALICE_ID.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        // Register a by-call trigger with Exactly(1) repeat
        let trig_id: TriggerId = "utrig_burn_rm".parse().unwrap();
        let trig = Trigger::new(
            trig_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Exactly(1),
                ALICE_ID.clone(),
                ExecuteTriggerEventFilter::new().for_trigger(trig_id.clone()),
            ),
        );
        Register::trigger(trig)
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        stx.apply();
        state_block.commit().unwrap();

        // Active IDs should include the trigger id
        {
            let mut ids = collect_active_trigger_ids(&state);
            ids.sort();
            assert!(ids.iter().any(|id| id == &trig_id));
        }

        // Burn to zero in the next block
        let mut state_block2 = state.block(BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            None,
            None,
            None,
            0,
            0,
        ));
        let mut stx2 = state_block2.transaction();
        Burn::trigger_repetitions(1, trig_id.clone())
            .execute(&ALICE_ID, &mut stx2)
            .unwrap();
        stx2.apply();
        state_block2.commit().unwrap();

        // Active IDs should no longer include the trigger id
        {
            let ids2 = collect_active_trigger_ids(&state);
            assert!(ids2.iter().all(|id| id != &trig_id));
        }
    }

    #[test]
    fn find_triggers_returns_registered_triggers_for_pass_predicate() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);

        let mut state_block = state.block(BlockHeader::new(
            NonZeroU64::new(1).unwrap(),
            None,
            None,
            None,
            0,
            0,
        ));
        let mut stx = state_block.transaction();
        let domain_id: DomainId = "wonderland".parse().unwrap();
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        Register::account(Account::new(
            ALICE_ID.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        let rose_id: TriggerId = "utrig_rose".parse().unwrap();
        let tulip_id: TriggerId = "utrig_tulip".parse().unwrap();
        for trigger_id in [&rose_id, &tulip_id] {
            let trigger = Trigger::new(
                trigger_id.clone(),
                Action::new(
                    Vec::<InstructionBox>::new(),
                    Repeats::Indefinitely,
                    ALICE_ID.clone(),
                    ExecuteTriggerEventFilter::new().for_trigger(trigger_id.clone()),
                ),
            );
            Register::trigger(trigger)
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
        }
        stx.apply();
        state_block.commit().unwrap();

        let view = state.view();
        let mut ids: Vec<_> = ValidQuery::execute(
            FindTriggers,
            iroha_data_model::query::dsl::CompoundPredicate::PASS,
            &view,
        )
        .expect("trigger query should succeed")
        .map(|trigger| trigger.id().clone())
        .collect();
        ids.sort();

        assert_eq!(ids, vec![rose_id, tulip_id]);
    }

    #[test]
    fn reserved_trigger_metadata_key_is_rejected() {
        let key: Name = "__registered_block_height".parse().expect("valid name");
        let err = isi::ensure_metadata_key_is_not_reserved(&key).expect_err("key must be rejected");
        assert!(
            matches!(
                err,
                Error::InvalidParameter(InvalidParameterError::SmartContract(_))
            ),
            "unexpected error variant: {err:?}"
        );
    }

    #[test]
    fn custom_trigger_metadata_key_is_allowed() {
        let key: Name = "custom_meta".parse().expect("valid name");
        isi::ensure_metadata_key_is_not_reserved(&key).expect("custom keys should be allowed");
    }

    #[test]
    fn time_trigger_with_zero_period_is_rejected() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);

        let block = new_dummy_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let domain_id: DomainId = "wonderland".parse().unwrap();
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        Register::account(Account::new(
            ALICE_ID.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        let trig_id: TriggerId = "time_zero_period".parse().unwrap();
        let schedule = ExecutionTime::Schedule(
            Schedule::starting_at(Duration::from_secs(0)).with_period(Duration::from_secs(0)),
        );
        let trigger = Trigger::new(
            trig_id,
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Exactly(1),
                ALICE_ID.clone(),
                TimeEventFilter(schedule),
            ),
        );

        let err = Register::trigger(trigger)
            .execute(&ALICE_ID, &mut stx)
            .expect_err("zero-period time trigger must be rejected");

        match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                msg,
            )) => assert!(
                msg.contains("period must be greater than zero"),
                "unexpected message: {msg}"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn trigger_registration_enforces_metadata_limit() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);

        let block = new_dummy_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let domain_id: DomainId = "wonderland".parse().unwrap();
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        Register::account(Account::new(
            ALICE_ID.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        let id = CustomParameterId::from_str("max_metadata_value_bytes").unwrap();
        let param = Parameter::Custom(CustomParameter::new(id, Json::new(4u32)));
        stx.world.parameters.get_mut().set_parameter(param);

        let mut metadata = Metadata::default();
        metadata.insert("oversized".parse().unwrap(), Json::new("0123456789"));

        let trigger = Trigger::new(
            "meta_limit".parse().unwrap(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Exactly(1),
                ALICE_ID.clone(),
                ExecuteTriggerEventFilter::new(),
            )
            .with_metadata(metadata),
        );

        let err = Register::trigger(trigger)
            .execute(&ALICE_ID, &mut stx)
            .expect_err("metadata over limit must be rejected");

        match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                msg,
            )) => assert!(
                msg.contains("max_metadata_value_bytes"),
                "unexpected message: {msg}"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
