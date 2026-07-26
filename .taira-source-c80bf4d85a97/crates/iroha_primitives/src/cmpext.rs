//! Utilities for working with ordered collections like [`std::collections::BTreeMap`]
//! for the map case or [`std::collections::BTreeSet`] when only keys are stored.
//!
//! The [`MinMaxExt`] type injects sentinel `Min`/`Max` variants so
//! prefix range queries can be expressed when using the `range` APIs.

use core::cmp::Ordering;

/// Adds two sentinel values to any comparable type:
/// - [`MinMaxExt::Min`] is smaller than every real value
/// - [`MinMaxExt::Max`] is greater than every real value
///
/// This enables prefix queries over composite keys in a `BTreeMap` or
/// `BTreeSet`.
///
/// Suppose a compound key of three parts: `K = (A, B, C)`. Keys are
/// ordered first by `A`, then `B`, then `C`. To fetch all records with a
/// specific `A`, range bounds can be expressed as
/// `(A, Min, Min)..(A, Max, Max)`, ensuring the binary search covers the
/// entire prefix.
#[derive(Debug, Clone, Copy)]
pub enum MinMaxExt<T> {
    /// Sentinel smaller than any real value
    Min,
    /// Sentinel greater than any real value
    Max,
    /// Actual data value
    Value(T),
}

impl<T: PartialEq> PartialEq for MinMaxExt<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Value(lhs), Self::Value(rhs)) => lhs.eq(rhs),
            (Self::Min, Self::Min) | (Self::Max, Self::Max) => true,
            _ => false,
        }
    }
}

impl<T: Eq> Eq for MinMaxExt<T> {}

impl<T: PartialOrd> PartialOrd for MinMaxExt<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Value(lhs), Self::Value(rhs)) => lhs.partial_cmp(rhs),
            (lhs, rhs) if lhs == rhs => Some(Ordering::Equal),
            (Self::Min, _) | (_, Self::Max) => Some(Ordering::Less),
            (Self::Max, _) | (_, Self::Min) => Some(Ordering::Greater),
        }
    }
}

impl<T: Ord> Ord for MinMaxExt<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Value(lhs), Self::Value(rhs)) => lhs.cmp(rhs),
            (lhs, rhs) if lhs == rhs => Ordering::Equal,
            (Self::Min, _) | (_, Self::Max) => Ordering::Less,
            (Self::Max, _) | (_, Self::Min) => Ordering::Greater,
        }
    }
}

impl<T> From<T> for MinMaxExt<T> {
    fn from(value: T) -> Self {
        MinMaxExt::Value(value)
    }
}

/// Helper macro to enable casting map keys to trait objects and derive
/// the required comparison traits.
///
/// This bypasses the limitation of [`core::borrow::Borrow`], which doesn't allow
/// constructing a value on the fly and returning a reference to it. The
/// macro is primarily used when a lookup needs only a subset of the key
/// fields.
#[macro_export]
macro_rules! impl_as_dyn_key {
    (target: $ty:ty, key: $key:ty, trait: $trait:ident) => {
        /// Trait to key from type
        pub trait $trait {
            /// Extract key
            fn as_key(&self) -> $key;
        }

        impl $trait for $key {
            fn as_key(&self) -> $key {
                *self
            }
        }

        impl PartialEq for dyn $trait + '_ {
            fn eq(&self, other: &Self) -> bool {
                self.as_key() == other.as_key()
            }
        }

        impl Eq for dyn $trait + '_ {}

        impl PartialOrd for dyn $trait + '_ {
            fn partial_cmp(&self, other: &Self) -> Option<::core::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for dyn $trait + '_ {
            fn cmp(&self, other: &Self) -> ::core::cmp::Ordering {
                self.as_key().cmp(&other.as_key())
            }
        }

        impl<'lt> ::core::borrow::Borrow<dyn $trait + 'lt> for $ty {
            fn borrow(&self) -> &(dyn $trait + 'lt) {
                self
            }
        }
    };
}

/// Tests validate the ordering semantics for the sentinel values.
#[cfg(test)]
mod tests {
    use super::*;

    const ORDER_SAMPLES: &[u64] = &[0, 1, 2, 42, u64::MAX - 1, u64::MAX];

    #[test]
    fn values_larger_than_min_compare_above_sentinel() {
        for &value in &ORDER_SAMPLES[1..] {
            assert!(MinMaxExt::Min < value.into(), "value={value}");
        }
    }

    #[test]
    fn values_smaller_than_max_compare_below_sentinel() {
        for &value in &ORDER_SAMPLES[..ORDER_SAMPLES.len() - 1] {
            assert!(MinMaxExt::Max > value.into(), "value={value}");
        }
    }

    #[test]
    fn equal_values_remain_equal_when_wrapped() {
        for &value in ORDER_SAMPLES {
            assert_eq!(MinMaxExt::from(value), MinMaxExt::from(value));
        }
    }
}
