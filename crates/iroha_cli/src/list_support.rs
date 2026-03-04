use eyre::{Result, WrapErr};
use iroha::data_model::query::{dsl::SelectorTuple, parameters::SortOrder};

use crate::parse_json;

/// Common pagination/sorting arguments shared by list commands.
#[derive(clap::Args, Debug, Clone)]
pub struct CommonArgs {
    /// Sort by metadata key
    #[arg(long)]
    pub sort_by_metadata_key: Option<iroha::data_model::name::Name>,
    /// Sort order (asc or desc)
    #[arg(long, value_enum)]
    pub order: Option<CliOrder>,
    /// Maximum number of items to return (server-side limit)
    #[arg(long)]
    pub limit: Option<u64>,
    /// Offset into the result set (server-side offset)
    #[arg(long, default_value_t = 0)]
    pub offset: u64,
    /// Batch fetch size for iterable queries
    #[arg(long)]
    pub fetch_size: Option<u64>,
    /// Experimental selector (JSON). Currently ignored; reserved for future server-side projection
    #[arg(long)]
    pub select: Option<String>,
}

pub fn parse_selector_tuple<Item>(sel: &str) -> Result<SelectorTuple<Item>>
where
    Item: 'static,
{
    #[cfg(feature = "ids_projection")]
    {
        let trimmed = sel.trim_matches('"');
        if trimmed.eq_ignore_ascii_case("ids") {
            return Ok(SelectorTuple::<Item>::ids_only());
        }
    }

    parse_json::<SelectorTuple<Item>>(sel).wrap_err("Failed to parse --select JSON")
}

/// CLI-friendly sort order wrapper.
#[derive(clap::ValueEnum, Clone, Debug, Copy, PartialEq, Eq)]
pub enum CliOrder {
    Asc,
    Desc,
}

impl From<CliOrder> for SortOrder {
    fn from(o: CliOrder) -> Self {
        match o {
            CliOrder::Asc => Self::Asc,
            CliOrder::Desc => Self::Desc,
        }
    }
}

/// Arguments for "list all" invocations.
#[derive(clap::Args, Debug, Clone)]
pub struct AllArgs {
    /// Display detailed entry information instead of just IDs (when supported)
    #[arg(short, long)]
    pub verbose: bool,
    #[command(flatten)]
    pub common: CommonArgs,
}

/// Arguments for "list with filter" invocations.
#[derive(Debug, Clone)]
pub struct FilterArgs<P> {
    predicate: P,
    common: CommonArgs,
}

impl<P> FilterArgs<P> {
    pub fn new(predicate: P, common: CommonArgs) -> Self {
        Self { predicate, common }
    }

    pub fn decompose(self) -> (P, CommonArgs) {
        (self.predicate, self.common)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::data_model::{prelude::*, query::dsl::CompoundPredicate};

    fn dummy_common_args() -> CommonArgs {
        CommonArgs {
            sort_by_metadata_key: None,
            order: None,
            limit: None,
            offset: 0,
            fetch_size: None,
            select: None,
        }
    }

    #[test]
    fn filter_args_split_preserves_fields() {
        let predicate = CompoundPredicate::<Domain>::PASS;
        let common = dummy_common_args();
        let args = FilterArgs::new(predicate.clone(), common.clone());
        let (actual_predicate, actual_common) = args.decompose();
        assert_eq!(format!("{predicate:?}"), format!("{actual_predicate:?}"));
        assert_eq!(common.order, actual_common.order);
        assert_eq!(common.limit, actual_common.limit);
        assert_eq!(common.offset, actual_common.offset);
        assert_eq!(common.fetch_size, actual_common.fetch_size);
        assert_eq!(
            common.sort_by_metadata_key,
            actual_common.sort_by_metadata_key
        );
        assert_eq!(common.select, actual_common.select);
    }

    #[test]
    fn parse_selector_tuple_rejects_invalid_json() {
        let err = super::parse_selector_tuple::<Domain>("not-json");
        assert!(err.is_err());
    }
}
