//! Columns, rotations, queries, selectors and polynomial expressions.
//!
//! These types mirror the halo2-axiom arithmetization one for one (single
//! advice phase, no challenges), so chips port mechanically and the oracle can
//! compare constraint systems node for node.
//!
//! An [`Expression`] stores each query as `(column, rotation)`. The constraint
//! system interns `(column, rotation)` pairs into per-kind query tables in the
//! order halo2 interns them, see `ConstraintSystem`.

use core::{
    cmp::Ordering,
    fmt,
    ops::{Add, Mul, Neg, Sub},
};

use iroha_pasta::PastaField;

/// A row offset relative to the current row: row `i` queried at rotation `r`
/// reads row `(i + r) mod n`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rotation(pub i32);

impl Rotation {
    /// The current row.
    #[must_use]
    pub const fn cur() -> Self {
        Self(0)
    }

    /// The next row.
    #[must_use]
    pub const fn next() -> Self {
        Self(1)
    }

    /// The previous row.
    #[must_use]
    pub const fn prev() -> Self {
        Self(-1)
    }
}

mod sealed {
    /// Seals [`super::ColumnType`] to the four column kinds of this module.
    pub trait Sealed {}
    impl Sealed for super::Advice {}
    impl Sealed for super::Fixed {}
    impl Sealed for super::Instance {}
    impl Sealed for super::Any {}
}

/// The kind of a column: [`Advice`], [`Fixed`], [`Instance`] or [`Any`].
pub trait ColumnType:
    sealed::Sealed + Copy + fmt::Debug + PartialEq + Eq + core::hash::Hash + Into<Any>
{
    /// The expression querying column `index` of this kind at `rotation`.
    fn query_expression<F>(self, index: usize, rotation: Rotation) -> Expression<F>;
}

/// A witness (advice) column. PIPA-v1 has a single advice phase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Advice;

/// A fixed (preprocessed) column, committed in the verifying key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fixed;

/// A public-input (instance) column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instance;

/// Any column kind.
///
/// The order is `Instance < Advice < Fixed`, as in halo2; layouts and copy
/// diagnostics depend on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Any {
    /// An advice column.
    Advice,
    /// A fixed column.
    Fixed,
    /// An instance column.
    Instance,
}

impl Any {
    /// Rank in the halo2 column order `Instance < Advice < Fixed`.
    const fn rank(self) -> u8 {
        match self {
            Self::Instance => 0,
            Self::Advice => 1,
            Self::Fixed => 2,
        }
    }
}

impl Ord for Any {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl PartialOrd for Any {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<Advice> for Any {
    fn from(_: Advice) -> Self {
        Self::Advice
    }
}

impl From<Fixed> for Any {
    fn from(_: Fixed) -> Self {
        Self::Fixed
    }
}

impl From<Instance> for Any {
    fn from(_: Instance) -> Self {
        Self::Instance
    }
}

impl ColumnType for Advice {
    fn query_expression<F>(self, index: usize, rotation: Rotation) -> Expression<F> {
        Expression::Advice(AdviceQuery {
            column_index: index,
            rotation,
        })
    }
}

impl ColumnType for Fixed {
    fn query_expression<F>(self, index: usize, rotation: Rotation) -> Expression<F> {
        Expression::Fixed(FixedQuery {
            column_index: index,
            rotation,
        })
    }
}

impl ColumnType for Instance {
    fn query_expression<F>(self, index: usize, rotation: Rotation) -> Expression<F> {
        Expression::Instance(InstanceQuery {
            column_index: index,
            rotation,
        })
    }
}

impl ColumnType for Any {
    fn query_expression<F>(self, index: usize, rotation: Rotation) -> Expression<F> {
        match self {
            Self::Advice => Advice.query_expression(index, rotation),
            Self::Fixed => Fixed.query_expression(index, rotation),
            Self::Instance => Instance.query_expression(index, rotation),
        }
    }
}

/// A column of kind `C` with its index among the columns of that kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Column<C: ColumnType> {
    index: usize,
    column_type: C,
}

impl<C: ColumnType> Column<C> {
    /// Creates a column handle. Only the constraint system allocates columns;
    /// tests and importers use this to name existing columns.
    #[must_use]
    pub const fn new(index: usize, column_type: C) -> Self {
        Self { index, column_type }
    }

    /// Index of this column among the columns of its kind.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Kind of this column.
    #[must_use]
    pub const fn column_type(&self) -> &C {
        &self.column_type
    }

    /// The expression querying this column at `rotation`.
    #[must_use]
    pub fn query_cell<F>(&self, rotation: Rotation) -> Expression<F> {
        self.column_type.query_expression(self.index, rotation)
    }

    /// The expression querying this column at the current row.
    #[must_use]
    pub fn cur<F>(&self) -> Expression<F> {
        self.query_cell(Rotation::cur())
    }

    /// The expression querying this column at the next row.
    #[must_use]
    pub fn next<F>(&self) -> Expression<F> {
        self.query_cell(Rotation::next())
    }

    /// The expression querying this column at the previous row.
    #[must_use]
    pub fn prev<F>(&self) -> Expression<F> {
        self.query_cell(Rotation::prev())
    }

    /// The expression querying this column at `rotation`.
    #[must_use]
    pub fn rot<F>(&self, rotation: i32) -> Expression<F> {
        self.query_cell(Rotation(rotation))
    }
}

impl<C: ColumnType> Ord for Column<C> {
    fn cmp(&self, other: &Self) -> Ordering {
        let left: Any = self.column_type.into();
        let right: Any = other.column_type.into();
        left.cmp(&right).then(self.index.cmp(&other.index))
    }
}

impl<C: ColumnType> PartialOrd for Column<C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<Column<Advice>> for Column<Any> {
    fn from(column: Column<Advice>) -> Self {
        Self::new(column.index, Any::Advice)
    }
}

impl From<Column<Fixed>> for Column<Any> {
    fn from(column: Column<Fixed>) -> Self {
        Self::new(column.index, Any::Fixed)
    }
}

impl From<Column<Instance>> for Column<Any> {
    fn from(column: Column<Instance>) -> Self {
        Self::new(column.index, Any::Instance)
    }
}

/// A [`Column<Any>`] was not of the requested kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WrongColumnKind {
    /// The requested kind.
    pub expected: Any,
    /// The kind of the column.
    pub found: Any,
}

impl fmt::Display for WrongColumnKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "expected a {:?} column, found {:?}",
            self.expected, self.found
        )
    }
}

impl std::error::Error for WrongColumnKind {}

impl TryFrom<Column<Any>> for Column<Advice> {
    type Error = WrongColumnKind;

    fn try_from(column: Column<Any>) -> Result<Self, Self::Error> {
        match column.column_type {
            Any::Advice => Ok(Self::new(column.index, Advice)),
            found => Err(WrongColumnKind {
                expected: Any::Advice,
                found,
            }),
        }
    }
}

impl TryFrom<Column<Any>> for Column<Fixed> {
    type Error = WrongColumnKind;

    fn try_from(column: Column<Any>) -> Result<Self, Self::Error> {
        match column.column_type {
            Any::Fixed => Ok(Self::new(column.index, Fixed)),
            found => Err(WrongColumnKind {
                expected: Any::Fixed,
                found,
            }),
        }
    }
}

impl TryFrom<Column<Any>> for Column<Instance> {
    type Error = WrongColumnKind;

    fn try_from(column: Column<Any>) -> Result<Self, Self::Error> {
        match column.column_type {
            Any::Instance => Ok(Self::new(column.index, Instance)),
            found => Err(WrongColumnKind {
                expected: Any::Instance,
                found,
            }),
        }
    }
}

/// A virtual selector.
///
/// A simple selector may appear in gates only, only as a factor, at most once
/// per polynomial; selector compression may then share fixed columns between
/// simple selectors. A complex selector may appear anywhere, including lookups,
/// and always gets its own fixed column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Selector {
    index: usize,
    simple: bool,
}

impl Selector {
    /// Creates a selector handle. Only the constraint system allocates
    /// selectors; tests and importers use this to name existing ones.
    #[must_use]
    pub const fn new(index: usize, simple: bool) -> Self {
        Self { index, simple }
    }

    /// Index of this selector.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Whether this selector is simple.
    #[must_use]
    pub const fn is_simple(&self) -> bool {
        self.simple
    }

    /// The expression of this selector.
    #[must_use]
    pub fn expr<F>(&self) -> Expression<F> {
        Expression::Selector(*self)
    }
}

/// A fixed column reserved for a lookup table.
///
/// Tables are loaded with `Layouter::assign_table`, which fills every usable
/// row after the table with the table's first row, so unloaded rows never add
/// values to the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableColumn {
    inner: Column<Fixed>,
}

impl TableColumn {
    /// Creates a table column over `inner`. Only the constraint system
    /// allocates table columns; importers use this to name existing ones.
    #[must_use]
    pub const fn new(inner: Column<Fixed>) -> Self {
        Self { inner }
    }

    /// The fixed column holding the table.
    #[must_use]
    pub const fn inner(self) -> Column<Fixed> {
        self.inner
    }
}

/// A query of a fixed column at a rotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixedQuery {
    /// The column index.
    pub column_index: usize,
    /// The rotation.
    pub rotation: Rotation,
}

/// A query of an advice column at a rotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AdviceQuery {
    /// The column index.
    pub column_index: usize,
    /// The rotation.
    pub rotation: Rotation,
}

/// A query of an instance column at a rotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstanceQuery {
    /// The column index.
    pub column_index: usize,
    /// The rotation.
    pub rotation: Rotation,
}

/// A polynomial over column queries, the halo2 `Expression` without
/// challenges.
///
/// The arithmetic operators build nodes exactly as halo2 does:
/// `a + b = Sum(a, b)`, `a - b = Sum(a, Negated(b))`, `a * b = Product(a, b)`,
/// `-a = Negated(a)` and `a * c = Scaled(a, c)` for a field constant `c`.
/// Unlike halo2 they never panic: the simple-selector rules are checked when a
/// gate or lookup is created, and a violation is reported by the constraint
/// system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expression<F> {
    /// A constant.
    Constant(F),
    /// A virtual selector, replaced by fixed columns before proving.
    Selector(Selector),
    /// A fixed column query.
    Fixed(FixedQuery),
    /// An advice column query.
    Advice(AdviceQuery),
    /// An instance column query.
    Instance(InstanceQuery),
    /// The negation of an expression.
    Negated(Box<Expression<F>>),
    /// The sum of two expressions.
    Sum(Box<Expression<F>>, Box<Expression<F>>),
    /// The product of two expressions.
    Product(Box<Expression<F>>, Box<Expression<F>>),
    /// An expression multiplied by a constant.
    Scaled(Box<Expression<F>>, F),
}

/// A fold over an [`Expression`], evaluated bottom-up with the left operand
/// before the right one.
pub trait ExpressionEvaluator<F> {
    /// The value an expression evaluates to.
    type Output;

    /// Evaluates a constant.
    fn constant(&mut self, value: &F) -> Self::Output;
    /// Evaluates a virtual selector.
    fn selector(&mut self, selector: Selector) -> Self::Output;
    /// Evaluates a fixed query.
    fn fixed(&mut self, query: FixedQuery) -> Self::Output;
    /// Evaluates an advice query.
    fn advice(&mut self, query: AdviceQuery) -> Self::Output;
    /// Evaluates an instance query.
    fn instance(&mut self, query: InstanceQuery) -> Self::Output;
    /// Negates a value.
    fn negated(&mut self, value: Self::Output) -> Self::Output;
    /// Adds two values.
    fn sum(&mut self, left: Self::Output, right: Self::Output) -> Self::Output;
    /// Multiplies two values.
    fn product(&mut self, left: Self::Output, right: Self::Output) -> Self::Output;
    /// Multiplies a value by a constant.
    fn scaled(&mut self, value: Self::Output, factor: &F) -> Self::Output;
}

/// A leaf query of an expression, by column kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QueryRef {
    /// The queried column.
    pub column: Column<Any>,
    /// The rotation.
    pub rotation: Rotation,
}

impl<F> Expression<F> {
    /// Evaluates this expression with `evaluator`, left operands first.
    pub fn evaluate<E: ExpressionEvaluator<F>>(&self, evaluator: &mut E) -> E::Output {
        match self {
            Self::Constant(value) => evaluator.constant(value),
            Self::Selector(selector) => evaluator.selector(*selector),
            Self::Fixed(query) => evaluator.fixed(*query),
            Self::Advice(query) => evaluator.advice(*query),
            Self::Instance(query) => evaluator.instance(*query),
            Self::Negated(inner) => {
                let value = inner.evaluate(evaluator);
                evaluator.negated(value)
            }
            Self::Sum(left, right) => {
                let left = left.evaluate(evaluator);
                let right = right.evaluate(evaluator);
                evaluator.sum(left, right)
            }
            Self::Product(left, right) => {
                let left = left.evaluate(evaluator);
                let right = right.evaluate(evaluator);
                evaluator.product(left, right)
            }
            Self::Scaled(inner, factor) => {
                let value = inner.evaluate(evaluator);
                evaluator.scaled(value, factor)
            }
        }
    }

    /// The degree: 0 for constants, 1 for queries and selectors, the sum over
    /// a product and the maximum over a sum.
    #[must_use]
    pub fn degree(&self) -> usize {
        match self {
            Self::Constant(_) => 0,
            Self::Selector(_) | Self::Fixed(_) | Self::Advice(_) | Self::Instance(_) => 1,
            Self::Negated(inner) | Self::Scaled(inner, _) => inner.degree(),
            Self::Sum(left, right) => left.degree().max(right.degree()),
            Self::Product(left, right) => left.degree().saturating_add(right.degree()),
        }
    }

    /// Number of nodes of this expression.
    #[must_use]
    pub fn node_count(&self) -> usize {
        match self {
            Self::Constant(_)
            | Self::Selector(_)
            | Self::Fixed(_)
            | Self::Advice(_)
            | Self::Instance(_) => 1,
            Self::Negated(inner) | Self::Scaled(inner, _) => inner.node_count().saturating_add(1),
            Self::Sum(left, right) | Self::Product(left, right) => left
                .node_count()
                .saturating_add(right.node_count())
                .saturating_add(1),
        }
    }

    /// Calls `visit` for every column query in evaluation order (left before
    /// right), repeats included. This is the order in which halo2 interns the
    /// queries of an expression.
    pub fn for_each_query(&self, visit: &mut impl FnMut(QueryRef)) {
        match self {
            Self::Constant(_) | Self::Selector(_) => {}
            Self::Fixed(query) => visit(QueryRef {
                column: Column::new(query.column_index, Any::Fixed),
                rotation: query.rotation,
            }),
            Self::Advice(query) => visit(QueryRef {
                column: Column::new(query.column_index, Any::Advice),
                rotation: query.rotation,
            }),
            Self::Instance(query) => visit(QueryRef {
                column: Column::new(query.column_index, Any::Instance),
                rotation: query.rotation,
            }),
            Self::Negated(inner) | Self::Scaled(inner, _) => inner.for_each_query(visit),
            Self::Sum(left, right) | Self::Product(left, right) => {
                left.for_each_query(visit);
                right.for_each_query(visit);
            }
        }
    }

    /// Calls `visit` for every selector in evaluation order, repeats included.
    pub fn for_each_selector(&self, visit: &mut impl FnMut(Selector)) {
        match self {
            Self::Selector(selector) => visit(*selector),
            Self::Constant(_) | Self::Fixed(_) | Self::Advice(_) | Self::Instance(_) => {}
            Self::Negated(inner) | Self::Scaled(inner, _) => inner.for_each_selector(visit),
            Self::Sum(left, right) | Self::Product(left, right) => {
                left.for_each_selector(visit);
                right.for_each_selector(visit);
            }
        }
    }

    /// Whether this expression contains a simple selector.
    #[must_use]
    pub fn contains_simple_selector(&self) -> bool {
        match self {
            Self::Selector(selector) => selector.is_simple(),
            Self::Constant(_) | Self::Fixed(_) | Self::Advice(_) | Self::Instance(_) => false,
            Self::Negated(inner) | Self::Scaled(inner, _) => inner.contains_simple_selector(),
            Self::Sum(left, right) | Self::Product(left, right) => {
                left.contains_simple_selector() || right.contains_simple_selector()
            }
        }
    }

    /// Checks the halo2 simple-selector rules and returns the simple selector
    /// of this gate polynomial, if any.
    ///
    /// halo2 rejects (by panicking in its operators) a sum or difference with
    /// an operand that contains a simple selector and a product of two
    /// operands that both contain one. Negation and scaling are allowed. So a
    /// valid polynomial holds at most one simple selector, as a factor.
    ///
    /// # Errors
    ///
    /// [`SimpleSelectorMisuse`] naming the offending construction.
    pub fn simple_selector(&self) -> Result<Option<Selector>, SimpleSelectorMisuse> {
        match self {
            Self::Selector(selector) => Ok(selector.is_simple().then_some(*selector)),
            Self::Constant(_) | Self::Fixed(_) | Self::Advice(_) | Self::Instance(_) => Ok(None),
            Self::Negated(inner) | Self::Scaled(inner, _) => inner.simple_selector(),
            Self::Sum(left, right) => {
                if left.contains_simple_selector() || right.contains_simple_selector() {
                    Err(SimpleSelectorMisuse::InSum)
                } else {
                    Ok(None)
                }
            }
            Self::Product(left, right) => match (left.simple_selector()?, right.simple_selector()?)
            {
                (Some(_), Some(_)) => Err(SimpleSelectorMisuse::TwoInProduct),
                (found, None) | (None, found) => Ok(found),
            },
        }
    }

    /// Squares this expression (`Product(self, self)`).
    #[must_use]
    pub fn square(self) -> Self
    where
        Self: Clone,
    {
        Self::Product(Box::new(self.clone()), Box::new(self))
    }
}

impl<F: Clone> Expression<F> {
    /// Rebuilds this expression node for node, replacing every selector `s`
    /// with `replacements[s.index()]`.
    ///
    /// # Errors
    ///
    /// [`SelectorReplacementError`] when a selector has no replacement.
    pub fn replace_selectors(
        &self,
        replacements: &[Expression<F>],
    ) -> Result<Self, SelectorReplacementError> {
        Ok(match self {
            Self::Selector(selector) => {
                replacements
                    .get(selector.index())
                    .cloned()
                    .ok_or(SelectorReplacementError {
                        selector: *selector,
                    })?
            }
            Self::Constant(value) => Self::Constant(value.clone()),
            Self::Fixed(query) => Self::Fixed(*query),
            Self::Advice(query) => Self::Advice(*query),
            Self::Instance(query) => Self::Instance(*query),
            Self::Negated(inner) => Self::Negated(Box::new(inner.replace_selectors(replacements)?)),
            Self::Sum(left, right) => Self::Sum(
                Box::new(left.replace_selectors(replacements)?),
                Box::new(right.replace_selectors(replacements)?),
            ),
            Self::Product(left, right) => Self::Product(
                Box::new(left.replace_selectors(replacements)?),
                Box::new(right.replace_selectors(replacements)?),
            ),
            Self::Scaled(inner, factor) => Self::Scaled(
                Box::new(inner.replace_selectors(replacements)?),
                factor.clone(),
            ),
        })
    }
}

/// A violation of the simple-selector rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimpleSelectorMisuse {
    /// A sum (or difference) has an operand containing a simple selector.
    InSum,
    /// Both operands of a product contain a simple selector.
    TwoInProduct,
    /// A lookup expression contains a simple selector.
    InLookup,
}

impl fmt::Display for SimpleSelectorMisuse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InSum => "a simple selector is used in an addition or subtraction",
            Self::TwoInProduct => "two expressions containing simple selectors are multiplied",
            Self::InLookup => "a simple selector is used in a lookup expression",
        })
    }
}

impl std::error::Error for SimpleSelectorMisuse {}

/// A selector had no replacement during selector substitution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectorReplacementError {
    /// The selector without a replacement.
    pub selector: Selector,
}

impl fmt::Display for SelectorReplacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "selector {} has no replacement", self.selector.index())
    }
}

impl std::error::Error for SelectorReplacementError {}

impl<F> Neg for Expression<F> {
    type Output = Self;

    fn neg(self) -> Self {
        Self::Negated(Box::new(self))
    }
}

impl<F> Add for Expression<F> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::Sum(Box::new(self), Box::new(rhs))
    }
}

impl<F> Sub for Expression<F> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::Sum(Box::new(self), Box::new(-rhs))
    }
}

impl<F> Mul for Expression<F> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Self::Product(Box::new(self), Box::new(rhs))
    }
}

impl<F: PastaField> Mul<F> for Expression<F> {
    type Output = Self;

    fn mul(self, rhs: F) -> Self {
        Self::Scaled(Box::new(self), rhs)
    }
}

#[cfg(test)]
mod tests {
    use ff::Field;
    use iroha_pasta::Fp;

    use super::*;

    fn advice(index: usize, rotation: i32) -> Expression<Fp> {
        Column::new(index, Advice).rot(rotation)
    }

    #[test]
    fn rotation_constants() {
        assert_eq!(Rotation::cur(), Rotation(0));
        assert_eq!(Rotation::next(), Rotation(1));
        assert_eq!(Rotation::prev(), Rotation(-1));
    }

    #[test]
    fn column_order_matches_halo2() {
        let instance = Column::<Any>::from(Column::new(5, Instance));
        let advice = Column::<Any>::from(Column::new(0, Advice));
        let fixed = Column::<Any>::from(Column::new(0, Fixed));
        assert!(instance < advice && advice < fixed);
        assert!(Column::new(1, Advice) < Column::new(2, Advice));
        assert_eq!(Any::Instance.cmp(&Any::Fixed), Ordering::Less);
        assert_eq!(
            Any::Fixed.partial_cmp(&Any::Advice),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn column_kind_conversions_round_trip_and_reject() {
        let any = Column::<Any>::from(Column::new(3, Fixed));
        assert_eq!(Column::<Fixed>::try_from(any), Ok(Column::new(3, Fixed)));
        assert_eq!(
            Column::<Advice>::try_from(any),
            Err(WrongColumnKind {
                expected: Any::Advice,
                found: Any::Fixed,
            })
        );
        let instance = Column::<Any>::from(Column::new(1, Instance));
        assert!(Column::<Instance>::try_from(instance).is_ok());
        assert!(Column::<Fixed>::try_from(instance).is_err());
        let adv = Column::<Any>::from(Column::new(2, Advice));
        assert_eq!(Column::<Advice>::try_from(adv), Ok(Column::new(2, Advice)));
        assert!(Column::<Instance>::try_from(adv).is_err());
        assert!(
            !WrongColumnKind {
                expected: Any::Advice,
                found: Any::Fixed
            }
            .to_string()
            .is_empty()
        );
    }

    #[test]
    fn column_query_helpers() {
        let column = Column::new(4, Advice);
        assert_eq!(
            column.next::<Fp>(),
            Expression::Advice(AdviceQuery {
                column_index: 4,
                rotation: Rotation(1)
            })
        );
        assert_eq!(column.prev::<Fp>(), advice(4, -1));
        assert_eq!(column.cur::<Fp>(), advice(4, 0));
        assert_eq!(*column.column_type(), Advice);
        assert_eq!(column.index(), 4);
        let fixed = Column::new(2, Fixed).query_cell::<Fp>(Rotation(3));
        assert_eq!(
            fixed,
            Expression::Fixed(FixedQuery {
                column_index: 2,
                rotation: Rotation(3)
            })
        );
        let any = Column::new(1, Any::Instance).cur::<Fp>();
        assert_eq!(
            any,
            Expression::Instance(InstanceQuery {
                column_index: 1,
                rotation: Rotation(0)
            })
        );
    }

    #[test]
    fn operators_build_halo2_nodes() {
        let a = advice(0, 0);
        let b = advice(1, 0);
        assert_eq!(
            a.clone() - b.clone(),
            Expression::Sum(
                Box::new(a.clone()),
                Box::new(Expression::Negated(Box::new(b.clone())))
            )
        );
        assert_eq!(
            a.clone() * Fp::from(3),
            Expression::Scaled(Box::new(a.clone()), Fp::from(3))
        );
        assert_eq!(
            a.clone().square(),
            Expression::Product(Box::new(a.clone()), Box::new(a))
        );
    }

    #[test]
    fn degree_and_node_count() {
        let s = Selector::new(0, true).expr::<Fp>();
        let a = advice(0, 0);
        let b = advice(1, 1);
        let expr = s * (a.clone() * b.clone() + Expression::Constant(Fp::ONE)) * Fp::from(2);
        assert_eq!(expr.degree(), 3);
        assert_eq!(expr.node_count(), 8);
        assert_eq!((-(a.clone() + b)).degree(), 1);
        assert_eq!(Expression::<Fp>::Constant(Fp::ONE).degree(), 0);
        assert_eq!(a.node_count(), 1);
    }

    #[test]
    fn query_visit_order_is_left_to_right() {
        let expr = (advice(2, 0) + Column::new(1, Fixed).cur())
            * (Column::new(0, Instance).next() - advice(2, -1));
        let mut seen = Vec::new();
        expr.for_each_query(&mut |query| seen.push((query.column, query.rotation.0)));
        assert_eq!(
            seen,
            vec![
                (Column::new(2, Any::Advice), 0),
                (Column::new(1, Any::Fixed), 0),
                (Column::new(0, Any::Instance), 1),
                (Column::new(2, Any::Advice), -1),
            ]
        );
        let mut selectors = Vec::new();
        (Selector::new(1, false).expr::<Fp>() * Selector::new(0, true).expr())
            .for_each_selector(&mut |s| selectors.push(s.index()));
        assert_eq!(selectors, vec![1, 0]);
    }

    #[test]
    fn simple_selector_rules_match_halo2() {
        let s = Selector::new(0, true).expr::<Fp>();
        let t = Selector::new(1, true).expr::<Fp>();
        let c = Selector::new(2, false).expr::<Fp>();
        let a = advice(0, 0);
        assert_eq!(
            (s.clone() * a.clone()).simple_selector(),
            Ok(Some(Selector::new(0, true)))
        );
        assert_eq!(
            (-(s.clone() * a.clone()) * Fp::from(5)).simple_selector(),
            Ok(Some(Selector::new(0, true)))
        );
        assert_eq!(
            (s.clone() + a.clone()).simple_selector(),
            Err(SimpleSelectorMisuse::InSum)
        );
        assert_eq!(
            (s.clone() * (t * a.clone())).simple_selector(),
            Err(SimpleSelectorMisuse::TwoInProduct)
        );
        assert_eq!((c.clone() + a.clone()).simple_selector(), Ok(None));
        assert!(!c.contains_simple_selector() && s.contains_simple_selector());
        assert!(!SimpleSelectorMisuse::InLookup.to_string().is_empty());
    }

    #[test]
    fn replace_selectors_is_node_for_node() {
        let s = Selector::new(1, true).expr::<Fp>();
        let a = advice(0, 0);
        let expr = s * (a.clone() - Expression::Constant(Fp::ONE));
        let q = Column::new(7, Fixed).cur::<Fp>();
        let replaced = expr
            .replace_selectors(&[Expression::Constant(Fp::ZERO), q.clone()])
            .expect("replacement exists");
        assert_eq!(
            replaced,
            q * (a - Expression::Constant(Fp::ONE)),
            "only the selector node changes"
        );
        let missing = Selector::new(3, false).expr::<Fp>().replace_selectors(&[]);
        assert_eq!(
            missing,
            Err(SelectorReplacementError {
                selector: Selector::new(3, false)
            })
        );
        assert!(!missing.unwrap_err().to_string().is_empty());
    }

    /// Counts nodes by kind to exercise every evaluator callback.
    struct Counter([usize; 9]);

    impl ExpressionEvaluator<Fp> for Counter {
        type Output = Fp;
        fn constant(&mut self, value: &Fp) -> Fp {
            self.0[0] += 1;
            *value
        }
        fn selector(&mut self, _: Selector) -> Fp {
            self.0[1] += 1;
            Fp::ONE
        }
        fn fixed(&mut self, _: FixedQuery) -> Fp {
            self.0[2] += 1;
            Fp::from(2)
        }
        fn advice(&mut self, _: AdviceQuery) -> Fp {
            self.0[3] += 1;
            Fp::from(3)
        }
        fn instance(&mut self, _: InstanceQuery) -> Fp {
            self.0[4] += 1;
            Fp::from(5)
        }
        fn negated(&mut self, value: Fp) -> Fp {
            self.0[5] += 1;
            -value
        }
        fn sum(&mut self, left: Fp, right: Fp) -> Fp {
            self.0[6] += 1;
            left + right
        }
        fn product(&mut self, left: Fp, right: Fp) -> Fp {
            self.0[7] += 1;
            left * right
        }
        fn scaled(&mut self, value: Fp, factor: &Fp) -> Fp {
            self.0[8] += 1;
            value * factor
        }
    }

    #[test]
    fn evaluate_visits_every_node_kind() {
        let expr = Selector::new(0, false).expr::<Fp>()
            * (Column::new(0, Fixed).cur::<Fp>() + advice(0, 0))
            * Fp::from(7)
            - Column::new(0, Instance).cur::<Fp>()
            + Expression::Constant(Fp::from(11));
        let mut counter = Counter([0; 9]);
        let value = expr.evaluate(&mut counter);
        // (1 * (2 + 3)) * 7 - 5 + 11 = 41
        assert_eq!(value, Fp::from(41));
        assert_eq!(counter.0, [1, 1, 1, 1, 1, 1, 3, 1, 1]);
    }

    #[test]
    fn table_column_and_selector_accessors() {
        let table = TableColumn::new(Column::new(9, Fixed));
        assert_eq!(table.inner(), Column::new(9, Fixed));
        let selector = Selector::new(4, false);
        assert_eq!((selector.index(), selector.is_simple()), (4, false));
    }
}
