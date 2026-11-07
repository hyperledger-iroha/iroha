//! Offline Global finality checkpoint export from an independently pinned genesis.
//!
//! A copied journal supplies carriers, never authority. The native portable verifier
//! authenticates the complete contiguous prefix, retaining three decisions. A historical
//! checkpoint and its next-height schedule do not establish live network freshness.

use super::*;
use iroha_data_model::{
    NetworkId,
    block::SignedBlock,
    sumeragi::epoch::ValidatorEpochContextV1,
    sumeragi_finality::{
        FinalityValidator, MAX_FINALITY_BLOCK_BYTES, ScheduledConfig, ScheduledSlot,
        SumeragiFinalityProof, SumeragiFinalityVerifier, authenticated_genesis,
    },
};
use iroha_model_base::{chain::ChainId, peer::PeerId};

const MAX_HEIGHT: u64 = 1_000_000;
const MAX_READ_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const MAX_REPORT_BYTES: usize = 4 * 1024 * 1024;
const CHECKPOINT_FILE: &str = "checkpoint.nrt";
const ROSTER_FILE: &str = "next-roster.json";
const REPORT_FILE: &str = "report.json";

/// Explicit trust, work bounds and destination for one offline verification.
#[derive(Debug, ClapArgs, Clone)]
pub(super) struct Args {
    /// Independently selected chain label, not a value discovered in the journal.
    #[clap(long)]
    chain_id: ChainId,
    /// Independently authenticated signed-genesis network identity.
    #[clap(long)]
    network_id: NetworkId,
    /// Verify every height from genesis through this exact height (2..=1000000).
    #[clap(short = 'H', long)]
    height: u64,
    /// Explicit cumulative wire-byte budget, at most 64 GiB; each frame is <=32 MiB.
    #[clap(long)]
    max_read_bytes: u64,
    /// New child directory outside the store; its existing parent must be owner-private.
    #[clap(long)]
    output_dir: PathBuf,
}

struct Export {
    checkpoint: Vec<u8>,
    roster: Vec<u8>,
    report: Vec<u8>,
}

impl Args {
    fn validate(&self) -> Outcome {
        if !(2..=MAX_HEIGHT).contains(&self.height) {
            return Err(eyre!("checkpoint height must be in 2..={MAX_HEIGHT}"));
        }
        if !(1..=MAX_READ_BYTES).contains(&self.max_read_bytes) {
            return Err(eyre!("max-read-bytes must be in 1..={MAX_READ_BYTES}"));
        }
        let chain: &str = self.chain_id.as_ref();
        if chain.is_empty() || chain.len() > 1024 || chain.chars().any(char::is_control) {
            return Err(eyre!("chain-id must be a bounded canonical chain label"));
        }
        Ok(())
    }
}

pub(super) fn run(writer: &mut dyn Write, block_store_path: &Path, options: &Args) -> Outcome {
    options.validate()?;
    let directory = resolve_block_store_dir(block_store_path)?;
    let output = crate::atomic_output::resolve_outside_directory(
        &directory,
        &options.output_dir,
        "block store",
    )?;
    if output.try_exists()? {
        return Err(eyre!("checkpoint output directory must be new"));
    }
    let parent =
        iroha_fs::OwnerDirectory::open(output.parent().expect("resolved output has a parent"))
            .wrap_err("open owner-private checkpoint output parent")?;
    let export = verify_prefix(&directory, options)?;
    let published = parent
        .publish_private_child(
            output.file_name().expect("resolved output has a name"),
            &[
                (CHECKPOINT_FILE, export.checkpoint.as_slice()),
                (ROSTER_FILE, export.roster.as_slice()),
                (REPORT_FILE, export.report.as_slice()),
            ],
        )
        .wrap_err("publish checkpoint bundle; reconcile exact destination before retrying")?;
    writeln!(writer, "checkpoint_bundle: {}", published.path().display())?;
    Ok(())
}

fn committee(epoch: &ValidatorEpochContextV1) -> Vec<FinalityValidator> {
    epoch
        .committee
        .iter()
        .map(|member| FinalityValidator {
            public_key: member.validator.public_key().clone(),
            proof_of_possession: member.proof_of_possession.clone(),
        })
        .collect()
}

fn read_block(
    store: &mut BlockStore,
    height: u64,
    read_bytes: &mut u64,
    max_read_bytes: u64,
) -> color_eyre::Result<(SignedBlock, Vec<u8>)> {
    let mut index = [BlockIndex {
        start: 0,
        length: 0,
    }];
    store.read_block_indices(height - 1, &mut index)?;
    let index = index[0];
    if index.length == 0 || index.length > MAX_FINALITY_BLOCK_BYTES as u64 {
        return Err(eyre!(
            "block {height} exceeds portable finality frame bounds"
        ));
    }
    let next_bytes = read_bytes
        .checked_add(index.length)
        .filter(|total| *total <= max_read_bytes)
        .ok_or_else(|| eyre!("checkpoint exceeds explicit cumulative wire-byte budget"))?;
    let length = usize::try_from(index.length).wrap_err("block length does not fit usize")?;
    let mut wire = Vec::new();
    wire.try_reserve_exact(length)
        .wrap_err("reserve bounded block frame")?;
    wire.resize(length, 0);
    store.read_block_data(index.start, &mut wire)?;
    *read_bytes = next_bytes;
    let block = norito::core::classify_decode_attempt(|| {
        norito::core::with_decode_limits_scope(norito::canonical_decode_limits(wire.len()), || {
            decode_framed_signed_block(&wire).map_err(norito::core::DecodeAttemptError::into_error)
        })
    })
    .wrap_err_with(|| format!("decode canonical block at {height}"))?;
    norito::core::reset_decode_state();
    if block.header().height().get() != height {
        return Err(eyre!("canonical block height differs from index {height}"));
    }
    Ok((block, wire))
}

fn next_schedule(
    verified: &iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock,
) -> color_eyre::Result<ScheduledConfig> {
    match &verified.commitment().schedule.next {
        ScheduledSlot::Ready(next) if next.height == verified.height() + 1 => Ok(next.clone()),
        _ => Err(eyre!(
            "verified tip has no authenticated immediate successor committee"
        )),
    }
}

fn verify_prefix(directory: &Path, options: &Args) -> color_eyre::Result<Export> {
    options.validate()?;
    let mut store =
        BlockStore::open_read_only(directory).wrap_err("open copied Kura journals read-only")?;
    let count = store.read_index_count()?;
    if count < options.height {
        return Err(eyre!("checkpoint height exceeds retained journal"));
    }
    let mut read_bytes = 0;
    let (genesis, wire) = read_block(&mut store, 1, &mut read_bytes, options.max_read_bytes)?;
    if NetworkId::from_genesis_hash(genesis.hash()) != options.network_id {
        return Err(eyre!(
            "signed genesis differs from independently pinned network-id"
        ));
    }
    let (epoch, _) = authenticated_genesis(&genesis)?.into_parts();
    let initial_committee = committee(&epoch);
    let mut verifier = SumeragiFinalityVerifier::new(
        &genesis,
        options.chain_id.as_ref(),
        initial_committee.clone(),
    )?;
    let mut proof = SumeragiFinalityProof {
        block_header: genesis.header(),
        block_wire: wire,
        committee: initial_committee,
    };
    let verified_genesis = verifier.verify(&proof)?;
    let mut parent_core_hash = verified_genesis.core_hash();
    let mut parent_result_hash = verified_genesis.result();
    let mut next = next_schedule(&verified_genesis)?;
    drop(genesis);
    for height in 2..=options.height {
        let (block, wire) =
            read_block(&mut store, height, &mut read_bytes, options.max_read_bytes)?;
        proof = SumeragiFinalityProof {
            block_header: block.header(),
            block_wire: wire,
            committee: committee(&next.epoch),
        };
        drop(block);
        let verified = verifier
            .verify(&proof)
            .wrap_err_with(|| format!("authenticate contiguous native finality at {height}"))?;
        verified.verify_global_scope(options.network_id, options.chain_id.as_ref())?;
        parent_core_hash = verified.core_hash();
        parent_result_hash = verified.result();
        next = next_schedule(&verified)?;
        verifier.retain_recent_decisions(3);
    }
    let checkpoint = verifier.export_checkpoint(&proof)?;
    let checkpoint_bytes = checkpoint.encode_canonical()?;
    // Preserve native epoch order. Peer/service labels and QC signer subsets are not a roster.
    let roster: Vec<PeerId> = next
        .epoch
        .committee
        .iter()
        .map(|member| member.validator.clone())
        .collect();
    let mut roster_bytes = norito::json::to_json_bounded(&roster, MAX_REPORT_BYTES)?.into_bytes();
    roster_bytes.push(b'\n');
    let report = norito::json!({
        "schema": "iroha.kura.authenticated-checkpoint.v1",
        "network_id": (options.network_id.to_string()),
        "chain_id": (options.chain_id.to_string()),
        "verified_prefix_start": 1_u64,
        "verified_prefix_end": (options.height),
        "block_hash": (checkpoint.block_hash().to_string()),
        "parent_core_hash_hex": (hex::encode(parent_core_hash.0)),
        "parent_result_hash_hex": (hex::encode(parent_result_hash.0)),
        "checkpoint_blake3_hex": (blake3::hash(&checkpoint_bytes).to_hex().to_string()),
        "next_roster_blake3_hex": (blake3::hash(&roster_bytes).to_hex().to_string()),
        "wire_bytes_read": read_bytes,
        "max_read_bytes": (options.max_read_bytes),
        "independently_pinned_genesis": true,
        "global_scope_verified": true,
        "live_freshness_established": false,
        "next_height_configuration": next
    });
    let mut report_bytes = norito::json::to_json_bounded(&report, MAX_REPORT_BYTES)?.into_bytes();
    report_bytes.push(b'\n');
    Ok(Export {
        checkpoint: checkpoint_bytes,
        roster: roster_bytes,
        report: report_bytes,
    })
}

#[cfg(test)]
mod tests;
