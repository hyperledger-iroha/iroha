//! Nexus helpers (lane governance reports and public-lane snapshots).

use eyre::{Result, eyre};
use iroha::data_model::{
    block::consensus::committed_lane_block_status_counts_as_progress,
    block::consensus_v2::SumeragiV2StatusResponse, nexus::LaneId,
};
use norito::json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryFrom,
    fmt::Write,
};

use crate::{Run, RunContext};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Show canonical Sumeragi v2 certification/application evidence per lane
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
    /// Show only lanes with incomplete certification or application evidence
    #[arg(
        long = "only-incomplete",
        alias = "only-missing",
        default_value_t = false
    )]
    pub only_incomplete: bool,
    /// Exit with non-zero status if any lane has incomplete evidence
    #[arg(
        long = "fail-on-incomplete",
        alias = "fail-on-sealed",
        default_value_t = false
    )]
    pub fail_on_incomplete: bool,
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
    let status = client.get_sumeragi_v2_status()?;
    let lanes = canonical_lane_entries(&status);
    let incomplete_count = count_incomplete(&lanes);
    let incomplete_lane_ids = collect_incomplete_lane_ids(&lanes);
    let filtered_lanes = if args.only_incomplete {
        filter_lane_entries(lanes, true)
    } else {
        lanes
    };
    if args.summary {
        context.println(format_lane_summary(&filtered_lanes, args.only_incomplete))?;
    } else {
        let mut map = Map::new();
        map.insert(
            "incomplete_total".into(),
            Value::from(u64::try_from(incomplete_count).unwrap_or(u64::MAX)),
        );
        map.insert(
            "incomplete_lane_ids".into(),
            Value::Array(incomplete_lane_ids.into_iter().map(Value::from).collect()),
        );
        map.insert("lanes".into(), filtered_lanes);
        context.print_data(&Value::Object(map))?;
    }
    if args.fail_on_incomplete && incomplete_count > 0 {
        return Err(eyre!(
            "{incomplete_count} lane(s) have incomplete canonical certification/application evidence"
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

#[derive(Debug, Default)]
struct CanonicalLaneEvidence {
    lane_id: u32,
    dataspaces: BTreeSet<u64>,
    incarnations: BTreeSet<String>,
    settlement_commitments: u64,
    relay_envelopes: u64,
    payload_ownerships: u64,
    committed_blocks: u64,
    active_sessions: u64,
    incomplete_sessions: u64,
    blocked_committed_blocks: u64,
    latest_global_height: u64,
    latest_lane_height: u64,
}

fn lane_evidence(
    lanes: &mut BTreeMap<u32, CanonicalLaneEvidence>,
    lane_id: LaneId,
) -> &mut CanonicalLaneEvidence {
    let lane_id = lane_id.as_u32();
    lanes
        .entry(lane_id)
        .or_insert_with(|| CanonicalLaneEvidence {
            lane_id,
            ..CanonicalLaneEvidence::default()
        })
}

fn canonical_lane_entries(status: &SumeragiV2StatusResponse) -> Value {
    let mut lanes = BTreeMap::<u32, CanonicalLaneEvidence>::new();

    for commitment in &status.lane_settlement_commitments {
        let lane = lane_evidence(&mut lanes, commitment.lane_id);
        lane.dataspaces.insert(commitment.dataspace_id.as_u64());
        lane.incarnations
            .insert(commitment.lane_incarnation.to_string());
        lane.settlement_commitments = lane.settlement_commitments.saturating_add(1);
        lane.latest_global_height = lane.latest_global_height.max(commitment.block_height);
    }
    for relay in &status.lane_relay_envelopes {
        let lane = lane_evidence(&mut lanes, relay.lane_id);
        lane.dataspaces.insert(relay.dataspace_id.as_u64());
        lane.incarnations.insert(relay.lane_incarnation.to_string());
        lane.relay_envelopes = lane.relay_envelopes.saturating_add(1);
        lane.latest_global_height = lane.latest_global_height.max(relay.block_height);
    }
    for ownership in &status.lane_payload_ownerships {
        let lane = lane_evidence(&mut lanes, ownership.lane_id);
        lane.dataspaces.insert(ownership.dataspace_id.as_u64());
        lane.incarnations
            .insert(ownership.lane_incarnation.to_string());
        lane.payload_ownerships = lane.payload_ownerships.saturating_add(1);
        lane.latest_global_height = lane.latest_global_height.max(ownership.proposal_height);
        lane.latest_lane_height = lane.latest_lane_height.max(ownership.lane_block_height);
    }
    for committed in &status.committed_lane_blocks {
        let lane = lane_evidence(&mut lanes, committed.lane_id);
        lane.dataspaces.insert(committed.dataspace_id.as_u64());
        lane.incarnations
            .insert(committed.lane_incarnation.to_string());
        lane.committed_blocks = lane.committed_blocks.saturating_add(1);
        lane.latest_lane_height = lane.latest_lane_height.max(committed.lane_block_height);
        if !committed_lane_block_status_counts_as_progress(
            &committed.execution_status,
            committed.executable_payload_available,
        ) {
            lane.blocked_committed_blocks = lane.blocked_committed_blocks.saturating_add(1);
        }
    }
    for session in &status.lane_block_sessions {
        let lane = lane_evidence(&mut lanes, session.lane_id);
        lane.dataspaces.insert(session.dataspace_id.as_u64());
        lane.incarnations
            .insert(session.lane_incarnation.to_string());
        lane.active_sessions = lane.active_sessions.saturating_add(1);
        lane.latest_lane_height = lane.latest_lane_height.max(session.lane_block_height);
        if !session.has_commit_qc || !session.committed_session_drained {
            lane.incomplete_sessions = lane.incomplete_sessions.saturating_add(1);
        }
    }

    Value::Array(
        lanes
            .into_values()
            .map(|lane| {
                let incomplete = lane.incomplete_sessions > 0 || lane.blocked_committed_blocks > 0;
                let status = if incomplete {
                    "incomplete"
                } else if lane.committed_blocks > 0 {
                    "committed"
                } else {
                    "observed"
                };
                Value::Object(Map::from_iter([
                    ("lane_id".into(), Value::from(u64::from(lane.lane_id))),
                    (
                        "dataspace_ids".into(),
                        Value::Array(lane.dataspaces.into_iter().map(Value::from).collect()),
                    ),
                    (
                        "incarnations".into(),
                        Value::Array(lane.incarnations.into_iter().map(Value::from).collect()),
                    ),
                    (
                        "settlement_commitments".into(),
                        Value::from(lane.settlement_commitments),
                    ),
                    ("relay_envelopes".into(), Value::from(lane.relay_envelopes)),
                    (
                        "payload_ownerships".into(),
                        Value::from(lane.payload_ownerships),
                    ),
                    (
                        "committed_blocks".into(),
                        Value::from(lane.committed_blocks),
                    ),
                    ("active_sessions".into(), Value::from(lane.active_sessions)),
                    (
                        "incomplete_sessions".into(),
                        Value::from(lane.incomplete_sessions),
                    ),
                    (
                        "blocked_committed_blocks".into(),
                        Value::from(lane.blocked_committed_blocks),
                    ),
                    (
                        "latest_global_height".into(),
                        Value::from(lane.latest_global_height),
                    ),
                    (
                        "latest_lane_height".into(),
                        Value::from(lane.latest_lane_height),
                    ),
                    ("status".into(), Value::from(status)),
                    ("incomplete".into(), Value::from(incomplete)),
                ]))
            })
            .collect(),
    )
}

fn lane_is_incomplete(entry: &Value) -> bool {
    entry
        .as_object()
        .and_then(|map| map.get("incomplete"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn filter_lane_entries(value: Value, only_incomplete: bool) -> Value {
    if !only_incomplete {
        return value;
    }
    if let Value::Array(entries) = value {
        Value::Array(entries.into_iter().filter(lane_is_incomplete).collect())
    } else {
        value
    }
}

fn count_incomplete(value: &Value) -> usize {
    value.as_array().map_or(0, |entries| {
        entries
            .iter()
            .filter(|entry| lane_is_incomplete(entry))
            .count()
    })
}

fn collect_incomplete_lane_ids(value: &Value) -> Vec<u64> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| lane_is_incomplete(entry))
        .filter_map(|entry| entry.get("lane_id").and_then(Value::as_u64))
        .collect()
}

fn format_lane_summary(value: &Value, only_incomplete: bool) -> String {
    let Some(array) = value.as_array() else {
        return "Canonical lane evidence response was not an array.".to_owned();
    };
    if array.is_empty() {
        return if only_incomplete {
            "No incomplete canonical lane evidence.".to_owned()
        } else {
            "No canonical lane evidence retained.".to_owned()
        };
    }

    let rows = array
        .iter()
        .filter_map(Value::as_object)
        .map(build_lane_row)
        .collect::<Vec<_>>();
    let header = format!(
        "{:>4}  {:<10}  {:<14}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>8}  {:>8}",
        "ID",
        "STATUS",
        "DATASPACES",
        "SETTLE",
        "RELAY",
        "OWNER",
        "COMMIT",
        "SESS",
        "GLOBAL_H",
        "LANE_H"
    );
    let mut formatted = String::with_capacity((rows.len() + 1) * header.len());
    formatted.push_str(&header);
    for row in rows {
        formatted.push('\n');
        formatted.push_str(&row);
    }
    formatted
}

fn build_lane_row(entry: &Map) -> String {
    let lane_id = object_u64(entry, "lane_id");
    let status = entry
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("malformed");
    let dataspaces = entry
        .get("dataspace_ids")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_u64)
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "-".to_owned());
    format!(
        "{lane_id:>4}  {status:<10}  {dataspaces:<14}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>8}  {:>8}",
        object_u64(entry, "settlement_commitments"),
        object_u64(entry, "relay_envelopes"),
        object_u64(entry, "payload_ownerships"),
        object_u64(entry, "committed_blocks"),
        object_u64(entry, "active_sessions"),
        object_u64(entry, "latest_global_height"),
        object_u64(entry, "latest_lane_height"),
    )
}

fn object_u64(entry: &Map, key: &str) -> u64 {
    entry.get(key).and_then(Value::as_u64).unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};

    fn fixture_account_i105(seed: u8) -> String {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed must derive a valid keypair");
        iroha::data_model::account::AccountId::new(key_pair.public_key().clone())
            .canonical_i105()
            .expect("canonical I105")
    }

    #[test]
    fn fixture_account_i105_uses_checked_seed_derivation() {
        assert!(!fixture_account_i105(0x10).is_empty());
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }

    fn canonical_lane_row(lane_id: u64, incomplete: bool) -> Value {
        Value::Object(Map::from_iter([
            ("lane_id".into(), Value::from(lane_id)),
            (
                "dataspace_ids".into(),
                Value::Array(vec![Value::from(lane_id.saturating_add(10))]),
            ),
            ("settlement_commitments".into(), Value::from(1_u64)),
            ("relay_envelopes".into(), Value::from(2_u64)),
            ("payload_ownerships".into(), Value::from(3_u64)),
            ("committed_blocks".into(), Value::from(4_u64)),
            ("active_sessions".into(), Value::from(u64::from(incomplete))),
            ("latest_global_height".into(), Value::from(8_u64)),
            ("latest_lane_height".into(), Value::from(6_u64)),
            (
                "status".into(),
                Value::from(if incomplete {
                    "incomplete"
                } else {
                    "committed"
                }),
            ),
            ("incomplete".into(), Value::from(incomplete)),
        ]))
    }

    #[test]
    fn lane_summary_formats_canonical_evidence_rows() {
        let value = Value::Array(vec![canonical_lane_row(2, true)]);
        let table = format_lane_summary(&value, false);
        assert!(table.contains("incomplete"));
        assert!(table.contains("    2"));
        assert!(table.contains("     4"));
    }

    #[test]
    fn lane_summary_handles_empty() {
        let value = Value::Array(Vec::new());
        assert_eq!(
            format_lane_summary(&value, false),
            "No canonical lane evidence retained."
        );
        assert_eq!(
            format_lane_summary(&value, true),
            "No incomplete canonical lane evidence."
        );
    }

    #[test]
    fn incomplete_filter_and_count_fail_closed() {
        let incomplete = canonical_lane_row(1, true);
        let complete = canonical_lane_row(2, false);
        let malformed = Value::Object(Map::new());
        let all = Value::Array(vec![incomplete, complete, malformed]);
        let filtered = filter_lane_entries(all.clone(), true);
        let entries = filtered.as_array().expect("filtered array");
        assert_eq!(entries.len(), 2);
        assert_eq!(count_incomplete(&all), 2);
        assert_eq!(collect_incomplete_lane_ids(&all), vec![1]);
        assert!(
            lane_is_incomplete(&Value::Null),
            "malformed lane evidence must never be reported as complete"
        );
    }

    #[test]
    fn validator_summary_formats_activation_and_status() {
        let validator = fixture_account_i105(0x11);
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
        let validator = fixture_account_i105(0x12);
        let staker = fixture_account_i105(0x13);
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
    fn truncate_field_respects_unicode_character_boundaries() {
        let input = "いろはにほへとちりぬるをわかよたれそ";
        let expected = "いろはにほへとちりぬるをわかよた";
        assert_eq!(truncate_field(input, 16), expected);
    }
}
