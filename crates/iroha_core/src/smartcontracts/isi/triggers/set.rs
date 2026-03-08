//! Trigger logic. Instead of defining a Trigger as an entity, we
//! provide a collection of triggers as the smallest unit, which is an
//! idea borrowed from lisp hooks.
//!
//! The point of the idea is to create an ordering (or hash function)
//! which maps the event filter and the event that triggers it to the
//! same approximate location in the hierarchy, thus using Binary
//! search trees (common lisp) or hash tables (racket) to quickly
//! trigger hooks.

use core::cmp::min;
use std::{collections::BTreeMap, fmt, num::NonZeroU64};

use iroha_crypto::HashOf;
use iroha_data_model::{
    events::EventFilter,
    isi::error::{InstructionExecutionError, MathError},
    prelude::*,
    query::error::FindError,
    transaction::IvmBytecode,
    trigger::action::EnsureTriggerAuthority,
};
use iroha_logger::prelude::*;
use iroha_primitives::const_vec::ConstVec;
use ivm::VMError;
use mv::storage::{
    Block as StorageBlock, Storage, StorageReadOnly, Transaction as StorageTransaction,
    View as StorageView,
};
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json;
#[cfg(feature = "json")]
use norito::json::{FastJsonWrite, JsonSerialize as JsonSerializeTrait};
use thiserror::Error;

use super::trigger_is_enabled;
use crate::smartcontracts::isi::triggers::specialized::{
    LoadedAction, LoadedActionTrait, SpecializedAction, SpecializedTrigger,
};

/// Error type for [`Set`] operations.
#[derive(Debug, Error, displaydoc::Display)]
pub enum Error {
    /// Failed to preload IVM trigger
    Preload(#[from] VMError),
}

/// Result type for [`Set`] operations.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// [`IvmBytecode`]s keyed by contract hash.
/// Stored together with usage counts so triggers sharing the same blob can be deduplicated.
type TriggerContractStore = Storage<HashOf<IvmBytecode>, IvmBytecodeEntry>;
type TriggerContractStoreBlock<'set> = StorageBlock<'set, HashOf<IvmBytecode>, IvmBytecodeEntry>;
type TriggerContractStoreTransaction<'block, 'set> =
    StorageTransaction<'block, 'set, HashOf<IvmBytecode>, IvmBytecodeEntry>;
type TriggerContractStoreView<'set> = StorageView<'set, HashOf<IvmBytecode>, IvmBytecodeEntry>;

/// Specialized structure that maps event filters to Triggers.
// NB: `Set` has custom `Serialize` and `DeserializeSeed` implementations
// which need to be manually updated when changing the struct
#[derive(Default)]
pub struct Set {
    /// Triggers using [`DataEventFilter`]
    data_triggers: Storage<TriggerId, LoadedAction<DataEventFilter>>,
    /// Triggers using [`PipelineEventFilterBox`]
    pipeline_triggers: Storage<TriggerId, LoadedAction<PipelineEventFilterBox>>,
    /// Triggers using [`TimeEventFilter`]
    time_triggers: Storage<TriggerId, LoadedAction<TimeEventFilter>>,
    /// Triggers using [`ExecuteTriggerEventFilter`]
    by_call_triggers: Storage<TriggerId, LoadedAction<ExecuteTriggerEventFilter>>,
    /// Trigger ids with type of events they process
    ids: Storage<TriggerId, TriggeringEventType>,
    /// [`IvmBytecode`]s map by contract blob hash.
    /// This map serves multiple purposes:
    /// 1. Querying original contract blob of trigger
    /// 2. Deduplicating triggers with the same contract blob
    contracts: TriggerContractStore,
}

impl Set {}

impl json::JsonDeserialize for Set {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let mut visitor = json::MapVisitor::new(parser)?;
        let mut data_triggers: Option<Storage<TriggerId, LoadedAction<DataEventFilter>>> = None;
        let mut pipeline_triggers: Option<
            Storage<TriggerId, LoadedAction<PipelineEventFilterBox>>,
        > = None;
        let mut time_triggers: Option<Storage<TriggerId, LoadedAction<TimeEventFilter>>> = None;
        let mut by_call_triggers: Option<
            Storage<TriggerId, LoadedAction<ExecuteTriggerEventFilter>>,
        > = None;
        let mut ids: Option<Storage<TriggerId, TriggeringEventType>> = None;
        let mut contracts: Option<Storage<HashOf<IvmBytecode>, IvmBytecodeEntry>> = None;

        while let Some(key) = visitor.next_key()? {
            match key.as_str() {
                "data_triggers" => {
                    data_triggers = Some(visitor.parse_value()?);
                }
                "pipeline_triggers" => {
                    pipeline_triggers = Some(visitor.parse_value()?);
                }
                "time_triggers" => {
                    time_triggers = Some(visitor.parse_value()?);
                }
                "by_call_triggers" => {
                    by_call_triggers = Some(visitor.parse_value()?);
                }
                "ids" => {
                    ids = Some(visitor.parse_value()?);
                }
                "contracts" => {
                    contracts = Some(visitor.parse_value()?);
                }
                other => {
                    visitor.skip_value()?;
                    trace!(%other, "ignoring unknown trigger set field");
                }
            }
        }

        visitor.finish()?;

        let data_triggers =
            data_triggers.ok_or_else(|| json::MapVisitor::missing_field("data_triggers"))?;
        let pipeline_triggers = pipeline_triggers
            .ok_or_else(|| json::MapVisitor::missing_field("pipeline_triggers"))?;
        let time_triggers =
            time_triggers.ok_or_else(|| json::MapVisitor::missing_field("time_triggers"))?;
        let by_call_triggers =
            by_call_triggers.ok_or_else(|| json::MapVisitor::missing_field("by_call_triggers"))?;
        let ids = ids.ok_or_else(|| json::MapVisitor::missing_field("ids"))?;
        let contracts = contracts.ok_or_else(|| json::MapVisitor::missing_field("contracts"))?;

        Ok(Self {
            data_triggers,
            pipeline_triggers,
            time_triggers,
            by_call_triggers,
            ids,
            contracts,
        })
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for Set {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        let mut write = |name: &str, serialize: &dyn Fn(&mut String)| {
            if first {
                first = false;
            } else {
                out.push(',');
            }
            norito::json::write_json_string(name, out);
            out.push(':');
            serialize(out);
        };
        write("data_triggers", &|out| {
            JsonSerializeTrait::json_serialize(&self.data_triggers, out);
        });
        write("pipeline_triggers", &|out| {
            JsonSerializeTrait::json_serialize(&self.pipeline_triggers, out);
        });
        write("time_triggers", &|out| {
            JsonSerializeTrait::json_serialize(&self.time_triggers, out);
        });
        write("by_call_triggers", &|out| {
            JsonSerializeTrait::json_serialize(&self.by_call_triggers, out);
        });
        write("ids", &|out| {
            JsonSerializeTrait::json_serialize(&self.ids, out)
        });
        write("contracts", &|out| {
            JsonSerializeTrait::json_serialize(&self.contracts, out)
        });
        out.push('}');
    }
}

/// Trigger set for block's aggregated changes
pub struct SetBlock<'set> {
    /// Triggers using [`DataEventFilter`]
    data_triggers: StorageBlock<'set, TriggerId, LoadedAction<DataEventFilter>>,
    /// Triggers using [`PipelineEventFilterBox`]
    pipeline_triggers: StorageBlock<'set, TriggerId, LoadedAction<PipelineEventFilterBox>>,
    /// Triggers using [`TimeEventFilter`]
    time_triggers: StorageBlock<'set, TriggerId, LoadedAction<TimeEventFilter>>,
    /// Triggers using [`ExecuteTriggerEventFilter`]
    by_call_triggers: StorageBlock<'set, TriggerId, LoadedAction<ExecuteTriggerEventFilter>>,
    /// Trigger ids with type of events they process
    ids: StorageBlock<'set, TriggerId, TriggeringEventType>,
    /// Original [`IvmBytecode`]s by [`TriggerId`] for querying purposes.
    contracts: TriggerContractStoreBlock<'set>,
}

/// Trigger set for transaction's aggregated changes
pub struct SetTransaction<'block, 'set> {
    /// Triggers using [`DataEventFilter`]
    data_triggers: StorageTransaction<'block, 'set, TriggerId, LoadedAction<DataEventFilter>>,
    /// Triggers using [`PipelineEventFilterBox`]
    pipeline_triggers:
        StorageTransaction<'block, 'set, TriggerId, LoadedAction<PipelineEventFilterBox>>,
    /// Triggers using [`TimeEventFilter`]
    time_triggers: StorageTransaction<'block, 'set, TriggerId, LoadedAction<TimeEventFilter>>,
    /// Triggers using [`ExecuteTriggerEventFilter`]
    by_call_triggers:
        StorageTransaction<'block, 'set, TriggerId, LoadedAction<ExecuteTriggerEventFilter>>,
    /// Trigger ids with type of events they process
    ids: StorageTransaction<'block, 'set, TriggerId, TriggeringEventType>,
    /// Original [`IvmBytecode`]s by [`TriggerId`] for querying purposes.
    contracts: TriggerContractStoreTransaction<'block, 'set>,
}

/// Consistent point in time view of the [`Set`]
pub struct SetView<'set> {
    /// Triggers using [`DataEventFilter`]
    data_triggers: StorageView<'set, TriggerId, LoadedAction<DataEventFilter>>,
    /// Triggers using [`PipelineEventFilterBox`]
    pipeline_triggers: StorageView<'set, TriggerId, LoadedAction<PipelineEventFilterBox>>,
    /// Triggers using [`TimeEventFilter`]
    time_triggers: StorageView<'set, TriggerId, LoadedAction<TimeEventFilter>>,
    /// Triggers using [`ExecuteTriggerEventFilter`]
    by_call_triggers: StorageView<'set, TriggerId, LoadedAction<ExecuteTriggerEventFilter>>,
    /// Trigger ids with type of events they process
    ids: StorageView<'set, TriggerId, TriggeringEventType>,
    /// Original [`IvmBytecode`]s by [`TriggerId`] for querying purposes.
    contracts: TriggerContractStoreView<'set>,
}

/// Entry in smart-contracts map
#[cfg_attr(feature = "json", derive(norito::derive::FastJsonWrite))]
#[derive(Debug, Clone)]
pub struct IvmBytecodeEntry {
    /// Original smart contract binary blob
    original_contract: IvmBytecode,
    /// Number of times this contract is used
    count: NonZeroU64,
}

impl json::JsonDeserialize for IvmBytecodeEntry {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let mut visitor = json::MapVisitor::new(parser)?;
        let mut original_contract: Option<IvmBytecode> = None;
        let mut count: Option<u64> = None;

        while let Some(key) = visitor.next_key()? {
            match key.as_str() {
                "original_contract" => {
                    original_contract = Some(visitor.parse_value()?);
                }
                "count" => {
                    count = Some(visitor.parse_value()?);
                }
                other => {
                    visitor.skip_value()?;
                    iroha_logger::warn!(
                        "unknown field `{other}` while decoding IVM bytecode entry; skipping"
                    );
                }
            }
        }

        visitor.finish()?;

        let original_contract = original_contract
            .ok_or_else(|| json::MapVisitor::missing_field("original_contract"))?;
        let raw_count = count.ok_or_else(|| json::MapVisitor::missing_field("count"))?;
        let count = NonZeroU64::new(raw_count).ok_or_else(|| json::Error::InvalidField {
            field: "count".into(),
            message: "must be non-zero".into(),
        })?;

        Ok(Self {
            original_contract,
            count,
        })
    }
}

// Norito DTOs for Set serialization

#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
enum ExecutableRefDto {
    Ivm(HashOf<IvmBytecode>),
    Instructions(ConstVec<InstructionBox>),
}

impl From<&ExecutableRef> for ExecutableRefDto {
    fn from(r: &ExecutableRef) -> Self {
        match r {
            ExecutableRef::Ivm(h) => ExecutableRefDto::Ivm(*h),
            ExecutableRef::Instructions(v) => ExecutableRefDto::Instructions(v.clone()),
        }
    }
}

impl TryFrom<ExecutableRefDto> for ExecutableRef {
    type Error = String;
    fn try_from(dto: ExecutableRefDto) -> Result<Self, Self::Error> {
        Ok(match dto {
            ExecutableRefDto::Ivm(h) => ExecutableRef::Ivm(h),
            ExecutableRefDto::Instructions(v) => ExecutableRef::Instructions(v),
        })
    }
}

#[derive(Encode, Decode)]
struct IvmBytecodeEntryDto {
    original_contract: IvmBytecode,
    count: u64,
}

impl From<&IvmBytecodeEntry> for IvmBytecodeEntryDto {
    fn from(e: &IvmBytecodeEntry) -> Self {
        IvmBytecodeEntryDto {
            original_contract: e.original_contract.clone(),
            count: e.count.get(),
        }
    }
}

impl TryFrom<IvmBytecodeEntryDto> for IvmBytecodeEntry {
    type Error = String;
    fn try_from(dto: IvmBytecodeEntryDto) -> Result<Self, Self::Error> {
        let nz = NonZeroU64::new(dto.count).ok_or_else(|| "count must be non-zero".to_string())?;
        Ok(IvmBytecodeEntry {
            original_contract: dto.original_contract,
            count: nz,
        })
    }
}

#[derive(Encode, Decode, Clone)]
struct LoadedActionDto<F> {
    executable: ExecutableRefDto,
    repeats: Repeats,
    authority: AccountId,
    filter: F,
    metadata: Metadata,
}

impl<F: Clone> From<&LoadedAction<F>> for LoadedActionDto<F> {
    fn from(a: &LoadedAction<F>) -> Self {
        LoadedActionDto {
            executable: ExecutableRefDto::from(&a.executable),
            repeats: a.repeats,
            authority: a.authority.clone(),
            filter: a.filter.clone(),
            metadata: a.metadata.clone(),
        }
    }
}

impl<F> TryFrom<LoadedActionDto<F>> for LoadedAction<F> {
    type Error = String;
    fn try_from(dto: LoadedActionDto<F>) -> Result<Self, Self::Error> {
        Ok(LoadedAction {
            executable: ExecutableRef::try_from(dto.executable)?,
            repeats: dto.repeats,
            authority: dto.authority,
            filter: dto.filter,
            metadata: dto.metadata,
        })
    }
}

/// Read-only accessor trait for trigger sets.
pub trait SetReadOnly {
    /// Data triggers map.
    fn data_triggers(&self) -> &impl StorageReadOnly<TriggerId, LoadedAction<DataEventFilter>>;
    /// Pipeline event triggers map.
    fn pipeline_triggers(
        &self,
    ) -> &impl StorageReadOnly<TriggerId, LoadedAction<PipelineEventFilterBox>>;
    /// Time triggers map.
    fn time_triggers(&self) -> &impl StorageReadOnly<TriggerId, LoadedAction<TimeEventFilter>>;
    /// Execute-by-call triggers map.
    fn by_call_triggers(
        &self,
    ) -> &impl StorageReadOnly<TriggerId, LoadedAction<ExecuteTriggerEventFilter>>;
    /// Trigger type registry.
    fn ids(&self) -> &impl StorageReadOnly<TriggerId, TriggeringEventType>;
    /// Mapping from code hash to bytecode entry.
    fn contracts(&self) -> &impl StorageReadOnly<HashOf<IvmBytecode>, IvmBytecodeEntry>;

    /// Get original [`IvmBytecode`] for [`TriggerId`].
    /// Returns `None` if there's no [`Trigger`]
    /// with specified `id` that has IVM executable
    #[inline]
    fn get_original_contract(&self, hash: &HashOf<IvmBytecode>) -> Option<&IvmBytecode> {
        self.contracts()
            .get(hash)
            .map(|entry| &entry.original_contract)
    }

    /// Convert [`LoadedAction`] to original [`Action`] by retrieving original
    /// [`IvmBytecode`] if applicable.
    ///
    /// Returns `None` when the original bytecode is missing.
    fn get_original_action<F>(&self, action: LoadedAction<F>) -> Option<SpecializedAction<F>>
    where
        F: Clone + EnsureTriggerAuthority,
    {
        let LoadedAction {
            executable,
            repeats,
            authority,
            filter,
            metadata,
        } = action;

        let original_executable = match executable {
            ExecutableRef::Ivm(ref blob_hash) => {
                let Some(original_contract) = self.get_original_contract(blob_hash).cloned() else {
                    warn!(
                        ?blob_hash,
                        "missing original trigger bytecode; skipping trigger action"
                    );
                    return None;
                };
                Executable::Ivm(original_contract)
            }
            ExecutableRef::Instructions(isi) => Executable::Instructions(isi),
        };

        let mut specialized =
            SpecializedAction::new(original_executable, repeats, authority, filter);
        specialized.metadata = metadata;
        Some(specialized)
    }

    /// Get all contained trigger ids without a particular order
    #[inline]
    fn ids_iter(&self) -> impl Iterator<Item = &TriggerId> {
        self.ids().iter().map(|(trigger_id, _)| trigger_id)
    }

    /// Returns an iterator over `(TriggerId, LoadedAction)` pairs for a given time event.
    fn match_time_event(
        &self,
        event: TimeEvent,
        current_block_height: u64,
        _current_block_time_ms: u64,
    ) -> impl Iterator<Item = (TriggerId, LoadedAction<TimeEventFilter>)> + '_ {
        let key_height = "__registered_block_height".parse::<Name>().ok();
        self.time_triggers()
            .iter()
            .filter(|(_, action)| trigger_is_enabled(action.metadata()))
            .flat_map(move |(id, action)| {
                let height_key = key_height.clone();
                let mut count = action.filter.count_matches(&event);
                if let Repeats::Exactly(repeats) = action.repeats {
                    count = min(repeats, count);
                }
                // Skip firing triggers that were registered in the same block that is being applied now.
                // Require `__registered_block_height` metadata set during registration.
                (0..count)
                    .map(move |_| (id.clone(), action.clone()))
                    .filter(move |(_, act)| {
                        let registered_height = height_key
                            .as_ref()
                            .and_then(|key| act.metadata().get(key))
                            .and_then(|json| json.try_into_any_norito::<u64>().ok());
                        registered_height.is_some_and(|height| height != current_block_height)
                    })
            })
    }

    /// Get [`ExecutableRef`] for given [`TriggerId`].
    /// Returns `None` if `id` is not in the set.
    fn get_executable(&self, id: &TriggerId) -> Option<&ExecutableRef> {
        let event_type = self.ids().get(id)?;

        let executable = match event_type {
            TriggeringEventType::Data => {
                self.data_triggers().get(id).map(|entry| &entry.executable)
            }
            TriggeringEventType::Pipeline => self
                .pipeline_triggers()
                .get(id)
                .map(|entry| &entry.executable),
            TriggeringEventType::Time => {
                self.time_triggers().get(id).map(|entry| &entry.executable)
            }
            TriggeringEventType::ExecuteTrigger => self
                .by_call_triggers()
                .get(id)
                .map(|entry| &entry.executable),
        };
        if executable.is_none() {
            warn!(
                trigger_id = %id,
                ?event_type,
                "trigger id missing from typed map while resolving executable"
            );
        }
        executable
    }

    /// Apply `f` to triggers whose action satisfies the predicate.
    ///
    /// Return an empty list if [`Set`] doesn't contain any such triggers.
    fn inspect_by_action<'a, P, F, R>(&'a self, filter: P, f: F) -> impl Iterator<Item = R> + 'a
    where
        P: Fn(&dyn LoadedActionTrait) -> bool + 'a,
        F: Fn(&TriggerId, &dyn LoadedActionTrait) -> R + 'a,
    {
        self.ids()
            .iter()
            .filter_map(move |(id, event_type)| match event_type {
                TriggeringEventType::Data => {
                    let Some(action) = self.data_triggers().get(id) else {
                        warn!(
                            trigger_id = %id,
                            ?event_type,
                            "trigger id missing from typed map while iterating triggers"
                        );
                        return None;
                    };
                    filter(action).then(|| f(id, action))
                }
                TriggeringEventType::Pipeline => {
                    let Some(action) = self.pipeline_triggers().get(id) else {
                        warn!(
                            trigger_id = %id,
                            ?event_type,
                            "trigger id missing from typed map while iterating triggers"
                        );
                        return None;
                    };
                    filter(action).then(|| f(id, action))
                }
                TriggeringEventType::Time => {
                    let Some(action) = self.time_triggers().get(id) else {
                        warn!(
                            trigger_id = %id,
                            ?event_type,
                            "trigger id missing from typed map while iterating triggers"
                        );
                        return None;
                    };
                    filter(action).then(|| f(id, action))
                }
                TriggeringEventType::ExecuteTrigger => {
                    let Some(action) = self.by_call_triggers().get(id) else {
                        warn!(
                            trigger_id = %id,
                            ?event_type,
                            "trigger id missing from typed map while iterating triggers"
                        );
                        return None;
                    };
                    filter(action).then(|| f(id, action))
                }
            })
    }

    /// Apply `f` to the trigger identified by `id`.
    ///
    /// Return [`None`] if [`Set`] doesn't contain the trigger with the given `id`.
    fn inspect_by_id<F, R>(&self, id: &TriggerId, f: F) -> Option<R>
    where
        F: Fn(&dyn LoadedActionTrait) -> R,
    {
        let event_type = self.ids().get(id).copied()?;

        let result = match event_type {
            TriggeringEventType::Data => self.data_triggers().get(id).map(|entry| f(entry)),
            TriggeringEventType::Pipeline => self.pipeline_triggers().get(id).map(|entry| f(entry)),
            TriggeringEventType::Time => self.time_triggers().get(id).map(|entry| f(entry)),
            TriggeringEventType::ExecuteTrigger => {
                self.by_call_triggers().get(id).map(|entry| f(entry))
            }
        };
        if result.is_none() {
            warn!(
                trigger_id = %id,
                ?event_type,
                "trigger id missing from typed map while inspecting trigger"
            );
        }
        result
    }
}

macro_rules! impl_set_ro {
    ($($ident:ty),*) => {$(
        impl SetReadOnly for $ident {
            fn data_triggers(&self) -> &impl StorageReadOnly<TriggerId, LoadedAction<DataEventFilter>> {
                &self.data_triggers
            }
            fn pipeline_triggers(&self) -> &impl StorageReadOnly<TriggerId, LoadedAction<PipelineEventFilterBox>> {
                &self.pipeline_triggers
            }
            fn time_triggers(&self) -> &impl StorageReadOnly<TriggerId, LoadedAction<TimeEventFilter>> {
                &self.time_triggers
            }
            fn by_call_triggers(&self) -> &impl StorageReadOnly<TriggerId, LoadedAction<ExecuteTriggerEventFilter>> {
                &self.by_call_triggers
            }
            fn ids(&self) -> &impl StorageReadOnly<TriggerId, TriggeringEventType> {
                &self.ids
            }
            fn contracts(&self) -> &impl StorageReadOnly<HashOf<IvmBytecode>, IvmBytecodeEntry> {
                &self.contracts
            }
        }
    )*};
}

impl_set_ro! {
    SetBlock<'_>, SetTransaction<'_, '_>, SetView<'_>
}

impl Set {
    /// Create struct to apply block's changes
    pub fn block(&self) -> SetBlock<'_> {
        SetBlock {
            data_triggers: self.data_triggers.block(),
            pipeline_triggers: self.pipeline_triggers.block(),
            time_triggers: self.time_triggers.block(),
            by_call_triggers: self.by_call_triggers.block(),
            ids: self.ids.block(),
            contracts: self.contracts.block(),
        }
    }

    /// Create struct to apply block's changes while reverting changes made in the latest block
    pub fn block_and_revert(&self) -> SetBlock<'_> {
        SetBlock {
            data_triggers: self.data_triggers.block_and_revert(),
            pipeline_triggers: self.pipeline_triggers.block_and_revert(),
            time_triggers: self.time_triggers.block_and_revert(),
            by_call_triggers: self.by_call_triggers.block_and_revert(),
            ids: self.ids.block_and_revert(),
            contracts: self.contracts.block_and_revert(),
        }
    }

    /// Create point in time view of the [`Set`]
    pub fn view(&self) -> SetView<'_> {
        SetView {
            data_triggers: self.data_triggers.view(),
            pipeline_triggers: self.pipeline_triggers.view(),
            time_triggers: self.time_triggers.view(),
            by_call_triggers: self.by_call_triggers.view(),
            ids: self.ids.view(),
            contracts: self.contracts.view(),
        }
    }

    /// Test-only helper to drop a trigger bytecode entry and commit the change.
    #[cfg(test)]
    pub(crate) fn remove_contract_for_test(&mut self, hash: HashOf<IvmBytecode>) -> bool {
        let mut block = self.block();
        let mut tx = block.transaction();
        let removed = tx.contracts.remove(hash).is_some();
        tx.apply();
        block.commit();
        removed
    }
}

impl<'set> SetBlock<'set> {
    /// Create struct to apply transaction's changes
    pub fn transaction(&mut self) -> SetTransaction<'_, 'set> {
        SetTransaction {
            data_triggers: self.data_triggers.transaction(),
            pipeline_triggers: self.pipeline_triggers.transaction(),
            time_triggers: self.time_triggers.transaction(),
            by_call_triggers: self.by_call_triggers.transaction(),
            ids: self.ids.transaction(),
            contracts: self.contracts.transaction(),
        }
    }

    /// Commit block's changes
    pub fn commit(self) {
        // NOTE: commit in reverse order
        self.contracts.commit();
        self.ids.commit();
        self.by_call_triggers.commit();
        self.time_triggers.commit();
        self.pipeline_triggers.commit();
        self.data_triggers.commit();
    }

    /// Returns an iterator over `(TriggerId, LoadedAction)` pairs for a given time event.
    pub fn match_time_event(
        &self,
        event: TimeEvent,
        current_block_height: u64,
        current_block_time_ms: u64,
    ) -> impl Iterator<Item = (TriggerId, LoadedAction<TimeEventFilter>)> + '_ {
        <Self as SetReadOnly>::match_time_event(
            self,
            event,
            current_block_height,
            current_block_time_ms,
        )
    }
}

trait TriggeringEventFilter: EventFilter {}
impl TriggeringEventFilter for DataEventFilter {}
impl TriggeringEventFilter for PipelineEventFilterBox {}
impl TriggeringEventFilter for TimeEventFilter {}
impl TriggeringEventFilter for ExecuteTriggerEventFilter {}

impl<'block, 'set> SetTransaction<'block, 'set> {
    /// Apply transaction's changes
    pub fn apply(self) {
        // NOTE: apply in reverse order
        self.contracts.apply();
        self.ids.apply();
        self.by_call_triggers.apply();
        self.time_triggers.apply();
        self.pipeline_triggers.apply();
        self.data_triggers.apply();
    }

    /// Replace occurrences of `old` with `new` in trigger authorities and filters.
    pub fn replace_account_id(&mut self, old: &AccountId, new: &AccountId) {
        if old == new {
            return;
        }
        let trigger_ids: Vec<_> = self.ids.iter().map(|(id, _)| id.clone()).collect();
        for trigger_id in trigger_ids {
            let Some(event_type) = self.ids.get(&trigger_id).copied() else {
                continue;
            };
            let updated = match event_type {
                TriggeringEventType::Data => self
                    .data_triggers
                    .get_mut(&trigger_id)
                    .map(|action| replace_trigger_authority(action, old, new)),
                TriggeringEventType::Pipeline => self
                    .pipeline_triggers
                    .get_mut(&trigger_id)
                    .map(|action| replace_trigger_authority(action, old, new)),
                TriggeringEventType::Time => self
                    .time_triggers
                    .get_mut(&trigger_id)
                    .map(|action| replace_trigger_authority(action, old, new)),
                TriggeringEventType::ExecuteTrigger => self
                    .by_call_triggers
                    .get_mut(&trigger_id)
                    .map(|action| replace_by_call_authority(action, old, new)),
            };
            if updated.is_none() {
                warn!(
                    trigger_id = %trigger_id,
                    ?event_type,
                    "`Set` ids referenced a missing trigger while rekeying"
                );
            }
        }
    }

    /// Add trigger with [`DataEventFilter`]
    ///
    /// Return `false` if a trigger with given id already exists
    ///
    /// # Errors
    ///
    /// Return [`Err`] if failed to preload IVM trigger
    #[inline]
    pub fn add_data_trigger(
        &mut self,
        trigger: SpecializedTrigger<DataEventFilter>,
    ) -> Result<bool> {
        Ok(self.add_to(trigger, TriggeringEventType::Data, |me| {
            &mut me.data_triggers
        }))
    }

    /// Add trigger with [`PipelineEventFilterBox`]
    ///
    /// Return `false` if a trigger with given id already exists
    ///
    /// # Errors
    ///
    /// Return [`Err`] if failed to preload IVM trigger
    #[inline]
    pub fn add_pipeline_trigger(
        &mut self,
        trigger: SpecializedTrigger<PipelineEventFilterBox>,
    ) -> Result<bool> {
        Ok(self.add_to(trigger, TriggeringEventType::Pipeline, |me| {
            &mut me.pipeline_triggers
        }))
    }

    /// Add trigger with [`TimeEventFilter`]
    ///
    /// Returns `false` if a trigger with given id already exists
    ///
    /// # Errors
    ///
    /// Return [`Err`] if failed to preload IVM trigger
    #[inline]
    pub fn add_time_trigger(
        &mut self,
        trigger: SpecializedTrigger<TimeEventFilter>,
    ) -> Result<bool> {
        Ok(self.add_to(trigger, TriggeringEventType::Time, |me| {
            &mut me.time_triggers
        }))
    }

    /// Add trigger with [`ExecuteTriggerEventFilter`]
    ///
    /// Returns `false` if a trigger with given id already exists
    ///
    /// # Errors
    ///
    /// Return [`Err`] if failed to preload IVM trigger
    #[inline]
    pub fn add_by_call_trigger(
        &mut self,
        trigger: SpecializedTrigger<ExecuteTriggerEventFilter>,
    ) -> Result<bool> {
        Ok(
            self.add_to(trigger, TriggeringEventType::ExecuteTrigger, |me| {
                &mut me.by_call_triggers
            }),
        )
    }

    /// Add generic trigger to generic collection
    ///
    /// Returns `false` if a trigger with given id already exists
    ///
    /// # Errors
    ///
    /// Return [`Err`] if failed to preload IVM trigger
    fn add_to<F: TriggeringEventFilter + mv::Value>(
        &mut self,
        trigger: SpecializedTrigger<F>,
        event_type: TriggeringEventType,
        map: impl FnOnce(&mut Self) -> &mut StorageTransaction<'block, 'set, TriggerId, LoadedAction<F>>,
    ) -> bool {
        let SpecializedTrigger {
            id: trigger_id,
            action:
                SpecializedAction {
                    executable,
                    repeats,
                    authority,
                    filter,
                    metadata,
                },
        } = trigger;

        if self.ids.get(&trigger_id).is_some() {
            return false;
        }

        let loaded_executable = match executable {
            Executable::Ivm(bytes) => {
                let hash = HashOf::new(&bytes);
                if let Some(IvmBytecodeEntry { count, .. }) = self.contracts.get_mut(&hash) {
                    let updated = count.get().strict_add(1);
                    *count = NonZeroU64::new(updated).expect(
                        "There is no way someone could register 2^64 amount of same triggers",
                    );
                } else {
                    self.contracts.insert(
                        hash,
                        IvmBytecodeEntry {
                            original_contract: bytes,
                            count: NonZeroU64::MIN,
                        },
                    );
                }
                ExecutableRef::Ivm(hash)
            }
            Executable::IvmProved(proved) => {
                // Triggers do not carry proof attachments; treat proved IVM executables as plain
                // bytecode and execute them via the standard IVM trigger machinery.
                let bytes = proved.bytecode;
                let hash = HashOf::new(&bytes);
                if let Some(IvmBytecodeEntry { count, .. }) = self.contracts.get_mut(&hash) {
                    let updated = count.get().strict_add(1);
                    *count = NonZeroU64::new(updated).expect(
                        "There is no way someone could register 2^64 amount of same triggers",
                    );
                } else {
                    self.contracts.insert(
                        hash,
                        IvmBytecodeEntry {
                            original_contract: bytes,
                            count: NonZeroU64::MIN,
                        },
                    );
                }
                ExecutableRef::Ivm(hash)
            }
            Executable::Instructions(instructions) => ExecutableRef::Instructions(instructions),
        };
        map(self).insert(
            trigger_id.clone(),
            LoadedAction {
                executable: loaded_executable,
                repeats,
                authority,
                filter,
                metadata,
            },
        );
        self.ids.insert(trigger_id, event_type);
        true
    }

    /// Apply `f` to the trigger identified by `id`.
    ///
    /// Return [`None`] if [`Set`] doesn't contain the trigger with the given `id`.
    pub fn inspect_by_id_mut<F, R>(&mut self, id: &TriggerId, f: F) -> Option<R>
    where
        F: Fn(&mut dyn LoadedActionTrait) -> R,
    {
        let event_type = self.ids.get(id).copied()?;

        let result = match event_type {
            TriggeringEventType::Data => self.data_triggers.get_mut(id).map(|entry| f(entry)),
            TriggeringEventType::Pipeline => {
                self.pipeline_triggers.get_mut(id).map(|entry| f(entry))
            }
            TriggeringEventType::Time => self.time_triggers.get_mut(id).map(|entry| f(entry)),
            TriggeringEventType::ExecuteTrigger => {
                self.by_call_triggers.get_mut(id).map(|entry| f(entry))
            }
        };
        if result.is_none() {
            warn!(
                trigger_id = %id,
                ?event_type,
                "trigger id missing from typed map while mutating trigger"
            );
        }
        result
    }

    /// Remove a trigger from the [`Set`].
    ///
    /// Return `false` if [`Set`] doesn't contain the trigger with the given `id`.
    /// Logs and continues if the internal storage is inconsistent.
    pub fn remove(&mut self, id: &TriggerId) -> bool {
        let Some(event_type) = self.ids.remove(id.clone()) else {
            return false;
        };

        let removed = match event_type {
            TriggeringEventType::Data => {
                Self::remove_from(&mut self.contracts, &mut self.data_triggers, id.clone())
            }
            TriggeringEventType::Pipeline => {
                Self::remove_from(&mut self.contracts, &mut self.pipeline_triggers, id.clone())
            }
            TriggeringEventType::Time => {
                Self::remove_from(&mut self.contracts, &mut self.time_triggers, id.clone())
            }
            TriggeringEventType::ExecuteTrigger => {
                Self::remove_from(&mut self.contracts, &mut self.by_call_triggers, id.clone())
            }
        };

        if !removed {
            warn!(
                trigger_id = %id,
                ?event_type,
                "`Set` ids referenced a missing trigger while removing"
            );
        }

        true
    }

    /// Modify repetitions of the hook identified by [`TriggerId`].
    ///
    /// # Errors
    ///
    /// - If a trigger with the given id is not found.
    /// - If updating the current trigger `repeats` causes an overflow. Indefinitely
    ///   repeating triggers and triggers set for exact time always cause an overflow.
    pub fn mod_repeats(
        &mut self,
        id: &TriggerId,
        f: impl Fn(u32) -> Result<u32, RepeatsOverflowError>,
    ) -> Result<(), ModRepeatsError> {
        self.inspect_by_id_mut(id, |action| match action.repeats() {
            Repeats::Exactly(repeats) => {
                let new_repeats = f(*repeats)?;
                action.set_repeats(Repeats::Exactly(new_repeats));
                Ok(())
            }
            _ => Err(ModRepeatsError::RepeatsOverflow(RepeatsOverflowError)),
        })
        .ok_or_else(|| ModRepeatsError::NotFound(id.clone()))
        // .flatten() -- unstable
        .and_then(std::convert::identity)
    }

    /// Remove trigger from `triggers` and decrease the counter of the original [`IvmBytecode`].
    ///
    /// Note that this function doesn't remove the trigger from [`Set::ids`].
    ///
    /// Returns `true` if trigger was removed and `false` otherwise.
    fn remove_from<F: mv::Value + EventFilter>(
        contracts: &mut TriggerContractStoreTransaction<'block, 'set>,
        triggers: &mut StorageTransaction<'block, 'set, TriggerId, LoadedAction<F>>,
        trigger_id: TriggerId,
    ) -> bool {
        triggers
            .remove(trigger_id)
            .map(|loaded_action| {
                if let Some(blob_hash) = loaded_action.extract_blob_hash() {
                    Self::remove_original_trigger(contracts, blob_hash);
                }
            })
            .is_some()
    }

    /// Decrease the counter of the original [`IvmBytecode`] by `blob_hash`
    /// or remove it if the counter reaches zero.
    /// Logs and skips removal if the bytecode entry is missing.
    fn remove_original_trigger(
        contracts: &mut TriggerContractStoreTransaction,
        blob_hash: HashOf<IvmBytecode>,
    ) {
        #[allow(clippy::option_if_let_else)] // More readable this way
        match contracts.get_mut(&blob_hash) {
            Some(entry) => {
                let count = &mut entry.count;
                if let Some(new_count) = NonZeroU64::new(count.get() - 1) {
                    *count = new_count;
                } else {
                    contracts.remove(blob_hash);
                }
            }
            None => {
                warn!(
                    ?blob_hash,
                    "`Set` contracts missing entry for trigger bytecode; skipping removal"
                );
            }
        }
    }

    /// Decrease `action`s for provided triggers and remove those whose counter reached zero.
    pub fn decrease_repeats<'a>(
        &'a mut self,
        triggers: impl Iterator<Item = &'a TriggerId>,
    ) -> Vec<TriggerId> {
        for id in triggers {
            // Ignoring error if trigger has not `Repeats::Exact(_)` but something else
            let _mod_repeats_res = self.mod_repeats(id, |n| Ok(n.saturating_sub(1)));
        }

        let mut removed = Vec::new();
        let Self {
            data_triggers,
            pipeline_triggers,
            time_triggers,
            by_call_triggers,
            ids,
            contracts,
            ..
        } = self;
        Self::remove_zeros(&mut removed, ids, contracts, data_triggers);
        Self::remove_zeros(&mut removed, ids, contracts, pipeline_triggers);
        Self::remove_zeros(&mut removed, ids, contracts, time_triggers);
        Self::remove_zeros(&mut removed, ids, contracts, by_call_triggers);

        removed
    }

    /// Remove actions with zero execution count from `triggers`
    fn remove_zeros<F: mv::Value + EventFilter>(
        removed: &mut Vec<TriggerId>,
        ids: &mut StorageTransaction<'block, 'set, TriggerId, TriggeringEventType>,
        contracts: &mut TriggerContractStoreTransaction<'block, 'set>,
        triggers: &mut StorageTransaction<'block, 'set, TriggerId, LoadedAction<F>>,
    ) {
        let mut to_remove: Vec<TriggerId> = triggers
            .iter()
            .filter(|(_, action)| action.repeats.is_depleted())
            .map(|(id, _)| id.clone())
            .collect();

        for id in &to_remove {
            let removed_id = ids.remove(id.clone()).is_some();
            let removed_trigger = Self::remove_from(contracts, triggers, id.clone());
            if !removed_id || !removed_trigger {
                warn!(
                    trigger_id = %id,
                    removed_id,
                    removed_trigger,
                    "`Set` trigger collections out of sync while removing depleted trigger"
                );
            }
        }

        removed.append(&mut to_remove);
    }
}

fn replace_trigger_authority<F>(
    action: &mut LoadedAction<F>,
    old: &AccountId,
    new: &AccountId,
) -> bool {
    if action.authority == *old {
        action.authority = new.clone();
        true
    } else {
        false
    }
}

fn replace_by_call_authority(
    action: &mut LoadedAction<ExecuteTriggerEventFilter>,
    old: &AccountId,
    new: &AccountId,
) -> bool {
    let mut updated = replace_trigger_authority(action, old, new);
    let update_filter = match action.filter.authority() {
        Some(authority) => authority == old,
        None => action.authority == *new,
    };
    if update_filter {
        action.filter = action.filter.clone().under_authority(new.clone());
        updated = true;
    }
    updated
}

/// Same as [`Executable`], but instead of
/// [`Ivm`](iroha_data_model::transaction::Executable::Ivm) contains hash of the IVM blob
/// Hash of the bytecode used by the trigger
#[derive(Clone)]
pub enum ExecutableRef {
    /// Loaded IVM
    Ivm(HashOf<IvmBytecode>),
    /// Vector of ISI
    Instructions(ConstVec<InstructionBox>),
}

impl core::fmt::Debug for ExecutableRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ivm(hash) => f.debug_tuple("Ivm").field(hash).finish(),
            Self::Instructions(instructions) => {
                f.debug_tuple("Instructions").field(instructions).finish()
            }
        }
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for ExecutableRef {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        match self {
            ExecutableRef::Ivm(hash) => {
                norito::json::write_json_string("Ivm", out);
                out.push(':');
                JsonSerializeTrait::json_serialize(hash, out);
            }
            ExecutableRef::Instructions(instrs) => {
                norito::json::write_json_string("Instructions", out);
                out.push(':');
                JsonSerializeTrait::json_serialize(instrs, out);
            }
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl json::JsonDeserialize for ExecutableRef {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = norito::json::Value::json_deserialize(parser)?;
        let map = match value {
            norito::json::Value::Object(map) => map,
            other => {
                return Err(json::Error::InvalidField {
                    field: "ExecutableRef".to_owned(),
                    message: format!("expected JSON object with single key, found {other:?}."),
                });
            }
        };

        if map.len() != 1 {
            return Err(json::Error::InvalidField {
                field: "ExecutableRef".to_owned(),
                message: "expected exactly one variant field".to_owned(),
            });
        }

        let (key, inner) = map.into_iter().next().expect("checked map length");
        match key.as_str() {
            "Ivm" => json::from_value(inner).map(ExecutableRef::Ivm),
            "Instructions" => json::from_value(inner).map(ExecutableRef::Instructions),
            other => Err(json::Error::unknown_field(other)),
        }
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use core::time::Duration;

    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        events::execute_trigger::ExecuteTriggerEventFilter,
        metadata::Metadata,
        prelude::{
            AccountId, Executable, ExecutionTime, InstructionBox, Level, Log, TimeEvent,
            TimeEventFilter, TimeInterval, TriggerId,
        },
    };
    use iroha_primitives::{const_vec::ConstVec, json::Json};

    use crate::smartcontracts::isi::triggers::TRIGGER_ENABLED_METADATA_KEY;

    use super::*;

    fn sample_hash() -> HashOf<IvmBytecode> {
        let bytecode = IvmBytecode::from_compiled(vec![0x01, 0x02, 0x03]);
        HashOf::new(&bytecode)
    }

    #[test]
    fn inspect_by_id_skips_missing_entry() {
        let mut set = Set::default();
        let trigger_id: TriggerId = "missing_trigger".parse().expect("valid trigger id");
        set.ids
            .insert(trigger_id.clone(), TriggeringEventType::Time);

        let view = set.view();
        let found = SetReadOnly::inspect_by_id(&view, &trigger_id, |_| ());
        assert!(found.is_none(), "missing trigger should return None");
    }

    #[test]
    fn inspect_by_id_mut_skips_missing_entry() {
        let set = Set::default();
        let trigger_id: TriggerId = "missing_trigger_mut".parse().expect("valid trigger id");

        let mut block = set.block();
        let mut tx = block.transaction();
        tx.ids.insert(trigger_id.clone(), TriggeringEventType::Time);

        let found = tx.inspect_by_id_mut(&trigger_id, |_| ());
        assert!(found.is_none(), "missing trigger should return None");
    }

    #[test]
    fn executable_ref_json_roundtrip_ivm() {
        let original = ExecutableRef::Ivm(sample_hash());

        let json = norito::json::to_json(&original).expect("serialize ExecutableRef::Ivm");
        let reparsed: ExecutableRef =
            norito::json::from_json(&json).expect("deserialize ExecutableRef::Ivm");

        match reparsed {
            ExecutableRef::Ivm(hash) => assert_eq!(hash, sample_hash()),
            other => panic!("expected Ivm variant, got {other:?}"),
        }
    }

    #[test]
    fn executable_ref_json_roundtrip_instructions() {
        let instruction = InstructionBox::from(Log::new(Level::INFO, "roundtrip".to_owned()));
        let instructions = ConstVec::from(vec![instruction]);
        let original = ExecutableRef::Instructions(instructions.clone());

        let json = norito::json::to_json(&original).expect("serialize ExecutableRef::Instructions");
        let reparsed: ExecutableRef =
            norito::json::from_json(&json).expect("deserialize ExecutableRef::Instructions");

        match reparsed {
            ExecutableRef::Instructions(restored) => assert_eq!(restored, instructions),
            other => panic!("expected Instructions variant, got {other:?}"),
        }
    }

    #[test]
    fn executable_ref_dto_decode_from_slice_roundtrip() {
        let instruction = InstructionBox::from(Log::new(Level::INFO, "dto-roundtrip".to_owned()));
        let dto =
            ExecutableRefDto::Instructions(ConstVec::from(vec![instruction.clone(), instruction]));
        let bytes =
            norito::to_bytes(&dto).expect("serialize ExecutableRefDto::Instructions variant");
        let decoded: ExecutableRefDto = norito::decode_from_bytes(&bytes).expect("decode dto");
        assert_eq!(decoded, dto);
    }

    #[test]
    fn replace_account_id_updates_trigger_authority_and_filter() {
        let set = Set::default();
        let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().unwrap();
        let old = AccountId::new(domain_id.clone(), KeyPair::random().public_key().clone());
        let new = AccountId::new(domain_id.clone(), KeyPair::random().public_key().clone());

        let instruction = InstructionBox::from(Log::new(Level::INFO, "noop".to_owned()));
        let executable = Executable::Instructions(ConstVec::from(vec![instruction.clone()]));
        let time_trigger_id: TriggerId = "time_trigger_rekey".parse().expect("valid id");
        let call_trigger_id: TriggerId = "call_trigger_rekey".parse().expect("valid id");

        let time_action = SpecializedAction::new(
            executable.clone(),
            Repeats::Exactly(1),
            old.clone(),
            TimeEventFilter(ExecutionTime::PreCommit),
        );
        let call_action = SpecializedAction::new(
            executable,
            Repeats::Exactly(1),
            old.clone(),
            ExecuteTriggerEventFilter::new(),
        );

        let mut block = set.block();
        let mut tx = block.transaction();
        tx.add_time_trigger(SpecializedTrigger::new(
            time_trigger_id.clone(),
            time_action,
        ))
        .expect("add time trigger");
        tx.add_by_call_trigger(SpecializedTrigger::new(
            call_trigger_id.clone(),
            call_action,
        ))
        .expect("add call trigger");
        tx.replace_account_id(&old, &new);
        tx.apply();
        block.commit();

        let view = set.view();
        let time_action = view
            .time_triggers()
            .get(&time_trigger_id)
            .expect("time trigger present");
        assert_eq!(time_action.authority, new);
        let call_action = view
            .by_call_triggers()
            .get(&call_trigger_id)
            .expect("call trigger present");
        assert_eq!(call_action.authority, new);
        assert_eq!(
            call_action.filter.authority(),
            Some(&new),
            "by-call filter authority should be updated"
        );
    }

    #[test]
    fn match_time_event_skips_recently_registered_triggers() {
        crate::test_alias::ensure();
        let set = Set::default();
        {
            let mut block = set.block();
            {
                let mut tx = block.transaction();
                let trigger_id: TriggerId = "time_trigger".parse().expect("valid id");
                let authority: AccountId =
                    "alice@wonderland".parse().expect("authority must parse");
                let instruction = InstructionBox::from(Log::new(Level::INFO, "noop".to_owned()));
                let executable = Executable::Instructions(ConstVec::from(vec![instruction]));
                let mut action = SpecializedAction::new(
                    executable,
                    Repeats::Exactly(1),
                    authority,
                    TimeEventFilter(ExecutionTime::PreCommit),
                );
                let mut metadata = Metadata::default();
                metadata.insert(
                    "__registered_block_height".parse().expect("valid name"),
                    Json::from(42_u64),
                );
                metadata.insert(
                    "__registered_at_ms".parse().expect("valid name"),
                    Json::from(1_234_u64),
                );
                action.metadata = metadata;
                let trigger = SpecializedTrigger::new(trigger_id, action);
                tx.add_time_trigger(trigger)
                    .expect("time trigger should be added");
                tx.apply();
            }
            block.commit();
        }

        let block_view = set.block();
        let interval_current =
            TimeInterval::new_since_to(Duration::from_millis(0), Duration::from_millis(1_234));
        let time_event = TimeEvent {
            interval: interval_current,
        };

        assert!(
            block_view
                .match_time_event(time_event, 42, 1_234)
                .next()
                .is_none(),
            "trigger registered in current block must be skipped"
        );
        let matches_same_time = block_view.match_time_event(time_event, 99, 1_234).count();
        assert_eq!(
            matches_same_time, 1,
            "trigger should match when height differs even if timestamp matches"
        );

        let interval_later =
            TimeInterval::new_since_to(Duration::from_millis(1_234), Duration::from_millis(2_000));
        let later_event = TimeEvent {
            interval: interval_later,
        };

        let matches_later = block_view.match_time_event(later_event, 99, 2_000).count();
        assert_eq!(
            matches_later, 1,
            "trigger should appear for subsequent blocks"
        );
    }

    #[test]
    fn match_time_event_skips_disabled_triggers() {
        crate::test_alias::ensure();
        let set = Set::default();
        {
            let mut block = set.block();
            {
                let mut tx = block.transaction();
                let trigger_id: TriggerId = "time_trigger_disabled".parse().expect("valid id");
                let authority: AccountId =
                    "alice@wonderland".parse().expect("authority must parse");
                let instruction = InstructionBox::from(Log::new(Level::INFO, "noop".to_owned()));
                let executable = Executable::Instructions(ConstVec::from(vec![instruction]));
                let mut action = SpecializedAction::new(
                    executable,
                    Repeats::Exactly(1),
                    authority,
                    TimeEventFilter(ExecutionTime::PreCommit),
                );
                let mut metadata = Metadata::default();
                metadata.insert(
                    "__registered_block_height".parse().expect("valid name"),
                    Json::from(42_u64),
                );
                metadata.insert(
                    "__registered_at_ms".parse().expect("valid name"),
                    Json::from(1_234_u64),
                );
                metadata.insert(
                    TRIGGER_ENABLED_METADATA_KEY.parse().expect("valid name"),
                    Json::from(false),
                );
                action.metadata = metadata;
                let trigger = SpecializedTrigger::new(trigger_id, action);
                tx.add_time_trigger(trigger)
                    .expect("time trigger should be added");
                tx.apply();
            }
            block.commit();
        }

        let block_view = set.block();
        let interval =
            TimeInterval::new_since_to(Duration::from_millis(0), Duration::from_millis(1_234));
        let time_event = TimeEvent { interval };

        assert!(
            block_view
                .match_time_event(time_event, 99, 1_234)
                .next()
                .is_none(),
            "disabled trigger must be skipped"
        );
    }

    #[test]
    fn match_time_event_requires_registration_metadata() {
        crate::test_alias::ensure();
        let set = Set::default();
        {
            let mut block = set.block();
            {
                let mut tx = block.transaction();
                let trigger_id: TriggerId = "time_trigger_missing_meta".parse().expect("valid id");
                let authority: AccountId =
                    "alice@wonderland".parse().expect("authority must parse");
                let instruction = InstructionBox::from(Log::new(Level::INFO, "noop".to_owned()));
                let executable = Executable::Instructions(ConstVec::from(vec![instruction]));
                let action = SpecializedAction::new(
                    executable,
                    Repeats::Exactly(1),
                    authority,
                    TimeEventFilter(ExecutionTime::PreCommit),
                );
                let trigger = SpecializedTrigger::new(trigger_id, action);
                tx.add_time_trigger(trigger)
                    .expect("time trigger should be added");
                tx.apply();
            }
            block.commit();
        }

        let block_view = set.block();
        let interval =
            TimeInterval::new_since_to(Duration::from_millis(0), Duration::from_millis(1_234));
        let time_event = TimeEvent { interval };

        assert!(
            block_view
                .match_time_event(time_event, 99, 1_234)
                .next()
                .is_none(),
            "trigger missing registration metadata must be skipped"
        );
    }
}

/// [`SetTransaction::mod_repeats()`] error
#[derive(Debug, Clone, thiserror::Error, displaydoc::Display)]
pub enum ModRepeatsError {
    /// Trigger with id = `{0}` not found
    NotFound(TriggerId),
    /// Trigger repeats count overflow error
    RepeatsOverflow(#[from] RepeatsOverflowError),
}

/// Trigger repeats count overflow
#[derive(Debug, Copy, Clone, thiserror::Error, displaydoc::Display)]
pub struct RepeatsOverflowError;

impl From<ModRepeatsError> for InstructionExecutionError {
    fn from(err: ModRepeatsError) -> Self {
        match err {
            ModRepeatsError::NotFound(not_found_id) => FindError::Trigger(not_found_id).into(),
            ModRepeatsError::RepeatsOverflow(_) => MathError::Overflow.into(),
        }
    }
}

// --- Norito DTO for Set (Phase 1 scaffolding) ---

/// Norito-encoded Data Transfer Object for serializing/deserializing the
/// `Set` of triggers and associated entries. Used in scaffolding paths where a
/// compact binary representation is required.
#[derive(Encode, Decode)]
pub struct SetDto {
    data: Vec<(TriggerId, LoadedActionDto<DataEventFilter>)>,
    pipeline: Vec<(TriggerId, LoadedActionDto<PipelineEventFilterBox>)>,
    time: Vec<(TriggerId, LoadedActionDto<TimeEventFilter>)>,
    by_call: Vec<(TriggerId, LoadedActionDto<ExecuteTriggerEventFilter>)>,
    ids: Vec<(TriggerId, TriggeringEventType)>,
    contracts: Vec<(HashOf<IvmBytecode>, IvmBytecodeEntryDto)>,
}

impl SetDto {
    /// Encode this DTO into Norito bytes.
    ///
    /// # Errors
    /// Returns an error if Norito encoding fails.
    pub fn encode(&self) -> Result<Vec<u8>, norito::core::Error> {
        norito::to_bytes(self)
    }
    /// Decode a DTO from Norito bytes.
    ///
    /// # Errors
    /// Returns an error if Norito decoding fails.
    pub fn decode(bytes: &[u8]) -> Result<Self, norito::core::Error> {
        norito::decode_from_bytes(bytes)
    }
}

impl From<&Set> for SetDto {
    fn from(set: &Set) -> Self {
        // Use a read-only view to iterate storages
        let view = SetView {
            data_triggers: set.data_triggers.view(),
            pipeline_triggers: set.pipeline_triggers.view(),
            time_triggers: set.time_triggers.view(),
            by_call_triggers: set.by_call_triggers.view(),
            ids: set.ids.view(),
            contracts: set.contracts.view(),
        };
        let data: Vec<(TriggerId, LoadedActionDto<DataEventFilter>)> = view
            .data_triggers
            .iter()
            .map(|(k, v)| (k.clone(), LoadedActionDto::from(v)))
            .collect();
        let pipeline: Vec<(TriggerId, LoadedActionDto<PipelineEventFilterBox>)> = view
            .pipeline_triggers
            .iter()
            .map(|(k, v)| (k.clone(), LoadedActionDto::from(v)))
            .collect();
        let time: Vec<(TriggerId, LoadedActionDto<TimeEventFilter>)> = view
            .time_triggers
            .iter()
            .map(|(k, v)| (k.clone(), LoadedActionDto::from(v)))
            .collect();
        let by_call: Vec<(TriggerId, LoadedActionDto<ExecuteTriggerEventFilter>)> = view
            .by_call_triggers
            .iter()
            .map(|(k, v)| (k.clone(), LoadedActionDto::from(v)))
            .collect();
        let ids = view.ids.iter().map(|(k, v)| (k.clone(), *v)).collect();
        let contracts: Vec<(HashOf<IvmBytecode>, IvmBytecodeEntryDto)> = view
            .contracts
            .iter()
            .map(|(k, v)| (*k, IvmBytecodeEntryDto::from(v)))
            .collect();
        SetDto {
            data,
            pipeline,
            time,
            by_call,
            ids,
            contracts,
        }
    }
}

fn load_trigger_entries<F>(
    raw: Vec<(TriggerId, LoadedActionDto<F>)>,
    event_type: TriggeringEventType,
    contracts: &BTreeMap<HashOf<IvmBytecode>, IvmBytecodeEntry>,
    ids: &mut BTreeMap<TriggerId, TriggeringEventType>,
    duplicate_ids: &mut Vec<TriggerId>,
    missing_contracts: &mut Vec<TriggerId>,
) -> Result<Vec<(TriggerId, LoadedAction<F>)>, String> {
    let mut entries = Vec::with_capacity(raw.len());
    for (id, dto) in raw {
        if ids.contains_key(&id) {
            duplicate_ids.push(id);
            continue;
        }
        let action = LoadedAction::try_from(dto)?;
        if let Some(blob_hash) = action.extract_blob_hash() {
            if !contracts.contains_key(&blob_hash) {
                missing_contracts.push(id);
                continue;
            }
        }
        ids.insert(id.clone(), event_type);
        entries.push((id, action));
    }
    Ok(entries)
}

impl TryFrom<SetDto> for Set {
    type Error = String;
    #[allow(clippy::too_many_lines)]
    fn try_from(dto: SetDto) -> Result<Self, Self::Error> {
        let SetDto {
            data,
            pipeline,
            time,
            by_call,
            ids: ids_raw,
            contracts,
        } = dto;

        let mut contracts_map = BTreeMap::new();
        let mut duplicate_contracts = 0usize;
        for (hash, entry) in contracts {
            let entry = IvmBytecodeEntry::try_from(entry)?;
            if contracts_map.insert(hash, entry).is_some() {
                duplicate_contracts = duplicate_contracts.saturating_add(1);
            }
        }

        let mut ids = BTreeMap::new();
        let mut duplicate_ids = Vec::new();
        let mut missing_contracts = Vec::new();

        let data = load_trigger_entries(
            data,
            TriggeringEventType::Data,
            &contracts_map,
            &mut ids,
            &mut duplicate_ids,
            &mut missing_contracts,
        )?;
        let pipeline = load_trigger_entries(
            pipeline,
            TriggeringEventType::Pipeline,
            &contracts_map,
            &mut ids,
            &mut duplicate_ids,
            &mut missing_contracts,
        )?;
        let time = load_trigger_entries(
            time,
            TriggeringEventType::Time,
            &contracts_map,
            &mut ids,
            &mut duplicate_ids,
            &mut missing_contracts,
        )?;
        let by_call = load_trigger_entries(
            by_call,
            TriggeringEventType::ExecuteTrigger,
            &contracts_map,
            &mut ids,
            &mut duplicate_ids,
            &mut missing_contracts,
        )?;

        let mut orphaned_ids = 0usize;
        let mut mismatched_ids = 0usize;
        for (id, event_type) in ids_raw {
            match ids.get(&id) {
                Some(actual) if actual == &event_type => {}
                Some(_) => mismatched_ids = mismatched_ids.saturating_add(1),
                None => orphaned_ids = orphaned_ids.saturating_add(1),
            }
        }

        if !duplicate_ids.is_empty() {
            warn!(
                count = duplicate_ids.len(),
                "dropping duplicate trigger ids while repairing trigger storage"
            );
        }
        if !missing_contracts.is_empty() {
            warn!(
                count = missing_contracts.len(),
                "dropping triggers referencing missing IVM bytecode"
            );
        }
        if orphaned_ids > 0 || mismatched_ids > 0 {
            warn!(
                orphaned_ids,
                mismatched_ids, "trigger id registry out of sync; rebuilding from typed triggers"
            );
        }
        if duplicate_contracts > 0 {
            warn!(
                count = duplicate_contracts,
                "duplicate trigger bytecode entries found; keeping latest"
            );
        }

        let mut contract_counts: BTreeMap<HashOf<IvmBytecode>, u64> = BTreeMap::new();
        for (_, action) in &data {
            if let Some(blob_hash) = action.extract_blob_hash() {
                let count = contract_counts.entry(blob_hash).or_insert(0);
                *count = count.saturating_add(1);
            }
        }
        for (_, action) in &pipeline {
            if let Some(blob_hash) = action.extract_blob_hash() {
                let count = contract_counts.entry(blob_hash).or_insert(0);
                *count = count.saturating_add(1);
            }
        }
        for (_, action) in &time {
            if let Some(blob_hash) = action.extract_blob_hash() {
                let count = contract_counts.entry(blob_hash).or_insert(0);
                *count = count.saturating_add(1);
            }
        }
        for (_, action) in &by_call {
            if let Some(blob_hash) = action.extract_blob_hash() {
                let count = contract_counts.entry(blob_hash).or_insert(0);
                *count = count.saturating_add(1);
            }
        }

        let mut repaired_contracts = BTreeMap::new();
        let mut dropped_contracts = 0usize;
        let mut fixed_counts = 0usize;
        for (hash, mut entry) in contracts_map {
            let Some(count) = contract_counts.get(&hash) else {
                dropped_contracts = dropped_contracts.saturating_add(1);
                continue;
            };
            let Some(new_count) = NonZeroU64::new(*count) else {
                warn!(
                    ?hash,
                    count, "invalid trigger bytecode reference count; dropping entry"
                );
                dropped_contracts = dropped_contracts.saturating_add(1);
                continue;
            };
            if entry.count.get() != new_count.get() {
                fixed_counts = fixed_counts.saturating_add(1);
                entry.count = new_count;
            }
            repaired_contracts.insert(hash, entry);
        }

        if dropped_contracts > 0 {
            warn!(
                count = dropped_contracts,
                "dropping unused trigger bytecode entries"
            );
        }
        if fixed_counts > 0 {
            warn!(
                count = fixed_counts,
                "repairing trigger bytecode reference counts"
            );
        }

        let set = Set::default();
        // Use a block + transaction to mutate storages safely
        {
            let mut block = set.block();
            let mut tx = block.transaction();
            for (k, v) in data {
                tx.data_triggers.insert(k, v);
            }
            for (k, v) in pipeline {
                tx.pipeline_triggers.insert(k, v);
            }
            for (k, v) in time {
                tx.time_triggers.insert(k, v);
            }
            for (k, v) in by_call {
                tx.by_call_triggers.insert(k, v);
            }
            for (k, v) in ids {
                tx.ids.insert(k, v);
            }
            for (k, v) in repaired_contracts {
                tx.contracts.insert(k, v);
            }
            tx.apply();
            block.commit();
        }
        Ok(set)
    }
}

#[cfg(test)]
mod dto_tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{HashOf, KeyPair};
    use iroha_data_model::{
        events::pipeline,
        prelude as dm,
        prelude::{BlockStatus, ExecutionTime, IvmBytecode, Log, SetKeyValue},
    };
    use iroha_primitives::const_vec::ConstVec;
    use mv::storage::StorageReadOnly;
    use norito::json;

    use super::*;

    fn sample_set() -> Set {
        let domain: dm::DomainId = "wonderland".parse().unwrap();
        let authority: dm::AccountId =
            dm::AccountId::new(domain, KeyPair::random().public_key().clone());

        let set = Set::default();
        {
            let mut block = set.block();
            let mut tx = block.transaction();

            // Data trigger with Instruction executable
            let data_id: dm::TriggerId = "data1".parse().unwrap();
            let data_filter = dm::DataEventFilter::Any;
            let instr =
                dm::InstructionBox::from(dm::Log::new(dm::Level::INFO, "hello".to_string()));
            let exec = dm::Executable::Instructions(ConstVec::from(vec![instr]));
            let action = SpecializedAction::new(
                exec,
                dm::Repeats::Exactly(1),
                authority.clone(),
                data_filter,
            );
            let trig = SpecializedTrigger::new(data_id, action);
            tx.add_data_trigger(trig).expect("add data trigger");

            // Pipeline trigger with BlockEventFilter variant
            let pipe_id: dm::TriggerId = "pipe1".parse().unwrap();
            let block_filter = pipeline::BlockEventFilter {
                height: Some(NonZeroU64::new(5).unwrap()),
                status: Some(BlockStatus::Committed),
            };
            let pipe_filter: dm::PipelineEventFilterBox = block_filter.into();
            let key: dm::Name = "k1".parse().unwrap();
            let val = dm::Json::new("v1");
            let set_kv = SetKeyValue::account(authority.clone(), key, val);
            let log = Log::new(dm::Level::INFO, "pipeline".to_string());
            let exec2 = dm::Executable::Instructions(ConstVec::from(vec![
                dm::InstructionBox::from(log),
                dm::InstructionBox::from(set_kv),
            ]));
            let action2 = SpecializedAction::new(
                exec2,
                dm::Repeats::Exactly(3),
                authority.clone(),
                pipe_filter,
            );
            let trig2 = SpecializedTrigger::new(pipe_id, action2);
            tx.add_pipeline_trigger(trig2)
                .expect("add pipeline trigger");

            // Time trigger at PreCommit with empty executable
            let time_id: dm::TriggerId = "time1".parse().unwrap();
            let time_filter = dm::TimeEventFilter(ExecutionTime::PreCommit);
            let exec3 =
                dm::Executable::Instructions(ConstVec::from(Vec::<dm::InstructionBox>::new()));
            let action3 = SpecializedAction::new(
                exec3,
                dm::Repeats::Exactly(1),
                authority.clone(),
                time_filter,
            );
            let trig3 = SpecializedTrigger::new(time_id, action3);
            tx.add_time_trigger(trig3).expect("add time trigger");

            // Execute-by-call trigger with IVM executable
            let call_id: dm::TriggerId = "call1".parse().unwrap();
            let call_filter = dm::ExecuteTriggerEventFilter::new();
            let ivm_code = IvmBytecode::from_compiled(vec![0xAA, 0xBB]);
            let exec4 = dm::Executable::Ivm(ivm_code);
            let action4 =
                SpecializedAction::new(exec4, dm::Repeats::Exactly(1), authority, call_filter);
            let trig4 = SpecializedTrigger::new(call_id, action4);
            tx.add_by_call_trigger(trig4).expect("add by-call trigger");

            tx.apply();
            block.commit();
        }

        set
    }

    #[test]
    fn empty_set_roundtrip_dto() {
        let set = Set::default();
        let dto = SetDto::from(&set);
        let bytes = dto.encode().expect("encode dto");
        let dto2 = SetDto::decode(&bytes).expect("decode dto");
        let set2 = Set::try_from(dto2).expect("dto to set");
        let dto3 = SetDto::from(&set2);
        assert_eq!(dto3.data.len(), 0);
        assert_eq!(dto3.pipeline.len(), 0);
        assert_eq!(dto3.time.len(), 0);
        assert_eq!(dto3.by_call.len(), 0);
        assert_eq!(dto3.ids.len(), 0);
        assert_eq!(dto3.contracts.len(), 0);
    }

    #[test]
    fn non_empty_set_roundtrip_dto() {
        let set = sample_set();
        let dto = SetDto::from(&set);
        assert_eq!(dto.data.len(), 1);
        assert_eq!(dto.pipeline.len(), 1);
        assert_eq!(dto.time.len(), 1);
        assert_eq!(dto.by_call.len(), 1);
        assert_eq!(dto.ids.len(), 4);
        assert_eq!(dto.contracts.len(), 1);

        let bytes = dto.encode().expect("encode dto");
        let dto2 = SetDto::decode(&bytes).expect("decode dto");

        // Reconstruct Set (full reconstruction path)
        let set2 = Set::try_from(dto2).expect("dto to set");
        let dto3 = SetDto::from(&set2);
        assert_eq!(dto3.data.len(), 1);
        assert_eq!(dto3.pipeline.len(), 1);
        assert_eq!(dto3.time.len(), 1);
        assert_eq!(dto3.by_call.len(), 1);
        assert_eq!(dto3.ids.len(), 4);
        assert_eq!(dto3.contracts.len(), 1);
    }

    #[test]
    fn set_dto_repairs_inconsistent_storage() {
        let domain: dm::DomainId = "wonderland".parse().unwrap();
        let authority: dm::AccountId =
            dm::AccountId::new(domain, KeyPair::random().public_key().clone());

        let missing_code = IvmBytecode::from_compiled(vec![0x01]);
        let missing_hash = HashOf::new(&missing_code);
        let valid_code = IvmBytecode::from_compiled(vec![0xAA]);
        let valid_hash = HashOf::new(&valid_code);
        let extra_code = IvmBytecode::from_compiled(vec![0xBB]);
        let extra_hash = HashOf::new(&extra_code);

        let data_id: dm::TriggerId = "data_missing".parse().unwrap();
        let call_id: dm::TriggerId = "call_valid".parse().unwrap();
        let orphan_id: dm::TriggerId = "orphan".parse().unwrap();

        let data_action = LoadedActionDto {
            executable: ExecutableRefDto::Ivm(missing_hash),
            repeats: dm::Repeats::Exactly(1),
            authority: authority.clone(),
            filter: dm::DataEventFilter::Any,
            metadata: dm::Metadata::default(),
        };
        let call_action = LoadedActionDto {
            executable: ExecutableRefDto::Ivm(valid_hash),
            repeats: dm::Repeats::Exactly(1),
            authority,
            filter: dm::ExecuteTriggerEventFilter::new(),
            metadata: dm::Metadata::default(),
        };

        let dto = SetDto {
            data: vec![(data_id.clone(), data_action)],
            pipeline: Vec::new(),
            time: Vec::new(),
            by_call: vec![(call_id.clone(), call_action)],
            ids: vec![
                (data_id.clone(), TriggeringEventType::Pipeline),
                (call_id.clone(), TriggeringEventType::ExecuteTrigger),
                (orphan_id.clone(), TriggeringEventType::Data),
            ],
            contracts: vec![
                (
                    valid_hash,
                    IvmBytecodeEntryDto {
                        original_contract: valid_code,
                        count: 9,
                    },
                ),
                (
                    extra_hash,
                    IvmBytecodeEntryDto {
                        original_contract: extra_code,
                        count: 1,
                    },
                ),
            ],
        };

        let set = Set::try_from(dto).expect("dto to set");
        let view = set.view();

        assert!(view.data_triggers().get(&data_id).is_none());
        assert!(view.by_call_triggers().get(&call_id).is_some());
        assert!(view.ids().get(&data_id).is_none());
        assert_eq!(
            view.ids().get(&call_id),
            Some(&TriggeringEventType::ExecuteTrigger)
        );
        assert!(view.ids().get(&orphan_id).is_none());
        let entry = view
            .contracts()
            .get(&valid_hash)
            .expect("valid contract should remain");
        assert_eq!(entry.count.get(), 1);
        assert!(view.contracts().get(&extra_hash).is_none());
    }

    #[test]
    fn set_json_roundtrip_matches_dto() {
        let set = sample_set();
        let json_repr = json::to_json(&set).expect("serialize set to json");
        let decoded: Set = json::from_json(&json_repr).expect("deserialize set from json");

        let original = SetDto::from(&set)
            .encode()
            .expect("encode original set dto");
        let decoded_bytes = SetDto::from(&decoded)
            .encode()
            .expect("encode decoded set dto");

        assert_eq!(original, decoded_bytes);
    }

    #[test]
    fn ivm_entry_rejects_zero_count() {
        let contract = IvmBytecode::from_compiled(vec![1, 2, 3, 4]);
        let encoded_contract = json::to_json(&contract).expect("encode contract");
        let candidate = format!("{{\"original_contract\":{encoded_contract},\"count\":0}}");

        let err = json::from_json::<IvmBytecodeEntry>(&candidate)
            .expect_err("zero count must produce error");

        match err {
            json::Error::InvalidField { field, .. } => assert_eq!(field, "count"),
            other => panic!("unexpected error {other}"),
        }
    }
}
