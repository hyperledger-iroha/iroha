//! Bounded command adapter for retained preparation, canonical export and replay.
//!
//! Preparation replies bind transport identities; export and replay replies
//! describe completed verification under caller-supplied launch authority. The parent must independently pin this executable, retain every
//! input, require terminal exit zero, and replay the published artifact. A reply
//! alone does not establish the launch authority or a scaling release result.
//! TODO: integrate the fixed-lane producer and deadline-bound parent readiness
//! barrier before permitting this adapter to satisfy the public scaling gate.

use super::export::filesystem::{
    CanonicalProofIdentity, PrepareOutputCaps, PreparedLaunchIdentity, PreparedOutputPair,
    ProofInputBinding, ProofOutput, export_bound_request, open_launcher, prepare_bound,
    replay_bound_request,
};
use crate::{Outcome, RunArgs};
use clap::{Args as ClapArgs, Subcommand};
use color_eyre::eyre::{Result, ensure, eyre};
use iroha_core::kura::CanonicalKuraEvidenceLimits;
use iroha_crypto::Hash;
use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_REPLY_HEADER_BYTES: usize = 1024;

/// Prepare, export or independently replay bounded canonical scaling evidence.
#[derive(Clone, Debug, ClapArgs)]
pub(crate) struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Debug, Subcommand)]
enum Command {
    /// Observe a stopped store's durable height under retained original genesis
    StoppedTip(Box<super::export::stopped_tip_command::Args>),
    /// Authenticate original launch inputs and publish canonical preparation facts
    Facts(Box<super::export::facts_command::Args>),
    /// Prepare two canonical transports from independently retained launch facts
    Prepare(PrepareArgs),
    /// Authenticate an immutable Kura interval and publish one canonical proof
    Export(ExportArgs),
    /// Reauthenticate a canonical proof and emit its complete ordered rows
    Replay(ReplayArgs),
}

#[derive(Clone, Debug, ClapArgs)]
struct PrepareArgs {
    /// Independently selected lowercase SHA-256 invocation identity
    #[arg(long, value_parser = parse_sha256)]
    invocation_id: [u8; 32],
    /// Absolute path to independently retained canonical preparation facts
    #[arg(long)]
    facts: PathBuf,
    /// Independently pinned raw SHA-256 of the facts file
    #[arg(long, value_parser = parse_sha256)]
    facts_sha256: [u8; 32],
    /// Reserved facts bytes, between 1 and 268435456
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    facts_max_bytes: u64,
    /// New absolute launcher request path; existing destinations are rejected
    #[arg(long)]
    request_output: PathBuf,
    /// New absolute supplied evidence bundle path; existing destinations are rejected
    #[arg(long)]
    bundle_output: PathBuf,
    /// Reserved request output bytes, between 1 and 268435456
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    request_max_bytes: u64,
    /// Reserved bundle output bytes, between 1 and 268435456
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    bundle_max_bytes: u64,
    /// Aggregate facts and both output reservations, between 1 and 268435456
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    total_max_bytes: u64,
    /// Reserved complete JSON reply bytes, including its final newline
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    reply_max_bytes: u64,
}

#[derive(Clone, Debug, ClapArgs)]
struct CommonArgs {
    /// Independently selected lowercase SHA-256 invocation identity
    #[arg(long, value_parser = parse_sha256)]
    invocation_id: [u8; 32],
    /// Absolute path to the independently retained canonical launcher request
    #[arg(long)]
    request: PathBuf,
    /// Independently pinned raw SHA-256 of the request file
    #[arg(long, value_parser = parse_sha256)]
    request_sha256: [u8; 32],
    /// Reserved request bytes, between 1 and 268435456
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    request_max_bytes: u64,
    /// Absolute path to the supplied evidence bundle or canonical replay proof
    #[arg(long)]
    input: PathBuf,
    /// Independently pinned raw SHA-256 of the input file
    #[arg(long, value_parser = parse_sha256)]
    input_sha256: [u8; 32],
    /// Reserved input bytes, between 1 and 268435456
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    input_max_bytes: u64,
    /// Reserved complete JSON reply bytes, including its final newline
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    reply_max_bytes: u64,
}

#[derive(Clone, Debug, ClapArgs)]
struct ExportArgs {
    #[command(flatten)]
    common: CommonArgs,
    /// Exact absolute lane directory containing the canonical block journals
    #[arg(long)]
    block_store: PathBuf,
    /// Exact absolute canonical merge-log file
    #[arg(long)]
    merge_log: PathBuf,
    /// New absolute proof path; existing destinations are rejected
    #[arg(long)]
    output: PathBuf,
    /// Reserved canonical output bytes, between 1 and 268435456
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    output_max_bytes: u64,
    #[command(flatten)]
    reader: ReaderArgs,
}

#[derive(Clone, Debug, ClapArgs)]
struct ReplayArgs {
    #[command(flatten)]
    common: CommonArgs,
    /// Independently pinned marked Iroha hash of the canonical proof bytes
    #[arg(long, value_parser = parse_iroha_hash)]
    proof_iroha_hash: Hash,
}

#[derive(Clone, Debug, ClapArgs)]
pub(super) struct ReaderArgs {
    /// First required carrier height, inclusive
    #[arg(long)]
    first_height: u64,
    /// Last required carrier height, inclusive
    #[arg(long)]
    last_height: u64,
    /// Maximum complete journal height admitted before reading
    #[arg(long)]
    max_committed_blocks: u64,
    /// Maximum underlying blocks.data bytes
    #[arg(long)]
    max_store_data_bytes: u64,
    /// Maximum canonical wire bytes for one carrier
    #[arg(long)]
    max_carrier_bytes: u64,
    /// Maximum complete merge-log bytes
    #[arg(long)]
    max_merge_log_bytes: u64,
    /// Maximum frames in the complete merge log
    #[arg(long)]
    max_merge_frames: u64,
    /// Maximum cumulative carrier and merge-entry bytes returned by the reader
    #[arg(long)]
    reader_max_output_bytes: u64,
    /// Maximum cumulative owned allocation per decoder invocation
    #[arg(long)]
    max_decode_allocation_bytes: u64,
    /// Independently expected Unix owner of the store directories and files
    #[arg(long)]
    owner_uid: u32,
}

impl ReaderArgs {
    pub(super) fn into_limits(self) -> Result<CanonicalKuraEvidenceLimits> {
        Ok(CanonicalKuraEvidenceLimits {
            first_height: self.first_height,
            last_height: self.last_height,
            max_committed_blocks: self.max_committed_blocks,
            max_store_data_bytes: self.max_store_data_bytes,
            max_carrier_bytes: usize::try_from(self.max_carrier_bytes)?,
            max_merge_log_bytes: self.max_merge_log_bytes,
            max_merge_frames: self.max_merge_frames,
            max_output_bytes: self.reader_max_output_bytes,
            max_decode_allocation_bytes: usize::try_from(self.max_decode_allocation_bytes)?,
            owner_uid: self.owner_uid,
        })
    }
}

impl PrepareArgs {
    fn bindings(&self) -> Result<(ProofInputBinding, PrepareOutputCaps)> {
        ensure!(
            [
                self.facts_max_bytes,
                self.request_max_bytes,
                self.bundle_max_bytes,
                self.total_max_bytes,
                self.reply_max_bytes,
            ]
            .iter()
            .all(|n| (1..=MAX_BYTES).contains(n)),
            "invalid prepare command byte reservations"
        );
        ensure!(
            self.facts_max_bytes
                .checked_add(self.request_max_bytes)
                .and_then(|n| n.checked_add(self.bundle_max_bytes))
                .is_some_and(|n| n <= self.total_max_bytes),
            "prepare facts and output reservations exceed total cap"
        );
        Ok((
            ProofInputBinding {
                path: self.facts.clone(),
                sha256: self.facts_sha256,
                max_bytes: self.facts_max_bytes,
            },
            PrepareOutputCaps {
                request_bytes: self.request_max_bytes,
                bundle_bytes: self.bundle_max_bytes,
                total_bytes: self.total_max_bytes,
            },
        ))
    }
}

impl CommonArgs {
    fn bindings(&self) -> Result<(ProofInputBinding, ProofInputBinding)> {
        ensure!(
            (1..=MAX_BYTES).contains(&self.request_max_bytes)
                && (1..=MAX_BYTES).contains(&self.input_max_bytes)
                && (1..=MAX_BYTES).contains(&self.reply_max_bytes),
            "invalid command byte reservations"
        );
        Ok((
            ProofInputBinding {
                path: self.request.clone(),
                sha256: self.request_sha256,
                max_bytes: self.request_max_bytes,
            },
            ProofInputBinding {
                path: self.input.clone(),
                sha256: self.input_sha256,
                max_bytes: self.input_max_bytes,
            },
        ))
    }
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        match self.command {
            Command::StoppedTip(args) => (*args).run(writer)?,
            Command::Facts(args) => (*args).run(writer)?,
            Command::Prepare(args) => {
                let (facts, caps) = args.bindings()?;
                let outputs =
                    PreparedOutputPair::admit(&args.request_output, &args.bundle_output, caps)?;
                let prepared = prepare_bound(facts, outputs)?;
                let identity = prepared.identity()?;
                PrepareReply::new(&args, identity)?.write(writer)?;
                ensure!(
                    prepared.identity()? == identity,
                    "prepared transport identity changed during reply"
                );
            }
            Command::Export(args) => {
                let (request, input) = args.common.bindings()?;
                let request = open_launcher(request)?;
                let output = ProofOutput::admit(&args.output, args.output_max_bytes)?;
                let proof = export_bound_request(
                    request,
                    &args.block_store,
                    &args.merge_log,
                    args.reader.into_limits()?,
                    input,
                )?;
                let published = output.publish(proof)?;
                let identity = published.identity()?;
                ReplyPrefix::new(&args.common, "export", identity)?.write(writer, None)?;
                ensure!(
                    published.identity()? == identity,
                    "proof identity changed during reply"
                );
            }
            Command::Replay(args) => {
                let (request, input) = args.common.bindings()?;
                let request = open_launcher(request)?;
                let proof = replay_bound_request(request, args.proof_iroha_hash, input)?;
                let identity = proof.identity()?;
                let reply = ReplyPrefix::new(&args.common, "replay", identity)?;
                let projection = proof.json_projection(reply.projection_bytes)?;
                reply.write(writer, Some(&projection))?;
                ensure!(
                    proof.identity()? == identity,
                    "proof identity changed during reply"
                );
            }
        }
        Ok(())
    }
}

pub(super) fn parse_sha256(value: &str) -> std::result::Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("expected exactly 64 lowercase hexadecimal characters".to_owned());
    }
    let mut result = [0u8; 32];
    hex::decode_to_slice(value, &mut result).map_err(|_| "invalid SHA-256".to_owned())?;
    Ok(result)
}

fn parse_iroha_hash(value: &str) -> std::result::Result<Hash, String> {
    parse_sha256(value)?;
    value
        .parse()
        .map_err(|_| "invalid marked Iroha hash".to_owned())
}

// Preparation reports raw file identities only. It neither constructs nor
// substitutes for a canonical execution proof or independent launch authority.
struct PrepareReply {
    bytes: Vec<u8>,
}
impl PrepareReply {
    fn new(args: &PrepareArgs, identity: PreparedLaunchIdentity) -> Result<Self> {
        ensure!(
            (1..=MAX_BYTES).contains(&args.reply_max_bytes),
            "invalid prepare reply reservation"
        );
        ensure!(
            identity.facts.raw_sha256 == args.facts_sha256,
            "prepared facts identity differs from independent pin"
        );
        // Four strings have fixed 64-byte values; all other fields are fixed
        // literals or u64 integers. No paths or variable-length rows are echoed.
        let metadata = norito::json!({
            "version": 1,
            "operation": "prepare",
            "invocation_id": (hex::encode(args.invocation_id)),
            "facts_sha256": (hex::encode(identity.facts.raw_sha256)),
            "facts_bytes": (identity.facts.byte_length),
            "request_sha256": (hex::encode(identity.request.raw_sha256)),
            "request_bytes": (identity.request.byte_length),
            "bundle_sha256": (hex::encode(identity.bundle.raw_sha256)),
            "bundle_bytes": (identity.bundle.byte_length),
        });
        let mut bytes = norito::json::to_vec(&metadata)?;
        bytes.push(b'\n');
        ensure!(
            bytes.len() <= MAX_REPLY_HEADER_BYTES,
            "prepare reply exceeds fixed bound"
        );
        ensure!(
            u64::try_from(bytes.len())? <= args.reply_max_bytes,
            "complete prepare reply exceeds byte reservation"
        );
        Ok(Self { bytes })
    }
    fn write<T: Write>(self, writer: &mut BufWriter<T>) -> Outcome {
        writer.write_all(&self.bytes)?;
        writer.flush()?;
        Ok(())
    }
}

// A fixed-size interop header and its independently reserved projection space.
// This is only an output bound, never a verification or launch-authority token.
struct ReplyPrefix {
    bytes: Vec<u8>,
    projection_bytes: u64,
    has_rows: bool,
}
impl ReplyPrefix {
    fn new(common: &CommonArgs, operation: &str, identity: CanonicalProofIdentity) -> Result<Self> {
        ensure!(
            operation == "export" || operation == "replay",
            "invalid reply operation"
        );
        ensure!(
            (1..=MAX_BYTES).contains(&common.reply_max_bytes),
            "invalid reply reservation"
        );
        // All variable-width strings have exactly 64 characters. This header has
        // a separate fixed 1 KiB bound before any row projection is materialized.
        let metadata = norito::json!({
            "version": 1,
            "operation": operation,
            "invocation_id": (hex::encode(common.invocation_id)),
            "request_sha256": (hex::encode(common.request_sha256)),
            "input_sha256": (hex::encode(common.input_sha256)),
            "proof_sha256": (hex::encode(identity.raw_sha256)),
            "proof_iroha_hash": (identity.iroha_hash.to_string()),
            "proof_bytes": (identity.byte_length),
        });
        let mut bytes = norito::json::to_vec(&metadata)?;
        ensure!(bytes.pop() == Some(b'}'), "reply header is not an object");
        let has_rows = operation == "replay";
        if has_rows {
            bytes.extend_from_slice(b",\"rows\":");
        }
        ensure!(
            bytes.len() <= MAX_REPLY_HEADER_BYTES,
            "reply header exceeds fixed bound"
        );
        let framing = u64::try_from(bytes.len())?
            .checked_add(2)
            .ok_or_else(|| eyre!("reply framing byte count overflow"))?;
        let projection_bytes = common
            .reply_max_bytes
            .checked_sub(framing)
            .ok_or_else(|| eyre!("complete reply exceeds byte reservation"))?;
        ensure!(
            !has_rows || projection_bytes >= 2,
            "reply has no room for its row array"
        );
        Ok(Self {
            bytes,
            projection_bytes,
            has_rows,
        })
    }
    fn write<T: Write>(self, writer: &mut BufWriter<T>, projection: Option<&[u8]>) -> Outcome {
        ensure!(
            self.has_rows == projection.is_some(),
            "invalid reply operation"
        );
        ensure!(
            u64::try_from(projection.map_or(0, <[u8]>::len))? <= self.projection_bytes,
            "complete reply exceeds byte reservation"
        );
        // Append the canonical row array directly. Re-parsing or concatenating
        // the entire result would allocate a second projection-sized buffer.
        writer.write_all(&self.bytes)?;
        if let Some(projection) = projection {
            writer.write_all(projection)?;
        }
        writer.write_all(b"}\n")?;
        writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
