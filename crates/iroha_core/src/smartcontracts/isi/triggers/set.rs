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
use std::{fmt, marker::PhantomData, num::NonZeroU64};

use iroha_crypto::HashOf;
use iroha_data_model::{
    events::EventFilter,
    isi::error::{InstructionExecutionError, MathError},
    prelude::*,
    query::error::FindError,
    transaction::WasmSmartContract,
};
use iroha_primitives::const_vec::ConstVec;
use mv::storage::{
    Block as StorageBlock, Storage, StorageReadOnly, Transaction as StorageTransaction,
    View as StorageView,
};
use serde::{
    de::{DeserializeSeed, MapAccess, Visitor},
    Deserialize, Serialize,
};
use thiserror::Error;

use crate::{
    smartcontracts::{
        isi::triggers::specialized::{
            LoadedAction, LoadedActionTrait, SpecializedAction, SpecializedTrigger,
        },
        wasm,
    },
    state::deserialize::WasmSeed,
};

/// Error type for [`Set`] operations.
#[derive(Debug, Error, displaydoc::Display)]
pub enum Error {
    /// Failed to preload wasm trigger
    Preload(#[from] wasm::error::Error),
}

/// Result type for [`Set`] operations.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// [`WasmSmartContract`]s by [`TriggerId`].
/// Stored together with number to count triggers with identical [`WasmSmartContract`].
type WasmSmartContractMap = Storage<HashOf<WasmSmartContract>, WasmSmartContractEntry>;
type WasmSmartContractMapBlock<'set> =
    StorageBlock<'set, HashOf<WasmSmartContract>, WasmSmartContractEntry>;
type WasmSmartContractMapTransaction<'block, 'set> =
    StorageTransaction<'block, 'set, HashOf<WasmSmartContract>, WasmSmartContractEntry>;
type WasmSmartContractMapView<'set> =
    StorageView<'set, HashOf<WasmSmartContract>, WasmSmartContractEntry>;

/// Specialized structure that maps event filters to Triggers.
// NB: `Set` has custom `Serialize` and `DeserializeSeed` implementations
// which need to be manually updated when changing the struct
#[derive(Default, Serialize)]
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
    /// [`WasmSmartContract`]s map by wasm blob hash.
    /// This map serves multiple purposes:
    /// 1. Querying original wasm blob of trigger
    /// 2. Getting compiled by wasmtime module for execution
    /// 3. Deduplicating triggers with the same wasm blob
    contracts: WasmSmartContractMap,
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
    /// Original [`WasmSmartContract`]s by [`TriggerId`] for querying purposes.
    contracts: WasmSmartContractMapBlock<'set>,
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
    /// Original [`WasmSmartContract`]s by [`TriggerId`] for querying purposes.
    contracts: WasmSmartContractMapTransaction<'block, 'set>,
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
    /// Original [`WasmSmartContract`]s by [`TriggerId`] for querying purposes.
    contracts: WasmSmartContractMapView<'set>,
}

/// Entry in wasm smart-contracts map
#[derive(Debug, Clone, Serialize)]
pub struct WasmSmartContractEntry {
    /// Original wasm binary blob
    original_contract: WasmSmartContract,
    /// Compiled with [`wasmtime`] smart-contract
    #[serde(skip)]
    compiled_contract: wasmtime::Module,
    /// Number of times this contract is used
    count: NonZeroU64,
}

impl<'de> DeserializeSeed<'de> for WasmSeed<'_, Set> {
    type Value = Set;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SetVisitor<'e> {
            loader: WasmSeed<'e, Set>,
        }

        impl<'de> Visitor<'de> for SetVisitor<'_> {
            type Value = Set;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Set")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut data_triggers = None;
                let mut pipeline_triggers = None;
                let mut time_triggers = None;
                let mut by_call_triggers = None;
                let mut ids = None;
                let mut contracts = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "data_triggers" => {
                            data_triggers = Some(map.next_value()?);
                        }
                        "pipeline_triggers" => {
                            pipeline_triggers = Some(map.next_value()?);
                        }
                        "time_triggers" => {
                            time_triggers = Some(map.next_value()?);
                        }
                        "by_call_triggers" => {
                            by_call_triggers = Some(map.next_value()?);
                        }
                        "ids" => {
                            ids = Some(map.next_value()?);
                        }
                        "contracts" => {
                            contracts = Some(map.next_value_seed(mv::serde::StorageSeeded {
                                kseed: PhantomData,
                                vseed: self.loader.cast::<WasmSmartContractEntry>(),
                            })?);
                        }
                        _ => { /* Ignore unknown fields */ }
                    }
                }

                Ok(Set {
                    data_triggers: data_triggers
                        .ok_or_else(|| serde::de::Error::missing_field("data_triggers"))?,
                    pipeline_triggers: pipeline_triggers
                        .ok_or_else(|| serde::de::Error::missing_field("pipeline_triggers"))?,
                    time_triggers: time_triggers
                        .ok_or_else(|| serde::de::Error::missing_field("time_triggers"))?,
                    by_call_triggers: by_call_triggers
                        .ok_or_else(|| serde::de::Error::missing_field("by_call_triggers"))?,
                    ids: ids.ok_or_else(|| serde::de::Error::missing_field("ids"))?,
                    contracts: contracts
                        .ok_or_else(|| serde::de::Error::missing_field("contracts"))?,
                })
            }
        }

        deserializer.deserialize_map(SetVisitor { loader: self })
    }
}

impl<'de> DeserializeSeed<'de> for WasmSeed<'_, WasmSmartContractEntry> {
    type Value = WasmSmartContractEntry;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct WasmSmartContractEntryVisitor<'e> {
            loader: WasmSeed<'e, WasmSmartContractEntry>,
        }

        impl<'de> Visitor<'de> for WasmSmartContractEntryVisitor<'_> {
            type Value = WasmSmartContractEntry;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct WasmSmartContractEntry")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut original_contract = None;
                let mut count = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "original_contract" => {
                            original_contract = Some(map.next_value()?);
                        }
                        "count" => {
                            count = Some(map.next_value()?);
                        }
                        _ => { /* Ignore unknown fields */ }
                    }
                }

                let original_contract = original_contract
                    .ok_or_else(|| serde::de::Error::missing_field("original_contract"))?;
                let count = count.ok_or_else(|| serde::de::Error::missing_field("count"))?;
                let compiled_contract = wasm::load_module(self.loader.engine, &original_contract)
                    .map_err(serde::de::Error::custom)?;

                Ok(WasmSmartContractEntry {
                    original_contract,
                    compiled_contract,
                    count,
                })
            }
        }

        deserializer.deserialize_map(WasmSmartContractEntryVisitor { loader: self })
    }
}
/// Trait to perform read-only operations on [`SetBlock`], [`SetTransaction`] and [`SetView`]
#[allow(missing_docs)]
pub trait SetReadOnly {
    fn data_triggers(&self) -> &impl StorageReadOnly<TriggerId, LoadedAction<DataEventFilter>>;
    fn pipeline_triggers(
        &self,
    ) -> &impl StorageReadOnly<TriggerId, LoadedAction<PipelineEventFilterBox>>;
    fn time_triggers(&self) -> &impl StorageReadOnly<TriggerId, LoadedAction<TimeEventFilter>>;
    fn by_call_triggers(
        &self,
    ) -> &impl StorageReadOnly<TriggerId, LoadedAction<ExecuteTriggerEventFilter>>;
    fn ids(&self) -> &impl StorageReadOnly<TriggerId, TriggeringEventType>;
    fn contracts(&self)
        -> &impl StorageReadOnly<HashOf<WasmSmartContract>, WasmSmartContractEntry>;

    /// Get original [`WasmSmartContract`] for [`TriggerId`].
    /// Returns `None` if there's no [`Trigger`]
    /// with specified `id` that has WASM executable
    #[inline]
    fn get_original_contract(
        &self,
        hash: &HashOf<WasmSmartContract>,
    ) -> Option<&WasmSmartContract> {
        self.contracts()
            .get(hash)
            .map(|entry| &entry.original_contract)
    }

    /// Get compiled [`wasmtime::Module`] for [`TriggerId`].
    /// Returns `None` if there's no [`Trigger`]
    /// with specified `id` that has WASM executable
    #[inline]
    fn get_compiled_contract(&self, hash: &HashOf<WasmSmartContract>) -> Option<&wasmtime::Module> {
        self.contracts()
            .get(hash)
            .map(|entry| &entry.compiled_contract)
    }

    /// Convert [`LoadedAction`] to original [`Action`] by retrieving original
    /// [`WasmSmartContract`] if applicable
    fn get_original_action<F: Clone>(&self, action: LoadedAction<F>) -> SpecializedAction<F> {
        let LoadedAction {
            executable,
            repeats,
            authority,
            filter,
            metadata,
        } = action;

        let original_executable = match executable {
            ExecutableRef::Wasm(ref blob_hash) => {
                let original_wasm = self
                    .get_original_contract(blob_hash)
                    .cloned()
                    .expect("No original smartcontract saved for trigger. This is a bug.");
                Executable::Wasm(original_wasm)
            }
            ExecutableRef::Instructions(isi) => Executable::Instructions(isi),
        };

        SpecializedAction {
            executable: original_executable,
            repeats,
            authority,
            filter,
            metadata,
        }
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
            fn contracts(&self) -> &impl StorageReadOnly<HashOf<WasmSmartContract>, WasmSmartContractEntry> {
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
    ) -> impl Iterator<Item = (TriggerId, LoadedAction<TimeEventFilter>)> + '_ {
        self.time_triggers.iter().flat_map(move |(id, action)| {
            let mut count = action.filter.count_matches(&event);
            if let Repeats::Exactly(repeats) = action.repeats {
                count = min(repeats, count);
            }
            (0..count).map(move |_| (id.clone(), action.clone()))
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
    /// Return [`Err`] if failed to preload wasm trigger
    #[inline]
    pub fn add_data_trigger(
        &mut self,
        engine: &wasmtime::Engine,
        trigger: SpecializedTrigger<DataEventFilter>,
    ) -> Result<bool> {
        self.add_to(engine, trigger, TriggeringEventType::Data, |me| {
            &mut me.data_triggers
        })
    }

    /// Add trigger with [`PipelineEventFilterBox`]
    ///
    /// Return `false` if a trigger with given id already exists
    ///
    /// # Errors
    ///
    /// Return [`Err`] if failed to preload wasm trigger
    #[inline]
    pub fn add_pipeline_trigger(
        &mut self,
        engine: &wasmtime::Engine,
        trigger: SpecializedTrigger<PipelineEventFilterBox>,
    ) -> Result<bool> {
        self.add_to(engine, trigger, TriggeringEventType::Pipeline, |me| {
            &mut me.pipeline_triggers
        })
    }

    /// Add trigger with [`TimeEventFilter`]
    ///
    /// Returns `false` if a trigger with given id already exists
    ///
    /// # Errors
    ///
    /// Return [`Err`] if failed to preload wasm trigger
    #[inline]
    pub fn add_time_trigger(
        &mut self,
        engine: &wasmtime::Engine,
        trigger: SpecializedTrigger<TimeEventFilter>,
    ) -> Result<bool> {
        self.add_to(engine, trigger, TriggeringEventType::Time, |me| {
            &mut me.time_triggers
        })
    }

    /// Add trigger with [`ExecuteTriggerEventFilter`]
    ///
    /// Returns `false` if a trigger with given id already exists
    ///
    /// # Errors
    ///
    /// Return [`Err`] if failed to preload wasm trigger
    #[inline]
    pub fn add_by_call_trigger(
        &mut self,
        engine: &wasmtime::Engine,
        trigger: SpecializedTrigger<ExecuteTriggerEventFilter>,
    ) -> Result<bool> {
        self.add_to(engine, trigger, TriggeringEventType::ExecuteTrigger, |me| {
            &mut me.by_call_triggers
        })
    }

    /// Add generic trigger to generic collection
    ///
    /// Returns `false` if a trigger with given id already exists
    ///
    /// # Errors
    ///
    /// Return [`Err`] if failed to preload wasm trigger
    fn add_to<F: TriggeringEventFilter + mv::Value>(
        &mut self,
        engine: &wasmtime::Engine,
        trigger: SpecializedTrigger<F>,
        event_type: TriggeringEventType,
        map: impl FnOnce(&mut Self) -> &mut StorageTransaction<'block, 'set, TriggerId, LoadedAction<F>>,
    ) -> Result<bool> {
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
            return Ok(false);
        }

        let loaded_executable = match executable {
            Executable::Wasm(bytes) => {
                let hash = HashOf::new(&bytes);
                // Store original executable representation to respond to queries with.
                if let Some(WasmSmartContractEntry { count, .. }) = self.contracts.get_mut(&hash) {
                    // Considering 1 trigger registration takes 1 second,
                    // it would take 584 942 417 355 years to overflow.
                    *count = count.checked_add(1).expect(
                        "There is no way someone could register 2^64 amount of same triggers",
                    );
                    // Cloning module is cheap, under Arc inside
                } else {
                    let module = wasm::load_module(engine, &bytes)?;
                    self.contracts.insert(
                        hash,
                        WasmSmartContractEntry {
                            original_contract: bytes,
                            compiled_contract: module,
                            count: NonZeroU64::MIN,
                        },
                    );
                }
                ExecutableRef::Wasm(hash)
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
        Ok(true)
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

    /// Remove trigger from `triggers` and decrease the counter of the original [`WasmSmartContract`].
    ///
    /// Note that this function doesn't remove the trigger from [`Set::ids`].
    ///
    /// Returns `true` if trigger was removed and `false` otherwise.
    fn remove_from<F: mv::Value + EventFilter>(
        contracts: &mut WasmSmartContractMapTransaction<'block, 'set>,
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

    /// Decrease the counter of the original [`WasmSmartContract`] by `blob_hash`
    /// or remove it if the counter reaches zero.
    ///
    /// # Panics
    ///
    /// Panics if `blob_hash` is not in the [`Set::contracts`].
    fn remove_original_trigger(
        contracts: &mut WasmSmartContractMapTransaction,
        blob_hash: HashOf<WasmSmartContract>,
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
        contracts: &mut WasmSmartContractMapTransaction<'block, 'set>,
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
/// [`Wasm`](iroha_data_model::transaction::Executable::Wasm) contains hash of the WASM blob
/// Which can be used to obtain compiled by `wasmtime` module
#[derive(Clone, Serialize, Deserialize)]
pub enum ExecutableRef {
    /// Loaded WASM
    Wasm(HashOf<WasmSmartContract>),
    /// Vector of ISI
    Instructions(ConstVec<InstructionBox>),
}

impl core::fmt::Debug for ExecutableRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wasm(hash) => f.debug_tuple("Wasm").field(hash).finish(),
            Self::Instructions(instructions) => {
                f.debug_tuple("Instructions").field(instructions).finish()
            }
        }
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
