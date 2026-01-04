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
use std::{fmt, num::NonZeroU64};

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
    /// [`IvmBytecode`] if applicable
    fn get_original_action<F>(&self, action: LoadedAction<F>) -> SpecializedAction<F>
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
                let original_contract = self
                    .get_original_contract(blob_hash)
                    .cloned()
                    .expect("No original smartcontract saved for trigger. This is a bug.");
                Executable::Ivm(original_contract)
            }
            ExecutableRef::Instructions(isi) => Executable::Instructions(isi),
        };

        let mut specialized =
            SpecializedAction::new(original_executable, repeats, authority, filter);
        specialized.metadata = metadata;
        specialized
    }

    /// Get all contained trigger ids without a particular order
    #[inline]
    fn ids_iter(&self) -> impl Iterator<Item = &TriggerId> {
        self.ids().iter().map(|(trigger_id, _)| trigger_id)
    }

    /// Get [`ExecutableRef`] for given [`TriggerId`].
    /// Returns `None` if `id` is not in the set.
    fn get_executable(&self, id: &TriggerId) -> Option<&ExecutableRef> {
        let event_type = self.ids().get(id)?;

        Some(match event_type {
            TriggeringEventType::Data => {
                &self
                    .data_triggers()
                    .get(id)
                    .expect("`Set::data_triggers` doesn't contain required id. This is a bug")
                    .executable
            }
            TriggeringEventType::Pipeline => {
                &self
                    .pipeline_triggers()
                    .get(id)
                    .expect("`Set::pipeline_triggers` doesn't contain required id. This is a bug")
                    .executable
            }
            TriggeringEventType::Time => {
                &self
                    .time_triggers()
                    .get(id)
                    .expect("`Set::time_triggers` doesn't contain required id. This is a bug")
                    .executable
            }
            TriggeringEventType::ExecuteTrigger => {
                &self
                    .by_call_triggers()
                    .get(id)
                    .expect("`Set::by_call_triggers` doesn't contain required id. This is a bug")
                    .executable
            }
        })
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
                    let action = self
                        .data_triggers()
                        .get(id)
                        .expect("`Set::data_triggers` doesn't contain required id. This is a bug");
                    filter(action).then(|| f(id, action))
                }
                TriggeringEventType::Pipeline => {
                    let action = self.pipeline_triggers().get(id).expect(
                        "`Set::pipeline_triggers` doesn't contain required id. This is a bug",
                    );
                    filter(action).then(|| f(id, action))
                }
                TriggeringEventType::Time => {
                    let action = self
                        .time_triggers()
                        .get(id)
                        .expect("`Set::time_triggers` doesn't contain required id. This is a bug");
                    filter(action).then(|| f(id, action))
                }
                TriggeringEventType::ExecuteTrigger => {
                    let action = self.by_call_triggers().get(id).expect(
                        "`Set::by_call_triggers` doesn't contain required id. This is a bug",
                    );
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
            TriggeringEventType::Data => self
                .data_triggers()
                .get(id)
                .map(|entry| f(entry))
                .expect("`Set::data_triggers` doesn't contain required id. This is a bug"),
            TriggeringEventType::Pipeline => self
                .pipeline_triggers()
                .get(id)
                .map(|entry| f(entry))
                .expect("`Set::pipeline_triggers` doesn't contain required id. This is a bug"),
            TriggeringEventType::Time => self
                .time_triggers()
                .get(id)
                .map(|entry| f(entry))
                .expect("`Set::time_triggers` doesn't contain required id. This is a bug"),
            TriggeringEventType::ExecuteTrigger => self
                .by_call_triggers()
                .get(id)
                .map(|entry| f(entry))
                .expect("`Set::by_call_triggers` doesn't contain required id. This is a bug"),
        };
        Some(result)
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
        _current_block_time_ms: u64,
    ) -> impl Iterator<Item = (TriggerId, LoadedAction<TimeEventFilter>)> + '_ {
        let key_height = "__registered_block_height".parse::<Name>().ok();
        self.time_triggers.iter().flat_map(move |(id, action)| {
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
                    match registered_height {
                        Some(height) => height != current_block_height,
                        None => false,
                    }
                })
        })
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
            TriggeringEventType::Data => self
                .data_triggers
                .get_mut(id)
                .map(|entry| f(entry))
                .expect("`Set::data_triggers` doesn't contain required id. This is a bug"),
            TriggeringEventType::Pipeline => self
                .pipeline_triggers
                .get_mut(id)
                .map(|entry| f(entry))
                .expect("`Set::pipeline_triggers` doesn't contain required id. This is a bug"),
            TriggeringEventType::Time => self
                .time_triggers
                .get_mut(id)
                .map(|entry| f(entry))
                .expect("`Set::time_triggers` doesn't contain required id. This is a bug"),
            TriggeringEventType::ExecuteTrigger => self
                .by_call_triggers
                .get_mut(id)
                .map(|entry| f(entry))
                .expect("`Set::by_call_triggers` doesn't contain required id. This is a bug"),
        };
        Some(result)
    }

    /// Remove a trigger from the [`Set`].
    ///
    /// Return `false` if [`Set`] doesn't contain the trigger with the given `id`.
    ///
    /// # Panics
    ///
    /// Panics on inconsistent state of [`Set`]. This is a bug.
    pub fn remove(&mut self, id: TriggerId) -> bool {
        let Some(event_type) = self.ids.remove(id.clone()) else {
            return false;
        };

        let removed = match event_type {
            TriggeringEventType::Data => {
                Self::remove_from(&mut self.contracts, &mut self.data_triggers, id)
            }
            TriggeringEventType::Pipeline => {
                Self::remove_from(&mut self.contracts, &mut self.pipeline_triggers, id)
            }
            TriggeringEventType::Time => {
                Self::remove_from(&mut self.contracts, &mut self.time_triggers, id)
            }
            TriggeringEventType::ExecuteTrigger => {
                Self::remove_from(&mut self.contracts, &mut self.by_call_triggers, id)
            }
        };

        assert!(
            removed,
            "`Set`'s `ids` and typed trigger collections are inconsistent. This is a bug"
        );

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
    ///
    /// # Panics
    ///
    /// Panics if `blob_hash` is not in the [`Set::contracts`].
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
                panic!("`Set::contracts` doesn't contain required hash. This is a bug")
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
            ids.remove(id.clone())
                .and_then(|_| Self::remove_from(contracts, triggers, id.clone()).then_some(()))
                .expect("`Set`'s `ids`, `contracts` and typed trigger collections are inconsistent. This is a bug")
        }

        removed.append(&mut to_remove);
    }
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

    use iroha_data_model::{
        metadata::Metadata,
        prelude::{
            AccountId, Executable, ExecutionTime, InstructionBox, Level, Log, TimeEvent,
            TimeEventFilter, TimeInterval, TriggerId,
        },
    };
    use iroha_primitives::{const_vec::ConstVec, json::Json};

    use super::*;

    fn sample_hash() -> HashOf<IvmBytecode> {
        let bytecode = IvmBytecode::from_compiled(vec![0x01, 0x02, 0x03]);
        HashOf::new(&bytecode)
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
        let interval = TimeInterval::new_since_to(
            Duration::from_millis(0),
            Duration::from_millis(1_234),
        );
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

impl TryFrom<SetDto> for Set {
    type Error = String;
    fn try_from(dto: SetDto) -> Result<Self, Self::Error> {
        let SetDto {
            data,
            pipeline,
            time,
            by_call,
            ids,
            contracts,
        } = dto;

        let set = Set::default();
        // Use a block + transaction to mutate storages safely
        {
            let mut block = set.block();
            let mut tx = block.transaction();
            for (k, v) in data {
                tx.data_triggers.insert(k, LoadedAction::try_from(v)?);
            }
            for (k, v) in pipeline {
                tx.pipeline_triggers.insert(k, LoadedAction::try_from(v)?);
            }
            for (k, v) in time {
                tx.time_triggers.insert(k, LoadedAction::try_from(v)?);
            }
            for (k, v) in by_call {
                tx.by_call_triggers.insert(k, LoadedAction::try_from(v)?);
            }
            for (k, v) in ids {
                tx.ids.insert(k, v);
            }
            for (k, v) in contracts {
                tx.contracts.insert(k, IvmBytecodeEntry::try_from(v)?);
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

    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        events::pipeline,
        prelude as dm,
        prelude::{BlockStatus, ExecutionTime, IvmBytecode, Log, SetKeyValue},
    };
    use iroha_primitives::const_vec::ConstVec;
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
