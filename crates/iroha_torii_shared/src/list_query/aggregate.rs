//! Aggregate (`group_by` + metrics) mode for list queries.
use super::filter::{FieldPath, FilterExpr};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};

/// Aggregate function applied to each group.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AggregateFn {
    /// Number of rows in the group.
    Count,
    /// Sum of a numeric field.
    Sum,
    /// Minimum of a numeric field.
    Min,
    /// Maximum of a numeric field.
    Max,
    /// Average of a numeric field.
    Avg,
    /// Number of distinct values of a scalar field.
    DistinctCount,
}

impl AggregateFn {
    /// JSON spelling of the function.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Sum => "sum",
            Self::Min => "min",
            Self::Max => "max",
            Self::Avg => "avg",
            Self::DistinctCount => "distinct_count",
        }
    }
}

impl json::JsonSerialize for AggregateFn {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(self.as_str(), out);
    }

    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        self.as_str().json_serialize_to(out)
    }
}

impl json::JsonDeserialize for AggregateFn {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let raw = String::json_deserialize(parser)?;
        match raw.as_str() {
            "count" => Ok(Self::Count),
            "sum" => Ok(Self::Sum),
            "min" => Ok(Self::Min),
            "max" => Ok(Self::Max),
            "avg" => Ok(Self::Avg),
            "distinct_count" => Ok(Self::DistinctCount),
            other => Err(json::Error::Message(format!(
                "unknown aggregate function `{other}`; expected one of: count, sum, min, max, avg, distinct_count"
            ))),
        }
    }
}

/// One metric computed per group.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct AggregateMetric {
    /// Output column name, also usable in `having` and `sort`.
    pub alias: String,
    /// Aggregate function.
    #[norito(rename = "fn")]
    pub r#fn: AggregateFn,
    /// Field consumed by the function (absent for `count`).
    #[norito(default)]
    pub field: Option<FieldPath>,
}

/// Grouping and metrics evaluated after filtering and before pagination.
#[derive(Debug, Clone, PartialEq, Eq, Default, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct AggregateSpec {
    /// Grouping dimensions.
    #[norito(default)]
    pub group_by: Vec<FieldPath>,
    /// Metrics computed per group.
    #[norito(default)]
    pub metrics: Vec<AggregateMetric>,
    /// Filter applied to the aggregated rows.
    #[norito(default)]
    pub having: Option<FilterExpr>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_spec_json_roundtrip() {
        let spec = AggregateSpec {
            group_by: vec!["primary_alias_domain".into()],
            metrics: vec![
                AggregateMetric {
                    alias: "accounts".into(),
                    r#fn: AggregateFn::Count,
                    field: None,
                },
                AggregateMetric {
                    alias: "total".into(),
                    r#fn: AggregateFn::Sum,
                    field: Some("quantity".into()),
                },
            ],
            having: Some(FilterExpr::parse("accounts > 1").expect("having")),
        };
        let encoded = json::to_json(&spec).expect("serialize");
        assert_eq!(
            json::from_str::<AggregateSpec>(&encoded).expect("decode"),
            spec
        );
        let text_having: AggregateSpec =
            json::from_str(r#"{"metrics":[{"alias":"n","fn":"count"}],"having":"n >= 2"}"#)
                .expect("text having");
        assert_eq!(
            text_having.having,
            Some(FilterExpr::parse("n >= 2").unwrap())
        );
    }

    #[test]
    fn misspelled_members_are_rejected() {
        for body in [
            r#"{"groupby":["owned_by"],"metrics":[{"alias":"n","fn":"count"}]}"#,
            r#"{"metrics":[{"alias":"n","fn":"count","feild":"quantity"}]}"#,
        ] {
            let err = json::from_str::<AggregateSpec>(body).expect_err("unknown member");
            assert!(err.to_string().contains("unknown"), "{err}");
        }
    }

    #[test]
    fn unknown_function_names_the_choices() {
        let err = json::from_str::<AggregateFn>("\"median\"").expect_err("unknown");
        assert!(err.to_string().contains("distinct_count"), "{err}");
    }
}
