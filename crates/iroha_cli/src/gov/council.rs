//! Council governance query helper.

use super::shared::print_with_summary;
use crate::{Run, RunContext};
use eyre::Result;
use iroha::client::Client;

/// Fetch the latest explicitly persisted council roster.
#[derive(clap::Args, Debug)]
pub struct CouncilArgs {}

impl Run for CouncilArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.get_gov_council_json()?;
        let epoch = value
            .get("epoch")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let members = account_ids(&value, "members");
        let alternates = account_ids(&value, "alternates");
        let candidate_count = value
            .get("candidate_count")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let derived_by = value
            .get("derived_by")
            .and_then(norito::json::Value::as_str)
            .unwrap_or("unknown");
        let summary = Some(format!(
            "council: epoch={epoch} members_count={} alternates_count={} candidate_count={candidate_count} derived_by={derived_by} members=[{}] alternates=[{}]",
            members.len(),
            alternates.len(),
            members.join(", "),
            alternates.join(", "),
        ));
        print_with_summary(context, summary, &value)
    }
}

fn account_ids(value: &norito::json::Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(norito::json::Value::as_array)
        .map_or_else(Vec::new, |entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    entry
                        .get("account_id")
                        .and_then(norito::json::Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .collect()
        })
}
