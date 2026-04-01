//! Nexus helpers (lane governance reports and public-lane snapshots).

use eyre::{Result, eyre};
use iroha::data_model::nexus::LaneId;
use norito::json::{Map, Value};
use std::{convert::TryFrom, fmt::Write};

use crate::{Run, RunContext};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Show governance manifest status per lane
    LaneReport(LaneReportArgs),
    /// Inspect public-lane validator lifecycle and stake state
    #[command(subcommand)]
    PublicLane(PublicLaneCommand),
}

#[derive(clap::Args, Debug, Default)]
pub struct LaneReportArgs {
    /// Print a compact table instead of JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
    /// Show only lanes that require a manifest but remain sealed
    #[arg(long, default_value_t = false)]
    pub only_missing: bool,
    /// Exit with non-zero status if any manifest is missing
    #[arg(long, default_value_t = false)]
    pub fail_on_sealed: bool,
}

#[derive(clap::Subcommand, Debug)]
pub enum PublicLaneCommand {
    /// List validators for a public lane with lifecycle hints
    Validators(PublicLaneValidatorsArgs),
    /// List bonded stake and pending unbonds for a public lane
    Stake(PublicLaneStakeArgs),
}

#[derive(clap::Args, Debug)]
pub struct PublicLaneValidatorsArgs {
    /// Public lane identifier (defaults to SINGLE lane)
    #[arg(long, value_name = "LANE", default_value_t = 0)]
    pub lane: u32,
    /// Render a compact table instead of raw JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
}

#[derive(clap::Args, Debug)]
pub struct PublicLaneStakeArgs {
    /// Public lane identifier (defaults to SINGLE lane)
    #[arg(long, value_name = "LANE", default_value_t = 0)]
    pub lane: u32,
    /// Filter for a specific validator account (optional)
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub validator: Option<String>,
    /// Render a compact table instead of raw JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::LaneReport(args) => lane_report(context, &args),
            Command::PublicLane(cmd) => match cmd {
                PublicLaneCommand::Validators(args) => public_lane_validators(context, &args),
                PublicLaneCommand::Stake(args) => public_lane_stake(context, &args),
            },
        }
    }
}

fn lane_report<C: RunContext>(context: &mut C, args: &LaneReportArgs) -> Result<()> {
    let client = context.client_from_config();
    let status = client.get_sumeragi_status_json()?;
    let lanes = status
        .get("lane_governance")
        .cloned()
        .unwrap_or(Value::Null);
    let sealed_count = status
        .get("lane_governance_sealed_total")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_else(|| count_sealed(&lanes));
    let sealed_aliases = status
        .get("lane_governance_sealed_aliases")
        .and_then(Value::as_array)
        .map_or_else(
            || collect_sealed_aliases(&lanes),
            |arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            },
        );
    let filtered_lanes = if args.only_missing {
        filter_lane_entries(lanes, true)
    } else {
        lanes
    };
    if args.summary {
        context.println(format_lane_summary(&filtered_lanes, args.only_missing))?;
    } else {
        let mut map = Map::new();
        map.insert(
            "sealed_total".into(),
            Value::from(u64::try_from(sealed_count).unwrap_or(u64::MAX)),
        );
        map.insert(
            "sealed_aliases".into(),
            Value::Array(sealed_aliases.iter().cloned().map(Value::from).collect()),
        );
        map.insert("lanes".into(), filtered_lanes);
        context.print_data(&Value::Object(map))?;
    }
    if args.fail_on_sealed && sealed_count > 0 {
        return Err(eyre!(
            "{sealed_count} lane(s) still sealed (governance manifest missing)"
        ));
    }
    Ok(())
}

fn public_lane_validators<C: RunContext>(
    context: &mut C,
    args: &PublicLaneValidatorsArgs,
) -> Result<()> {
    let client = context.client_from_config();
    let payload = client.get_public_lane_validators(LaneId::new(args.lane))?;
    if args.summary {
        context.println(format_validator_summary(&payload)?)?;
    } else {
        context.print_data(&payload)?;
    }
    Ok(())
}

fn public_lane_stake<C: RunContext>(context: &mut C, args: &PublicLaneStakeArgs) -> Result<()> {
    let client = context.client_from_config();
    let validator = args
        .validator
        .as_deref()
        .map(|literal| crate::resolve_account_id(context, literal))
        .transpose()?
        .map(|account| account.to_string());
    let payload = client.get_public_lane_stake(LaneId::new(args.lane), validator.as_deref())?;
    if args.summary {
        context.println(format_stake_summary(&payload)?)?;
    } else {
        context.print_data(&payload)?;
    }
    Ok(())
}

fn filter_lane_entries(value: Value, only_missing: bool) -> Value {
    if !only_missing {
        return value;
    }
    if let Value::Array(entries) = value {
        let filtered: Vec<_> = entries.into_iter().filter(lane_still_sealed).collect();
        Value::Array(filtered)
    } else {
        value
    }
}

fn lane_still_sealed(entry: &Value) -> bool {
    let Some(map) = entry.as_object() else {
        return false;
    };
    let required = map
        .get("manifest_required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let ready = map
        .get("manifest_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    required && !ready
}

fn count_sealed(value: &Value) -> usize {
    match value {
        Value::Array(entries) => entries
            .iter()
            .filter(|entry| lane_still_sealed(entry))
            .count(),
        _ => 0,
    }
}

fn collect_sealed_aliases(value: &Value) -> Vec<String> {
    match value {
        Value::Array(entries) => entries
            .iter()
            .filter(|entry| lane_still_sealed(entry))
            .filter_map(|entry| {
                entry
                    .as_object()
                    .and_then(|map| map.get("alias"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn format_lane_summary(value: &Value, only_missing: bool) -> String {
    let Some(array) = value.as_array() else {
        return "No lane governance entries returned.".to_string();
    };
    if array.is_empty() {
        return if only_missing {
            "All governance manifests are provisioned.".to_string()
        } else {
            "No lane governance entries returned.".to_string()
        };
    }

    let mut rows = Vec::with_capacity(array.len());
    for entry in array {
        if let Some(map) = entry.as_object() {
            rows.push(build_lane_row(map));
        }
    }
    if rows.is_empty() {
        return if only_missing {
            "All governance manifests are provisioned.".to_string()
        } else {
            "No lane governance entries returned.".to_string()
        };
    }

    let header = format!(
        "{:>4}  {:<16}  {:<16}  {:<7}  {:>6}  {:>10}  {}",
        "ID", "ALIAS", "MODULE", "STATUS", "QUORUM", "VALIDATORS", "DETAIL"
    );
    let mut formatted = String::with_capacity((rows.len() + 1) * header.len());
    formatted.push_str(&header);
    formatted.push('\n');
    for row in rows {
        formatted.push_str(&row);
        formatted.push('\n');
    }
    formatted.trim_end().to_string()
}

fn build_lane_row(entry: &Map) -> String {
    let lane_id = entry
        .get("lane_id")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let alias = entry
        .get("alias")
        .and_then(Value::as_str)
        .map_or_else(|| "-".to_string(), normalize_width);
    let module = entry
        .get("governance")
        .and_then(Value::as_str)
        .map_or_else(|| "-".to_string(), normalize_width);
    let manifest_required = entry
        .get("manifest_required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let manifest_ready = entry
        .get("manifest_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status = if manifest_required {
        if manifest_ready { "READY" } else { "SEALED" }
    } else {
        "N/A"
    };
    let quorum = entry
        .get("quorum")
        .and_then(Value::as_u64)
        .map_or_else(|| "-".to_string(), |q| q.to_string());
    let validator_count = entry
        .get("validator_ids")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let validators = validator_count.to_string();
    let detail = lane_detail(entry, manifest_required, manifest_ready);

    format!(
        "{lane_id:>4}  {alias:<16}  {module:<16}  {status:<7}  {quorum:>6}  {validators:>10}  {detail}"
    )
}

fn lane_detail(entry: &Map, manifest_required: bool, manifest_ready: bool) -> String {
    if !manifest_required {
        return "governance not configured".to_string();
    }
    if manifest_ready {
        if let Some(path) = entry
            .get("manifest_path")
            .and_then(Value::as_str)
            .filter(|p| !p.is_empty())
        {
            return path.to_string();
        }
        return "manifest loaded".to_string();
    }
    "manifest missing".to_string()
}

fn format_validator_summary(payload: &Value) -> Result<String> {
    let mut entries = lane_items(payload)?;
    if entries.is_empty() {
        return Ok("No validator entries returned.".to_string());
    }
    entries.sort_by(|lhs, rhs| {
        let l_val = lhs.get("validator").and_then(Value::as_str).unwrap_or("");
        let r_val = rhs.get("validator").and_then(Value::as_str).unwrap_or("");
        l_val.cmp(r_val)
    });

    let mut output = String::new();
    writeln!(
        &mut output,
        "{:<36}  {:<24}  {:<18}  {:<22}  {:<20}  {:<11}",
        "VALIDATOR", "PEER_ID", "STATUS", "ACTIVATION", "STAKE", "LAST_REWARD"
    )?;
    for entry in entries {
        let row = build_validator_row(entry);
        writeln!(
            &mut output,
            "{:<36}  {:<24}  {:<18}  {:<22}  {:<20}  {:<11}",
            truncate_field(&row.validator, 36),
            truncate_field(&row.peer_id, 24),
            truncate_field(&row.status, 18),
            truncate_field(&row.activation, 22),
            truncate_field(&row.stake, 20),
            truncate_field(&row.last_reward, 11),
        )?;
    }

    Ok(output.trim_end().to_string())
}

fn format_stake_summary(payload: &Value) -> Result<String> {
    let mut entries = lane_items(payload)?;
    if entries.is_empty() {
        return Ok("No stake entries returned.".to_string());
    }
    entries.sort_by(|lhs, rhs| {
        let l_val = lhs.get("validator").and_then(Value::as_str).unwrap_or("");
        let r_val = rhs.get("validator").and_then(Value::as_str).unwrap_or("");
        l_val.cmp(r_val).then_with(|| {
            let l_staker = lhs.get("staker").and_then(Value::as_str).unwrap_or("");
            let r_staker = rhs.get("staker").and_then(Value::as_str).unwrap_or("");
            l_staker.cmp(r_staker)
        })
    });

    let mut output = String::new();
    writeln!(
        &mut output,
        "{:<32}  {:<32}  {:>14}  {:<22}",
        "VALIDATOR", "STAKER", "BONDED", "PENDING_UNBONDS"
    )?;
    for entry in entries {
        let row = build_stake_row(entry);
        writeln!(
            &mut output,
            "{:<32}  {:<32}  {:>14}  {:<22}",
            truncate_field(&row.validator, 32),
            truncate_field(&row.staker, 32),
            row.bonded,
            truncate_field(&row.pending_unbonds, 22),
        )?;
    }

    Ok(output.trim_end().to_string())
}

fn lane_items(payload: &Value) -> Result<Vec<&Map>> {
    let Some(items) = payload.get("items").and_then(Value::as_array) else {
        return Err(eyre!(
            "public lane response missing `items` array; unexpected payload shape"
        ));
    };
    let mut mapped = Vec::with_capacity(items.len());
    for item in items {
        let Some(map) = item.as_object() else {
            return Err(eyre!("public lane item was not an object"));
        };
        mapped.push(map);
    }
    Ok(mapped)
}

struct ValidatorRow {
    validator: String,
    peer_id: String,
    status: String,
    activation: String,
    stake: String,
    last_reward: String,
}

fn build_validator_row(entry: &Map) -> ValidatorRow {
    let validator = entry
        .get("validator")
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
    let peer_id = entry
        .get("peer_id")
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
    let status = validator_status_label(entry.get("status"));
    let activation = activation_label(entry);
    let total_stake = entry
        .get("total_stake")
        .map_or_else(|| "-".to_string(), stringify_value);
    let self_stake = entry
        .get("self_stake")
        .map_or_else(|| "-".to_string(), stringify_value);
    let stake = format!("{total_stake} (self {self_stake})");
    let last_reward = entry
        .get("last_reward_epoch")
        .and_then(Value::as_u64)
        .map_or_else(|| "-".to_string(), |value| value.to_string());

    ValidatorRow {
        validator,
        peer_id,
        status,
        activation,
        stake,
        last_reward,
    }
}

fn validator_status_label(status: Option<&Value>) -> String {
    let Some(map) = status.and_then(Value::as_object) else {
        return "-".to_string();
    };
    let Some(kind) = map.get("type").and_then(Value::as_str) else {
        return "-".to_string();
    };
    match kind {
        "PendingActivation" => {
            let epoch = map
                .get("activates_at_epoch")
                .and_then(Value::as_u64)
                .map_or_else(String::new, |v| format!("epoch {v}"));
            if epoch.is_empty() {
                "Pending".to_string()
            } else {
                format!("Pending({epoch})")
            }
        }
        "Active" => "Active".to_string(),
        "Jailed" => map.get("reason").and_then(Value::as_str).map_or_else(
            || "Jailed".to_string(),
            |reason| format!("Jailed({})", truncate_field(reason, 14)),
        ),
        "Exiting" => map
            .get("releases_at_ms")
            .and_then(Value::as_u64)
            .map_or_else(|| "Exiting".to_string(), |ts| format!("Exiting({ts})")),
        "Exited" => "Exited".to_string(),
        "Slashed" => map.get("slash_id").and_then(Value::as_str).map_or_else(
            || "Slashed".to_string(),
            |id| format!("Slashed({})", truncate_field(id, 14)),
        ),
        other => other.to_string(),
    }
}

fn activation_label(entry: &Map) -> String {
    let epoch = entry
        .get("activation_epoch")
        .and_then(Value::as_u64)
        .map(|v| v.to_string());
    let height = entry
        .get("activation_height")
        .and_then(Value::as_u64)
        .map(|v| v.to_string());
    match (epoch, height) {
        (Some(e), Some(h)) => format!("epoch {e} @ {h}"),
        (Some(e), None) => format!("epoch {e}"),
        (None, Some(h)) => format!("height {h}"),
        _ => "-".to_string(),
    }
}

struct StakeRow {
    validator: String,
    staker: String,
    bonded: String,
    pending_unbonds: String,
}

fn build_stake_row(entry: &Map) -> StakeRow {
    let validator = entry
        .get("validator")
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
    let staker = entry
        .get("staker")
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
    let bonded = entry
        .get("bonded")
        .map_or_else(|| "-".to_string(), stringify_value);
    let pending_unbonds = pending_unbond_label(entry);

    StakeRow {
        validator,
        staker,
        bonded,
        pending_unbonds,
    }
}

fn pending_unbond_label(entry: &Map) -> String {
    let Some(pending) = entry.get("pending_unbonds").and_then(Value::as_array) else {
        return "-".to_string();
    };
    if pending.is_empty() {
        return "-".to_string();
    }
    let mut next_release: Option<u64> = None;
    for item in pending {
        if let Some(release_at) = item
            .as_object()
            .and_then(|map| map.get("release_at_ms"))
            .and_then(Value::as_u64)
        {
            next_release = Some(next_release.map_or(release_at, |current| current.min(release_at)));
        }
    }
    next_release.map_or_else(
        || format!("{} pending", pending.len()),
        |ts| format!("{} pending (next @ {ts})", pending.len()),
    )
}

fn stringify_value(value: &Value) -> String {
    if let Some(as_str) = value.as_str() {
        return as_str.to_owned();
    }
    norito::json::to_string(value).unwrap_or_else(|_| "-".to_string())
}

fn truncate_field(value: &str, max_len: usize) -> String {
    value.chars().take(max_len).collect()
}

fn normalize_width(value: &str) -> String {
    const MAX_LEN: usize = 16;
    value.chars().take(MAX_LEN).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_summary_formats_rows() {
        let entry = Map::from_iter([
            ("lane_id".into(), Value::from(2u64)),
            ("alias".into(), Value::from("governance")),
            ("governance".into(), Value::from("parliament")),
            ("manifest_required".into(), Value::from(true)),
            ("manifest_ready".into(), Value::from(false)),
            ("quorum".into(), Value::from(3u64)),
            (
                "validator_ids".into(),
                Value::Array(vec![
                    Value::from(
                        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    ),
                    Value::from(
                        "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                    ),
                ]),
            ),
            ("manifest_path".into(), Value::Null),
        ]);
        let value = Value::Array(vec![Value::Object(entry)]);
        let table = format_lane_summary(&value, false);
        assert!(table.contains("SEALED"));
        assert!(table.contains("manifest missing"));
    }

    #[test]
    fn lane_summary_handles_empty() {
        let value = Value::Array(Vec::new());
        let table = format_lane_summary(&value, false);
        assert_eq!(table, "No lane governance entries returned.");
        let filtered = format_lane_summary(&value, true);
        assert_eq!(filtered, "All governance manifests are provisioned.");
    }

    #[test]
    fn filter_removes_ready_lanes() {
        let sealed = Map::from_iter([
            ("lane_id".into(), Value::from(1u64)),
            ("alias".into(), Value::from("sealed")),
            ("governance".into(), Value::from("parliament")),
            ("manifest_required".into(), Value::from(true)),
            ("manifest_ready".into(), Value::from(false)),
        ]);
        let ready = Map::from_iter([
            ("lane_id".into(), Value::from(2u64)),
            ("alias".into(), Value::from("ready")),
            ("governance".into(), Value::from("parliament")),
            ("manifest_required".into(), Value::from(true)),
            ("manifest_ready".into(), Value::from(true)),
        ]);
        let filtered = filter_lane_entries(
            Value::Array(vec![Value::Object(sealed.clone()), Value::Object(ready)]),
            true,
        );
        match &filtered {
            Value::Array(entries) => {
                assert_eq!(entries.len(), 1);
                let map = entries[0].as_object().expect("object");
                assert_eq!(map.get("alias").and_then(Value::as_str), Some("sealed"));
            }
            _ => panic!("expected array"),
        }
        let summary = format_lane_summary(&filtered, true);
        assert!(summary.contains("sealed"));
        assert!(!summary.contains("ready"));
        assert_eq!(
            count_sealed(&Value::Array(vec![Value::Object(sealed.clone())])),
            1
        );
        assert_eq!(
            collect_sealed_aliases(&Value::Array(vec![Value::Object(sealed)])),
            vec![String::from("sealed")]
        );
    }

    #[test]
    fn collect_sealed_aliases_returns_empty_on_non_array() {
        assert!(collect_sealed_aliases(&Value::Null).is_empty());
    }

    #[test]
    fn validator_summary_formats_activation_and_status() {
        use iroha_crypto::{Algorithm, KeyPair};

        let validator = iroha::data_model::account::AccountId::new(
            KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
        .canonical_i105()
        .expect("canonical I105");
        let record = Map::from_iter([
            ("lane_id".into(), Value::from(0u64)),
            ("validator".into(), Value::from(validator.clone())),
            ("stake_account".into(), Value::from(validator.clone())),
            ("total_stake".into(), Value::from("1000")),
            ("self_stake".into(), Value::from("800")),
            (
                "status".into(),
                Value::Object(Map::from_iter([
                    ("type".into(), Value::from("PendingActivation")),
                    ("activates_at_epoch".into(), Value::from(2u64)),
                ])),
            ),
            ("activation_epoch".into(), Value::from(1u64)),
            ("activation_height".into(), Value::from(3601u64)),
            ("last_reward_epoch".into(), Value::Null),
        ]);
        let payload = Value::Object(Map::from_iter([
            ("lane_id".into(), Value::from(0u64)),
            ("total".into(), Value::from(1u64)),
            ("items".into(), Value::Array(vec![Value::Object(record)])),
        ]));

        let summary = format_validator_summary(&payload).expect("format summary");
        assert!(summary.contains(&truncate_field(&validator, 36)));
        assert!(summary.contains("Pending(epoch 2)"));
        assert!(summary.contains("epoch 1 @ 3601"));
        assert!(summary.contains("1000 (self 800)"));
    }

    #[test]
    fn stake_summary_marks_pending_unbonds() {
        use iroha_crypto::{Algorithm, KeyPair};

        let validator = iroha::data_model::account::AccountId::new(
            KeyPair::from_seed(vec![0x12; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
        .canonical_i105()
        .expect("canonical I105");
        let staker = iroha::data_model::account::AccountId::new(
            KeyPair::from_seed(vec![0x13; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
        .canonical_i105()
        .expect("canonical I105");
        let pending = Map::from_iter([
            ("request_id".into(), Value::from("deadbeef")),
            ("amount".into(), Value::from("250")),
            ("release_at_ms".into(), Value::from(10u64)),
        ]);
        let record = Map::from_iter([
            ("lane_id".into(), Value::from(0u64)),
            ("validator".into(), Value::from(validator.clone())),
            ("staker".into(), Value::from(staker.clone())),
            ("bonded".into(), Value::from("750")),
            (
                "pending_unbonds".into(),
                Value::Array(vec![Value::Object(pending)]),
            ),
        ]);
        let payload = Value::Object(Map::from_iter([
            ("lane_id".into(), Value::from(0u64)),
            ("total".into(), Value::from(1u64)),
            ("items".into(), Value::Array(vec![Value::Object(record)])),
        ]));

        let summary = format_stake_summary(&payload).expect("format summary");
        assert!(summary.contains(&truncate_field(&validator, 32)));
        assert!(summary.contains(&truncate_field(&staker, 32)));
        assert!(summary.contains("750"));
        assert!(summary.contains("pending (next @ 10)"));
    }

    #[test]
    fn normalize_width_preserves_short_values() {
        assert_eq!(normalize_width("governance"), "governance");
    }

    #[test]
    fn normalize_width_truncates_unicode_on_char_boundary() {
        let input = "いろはにほへとちりぬるをわかよたれそ";
        let expected = "いろはにほへとちりぬるをわかよた";
        assert_eq!(normalize_width(input), expected);
    }
}
