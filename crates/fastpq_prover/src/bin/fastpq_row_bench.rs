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
use fastpq_prover::{OperationKind, RowUsage, StateTransition, TransitionBatch};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    asset::id::AssetDefinitionId,
    domain::DomainId,
    fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
};
use iroha_primitives::numeric::Numeric;
use norito::json::{self, Map, Value, to_writer, to_writer_pretty};
use norito::to_bytes;
use std::str::FromStr;

const DEFAULT_TRANSFER_ROWS: usize = 2048;
const DEFAULT_MINT_ROWS: usize = 64;
const DEFAULT_BURN_ROWS: usize = 32;
const DEFAULT_ROLE_GRANT_ROWS: usize = 8;
const DEFAULT_ROLE_REVOKE_ROWS: usize = 8;
const DEFAULT_META_SET_ROWS: usize = 16;
const ACCOUNT_POOL_SIZE: usize = 256;

/// Capture row-usage summaries for synthetic FASTPQ batches.
#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    /// Canonical parameter set to tag the synthetic batch with.
    #[arg(long, default_value = "fastpq-lane-balanced")]
    parameter: String,
    /// Number of transfer selector rows to emit (counts per-row; must be even).
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
    if counts.transfer_rows % 2 != 0 {
        return Err(format!(
            "transfer_rows must be even to build paired transfer transcripts (got {})",
            counts.transfer_rows
        )
        .into());
    }
    let batch = build_synthetic_batch(&args.parameter, &counts, args.seed, false);
    let trace = trace_summary(&batch);
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

fn summary_value(parameter: &str, counts: &ScenarioCounts, trace: &TraceSummary) -> Value {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TraceSummary {
    rows: usize,
    padded_len: usize,
    row_usage: RowUsage,
}

fn trace_summary(batch: &TransitionBatch) -> TraceSummary {
    let rows = batch.transitions.len();
    let padded_len = rows.max(1).next_power_of_two();
    let mut usage = RowUsage {
        total_rows: rows,
        ..RowUsage::default()
    };
    for transition in &batch.transitions {
        match transition.operation {
            OperationKind::Transfer => {
                usage.transfer_rows = usage.transfer_rows.saturating_add(1);
            }
            OperationKind::Mint => {
                usage.mint_rows = usage.mint_rows.saturating_add(1);
            }
            OperationKind::Burn => {
                usage.burn_rows = usage.burn_rows.saturating_add(1);
            }
            OperationKind::RoleGrant { .. } => {
                usage.role_grant_rows = usage.role_grant_rows.saturating_add(1);
                usage.permission_rows = usage.permission_rows.saturating_add(1);
            }
            OperationKind::RoleRevoke { .. } => {
                usage.role_revoke_rows = usage.role_revoke_rows.saturating_add(1);
                usage.permission_rows = usage.permission_rows.saturating_add(1);
            }
            OperationKind::MetaSet => {
                usage.meta_set_rows = usage.meta_set_rows.saturating_add(1);
            }
        }
    }
    TraceSummary {
        rows,
        padded_len,
        row_usage: usage,
    }
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

fn build_synthetic_batch(
    parameter: &str,
    counts: &ScenarioCounts,
    seed: u64,
    include_transcripts: bool,
) -> TransitionBatch {
    let mut batch = TransitionBatch::new(parameter, fastpq_prover::PublicInputs::default());
    batch.transitions.reserve(counts.total_rows());
    batch.public_inputs.dsid = seed_dsid(seed);
    batch.public_inputs.slot = seed.rotate_left(7);
    batch.public_inputs.old_root = seed_root(seed ^ 0xA5A5_A5A5_A5A5_A5A5);
    batch.public_inputs.new_root = seed_root(seed ^ 0x5A5A_5A5A_5A5A_5A5A);
    batch.public_inputs.tx_set_hash = seed_root(seed ^ 0xC3C3_C3C3_C3C3_C3C3);
    let mut generator = RowGenerator::new(seed);
    let transfer_pairs = counts.transfer_rows / 2;
    let mut transcripts = if include_transcripts {
        Vec::with_capacity(transfer_pairs)
    } else {
        Vec::new()
    };
    for _ in 0..transfer_pairs {
        let (transcript, sender, receiver) = generator.next_transfer_pair(include_transcripts);
        batch.push(sender);
        batch.push(receiver);
        if let Some(transcript) = transcript {
            transcripts.push(transcript);
        }
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
    if include_transcripts && !transcripts.is_empty() {
        batch.metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            to_bytes(&transcripts).expect("encode transcripts"),
        );
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
    sender_accounts: Vec<AccountId>,
    receiver_accounts: Vec<AccountId>,
}

impl RowGenerator {
    fn new(seed: u64) -> Self {
        let domain = DomainId::from_str("lane").expect("domain id");
        let mut sender_accounts = Vec::with_capacity(ACCOUNT_POOL_SIZE);
        let mut receiver_accounts = Vec::with_capacity(ACCOUNT_POOL_SIZE);
        for idx in 0..ACCOUNT_POOL_SIZE {
            sender_accounts.push(deterministic_account(
                &format!("bench_sender_{idx:04}"),
                &domain,
            ));
            receiver_accounts.push(deterministic_account(
                &format!("bench_receiver_{idx:04}"),
                &domain,
            ));
        }
        Self {
            seed,
            transfer_index: 0,
            mint_index: 0,
            burn_index: 0,
            grant_index: 0,
            revoke_index: 0,
            meta_index: 0,
            sender_accounts,
            receiver_accounts,
        }
    }

    fn next_transfer_pair(
        &mut self,
        include_transcript: bool,
    ) -> (Option<TransferTranscript>, StateTransition, StateTransition) {
        let pair_idx = self.transfer_index;
        self.transfer_index = self.transfer_index.wrapping_add(1);
        let asset_group = pair_idx % 128;
        let asset_id = format!("bench_asset_{asset_group}#lane");
        let from_account = self.sender_accounts[pair_idx % self.sender_accounts.len()].clone();
        let to_account = self.receiver_accounts[pair_idx % self.receiver_accounts.len()].clone();
        let asset_definition = AssetDefinitionId::from_str(&asset_id).expect("asset definition");
        let rotation = u32::try_from(pair_idx % 31).expect("rotation fits in u32") + 1;
        let base = 1_000_000_u64
            .wrapping_add(self.seed.rotate_left(rotation))
            .wrapping_add(pair_idx as u64);
        let amount = 1 + ((self.seed ^ (pair_idx as u64 * 7919)) % 1_000);
        let from_pre = base;
        let from_post = base.saturating_sub(amount);
        let to_pre = base / 2;
        let to_post = to_pre.saturating_add(amount);
        let transcript = if include_transcript {
            let delta = TransferDeltaTranscript {
                from_account: from_account.clone(),
                to_account: to_account.clone(),
                asset_definition: asset_definition.clone(),
                amount: Numeric::from(amount),
                from_balance_before: Numeric::from(from_pre),
                from_balance_after: Numeric::from(from_post),
                to_balance_before: Numeric::from(to_pre),
                to_balance_after: Numeric::from(to_post),
                from_merkle_proof: None,
                to_merkle_proof: None,
            };
            let mut payload = Vec::with_capacity(32);
            payload.extend_from_slice(b"fastpq-row-bench");
            payload.extend_from_slice(&self.seed.to_le_bytes());
            payload.extend_from_slice(&(pair_idx as u64).to_le_bytes());
            let batch_hash = Hash::new(payload);
            let digest =
                fastpq_prover::gadgets::transfer::compute_poseidon_digest(&delta, &batch_hash);
            Some(TransferTranscript {
                batch_hash,
                deltas: vec![delta],
                authority_digest: Hash::new(b"authority"),
                poseidon_preimage_digest: Some(digest),
            })
        } else {
            None
        };
        let sender = StateTransition::new(
            format!("asset/{asset_definition}/{from_account}").into_bytes(),
            encode_u64(from_pre),
            encode_u64(from_post),
            OperationKind::Transfer,
        );
        let receiver = StateTransition::new(
            format!("asset/{asset_definition}/{to_account}").into_bytes(),
            encode_u64(to_pre),
            encode_u64(to_post),
            OperationKind::Transfer,
        );
        (transcript, sender, receiver)
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

fn seed_dsid(seed: u64) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&seed.to_le_bytes());
    out[8..].copy_from_slice(&seed.rotate_left(17).to_le_bytes());
    out
}

fn seed_root(seed: u64) -> [u8; 32] {
    let parts = [
        seed,
        seed.rotate_left(13),
        seed.rotate_left(27),
        seed.rotate_left(41),
    ];
    let mut out = [0u8; 32];
    for (chunk, part) in out.chunks_mut(8).zip(parts) {
        chunk.copy_from_slice(&part.to_le_bytes());
    }
    out
}

fn deterministic_account(label: &str, domain: &DomainId) -> AccountId {
    let seed: [u8; Hash::LENGTH] = Hash::new(format!("{label}@{domain}")).into();
    let keypair = KeyPair::from_seed(seed.to_vec(), Algorithm::default());
    let _ = domain;
    AccountId::new(keypair.public_key().clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fastpq_prover::build_trace;

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
        let batch = build_synthetic_batch("fastpq-lane-balanced", &counts, 99, false);
        assert_eq!(batch.transitions.len(), counts.total_rows());
    }

    #[test]
    fn trace_summary_matches_build_trace() {
        let counts = ScenarioCounts {
            transfer_rows: 4,
            mint_rows: 2,
            burn_rows: 1,
            meta_set_rows: 1,
            role_grant_rows: 0,
            role_revoke_rows: 0,
        };
        let batch = build_synthetic_batch("fastpq-lane-balanced", &counts, 42, true);
        let summary = trace_summary(&batch);
        let trace = build_trace(&batch).expect("trace");
        assert_eq!(summary.rows, trace.rows);
        assert_eq!(summary.padded_len, trace.padded_len);
        assert_eq!(summary.row_usage, trace.row_usage);
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
        let batch = build_synthetic_batch("fastpq-lane-balanced", &counts, 7, false);
        let summary = trace_summary(&batch);
        assert_eq!(summary.rows, counts.total_rows());
        assert_eq!(summary.row_usage.transfer_rows, counts.transfer_rows);
        assert_eq!(summary.padded_len, 65_536);
    }
}
