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
use std::{collections::HashMap, fmt};

use iroha_crypto::HashOf;
use iroha_data_model::{
    events::Filter as EventFilter,
    isi::error::{InstructionExecutionError, MathError},
    prelude::*,
    query::error::FindError,
    trigger::{action::ActionTrait, OptimizedExecutable, Trigger, WasmInternalRepr},
};
use serde::{
    de::{DeserializeSeed, MapAccess, Visitor},
    ser::{SerializeMap, SerializeStruct},
    Serialize, Serializer,
};
use thiserror::Error;

use crate::{smartcontracts::wasm, wsv::WasmSeed};

/// Error type for [`Set`] operations.
#[derive(Debug, Error, displaydoc::Display)]
pub enum Error {
    /// Failed to preload wasm trigger
    Preload(#[from] wasm::error::Error),
}

/// Result type for [`Set`] operations.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Type of action with pre-loaded executable.
pub type LoadedAction<F> = Action<F, LoadedExecutable>;

/// Specialized structure that maps event filters to Triggers.
// NB: `Set` has custom `Serialize` and `DeserializeSeed` implementations
// which need to be manually updated when changing the struct
#[derive(Debug, Default)]
pub struct Set {
    /// Triggers using [`DataEventFilter`]
    data_triggers: HashMap<TriggerId, LoadedAction<DataEventFilter>>,
    /// Triggers using [`PipelineEventFilter`]
    pipeline_triggers: HashMap<TriggerId, LoadedAction<PipelineEventFilter>>,
    /// Triggers using [`TimeEventFilter`]
    time_triggers: HashMap<TriggerId, LoadedAction<TimeEventFilter>>,
    /// Triggers using [`ExecuteTriggerEventFilter`]
    by_call_triggers: HashMap<TriggerId, LoadedAction<ExecuteTriggerEventFilter>>,
    /// Trigger ids with type of events they process
    ids: HashMap<TriggerId, TriggeringEventType>,
    /// Original [`WasmSmartContract`]s by [`TriggerId`] for querying purposes.
    original_contracts: HashMap<TriggerId, WasmSmartContract>,
    /// List of actions that should be triggered by events provided by `handle_*` methods.
    /// Vector is used to save the exact triggers order.
    matched_ids: Vec<(Event, TriggerId)>,
}

/// Helper struct for serializing triggers.
struct TriggersWithContext<'s, F> {
    /// Triggers being serialized
    triggers: &'s HashMap<TriggerId, LoadedAction<F>>,
    /// Containing Set, used for looking up origignal [`WasmSmartContract`]s
    /// during serialization.
    set: &'s Set,
}

impl<'s, F> TriggersWithContext<'s, F> {
    fn new(triggers: &'s HashMap<TriggerId, LoadedAction<F>>, set: &'s Set) -> Self {
        Self { triggers, set }
    }
}

impl<F: Clone + Serialize> Serialize for TriggersWithContext<'_, F> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.triggers.len()))?;
        for (id, action) in self.triggers.iter() {
            let action = self.set.get_original_action(id, action.clone());
            map.serialize_entry(&id, &action)?;
        }
        map.end()
    }
}

impl Serialize for Set {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut set = serializer.serialize_struct("Set", 6)?;
        set.serialize_field(
            "data_triggers",
            &TriggersWithContext::new(&self.data_triggers, self),
        )?;
        set.serialize_field(
            "pipeline_triggers",
            &TriggersWithContext::new(&self.pipeline_triggers, self),
        )?;
        set.serialize_field(
            "time_triggers",
            &TriggersWithContext::new(&self.time_triggers, self),
        )?;
        set.serialize_field(
            "by_call_triggers",
            &TriggersWithContext::new(&self.by_call_triggers, self),
        )?;
        set.serialize_field("ids", &self.ids)?;
        set.end()
    }
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
                let mut set = Set::default();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "data_triggers" => {
                            let triggers: HashMap<TriggerId, Action<DataEventFilter, Executable>> =
                                map.next_value()?;
                            for (id, action) in triggers {
                                set.add_data_trigger(self.loader.engine, Trigger::new(id, action))
                                    .unwrap();
                            }
                        }
                        "pipeline_triggers" => {
                            let triggers: HashMap<
                                TriggerId,
                                Action<PipelineEventFilter, Executable>,
                            > = map.next_value()?;
                            for (id, action) in triggers {
                                set.add_pipeline_trigger(
                                    self.loader.engine,
                                    Trigger::new(id, action),
                                )
                                .unwrap();
                            }
                        }
                        "time_triggers" => {
                            let triggers: HashMap<TriggerId, Action<TimeEventFilter, Executable>> =
                                map.next_value()?;
                            for (id, action) in triggers {
                                set.add_time_trigger(self.loader.engine, Trigger::new(id, action))
                                    .unwrap();
                            }
                        }
                        "by_call_triggers" => {
                            let triggers: HashMap<
                                TriggerId,
                                Action<ExecuteTriggerEventFilter, Executable>,
                            > = map.next_value()?;
                            for (id, action) in triggers {
                                set.add_by_call_trigger(
                                    self.loader.engine,
                                    Trigger::new(id, action),
                                )
                                .unwrap();
                            }
                        }
                        "ids" => {
                            set.ids = map.next_value()?;
                        }
                        _ => { /* Ignore unknown fields */ }
                    }
                }

                Ok(set)
            }
        }

        deserializer.deserialize_map(SetVisitor { loader: self })
    }
}

impl Clone for Set {
    fn clone(&self) -> Self {
        Self {
            data_triggers: self.data_triggers.clone(),
            pipeline_triggers: self.pipeline_triggers.clone(),
            time_triggers: self.time_triggers.clone(),
            by_call_triggers: self.by_call_triggers.clone(),
            ids: self.ids.clone(),
            original_contracts: self.original_contracts.clone(),
            matched_ids: Vec::default(),
        }
    }
}

impl Set {
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
        trigger: Trigger<DataEventFilter, Executable>,
    ) -> Result<bool> {
        self.add_to(engine, trigger, TriggeringEventType::Data, |me| {
            &mut me.data_triggers
        })
    }

    /// Add trigger with [`PipelineEventFilter`]
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
        trigger: Trigger<PipelineEventFilter, Executable>,
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
        trigger: Trigger<TimeEventFilter, Executable>,
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
        trigger: Trigger<ExecuteTriggerEventFilter, Executable>,
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
    fn add_to<F: Filter>(
        &mut self,
        engine: &wasmtime::Engine,
        trigger: Trigger<F, Executable>,
        event_type: TriggeringEventType,
        map: impl FnOnce(&mut Self) -> &mut HashMap<TriggerId, LoadedAction<F>>,
    ) -> Result<bool> {
        if self.contains(trigger.id()) {
            return Ok(false);
        }

        let trigger_id = trigger.id;
        let Action {
            executable,
            repeats,
            authority,
            filter,
            metadata,
        } = trigger.action;

        let loaded_executable = match executable {
            Executable::Wasm(bytes) => {
                let loaded = LoadedExecutable::Wasm(LoadedWasm {
                    module: wasm::load_module(engine, &bytes)?,
                    blob_hash: HashOf::new(&bytes),
                });
                // Store original executable representation to respond to queries with.
                self.original_contracts.insert(trigger_id.clone(), bytes);
                loaded
            }
            Executable::Instructions(instructions) => LoadedExecutable::Instructions(instructions),
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

    /// Get original [`WasmSmartContract`] for [`TriggerId`].
    /// Returns `None` if there's no [`Trigger`]
    /// with specified `id` that has WASM executable
    #[inline]
    pub fn get_original_contract(&self, id: &TriggerId) -> Option<&WasmSmartContract> {
        self.original_contracts.get(id)
    }

    fn get_original_action<F: Clone>(
        &self,
        id: &TriggerId,
        action: LoadedAction<F>,
    ) -> Action<F, Executable> {
        let Action {
            executable,
            repeats,
            authority,
            filter,
            metadata,
        } = action;

        let original_executable = match executable {
            LoadedExecutable::Wasm(_) => {
                let original_wasm = self
                    .get_original_contract(id)
                    .cloned()
                    .expect("No original smartcontract saved for trigger. This is a bug.");
                Executable::Wasm(original_wasm)
            }
            LoadedExecutable::Instructions(isi) => Executable::Instructions(isi),
        };

        Action {
            executable: original_executable,
            repeats,
            authority,
            filter,
            metadata,
        }
    }

    /// Get all contained trigger ids without a particular order
    #[inline]
    pub fn ids(&self) -> impl ExactSizeIterator<Item = &TriggerId> {
        self.ids.keys()
    }

    /// Apply `f` to triggers that belong to the given [`DomainId`]
    ///
    /// Return an empty list if [`Set`] doesn't contain any triggers belonging to [`DomainId`].
    pub fn inspect_by_domain_id<'a, F: 'a, R>(
        &'a self,
        domain_id: &DomainId,
        f: F,
    ) -> impl Iterator<Item = R> + '_
    where
        F: Fn(&TriggerId, &dyn ActionTrait<Executable = LoadedExecutable>) -> R,
    {
        let domain_id = domain_id.clone();

        self.ids.iter().filter_map(move |(id, event_type)| {
            let trigger_domain_id = id.domain_id.as_ref()?;

            if *trigger_domain_id != domain_id {
                return None;
            }

            let result = match event_type {
                TriggeringEventType::Data => self
                    .data_triggers
                    .get(id)
                    .map(|trigger| f(id, trigger))
                    .expect("`Set::data_triggers` doesn't contain required id. This is a bug"),
                TriggeringEventType::Pipeline => self
                    .pipeline_triggers
                    .get(id)
                    .map(|trigger| f(id, trigger))
                    .expect("`Set::pipeline_triggers` doesn't contain required id. This is a bug"),
                TriggeringEventType::Time => self
                    .time_triggers
                    .get(id)
                    .map(|trigger| f(id, trigger))
                    .expect("`Set::time_triggers` doesn't contain required id. This is a bug"),
                TriggeringEventType::ExecuteTrigger => self
                    .by_call_triggers
                    .get(id)
                    .map(|trigger| f(id, trigger))
                    .expect("`Set::by_call_triggers` doesn't contain required id. This is a bug"),
            };

            Some(result)
        })
    }

    /// Apply `f` to the trigger identified by `id`.
    ///
    /// Return [`None`] if [`Set`] doesn't contain the trigger with the given `id`.
    pub fn inspect_by_id<F, R>(&self, id: &TriggerId, f: F) -> Option<R>
    where
        F: Fn(&dyn ActionTrait<Executable = LoadedExecutable>) -> R,
    {
        let event_type = self.ids.get(id).copied()?;

        let result = match event_type {
            TriggeringEventType::Data => self
                .data_triggers
                .get(id)
                .map(|entry| f(entry))
                .expect("`Set::data_triggers` doesn't contain required id. This is a bug"),
            TriggeringEventType::Pipeline => self
                .pipeline_triggers
                .get(id)
                .map(|entry| f(entry))
                .expect("`Set::pipeline_triggers` doesn't contain required id. This is a bug"),
            TriggeringEventType::Time => self
                .time_triggers
                .get(id)
                .map(|entry| f(entry))
                .expect("`Set::time_triggers` doesn't contain required id. This is a bug"),
            TriggeringEventType::ExecuteTrigger => self
                .by_call_triggers
                .get(id)
                .map(|entry| f(entry))
                .expect("`Set::by_call_triggers` doesn't contain required id. This is a bug"),
        };
        Some(result)
    }

    /// Apply `f` to the trigger identified by `id`.
    ///
    /// Return [`None`] if [`Set`] doesn't contain the trigger with the given `id`.
    pub fn inspect_by_id_mut<F, R>(&mut self, id: &TriggerId, f: F) -> Option<R>
    where
        F: Fn(&mut dyn ActionTrait<Executable = LoadedExecutable>) -> R,
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
    pub fn remove(&mut self, id: &TriggerId) -> bool {
        self.original_contracts.remove(id);
        self.ids
            .remove(id)
            .map(|event_type| match event_type {
                TriggeringEventType::Data => self
                    .data_triggers
                    .remove(id)
                    .map(|_| ())
                    .expect("`Set::data_triggers` doesn't contain required id. This is a bug"),
                TriggeringEventType::Pipeline => {
                    self.pipeline_triggers.remove(id).map(|_| ()).expect(
                        "`Set::pipeline_triggers` doesn't contain required id. This is a bug",
                    )
                }
                TriggeringEventType::Time => self
                    .time_triggers
                    .remove(id)
                    .map(|_| ())
                    .expect("`Set::time_triggers` doesn't contain required id. This is a bug"),
                TriggeringEventType::ExecuteTrigger => {
                    self.by_call_triggers.remove(id).map(|_| ()).expect(
                        "`Set::by_call_triggers` doesn't contain required id. This is a bug",
                    )
                }
            })
            .is_some()
    }

    /// Check if [`Set`] contains `id`.
    #[inline]
    pub fn contains(&self, id: &TriggerId) -> bool {
        self.ids.contains_key(id)
    }

    /// Modify repetitions of the hook identified by [`Id`].
    ///
    /// # Errors
    ///
    /// - If a trigger with the given id is not found.
    /// - If updating the current trigger `repeats` causes an overflow. Indefinitely
    /// repeating triggers and triggers set for exact time always cause an overflow.
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

    /// Handle [`DataEvent`].
    ///
    /// Finds all actions, that are triggered by `event` and stores them.
    /// This actions will be inspected in the next [`Set::handle_data_event()`] call
    // Passing by value to follow other `handle_` methods interface
    #[allow(clippy::needless_pass_by_value)]
    pub fn handle_data_event(&mut self, event: DataEvent) {
        self.data_triggers
            .iter()
            .filter(|(id, _)| id.domain_id.is_none() || id.domain_id.as_ref() == event.domain_id())
            .for_each(|entry| {
                Self::match_and_insert_trigger(&mut self.matched_ids, event.clone(), entry)
            });
    }

    /// Handle [`ExecuteTriggerEvent`].
    ///
    /// Find all actions that are triggered by `event` and store them.
    /// These actions are inspected in the next [`Set::inspect_matched()`] call.
    pub fn handle_execute_trigger_event(&mut self, event: ExecuteTriggerEvent) {
        if let Some(action) = self.by_call_triggers.get(&event.trigger_id) {
            let id = event.trigger_id.clone();
            Self::match_and_insert_trigger(&mut self.matched_ids, event, (&id, action));
        };
    }

    /// Handle [`PipelineEvent`].
    ///
    /// Find all actions that are triggered by `event` and store them.
    /// These actions are inspected in the next [`Set::inspect_matched()`] call.
    // Passing by value to follow other `handle_` methods interface
    #[allow(clippy::needless_pass_by_value)]
    pub fn handle_pipeline_event(&mut self, event: PipelineEvent) {
        self.pipeline_triggers.iter().for_each(|entry| {
            Self::match_and_insert_trigger(&mut self.matched_ids, event.clone(), entry)
        });
    }

    /// Handle [`TimeEvent`].
    ///
    /// Find all actions that are triggered by `event` and store them.
    /// These actions are inspected in the next [`Set::inspect_matched()`] call.
    pub fn handle_time_event(&mut self, event: TimeEvent) {
        for (id, action) in &self.time_triggers {
            let mut count = action.filter.count_matches(&event);
            if let Repeats::Exactly(repeats) = action.repeats {
                count = min(repeats, count);
            }
            if count == 0 {
                continue;
            }

            let ids = core::iter::repeat_with(|| (Event::Time(event), id.clone())).take(
                count
                    .try_into()
                    .expect("`u32` should always fit in `usize`"),
            );
            self.matched_ids.extend(ids);
        }
    }

    /// Match and insert a [`TriggerId`] into the set of matched ids.
    ///
    /// Skips insertion:
    /// - If the action's filter doesn't match an event
    /// - If the action's repeats count equals to 0
    fn match_and_insert_trigger<E: Into<Event>, F: EventFilter<Event = E>>(
        matched_ids: &mut Vec<(Event, TriggerId)>,
        event: E,
        (id, action): (&TriggerId, &LoadedAction<F>),
    ) {
        if !action.filter.matches(&event) {
            return;
        }

        if let Repeats::Exactly(repeats) = action.repeats {
            if repeats == 0 {
                return;
            }
        }

        matched_ids.push((event.into(), id.clone()));
    }

    /// Decrease `action`s for provided triggers and remove those whose counter reached zero.
    pub fn decrease_repeats(&mut self, triggers: &[TriggerId]) {
        for id in triggers {
            // Ignoring error if trigger has not `Repeats::Exact(_)` but something else
            let _mod_repeats_res = self.mod_repeats(id, |n| Ok(n.saturating_sub(1)));
        }

        let Self {
            data_triggers,
            pipeline_triggers,
            time_triggers,
            by_call_triggers,
            ids,
            ..
        } = self;
        Self::remove_zeros(ids, data_triggers);
        Self::remove_zeros(ids, pipeline_triggers);
        Self::remove_zeros(ids, time_triggers);
        Self::remove_zeros(ids, by_call_triggers);
    }

    /// Remove actions with zero execution count from `triggers`
    fn remove_zeros<F: Filter>(
        ids: &mut HashMap<TriggerId, TriggeringEventType>,
        triggers: &mut HashMap<TriggerId, LoadedAction<F>>,
    ) {
        let to_remove: Vec<TriggerId> = triggers
            .iter()
            .filter_map(|(id, action)| {
                if let Repeats::Exactly(repeats) = action.repeats {
                    if repeats == 0 {
                        return Some(id.clone());
                    }
                }
                None
            })
            .collect();

        for id in to_remove {
            triggers.remove(&id).and_then(|_| ids.remove(&id)).expect(
                "Removing existing keys from `Set` should be always possible. This is a bug",
            );
        }
    }

    /// Extract `matched_id`
    pub fn extract_matched_ids(&mut self) -> Vec<(Event, TriggerId)> {
        core::mem::take(&mut self.matched_ids)
    }
}

/// WASM blob loaded with `wasmtime`
#[derive(Clone)]
pub struct LoadedWasm {
    /// Loaded Module
    pub module: wasmtime::Module,
    /// Hash of original WASM blob on blockchain
    pub blob_hash: HashOf<WasmSmartContract>,
}

/// Same as [`Executable`](iroha_data_model::transaction::Executable), but instead of
/// [`Wasm`](iroha_data_model::transaction::Executable::Wasm) contains WASM module as loaded
/// by `wasmtime`
#[derive(Clone)]
pub enum LoadedExecutable {
    /// Loaded WASM
    Wasm(LoadedWasm),
    /// Vector of ISI
    Instructions(Vec<InstructionExpr>),
}

impl core::fmt::Debug for LoadedExecutable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wasm(_) => f
                .debug_tuple("Wasm")
                .field(&"<Wasm module is truncated>")
                .finish(),
            Self::Instructions(instructions) => {
                f.debug_tuple("Instructions").field(instructions).finish()
            }
        }
    }
}

impl From<LoadedExecutable> for OptimizedExecutable {
    fn from(executable: LoadedExecutable) -> Self {
        match executable {
            LoadedExecutable::Wasm(LoadedWasm { module, blob_hash }) => {
                OptimizedExecutable::WasmInternalRepr(WasmInternalRepr {
                    serialized: module
                        .serialize()
                        .expect("Serialization of optimized wasm module should always succeed"),
                    blob_hash,
                })
            }
            LoadedExecutable::Instructions(instructions) => {
                OptimizedExecutable::Instructions(instructions)
            }
        }
    }
}

/// [`Set::mod_repeats()`] error
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
