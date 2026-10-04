//! Fluent construction of filters and sort keys.
//!
//! ```
//! use iroha_torii_shared::list_query::field;
//!
//! let filter = field("owned_by").eq("alice")
//!     & field("quantity").gte(10)
//!     & !field("metadata.frozen").exists();
//! assert_eq!(
//!     filter.to_string(),
//!     r#"owned_by = "alice" and quantity >= 10 and not exists(metadata.frozen)"#
//! );
//! ```
use super::{
    filter::{FieldPath, FilterExpr},
    sort::SortKey,
};
use iroha_primitives::numeric::Numeric;
use norito::json::Value;
use std::ops::{BitAnd, BitOr, Not};

/// Start a predicate or sort key on a field path such as `metadata.tier`.
pub fn field(path: impl Into<FieldPath>) -> Field {
    Field(path.into())
}

/// A field awaiting an operator; see [`field`]. Reusable: every method borrows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field(FieldPath);

impl Field {
    /// `field = value`
    pub fn eq(&self, value: impl IntoLiteral) -> FilterExpr {
        FilterExpr::Eq(self.0.clone(), value.into_literal())
    }

    /// `field != value` (also matches rows where the field is absent).
    pub fn ne(&self, value: impl IntoLiteral) -> FilterExpr {
        FilterExpr::Ne(self.0.clone(), value.into_literal())
    }

    /// `field < value`
    pub fn lt(&self, value: impl IntoLiteral) -> FilterExpr {
        FilterExpr::Lt(self.0.clone(), value.into_literal())
    }

    /// `field <= value`
    pub fn lte(&self, value: impl IntoLiteral) -> FilterExpr {
        FilterExpr::Lte(self.0.clone(), value.into_literal())
    }

    /// `field > value`
    pub fn gt(&self, value: impl IntoLiteral) -> FilterExpr {
        FilterExpr::Gt(self.0.clone(), value.into_literal())
    }

    /// `field >= value`
    pub fn gte(&self, value: impl IntoLiteral) -> FilterExpr {
        FilterExpr::Gte(self.0.clone(), value.into_literal())
    }

    /// `field in [values...]`
    pub fn is_in<I>(&self, values: I) -> FilterExpr
    where
        I: IntoIterator,
        I::Item: IntoLiteral,
    {
        FilterExpr::In(
            self.0.clone(),
            values.into_iter().map(IntoLiteral::into_literal).collect(),
        )
    }

    /// `field not in [values...]`
    pub fn not_in<I>(&self, values: I) -> FilterExpr
    where
        I: IntoIterator,
        I::Item: IntoLiteral,
    {
        FilterExpr::Nin(
            self.0.clone(),
            values.into_iter().map(IntoLiteral::into_literal).collect(),
        )
    }

    /// `exists(field)`
    pub fn exists(&self) -> FilterExpr {
        FilterExpr::Exists(self.0.clone())
    }

    /// `field is null` (absent or null).
    pub fn is_null(&self) -> FilterExpr {
        FilterExpr::IsNull(self.0.clone())
    }

    /// `field is not null`
    pub fn is_not_null(&self) -> FilterExpr {
        FilterExpr::Not(Box::new(FilterExpr::IsNull(self.0.clone())))
    }

    /// Ascending sort key on this field.
    pub fn asc(&self) -> SortKey {
        SortKey::asc(self.0.clone())
    }

    /// Descending sort key on this field.
    pub fn desc(&self) -> SortKey {
        SortKey::desc(self.0.clone())
    }
}

/// Values usable as filter literals.
///
/// Integers that fit `u64`/`i64` become JSON numbers; decimals and wider
/// integers become exact decimal strings, matching the text grammar.
pub trait IntoLiteral {
    /// Convert into the JSON literal stored in the filter tree.
    fn into_literal(self) -> Value;
}

macro_rules! unsigned_literal {
    ($($ty:ty),*) => {$(
        impl IntoLiteral for $ty {
            fn into_literal(self) -> Value {
                Value::from(u64::from(self))
            }
        }
    )*};
}
unsigned_literal!(u8, u16, u32, u64);

macro_rules! signed_literal {
    ($($ty:ty),*) => {$(
        impl IntoLiteral for $ty {
            fn into_literal(self) -> Value {
                let wide = i64::from(self);
                u64::try_from(wide).map_or_else(|_| Value::from(wide), Value::from)
            }
        }
    )*};
}
signed_literal!(i8, i16, i32, i64);

impl IntoLiteral for usize {
    fn into_literal(self) -> Value {
        u64::try_from(self).map_or_else(|_| Value::from(self.to_string()), Value::from)
    }
}

impl IntoLiteral for u128 {
    fn into_literal(self) -> Value {
        u64::try_from(self).map_or_else(|_| Value::from(self.to_string()), Value::from)
    }
}

impl IntoLiteral for i128 {
    fn into_literal(self) -> Value {
        u64::try_from(self).map_or_else(
            |_| i64::try_from(self).map_or_else(|_| Value::from(self.to_string()), Value::from),
            Value::from,
        )
    }
}

impl IntoLiteral for bool {
    fn into_literal(self) -> Value {
        Value::Bool(self)
    }
}

impl IntoLiteral for &str {
    fn into_literal(self) -> Value {
        Value::from(self)
    }
}

impl IntoLiteral for String {
    fn into_literal(self) -> Value {
        Value::String(self)
    }
}

impl IntoLiteral for &String {
    fn into_literal(self) -> Value {
        Value::String(self.clone())
    }
}

impl IntoLiteral for Value {
    fn into_literal(self) -> Value {
        self
    }
}

impl IntoLiteral for Numeric {
    fn into_literal(self) -> Value {
        super::text::number_value(&self.to_string())
    }
}

impl IntoLiteral for &Numeric {
    fn into_literal(self) -> Value {
        super::text::number_value(&self.to_string())
    }
}

impl FilterExpr {
    /// `self and other`, flattening chains of `and`.
    #[must_use]
    pub fn and(self, other: FilterExpr) -> FilterExpr {
        match (self, other) {
            (FilterExpr::And(mut left), FilterExpr::And(right)) => {
                left.extend(right);
                FilterExpr::And(left)
            }
            (FilterExpr::And(mut left), right) => {
                left.push(right);
                FilterExpr::And(left)
            }
            (left, FilterExpr::And(mut right)) => {
                right.insert(0, left);
                FilterExpr::And(right)
            }
            (left, right) => FilterExpr::And(vec![left, right]),
        }
    }

    /// `self or other`, flattening chains of `or`.
    #[must_use]
    pub fn or(self, other: FilterExpr) -> FilterExpr {
        match (self, other) {
            (FilterExpr::Or(mut left), FilterExpr::Or(right)) => {
                left.extend(right);
                FilterExpr::Or(left)
            }
            (FilterExpr::Or(mut left), right) => {
                left.push(right);
                FilterExpr::Or(left)
            }
            (left, FilterExpr::Or(mut right)) => {
                right.insert(0, left);
                FilterExpr::Or(right)
            }
            (left, right) => FilterExpr::Or(vec![left, right]),
        }
    }

    /// `not self`
    #[must_use]
    pub fn negate(self) -> FilterExpr {
        FilterExpr::Not(Box::new(self))
    }

    /// Conjunction of all filters, or `None` when the iterator is empty.
    pub fn all(filters: impl IntoIterator<Item = FilterExpr>) -> Option<FilterExpr> {
        filters.into_iter().reduce(FilterExpr::and)
    }

    /// Disjunction of all filters, or `None` when the iterator is empty.
    pub fn any(filters: impl IntoIterator<Item = FilterExpr>) -> Option<FilterExpr> {
        filters.into_iter().reduce(FilterExpr::or)
    }
}

impl BitAnd for FilterExpr {
    type Output = FilterExpr;

    fn bitand(self, rhs: FilterExpr) -> FilterExpr {
        self.and(rhs)
    }
}

impl BitOr for FilterExpr {
    type Output = FilterExpr;

    fn bitor(self, rhs: FilterExpr) -> FilterExpr {
        self.or(rhs)
    }
}

impl Not for FilterExpr {
    type Output = FilterExpr;

    fn not(self) -> FilterExpr {
        self.negate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_matches_the_parser() {
        let built = field("owned_by").eq("alice")
            & field("quantity").gte(Numeric::new(105, 1))
            & (field("status").is_in(["A", "B"]) | field("tier").lt(-1))
            & !field("metadata.frozen").exists()
            & field("note").is_not_null();
        let parsed = FilterExpr::parse(
            r#"owned_by = "alice" and quantity >= 10.5 and (status in ["A", "B"] or tier < -1)
               and not exists(metadata.frozen) and note is not null"#,
        )
        .expect("parse");
        assert_eq!(built, parsed);
        assert!(built.validate().is_ok());
    }

    #[test]
    fn literals_follow_the_text_rules() {
        assert_eq!(7i32.into_literal(), Value::from(7u64));
        assert_eq!((-7i32).into_literal(), Value::from(-7i64));
        assert_eq!(u128::MAX.into_literal(), Value::from(u128::MAX.to_string()));
        assert_eq!(Numeric::new(25, 0).into_literal(), Value::from(25u64));
    }

    #[test]
    fn all_and_any_fold() {
        assert_eq!(FilterExpr::all(Vec::new()), None);
        let any = FilterExpr::any([field("a").eq(1), field("b").eq(2), field("c").eq(3)])
            .expect("non-empty");
        assert!(matches!(any, FilterExpr::Or(ref list) if list.len() == 3));
    }

    #[test]
    fn fields_are_reusable() {
        let tier = field("tier");
        assert_eq!(
            (tier.gt(1) & tier.lt(5)).to_string(),
            "tier > 1 and tier < 5"
        );
    }

    #[test]
    fn sort_helpers() {
        assert_eq!(field("id").desc().to_string(), "-id");
        assert_eq!(field("id").asc().to_string(), "id");
    }
}
