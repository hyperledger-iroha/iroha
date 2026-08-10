//! Lightweight DSL shims for predicates and selectors used by queries.

use std::marker::PhantomData;

use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId};
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize, Parser, Value};

#[cfg(feature = "ids_projection")]
use crate::Identifiable;
#[cfg(feature = "json")]
use crate::query::json::{EqualsCondition, InCondition, PredicateJson};
pub use crate::query::tx_predicate::CommittedTxPredicate;
use crate::query::tx_predicate::{
    committed_tx_filters_from_predicate,
    committed_tx_predicate_from_filters as filters_to_tx_predicate,
};
#[cfg(feature = "json")]
use crate::query::tx_predicate::{
    committed_tx_predicate_from_canonical_json, committed_tx_predicate_from_predicate_json,
    committed_tx_predicate_from_value,
};

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
    /// Output produced by a projection.
    type Projection;
    /// Construct a projection from the provided atom.
    fn atom(_: Self::AtomType) -> Self::Projection;
}

impl<T, Marker> HasProjection<Marker> for T {
    type Projection = ();
    fn atom((): Self::AtomType) -> Self::Projection {}
}

/// Prototype provider used at compile-time in builder APIs.
pub trait HasPrototype {
    /// Prototype builder used by the DSL.
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
impl<T: 'static> IntoPredicate<T> for PredicateBuilder<T> {
    fn into_predicate(self) -> CompoundPredicate<T> {
        if self.predicate.is_empty() {
            return CompoundPredicate::PASS;
        }
        CompoundPredicate::from_predicate_json(&self.predicate)
    }
}

#[cfg(feature = "json")]
impl<T: 'static> IntoPredicate<T> for PredicateJson {
    fn into_predicate(self) -> CompoundPredicate<T> {
        if self.is_empty() {
            return CompoundPredicate::PASS;
        }
        CompoundPredicate::from_predicate_json(&self)
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
    payload: Option<std::sync::Arc<dyn core::any::Any + Send + Sync + 'static>>,
    marker: PhantomData<T>,
}

#[derive(Clone)]
struct PredicateJsonPayload {
    raw: String,
}

impl PredicateJsonPayload {
    #[cfg(feature = "json")]
    fn from_predicate(predicate: &PredicateJson) -> Self {
        let mut raw = String::new();
        JsonSerialize::json_serialize(predicate, &mut raw);
        Self { raw }
    }

    fn from_raw(raw: String) -> Self {
        Self { raw }
    }

    fn as_str(&self) -> &str {
        &self.raw
    }
}

// Compare the canonical wire representation so predicate equality reflects semantics.
impl<T> PartialEq for CompoundPredicate<T> {
    fn eq(&self, other: &Self) -> bool {
        self.to_wire() == other.to_wire()
    }
}

impl<T> Eq for CompoundPredicate<T> {}

impl<T> CompoundPredicate<T> {
    /// Predicate representing a pass-through (true) condition.
    pub const PASS: Self = Self {
        payload: None,
        marker: PhantomData,
    };

    #[inline]
    #[must_use]
    /// Combine two predicates via logical AND.
    pub fn and(self, other: Self) -> Self
    where
        T: 'static,
    {
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
                    return Self::from_predicate_json(&merged);
                }

                if let (Some(left_tree), Some(right_tree)) = (
                    left.as_ref().downcast_ref::<CommittedTxPredicate>(),
                    right.as_ref().downcast_ref::<CommittedTxPredicate>(),
                ) {
                    let merged = and_committed_tx_predicates(left_tree.clone(), right_tree.clone());
                    return Self::with_payload(std::sync::Arc::new(merged));
                }

                // JSON and typed committed-transaction predicates must preserve both
                // sides. Convert the JSON side through the strict typed codec;
                // unsupported mixed payloads reject every row instead of
                // silently dropping a condition.
                if let (Some(json), Some(tree)) = (
                    left.as_ref().downcast_ref::<PredicateJsonPayload>(),
                    right.as_ref().downcast_ref::<CommittedTxPredicate>(),
                ) {
                    let merged = committed_tx_predicate_from_json_payload(json.as_str())
                        .map_or(CommittedTxPredicate::Const(false), |left| {
                            and_committed_tx_predicates(left, tree.clone())
                        });
                    return Self::with_payload(std::sync::Arc::new(merged));
                }
                if let (Some(tree), Some(json)) = (
                    left.as_ref().downcast_ref::<CommittedTxPredicate>(),
                    right.as_ref().downcast_ref::<PredicateJsonPayload>(),
                ) {
                    let merged = committed_tx_predicate_from_json_payload(json.as_str())
                        .map_or(CommittedTxPredicate::Const(false), |right| {
                            and_committed_tx_predicates(tree.clone(), right)
                        });
                    return Self::with_payload(std::sync::Arc::new(merged));
                }
                Self::with_payload(std::sync::Arc::new(CommittedTxPredicate::Const(false)))
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

    fn with_payload(payload: std::sync::Arc<dyn core::any::Any + Send + Sync + 'static>) -> Self {
        Self {
            payload: Some(payload),
            marker: PhantomData,
        }
    }

    #[cfg(feature = "json")]
    fn from_json_value(value: &norito::json::Value) -> Result<Self, norito::json::Error>
    where
        T: 'static,
    {
        if core::any::TypeId::of::<T>()
            == core::any::TypeId::of::<crate::query::CommittedTransaction>()
        {
            let predicate = committed_tx_predicate_from_value(value)
                .map_err(|error| norito::json::Error::Message(error.to_string()))?;
            return Ok(Self::with_payload(std::sync::Arc::new(predicate)));
        }
        match PredicateJson::try_from_value(value) {
            Ok(predicate) if predicate.is_empty() => Ok(Self::PASS),
            Ok(predicate) => Ok(Self::from_predicate_json(&predicate)),
            Err(schema_error) => Err(norito::json::Error::Message(schema_error.to_string())),
        }
    }

    #[cfg(feature = "json")]
    fn from_predicate_json(predicate: &PredicateJson) -> Self
    where
        T: 'static,
    {
        if core::any::TypeId::of::<T>()
            == core::any::TypeId::of::<crate::query::CommittedTransaction>()
        {
            let predicate = committed_tx_predicate_from_predicate_json(predicate)
                .unwrap_or(CommittedTxPredicate::Const(false));
            return Self::with_payload(std::sync::Arc::new(predicate));
        }
        let payload = PredicateJsonPayload::from_predicate(predicate);
        Self::with_payload(std::sync::Arc::new(payload))
    }

    fn from_json_raw(raw: String) -> Result<Self, norito::core::Error>
    where
        T: 'static,
    {
        #[cfg(feature = "json")]
        {
            if core::any::TypeId::of::<T>()
                == core::any::TypeId::of::<crate::query::CommittedTransaction>()
            {
                committed_tx_predicate_from_canonical_json(&raw)
                    .map_err(|error| norito::core::Error::Message(error.to_string()))?;
                return Err(norito::core::Error::Message(
                    "committed transaction predicates must use the TxPredicate wire variant"
                        .to_owned(),
                ));
            }
            let value = norito::json::from_json::<Value>(&raw)
                .map_err(|error| norito::core::Error::Message(error.to_string()))?;
            let canonical = match PredicateJson::try_from_value(&value) {
                Ok(predicate) if predicate.is_empty() => {
                    return Err(norito::core::Error::Message(
                        "empty predicate JSON must use the Pass wire variant".to_owned(),
                    ));
                }
                Ok(predicate) => PredicateJsonPayload::from_predicate(&predicate)
                    .as_str()
                    .to_owned(),
                Err(schema_error) => {
                    return Err(norito::core::Error::Message(schema_error.to_string()));
                }
            };
            if canonical != raw {
                return Err(norito::core::Error::Message(
                    "predicate JSON payload must use canonical encoding".to_owned(),
                ));
            }
        }
        #[cfg(not(feature = "json"))]
        {
            let _ = &raw;
            return Err(norito::core::Error::Message(
                "JSON predicate wire variant requires the `json` feature".to_owned(),
            ));
        }
        let payload = PredicateJsonPayload::from_raw(raw);
        Ok(Self::with_payload(std::sync::Arc::new(payload)))
    }

    fn to_wire(&self) -> CompoundPredicateWire {
        if let Some(payload) = self.payload.as_ref() {
            if let Some(tree) = payload.downcast_ref::<CommittedTxPredicate>() {
                return CompoundPredicateWire::TxPredicate(tree.clone());
            }
            if let Some(json) = payload.downcast_ref::<PredicateJsonPayload>() {
                return CompoundPredicateWire::Json(json.as_str().to_owned());
            }
            return CompoundPredicateWire::TxPredicate(CommittedTxPredicate::Const(false));
        }
        CompoundPredicateWire::Pass
    }

    fn from_wire(wire: CompoundPredicateWire) -> Result<Self, norito::core::Error>
    where
        T: 'static,
    {
        Ok(match wire {
            CompoundPredicateWire::Pass => Self::PASS,
            CompoundPredicateWire::Json(raw) => Self::from_json_raw(raw)?,
            CompoundPredicateWire::TxPredicate(tree)
                if core::any::TypeId::of::<T>()
                    == core::any::TypeId::of::<crate::query::CommittedTransaction>() =>
            {
                Self::with_payload(std::sync::Arc::new(tree))
            }
            CompoundPredicateWire::TxPredicate(_) => {
                return Err(norito::core::Error::Message(
                    "committed transaction predicate wire variant used for another query type"
                        .to_owned(),
                ));
            }
        })
    }

    #[cfg(feature = "json")]
    /// Return the raw JSON payload carried by the predicate, if any.
    pub fn json_payload(&self) -> Option<&str> {
        self.payload
            .as_ref()
            .and_then(|p| p.downcast_ref::<PredicateJsonPayload>())
            .map(PredicateJsonPayload::as_str)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize)]
enum CompoundPredicateWire {
    Pass,
    Json(String),
    TxPredicate(CommittedTxPredicate),
}

// Manual Norito core codec: normalize every payload through the closed wire enum.
impl<T> norito::core::NoritoSerialize for CompoundPredicate<T> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let wire = self.to_wire();
        norito::core::NoritoSerialize::serialize(&wire, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.to_wire().encoded_len_hint()
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.to_wire().encoded_len_exact()
    }
}

impl<'de, T: 'static> norito::core::NoritoDeserialize<'de> for CompoundPredicate<T> {
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
        Self::from_wire(wire)
    }
}

#[cfg(feature = "json")]
impl<T: 'static> JsonSerialize for CompoundPredicate<T> {
    fn json_serialize(&self, out: &mut String) {
        if core::any::TypeId::of::<T>()
            == core::any::TypeId::of::<crate::query::CommittedTransaction>()
            && let Some(payload) = self.payload.as_ref()
        {
            if let Some(predicate) = payload.downcast_ref::<CommittedTxPredicate>() {
                predicate.json_serialize(out);
                return;
            }
            if let Some(json) = payload.downcast_ref::<PredicateJsonPayload>()
                && let Some(predicate) = predicate_json_from_raw(json.as_str())
                && let Ok(predicate) = committed_tx_predicate_from_predicate_json(&predicate)
            {
                predicate.json_serialize(out);
                return;
            }
            // Unknown committed-transaction payloads fail closed and never
            // turn into the pass-through empty object.
            CommittedTxPredicate::Const(false).json_serialize(out);
            return;
        }
        if let Some(json) = self
            .payload
            .as_ref()
            .and_then(|payload| payload.downcast_ref::<PredicateJsonPayload>())
        {
            out.push_str(json.as_str());
            return;
        }
        out.push('{');
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl<T: 'static> JsonDeserialize for CompoundPredicate<T> {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, norito::json::Error> {
        use norito::json::Value;

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
    predicate_json_from_value(&value)
}

#[cfg(feature = "json")]
fn committed_tx_predicate_from_json_payload(raw: &str) -> Option<CommittedTxPredicate> {
    predicate_json_from_raw(raw)
        .and_then(|predicate| committed_tx_predicate_from_predicate_json(&predicate).ok())
}

#[cfg(not(feature = "json"))]
fn committed_tx_predicate_from_json_payload(_raw: &str) -> Option<CommittedTxPredicate> {
    None
}

#[cfg(feature = "json")]
fn predicate_json_from_value(value: &Value) -> Option<PredicateJson> {
    PredicateJson::try_from_value(value).ok()
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
    /// Return `true` when the predicate matches the provided input.
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
            return false;
        };
        let Some(predicate) = predicate_json_from_raw(json_payload.as_str()) else {
            return false;
        };
        let Ok(value) = json::to_value(input) else {
            return false;
        };
        predicate_json_applies(&predicate, &value)
    }
}

#[cfg(not(feature = "json"))]
impl<U: ?Sized, T> EvaluatePredicate<U> for CompoundPredicate<T> {
    fn applies(&self, _input: &U) -> bool {
        self.payload.is_none()
    }
}

// -------- Server-side transaction predicates --------

/// Trait that allows to get the predicate type for a given type.
pub trait HasPredicateAtom {
    /// Predicate type associated with the implementor.
    type Predicate: EvaluatePredicate<Self>;
}

impl<T> HasPredicateAtom for T {
    type Predicate = ();
}

impl<T> EvaluatePredicate<T> for () {}

/// Lightweight selector tuple returned by the classic DSL.
#[derive(Debug, PartialEq, Eq, Decode, Encode)]
pub struct SelectorTuple<T>(
    #[cfg(feature = "ids_projection")] SelectorMode,
    PhantomData<T>,
);

/// Experimental selector mode to prototype basic projections.
#[cfg(feature = "ids_projection")]
#[derive(Debug, Clone, Copy, Decode, Encode, PartialEq, Eq)]
/// Controls how selectors behave when the ids projection feature is enabled.
pub enum SelectorMode {
    /// Request the full object for each row.
    Full,
    /// Request identifiers only.
    IdsOnly,
}

#[cfg(all(feature = "json", feature = "ids_projection"))]
impl JsonSerialize for SelectorMode {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            SelectorMode::Full => "Full",
            SelectorMode::IdsOnly => "IdsOnly",
        };
        norito::json::write_json_string(label, out);
    }
}

#[cfg(all(feature = "json", feature = "ids_projection"))]
impl JsonDeserialize for SelectorMode {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "Full" => Ok(SelectorMode::Full),
            "IdsOnly" => Ok(SelectorMode::IdsOnly),
            other => Err(norito::json::Error::unknown_field(other.to_owned())),
        }
    }
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
    /// Iterate over the selector payload (always empty in lightweight mode).
    pub fn iter(&self) -> impl Iterator<Item = ()> {
        #[cfg(not(feature = "ids_projection"))]
        {
            core::iter::empty()
        }
        #[cfg(feature = "ids_projection")]
        {
            match self.0 {
                SelectorMode::Full => core::iter::empty(),
                // The actual projector is provided by a blanket impl on () via EvaluateSelector below;
                // we emit a single unit value to trigger projection when ids-only is requested.
                SelectorMode::IdsOnly => core::iter::once(()),
            }
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
impl<T> JsonSerialize for SelectorTuple<T> {
    fn json_serialize(&self, out: &mut String) {
        out.push('[');
        #[cfg(feature = "ids_projection")]
        {
            let label = match self.0 {
                SelectorMode::Full => "Full",
                SelectorMode::IdsOnly => "IdsOnly",
            };
            norito::json::write_json_string(label, out);
        }
        out.push(']');
    }
}

#[cfg(feature = "json")]
impl<T> JsonDeserialize for SelectorTuple<T> {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, norito::json::Error> {
        parser.skip_ws();
        parser.expect(b'[')?;
        parser.skip_ws();
        #[cfg(feature = "ids_projection")]
        let mode = if parser.try_consume_char(b']')? {
            SelectorMode::Full
        } else {
            let label = parser.parse_string()?;
            parser.skip_ws();
            parser.expect(b']')?;
            match label.as_str() {
                "Full" => SelectorMode::Full,
                "IdsOnly" => SelectorMode::IdsOnly,
                other => return Err(norito::json::Error::unknown_field(other.to_owned())),
            }
        };
        #[cfg(not(feature = "ids_projection"))]
        {
            if !parser.try_consume_char(b']')? {
                loop {
                    parser.skip_value()?;
                    parser.skip_ws();
                    if parser.try_consume_char(b']')? {
                        break;
                    }
                    parser.expect(b',')?;
                    parser.skip_ws();
                }
            }
        }
        Ok(Self(
            #[cfg(feature = "ids_projection")]
            mode,
            PhantomData,
        ))
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
mod json_tests {
    use norito::json;

    use super::*;

    #[test]
    fn compound_predicate_serializes_empty_object() {
        let predicate = CompoundPredicate::<u32>::PASS;
        let value = json::to_value(&predicate).expect("serialize predicate");
        assert!(matches!(value, json::Value::Object(ref map) if map.is_empty()));

        let roundtrip: CompoundPredicate<u32> =
            json::from_value(value).expect("deserialize predicate");
        assert_eq!(roundtrip, predicate);
    }
}

#[cfg(all(test, feature = "json"))]
mod codec_tests {
    use std::time::Duration;

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, MerkleProof};
    use iroha_primitives::json::Json;
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

    fn expect_committed_tx_tree(predicate: &CompoundPredicate<query::CommittedTransaction>) -> P {
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

    fn test_network_id() -> dm::NetworkId {
        dm::NetworkId::from_genesis_hash(HashOf::<block::BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x15; Hash::LENGTH]),
        ))
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
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("fixture seed derives Ed25519 keypair")
                    .into_parts();
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
        let mut builder = signed::TransactionBuilder::new(
            test_network_id(),
            authority.id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
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
        let result = signed::TransactionResult::new(result_inner);
        let result_hash = transaction::TransactionResult::hash_from_inner(&result.0);

        query::CommittedTransaction {
            block_hash: dummy_block_hash(),
            entrypoint_hash: entry_hash,
            entrypoint_proof: dummy_proof_entry(),
            entrypoint,
            result_hash,
            result_proof: dummy_proof_result(),
            result,
            merge_inclusion: None,
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
        let explicit_empty_predicate: CompoundPredicate<norito::json::Value> =
            norito::json::from_json("{\"equals\":[],\"in\":[],\"exists\":[]}")
                .expect("explicit empty predicate");

        assert!(matches!(
            null_predicate.to_wire(),
            CompoundPredicateWire::Pass
        ));
        assert!(matches!(
            empty_predicate.to_wire(),
            CompoundPredicateWire::Pass
        ));
        assert!(matches!(
            explicit_empty_predicate.to_wire(),
            CompoundPredicateWire::Pass
        ));
        assert_eq!(null_predicate.json_payload(), None);
        assert_eq!(empty_predicate.json_payload(), None);
        assert_eq!(explicit_empty_predicate.json_payload(), None);
    }

    #[test]
    fn compound_predicate_json_deserialize_rejects_non_object_payload() {
        let error = norito::json::from_json::<CompoundPredicate<norito::json::Value>>("[1,2,3]")
            .expect_err("array predicate must be rejected");

        assert!(
            error
                .to_string()
                .contains("predicate JSON must be an object")
        );
    }

    #[test]
    fn compound_predicate_invalid_raw_json_rejects() {
        let error = CompoundPredicate::<norito::json::Value>::from_wire(
            CompoundPredicateWire::Json("{".into()),
        )
        .expect_err("invalid raw predicate JSON must be rejected");
        assert!(error.to_string().contains("unexpected end of input"));
    }

    #[test]
    fn compound_predicate_noncanonical_raw_json_rejects() {
        let predicate = CompoundPredicate::<Domain>::build(|p| p.exists("id"));
        let noncanonical = format!("{} ", predicate.json_payload().expect("JSON payload"));
        let error =
            CompoundPredicate::<Domain>::from_wire(CompoundPredicateWire::Json(noncanonical))
                .expect_err("noncanonical raw predicate JSON must be rejected");

        assert!(error.to_string().contains("canonical encoding"));
    }

    #[test]
    fn compound_predicate_equality_tracks_predicate_semantics() {
        let by_id = CompoundPredicate::<Domain>::build(|p| p.exists("id"));
        let by_name = CompoundPredicate::<Domain>::build(|p| p.exists("name"));

        assert_eq!(by_id, by_id.clone());
        assert_ne!(by_id, by_name);
        assert_ne!(CompoundPredicate::<Domain>::PASS, by_id);

        let successful =
            CompoundPredicate::<query::CommittedTransaction>::from_committed_tx_predicate(
                P::ResultEq(true),
            );
        let recent = CompoundPredicate::<query::CommittedTransaction>::from_committed_tx_predicate(
            P::TsGte(10),
        );
        assert_ne!(successful, recent);
    }

    #[test]
    fn committed_tx_compound_predicate_norito_roundtrip_canonicalizes_filters_to_typed_wire() {
        let predicate = CompoundPredicate::<query::CommittedTransaction>::from_filters(
            query::CommittedTxFilters {
                authority_exists: Some(true),
                result_ok: Some(true),
                ..Default::default()
            },
        );
        let wire = predicate.to_wire();
        assert!(matches!(wire, CompoundPredicateWire::TxPredicate(_)));
        assert_eq!(predicate.encoded_len_hint(), wire.encoded_len_hint());
        assert_eq!(predicate.encoded_len_exact(), wire.encoded_len_exact());

        let bytes = norito::to_bytes(&predicate).expect("encode filters predicate");
        let decoded: CompoundPredicate<query::CommittedTransaction> =
            norito::decode_from_bytes(&bytes).expect("decode filters predicate");

        assert!(matches!(
            decoded.to_wire(),
            CompoundPredicateWire::TxPredicate(_)
        ));
        let filters = decoded
            .committed_tx_filters()
            .expect("lossless flat index-planning view");
        assert_eq!(filters.authority_exists, Some(true));
        assert_eq!(filters.result_ok, Some(true));
        assert!(filters.entry_in.is_empty());
    }

    #[test]
    fn committed_tx_compound_predicate_json_roundtrip_preserves_filter_conditions() {
        let predicate = CompoundPredicate::<query::CommittedTransaction>::from_filters(
            query::CommittedTxFilters {
                authority_exists: Some(true),
                result_ok: Some(false),
                ts_ge: Some(42),
                ..Default::default()
            },
        );

        let raw = norito::json::to_json(&predicate).expect("encode filters as typed JSON");
        assert_ne!(raw, "{}");
        let decoded: CompoundPredicate<query::CommittedTransaction> =
            norito::json::from_json(&raw).expect("decode typed filters JSON");
        assert!(matches!(
            decoded.to_wire(),
            CompoundPredicateWire::TxPredicate(P::And(children))
                if matches!(
                    children.as_slice(),
                    [P::AuthorityExists(true), P::TsGte(42), P::ResultEq(false)]
                )
        ));
    }

    #[test]
    fn committed_tx_compound_predicate_json_fails_closed_for_invalid_filter_sets() {
        let predicate = CompoundPredicate::<query::CommittedTransaction>::from_filters(
            query::CommittedTxFilters {
                result_ok_in: vec![true, true],
                ..Default::default()
            },
        );
        assert!(
            norito::to_bytes(&predicate).is_err(),
            "invalid internal filters must not enter the binary wire format"
        );
        let raw = norito::json::to_json(&predicate).expect("encode invalid filters safely");
        let decoded: CompoundPredicate<query::CommittedTransaction> =
            norito::json::from_json(&raw).expect("decode fail-closed predicate");
        assert!(matches!(
            decoded.to_wire(),
            CompoundPredicateWire::TxPredicate(P::Const(false))
        ));
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
    fn committed_tx_compound_predicate_json_roundtrip_preserves_boolean_tree() {
        let tree = P::Or(vec![
            P::Not(Box::new(P::ResultEq(false))),
            P::MetadataIn {
                key: "tier".parse().expect("metadata key"),
                values: vec![Json::new("gold"), Json::new("silver")],
            },
        ]);
        let predicate =
            CompoundPredicate::<query::CommittedTransaction>::from_committed_tx_predicate(
                tree.clone(),
            );

        let raw = norito::json::to_json(&predicate).expect("encode tree as typed JSON");
        assert_ne!(raw, "{}");
        let decoded: CompoundPredicate<query::CommittedTransaction> =
            norito::json::from_json(&raw).expect("decode typed tree JSON");
        assert!(
            matches!(decoded.to_wire(), CompoundPredicateWire::TxPredicate(value) if value == tree)
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

        let tree = expect_committed_tx_tree(&left.and(right));
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

        let tree = expect_committed_tx_tree(&left.and(right));
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

        let tree = expect_committed_tx_tree(&left.and(right));
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

        let tree = expect_committed_tx_tree(&left.and(right));
        assert!(matches!(
            tree,
            P::And(children)
                if matches!(children.as_slice(), [P::ResultEq(true), P::EntryExists(true)])
        ));
    }

    #[test]
    fn committed_tx_compound_predicate_json_expression_uses_typed_tree_codec() {
        let authority = TestAuthority::new(0x11);
        let ok_tx = build_ext_tx(&authority, 42, true, dm::Metadata::default());
        let err_tx = build_ext_tx(&authority, 42, false, dm::Metadata::default());
        let predicate: CompoundPredicate<query::CommittedTransaction> =
            norito::json::from_value(norito::json!({
                "op": "eq",
                "args": [
                    "result_ok",
                    true
                ]
            }))
            .expect("predicate value");

        assert!(predicate.applies(&ok_tx));
        assert!(!predicate.applies(&err_tx));
        assert!(matches!(
            predicate.to_wire(),
            CompoundPredicateWire::TxPredicate(P::ResultEq(true))
        ));
        let raw = norito::json::to_json(&predicate).expect("encode typed predicate");
        assert_ne!(raw, "{}");
        let decoded: CompoundPredicate<query::CommittedTransaction> =
            norito::json::from_json(&raw).expect("decode typed predicate");
        assert_eq!(decoded, predicate);
    }

    #[test]
    fn committed_tx_compound_predicate_json_expression_rejects_adversarial_shapes() {
        let invalid = [
            norito::json!({"op": "unknown", "args": []}),
            norito::json!({"op": "eq", "args": [{"FieldPath": "result_ok"}]}),
            norito::json!({"op": "eq", "args": [{"FieldPath": "result_ok"}, "true"]}),
            norito::json!({"op": "eq", "args": [{"FieldPath": "unknown"}, true]}),
        ];

        for value in invalid {
            assert!(
                norito::json::from_value::<CompoundPredicate<query::CommittedTransaction>>(value)
                    .is_err(),
                "malformed committed transaction expression must be rejected"
            );
        }

        let type_confusion = norito::json::from_value::<CompoundPredicate<Domain>>(norito::json!({
            "op": "eq",
            "args": ["result_ok", true]
        }));
        assert!(type_confusion.is_err());
    }

    #[test]
    fn committed_tx_compound_predicate_rejects_unknown_shorthand_map() {
        let error = norito::json::from_value::<CompoundPredicate<query::CommittedTransaction>>(
            norito::json!({"result_hash": "placeholder"}),
        )
        .expect_err("unknown shorthand predicate must be rejected");

        assert!(
            error
                .to_string()
                .contains("unknown committed transaction predicate field")
        );
    }

    #[test]
    fn committed_tx_compound_predicate_rejects_redundant_empty_json_wire_variant() {
        for raw in ["{}", "{\"equals\":[],\"in\":[],\"exists\":[]}"] {
            let error = CompoundPredicate::<query::CommittedTransaction>::from_wire(
                CompoundPredicateWire::Json(raw.into()),
            )
            .expect_err("committed predicates must not use the JSON wire variant");
            assert!(!error.to_string().is_empty());
        }

        let canonical = norito::json::to_json(&P::ResultEq(true)).expect("canonical tree JSON");
        let error = CompoundPredicate::<query::CommittedTransaction>::from_wire(
            CompoundPredicateWire::Json(canonical),
        )
        .expect_err("typed predicate replay through JSON wire variant must be rejected");
        assert!(error.to_string().contains("TxPredicate wire variant"));
    }

    #[test]
    fn committed_tx_compound_predicate_invalid_raw_json_rejects() {
        let error = CompoundPredicate::<query::CommittedTransaction>::from_wire(
            CompoundPredicateWire::Json("{".into()),
        )
        .expect_err("invalid raw predicate JSON must be rejected");
        assert!(error.to_string().contains("unexpected end of input"));
    }

    #[test]
    fn committed_tx_typed_wire_rejects_non_transaction_type_confusion() {
        let error = CompoundPredicate::<Domain>::from_wire(CompoundPredicateWire::TxPredicate(
            P::ResultEq(true),
        ))
        .expect_err("typed transaction wire variant must reject Domain predicates");
        assert!(error.to_string().contains("another query type"));

        let committed =
            CompoundPredicate::<query::CommittedTransaction>::from_committed_tx_predicate(
                P::ResultEq(true),
            );
        let bytes = norito::to_bytes(&committed).expect("encode committed predicate");
        assert!(norito::decode_from_bytes::<CompoundPredicate<Domain>>(&bytes).is_err());
    }

    #[test]
    fn committed_tx_compound_predicate_and_preserves_both_mixed_payloads() {
        let json = PredicateJson {
            equals: vec![EqualsCondition::new(
                "result_ok",
                norito::json::Value::Bool(true),
            )],
            ..PredicateJson::default()
        };
        let json_predicate: CompoundPredicate<query::CommittedTransaction> =
            CompoundPredicate::with_payload(std::sync::Arc::new(
                PredicateJsonPayload::from_predicate(&json),
            ));
        let tree_predicate =
            CompoundPredicate::<query::CommittedTransaction>::from_committed_tx_predicate(
                P::TsGte(10),
            );

        let json_then_tree = json_predicate.clone().and(tree_predicate.clone());
        let tree_then_json = tree_predicate.and(json_predicate);

        assert!(matches!(
            json_then_tree.to_wire(),
            CompoundPredicateWire::TxPredicate(P::And(children))
                if matches!(children.as_slice(), [P::ResultEq(true), P::TsGte(10)])
        ));
        assert!(matches!(
            tree_then_json.to_wire(),
            CompoundPredicateWire::TxPredicate(P::And(children))
                if matches!(children.as_slice(), [P::TsGte(10), P::ResultEq(true)])
        ));
    }

    #[test]
    fn committed_tx_generic_builder_promotes_to_typed_tree_before_wire_encoding() {
        let predicate: CompoundPredicate<query::CommittedTransaction> = PredicateJson {
            equals: vec![EqualsCondition::new(
                "result_ok",
                norito::json::Value::Bool(true),
            )],
            ..PredicateJson::default()
        }
        .into_predicate();
        assert!(matches!(
            predicate.to_wire(),
            CompoundPredicateWire::TxPredicate(P::ResultEq(true))
        ));

        let bytes = norito::to_bytes(&predicate).expect("encode promoted predicate");
        let decoded: CompoundPredicate<query::CommittedTransaction> =
            norito::decode_from_bytes(&bytes).expect("decode promoted predicate");
        assert_eq!(decoded, predicate);
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
    fn predicate_json_from_value_rejects_unknown_shorthand_maps() {
        assert!(predicate_json_from_value(&norito::json!({"field": []})).is_none());
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

        let tree = expect_committed_tx_tree(&left.and(right));
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
        let predicate = filters_to_tx_predicate(&query::CommittedTxFilters {
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
        let (public_key, _private_key) = KeyPair::try_from_seed(vec![0x42; 32], Algorithm::Ed25519)
            .expect("fixture seed derives Ed25519 keypair")
            .into_parts();
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

        let mismatch = CompoundPredicate::<Domain>::build(|p| p.equals("metadata.rank", 2_u32));
        assert!(!mismatch.applies(&domain));
    }

    #[test]
    fn compound_predicate_and_merges_json() {
        let domain = sample_domain();
        let left = CompoundPredicate::<Domain>::build(|p| p.equals("metadata.rank", 1_u32));
        let right = CompoundPredicate::<Domain>::build(|p| p.exists("metadata.label"));
        let combined = left.and(right);
        assert!(combined.applies(&domain));

        let missing = CompoundPredicate::<Domain>::build(|p| p.exists("metadata.missing"));
        assert!(!combined.clone().and(missing).applies(&domain));

        let payload = combined.json_payload().expect("payload");
        let parsed: PredicateJson = json::from_json(payload).expect("predicate json");
        assert_eq!(parsed.equals.len(), 1);
        assert_eq!(parsed.exists.len(), 1);
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

#[cfg(test)]
mod committed_tx_predicate_tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, MerkleProof};
    use iroha_primitives::json::Json;
    use norito::json;

    use super::CommittedTxPredicate as P;
    // Explicit module/type imports to avoid relying on prelude module paths
    use crate::prelude::{
        DataTriggerSequence, TransactionEntrypoint, TransactionRejectionReason, TransactionResult,
    };
    use crate::{
        account, block, domain::DomainId, prelude as dm, query, transaction, transaction::signed,
        trigger,
    };

    fn dummy_block_hash() -> HashOf<block::BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed([0xA5; Hash::LENGTH]))
    }
    fn test_network_id() -> dm::NetworkId {
        dm::NetworkId::from_genesis_hash(HashOf::<block::BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x15; Hash::LENGTH]),
        ))
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
            let _domain: crate::domain::DomainId =
                DomainId::try_new("wonderland", "universal").unwrap();
            let (public_key, private_key) =
                iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("fixture seed derives Ed25519 keypair")
                    .into_parts();
            let id = account::AccountId::new(public_key);
            Self { id, private_key }
        }

        fn id_str(&self) -> String {
            self.id.to_string()
        }
    }

    fn build_ext_tx(
        authority: &TestAuthority,
        ts_ms: u64,
        ok: bool,
        metadata: dm::Metadata,
    ) -> query::CommittedTransaction {
        let mut b = signed::TransactionBuilder::new(
            test_network_id(),
            authority.id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        b.set_creation_time(core::time::Duration::from_millis(ts_ms));
        let signed: signed::SignedTransaction = b
            .with_metadata(metadata)
            .with_instructions::<dm::InstructionBox>([])
            .sign(&authority.private_key);

        let entry_hash = signed.hash_as_entrypoint();
        let entry = TransactionEntrypoint::External(signed);
        let result_inner: signed::TransactionResultInner = if ok {
            Ok(DataTriggerSequence::default())
        } else {
            Err(TransactionRejectionReason::Validation(
                dm::ValidationFail::InternalError("x".into()),
            ))
        };
        let result = signed::TransactionResult::new(result_inner);
        let result_hash = TransactionResult::hash_from_inner(&result.0);

        query::CommittedTransaction {
            block_hash: dummy_block_hash(),
            entrypoint_hash: entry_hash,
            entrypoint_proof: dummy_proof_entry(),
            entrypoint: entry,
            result_hash,
            result_proof: dummy_proof_result(),
            result,
            merge_inclusion: None,
        }
    }

    fn make_ext_tx(authority: &TestAuthority, ts_ms: u64, ok: bool) -> query::CommittedTransaction {
        build_ext_tx(authority, ts_ms, ok, dm::Metadata::default())
    }

    fn make_ext_tx_with_metadata(
        authority: &TestAuthority,
        ts_ms: u64,
        ok: bool,
        metadata: dm::Metadata,
    ) -> query::CommittedTransaction {
        build_ext_tx(authority, ts_ms, ok, metadata)
    }

    #[test]
    fn metadata_predicates_apply_values() {
        let authority = TestAuthority::new(0x10);
        let mut meta = dm::Metadata::default();
        let display_key: dm::Name = "display_name".parse().unwrap();
        meta.insert(display_key.clone(), Json::new("Alice"));
        let tx = make_ext_tx_with_metadata(&authority, 42, true, meta);

        let value_alice = Json::new("Alice");
        let value_bob = Json::new("Bob");

        assert!(
            P::MetadataEq {
                key: display_key.clone(),
                value: value_alice.clone(),
            }
            .applies(&tx)
        );
        assert!(
            !P::MetadataEq {
                key: display_key.clone(),
                value: value_bob.clone(),
            }
            .applies(&tx)
        );

        assert!(
            P::MetadataNe {
                key: display_key.clone(),
                value: value_bob.clone(),
            }
            .applies(&tx)
        );
        assert!(
            !P::MetadataNe {
                key: display_key.clone(),
                value: value_alice.clone(),
            }
            .applies(&tx)
        );

        assert!(
            P::MetadataExists {
                key: display_key.clone(),
                exists: true,
            }
            .applies(&tx)
        );
        assert!(
            !P::MetadataExists {
                key: display_key,
                exists: false,
            }
            .applies(&tx)
        );
    }

    #[test]
    fn metadata_predicates_apply_null_and_missing() {
        let authority = TestAuthority::new(0x12);
        let mut meta_null = dm::Metadata::default();
        let note_key: dm::Name = "note".parse().unwrap();
        meta_null.insert(note_key.clone(), Json::new(json::Value::Null));
        let tx_null = make_ext_tx_with_metadata(&authority, 84, true, meta_null);

        assert!(
            P::MetadataIsNull {
                key: note_key.clone(),
                is_null: true,
            }
            .applies(&tx_null)
        );
        assert!(
            !P::MetadataIsNull {
                key: note_key,
                is_null: false,
            }
            .applies(&tx_null)
        );

        let missing_key: dm::Name = "missing".parse().unwrap();
        let value_alice = Json::new("Alice");
        assert!(
            !P::MetadataEq {
                key: missing_key.clone(),
                value: value_alice.clone(),
            }
            .applies(&tx_null)
        );
        assert!(
            P::MetadataNe {
                key: missing_key.clone(),
                value: value_alice,
            }
            .applies(&tx_null)
        );
        assert!(
            !P::MetadataExists {
                key: missing_key.clone(),
                exists: true,
            }
            .applies(&tx_null)
        );
        assert!(
            P::MetadataExists {
                key: missing_key,
                exists: false,
            }
            .applies(&tx_null)
        );
    }

    fn make_time_tx(ok: bool) -> query::CommittedTransaction {
        let empty: [u8; 32] = [0; 32];
        let h_block = HashOf::<block::BlockHeader>::from_untyped_unchecked(Hash::prehashed(empty));
        let h_entry = HashOf::<transaction::TransactionEntrypoint>::from_untyped_unchecked(
            Hash::prehashed(empty),
        );
        let h_result = HashOf::<transaction::TransactionResult>::from_untyped_unchecked(
            Hash::prehashed(empty),
        );
        let entry_proof: MerkleProof<transaction::TransactionEntrypoint> =
            MerkleProof::from_audit_path(0, vec![]);
        let result_proof: MerkleProof<transaction::TransactionResult> =
            MerkleProof::from_audit_path(0, vec![]);

        let authority = account::AccountId::parse_encoded(
            "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
        )
        .expect("valid authority")
        .into_account_id();
        let trigger_id: trigger::TriggerId = "test_trigger".parse().unwrap();
        let time_entry = trigger::TimeTriggerEntrypoint {
            id: trigger_id,
            instructions: signed::ExecutionStep(Vec::<dm::InstructionBox>::new().into()),
            authority,
        };
        let entry = TransactionEntrypoint::Time(time_entry);
        let result = if ok {
            signed::TransactionResult::new(Ok(DataTriggerSequence::default()))
        } else {
            signed::TransactionResult::new(Err(TransactionRejectionReason::Validation(
                dm::ValidationFail::NotPermitted("no".into()),
            )))
        };

        query::CommittedTransaction {
            block_hash: h_block,
            entrypoint_hash: h_entry,
            entrypoint_proof: entry_proof,
            entrypoint: entry,
            result_hash: h_result,
            result_proof,
            result,
            merge_inclusion: None,
        }
    }

    #[test]
    fn authority_sets_and_exists() {
        let authority_a = TestAuthority::new(0x11);
        let authority_b = TestAuthority::new(0x22);
        let a = authority_a.id_str();
        let b = authority_b.id_str();
        let account_a = account::AccountId::parse_encoded(&a)
            .expect("authority A")
            .into_account_id();
        let account_b = account::AccountId::parse_encoded(&b)
            .expect("authority B")
            .into_account_id();
        let tx_a = make_ext_tx(&authority_a, 1000, true);
        let tx_b = make_ext_tx(&authority_b, 2000, false);

        assert!(P::AuthorityEq(account_a.clone()).applies(&tx_a));
        assert!(!P::AuthorityEq(account_a.clone()).applies(&tx_b));
        assert!(P::AuthorityIn(vec![account_a.clone()]).applies(&tx_a));
        assert!(!P::AuthorityIn(vec![account_a.clone()]).applies(&tx_b));
        assert!(P::AuthorityNin(vec![account_a]).applies(&tx_b));
        assert!(!P::AuthorityNin(vec![account_b]).applies(&tx_b));
        assert!(P::AuthorityExists(true).applies(&tx_a));
        assert!(P::AuthorityExists(true).applies(&tx_b));
        assert!(!P::AuthorityExists(false).applies(&tx_a));
    }

    #[test]
    fn timestamp_bounds_and_exists() {
        let authority = TestAuthority::new(0x33);
        let tx = make_ext_tx(&authority, 1500, true);
        assert!(P::TsGte(1000).applies(&tx));
        assert!(P::TsLte(2000).applies(&tx));
        assert!(!P::TsLt(1500).applies(&tx));
        assert!(P::TsEq(1500).applies(&tx));
        assert!(P::TsIn(vec![1000, 1500]).applies(&tx));
        assert!(P::TsExists(true).applies(&tx));

        let t = make_time_tx(true);
        assert!(P::TsExists(false).applies(&t));
        assert!(!P::TsExists(true).applies(&t));
    }

    #[test]
    fn entry_and_result_checks() {
        let authority_true = TestAuthority::new(0x44);
        let authority_false = TestAuthority::new(0x55);
        let tx_true = make_ext_tx(&authority_true, 777, true);
        let tx_false = make_ext_tx(&authority_false, 888, false);
        let entry = tx_true.entrypoint_hash;

        assert!(P::EntryEq(entry).applies(&tx_true));
        assert!(!P::EntryEq(entry).applies(&tx_false));
        assert!(P::EntryIn(vec![entry]).applies(&tx_true));
        assert!(P::EntryExists(true).applies(&tx_true));
        assert!(!P::EntryExists(false).applies(&tx_true));

        assert!(P::ResultEq(true).applies(&tx_true));
        assert!(P::ResultNe(true).applies(&tx_false));
        assert!(P::ResultIn(vec![true]).applies(&tx_true));
        assert!(P::ResultNin(vec![true]).applies(&tx_false));
        assert!(P::ResultExists(true).applies(&tx_true));
        assert!(!P::ResultExists(false).applies(&tx_true));
    }

    #[test]
    fn boolean_composition_across_fields() {
        let authority_a = TestAuthority::new(0x66);
        let authority_b = TestAuthority::new(0x77);
        let a = authority_a.id_str();
        let account_a = account::AccountId::parse_encoded(&a)
            .expect("authority A")
            .into_account_id();
        let tx_a_true = make_ext_tx(&authority_a, 1500, true);
        let tx_b_false = make_ext_tx(&authority_b, 500, false);

        // (authority == A AND ts >= 1000) OR (result_ok == false)
        let left = P::And(vec![P::AuthorityEq(account_a), P::TsGte(1000)]);
        let pred = P::Or(vec![left, P::ResultEq(false)]);
        assert!(pred.applies(&tx_a_true));
        assert!(pred.applies(&tx_b_false));
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

impl<T> Clone for CompoundPredicate<T> {
    fn clone(&self) -> Self {
        Self {
            payload: self.payload.clone(),
            marker: PhantomData,
        }
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

impl CompoundPredicate<crate::query::CommittedTransaction> {
    /// Build a predicate from an existing filter set.
    pub fn from_filters(filters: crate::query::CommittedTxFilters) -> Self {
        let tree = filters_to_tx_predicate(filters);
        if matches!(tree, CommittedTxPredicate::Const(true)) {
            Self::PASS
        } else {
            Self::with_payload(std::sync::Arc::new(tree))
        }
    }

    /// Construct from a typed predicate tree for committed transactions.
    pub fn from_committed_tx_predicate(tree: CommittedTxPredicate) -> Self {
        Self::with_payload(std::sync::Arc::new(tree))
    }

    fn payload_any(&self) -> Option<&std::sync::Arc<dyn core::any::Any + Send + Sync + 'static>> {
        self.payload.as_ref()
    }

    /// Return committed-transaction filters carried by this predicate, if available.
    pub fn committed_tx_filters(&self) -> Option<crate::query::CommittedTxFilters> {
        self.payload_any()?
            .downcast_ref::<CommittedTxPredicate>()
            .and_then(committed_tx_filters_from_predicate)
    }

    /// Evaluate the predicate against a committed transaction.
    pub fn applies(&self, input: &crate::query::CommittedTransaction) -> bool {
        if let Some(p) = self.payload_any()
            && let Some(tree) = p.downcast_ref::<CommittedTxPredicate>()
        {
            return tree.applies(input);
        }
        self.payload_any().is_none()
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
    /// Element type accepted by the selector.
    type SelectingType;
    /// Type produced by the selector.
    type SelectedType;
    /// Convert the receiver into a selector instance.
    fn into_selector(self) -> ();
}

/// Trait defining conversion into a selector tuple.
pub trait IntoSelectorTuple {
    /// Element type accepted by the selector tuple.
    type SelectingType;
    /// Concrete selector tuple produced by the conversion.
    type SelectedTuple;
    /// Convert the receiver into a selector tuple.
    fn into_selector_tuple(self) -> SelectorTuple<Self::SelectingType>;
}

impl<T> IntoSelectorTuple for SelectorTuple<T> {
    type SelectingType = T;
    type SelectedTuple = T;
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
    /// Project a batch of references into a serializable batch box.
    ///
    /// # Errors
    /// Returns an error if projection is not supported for this selector or target type.
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

    /// Project a batch of owned items into a serializable batch box.
    ///
    /// # Errors
    /// Returns an error if projection is not supported for this selector or target type.
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

impl<T: 'static> EvaluateSelector<T> for () {}

// Experimental ids-only projection: When the selector tuple is in `IdsOnly` mode, `iter()`
// yields a single unit value. We conditionally implement EvaluateSelector for `()` to map
// well-known types to their id vectors.
#[cfg(feature = "ids_projection")]
impl EvaluateSelector<crate::domain::Domain> for () {
    fn project_clone<'a, I>(
        &self,
        batch: I,
    ) -> Result<crate::query::QueryOutputBatchBox, crate::query::error::QueryExecutionFail>
    where
        I: Iterator<Item = &'a crate::domain::Domain> + 'a,
    {
        let ids: Vec<crate::domain::DomainId> = batch.map(|d| d.id().clone()).collect();
        Ok(crate::query::QueryOutputBatchBox::from(ids))
    }
    fn project(
        &self,
        batch: impl Iterator<Item = crate::domain::Domain>,
    ) -> Result<crate::query::QueryOutputBatchBox, crate::query::error::QueryExecutionFail> {
        let ids: Vec<crate::domain::DomainId> = batch.map(|d| d.id().clone()).collect();
        Ok(crate::query::QueryOutputBatchBox::from(ids))
    }
}

#[cfg(feature = "ids_projection")]
impl EvaluateSelector<crate::account::Account> for () {
    fn project_clone<'a, I>(
        &self,
        batch: I,
    ) -> Result<crate::query::QueryOutputBatchBox, crate::query::error::QueryExecutionFail>
    where
        I: Iterator<Item = &'a crate::account::Account> + 'a,
    {
        let ids: Vec<crate::account::AccountId> = batch.map(|a| a.id().clone()).collect();
        Ok(crate::query::QueryOutputBatchBox::from(ids))
    }
    fn project(
        &self,
        batch: impl Iterator<Item = crate::account::Account>,
    ) -> Result<crate::query::QueryOutputBatchBox, crate::query::error::QueryExecutionFail> {
        let ids: Vec<crate::account::AccountId> = batch.map(|a| a.id().clone()).collect();
        Ok(crate::query::QueryOutputBatchBox::from(ids))
    }
}

#[cfg(feature = "ids_projection")]
impl EvaluateSelector<crate::asset::definition::AssetDefinition> for () {
    fn project_clone<'a, I>(
        &self,
        batch: I,
    ) -> Result<crate::query::QueryOutputBatchBox, crate::query::error::QueryExecutionFail>
    where
        I: Iterator<Item = &'a crate::asset::definition::AssetDefinition> + 'a,
    {
        let ids: Vec<crate::asset::definition::AssetDefinitionId> =
            batch.map(|ad| ad.id().clone()).collect();
        Ok(crate::query::QueryOutputBatchBox::from(ids))
    }
    fn project(
        &self,
        batch: impl Iterator<Item = crate::asset::definition::AssetDefinition>,
    ) -> Result<crate::query::QueryOutputBatchBox, crate::query::error::QueryExecutionFail> {
        let ids: Vec<crate::asset::definition::AssetDefinitionId> =
            batch.map(|ad| ad.id().clone()).collect();
        Ok(crate::query::QueryOutputBatchBox::from(ids))
    }
}

#[cfg(feature = "ids_projection")]
impl EvaluateSelector<crate::nft::Nft> for () {
    fn project_clone<'a, I>(
        &self,
        batch: I,
    ) -> Result<crate::query::QueryOutputBatchBox, crate::query::error::QueryExecutionFail>
    where
        I: Iterator<Item = &'a crate::nft::Nft> + 'a,
    {
        let ids: Vec<crate::nft::NftId> = batch.map(|n| n.id().clone()).collect();
        Ok(crate::query::QueryOutputBatchBox::from(ids))
    }
    fn project(
        &self,
        batch: impl Iterator<Item = crate::nft::Nft>,
    ) -> Result<crate::query::QueryOutputBatchBox, crate::query::error::QueryExecutionFail> {
        let ids: Vec<crate::nft::NftId> = batch.map(|n| n.id().clone()).collect();
        Ok(crate::query::QueryOutputBatchBox::from(ids))
    }
}

#[cfg(feature = "ids_projection")]
impl EvaluateSelector<crate::rwa::Rwa> for () {
    fn project_clone<'a, I>(
        &self,
        batch: I,
    ) -> Result<crate::query::QueryOutputBatchBox, crate::query::error::QueryExecutionFail>
    where
        I: Iterator<Item = &'a crate::rwa::Rwa> + 'a,
    {
        let ids: Vec<crate::rwa::RwaId> = batch.map(|rwa| rwa.id().clone()).collect();
        Ok(crate::query::QueryOutputBatchBox::from(ids))
    }
    fn project(
        &self,
        batch: impl Iterator<Item = crate::rwa::Rwa>,
    ) -> Result<crate::query::QueryOutputBatchBox, crate::query::error::QueryExecutionFail> {
        let ids: Vec<crate::rwa::RwaId> = batch.map(|rwa| rwa.id().clone()).collect();
        Ok(crate::query::QueryOutputBatchBox::from(ids))
    }
}

#[cfg(feature = "ids_projection")]
impl EvaluateSelector<crate::role::Role> for () {
    fn project_clone<'a, I>(
        &self,
        batch: I,
    ) -> Result<crate::query::QueryOutputBatchBox, crate::query::error::QueryExecutionFail>
    where
        I: Iterator<Item = &'a crate::role::Role> + 'a,
    {
        let ids: Vec<crate::role::RoleId> = batch.map(|r| r.id().clone()).collect();
        Ok(crate::query::QueryOutputBatchBox::from(ids))
    }
    fn project(
        &self,
        batch: impl Iterator<Item = crate::role::Role>,
    ) -> Result<crate::query::QueryOutputBatchBox, crate::query::error::QueryExecutionFail> {
        let ids: Vec<crate::role::RoleId> = batch.map(|r| r.id().clone()).collect();
        Ok(crate::query::QueryOutputBatchBox::from(ids))
    }
}

#[cfg(feature = "ids_projection")]
impl EvaluateSelector<crate::trigger::Trigger> for () {
    fn project_clone<'a, I>(
        &self,
        batch: I,
    ) -> Result<crate::query::QueryOutputBatchBox, crate::query::error::QueryExecutionFail>
    where
        I: Iterator<Item = &'a crate::trigger::Trigger> + 'a,
    {
        let ids: Vec<crate::trigger::TriggerId> = batch.map(|t| t.id().clone()).collect();
        Ok(crate::query::QueryOutputBatchBox::from(ids))
    }
    fn project(
        &self,
        batch: impl Iterator<Item = crate::trigger::Trigger>,
    ) -> Result<crate::query::QueryOutputBatchBox, crate::query::error::QueryExecutionFail> {
        let ids: Vec<crate::trigger::TriggerId> = batch.map(|t| t.id().clone()).collect();
        Ok(crate::query::QueryOutputBatchBox::from(ids))
    }
}

#[cfg(test)]
mod evaluate_selector_tests {
    use super::EvaluateSelector;

    fn assert_impl_selector<S: EvaluateSelector<u32>>() {}

    #[test]
    fn unit_type_implements_evaluate_selector() {
        assert_impl_selector::<()>();
    }
}

/// Prelude re-export for the classic query DSL.
pub mod prelude {
    pub use super::{
        BaseProjector, CompoundPredicate, IntoSelector, IntoSelectorTuple, PredicateMarker,
        SelectorMarker, SelectorTuple,
    };
}
