//! Lightweight/fast DSL used under the `fast_dsl` feature.
//!
//! This mirrors the public surface of `query::dsl` so other modules can import
//! the same names regardless of feature flags. Predicate payloads are captured
//! and evaluated via JSON when enabled; selector projections remain minimal.

#[cfg(feature = "ids_projection")]
use std::any::TypeId as StdTypeId;
use std::{any::Any, marker::PhantomData, sync::Arc};

use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId};
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json::{self, JsonSerialize, Value};

#[cfg(feature = "ids_projection")]
use crate::Identifiable;
#[cfg(feature = "json")]
use crate::query::json::{EqualsCondition, InCondition, PredicateJson};
pub use crate::query::tx_predicate::CommittedTxPredicate;
use crate::query::tx_predicate::committed_tx_filters_from_json;

/// Marker for predicate projections.
#[derive(Debug, Clone, Copy)]
pub struct PredicateMarker;

/// Marker for selector projections.
#[derive(Debug, Clone, Copy)]
pub struct SelectorMarker;

/// Projectable type: all types are projectable with unit atoms.
pub trait Projectable<Marker> {
    /// Lightweight atom used to seed a projection.
    type AtomType;
}

impl<T, Marker> Projectable<Marker> for T {
    type AtomType = ();
}

/// Projection capability: every type has a unit projection.
pub trait HasProjection<Marker>: Projectable<Marker> {
    /// Output produced by the projection.
    type Projection;
    /// Construct a projection from the provided atom (no-op in lightweight mode).
    fn atom(_: Self::AtomType) -> Self::Projection;
}

impl<T, Marker> HasProjection<Marker> for T {
    type Projection = ();
    fn atom((): Self::AtomType) -> Self::Projection {}
}

/// Prototype provider used at compile-time in builder APIs.
pub trait HasPrototype {
    /// Prototype builder used by the DSL scaffolding.
    type Prototype<Marker, Projector>: Default;
}

/// Default prototype used by the lightweight DSL builders.
#[derive(Copy, Clone)]
pub struct Prototype<Marker, Projector>(PhantomData<(Marker, Projector)>);

impl<Marker, Projector> Default for Prototype<Marker, Projector> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T> HasPrototype for T {
    type Prototype<Marker, Projector> = Prototype<Marker, Projector>;
}

/// Object projector that passes projections through unchanged.
#[derive(Copy, Clone)]
pub struct BaseProjector<Marker, T>(PhantomData<(Marker, T)>);

impl<Marker, T> Default for BaseProjector<Marker, T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

/// Convert predicate builder outputs into a concrete predicate.
pub trait IntoPredicate<T> {
    /// Convert into a compound predicate for `T`.
    fn into_predicate(self) -> CompoundPredicate<T>;
}

impl<T> IntoPredicate<T> for CompoundPredicate<T> {
    fn into_predicate(self) -> CompoundPredicate<T> {
        self
    }
}

impl<T> IntoPredicate<T> for () {
    fn into_predicate(self) -> CompoundPredicate<T> {
        CompoundPredicate::PASS
    }
}

#[cfg(feature = "json")]
/// Convert predicate values into Norito JSON values.
pub trait IntoPredicateValue {
    /// Convert into a JSON value for predicate evaluation.
    fn into_value(self) -> Value;
}

#[cfg(feature = "json")]
impl<T> IntoPredicateValue for T
where
    T: JsonSerialize,
{
    fn into_value(self) -> Value {
        json::to_value(&self).expect("predicate value serialize")
    }
}

#[cfg(feature = "json")]
/// Builder for JSON predicate payloads.
#[derive(Debug, Clone)]
pub struct PredicateBuilder<T> {
    predicate: PredicateJson,
    marker: PhantomData<T>,
}

#[cfg(feature = "json")]
impl<T> Default for PredicateBuilder<T> {
    fn default() -> Self {
        Self {
            predicate: PredicateJson::default(),
            marker: PhantomData,
        }
    }
}

#[cfg(feature = "json")]
impl<T> PredicateBuilder<T> {
    /// Add an equality predicate on the provided field path.
    #[must_use]
    pub fn equals(mut self, field: impl Into<String>, value: impl IntoPredicateValue) -> Self {
        self.predicate
            .equals
            .push(EqualsCondition::new(field, value.into_value()));
        self
    }

    /// Add a membership predicate on the provided field path.
    #[must_use]
    pub fn in_values<I, V>(mut self, field: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: IntoPredicateValue,
    {
        let values: Vec<_> = values
            .into_iter()
            .map(IntoPredicateValue::into_value)
            .collect();
        if !values.is_empty() {
            self.predicate.r#in.push(InCondition::new(field, values));
        }
        self
    }

    /// Add an existence predicate on the provided field path.
    #[must_use]
    pub fn exists(mut self, field: impl Into<String>) -> Self {
        self.predicate.exists.push(field.into());
        self
    }
}

#[cfg(feature = "json")]
impl<T> IntoPredicate<T> for PredicateBuilder<T> {
    fn into_predicate(self) -> CompoundPredicate<T> {
        if self.predicate.is_empty() {
            return CompoundPredicate::PASS;
        }
        self.predicate
            .into_compound::<T>()
            .unwrap_or(CompoundPredicate::PASS)
    }
}

#[cfg(feature = "json")]
impl<T> IntoPredicate<T> for PredicateJson {
    fn into_predicate(self) -> CompoundPredicate<T> {
        if self.is_empty() {
            return CompoundPredicate::PASS;
        }
        self.into_compound::<T>().unwrap_or(CompoundPredicate::PASS)
    }
}

#[cfg(feature = "ids_projection")]
#[derive(Debug, Copy, Clone)]
/// Marker returned from selector builders to request ids-only projection.
pub struct SelectorField<T>(PhantomData<T>);

#[cfg(feature = "json")]
impl<T> Prototype<PredicateMarker, BaseProjector<PredicateMarker, T>> {
    /// Start an equality predicate for the provided field path.
    #[must_use]
    pub fn equals(
        &self,
        field: impl Into<String>,
        value: impl IntoPredicateValue,
    ) -> PredicateBuilder<T> {
        PredicateBuilder::default().equals(field, value)
    }

    /// Start a membership predicate for the provided field path.
    #[must_use]
    pub fn in_values<I, V>(&self, field: impl Into<String>, values: I) -> PredicateBuilder<T>
    where
        I: IntoIterator<Item = V>,
        V: IntoPredicateValue,
    {
        PredicateBuilder::default().in_values(field, values)
    }

    /// Start an existence predicate for the provided field path.
    #[must_use]
    pub fn exists(&self, field: impl Into<String>) -> PredicateBuilder<T> {
        PredicateBuilder::default().exists(field)
    }
}

#[cfg(feature = "ids_projection")]
impl<T> Prototype<SelectorMarker, BaseProjector<SelectorMarker, T>>
where
    T: Identifiable,
{
    /// Request ids-only projection for the selected type.
    #[must_use]
    pub fn ids_only(&self) -> SelectorField<T> {
        SelectorField(PhantomData)
    }
}

/// Lightweight predicate container.
///
/// Serialized with a stable wire wrapper so predicate payloads remain consistent
/// while still carrying runtime filter data.
#[derive(Debug)]
pub struct CompoundPredicate<T> {
    payload: Option<Arc<dyn Any + Send + Sync + 'static>>,
    marker: PhantomData<T>,
}

#[derive(Clone)]
struct PredicateJsonPayload {
    raw: String,
}

impl PredicateJsonPayload {
    #[cfg(feature = "json")]
    fn from_value(value: &Value) -> Result<Self, norito::json::Error> {
        if let Ok(predicate) = PredicateJson::try_from_value(value) {
            let raw = norito::json::to_json(&predicate)?;
            return Ok(Self { raw });
        }
        let raw = norito::json::to_json(value)?;
        Ok(Self { raw })
    }

    fn from_raw(raw: String) -> Self {
        Self { raw }
    }

    fn as_str(&self) -> &str {
        &self.raw
    }
}

impl<T> PartialEq for CompoundPredicate<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T> Eq for CompoundPredicate<T> {}

impl<T> Clone for CompoundPredicate<T> {
    fn clone(&self) -> Self {
        Self {
            payload: self.payload.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> CompoundPredicate<T> {
    /// Predicate representing a pass-through (true) condition.
    pub const PASS: Self = Self {
        payload: None,
        marker: PhantomData,
    };

    #[inline]
    #[must_use]
    /// Combine two predicates via logical AND.
    pub fn and(self, other: Self) -> Self {
        match (self.payload, other.payload) {
            (None, None) => Self::PASS,
            (None, Some(payload)) | (Some(payload), None) => Self::with_payload(payload),
            (Some(left), Some(right)) => {
                #[cfg(feature = "json")]
                if let (Some(left_json), Some(right_json)) = (
                    left.as_ref().downcast_ref::<PredicateJsonPayload>(),
                    right.as_ref().downcast_ref::<PredicateJsonPayload>(),
                ) && let (Some(left_pred), Some(right_pred)) = (
                    predicate_json_from_raw(left_json.as_str()),
                    predicate_json_from_raw(right_json.as_str()),
                ) {
                    let merged = merge_predicate_json(left_pred, right_pred);
                    return merged.into_predicate();
                }

                if let (Some(left_tree), Some(right_tree)) = (
                    left.as_ref().downcast_ref::<CommittedTxPredicate>(),
                    right.as_ref().downcast_ref::<CommittedTxPredicate>(),
                ) {
                    let merged = and_committed_tx_predicates(left_tree.clone(), right_tree.clone());
                    return Self::with_payload(Arc::new(merged));
                }

                if let (Some(left_filters), Some(right_filters)) = (
                    left.as_ref()
                        .downcast_ref::<crate::query::CommittedTxFilters>(),
                    right
                        .as_ref()
                        .downcast_ref::<crate::query::CommittedTxFilters>(),
                ) {
                    let left_tree = committed_tx_predicate_from_filters(left_filters);
                    let right_tree = committed_tx_predicate_from_filters(right_filters);
                    let merged = and_committed_tx_predicates(left_tree, right_tree);
                    return Self::with_payload(Arc::new(merged));
                }

                if let (Some(filters), Some(tree)) = (
                    left.as_ref()
                        .downcast_ref::<crate::query::CommittedTxFilters>(),
                    right.as_ref().downcast_ref::<CommittedTxPredicate>(),
                ) {
                    let left_tree = committed_tx_predicate_from_filters(filters);
                    let merged = and_committed_tx_predicates(left_tree, tree.clone());
                    return Self::with_payload(Arc::new(merged));
                }

                if let (Some(tree), Some(filters)) = (
                    left.as_ref().downcast_ref::<CommittedTxPredicate>(),
                    right
                        .as_ref()
                        .downcast_ref::<crate::query::CommittedTxFilters>(),
                ) {
                    let right_tree = committed_tx_predicate_from_filters(filters);
                    let merged = and_committed_tx_predicates(tree.clone(), right_tree);
                    return Self::with_payload(Arc::new(merged));
                }

                // Mixed payloads are not merged; prefer the most recently supplied predicate.
                Self::with_payload(right)
            }
        }
    }

    #[inline]
    /// Evaluate the provided closure and convert the result into a predicate.
    pub fn build<F, P>(f: F) -> Self
    where
        T: HasPrototype,
        F: FnOnce(
            <T as HasPrototype>::Prototype<PredicateMarker, BaseProjector<PredicateMarker, T>>,
        ) -> P,
        <T as HasPrototype>::Prototype<PredicateMarker, BaseProjector<PredicateMarker, T>>: Default,
        P: IntoPredicate<T>,
    {
        f(Default::default()).into_predicate()
    }

    fn with_payload(payload: Arc<dyn Any + Send + Sync + 'static>) -> Self {
        Self {
            payload: Some(payload),
            marker: PhantomData,
        }
    }

    #[cfg(feature = "json")]
    fn from_json_value(value: &Value) -> Result<Self, norito::json::Error> {
        let payload = PredicateJsonPayload::from_value(value)?;
        Ok(Self::with_payload(Arc::new(payload)))
    }

    fn from_json_raw(raw: String) -> Self {
        let payload = PredicateJsonPayload::from_raw(raw);
        Self::with_payload(Arc::new(payload))
    }

    fn to_wire(&self) -> CompoundPredicateWire {
        if let Some(payload) = self.payload.as_ref() {
            if let Some(filters) = payload.downcast_ref::<crate::query::CommittedTxFilters>() {
                return CompoundPredicateWire::TxFilters(Box::new(filters.clone()));
            }
            if let Some(tree) = payload.downcast_ref::<CommittedTxPredicate>() {
                return CompoundPredicateWire::TxPredicate(tree.clone());
            }
            if let Some(json) = payload.downcast_ref::<PredicateJsonPayload>() {
                return CompoundPredicateWire::Json(json.as_str().to_owned());
            }
        }
        CompoundPredicateWire::Pass
    }

    fn from_wire(wire: CompoundPredicateWire) -> Self {
        match wire {
            CompoundPredicateWire::Pass => Self::PASS,
            CompoundPredicateWire::Json(raw) => Self::from_json_raw(raw),
            CompoundPredicateWire::TxFilters(filters) => Self::with_payload(Arc::new(*filters)),
            CompoundPredicateWire::TxPredicate(tree) => Self::with_payload(Arc::new(tree)),
        }
    }

    #[cfg(feature = "json")]
    /// Return the raw JSON payload embedded in this predicate, if the predicate originated from JSON.
    pub fn json_payload(&self) -> Option<&str> {
        self.payload
            .as_ref()
            .and_then(|p| p.downcast_ref::<PredicateJsonPayload>())
            .map(PredicateJsonPayload::as_str)
    }
}

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize)]
enum CompoundPredicateWire {
    Pass,
    Json(String),
    TxFilters(Box<crate::query::CommittedTxFilters>),
    TxPredicate(CommittedTxPredicate),
}

impl<T> norito::core::NoritoSerialize for CompoundPredicate<T> {
    fn serialize<W: std::io::Write>(&self, mut writer: W) -> Result<(), norito::core::Error> {
        let wire = self.to_wire();
        norito::core::NoritoSerialize::serialize(&wire, &mut writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.to_wire().encoded_len_hint()
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.to_wire().encoded_len_exact()
    }
}

impl<'de, T> norito::core::NoritoDeserialize<'de> for CompoundPredicate<T> {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("compound predicate wire should deserialize")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let wire_archived = archived.cast::<CompoundPredicateWire>();
        let wire = <CompoundPredicateWire as norito::core::NoritoDeserialize>::try_deserialize(
            wire_archived,
        )?;
        Ok(Self::from_wire(wire))
    }
}

#[cfg(feature = "json")]
impl<T> norito::json::JsonSerialize for CompoundPredicate<T> {
    fn json_serialize(&self, out: &mut String) {
        if let Some(payload) = self.payload.as_ref()
            && let Some(json) = payload.downcast_ref::<PredicateJsonPayload>()
        {
            out.push_str(json.as_str());
            return;
        }
        out.push('{');
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl<T> norito::json::JsonDeserialize for CompoundPredicate<T> {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = Value::json_deserialize(parser)?;
        match value {
            Value::Null => Ok(Self::PASS),
            Value::Object(ref map) if map.is_empty() => Ok(Self::PASS),
            other => Self::from_json_value(&other),
        }
    }
}

impl<T: 'static> TypeId for CompoundPredicate<T> {
    fn id() -> iroha_schema::Ident {
        std::any::type_name::<Self>().to_owned()
    }
}

impl<T: 'static> IntoSchema for CompoundPredicate<T> {
    fn type_name() -> iroha_schema::Ident {
        "CompoundPredicate".to_owned()
    }
    fn update_schema_map(m: &mut MetaMap) {
        // Represent as an empty tuple
        m.insert::<Self>(Metadata::Tuple(iroha_schema::UnnamedFieldsMeta {
            types: vec![],
        }));
    }
}

#[cfg(feature = "json")]
fn predicate_json_from_raw(raw: &str) -> Option<PredicateJson> {
    let value = norito::json::from_json::<Value>(raw).ok()?;
    predicate_json_from_value(value)
}

#[cfg(feature = "json")]
fn predicate_json_from_value(value: Value) -> Option<PredicateJson> {
    PredicateJson::try_from_value(&value)
        .ok()
        .or_else(|| predicate_json_from_map(value))
}

#[cfg(feature = "json")]
fn predicate_json_from_map(value: Value) -> Option<PredicateJson> {
    let Value::Object(map) = value else {
        return None;
    };
    let mut predicate = PredicateJson::default();
    for (field, raw_value) in map {
        match raw_value {
            Value::Array(values) if !values.is_empty() => {
                predicate.r#in.push(InCondition::new(field, values));
            }
            other => predicate.equals.push(EqualsCondition::new(field, other)),
        }
    }
    Some(predicate)
}

#[cfg(feature = "json")]
fn merge_predicate_json(mut left: PredicateJson, right: PredicateJson) -> PredicateJson {
    left.equals.extend(right.equals);
    left.r#in.extend(right.r#in);
    left.exists.extend(right.exists);
    left
}

#[cfg(feature = "json")]
fn predicate_value_at_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return None;
    }
    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }
        match current {
            Value::Object(map) => {
                current = map.get(segment)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

#[cfg(feature = "json")]
fn predicate_json_applies(predicate: &PredicateJson, value: &Value) -> bool {
    for cond in &predicate.equals {
        let Some(actual) = predicate_value_at_path(value, &cond.field) else {
            return false;
        };
        if actual != &cond.value {
            return false;
        }
    }
    for cond in &predicate.r#in {
        let Some(actual) = predicate_value_at_path(value, &cond.field) else {
            return false;
        };
        if !cond.values.iter().any(|candidate| candidate == actual) {
            return false;
        }
    }
    for field in &predicate.exists {
        let Some(actual) = predicate_value_at_path(value, field) else {
            return false;
        };
        if actual.is_null() {
            return false;
        }
    }
    true
}

fn committed_tx_predicate_from_filters(
    filters: &crate::query::CommittedTxFilters,
) -> CommittedTxPredicate {
    use CommittedTxPredicate as P;

    let mut parts = Vec::new();
    if let Some(value) = filters.authority_eq.as_ref() {
        parts.push(P::AuthorityEq(value.clone()));
    }
    if let Some(value) = filters.authority_ne.as_ref() {
        parts.push(P::AuthorityNe(value.clone()));
    }
    if !filters.authority_in.is_empty() {
        parts.push(P::AuthorityIn(filters.authority_in.clone()));
    }
    if !filters.authority_nin.is_empty() {
        parts.push(P::AuthorityNin(filters.authority_nin.clone()));
    }
    if let Some(value) = filters.authority_exists {
        parts.push(P::AuthorityExists(value));
    }
    if let Some(value) = filters.ts_ge {
        parts.push(P::TsGte(value));
    }
    if let Some(value) = filters.ts_le {
        parts.push(P::TsLte(value));
    }
    if let Some(value) = filters.entry_eq.as_ref() {
        parts.push(P::EntryEq(*value));
    }
    if let Some(value) = filters.entry_ne.as_ref() {
        parts.push(P::EntryNe(*value));
    }
    if !filters.entry_in.is_empty() {
        parts.push(P::EntryIn(filters.entry_in.clone()));
    }
    if !filters.entry_nin.is_empty() {
        parts.push(P::EntryNin(filters.entry_nin.clone()));
    }
    if let Some(value) = filters.entry_exists {
        parts.push(P::EntryExists(value));
    }
    if let Some(value) = filters.result_ok {
        parts.push(P::ResultEq(value));
    }
    if let Some(value) = filters.result_ok_ne {
        parts.push(P::ResultNe(value));
    }
    if !filters.result_ok_in.is_empty() {
        parts.push(P::ResultIn(filters.result_ok_in.clone()));
    }
    if !filters.result_ok_nin.is_empty() {
        parts.push(P::ResultNin(filters.result_ok_nin.clone()));
    }
    if let Some(value) = filters.result_exists {
        parts.push(P::ResultExists(value));
    }

    match parts.len() {
        0 => P::Const(true),
        1 => parts.into_iter().next().expect("non-empty parts"),
        _ => P::And(parts),
    }
}

fn and_committed_tx_predicates(
    left: CommittedTxPredicate,
    right: CommittedTxPredicate,
) -> CommittedTxPredicate {
    use CommittedTxPredicate as P;
    match (left, right) {
        (P::Const(true), other) | (other, P::Const(true)) => other,
        (P::Const(false), _) | (_, P::Const(false)) => P::Const(false),
        (P::And(mut lhs), P::And(mut rhs)) => {
            lhs.append(&mut rhs);
            P::And(lhs)
        }
        (P::And(mut lhs), rhs) => {
            lhs.push(rhs);
            P::And(lhs)
        }
        (lhs, P::And(mut rhs)) => {
            rhs.insert(0, lhs);
            P::And(rhs)
        }
        (lhs, rhs) => P::And(vec![lhs, rhs]),
    }
}

/// Trait for types that can be evaluated as predicates.
pub trait EvaluatePredicate<U: ?Sized> {
    /// Returns `true` when the predicate matches the supplied input.
    fn applies(&self, _input: &U) -> bool {
        true
    }
}

#[cfg(feature = "json")]
impl<T> EvaluatePredicate<T> for CompoundPredicate<T>
where
    T: JsonSerialize,
{
    fn applies(&self, input: &T) -> bool {
        let Some(payload) = self.payload.as_ref() else {
            return true;
        };
        let Some(json_payload) = payload.downcast_ref::<PredicateJsonPayload>() else {
            return true;
        };
        let Some(predicate) = predicate_json_from_raw(json_payload.as_str()) else {
            return true;
        };
        let Ok(value) = json::to_value(input) else {
            return true;
        };
        predicate_json_applies(&predicate, &value)
    }
}

#[cfg(not(feature = "json"))]
impl<U: ?Sized, T> EvaluatePredicate<U> for CompoundPredicate<T> {}

impl CompoundPredicate<crate::query::CommittedTransaction> {
    /// Build a predicate from a committed-transaction filter set.
    pub fn from_filters(filters: crate::query::CommittedTxFilters) -> Self {
        Self::with_payload(Arc::new(filters))
    }

    /// Build a predicate from a committed-transaction predicate tree.
    pub fn from_committed_tx_predicate(tree: CommittedTxPredicate) -> Self {
        Self::with_payload(Arc::new(tree))
    }

    fn payload_any(&self) -> Option<&Arc<dyn Any + Send + Sync + 'static>> {
        self.payload.as_ref()
    }

    /// Evaluate the predicate against a committed transaction.
    pub fn applies(&self, input: &crate::query::CommittedTransaction) -> bool {
        if let Some(payload) = self.payload_any() {
            if let Some(filters) = payload.downcast_ref::<crate::query::CommittedTxFilters>() {
                return filters.applies(input);
            }
            if let Some(tree) = payload.downcast_ref::<CommittedTxPredicate>() {
                return tree.applies(input);
            }
            if let Some(json) = payload.downcast_ref::<PredicateJsonPayload>()
                && let Some(filters) = committed_tx_filters_from_json(json.as_str())
            {
                return filters.applies(input);
            }
            #[cfg(feature = "json")]
            if let Some(json) = payload.downcast_ref::<PredicateJsonPayload>()
                && let Some(predicate) = predicate_json_from_raw(json.as_str())
                && let Ok(value) = json::to_value(input)
            {
                return predicate_json_applies(&predicate, &value);
            }
        }
        true
    }
}

/// Trait that allows to get the predicate type for a given type.
pub trait HasPredicateAtom {
    /// Predicate type evaluated against the implementor.
    type Predicate: EvaluatePredicate<Self>;
}

impl<T> HasPredicateAtom for T {
    type Predicate = ();
}

impl<T> EvaluatePredicate<T> for () {}

/// Lightweight selector tuple returned by stubbed DSL builders.
#[derive(Debug, PartialEq, Eq, Decode, Encode)]
pub struct SelectorTuple<T>(
    #[cfg(feature = "ids_projection")] SelectorMode,
    PhantomData<T>,
);

// Norito slice decoding delegations for lightweight DSL containers.
impl<'a, T> norito::core::DecodeFromSlice<'a> for CompoundPredicate<T> {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = std::io::Cursor::new(bytes);
        let v: Self = <Self as norito::codec::Decode>::decode(&mut cursor)?;
        let used = usize::try_from(cursor.position())
            .map_err(|_| norito::core::Error::from("cursor position does not fit usize"))?;
        Ok((v, used))
    }
}

impl<'a, T> norito::core::DecodeFromSlice<'a> for SelectorTuple<T> {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = std::io::Cursor::new(bytes);
        let v: Self = <Self as norito::codec::Decode>::decode(&mut cursor)?;
        let used = usize::try_from(cursor.position())
            .map_err(|_| norito::core::Error::from("cursor position does not fit usize"))?;
        Ok((v, used))
    }
}

/// Experimental selector mode to prototype basic projections.
#[cfg(feature = "ids_projection")]
#[derive(Debug, Clone, Copy, Decode, Encode, PartialEq, Eq)]
/// Controls how selector tuples project data when the experimental ids projection feature is enabled.
pub enum SelectorMode {
    /// Request the full object for each row.
    Full,
    /// Request identifiers only.
    IdsOnly,
}

impl<T> SelectorTuple<T> {
    #[inline]
    /// Build a selector tuple using the provided closure.
    pub fn build<F, O>(f: F) -> Self
    where
        T: HasPrototype,
        F: FnOnce(
            <T as HasPrototype>::Prototype<SelectorMarker, BaseProjector<SelectorMarker, T>>,
        ) -> O,
        <T as HasPrototype>::Prototype<SelectorMarker, BaseProjector<SelectorMarker, T>>: Default,
        O: IntoSelectorTuple<SelectingType = T>,
    {
        f(Default::default()).into_selector_tuple()
    }

    #[inline]
    /// Iterate over the selector payload applied to a batch (always empty in lightweight mode).
    pub fn iter(&self) -> impl Iterator<Item = ()> {
        #[cfg(not(feature = "ids_projection"))]
        {
            core::iter::empty()
        }
        #[cfg(feature = "ids_projection")]
        {
            let count = match self.0 {
                SelectorMode::Full => 0,
                // The actual projector is provided by EvaluateSelector on unit; we emit a single
                // unit value to trigger projection when ids-only is requested.
                SelectorMode::IdsOnly => 1,
            };
            core::iter::repeat(()).take(count)
        }
    }

    /// Construct an ids-only selector (experimental; feature-gated).
    #[cfg(feature = "ids_projection")]
    #[must_use]
    pub fn ids_only() -> Self {
        Self(SelectorMode::IdsOnly, PhantomData)
    }

    /// Returns true if this tuple requests ids-only projection.
    #[cfg(feature = "ids_projection")]
    pub fn is_ids_only(&self) -> bool {
        matches!(self.0, SelectorMode::IdsOnly)
    }
}

#[cfg(feature = "json")]
impl<T> norito::json::JsonSerialize for SelectorTuple<T> {
    fn json_serialize(&self, out: &mut String) {
        #[cfg(feature = "ids_projection")]
        {
            let mode = match self.0 {
                SelectorMode::Full => "Full",
                SelectorMode::IdsOnly => "IdsOnly",
            };
            norito::json::write_json_string(mode, out);
        }
        #[cfg(not(feature = "ids_projection"))]
        {
            Value::Null.json_serialize(out);
        }
    }
}

#[cfg(feature = "json")]
impl<T> norito::json::JsonDeserialize for SelectorTuple<T> {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        #[cfg(feature = "ids_projection")]
        {
            match Value::json_deserialize(parser)? {
                Value::Null => Ok(Self::default()),
                Value::String(s) => {
                    let mode = match s.as_str() {
                        "Full" => SelectorMode::Full,
                        "IdsOnly" => SelectorMode::IdsOnly,
                        other => {
                            return Err(norito::json::Error::Message(format!(
                                "invalid selector mode `{other}`"
                            )));
                        }
                    };
                    Ok(Self(mode, PhantomData))
                }
                other => Err(norito::json::Error::Message(format!(
                    "expected string or null for SelectorTuple, got {other:?}"
                ))),
            }
        }
        #[cfg(not(feature = "ids_projection"))]
        {
            match Value::json_deserialize(parser)? {
                Value::Null => Ok(Self(PhantomData)),
                other => Err(norito::json::Error::Message(format!(
                    "expected null for SelectorTuple, got {other:?}"
                ))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_copy<T: Copy>() {}

    #[test]
    fn predicate_marker_is_copy() {
        assert_copy::<PredicateMarker>();
    }

    #[test]
    fn selector_marker_is_copy() {
        assert_copy::<SelectorMarker>();
    }

    #[test]
    fn selector_tuple_iter_is_empty() {
        let selector = SelectorTuple::<u32>::default();
        assert_eq!(selector.iter().count(), 0);
    }
}

#[cfg(all(test, feature = "json"))]
mod codec_tests {
    use std::time::Duration;

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, MerkleProof};
    use norito::NoritoSerialize;

    use super::*;
    use crate::{
        account, block,
        domain::Domain,
        domain::DomainId,
        prelude as dm,
        query::{self, tx_predicate::CommittedTxPredicate as P},
        transaction,
        transaction::signed,
        trigger,
    };

    fn expect_committed_tx_tree(predicate: CompoundPredicate<query::CommittedTransaction>) -> P {
        match predicate.to_wire() {
            CompoundPredicateWire::TxPredicate(tree) => tree,
            other => panic!(
                "expected committed tx predicate tree variant, got {:?}",
                core::mem::discriminant(&other)
            ),
        }
    }

    fn dummy_block_hash() -> HashOf<block::BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed([0xA5; Hash::LENGTH]))
    }

    fn dummy_proof_entry() -> MerkleProof<transaction::TransactionEntrypoint> {
        MerkleProof::from_audit_path(0, vec![])
    }

    fn dummy_proof_result() -> MerkleProof<transaction::TransactionResult> {
        MerkleProof::from_audit_path(0, vec![])
    }

    #[derive(Clone)]
    struct TestAuthority {
        id: account::AccountId,
        private_key: iroha_crypto::PrivateKey,
    }

    impl TestAuthority {
        fn new(seed: u8) -> Self {
            let _domain = DomainId::try_new("wonderland", "universal").expect("domain");
            let (public_key, private_key) =
                KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519).into_parts();
            let id = account::AccountId::new(public_key);
            Self { id, private_key }
        }
    }

    fn build_ext_tx(
        authority: &TestAuthority,
        ts_ms: u64,
        ok: bool,
        metadata: dm::Metadata,
    ) -> query::CommittedTransaction {
        let chain: dm::ChainId = "test-chain".parse().expect("chain id");
        let mut builder = signed::TransactionBuilder::new(chain, authority.id.clone());
        builder.set_creation_time(Duration::from_millis(ts_ms));
        let signed: signed::SignedTransaction = builder
            .with_metadata(metadata)
            .with_instructions::<dm::InstructionBox>([])
            .sign(&authority.private_key);

        let entry_hash = signed.hash_as_entrypoint();
        let entrypoint = transaction::TransactionEntrypoint::External(signed);
        let result_inner: signed::TransactionResultInner = if ok {
            Ok(trigger::DataTriggerSequence::default())
        } else {
            Err(dm::TransactionRejectionReason::Validation(
                dm::ValidationFail::InternalError("x".into()),
            ))
        };
        let result = signed::TransactionResult(result_inner);
        let result_hash = transaction::TransactionResult::hash_from_inner(&result.0);

        query::CommittedTransaction {
            block_hash: dummy_block_hash(),
            entrypoint_hash: entry_hash,
            entrypoint_proof: dummy_proof_entry(),
            entrypoint,
            result_hash,
            result_proof: dummy_proof_result(),
            result,
        }
    }

    #[test]
    fn compound_predicate_norito_roundtrip_preserves_pass_variant() {
        let predicate = CompoundPredicate::<Domain>::PASS;
        let wire = predicate.to_wire();
        assert!(matches!(wire, CompoundPredicateWire::Pass));
        assert_eq!(predicate.encoded_len_hint(), wire.encoded_len_hint());
        assert_eq!(predicate.encoded_len_exact(), wire.encoded_len_exact());

        let bytes = norito::to_bytes(&predicate).expect("encode pass predicate");
        let decoded: CompoundPredicate<Domain> =
            norito::decode_from_bytes(&bytes).expect("decode pass predicate");

        assert!(matches!(decoded.to_wire(), CompoundPredicateWire::Pass));
        assert_eq!(decoded.json_payload(), None);
    }

    #[test]
    fn compound_predicate_norito_roundtrip_preserves_json_variant() {
        let predicate = CompoundPredicate::<Domain>::build(|p| p.exists("id"));
        let wire = predicate.to_wire();
        assert!(matches!(wire, CompoundPredicateWire::Json(_)));
        assert_eq!(predicate.encoded_len_hint(), wire.encoded_len_hint());
        assert_eq!(predicate.encoded_len_exact(), wire.encoded_len_exact());

        let bytes = norito::to_bytes(&predicate).expect("encode json predicate");
        let decoded: CompoundPredicate<Domain> =
            norito::decode_from_bytes(&bytes).expect("decode json predicate");

        assert!(matches!(decoded.to_wire(), CompoundPredicateWire::Json(_)));
        assert_eq!(decoded.json_payload(), predicate.json_payload());
    }

    #[test]
    fn compound_predicate_and_with_pass_keeps_other_payload() {
        let predicate = CompoundPredicate::<Domain>::build(|p| p.exists("id"));

        let left = CompoundPredicate::<Domain>::PASS.and(predicate.clone());
        let right = predicate.clone().and(CompoundPredicate::<Domain>::PASS);

        assert_eq!(left.json_payload(), predicate.json_payload());
        assert_eq!(right.json_payload(), predicate.json_payload());
    }

    #[test]
    fn compound_predicate_json_deserialize_treats_null_and_empty_object_as_pass() {
        let null_predicate: CompoundPredicate<norito::json::Value> =
            norito::json::from_json("null").expect("null predicate");
        let empty_predicate: CompoundPredicate<norito::json::Value> =
            norito::json::from_json("{}").expect("empty predicate");

        assert!(matches!(
            null_predicate.to_wire(),
            CompoundPredicateWire::Pass
        ));
        assert!(matches!(
            empty_predicate.to_wire(),
            CompoundPredicateWire::Pass
        ));
        assert_eq!(null_predicate.json_payload(), None);
        assert_eq!(empty_predicate.json_payload(), None);
    }

    #[test]
    fn compound_predicate_json_deserialize_keeps_raw_non_object_payload() {
        let predicate: CompoundPredicate<norito::json::Value> =
            norito::json::from_json("[1,2,3]").expect("array predicate");

        assert_eq!(predicate.json_payload(), Some("[1,2,3]"));
        assert!(predicate.applies(&norito::json!({ "id": 1 })));
    }

    #[test]
    fn compound_predicate_invalid_raw_json_defaults_true() {
        let predicate = CompoundPredicate::<norito::json::Value>::from_wire(
            CompoundPredicateWire::Json("{".into()),
        );

        assert_eq!(predicate.json_payload(), Some("{"));
        assert!(predicate.applies(&norito::json!({ "id": 1 })));
    }

    #[test]
    fn committed_tx_compound_predicate_norito_roundtrip_preserves_filters_variant() {
        let predicate = CompoundPredicate::<query::CommittedTransaction>::from_filters(
            query::CommittedTxFilters {
                authority_exists: Some(true),
                result_ok: Some(true),
                ..Default::default()
            },
        );
        let wire = predicate.to_wire();
        assert!(matches!(wire, CompoundPredicateWire::TxFilters(_)));
        assert_eq!(predicate.encoded_len_hint(), wire.encoded_len_hint());
        assert_eq!(predicate.encoded_len_exact(), wire.encoded_len_exact());

        let bytes = norito::to_bytes(&predicate).expect("encode filters predicate");
        let decoded: CompoundPredicate<query::CommittedTransaction> =
            norito::decode_from_bytes(&bytes).expect("decode filters predicate");

        assert!(matches!(
            decoded.to_wire(),
            CompoundPredicateWire::TxFilters(_)
        ));
        let payload = decoded.payload_any().expect("filters payload");
        let filters = payload
            .downcast_ref::<query::CommittedTxFilters>()
            .expect("committed tx filters");
        assert_eq!(filters.authority_exists, Some(true));
        assert_eq!(filters.result_ok, Some(true));
        assert!(filters.entry_in.is_empty());
    }

    #[test]
    fn committed_tx_compound_predicate_norito_roundtrip_preserves_tree_variant() {
        let predicate =
            CompoundPredicate::<query::CommittedTransaction>::from_committed_tx_predicate(P::And(
                vec![P::ResultEq(true), P::TsGte(10)],
            ));
        let wire = predicate.to_wire();
        assert!(matches!(wire, CompoundPredicateWire::TxPredicate(_)));
        assert_eq!(predicate.encoded_len_hint(), wire.encoded_len_hint());
        assert_eq!(predicate.encoded_len_exact(), wire.encoded_len_exact());

        let bytes = norito::to_bytes(&predicate).expect("encode tree predicate");
        let decoded: CompoundPredicate<query::CommittedTransaction> =
            norito::decode_from_bytes(&bytes).expect("decode tree predicate");

        assert!(matches!(
            decoded.to_wire(),
            CompoundPredicateWire::TxPredicate(_)
        ));
        let payload = decoded.payload_any().expect("tree payload");
        let tree = payload
            .downcast_ref::<P>()
            .expect("committed tx predicate tree");
        assert!(
            matches!(tree, P::And(children) if matches!(children.as_slice(), [P::ResultEq(true), P::TsGte(10)]))
        );
    }

    #[test]
    fn committed_tx_compound_predicate_and_merges_filter_pairs() {
        let left = CompoundPredicate::<query::CommittedTransaction>::from_filters(
            query::CommittedTxFilters::default(),
        );
        let right = CompoundPredicate::<query::CommittedTransaction>::from_filters(
            query::CommittedTxFilters {
                result_ok: Some(true),
                ..Default::default()
            },
        );

        let tree = expect_committed_tx_tree(left.and(right));
        assert!(matches!(tree, P::ResultEq(true)));
    }

    #[test]
    fn committed_tx_compound_predicate_and_merges_tree_pairs() {
        let left = CompoundPredicate::<query::CommittedTransaction>::from_committed_tx_predicate(
            P::And(vec![P::ResultEq(true)]),
        );
        let right = CompoundPredicate::<query::CommittedTransaction>::from_committed_tx_predicate(
            P::And(vec![P::TsGte(10), P::EntryExists(true)]),
        );

        let tree = expect_committed_tx_tree(left.and(right));
        assert!(matches!(
            tree,
            P::And(children)
                if matches!(
                    children.as_slice(),
                    [P::ResultEq(true), P::TsGte(10), P::EntryExists(true)]
                )
        ));
    }

    #[test]
    fn committed_tx_compound_predicate_and_merges_filters_with_tree() {
        let left = CompoundPredicate::<query::CommittedTransaction>::from_filters(
            query::CommittedTxFilters {
                authority_exists: Some(true),
                ..Default::default()
            },
        );
        let right = CompoundPredicate::<query::CommittedTransaction>::from_committed_tx_predicate(
            P::TsGte(10),
        );

        let tree = expect_committed_tx_tree(left.and(right));
        assert!(matches!(
            tree,
            P::And(children)
                if matches!(children.as_slice(), [P::AuthorityExists(true), P::TsGte(10)])
        ));
    }

    #[test]
    fn committed_tx_compound_predicate_and_merges_tree_with_filters() {
        let left = CompoundPredicate::<query::CommittedTransaction>::from_committed_tx_predicate(
            P::ResultEq(true),
        );
        let right = CompoundPredicate::<query::CommittedTransaction>::from_filters(
            query::CommittedTxFilters {
                entry_exists: Some(true),
                ..Default::default()
            },
        );

        let tree = expect_committed_tx_tree(left.and(right));
        assert!(matches!(
            tree,
            P::And(children)
                if matches!(children.as_slice(), [P::ResultEq(true), P::EntryExists(true)])
        ));
    }

    #[test]
    fn committed_tx_compound_predicate_json_expression_uses_filter_parser_path() {
        let authority = TestAuthority::new(0x11);
        let ok_tx = build_ext_tx(&authority, 42, true, dm::Metadata::default());
        let err_tx = build_ext_tx(&authority, 42, false, dm::Metadata::default());
        let predicate: CompoundPredicate<query::CommittedTransaction> =
            norito::json::from_value(norito::json!({
                "op": "eq",
                "args": [
                    {"FieldPath": "result_ok"},
                    true
                ]
            }))
            .expect("predicate value");

        assert!(predicate.applies(&ok_tx));
        assert!(!predicate.applies(&err_tx));
    }

    #[test]
    fn committed_tx_compound_predicate_json_map_uses_generic_json_fallback() {
        let authority = TestAuthority::new(0x22);
        let tx = build_ext_tx(&authority, 55, true, dm::Metadata::default());
        let tx_json = norito::json::to_value(&tx).expect("transaction json");
        let result_hash = tx_json
            .as_object()
            .and_then(|map| map.get("result_hash"))
            .cloned()
            .expect("result_hash field");
        let predicate: CompoundPredicate<query::CommittedTransaction> =
            norito::json::from_value(norito::json!({
                "result_hash": result_hash
            }))
            .expect("predicate value");

        assert!(predicate.applies(&tx));
    }

    #[test]
    fn committed_tx_compound_predicate_raw_non_object_defaults_true() {
        let authority = TestAuthority::new(0x33);
        let tx = build_ext_tx(&authority, 77, true, dm::Metadata::default());
        let predicate =
            CompoundPredicate::<query::CommittedTransaction>::from_json_raw("[]".into());

        assert!(predicate.applies(&tx));
    }

    #[test]
    fn committed_tx_compound_predicate_invalid_raw_json_defaults_true() {
        let authority = TestAuthority::new(0x34);
        let tx = build_ext_tx(&authority, 88, true, dm::Metadata::default());
        let predicate = CompoundPredicate::<query::CommittedTransaction>::from_wire(
            CompoundPredicateWire::Json("{".into()),
        );

        assert!(predicate.applies(&tx));
    }

    #[test]
    fn committed_tx_compound_predicate_and_prefers_right_for_mixed_payloads() {
        let json_predicate = CompoundPredicate::<query::CommittedTransaction>::from_json_raw(
            "{\"result_hash\":\"placeholder\"}".into(),
        );
        let tree_predicate =
            CompoundPredicate::<query::CommittedTransaction>::from_committed_tx_predicate(
                P::ResultEq(true),
            );

        let json_then_tree = json_predicate.clone().and(tree_predicate.clone());
        let tree_then_json = tree_predicate.and(json_predicate);

        assert!(matches!(
            json_then_tree.to_wire(),
            CompoundPredicateWire::TxPredicate(P::ResultEq(true))
        ));
        assert!(matches!(
            tree_then_json.to_wire(),
            CompoundPredicateWire::Json(_)
        ));
    }

    #[test]
    fn predicate_value_at_path_rejects_empty_and_blank_segments() {
        let value = norito::json!({
            "outer": { "inner": 1 },
            "null_field": null
        });

        assert!(predicate_value_at_path(&value, "").is_none());
        assert!(predicate_value_at_path(&value, "outer..inner").is_none());
        assert!(matches!(
            predicate_value_at_path(&value, "outer.inner"),
            Some(norito::json::Value::Number(number)) if number.as_u64() == Some(1)
        ));
    }

    #[test]
    fn predicate_json_from_map_treats_empty_arrays_as_equals_conditions() {
        let predicate = predicate_json_from_map(norito::json!({
            "field": []
        }))
        .expect("predicate");

        assert_eq!(predicate.equals.len(), 1);
        assert!(predicate.r#in.is_empty());
        assert_eq!(predicate.equals[0].field, "field");
        assert!(matches!(
            predicate.equals[0].value,
            norito::json::Value::Array(ref values) if values.is_empty()
        ));
    }

    #[test]
    fn predicate_json_applies_rejects_missing_equals_path() {
        let predicate = PredicateJson {
            equals: vec![EqualsCondition::new(
                "outer.missing",
                norito::json::Value::from(1_u64),
            )],
            ..PredicateJson::default()
        };
        let value = norito::json!({
            "outer": { "inner": 1 }
        });

        assert!(!predicate_json_applies(&predicate, &value));
    }

    #[test]
    fn predicate_json_applies_rejects_missing_membership_and_null_exists() {
        let membership = PredicateJson {
            r#in: vec![InCondition::new(
                "outer.missing",
                vec![norito::json::Value::from(1_u64)],
            )],
            ..PredicateJson::default()
        };
        let exists = PredicateJson {
            exists: vec!["null_field".into()],
            ..PredicateJson::default()
        };
        let value = norito::json!({
            "outer": { "inner": 1 },
            "null_field": null
        });

        assert!(!predicate_json_applies(&membership, &value));
        assert!(!predicate_json_applies(&exists, &value));
    }

    #[test]
    fn committed_tx_and_short_circuits_const_false() {
        let left = CompoundPredicate::<query::CommittedTransaction>::from_committed_tx_predicate(
            P::Const(false),
        );
        let right = CompoundPredicate::<query::CommittedTransaction>::from_committed_tx_predicate(
            P::ResultEq(true),
        );

        let tree = expect_committed_tx_tree(left.and(right));
        assert!(matches!(tree, P::Const(false)));
    }

    #[test]
    fn committed_tx_and_appends_rhs_to_existing_and() {
        let predicate = and_committed_tx_predicates(
            P::And(vec![P::AuthorityExists(true), P::TsGte(10)]),
            P::ResultEq(true),
        );

        assert!(matches!(
            predicate,
            P::And(children)
                if matches!(
                    children.as_slice(),
                    [P::AuthorityExists(true), P::TsGte(10), P::ResultEq(true)]
                )
        ));
    }

    #[test]
    fn committed_tx_and_prepends_lhs_to_existing_and() {
        let predicate = and_committed_tx_predicates(
            P::EntryExists(true),
            P::And(vec![P::ResultEq(true), P::TsLte(90)]),
        );

        assert!(matches!(
            predicate,
            P::And(children)
                if matches!(
                    children.as_slice(),
                    [P::EntryExists(true), P::ResultEq(true), P::TsLte(90)]
                )
        ));
    }

    #[test]
    fn committed_tx_predicate_from_filters_preserves_extended_field_order() {
        let authority = TestAuthority::new(0x44);
        let tx = build_ext_tx(&authority, 90, true, dm::Metadata::default());
        let predicate = committed_tx_predicate_from_filters(&query::CommittedTxFilters {
            authority_ne: Some(authority.id.clone()),
            ts_le: Some(90),
            entry_nin: vec![tx.entrypoint_hash],
            result_ok_ne: Some(true),
            result_exists: Some(false),
            ..Default::default()
        });

        assert!(matches!(
            predicate,
            P::And(children)
                if matches!(
                    children.as_slice(),
                    [
                        P::AuthorityNe(_),
                        P::TsLte(90),
                        P::EntryNin(_),
                        P::ResultNe(true),
                        P::ResultExists(false)
                    ]
                )
        ));
    }
}

#[cfg(all(test, feature = "json"))]
mod predicate_tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_primitives::json::Json;
    use norito::json;

    use super::*;
    use crate::{
        Registrable,
        account::AccountId,
        domain::{Domain, DomainId},
        query::json::PredicateJson,
    };

    fn test_authority() -> AccountId {
        let (public_key, _private_key) =
            KeyPair::from_seed(vec![0x24; 32], Algorithm::Ed25519).into_parts();
        AccountId::new(public_key)
    }

    fn sample_domain() -> Domain {
        let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
        let authority = test_authority();
        let mut domain = Domain::new(domain_id).build(&authority);
        domain
            .metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(1_u32));
        domain
            .metadata_mut()
            .insert("label".parse().unwrap(), Json::from("gold"));
        domain
    }

    #[test]
    fn predicate_builder_matches_metadata() {
        let domain = sample_domain();
        let predicate = CompoundPredicate::<Domain>::build(|p| {
            p.equals("metadata.rank", 1_u32)
                .in_values("metadata.label", ["gold", "silver"])
                .exists("metadata.rank")
        });
        assert!(predicate.applies(&domain));
    }

    #[test]
    fn compound_predicate_and_merges_json() {
        let domain = sample_domain();
        let left = CompoundPredicate::<Domain>::build(|p| p.equals("metadata.rank", 1_u32));
        let right = CompoundPredicate::<Domain>::build(|p| p.exists("metadata.label"));
        let combined = left.and(right);
        assert!(combined.applies(&domain));
    }

    #[test]
    fn predicate_json_payload_canonicalizes() {
        let value = norito::json!({
            "equals": [
                {"field": "b", "value": 1},
                {"field": "a", "value": 2}
            ],
            "exists": ["z", "y"]
        });
        let predicate: CompoundPredicate<Domain> =
            json::from_value(value).expect("predicate value");
        let payload = predicate.json_payload().expect("payload");
        let parsed: PredicateJson = json::from_json(payload).expect("predicate json");
        let fields: Vec<_> = parsed.equals.iter().map(|c| c.field.as_str()).collect();
        assert_eq!(fields, vec!["a", "b"]);
        assert_eq!(parsed.exists, vec!["y".to_string(), "z".to_string()]);
    }
}

#[cfg(all(test, feature = "ids_projection"))]
mod selector_tests {
    use super::*;
    use crate::account::Account;

    #[test]
    fn selector_build_ids_only() {
        let selector = SelectorTuple::<Account>::build(|s| s.ids_only());
        assert!(selector.is_ids_only());
    }
}

impl<T> Default for SelectorTuple<T> {
    fn default() -> Self {
        Self(
            #[cfg(feature = "ids_projection")]
            SelectorMode::Full,
            PhantomData,
        )
    }
}

impl<T> Clone for SelectorTuple<T> {
    fn clone(&self) -> Self {
        Self(
            #[cfg(feature = "ids_projection")]
            self.0,
            PhantomData,
        )
    }
}

impl<T: 'static> TypeId for SelectorTuple<T> {
    fn id() -> iroha_schema::Ident {
        std::any::type_name::<Self>().to_owned()
    }
}

impl<T: 'static> IntoSchema for SelectorTuple<T> {
    fn type_name() -> iroha_schema::Ident {
        "SelectorTuple".to_owned()
    }
    fn update_schema_map(m: &mut MetaMap) {
        m.insert::<Self>(Metadata::Tuple(iroha_schema::UnnamedFieldsMeta {
            types: vec![],
        }));
    }
}

/// Trait defining conversion into a selector.
pub trait IntoSelector {
    /// Element type accepted by the resulting selector.
    type SelectingType;
    /// Type produced by the selector.
    type SelectedType;
    /// Convert the receiver into a selector (no-op in lightweight mode).
    fn into_selector(self) -> ();
}

/// Trait defining conversion into a selector tuple.
pub trait IntoSelectorTuple {
    /// Element type accepted by the resulting tuple.
    type SelectingType;
    /// Concrete selector tuple produced by the conversion.
    type SelectedTuple;
    /// Convert the receiver into a selector tuple.
    fn into_selector_tuple(self) -> SelectorTuple<Self::SelectingType>;
}

impl<T> IntoSelectorTuple for SelectorTuple<T> {
    type SelectingType = T;
    type SelectedTuple = Self;
    fn into_selector_tuple(self) -> SelectorTuple<Self::SelectingType> {
        self
    }
}

#[cfg(feature = "ids_projection")]
impl<T> IntoSelectorTuple for SelectorField<T>
where
    T: Identifiable,
{
    type SelectingType = T;
    type SelectedTuple = <T as Identifiable>::Id;
    fn into_selector_tuple(self) -> SelectorTuple<Self::SelectingType> {
        SelectorTuple::ids_only()
    }
}

// -----------------------------------------------------------------------------
// Query error integration (minimal)
// -----------------------------------------------------------------------------

use crate::query::QueryOutputBatchBox;

/// Trait implemented on all evaluable selectors (minimal version).
pub trait EvaluateSelector<T: 'static> {
    /// Project a batch of references into a `QueryOutputBatchBox`.
    ///
    /// # Errors
    ///
    /// Returns an error when the lightweight DSL cannot project the provided
    /// items or when conversion fails in downstream codecs.
    #[allow(unused_variables)]
    fn project_clone<'a, I>(
        &self,
        batch: I,
    ) -> Result<QueryOutputBatchBox, crate::query::error::QueryExecutionFail>
    where
        I: Iterator<Item = &'a T> + 'a,
    {
        Err(crate::query::error::QueryExecutionFail::Conversion(
            "lightweight dsl does not project".to_string(),
        ))
    }

    /// Project a batch of owned items into a `QueryOutputBatchBox`.
    ///
    /// # Errors
    ///
    /// Returns an error when the lightweight DSL cannot project the provided
    /// items or when conversion fails in downstream codecs.
    #[allow(unused_variables)]
    fn project(
        &self,
        batch: impl Iterator<Item = T>,
    ) -> Result<QueryOutputBatchBox, crate::query::error::QueryExecutionFail> {
        Err(crate::query::error::QueryExecutionFail::Conversion(
            "lightweight dsl does not project".to_string(),
        ))
    }
}

impl<T: 'static> EvaluateSelector<T> for () {
    fn project_clone<'a, I>(
        &self,
        batch: I,
    ) -> Result<QueryOutputBatchBox, crate::query::error::QueryExecutionFail>
    where
        I: Iterator<Item = &'a T> + 'a,
    {
        #[cfg(not(feature = "ids_projection"))]
        let _ = &batch;
        #[cfg(feature = "ids_projection")]
        {
            macro_rules! project_ids_ref {
                ($target:ty, $id:ty, $iter:expr) => {{
                    if StdTypeId::of::<T>() == StdTypeId::of::<$target>() {
                        let ids: Vec<$id> = $iter
                            .map(|item| {
                                let any: &dyn Any = item;
                                any.downcast_ref::<$target>()
                                    .expect("type checked via TypeId")
                                    .id()
                                    .clone()
                            })
                            .collect();
                        return Ok(QueryOutputBatchBox::from(ids));
                    }
                }};
            }

            project_ids_ref!(crate::domain::Domain, crate::domain::DomainId, batch);
            project_ids_ref!(crate::account::Account, crate::account::AccountId, batch);
            project_ids_ref!(
                crate::asset::definition::AssetDefinition,
                crate::AssetDefinitionId,
                batch
            );
            project_ids_ref!(crate::nft::Nft, crate::nft::NftId, batch);
            project_ids_ref!(crate::rwa::Rwa, crate::rwa::RwaId, batch);
            project_ids_ref!(crate::role::Role, crate::role::RoleId, batch);
            project_ids_ref!(crate::trigger::Trigger, crate::trigger::TriggerId, batch);
        }

        Err(crate::query::error::QueryExecutionFail::Conversion(
            "lightweight dsl does not project".to_string(),
        ))
    }

    fn project(
        &self,
        batch: impl Iterator<Item = T>,
    ) -> Result<QueryOutputBatchBox, crate::query::error::QueryExecutionFail> {
        #[cfg(not(feature = "ids_projection"))]
        let _ = &batch;
        #[cfg(feature = "ids_projection")]
        {
            macro_rules! project_ids_owned {
                ($target:ty, $id:ty, $iter:expr) => {{
                    if StdTypeId::of::<T>() == StdTypeId::of::<$target>() {
                        let ids: Vec<$id> = $iter
                            .map(|item| {
                                let any: &dyn Any = &item;
                                any.downcast_ref::<$target>()
                                    .expect("type checked via TypeId")
                                    .id()
                                    .clone()
                            })
                            .collect();
                        return Ok(QueryOutputBatchBox::from(ids));
                    }
                }};
            }

            project_ids_owned!(crate::domain::Domain, crate::domain::DomainId, batch);
            project_ids_owned!(crate::account::Account, crate::account::AccountId, batch);
            project_ids_owned!(
                crate::asset::definition::AssetDefinition,
                crate::AssetDefinitionId,
                batch
            );
            project_ids_owned!(crate::nft::Nft, crate::nft::NftId, batch);
            project_ids_owned!(crate::rwa::Rwa, crate::rwa::RwaId, batch);
            project_ids_owned!(crate::role::Role, crate::role::RoleId, batch);
            project_ids_owned!(crate::trigger::Trigger, crate::trigger::TriggerId, batch);
        }

        Err(crate::query::error::QueryExecutionFail::Conversion(
            "lightweight dsl does not project".to_string(),
        ))
    }
}

/// Prelude re-export for the lightweight query DSL.
pub mod prelude {
    pub use super::{
        BaseProjector, CompoundPredicate, IntoSelector, IntoSelectorTuple, PredicateMarker,
        SelectorMarker, SelectorTuple,
    };
}
