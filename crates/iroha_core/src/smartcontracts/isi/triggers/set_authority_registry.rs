//! Trigger-owned classification; private stores never escape their owner.

use super::*;
use crate::state::authority_registry::{
    Canonical, DerivationCheck, Field, Role, Schema, V1_LAYOUT, classified_owner, schema,
};

classified_owner!(Set, check_trigger_fields, AUTHORITY_FIELDS, {
    data_triggers: Storage<TriggerId, LoadedAction<DataEventFilter>> => ("triggers.data",
        Role::Canonical(Canonical::Table { key: schema::<TriggerId>(), value: Schema::Semantic {
            identity: "iroha:state:trigger-data-action:v1", encoder: "set::BorrowedWorldAction<DataEventFilter>; hash_world_action", layout: V1_LAYOUT,
        } }));
    pipeline_triggers: Storage<TriggerId, LoadedAction<PipelineEventFilterBox>> => ("triggers.pipeline",
        Role::Canonical(Canonical::Table { key: schema::<TriggerId>(), value: Schema::Semantic {
            identity: "iroha:state:trigger-pipeline-action:v1", encoder: "set::BorrowedWorldAction<PipelineEventFilterBox>; hash_world_action", layout: V1_LAYOUT,
        } }));
    time_triggers: Storage<TriggerId, LoadedAction<TimeEventFilter>> => ("triggers.time",
        Role::Canonical(Canonical::Table { key: schema::<TriggerId>(), value: Schema::Semantic {
            identity: "iroha:state:trigger-time-action:v1", encoder: "set::BorrowedWorldAction<TimeEventFilter>; hash_world_action includes retry policy/state", layout: V1_LAYOUT,
        } }));
    by_call_triggers: Storage<TriggerId, LoadedAction<ExecuteTriggerEventFilter>> => ("triggers.by_call",
        Role::Canonical(Canonical::Table { key: schema::<TriggerId>(), value: Schema::Semantic {
            identity: "iroha:state:trigger-by-call-action:v1", encoder: "set::BorrowedWorldAction<ExecuteTriggerEventFilter>; hash_world_action", layout: V1_LAYOUT,
        } }));
    ids: Storage<TriggerId, TriggeringEventType> => ("triggers.ids",
        Role::Derived { sources: &["triggers.data", "triggers.pipeline", "triggers.time", "triggers.by_call"], check: DerivationCheck::Rebuild("set::load_trigger_entries reconstructs unique type-tagged IDs; SetDto recovery validates ids_raw") });
    active_data_trigger_ids: ActiveTriggerIdStore => ("triggers.active_data",
        Role::Derived { sources: &["triggers.data"], check: DerivationCheck::Rebuild("Set::collect_active_ids; repeats and enabled metadata, current and predecessor history") });
    active_pipeline_trigger_ids: ActiveTriggerIdStore => ("triggers.active_pipeline",
        Role::Derived { sources: &["triggers.pipeline"], check: DerivationCheck::Rebuild("Set::collect_active_ids; repeats and enabled metadata, current and predecessor history") });
    active_time_trigger_ids: ActiveTriggerIdStore => ("triggers.active_time",
        Role::Derived { sources: &["triggers.time"], check: DerivationCheck::Rebuild("Set::collect_active_ids; repeats and enabled metadata, current and predecessor history") });
    active_by_call_trigger_ids: ActiveTriggerIdStore => ("triggers.active_by_call",
        Role::Derived { sources: &["triggers.by_call"], check: DerivationCheck::Rebuild("Set::collect_active_ids; repeats and enabled metadata, current and predecessor history") });
    contracts: TriggerContractStore => ("triggers.contracts",
        Role::Canonical(Canonical::Table { key: schema::<HashOf<IvmBytecode>>(), value: Schema::Semantic {
            identity: "iroha:state:trigger-contract-bytecode:v1",
            encoder: "set::BorrowedWorldContract; hash_world_contract; SetBlock::validate_world_contract_rows checks key, code_hash and four-store count",
            layout: V1_LAYOUT,
        } }));
});
