//! Generate FASTPQ row-usage summaries for synthetic batches.
//!
//! This utility builds deterministic transition batches with configurable
//! selector counts so operators can capture regression data (e.g., for the
//! 65 536-row planner ceiling) without replaying full witnesses.

use std::{
    error::Error,
    fs::File,
    io::{self, Write},
    path::PathBuf,
};

use clap::Parser;
use fastpq_prover::{
    OperationKind, RowUsage, StateTransition, Trace, TransitionBatch, build_trace,
};
use norito::json::{self, Map, Value, to_writer, to_writer_pretty};

const DEFAULT_TRANSFER_ROWS: usize = 2048;
const DEFAULT_MINT_ROWS: usize = 64;
const DEFAULT_BURN_ROWS: usize = 32;
const DEFAULT_ROLE_GRANT_ROWS: usize = 8;
const DEFAULT_ROLE_REVOKE_ROWS: usize = 8;
const DEFAULT_META_SET_ROWS: usize = 16;

/// Capture row-usage summaries for synthetic FASTPQ batches.
#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    /// Canonical parameter set to tag the synthetic batch with.
    #[arg(long, default_value = "fastpq-lane-balanced")]
    parameter: String,
    /// Number of transfer selector rows to emit (counts per-row, not per-instruction).
    #[arg(long, default_value_t = DEFAULT_TRANSFER_ROWS)]
    transfer_rows: usize,
    /// Number of mint selector rows to emit.
    #[arg(long, default_value_t = DEFAULT_MINT_ROWS)]
    mint_rows: usize,
    /// Number of burn selector rows to emit.
    #[arg(long, default_value_t = DEFAULT_BURN_ROWS)]
    burn_rows: usize,
    /// Number of role-grant selector rows to emit.
    #[arg(long, default_value_t = DEFAULT_ROLE_GRANT_ROWS)]
    role_grant_rows: usize,
    /// Number of role-revoke selector rows to emit.
    #[arg(long, default_value_t = DEFAULT_ROLE_REVOKE_ROWS)]
    role_revoke_rows: usize,
    /// Number of meta-set selector rows to emit.
    #[arg(long, default_value_t = DEFAULT_META_SET_ROWS)]
    meta_set_rows: usize,
    /// Seed used to derive deterministic balances and identifiers.
    #[arg(long, default_value_t = 0x950d_1eaf)]
    seed: u64,
    /// Optional output path (stdout when omitted).
    #[arg(long)]
    output: Option<PathBuf>,
    /// Emit pretty-printed JSON instead of the compact form.
    #[arg(long)]
    pretty: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("fastpq_row_bench: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let counts = ScenarioCounts::from(&args);
    if counts.total_rows() == 0 {
        return Err("at least one selector row must be requested".into());
    }
    let batch = build_synthetic_batch(&args.parameter, &counts, args.seed);
    let trace = build_trace(&batch)?;
    let summary = summary_value(&args.parameter, &counts, &trace);
    write_summary(&summary, args.output.as_deref(), args.pretty)?;
    Ok(())
}

fn write_summary(
    summary: &Value,
    output_path: Option<&std::path::Path>,
    pretty: bool,
) -> Result<(), Box<dyn Error>> {
    if let Some(path) = output_path {
        let mut file = File::create(path)?;
        write_json(summary, &mut file, pretty)?;
        file.write_all(b"\n")?;
    } else {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        write_json(summary, &mut handle, pretty)?;
        handle.write_all(b"\n")?;
    }
    Ok(())
}

fn write_json<W: Write>(value: &Value, writer: &mut W, pretty: bool) -> Result<(), Box<dyn Error>> {
    let result = if pretty {
        to_writer_pretty(writer, value)
    } else {
        to_writer(writer, value)
    };
    result.map_err(|error| -> Box<dyn Error> { Box::new(error) })
}

fn summary_value(parameter: &str, counts: &ScenarioCounts, trace: &Trace) -> Value {
    let usage = &trace.row_usage;
    // Row counts are bounded by CLI inputs (default ≤ 65k) so casting to f64 is precise enough.
    #[allow(clippy::cast_precision_loss)]
    let ratio = if usage.total_rows == 0 {
        0.0
    } else {
        (usage.transfer_rows as f64) / (usage.total_rows as f64)
    };
    let padded_log2 = log2(trace.padded_len);
    let mut scenario = Map::new();
    scenario.insert("transfer_rows".into(), json_value(&counts.transfer_rows));
    scenario.insert("mint_rows".into(), json_value(&counts.mint_rows));
    scenario.insert("burn_rows".into(), json_value(&counts.burn_rows));
    scenario.insert(
        "role_grant_rows".into(),
        json_value(&counts.role_grant_rows),
    );
    scenario.insert(
        "role_revoke_rows".into(),
        json_value(&counts.role_revoke_rows),
    );
    scenario.insert("meta_set_rows".into(), json_value(&counts.meta_set_rows));
    let total_rows = counts.total_rows();
    scenario.insert("total_rows".into(), json_value(&total_rows));

    let mut trace_map = Map::new();
    trace_map.insert("rows".into(), json_value(&trace.rows));
    trace_map.insert("padded_len".into(), json_value(&trace.padded_len));
    trace_map.insert("padded_log2".into(), json_value(&padded_log2));

    let mut root = Map::new();
    root.insert("parameter".into(), json_value(parameter));
    root.insert("scenario".into(), Value::Object(scenario));
    root.insert("trace".into(), Value::Object(trace_map));
    root.insert("row_usage".into(), row_usage_json(usage, ratio));
    Value::Object(root)
}

fn row_usage_json(usage: &RowUsage, ratio: f64) -> Value {
    let mut map = Map::new();
    map.insert("total_rows".into(), json_value(&usage.total_rows));
    map.insert("transfer_rows".into(), json_value(&usage.transfer_rows));
    let non_transfer_rows = usage.non_transfer_rows();
    map.insert("non_transfer_rows".into(), json_value(&non_transfer_rows));
    map.insert("mint_rows".into(), json_value(&usage.mint_rows));
    map.insert("burn_rows".into(), json_value(&usage.burn_rows));
    map.insert("role_grant_rows".into(), json_value(&usage.role_grant_rows));
    map.insert(
        "role_revoke_rows".into(),
        json_value(&usage.role_revoke_rows),
    );
    map.insert("meta_set_rows".into(), json_value(&usage.meta_set_rows));
    map.insert("permission_rows".into(), json_value(&usage.permission_rows));
    let transfer_ratio = round3(ratio);
    map.insert("transfer_ratio".into(), json_value(&transfer_ratio));
    Value::Object(map)
}

fn json_value<T>(value: &T) -> Value
where
    T: json::JsonSerialize + ?Sized,
{
    json::to_value(value).expect("serialize JSON value")
}

fn round3(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}

fn log2(value: usize) -> Option<u32> {
    if value == 0 {
        return None;
    }
    Some(usize::BITS - value.leading_zeros() - 1)
}

#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_field_names)] // explicit *_rows suffix keeps CLI semantics clear
struct ScenarioCounts {
    transfer_rows: usize,
    mint_rows: usize,
    burn_rows: usize,
    role_grant_rows: usize,
    role_revoke_rows: usize,
    meta_set_rows: usize,
}

impl ScenarioCounts {
    const fn total_rows(&self) -> usize {
        self.transfer_rows
            + self.mint_rows
            + self.burn_rows
            + self.role_grant_rows
            + self.role_revoke_rows
            + self.meta_set_rows
    }
}

impl From<&Args> for ScenarioCounts {
    fn from(args: &Args) -> Self {
        Self {
            transfer_rows: args.transfer_rows,
            mint_rows: args.mint_rows,
            burn_rows: args.burn_rows,
            role_grant_rows: args.role_grant_rows,
            role_revoke_rows: args.role_revoke_rows,
            meta_set_rows: args.meta_set_rows,
        }
    }
}

fn build_synthetic_batch(parameter: &str, counts: &ScenarioCounts, seed: u64) -> TransitionBatch {
    let mut batch = TransitionBatch::new(parameter, fastpq_prover::PublicInputs::default());
    batch.transitions.reserve(counts.total_rows());
    batch
        .metadata
        .insert("slot".into(), encode_u64(seed.rotate_left(7)));
    batch
        .metadata
        .insert("dsid".into(), encode_u64(seed.rotate_left(19)));
    let mut generator = RowGenerator::new(seed);
    for _ in 0..counts.transfer_rows {
        batch.push(generator.next_transfer());
    }
    for _ in 0..counts.mint_rows {
        batch.push(generator.next_mint());
    }
    for _ in 0..counts.burn_rows {
        batch.push(generator.next_burn());
    }
    for _ in 0..counts.role_grant_rows {
        batch.push(generator.next_role_grant());
    }
    for _ in 0..counts.role_revoke_rows {
        batch.push(generator.next_role_revoke());
    }
    for _ in 0..counts.meta_set_rows {
        batch.push(generator.next_meta_set());
    }
    batch
}

struct RowGenerator {
    seed: u64,
    transfer_index: usize,
    mint_index: usize,
    burn_index: usize,
    grant_index: usize,
    revoke_index: usize,
    meta_index: usize,
}

impl RowGenerator {
    fn new(seed: u64) -> Self {
        Self {
            seed,
            transfer_index: 0,
            mint_index: 0,
            burn_index: 0,
            grant_index: 0,
            revoke_index: 0,
            meta_index: 0,
        }
    }

    fn next_transfer(&mut self) -> StateTransition {
        let idx = self.transfer_index;
        self.transfer_index = self.transfer_index.wrapping_add(1);
        let asset_group = idx / 2;
        let asset_id = format!("bench_asset_{asset_group}#lane");
        let account_id = format!("account_{idx:05}");
        let key = format!("asset/{asset_id}/{account_id}").into_bytes();
        let rotation = u32::try_from(idx % 31).expect("rotation fits in u32") + 1;
        let base = 1_000_000_u64
            .wrapping_add(self.seed.rotate_left(rotation))
            .wrapping_add(idx as u64);
        let amount = 1 + ((self.seed ^ (idx as u64 * 7919)) % 1_000);
        let (pre, post) = if idx.is_multiple_of(2) {
            (base, base.saturating_sub(amount))
        } else {
            (base, base.saturating_add(amount))
        };
        StateTransition::new(
            key,
            encode_u64(pre),
            encode_u64(post),
            OperationKind::Transfer,
        )
    }

    fn next_mint(&mut self) -> StateTransition {
        let idx = self.mint_index;
        self.mint_index = self.mint_index.wrapping_add(1);
        let key = format!("asset/mint_asset_{idx}#lane/emitter").into_bytes();
        let base = 50_000_u64 + (idx as u64);
        let rotation = u32::try_from(idx % 17).expect("rotation fits in u32");
        let amount = 25 + ((self.seed.rotate_right(rotation)) % 250);
        let post = base.saturating_add(amount);
        StateTransition::new(key, encode_u64(base), encode_u64(post), OperationKind::Mint)
    }

    fn next_burn(&mut self) -> StateTransition {
        let idx = self.burn_index;
        self.burn_index = self.burn_index.wrapping_add(1);
        let key = format!("asset/burn_asset_{idx}#lane/sink").into_bytes();
        let base = 75_000_u64 + (idx as u64 * 2);
        let amount = 10 + ((self.seed ^ (idx as u64 * 13)) % 128);
        let post = base.saturating_sub(amount);
        StateTransition::new(key, encode_u64(base), encode_u64(post), OperationKind::Burn)
    }

    fn next_role_grant(&mut self) -> StateTransition {
        let idx = self.grant_index;
        self.grant_index = self.grant_index.wrapping_add(1);
        let key = format!("role/grant/{idx:04}").into_bytes();
        StateTransition::new(
            key,
            encode_u64(idx as u64),
            encode_u64(idx as u64 + 1),
            OperationKind::RoleGrant {
                role_id: format!("role:bench:{idx:04}").into_bytes(),
                permission_id: format!("perm:bench:{idx:04}").into_bytes(),
                epoch: 1 + idx as u64,
            },
        )
    }

    fn next_role_revoke(&mut self) -> StateTransition {
        let idx = self.revoke_index;
        self.revoke_index = self.revoke_index.wrapping_add(1);
        let key = format!("role/revoke/{idx:04}").into_bytes();
        StateTransition::new(
            key,
            encode_u64(idx as u64 + 10),
            encode_u64(idx as u64),
            OperationKind::RoleRevoke {
                role_id: format!("role:bench:{idx:04}").into_bytes(),
                permission_id: format!("perm:bench:{idx:04}").into_bytes(),
                epoch: 10 + idx as u64,
            },
        )
    }

    fn next_meta_set(&mut self) -> StateTransition {
        let idx = self.meta_index;
        self.meta_index = self.meta_index.wrapping_add(1);
        let key = format!("meta/domain_{idx}").into_bytes();
        let pre = (idx as u64) * 2;
        StateTransition::new(
            key,
            encode_u64(pre),
            encode_u64(pre + 5),
            OperationKind::MetaSet,
        )
    }
}

fn encode_u64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_count_totals_match_batch_rows() {
        let counts = ScenarioCounts {
            transfer_rows: 10,
            mint_rows: 2,
            burn_rows: 1,
            role_grant_rows: 1,
            role_revoke_rows: 1,
            meta_set_rows: 2,
        };
        let batch = build_synthetic_batch("fastpq-lane-balanced", &counts, 99);
        assert_eq!(batch.transitions.len(), counts.total_rows());
    }

    #[test]
    fn handles_65k_transfer_rows() {
        let counts = ScenarioCounts {
            transfer_rows: 65_536,
            mint_rows: 0,
            burn_rows: 0,
            role_grant_rows: 0,
            role_revoke_rows: 0,
            meta_set_rows: 0,
        };
        let batch = build_synthetic_batch("fastpq-lane-balanced", &counts, 7);
        let trace = build_trace(&batch).expect("trace");
        assert_eq!(trace.rows, counts.total_rows());
        assert_eq!(trace.row_usage.transfer_rows, counts.transfer_rows);
        assert_eq!(trace.padded_len, 65_536);
    }
}
