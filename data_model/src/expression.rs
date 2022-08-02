//! Expressions to use inside of ISIs.

#![allow(
    // Because of `codec(skip)`
    clippy::default_trait_access,
    // Because of length on instructions and expressions (can't be 0)
    clippy::len_without_is_empty,
    // Because of length on instructions and expressions (XXX: Should it be trait?)
    clippy::unused_self
)]

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, collections::btree_map, format, string::String, vec, vec::Vec};
use core::marker::PhantomData;
#[cfg(feature = "std")]
use std::collections::btree_map;

use derive_more::Display;
use iroha_macro::FromVariant;
use iroha_schema::prelude::*;
use operation::*;
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use super::{query::QueryBox, Value, ValueBox};

/// Bound name for a value.
pub type ValueName = String;

/// Context, composed of (name, value) pairs.
pub type Context = btree_map::BTreeMap<ValueName, Value>;

/// Boxed expression.
pub type ExpressionBox = Box<Expression>;

/// Struct for type checking and converting expression results.
#[derive(
    Debug, Display, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize, PartialOrd, Ord,
)]
#[serde(transparent)]
// As this structure exists only for type checking
// it makes sense to display `expression` directly
#[display(fmt = "{}", expression)]
pub struct EvaluatesTo<V: TryFrom<Value>> {
    /// Expression.
    #[serde(flatten)]
    pub expression: ExpressionBox,
    #[codec(skip)]
    _value_type: PhantomData<V>,
}

impl<V: TryFrom<Value>, E: Into<ExpressionBox> + Into<V>> From<E> for EvaluatesTo<V> {
    fn from(expression: E) -> Self {
        Self::new_unchecked(expression.into())
    }
}

impl<V: TryFrom<Value>> EvaluatesTo<V> {
    /// Number of underneath expressions.
    #[inline]
    pub fn len(&self) -> usize {
        self.expression.len()
    }

    /// Construct new [`EvaluatesTo`] from [`ExpressionBox`] without type checking.
    ///
    /// # Warning
    /// Prefer using [`Into`] conversions rather than this method,
    /// cause it does not check type at compile-time.
    ///
    /// Exists mainly to test unsupported expressions.
    #[inline]
    pub fn new_unchecked(expression: ExpressionBox) -> Self {
        Self {
            expression,
            _value_type: PhantomData::default(),
        }
    }

    fn operation(&self) -> Operation {
        use Expression::*;

        match self.expression.as_ref() {
            Add(_) => Operation::Add,
            Subtract(_) => Operation::Subtract,
            Multiply(_) => Operation::Multiply,
            Divide(_) => Operation::Divide,
            Mod(_) => Operation::Mod,
            RaiseTo(_) => Operation::RaiseTo,
            Greater(_) => Operation::Greater,
            Less(_) => Operation::Less,
            Equal(_) => Operation::Equal,
            Not(_) => Operation::Not,
            And(_) => Operation::And,
            Or(_) => Operation::Or,
            Contains(_) | ContainsAll(_) | ContainsAny(_) => Operation::MethodCall,
            If(_) | Raw(_) | Query(_) | Where(_) | ContextValue(_) => Operation::Other,
        }
    }

    /// Wrap expression into parentheses depending on `operation` and get the resulting string.
    ///
    /// # Panics
    /// - If `operation` has [`Other`](Operation::Other) value.
    fn parenthesise(&self, operation: Operation) -> String {
        if self.operation().priority() < operation.priority()
            && !matches!(self.expression.as_ref(), Expression::Raw(_))
        {
            format!("({})", self.expression)
        } else {
            format!("{}", self.expression)
        }
    }
}

impl EvaluatesTo<Value> {
    /// Construct `EvaluatesTo<Value>` from any `expression`
    /// because all of them evaluate to [`Value`].
    #[inline]
    pub fn new_evaluates_to_value(expression: ExpressionBox) -> Self {
        Self::new_unchecked(expression)
    }
}

impl<V: IntoSchema + TryFrom<Value>> IntoSchema for EvaluatesTo<V> {
    fn type_name() -> String {
        format!("{}::EvaluatesTo<{}>", module_path!(), V::type_name())
    }
    fn schema(map: &mut MetaMap) {
        ExpressionBox::schema(map);

        map.entry(Self::type_name()).or_insert_with(|| {
            const EXPRESSION: &str = "expression";

            Metadata::Struct(NamedFieldsMeta {
                declarations: vec![Declaration {
                    name: String::from(EXPRESSION),
                    ty: ExpressionBox::type_name(),
                }],
            })
        });
    }
}

mod operation {
    //! Module containing operations and their priorities.

    /// Type of expression operation.
    #[derive(Copy, Clone, PartialEq, Eq)]
    pub enum Operation {
        MethodCall,
        RaiseTo,
        Multiply,
        Divide,
        Mod,
        Add,
        Subtract,
        Greater,
        Less,
        Equal,
        Not,
        And,
        Or,
        Other,
    }

    /// Priority of operation.
    ///
    /// [`First`](Operation::First) is the highest priority
    /// and [`Eight`](Operation::Eight) is the lowest.
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub enum Priority {
        First = 1,
        Second = 2,
        Third = 3,
        Fourth = 4,
        Fifth = 5,
        Sixth = 6,
        Seventh = 7,
        Eighth = 8,
        Ninth = 9,
    }

    impl Operation {
        /// Get the priority of the operation.
        ///
        /// Ordering is the same as in Python code.
        /// See [`here`](https://docs.python.org/3/reference/expressions.html#operator-precedence)
        /// for more details.
        pub fn priority(self) -> Priority {
            use Operation::*;

            match self {
                MethodCall => Priority::First,
                RaiseTo => Priority::Second,
                Multiply | Divide | Mod => Priority::Third,
                Add | Subtract => Priority::Fourth,
                Greater | Less | Equal => Priority::Fifth,
                Not => Priority::Sixth,
                And => Priority::Seventh,
                Or => Priority::Eighth,
                Other => Priority::Ninth,
            }
        }
    }

    impl PartialOrd for Priority {
        fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for Priority {
        fn cmp(&self, other: &Self) -> core::cmp::Ordering {
            use core::cmp::Ordering::*;

            let lhs = *self as u8;
            let rhs = *other as u8;

            match lhs.cmp(&rhs) {
                Less => Greater,
                Equal => Equal,
                Greater => Less,
            }
        }
    }
}

/// Represents all possible expressions.
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    FromVariant,
    IntoSchema,
    PartialOrd,
    Ord,
)]
pub enum Expression {
    /// Add expression.
    Add(Add),
    /// Subtract expression.
    Subtract(Subtract),
    /// Multiply expression.
    Multiply(Multiply),
    /// Divide expression.
    Divide(Divide),
    /// Module expression.
    Mod(Mod),
    /// Raise to power expression.
    RaiseTo(RaiseTo),
    /// Greater expression.
    Greater(Greater),
    /// Less expression.
    Less(Less),
    /// Equal expression.
    Equal(Equal),
    /// Not expression.
    Not(Not),
    /// And expression.
    And(And),
    /// Or expression.
    Or(Or),
    /// If expression.
    If(If),
    /// Raw value.
    Raw(ValueBox),
    /// Query to Iroha state.
    Query(QueryBox),
    /// Contains expression for vectors.
    Contains(Contains),
    /// Contains all expression for vectors.
    ContainsAll(ContainsAll),
    /// Contains any expression for vectors.
    ContainsAny(ContainsAny),
    /// Where expression to supply temporary values to local context.
    Where(Where),
    /// Get a temporary value by name
    ContextValue(ContextValue),
}

impl Expression {
    /// Number of underneath expressions.
    #[inline]
    pub fn len(&self) -> usize {
        use Expression::*;

        match self {
            Add(add) => add.len(),
            Subtract(subtract) => subtract.len(),
            Greater(greater) => greater.len(),
            Less(less) => less.len(),
            Equal(equal) => equal.len(),
            Not(not) => not.len(),
            And(and) => and.len(),
            Or(or) => or.len(),
            If(if_expression) => if_expression.len(),
            Raw(raw) => raw.len(),
            Query(query) => query.len(),
            Contains(contains) => contains.len(),
            ContainsAll(contains_all) => contains_all.len(),
            ContainsAny(contains_any) => contains_any.len(),
            Where(where_expression) => where_expression.len(),
            ContextValue(context_value) => context_value.len(),
            Multiply(multiply) => multiply.len(),
            Divide(divide) => divide.len(),
            Mod(modulus) => modulus.len(),
            RaiseTo(raise_to) => raise_to.len(),
        }
    }
}

impl<T: Into<Value>> From<T> for ExpressionBox {
    fn from(value: T) -> Self {
        Expression::Raw(Box::new(value.into())).into()
    }
}

/// Get a temporary value by name.
/// The values are brought into [`Context`] by [`Where`] expression.
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(fmt = "CONTEXT `{}`", value_name)]
pub struct ContextValue {
    /// Name bound to the value.
    pub value_name: String,
}

impl ContextValue {
    /// Number of underneath expressions.
    #[inline]
    pub const fn len(&self) -> usize {
        1
    }

    /// Constructs `ContextValue`.
    #[inline]
    pub fn new(value_name: &str) -> Self {
        Self {
            value_name: String::from(value_name),
        }
    }
}

impl From<ContextValue> for ExpressionBox {
    #[inline]
    fn from(expression: ContextValue) -> Self {
        Expression::ContextValue(expression).into()
    }
}

/// Evaluates to the multiplication of right and left expressions.
/// Works only for [`Value::U32`]
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "{}*{}", // Keep without spaces
    "left.parenthesise(Operation::Multiply)",
    "right.parenthesise(Operation::Multiply)"
)]
pub struct Multiply {
    /// Left operand.
    pub left: EvaluatesTo<u32>,
    /// Right operand.
    pub right: EvaluatesTo<u32>,
}

impl Multiply {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.left.len() + self.right.len() + 1
    }

    /// Constructs `Multiply` expression.
    pub fn new(left: impl Into<EvaluatesTo<u32>>, right: impl Into<EvaluatesTo<u32>>) -> Self {
        Self {
            left: left.into(),
            right: right.into(),
        }
    }
}

impl From<Multiply> for ExpressionBox {
    fn from(expression: Multiply) -> Self {
        Expression::Multiply(expression).into()
    }
}

impl From<Multiply> for EvaluatesTo<u32> {
    fn from(expression: Multiply) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// Evaluates to the division of right and left expressions.
/// Works only for [`Value::U32`]
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "{}/{}", // Keep without spaces
    "left.parenthesise(Operation::Divide)",
    "right.parenthesise(Operation::Divide)"
)]
pub struct Divide {
    /// Left operand.
    pub left: EvaluatesTo<u32>,
    /// Right operand.
    pub right: EvaluatesTo<u32>,
}

impl Divide {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.left.len() + self.right.len() + 1
    }

    /// Constructs `Multiply` expression.
    pub fn new(left: impl Into<EvaluatesTo<u32>>, right: impl Into<EvaluatesTo<u32>>) -> Self {
        Self {
            left: left.into(),
            right: right.into(),
        }
    }
}

impl From<Divide> for ExpressionBox {
    fn from(expression: Divide) -> Self {
        Expression::Divide(expression).into()
    }
}

impl From<Divide> for EvaluatesTo<u32> {
    fn from(expression: Divide) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// Evaluates to the modulus of right and left expressions.
/// Works only for [`Value::U32`]
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "{} % {}",
    "left.parenthesise(Operation::Mod)",
    "right.parenthesise(Operation::Mod)"
)]
pub struct Mod {
    /// Left operand.
    pub left: EvaluatesTo<u32>,
    /// Right operand.
    pub right: EvaluatesTo<u32>,
}

impl Mod {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.left.len() + self.right.len() + 1
    }

    /// Constructs `Mod` expression.
    pub fn new(left: impl Into<EvaluatesTo<u32>>, right: impl Into<EvaluatesTo<u32>>) -> Self {
        Self {
            left: left.into(),
            right: right.into(),
        }
    }
}

impl From<Mod> for ExpressionBox {
    fn from(expression: Mod) -> Self {
        Expression::Mod(expression).into()
    }
}

impl From<Mod> for EvaluatesTo<u32> {
    fn from(expression: Mod) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// Evaluates to the right expression in power of left expressions.
/// Works only for [`Value::U32`]
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "{}**{}",
    "left.parenthesise(Operation::RaiseTo)",
    "right.parenthesise(Operation::RaiseTo)"
)]
pub struct RaiseTo {
    /// Left operand.
    pub left: EvaluatesTo<u32>,
    /// Right operand.
    pub right: EvaluatesTo<u32>,
}

impl RaiseTo {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.left.len() + self.right.len() + 1
    }

    /// Constructs `RaiseTo` expression.
    pub fn new(left: impl Into<EvaluatesTo<u32>>, right: impl Into<EvaluatesTo<u32>>) -> Self {
        Self {
            left: left.into(),
            right: right.into(),
        }
    }
}

impl From<RaiseTo> for ExpressionBox {
    fn from(expression: RaiseTo) -> Self {
        Expression::RaiseTo(expression).into()
    }
}

impl From<RaiseTo> for EvaluatesTo<u32> {
    fn from(expression: RaiseTo) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// Evaluates to the sum of right and left expressions.
/// Works only for [`Value::U32`]
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "{}+{}",
    "left.parenthesise(Operation::Add)",
    "right.parenthesise(Operation::Add)"
)]
pub struct Add {
    /// Left operand.
    pub left: EvaluatesTo<u32>,
    /// Right operand.
    pub right: EvaluatesTo<u32>,
}

impl Add {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.left.len() + self.right.len() + 1
    }

    /// Constructs `Add` expression.
    pub fn new<L: Into<EvaluatesTo<u32>>, R: Into<EvaluatesTo<u32>>>(left: L, right: R) -> Self {
        Self {
            left: left.into(),
            right: right.into(),
        }
    }
}

impl From<Add> for ExpressionBox {
    fn from(expression: Add) -> Self {
        Expression::Add(expression).into()
    }
}

impl From<Add> for EvaluatesTo<u32> {
    fn from(expression: Add) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// Evaluates to the difference of right and left expressions.
/// Works only for [`Value::U32`]
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "{}-{}",
    "left.parenthesise(Operation::Subtract)",
    "right.parenthesise(Operation::Subtract)"
)]

pub struct Subtract {
    /// Left operand.
    pub left: EvaluatesTo<u32>,
    /// Right operand.
    pub right: EvaluatesTo<u32>,
}

impl Subtract {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.left.len() + self.right.len() + 1
    }

    /// Constructs `Subtract` expression.
    pub fn new<L: Into<EvaluatesTo<u32>>, R: Into<EvaluatesTo<u32>>>(left: L, right: R) -> Self {
        Self {
            left: left.into(),
            right: right.into(),
        }
    }
}

impl From<Subtract> for ExpressionBox {
    fn from(expression: Subtract) -> Self {
        Expression::Subtract(expression).into()
    }
}

impl From<Subtract> for EvaluatesTo<u32> {
    fn from(expression: Subtract) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// Returns whether the `left` expression is greater than the `right`.
/// Works only for [`Value::U32`].
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "{} > {}",
    "left.parenthesise(Operation::Greater)",
    "right.parenthesise(Operation::Greater)"
)]
pub struct Greater {
    /// Left operand.
    pub left: EvaluatesTo<u32>,
    /// Right operand.
    pub right: EvaluatesTo<u32>,
}

impl Greater {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.left.len() + self.right.len() + 1
    }

    /// Constructs `Greater` expression.
    pub fn new<L: Into<EvaluatesTo<u32>>, R: Into<EvaluatesTo<u32>>>(left: L, right: R) -> Self {
        Self {
            left: left.into(),
            right: right.into(),
        }
    }
}

impl From<Greater> for ExpressionBox {
    fn from(expression: Greater) -> Self {
        Expression::Greater(expression).into()
    }
}

impl From<Greater> for EvaluatesTo<bool> {
    fn from(expression: Greater) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// Returns whether the `left` expression is less than the `right`.
/// Works only for [`Value::U32`].
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "{} < {}",
    "left.parenthesise(Operation::Less)",
    "right.parenthesise(Operation::Less)"
)]
pub struct Less {
    /// Left operand.
    pub left: EvaluatesTo<u32>,
    /// Right operand.
    pub right: EvaluatesTo<u32>,
}

impl Less {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.left.len() + self.right.len() + 1
    }

    /// Constructs `Less` expression.
    pub fn new<L: Into<EvaluatesTo<u32>>, R: Into<EvaluatesTo<u32>>>(left: L, right: R) -> Self {
        Self {
            left: left.into(),
            right: right.into(),
        }
    }
}

impl From<Less> for ExpressionBox {
    fn from(expression: Less) -> Self {
        Expression::Less(expression).into()
    }
}

impl From<Less> for EvaluatesTo<bool> {
    fn from(expression: Less) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// Negates the result of the `expression`.
/// Works only for `Value::Bool`.
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(fmt = "!{}", "expression.parenthesise(Operation::Not)")]
pub struct Not {
    /// Expression that should evaluate to `Value::Bool`.
    pub expression: EvaluatesTo<bool>,
}

impl Not {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.expression.len() + 1
    }

    /// Constructs `Not` expression.
    pub fn new<E: Into<EvaluatesTo<bool>>>(expression: E) -> Self {
        Self {
            expression: expression.into(),
        }
    }
}

impl From<Not> for ExpressionBox {
    fn from(expression: Not) -> Self {
        Expression::Not(expression).into()
    }
}

impl From<Not> for EvaluatesTo<bool> {
    fn from(expression: Not) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// Applies the logical `and` to two `Value::Bool` operands.
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "{} && {}",
    "left.parenthesise(Operation::And)",
    "right.parenthesise(Operation::And)"
)]
pub struct And {
    /// Left operand.
    pub left: EvaluatesTo<bool>,
    /// Right operand.
    pub right: EvaluatesTo<bool>,
}

impl And {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.left.len() + self.right.len() + 1
    }

    /// Constructs `And` expression.
    pub fn new<L: Into<EvaluatesTo<bool>>, R: Into<EvaluatesTo<bool>>>(left: L, right: R) -> Self {
        Self {
            left: left.into(),
            right: right.into(),
        }
    }
}

impl From<And> for ExpressionBox {
    fn from(expression: And) -> Self {
        Expression::And(expression).into()
    }
}

impl From<And> for EvaluatesTo<bool> {
    fn from(expression: And) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// Applies the logical `or` to two `Value::Bool` operands.
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "{} || {}",
    "left.parenthesise(Operation::Or)",
    "right.parenthesise(Operation::Or)"
)]
pub struct Or {
    /// Left operand.
    pub left: EvaluatesTo<bool>,
    /// Right operand.
    pub right: EvaluatesTo<bool>,
}

impl Or {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.left.len() + self.right.len() + 1
    }

    /// Constructs `Or` expression.
    pub fn new<L: Into<EvaluatesTo<bool>>, R: Into<EvaluatesTo<bool>>>(left: L, right: R) -> Self {
        Self {
            left: left.into(),
            right: right.into(),
        }
    }
}

impl From<Or> for ExpressionBox {
    fn from(expression: Or) -> Self {
        Expression::Or(expression).into()
    }
}

impl From<Or> for EvaluatesTo<bool> {
    fn from(expression: Or) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// Builder for [`If`] expression.
#[derive(Debug)]
#[must_use = ".build() not used"]
pub struct IfBuilder {
    /// Condition expression, which should evaluate to `Value::Bool`.
    /// If it is `true` then the evaluated `then_expression` will be returned, else - evaluated `else_expression`.
    pub condition: EvaluatesTo<bool>,
    /// Expression evaluated and returned if the condition is `true`.
    pub then_expression: Option<EvaluatesTo<Value>>,
    /// Expression evaluated and returned if the condition is `false`.
    pub else_expression: Option<EvaluatesTo<Value>>,
}

impl IfBuilder {
    ///Sets the `condition`.
    pub fn condition<C: Into<EvaluatesTo<bool>>>(condition: C) -> Self {
        IfBuilder {
            condition: condition.into(),
            then_expression: None,
            else_expression: None,
        }
    }

    /// Sets `then_expression`.
    pub fn then_expression<E: Into<EvaluatesTo<Value>>>(self, expression: E) -> Self {
        IfBuilder {
            then_expression: Some(expression.into()),
            ..self
        }
    }

    /// Sets `else_expression`.
    pub fn else_expression<E: Into<EvaluatesTo<Value>>>(self, expression: E) -> Self {
        IfBuilder {
            else_expression: Some(expression.into()),
            ..self
        }
    }

    /// Returns [`If`] expression, if all the fields are filled.
    ///
    /// # Errors
    ///
    /// Fails if some of fields are not filled.
    pub fn build(self) -> Result<If, &'static str> {
        if let (Some(then_expression), Some(else_expression)) =
            (self.then_expression, self.else_expression)
        {
            return Ok(If::new(self.condition, then_expression, else_expression));
        }

        Err("Not all fields filled")
    }
}

/// If expression. Returns either a result of `then_expression`, or a result of `else_expression`
/// based on the `condition`.
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "if {} {{ {} }} else {{ {} }}",
    condition,
    then_expression,
    else_expression
)]
pub struct If {
    /// Condition expression, which should evaluate to `Value::Bool`.
    pub condition: EvaluatesTo<bool>,
    /// Expression evaluated and returned if the condition is `true`.
    pub then_expression: EvaluatesTo<Value>,
    /// Expression evaluated and returned if the condition is `false`.
    pub else_expression: EvaluatesTo<Value>,
}

impl If {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.condition.len() + self.then_expression.len() + self.else_expression.len() + 1
    }

    /// Constructs `If` expression.
    pub fn new<
        C: Into<EvaluatesTo<bool>>,
        T: Into<EvaluatesTo<Value>>,
        E: Into<EvaluatesTo<Value>>,
    >(
        condition: C,
        then_expression: T,
        else_expression: E,
    ) -> Self {
        Self {
            condition: condition.into(),
            then_expression: then_expression.into(),
            else_expression: else_expression.into(),
        }
    }
}

impl From<If> for ExpressionBox {
    fn from(if_expression: If) -> Self {
        Expression::If(if_expression).into()
    }
}

/// `Contains` expression.
/// Returns `true` if `collection` contains an `element`, `false` otherwise.
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "{}.contains({})",
    "collection.parenthesise(Operation::MethodCall)",
    "element"
)]
pub struct Contains {
    /// Expression, which should evaluate to `Value::Vec`.
    pub collection: EvaluatesTo<Vec<Value>>,
    /// Element expression.
    pub element: EvaluatesTo<Value>,
}

impl Contains {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.collection.len() + self.element.len() + 1
    }

    /// Constructs `Contains` expression.
    pub fn new<C: Into<EvaluatesTo<Vec<Value>>>, E: Into<EvaluatesTo<Value>>>(
        collection: C,
        element: E,
    ) -> Self {
        Self {
            collection: collection.into(),
            element: element.into(),
        }
    }
}

impl From<Contains> for ExpressionBox {
    fn from(expression: Contains) -> Self {
        Expression::Contains(expression).into()
    }
}

impl From<Contains> for EvaluatesTo<bool> {
    fn from(expression: Contains) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// `Contains` expression.
/// Returns `true` if `collection` contains all `elements`, `false` otherwise.
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "{}.contains_all({})",
    "collection.parenthesise(Operation::MethodCall)",
    "elements"
)]

pub struct ContainsAll {
    /// Expression, which should evaluate to `Value::Vec`.
    pub collection: EvaluatesTo<Vec<Value>>,
    /// Expression, which should evaluate to `Value::Vec`.
    pub elements: EvaluatesTo<Vec<Value>>,
}

impl ContainsAll {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.collection.len() + self.elements.len() + 1
    }

    /// Constructs `Contains` expression.
    pub fn new<C: Into<EvaluatesTo<Vec<Value>>>, E: Into<EvaluatesTo<Vec<Value>>>>(
        collection: C,
        elements: E,
    ) -> Self {
        Self {
            collection: collection.into(),
            elements: elements.into(),
        }
    }
}

impl From<ContainsAll> for ExpressionBox {
    fn from(expression: ContainsAll) -> Self {
        Expression::ContainsAll(expression).into()
    }
}

impl From<ContainsAll> for EvaluatesTo<bool> {
    fn from(expression: ContainsAll) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// `Contains` expression.
/// Returns `true` if `collection` contains any element out of the `elements`, `false` otherwise.
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "{}.contains_any({})",
    "collection.parenthesise(Operation::MethodCall)",
    "elements"
)]
pub struct ContainsAny {
    /// Expression, which should evaluate to `Value::Vec`.
    pub collection: EvaluatesTo<Vec<Value>>,
    /// Expression, which should evaluate to `Value::Vec`.
    pub elements: EvaluatesTo<Vec<Value>>,
}

impl ContainsAny {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.collection.len() + self.elements.len() + 1
    }

    /// Constructs `Contains` expression.
    pub fn new<C: Into<EvaluatesTo<Vec<Value>>>, E: Into<EvaluatesTo<Vec<Value>>>>(
        collection: C,
        elements: E,
    ) -> Self {
        Self {
            collection: collection.into(),
            elements: elements.into(),
        }
    }
}

impl From<ContainsAny> for ExpressionBox {
    fn from(expression: ContainsAny) -> Self {
        Expression::ContainsAny(expression).into()
    }
}

impl From<ContainsAny> for EvaluatesTo<bool> {
    fn from(expression: ContainsAny) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// Returns `true` if `left` operand is equal to the `right` operand.
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
    PartialOrd,
    Ord,
)]
#[display(
    fmt = "{} == {}",
    "left.parenthesise(Operation::Equal)",
    "right.parenthesise(Operation::Equal)"
)]
pub struct Equal {
    /// Left operand.
    pub left: EvaluatesTo<Value>,
    /// Right operand.
    pub right: EvaluatesTo<Value>,
}

impl Equal {
    /// Number of underneath expressions.
    pub fn len(&self) -> usize {
        self.left.len() + self.right.len() + 1
    }

    /// Constructs `Or` expression.
    pub fn new<L: Into<EvaluatesTo<Value>>, R: Into<EvaluatesTo<Value>>>(
        left: L,
        right: R,
    ) -> Self {
        Self {
            left: left.into(),
            right: right.into(),
        }
    }
}

impl From<Equal> for ExpressionBox {
    fn from(equal: Equal) -> Self {
        Expression::Equal(equal).into()
    }
}

impl From<Equal> for EvaluatesTo<bool> {
    fn from(expression: Equal) -> Self {
        EvaluatesTo::new_unchecked(expression.into())
    }
}

/// [`Where`] builder.
#[derive(Debug)]
pub struct WhereBuilder {
    /// Expression to be evaluated.
    expression: EvaluatesTo<Value>,
    /// Context values for the context binded to their `String` names.
    values: btree_map::BTreeMap<ValueName, EvaluatesTo<Value>>,
}

impl WhereBuilder {
    /// Sets the `expression` to be evaluated.
    #[must_use]
    pub fn evaluate<E: Into<EvaluatesTo<Value>>>(expression: E) -> Self {
        Self {
            expression: expression.into(),
            values: btree_map::BTreeMap::new(),
        }
    }

    /// Binds `expression` result to a `value_name`, by which it will be reachable from the main expression.
    #[must_use]
    pub fn with_value<E: Into<EvaluatesTo<Value>>>(
        mut self,
        value_name: ValueName,
        expression: E,
    ) -> Self {
        let _result = self.values.insert(value_name, expression.into());
        self
    }

    /// Returns a [`Where`] expression.
    #[inline]
    #[must_use]
    pub fn build(self) -> Where {
        Where::new(self.expression, self.values)
    }
}

/// Adds a local context of `values` for the `expression`.
/// It is similar to *Haskell's where syntax* although, evaluated eagerly.
#[derive(
    Debug, Clone, PartialEq, Eq, Decode, Encode, Deserialize, Serialize, IntoSchema, PartialOrd, Ord,
)]
pub struct Where {
    /// Expression to be evaluated.
    pub expression: EvaluatesTo<Value>,
    /// Context values for the context binded to their `String` names.
    pub values: btree_map::BTreeMap<ValueName, EvaluatesTo<Value>>,
}

impl core::fmt::Display for Where {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "`{} where: [", self.expression)?;

        let mut first = true;
        for (key, value) in &self.values {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            write!(f, "`{}` : `{}`", key, value)?;
        }

        write!(f, "]")
    }
}

impl Where {
    /// Number of underneath expressions.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.expression.len() + self.values.values().map(EvaluatesTo::len).sum::<usize>() + 1
    }

    /// Constructs `Or` expression.
    #[must_use]
    pub fn new<E: Into<EvaluatesTo<Value>>>(
        expression: E,
        values: btree_map::BTreeMap<ValueName, EvaluatesTo<Value>>,
    ) -> Self {
        Self {
            expression: expression.into(),
            values,
        }
    }
}

impl From<Where> for ExpressionBox {
    fn from(where_expression: Where) -> Self {
        Expression::Where(where_expression).into()
    }
}

impl QueryBox {
    /// Number of underneath expressions.
    pub const fn len(&self) -> usize {
        1
    }
}

impl From<QueryBox> for ExpressionBox {
    fn from(query: QueryBox) -> Self {
        Expression::Query(query).into()
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::{
        Add, And, Contains, ContainsAll, ContainsAny, Context, ContextValue, Divide, Equal,
        EvaluatesTo, Expression, ExpressionBox, Greater, If as IfExpression, IfBuilder, Less, Mod,
        Multiply, Not, Or, RaiseTo, Subtract, ValueName, Where, WhereBuilder,
    };
}
