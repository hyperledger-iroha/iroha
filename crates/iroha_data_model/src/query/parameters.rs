//! Defines parameters that can be sent along with a query.
//!
//! They are used together with [`QueryBox`](crate::query::QueryBox) to
//! configure execution of trait-object queries.

use std::{borrow::ToOwned, format, num::NonZeroU64, string::String, vec::Vec};

use derive_more::Constructor;
use getset::Getters;
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use iroha_version::{Decode, Encode};
use nonzero_ext::nonzero;

use crate::name::Name;

/// Default value for `fetch_size` parameter in queries.
pub const DEFAULT_FETCH_SIZE: NonZeroU64 = nonzero!(100_u64);
/// Max value for `fetch_size` parameter in queries.
pub const MAX_FETCH_SIZE: NonZeroU64 = nonzero!(10_000_u64);

pub use self::model::*;

/// Unique id of a query
pub type QueryId = String;

#[model]
mod model {
    use super::*;

    /// Forward-only (a.k.a non-scrollable) cursor
    #[derive(Debug, Clone, PartialEq, Eq, Getters, Encode, Decode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[getset(get = "pub")]
    pub struct ForwardCursor {
        /// Unique ID of query. When provided in a query the query will look up if there
        /// is was already a query with a matching ID and resume returning result batches
        pub query: QueryId,
        /// Pointer to the next element in the result set
        pub cursor: NonZeroU64,
        /// Optional gas budget presented by the client when requesting continuation.
        ///
        /// When present, this value is compared against server-configured minimums for
        /// stored cursors. Servers may also echo the minimum required budget back to
        /// clients to simplify subsequent continuation requests.
        #[norito(default)]
        pub gas_budget: Option<u64>,
    }

    /// Structure for pagination requests
    #[derive(
        derive_more::Debug,
        derive_more::Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        Default,
        Decode,
        Encode,
        IntoSchema,
        Constructor,
        Getters,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[getset(get_copy = "pub")]
    #[display(
        "{}--{}",
        offset,
        limit.map_or(".inf".to_owned(), |n| n.to_string())
    )]
    pub struct Pagination {
        /// limit of indexing
        pub limit: Option<NonZeroU64>,
        /// start of indexing
        pub offset: u64,
    }

    /// Struct for sorting requests
    #[derive(Debug, Clone, Default, PartialEq, Eq, Decode, Encode, IntoSchema, Constructor)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct Sorting {
        /// Sort query result using [`Name`] of the key in metadata.
        pub sort_by_metadata_key: Option<Name>,
        /// Optional order of sorting; defaults to ascending when not provided.
        #[norito(default)]
        pub order: Option<SortOrder>,
    }

    /// Sorting order. Defaults to `Asc`.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Decode, Encode, IntoSchema)]
    pub enum SortOrder {
        #[default]
        Asc,
        Desc,
    }

    #[cfg(feature = "json")]
    impl norito::json::JsonSerialize for SortOrder {
        fn json_serialize(&self, out: &mut String) {
            let label = match self {
                SortOrder::Asc => "Asc",
                SortOrder::Desc => "Desc",
            };
            norito::json::write_json_string(label, out);
        }
    }

    #[cfg(feature = "json")]
    impl norito::json::JsonDeserialize for SortOrder {
        fn json_deserialize(
            parser: &mut norito::json::Parser<'_>,
        ) -> Result<Self, norito::json::Error> {
            let value = parser.parse_string()?;
            match value.as_str() {
                "Asc" => Ok(SortOrder::Asc),
                "Desc" => Ok(SortOrder::Desc),
                other => Err(norito::json::Error::unknown_field(other.to_owned())),
            }
        }
    }

    /// Structure for query fetch size parameter encoding/decoding
    #[derive(
        Debug, Default, Clone, Copy, PartialEq, Eq, Constructor, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct FetchSize {
        /// Inner value of a fetch size.
        ///
        /// If not specified then [`DEFAULT_FETCH_SIZE`] is used.
        pub fetch_size: Option<NonZeroU64>,
    }

    /// Parameters that can modify iterable query execution.
    #[derive(Debug, Clone, PartialEq, Eq, Default, Constructor, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct QueryParams {
        pub pagination: Pagination,
        pub sorting: Sorting,
        pub fetch_size: FetchSize,
    }
}

impl Pagination {
    /// Return the pagination offset.
    pub fn offset_value(&self) -> u64 {
        self.offset
    }

    /// Return the pagination limit.
    pub fn limit_value(&self) -> Option<NonZeroU64> {
        self.limit
    }
}

impl QueryParams {
    /// Return the pagination configuration associated with this query.
    #[must_use]
    pub fn pagination(&self) -> &Pagination {
        &self.pagination
    }

    /// Return the sorting configuration associated with this query.
    #[must_use]
    pub fn sorting(&self) -> &Sorting {
        &self.sorting
    }

    /// Return the fetch-size configuration associated with this query.
    #[must_use]
    pub fn fetch_size(&self) -> &FetchSize {
        &self.fetch_size
    }
}

impl Sorting {
    /// Creates a sorting by [`Name`] of the key.
    pub fn by_metadata_key(key: Name) -> Self {
        Self {
            sort_by_metadata_key: Some(key),
            order: None,
        }
    }

    /// Return the metadata key used for sorting, when present.
    #[must_use]
    pub fn sort_by_metadata_key(&self) -> Option<&Name> {
        self.sort_by_metadata_key.as_ref()
    }

    /// Return the configured sort order.
    #[must_use]
    pub fn order(&self) -> Option<SortOrder> {
        self.order
    }
}

impl FetchSize {
    /// Return the configured fetch-size hint.
    #[must_use]
    pub fn value(&self) -> Option<NonZeroU64> {
        self.fetch_size
    }
}

pub mod prelude {
    //! Prelude: re-export most commonly used traits, structs and macros from this module.
    pub use super::{FetchSize, Pagination, Sorting};
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use std::str::FromStr;

    use norito::json;

    use super::*;

    #[test]
    fn sorting_defaults_order() {
        let mut value = json::to_value(&Sorting::by_metadata_key(Name::from_str("key").unwrap()))
            .expect("serialize sorting");

        if let json::Value::Object(ref mut map) = value {
            map.remove("order");
        }

        let parsed: Sorting = json::from_value(value).expect("deserialize sorting");
        assert!(parsed.order.is_none());
    }
}
