#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, format, string::String, vec, vec::Vec};

use derive_where::derive_where;
use iroha_macro::serde_where;
use iroha_schema::IntoSchema;
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{
    account::{Account, AccountEntry},
    asset::{Asset, AssetEntry},
    nft::{Nft, NftEntry},
    prelude::{AccountProjection, AssetProjection, NftProjection},
    query::dsl::{BaseProjector, EvaluatePredicate, HasProjection, HasPrototype, PredicateMarker},
};

/// A compound predicate that is be used to combine multiple predicates using logical operators.
#[derive_where(Debug, Eq, PartialEq, Clone; T::Projection)]
#[serde_where(T::Projection)]
#[derive(Decode, Encode, Deserialize, Serialize, IntoSchema)]
pub enum CompoundPredicate<T: HasProjection<PredicateMarker>> {
    /// A predicate as-is
    Atom(T::Projection),
    /// A negation of a compound predicate.
    Not(Box<CompoundPredicate<T>>),
    /// A conjunction of multiple predicates.
    And(Vec<CompoundPredicate<T>>),
    /// A disjunction of multiple predicates.
    Or(Vec<CompoundPredicate<T>>),
}

impl<T: HasProjection<PredicateMarker>> CompoundPredicate<T> {
    /// A compound predicate that always evaluates to `true`.
    pub const PASS: Self = Self::And(Vec::new());
    /// A compound predicate that always evaluates to `false`.
    pub const FAIL: Self = Self::Or(Vec::new());

    // aliases for logical operations
    /// Negate the predicate.
    #[must_use]
    #[expect(clippy::should_implement_trait)] // we do implement the `Not` trait, this is just a shorthand to avoid requiring importing it
    pub fn not(self) -> Self {
        !self
    }

    /// Combine two predicates with an "and" operation.
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        self & other
    }

    /// Combine two predicates with an "or" operation.
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        self | other
    }
}

macro_rules! impl_applies {
    ($applies:ident $input_ty:ty) => {
        /// Evaluate the predicate on the given input.
        pub fn $applies(&self, input: $input_ty) -> bool {
            match self {
                CompoundPredicate::Atom(projection) => projection.$applies(input),
                CompoundPredicate::Not(expr) => !expr.$applies(input),
                CompoundPredicate::And(and_list) => {
                    and_list.iter().all(|expr| expr.$applies(input))
                }
                CompoundPredicate::Or(or_list) => or_list.iter().any(|expr| expr.$applies(input)),
            }
        }
    };
}

impl<T> CompoundPredicate<T>
where
    T: HasProjection<PredicateMarker>,
    T::Projection: EvaluatePredicate<T>,
{
    impl_applies!(applies & T);
}

// This impl and `impl *Projection<...>` below
// is a small workaround to support `*Entry` structs in filters without copying.
// Alternatively we can use `*Entry` classes directly in `type_descriptions!()`,
// but because of lifetimes it will be very complicated.
impl CompoundPredicate<Account> {
    impl_applies!(applies_to_entry AccountEntry);
}

impl CompoundPredicate<Asset> {
    impl_applies!(applies_to_entry AssetEntry);
}

impl CompoundPredicate<Nft> {
    impl_applies!(applies_to_entry NftEntry);
}

impl AccountProjection<PredicateMarker> {
    fn applies_to_entry(&self, input: AccountEntry) -> bool {
        use AccountProjection::*;
        match self {
            Atom(atom) => match *atom {},
            Id(field) => field.applies(input.id),
            Metadata(field) => field.applies(input.metadata),
        }
    }
}

impl AssetProjection<PredicateMarker> {
    fn applies_to_entry(&self, input: AssetEntry) -> bool {
        use AssetProjection::*;
        match self {
            Atom(atom) => match *atom {},
            Id(field) => field.applies(input.id),
            Value(field) => field.applies(input.value),
        }
    }
}

impl NftProjection<PredicateMarker> {
    fn applies_to_entry(&self, input: NftEntry) -> bool {
        use NftProjection::*;
        match self {
            Atom(atom) => match *atom {},
            Id(field) => field.applies(input.id),
            Metadata(field) => field.applies(input.content),
            AccountId(field) => field.applies(input.owned_by),
        }
    }
}

impl<T: HasProjection<PredicateMarker>> core::ops::Not for CompoundPredicate<T> {
    type Output = CompoundPredicate<T>;

    fn not(self) -> Self::Output {
        match self {
            // if the top-level predicate is a negation, we can just remove it
            CompoundPredicate::Not(expr) => *expr,
            this => CompoundPredicate::Not(Box::new(this)),
        }
    }
}

impl<T: HasProjection<PredicateMarker>> core::ops::BitAnd for CompoundPredicate<T> {
    type Output = CompoundPredicate<T>;

    fn bitand(self, other: Self) -> Self::Output {
        match (self, other) {
            // if any of the predicates is an and - flatten it
            (CompoundPredicate::And(mut and_list), other) => {
                and_list.push(other);
                CompoundPredicate::And(and_list)
            }
            (this, CompoundPredicate::And(mut and_list)) => {
                // push to front to preserve user-specified order (our predicates are short-circuiting)
                and_list.insert(0, this);
                CompoundPredicate::And(and_list)
            }
            (this, other) => CompoundPredicate::And(vec![this, other]),
        }
    }
}

impl<T: HasProjection<PredicateMarker>> core::ops::BitOr for CompoundPredicate<T> {
    type Output = CompoundPredicate<T>;

    fn bitor(self, other: Self) -> Self::Output {
        match (self, other) {
            // if any of the predicates is an or - flatten it
            (CompoundPredicate::Or(mut or_list), other) => {
                or_list.push(other);
                CompoundPredicate::Or(or_list)
            }
            (this, CompoundPredicate::Or(mut or_list)) => {
                // push to front to preserve user-specified order (our predicates are short-circuiting)
                or_list.insert(0, this);
                CompoundPredicate::Or(or_list)
            }
            (this, other) => CompoundPredicate::Or(vec![this, other]),
        }
    }
}

impl<T: HasProjection<PredicateMarker>> CompoundPredicate<T> {
    /// Build a new compound predicate using the provided closure.
    pub fn build<F>(f: F) -> Self
    where
        T: HasPrototype,
        F: FnOnce(
            <T as HasPrototype>::Prototype<PredicateMarker, BaseProjector<PredicateMarker, T>>,
        ) -> CompoundPredicate<T>,
        <T as HasPrototype>::Prototype<PredicateMarker, BaseProjector<PredicateMarker, T>>: Default,
    {
        f(Default::default())
    }
}
