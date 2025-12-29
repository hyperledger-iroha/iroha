//! Example of custom expression system.
//!
//! Only few expressions are implemented to show proof-of-concept.
//! See `crates/ivm/target/prebuilt/samples/executor_custom_instructions_complex`.
//! This is simplified version of expression system from Iroha v2.0.0-pre-rc.20

pub use evaluate::*;
pub use expression::*;
pub use isi::*;

mod isi {
    use std::{borrow::ToOwned, boxed::Box, format, string::String, vec::Vec};

    use iroha_data_model::{
        isi::{CustomInstruction, InstructionBox},
        prelude::Json,
    };
    use iroha_schema::IntoSchema;
    use norito::{
        derive::{JsonDeserialize, JsonSerialize},
        json,
    };

    use crate::complex_isi::expression::EvaluatesTo;

    #[derive(Debug, IntoSchema)]
    pub enum CustomInstructionExpr {
        Core(CoreExpr),
        If(Box<ConditionalExpr>),
        // Other custom instructions
    }

    impl From<CoreExpr> for CustomInstructionExpr {
        fn from(isi: CoreExpr) -> Self {
            Self::Core(isi)
        }
    }

    impl From<ConditionalExpr> for CustomInstructionExpr {
        fn from(isi: ConditionalExpr) -> Self {
            Self::If(Box::new(isi))
        }
    }

    // NOTE: Do not implement the sealed `Instruction` trait for custom types.
    // These expressions are wrapped into `CustomInstruction` and then into
    // `InstructionBox` via the `From` impls below.

    impl From<CustomInstructionExpr> for CustomInstruction {
        fn from(isi: CustomInstructionExpr) -> Self {
            let payload =
                json::to_value(&isi).expect("INTERNAL BUG: Couldn't serialize custom instruction");

            Self::new(payload)
        }
    }

    impl From<CustomInstructionExpr> for InstructionBox {
        fn from(isi: CustomInstructionExpr) -> Self {
            InstructionBox::from(CustomInstruction::from(isi))
        }
    }

    impl From<CoreExpr> for InstructionBox {
        fn from(isi: CoreExpr) -> Self {
            InstructionBox::from(CustomInstruction::from(CustomInstructionExpr::from(isi)))
        }
    }

    impl From<ConditionalExpr> for InstructionBox {
        fn from(isi: ConditionalExpr) -> Self {
            InstructionBox::from(CustomInstruction::from(CustomInstructionExpr::from(isi)))
        }
    }

    impl TryFrom<&Json> for CustomInstructionExpr {
        type Error = json::Error;

        fn try_from(payload: &Json) -> Result<Self, json::Error> {
            json::from_str::<Self>(payload.as_ref())
        }
    }

    impl json::JsonSerialize for CustomInstructionExpr {
        fn json_serialize(&self, out: &mut String) {
            out.push('{');
            match self {
                Self::Core(value) => {
                    json::write_json_string("Core", out);
                    out.push(':');
                    json::JsonSerialize::json_serialize(value, out);
                }
                Self::If(expr) => {
                    json::write_json_string("If", out);
                    out.push(':');
                    json::JsonSerialize::json_serialize(expr, out);
                }
            }
            out.push('}');
        }
    }

    impl json::JsonDeserialize for CustomInstructionExpr {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let mut visitor = json::MapVisitor::new(parser)?;
            let mut variant: Option<Self> = None;

            while let Some(key) = visitor.next_key()? {
                let key_str = key.as_str();
                match key_str {
                    "Core" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(key_str.to_owned()));
                        }
                        let value = visitor.parse_value::<CoreExpr>()?;
                        variant = Some(Self::Core(value));
                    }
                    "If" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(key_str.to_owned()));
                        }
                        let value = visitor.parse_value::<ConditionalExpr>()?;
                        variant = Some(Self::If(Box::new(value)));
                    }
                    _ => {
                        visitor.skip_value()?;
                        return Err(json::Error::unknown_field(key_str.to_owned()));
                    }
                }
            }

            visitor.finish()?;

            variant.ok_or_else(|| json::Error::missing_field("CustomInstructionExpr"))
        }
    }

    // Built-in instruction (can be evaluated based on query values, etc)
    #[derive(Debug, JsonDeserialize, JsonSerialize, IntoSchema)]
    pub struct CoreExpr {
        pub object: EvaluatesTo<InstructionBox>,
    }

    impl CoreExpr {
        pub fn new(object: impl Into<EvaluatesTo<InstructionBox>>) -> Self {
            Self {
                object: object.into(),
            }
        }
    }

    /// Composite instruction for a conditional execution of other instructions.
    #[derive(Debug, JsonDeserialize, JsonSerialize, IntoSchema)]
    pub struct ConditionalExpr {
        /// Condition to be checked.
        pub condition: EvaluatesTo<bool>,
        /// Instruction to be executed if condition pass.
        pub then: CustomInstructionExpr,
    }

    impl ConditionalExpr {
        pub fn new(
            condition: impl Into<EvaluatesTo<bool>>,
            then: impl Into<CustomInstructionExpr>,
        ) -> Self {
            Self {
                condition: condition.into(),
                then: then.into(),
            }
        }
    }
}

mod expression {
    use core::marker::PhantomData;
    use std::{
        borrow::ToOwned,
        boxed::Box,
        format,
        string::{String, ToString},
        vec,
        vec::Vec,
    };

    use iroha_data_model::{
        asset::{AssetDefinitionId, AssetId},
        isi::InstructionBox,
        prelude::Numeric,
    };
    use iroha_schema::{IntoSchema, TypeId};
    use norito::{
        derive::{JsonDeserialize, JsonSerialize},
        json,
    };

    /// Struct for type checking and converting expression results.
    #[derive(Debug, TypeId)]
    pub struct EvaluatesTo<V> {
        pub(crate) expression: Box<Expression>,
        type_: PhantomData<V>,
    }

    impl<V> EvaluatesTo<V> {
        pub fn new_unchecked(expression: impl Into<Expression>) -> Self {
            Self {
                expression: Box::new(expression.into()),
                type_: PhantomData,
            }
        }
    }

    impl<V> json::JsonSerialize for EvaluatesTo<V> {
        fn json_serialize(&self, out: &mut String) {
            json::JsonSerialize::json_serialize(&self.expression, out);
        }
    }

    impl<V> json::JsonDeserialize for EvaluatesTo<V> {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let expr = Expression::json_deserialize(parser)?;
            Ok(Self {
                expression: Box::new(expr),
                type_: PhantomData,
            })
        }
    }

    /// Represents all possible queries returning a numerical result.
    #[derive(Debug, Clone, IntoSchema)]
    pub enum NumericQuery {
        FindAssetQuantityById(AssetId),
        FindTotalAssetQuantityByAssetDefinitionId(AssetDefinitionId),
    }

    /// Represents all possible expressions.
    #[derive(Debug, IntoSchema)]
    pub enum Expression {
        /// Raw value.
        Raw(Value),
        /// Greater expression.
        Greater(Greater),
        /// Query to Iroha state.
        Query(NumericQuery),
    }

    /// Returns whether the `left` expression is greater than the `right`.
    #[derive(Debug, JsonDeserialize, JsonSerialize, IntoSchema)]
    pub struct Greater {
        pub left: EvaluatesTo<Numeric>,
        pub right: EvaluatesTo<Numeric>,
    }

    impl Greater {
        /// Construct new [`Greater`] expression
        pub fn new(
            left: impl Into<EvaluatesTo<Numeric>>,
            right: impl Into<EvaluatesTo<Numeric>>,
        ) -> Self {
            Self {
                left: left.into(),
                right: right.into(),
            }
        }
    }

    impl From<Greater> for EvaluatesTo<bool> {
        fn from(expression: Greater) -> Self {
            let expression = Expression::Greater(expression);
            EvaluatesTo::new_unchecked(expression)
        }
    }

    /// Sized container for all possible values.
    #[derive(Debug, Clone, IntoSchema)]
    pub enum Value {
        Bool(bool),
        Numeric(Numeric),
        InstructionBox(InstructionBox),
    }

    impl From<bool> for Value {
        fn from(value: bool) -> Self {
            Value::Bool(value)
        }
    }

    impl From<Numeric> for EvaluatesTo<Numeric> {
        fn from(value: Numeric) -> Self {
            let value = Value::Numeric(value);
            let expression = Expression::Raw(value);
            EvaluatesTo::new_unchecked(expression)
        }
    }

    impl From<InstructionBox> for EvaluatesTo<InstructionBox> {
        fn from(value: InstructionBox) -> Self {
            let value = Value::InstructionBox(value);
            let expression = Expression::Raw(value);
            EvaluatesTo::new_unchecked(expression)
        }
    }

    impl TryFrom<Value> for bool {
        type Error = String;

        fn try_from(value: Value) -> Result<Self, Self::Error> {
            match value {
                Value::Bool(value) => Ok(value),
                _ => Err("Expected bool".to_string()),
            }
        }
    }

    impl TryFrom<Value> for Numeric {
        type Error = String;

        fn try_from(value: Value) -> Result<Self, Self::Error> {
            match value {
                Value::Numeric(value) => Ok(value),
                _ => Err("Expected Numeric".to_string()),
            }
        }
    }

    impl TryFrom<Value> for InstructionBox {
        type Error = String;

        fn try_from(value: Value) -> Result<Self, Self::Error> {
            match value {
                Value::InstructionBox(value) => Ok(value),
                _ => Err("Expected InstructionBox".to_string()),
            }
        }
    }

    impl<V: TryFrom<Value> + IntoSchema> IntoSchema for EvaluatesTo<V> {
        fn type_name() -> String {
            format!("EvaluatesTo<{}>", V::type_name())
        }
        fn update_schema_map(map: &mut iroha_schema::MetaMap) {
            const EXPRESSION: &str = "expression";

            if !map.contains_key::<Self>() {
                map.insert::<Self>(iroha_schema::Metadata::Struct(
                    iroha_schema::NamedFieldsMeta {
                        declarations: vec![iroha_schema::Declaration {
                            name: String::from(EXPRESSION),
                            ty: core::any::TypeId::of::<Expression>(),
                        }],
                    },
                ));

                Expression::update_schema_map(map);
            }
        }
    }

    impl json::JsonSerialize for NumericQuery {
        fn json_serialize(&self, out: &mut String) {
            out.push('{');
            match self {
                Self::FindAssetQuantityById(id) => {
                    json::write_json_string("FindAssetQuantityById", out);
                    out.push(':');
                    json::JsonSerialize::json_serialize(id, out);
                }
                Self::FindTotalAssetQuantityByAssetDefinitionId(id) => {
                    json::write_json_string("FindTotalAssetQuantityByAssetDefinitionId", out);
                    out.push(':');
                    json::JsonSerialize::json_serialize(id, out);
                }
            }
            out.push('}');
        }
    }

    impl json::JsonDeserialize for NumericQuery {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let mut visitor = json::MapVisitor::new(parser)?;
            let mut variant: Option<Self> = None;

            while let Some(key) = visitor.next_key()? {
                let key_str = key.as_str();
                match key_str {
                    "FindAssetQuantityById" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(key_str.to_owned()));
                        }
                        let value = visitor.parse_value::<AssetId>()?;
                        variant = Some(Self::FindAssetQuantityById(value));
                    }
                    "FindTotalAssetQuantityByAssetDefinitionId" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(key_str.to_owned()));
                        }
                        let value = visitor.parse_value::<AssetDefinitionId>()?;
                        variant = Some(Self::FindTotalAssetQuantityByAssetDefinitionId(value));
                    }
                    _ => {
                        visitor.skip_value()?;
                        return Err(json::Error::unknown_field(key_str.to_owned()));
                    }
                }
            }

            visitor.finish()?;

            variant.ok_or_else(|| json::Error::missing_field("NumericQuery"))
        }
    }

    impl json::JsonSerialize for Expression {
        fn json_serialize(&self, out: &mut String) {
            out.push('{');
            match self {
                Self::Raw(value) => {
                    json::write_json_string("Raw", out);
                    out.push(':');
                    json::JsonSerialize::json_serialize(value, out);
                }
                Self::Greater(expr) => {
                    json::write_json_string("Greater", out);
                    out.push(':');
                    json::JsonSerialize::json_serialize(expr, out);
                }
                Self::Query(query) => {
                    json::write_json_string("Query", out);
                    out.push(':');
                    json::JsonSerialize::json_serialize(query, out);
                }
            }
            out.push('}');
        }
    }

    impl json::JsonDeserialize for Expression {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let mut visitor = json::MapVisitor::new(parser)?;
            let mut variant: Option<Self> = None;

            while let Some(key) = visitor.next_key()? {
                let key_str = key.as_str();
                match key_str {
                    "Raw" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(key_str.to_owned()));
                        }
                        let value = visitor.parse_value::<Value>()?;
                        variant = Some(Self::Raw(value));
                    }
                    "Greater" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(key_str.to_owned()));
                        }
                        let value = visitor.parse_value::<Greater>()?;
                        variant = Some(Self::Greater(value));
                    }
                    "Query" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(key_str.to_owned()));
                        }
                        let value = visitor.parse_value::<NumericQuery>()?;
                        variant = Some(Self::Query(value));
                    }
                    _ => {
                        visitor.skip_value()?;
                        return Err(json::Error::unknown_field(key_str.to_owned()));
                    }
                }
            }

            visitor.finish()?;

            variant.ok_or_else(|| json::Error::missing_field("Expression"))
        }
    }

    impl json::JsonSerialize for Value {
        fn json_serialize(&self, out: &mut String) {
            out.push('{');
            match self {
                Self::Bool(value) => {
                    json::write_json_string("Bool", out);
                    out.push(':');
                    json::JsonSerialize::json_serialize(value, out);
                }
                Self::Numeric(value) => {
                    json::write_json_string("Numeric", out);
                    out.push(':');
                    json::JsonSerialize::json_serialize(value, out);
                }
                Self::InstructionBox(value) => {
                    json::write_json_string("InstructionBox", out);
                    out.push(':');
                    json::JsonSerialize::json_serialize(value, out);
                }
            }
            out.push('}');
        }
    }

    impl json::JsonDeserialize for Value {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let mut visitor = json::MapVisitor::new(parser)?;
            let mut variant: Option<Self> = None;

            while let Some(key) = visitor.next_key()? {
                let key_str = key.as_str();
                match key_str {
                    "Bool" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(key_str.to_owned()));
                        }
                        let value = visitor.parse_value::<bool>()?;
                        variant = Some(Self::Bool(value));
                    }
                    "Numeric" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(key_str.to_owned()));
                        }
                        let value = visitor.parse_value::<Numeric>()?;
                        variant = Some(Self::Numeric(value));
                    }
                    "InstructionBox" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(key_str.to_owned()));
                        }
                        let value = visitor.parse_value::<InstructionBox>()?;
                        variant = Some(Self::InstructionBox(value));
                    }
                    _ => {
                        visitor.skip_value()?;
                        return Err(json::Error::unknown_field(key_str.to_owned()));
                    }
                }
            }

            visitor.finish()?;

            variant.ok_or_else(|| json::Error::missing_field("Value"))
        }
    }
}

mod evaluate {
    use std::string::ToString;

    use iroha_data_model::{ValidationFail, isi::error::InstructionExecutionError};

    use crate::complex_isi::{
        NumericQuery,
        expression::{EvaluatesTo, Expression, Greater, Value},
    };

    pub trait Evaluate {
        /// The resulting type of the expression.
        type Value;

        /// Calculate result.
        fn evaluate(&self, context: &impl Context) -> Result<Self::Value, ValidationFail>;
    }

    pub trait Context {
        /// Execute query against the current state of `Iroha`
        fn query(&self, query: &NumericQuery) -> Result<Value, ValidationFail>;
    }

    impl<V: TryFrom<Value>> Evaluate for EvaluatesTo<V>
    where
        V::Error: ToString,
    {
        type Value = V;

        fn evaluate(&self, context: &impl Context) -> Result<Self::Value, ValidationFail> {
            let expr = self.expression.evaluate(context)?;
            V::try_from(expr)
                .map_err(|e| InstructionExecutionError::Conversion(e.to_string()))
                .map_err(ValidationFail::InstructionFailed)
        }
    }

    impl Evaluate for Expression {
        type Value = Value;

        fn evaluate(&self, context: &impl Context) -> Result<Self::Value, ValidationFail> {
            match self {
                Expression::Raw(value) => Ok(value.clone()),
                Expression::Greater(expr) => expr.evaluate(context).map(Into::into),
                Expression::Query(expr) => expr.evaluate(context),
            }
        }
    }

    impl Evaluate for Greater {
        type Value = bool;

        fn evaluate(&self, context: &impl Context) -> Result<Self::Value, ValidationFail> {
            let left = self.left.evaluate(context)?;
            let right = self.right.evaluate(context)?;
            Ok(left > right)
        }
    }

    impl Evaluate for NumericQuery {
        type Value = Value;

        fn evaluate(&self, context: &impl Context) -> Result<Self::Value, ValidationFail> {
            context.query(self)
        }
    }
}
