//! Bounded stopped-height observation with original-genesis and Core custody through reply.
//!
//! This reports a durable marker under an independently pinned original genesis. It does not
//! authenticate later carrier QCs or execution. Collection and facts still verify the full
//! interval from genesis through the observed height before any scaling result is accepted.

use super::filesystem::{ProofInputBinding, StoppedTipIdentity, observe_stopped_tip};
use crate::{
    Outcome, RunArgs,
    kura::scaling_evidence::command::{ReaderArgs, parse_sha256},
};
use clap::Args as ClapArgs;
use color_eyre::eyre::{Result, ensure};
use iroha_data_model::NetworkId;
use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

const MAX_GENESIS_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REPLY_BYTES: usize = 512;

/// Observe the complete durable height while retaining the original genesis and store.
#[derive(Clone, Debug, ClapArgs)]
pub(crate) struct Args {
    /// Independently selected lowercase SHA-256 invocation identity
    #[arg(long, value_parser = parse_sha256)]
    invocation_id: [u8; 32],
    /// Absolute path to the independently retained original canonical signed genesis
    #[arg(long)]
    signed_genesis: PathBuf,
    /// Independently pinned raw SHA-256 of the original signed genesis
    #[arg(long, value_parser = parse_sha256)]
    signed_genesis_sha256: [u8; 32],
    /// Maximum original genesis bytes, between 1 and 33554432
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_GENESIS_BYTES))]
    signed_genesis_max_bytes: u64,
    /// Expected genesis-header NetworkId in its canonical checked hash literal form
    #[arg(long)]
    network_id: NetworkId,
    /// Exact absolute stopped lane directory containing the canonical block journals
    #[arg(long)]
    block_store: PathBuf,
    /// Exact absolute stopped canonical merge-log file
    #[arg(long)]
    merge_log: PathBuf,
    /// Reserved complete JSON reply bytes, including its final newline
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_REPLY_BYTES as u64))]
    reply_max_bytes: u64,
    /// Explicit finite Core reader bounds; first and last height must both be one
    #[command(flatten)]
    reader: ReaderArgs,
}

impl<W: Write> RunArgs<W> for Args {
    fn run(self, writer: &mut BufWriter<W>) -> Outcome {
        ensure!(
            (1..=MAX_GENESIS_BYTES).contains(&self.signed_genesis_max_bytes)
                && (1..=MAX_REPLY_BYTES as u64).contains(&self.reply_max_bytes),
            "invalid stopped-tip genesis or reply reservation"
        );
        let reader = self.reader.into_limits()?;
        ensure!(
            reader.first_height == 1 && reader.last_height == 1,
            "stopped-tip observation requires exactly the genesis interval"
        );
        let retained = observe_stopped_tip(
            ProofInputBinding {
                path: self.signed_genesis,
                sha256: self.signed_genesis_sha256,
                max_bytes: self.signed_genesis_max_bytes,
            },
            self.network_id,
            &self.block_store,
            &self.merge_log,
            reader,
        )?;
        retained.finish_reply(|identity| {
            ensure!(
                identity.network_id == self.network_id,
                "stopped-tip network changed before reply"
            );
            StoppedTipReply::new(self.invocation_id, self.reply_max_bytes, identity)?.write(writer)
        })?;
        Ok(())
    }
}

// Fixed literals, two 64-byte lowercase digests and two positive bounded u64 values only.
// Network identity is independently selected and checked above; no path, key or row is echoed.
struct StoppedTipReply {
    bytes: [u8; MAX_REPLY_BYTES],
    len: usize,
}
impl StoppedTipReply {
    fn new(invocation: [u8; 32], maximum: u64, identity: StoppedTipIdentity) -> Result<Self> {
        ensure!(
            (1..=MAX_REPLY_BYTES as u64).contains(&maximum)
                && (1..=MAX_GENESIS_BYTES).contains(&identity.genesis.byte_length)
                && (1..=1_000_000).contains(&identity.committed_height),
            "invalid stopped-tip reply bounds"
        );
        let mut invocation_hex = [0u8; 64];
        let mut genesis_hex = [0u8; 64];
        hex::encode_to_slice(invocation, &mut invocation_hex)?;
        hex::encode_to_slice(identity.genesis.raw_sha256, &mut genesis_hex)?;
        let mut bytes = [0u8; MAX_REPLY_BYTES];
        let mut remaining = &mut bytes[..];
        writeln!(
            remaining,
            "{{\"version\":1,\"operation\":\"stopped_tip\",\"invocation_id\":\"{}\",\"genesis_sha256\":\"{}\",\"genesis_bytes\":{},\"committed_height\":{}}}",
            std::str::from_utf8(&invocation_hex)?,
            std::str::from_utf8(&genesis_hex)?,
            identity.genesis.byte_length,
            identity.committed_height
        )?;
        let len = MAX_REPLY_BYTES - remaining.len();
        ensure!(
            u64::try_from(len)? <= maximum,
            "stopped-tip reply exceeds reservation"
        );
        Ok(Self { bytes, len })
    }

    fn write<W: Write>(self, writer: &mut BufWriter<W>) -> Outcome {
        writer.write_all(&self.bytes[..self.len])?;
        writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
