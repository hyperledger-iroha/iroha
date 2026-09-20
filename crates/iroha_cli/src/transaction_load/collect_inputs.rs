//! Collect existing canonical SDK proof/query vectors against an entire stopped Kura store.
//!
//! The launcher owns successful validator termination, the surviving query endpoint and the
//! original genesis context. This command authenticates every carrier and reconciles every merge
//! leaf. It preserves committed rejections. Kagami owns launch-authority and workload qualification.

use std::{io::Write, mem::size_of, num::NonZeroU64, path::PathBuf};

use eyre::{Result, WrapErr, ensure, eyre};
use iroha::{client::Client, config::Config};
use iroha_core::{
    kura::{
        CanonicalKuraEvidenceComplete, CanonicalKuraEvidenceError, CanonicalKuraEvidenceLimits,
        CanonicalKuraEvidenceReader, CanonicalKuraMergeRequest,
    },
    merge::{merge_application_header_from_carrier, merge_execution_batch_commitments_match},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::{
        SignedBlock,
        consensus_v2::{HeightContext, MergeCarrierCommitmentV1},
        decode_versioned_signed_block,
    },
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    merge::{MAX_MERGE_LEDGER_ENTRY_BYTES, MergeLaneExecution, MergeLedgerEntry},
    query::CommittedTransaction,
    transaction::signed::TransactionEntrypoint,
};

use super::output::canonical_inputs::{
    CanonicalInputCaps, CanonicalInputPair, CanonicalInputsIdentity, OriginalInputBinding,
    RetainedOriginalInput,
};
use crate::{Run, RunContext};
use iroha_model_base::topology::{DataSpaceId, LaneId};

mod lease;

const MIB: usize = 1024 * 1024;
const MAX_HEIGHTS: u64 = 1_000_000;
const MAX_LEAVES: usize = 1_000_000;
const MAX_FRAME_BYTES: usize = 256 * MIB;
const MAX_DECODE_BYTES: usize = 512 * MIB;
const MAX_REPLY_BYTES: usize = 4096;
// Admission planning for dependency work, separate from the hard Norito/transport/frame limits.
// This is not an allocator or RSS cap: dependency hash, crypto and collection helpers own their
// internal allocations. The actual scaling resource gate still measures process memory.
const MAX_DEPENDENCY_WORK_RESERVATION: usize = 2 * 1024 * MIB;

/// Collect finality and committed-query inputs from a stopped validator and a surviving peer.
#[derive(clap::Args, Clone, Debug)]
pub struct Args {
    /// Unique original native collection invocation, exactly 64 lowercase hex characters.
    #[arg(long)]
    invocation_id: String,
    /// Original generator network identity, independent of fetched proof contents.
    #[arg(long)]
    network_id: NetworkId,
    /// Raw SHA-256 of the original inherited peer client configuration.
    #[arg(long)]
    client_config_sha256: String,
    /// Maximum original inherited client bytes, at most one MiB.
    #[arg(long)]
    client_config_max_bytes: u64,
    /// Original absolute host monotonic nanosecond deadline; never refreshed by this command.
    #[arg(long)]
    deadline_monotonic_ns: u64,
    /// Original stopped validator's primary block directory; absolute and normalized.
    #[arg(long)]
    block_store: PathBuf,
    /// Original stopped validator's complete merge log; absolute and normalized.
    #[arg(long)]
    merge_log: PathBuf,
    /// Original canonical genesis HeightContext, supplied independently of fetched proofs.
    #[arg(long)]
    context: PathBuf,
    /// Independently pinned lowercase raw SHA-256 of the original context file.
    #[arg(long)]
    context_sha256: String,
    /// Reserved maximum original context bytes, at most 8 MiB.
    #[arg(long)]
    context_max_bytes: u64,
    /// New canonical Vec<BridgeFinalityProof> destination.
    #[arg(long)]
    finality_out: PathBuf,
    /// New canonical Vec<CommittedTransaction> destination, including any committed rejection.
    #[arg(long)]
    queries_out: PathBuf,
    /// Reserved maximum canonical finality-vector bytes.
    #[arg(long)]
    finality_max_bytes: u64,
    /// Reserved maximum canonical committed-query-vector bytes.
    #[arg(long)]
    queries_max_bytes: u64,
    /// Original context, client config and both output reservations, at most 256 MiB.
    #[arg(long)]
    total_max_bytes: u64,
    /// Maximum complete JSON reply bytes, including the newline, at most 4096.
    #[arg(long)]
    reply_max_bytes: usize,
    /// Independent height, count and allocation limits for the complete collection.
    #[command(flatten)]
    limits: Limits,
}

#[derive(clap::Args, Clone, Debug)]
struct Limits {
    /// Expected final height; a prefix of the actual committed marker is rejected.
    #[arg(long)]
    last_height: u64,
    /// Maximum complete committed store height, at most one million.
    #[arg(long)]
    max_committed_blocks: u64,
    /// Maximum original blocks.data bytes, at most 2 GiB.
    #[arg(long)]
    max_store_data_bytes: u64,
    /// Maximum canonical carrier bytes, at most 32 MiB.
    #[arg(long)]
    max_carrier_bytes: usize,
    /// Maximum complete merge-log bytes, at most 256 MiB.
    #[arg(long)]
    max_merge_log_bytes: u64,
    /// Maximum complete merge-log frames, including empty execution epochs.
    #[arg(long)]
    max_merge_frames: u64,
    /// Maximum cumulative carrier and canonical merge-entry bytes returned by Core.
    #[arg(long)]
    max_input_bytes: u64,
    /// Maximum ordinary plus merge leaves across the complete history.
    #[arg(long)]
    max_total_leaves: usize,
    /// Maximum ordinary plus merge leaves in one carrier.
    #[arg(long)]
    max_leaves_per_carrier: usize,
    /// Cumulative Norito-owned allocations and explicitly charged collector vector slots.
    #[arg(long)]
    max_decode_bytes: usize,
    /// Per-value Norito allocation limit, also charged to the cumulative limit.
    #[arg(long)]
    max_value_decode_bytes: usize,
}

impl Limits {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.last_height > 0
                && self.last_height <= self.max_committed_blocks
                && self.max_committed_blocks <= MAX_HEIGHTS,
            "invalid complete-store height limits"
        );
        ensure!(
            (1..=2 * 1024 * 1024 * 1024).contains(&self.max_store_data_bytes)
                && (1..=32 * MIB).contains(&self.max_carrier_bytes)
                && self.max_merge_log_bytes <= MAX_FRAME_BYTES as u64
                && self.max_merge_frames <= self.max_committed_blocks
                && (1..=MAX_FRAME_BYTES as u64).contains(&self.max_input_bytes),
            "invalid carrier or merge byte limits"
        );
        ensure!(
            (1..=MAX_LEAVES).contains(&self.max_total_leaves)
                && (1..=self.max_total_leaves).contains(&self.max_leaves_per_carrier)
                && (1..=MAX_DECODE_BYTES).contains(&self.max_decode_bytes)
                && (1..=self.max_decode_bytes).contains(&self.max_value_decode_bytes),
            "invalid leaf or decode allocation limits"
        );
        ensure!(
            dependency_work_reservation(self)? <= MAX_DEPENDENCY_WORK_RESERVATION,
            "dependency working-memory planning reservation exceeds 2 GiB"
        );
        Ok(())
    }

    fn reader(&self, owner_uid: u32) -> CanonicalKuraEvidenceLimits {
        CanonicalKuraEvidenceLimits {
            first_height: 1,
            last_height: self.last_height,
            max_committed_blocks: self.max_committed_blocks,
            max_store_data_bytes: self.max_store_data_bytes,
            max_carrier_bytes: self.max_carrier_bytes,
            max_merge_log_bytes: self.max_merge_log_bytes,
            max_merge_frames: self.max_merge_frames,
            max_output_bytes: self.max_input_bytes,
            max_decode_allocation_bytes: self.max_value_decode_bytes,
            owner_uid,
        }
    }

    fn decode(&self, allocation: usize) -> norito::DecodeLimits {
        // The protocol's large byte vectors are not transaction counts. Bound both dimensions
        // independently; leaf admission below checks the concrete decoded transcript counts.
        norito::DecodeLimits::new(64 * MIB, 64 * MIB, self.max_decode_bytes, allocation, 64)
    }
}

fn dependency_work_reservation(limits: &Limits) -> Result<usize> {
    let term = |count: usize, bytes: usize| {
        count
            .checked_mul(bytes)
            .ok_or_else(|| eyre!("dependency planning reservation overflow"))
    };
    let tree_leaves = limits
        .max_leaves_per_carrier
        .checked_next_power_of_two()
        .ok_or_else(|| eyre!("Merkle planning reservation overflow"))?;
    let frame = limits
        .max_carrier_bytes
        .max(MAX_MERGE_LEDGER_ENTRY_BYTES)
        .max(8 * MIB);
    // Transport allows body reallocation overlap; Core's index/hash images use 16+32 bytes per
    // admitted height. 512 bytes per requested height conservatively plans its two internal maps.
    // The remaining coefficients plan verifier/context clones, helper encoding, hashes and trees;
    // they are deliberately not presented as a portable theorem about std or crypto allocators.
    let terms = [
        2 * 64 * MIB + 2 * 16 * 1024 + 256 * 1024,
        term(usize::try_from(limits.max_committed_blocks)?, 48)?,
        term(usize::try_from(limits.last_height)?, 512)?,
        term(limits.max_value_decode_bytes, 4)?,
        term(frame, 4)?,
        term(tree_leaves, 16 * size_of::<Option<Hash>>())?,
        term(limits.max_leaves_per_carrier, 16 * size_of::<Hash>())?,
    ];
    terms.into_iter().try_fold(0_usize, |sum, bytes| {
        sum.checked_add(bytes)
            .ok_or_else(|| eyre!("dependency planning reservation overflow"))
    })
}

fn raw_sha256(value: &str) -> Result<[u8; 32]> {
    ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "context digest must be exactly 64 lowercase hexadecimal characters"
    );
    let mut digest = [0_u8; 32];
    hex::decode_to_slice(value, &mut digest)?;
    Ok(digest)
}

impl Args {
    fn validate(&self) -> Result<[u8; 32]> {
        self.limits.validate()?;
        raw_sha256(&self.invocation_id)?;
        ensure!(
            self.invocation_id != "0".repeat(64),
            "zero collection invocation"
        );
        raw_sha256(&self.client_config_sha256)?;
        ensure!(
            (1..=lease::MAX_CONFIG_BYTES).contains(&self.client_config_max_bytes)
                && self.deadline_monotonic_ns > 0,
            "invalid original client or deadline admission"
        );
        let digest = raw_sha256(&self.context_sha256)?;
        ensure!(
            (1..=8 * MIB as u64).contains(&self.context_max_bytes)
                && (1..=MAX_FRAME_BYTES as u64).contains(&self.finality_max_bytes)
                && (1..=MAX_FRAME_BYTES as u64).contains(&self.queries_max_bytes)
                && (1..=MAX_FRAME_BYTES as u64).contains(&self.total_max_bytes)
                && (1..=MAX_REPLY_BYTES).contains(&self.reply_max_bytes),
            "invalid context, transport or reply allocation"
        );
        let total = self
            .context_max_bytes
            .checked_add(self.client_config_max_bytes)
            .and_then(|n| n.checked_add(self.finality_max_bytes))
            .and_then(|n| n.checked_add(self.queries_max_bytes))
            .ok_or_else(|| eyre!("transport reservation overflow"))?;
        ensure!(
            total <= self.total_max_bytes,
            "transport reservations exceed aggregate cap"
        );
        ensure!(
            tokio::runtime::Handle::try_current().is_err(),
            "synchronous evidence collection cannot run inside a Tokio runtime"
        );
        Ok(digest)
    }

    /// Run the sole fixed command route from the original explicitly inherited client.
    pub(crate) fn run_with_inherited(
        self,
        fd: u32,
        source: &std::path::Path,
        writer: &mut impl Write,
    ) -> Result<()> {
        (|| {
            self.validate()?;
            let (original, config) = lease::OriginalClient::admit(&self, fd, source)?;
            let _chain = iroha_data_model::account::address::ChainDiscriminantGuard::enter(
                config.account_chain_discriminant,
            );
            let client = original.client(&config)?;
            self.run_with_client(&config, &client, writer, &|| original.check())
        })()
        .map_err(|_| eyre!("scaling canonical input collection failed"))
    }

    fn run_with_client(
        self,
        config: &Config,
        client: &Client,
        writer: &mut impl Write,
        verify: &impl Fn() -> Result<()>,
    ) -> Result<()> {
        verify()?;
        let digest = self.validate()?;
        // All nested SDK/Core decoder scopes debit this one cumulative operation budget.
        norito::with_decode_limits_scope(self.limits.decode(self.limits.max_decode_bytes), || {
            let original = RetainedOriginalInput::open(OriginalInputBinding {
                path: self.context.clone(),
                raw_sha256: digest,
                max_bytes: self.context_max_bytes,
            })?;
            verify()?;
            let anchor = original.with_bytes(|bytes| {
                let anchor: HeightContext = norito::decode_canonical_with_limits(
                    bytes,
                    self.limits.decode(self.limits.max_value_decode_bytes),
                )?;
                anchor.validate()?;
                ensure!(
                    anchor.height == 1
                        && anchor.network_id == config.network_id
                        && anchor.snapshot_bootstrap.is_none()
                        && anchor.parent_commit_qc.is_none(),
                    "original context is not this network's genesis anchor"
                );
                Ok(anchor.id())
            })?;
            verify()?;
            let outputs = CanonicalInputPair::admit(
                &self.finality_out,
                &self.queries_out,
                CanonicalInputCaps {
                    finality_bytes: self.finality_max_bytes,
                    query_bytes: self.queries_max_bytes,
                    total_bytes: self.total_max_bytes,
                },
            )?;
            verify()?;
            let reader = CanonicalKuraEvidenceReader::open(
                &self.block_store,
                &self.merge_log,
                self.limits.reader(owner_uid()?),
            )?;
            let mut verifier = BridgeFinalityVerifier::with_context(config.network_id, anchor);
            let collected = collect(
                reader,
                client,
                config.network_id,
                &mut verifier,
                &self.limits,
                verify,
            )?;
            verify()?;
            drop(verifier);
            let finality_len = norito::canonical_frame_len(&collected.finality)?;
            let query_len = norito::canonical_frame_len(&collected.queries)?;
            ensure!(
                finality_len as u64 <= self.finality_max_bytes
                    && query_len as u64 <= self.queries_max_bytes,
                "actual canonical transport vector exceeds its allocation"
            );
            let proofs = collected.finality.len();
            let queries = collected.queries.len();
            let published = outputs.publish(
                original,
                collected.complete,
                &collected.finality,
                &collected.queries,
                verify,
            )?;
            verify()?;
            let identity = published.identity()?;
            write_reply(
                writer,
                self.reply_max_bytes,
                self.limits.last_height,
                proofs,
                queries,
                identity,
                &self.invocation_id,
                raw_sha256(&self.client_config_sha256)?,
                verify,
            )?;
            verify()?;
            // Hold all originals and both published files through the real writer's flush.
            ensure!(
                published.identity()? == identity,
                "canonical inputs changed during reply publication"
            );
            verify()?;
            Ok(())
        })
    }
}

impl Run for Args {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        self.validate()?;
        let _ = context.client_from_config()?;
        // Only main's mandatory inherited-config dispatch may execute this command.
        Err(eyre!(
            "scaling collection requires its original inherited client dispatch"
        ))
    }
}

fn owner_uid() -> Result<u32> {
    #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
    {
        Ok(rustix::process::geteuid().as_raw())
    }
    #[cfg(not(all(unix, not(any(target_os = "redox", target_os = "espidf")))))]
    {
        Err(eyre!(
            "canonical evidence collection requires secure Unix filesystem operations"
        ))
    }
}

struct Collected {
    complete: CanonicalKuraEvidenceComplete,
    finality: Vec<BridgeFinalityProof>,
    queries: Vec<CommittedTransaction>,
}

fn bounded_vec<T>(maximum: usize) -> Result<Vec<T>> {
    let bytes = maximum
        .checked_mul(size_of::<T>())
        .ok_or_else(|| eyre!("vector allocation overflow"))?;
    norito::core::reserve_decode_allocation(bytes)?;
    let mut values = Vec::new();
    values.try_reserve_exact(maximum)?;
    Ok(values)
}

fn reserve_leaves<T>(values: &mut Vec<T>, additional: usize, maximum: usize) -> Result<()> {
    let needed = values
        .len()
        .checked_add(additional)
        .ok_or_else(|| eyre!("leaf allocation overflow"))?;
    ensure!(needed <= maximum, "leaf allocation exceeds count limit");
    if needed > values.capacity() {
        let target = needed.max(values.capacity().saturating_mul(2).min(maximum));
        // Charge the entire replacement allocation before reallocation, including the temporary
        // overlap with the old vector. The cumulative budget deliberately never refunds work.
        let bytes = target
            .checked_mul(size_of::<T>())
            .ok_or_else(|| eyre!("leaf slot size overflow"))?;
        norito::core::reserve_decode_allocation(bytes)?;
        values.try_reserve_exact(target - values.len())?;
    }
    Ok(())
}

fn collect(
    mut reader: CanonicalKuraEvidenceReader,
    client: &Client,
    network: NetworkId,
    verifier: &mut BridgeFinalityVerifier,
    limits: &Limits,
    verify: &impl Fn() -> Result<()>,
) -> Result<Collected> {
    verify()?;
    let heights = usize::try_from(limits.last_height)?;
    let mut carriers = bounded_vec::<SignedBlock>(heights)?;
    let mut finality = bounded_vec::<BridgeFinalityProof>(heights)?;
    let mut requests = bounded_vec::<CanonicalKuraMergeRequest>(heights)?;
    let mut queries = Vec::<CommittedTransaction>::new();
    let mut hashes = Vec::<HashOf<TransactionEntrypoint>>::new();
    for height in 1..=limits.last_height {
        verify()?;
        let wire = reader.read_carrier(height)?;
        let block =
            norito::with_decode_limits_scope(limits.decode(limits.max_value_decode_bytes), || {
                decode_versioned_signed_block(&wire)
            })?;
        let proof =
            norito::with_decode_limits_scope(limits.decode(limits.max_value_decode_bytes), || {
                verify()?;
                client.get_bridge_finality_proof(
                    NonZeroU64::new(height).ok_or_else(|| eyre!("zero height"))?,
                    block.hash(),
                    verifier,
                )
            })?;
        verify()?;
        proof
            .finality_artifact
            .validate_for_header(&block.header())?;
        let execution = &proof.finality_artifact.commit_qc.execution_commitment;
        ensure!(
            block.header() == proof.block_header
                && Hash::new(&wire) == execution.executed_block_wire_hash
                && u64::try_from(wire.len())? == execution.executed_block_wire_len,
            "finality does not bind the exact executed Kura wire"
        );
        let ordinary = block.entrypoint_hashes().len();
        ensure!(
            ordinary <= limits.max_leaves_per_carrier
                && ordinary <= limits.max_total_leaves - hashes.len(),
            "ordinary leaf count exceeds allocation"
        );
        reserve_leaves(&mut hashes, ordinary, limits.max_total_leaves)?;
        hashes.extend(block.entrypoint_hashes());
        let reference = block
            .execution_context()
            .and_then(|c| c.merge_entry.as_ref());
        ensure!(
            reference.is_some() == execution.merge_carrier.is_some(),
            "missing or extra merge commitment"
        );
        if let Some(reference) = reference {
            ensure!(
                requests.len() < usize::try_from(limits.max_merge_frames)?,
                "merge request count exceeds allocation"
            );
            // Decode a bounded canonical copy instead of an unmetered recursive clone.
            let size = norito::canonical_frame_len(reference)?;
            ensure!(
                size <= limits.max_carrier_bytes,
                "compact reference exceeds carrier allocation"
            );
            norito::core::reserve_decode_allocation(size)?;
            let _flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            let bytes = norito::core::to_bytes_bounded(reference, size)?;
            let reference = norito::decode_canonical_with_limits(
                &bytes,
                limits.decode(limits.max_value_decode_bytes),
            )?;
            requests.push(CanonicalKuraMergeRequest {
                carrier_height: height,
                reference,
            });
        }
        carriers.push(block);
        finality.push(proof);
    }
    verify()?;
    let mut callback_error = None;
    let scan = reader.scan_merge_entries(&requests, |height, entry, _bytes| {
        let result = (|| {
            verify()?;
            let index = usize::try_from(
                height
                    .checked_sub(1)
                    .ok_or_else(|| eyre!("zero merge height"))?,
            )?;
            let block = carriers
                .get(index)
                .ok_or_else(|| eyre!("merge carrier out of range"))?;
            let proof = finality
                .get(index)
                .ok_or_else(|| eyre!("merge finality out of range"))?;
            collect_merge(
                client,
                network,
                block,
                proof,
                entry,
                limits,
                &mut hashes,
                &mut queries,
                verify,
            )
        })();
        if let Err(error) = result {
            callback_error = Some(error);
            return Err(CanonicalKuraEvidenceError::Invalid(
                "SDK merge collection callback failed",
            ));
        }
        Ok(())
    });
    if let Some(error) = callback_error {
        return Err(error.wrap_err("complete merge collection failed"));
    }
    scan?;
    verify()?;
    hashes.sort_unstable();
    ensure!(
        hashes.windows(2).all(|pair| pair[0] != pair[1]),
        "duplicate entrypoint in complete canonical history"
    );
    verify()?;
    let complete = reader.finish()?;
    verify()?;
    ensure!(
        complete.committed_height() == limits.last_height
            && complete.carrier_count() == limits.last_height,
        "collection ended before the actual committed tip"
    );
    Ok(Collected {
        complete,
        finality,
        queries,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "one exact carrier join shares the admitted collection budgets"
)]
fn collect_merge(
    client: &Client,
    network: NetworkId,
    block: &SignedBlock,
    proof: &BridgeFinalityProof,
    entry: &MergeLedgerEntry,
    limits: &Limits,
    hashes: &mut Vec<HashOf<TransactionEntrypoint>>,
    queries: &mut Vec<CommittedTransaction>,
    verify: &impl Fn() -> Result<()>,
) -> Result<()> {
    verify()?;
    ensure!(
        norito::canonical_frame_len(entry)? <= MAX_MERGE_LEDGER_ENTRY_BYTES,
        "full merge transcript exceeds helper frame bound"
    );
    let execution = &proof.finality_artifact.commit_qc.execution_commitment;
    ensure!(
        entry.version == MergeLedgerEntry::VERSION
            && execution.merge_carrier
                == Some(MergeCarrierCommitmentV1::new(entry.canonical_hash())),
        "full merge entry is not committed by carrier QC"
    );
    let qc = &entry.merge_qc;
    ensure!(
        qc.network_id == network
            && qc.carrier_height == block.header().height().get()
            && Some(qc.carrier_parent_hash) == block.header().prev_block_hash()
            && qc.view == block.header().view_change_index()
            && qc.epoch_id == entry.epoch_id,
        "merge certificate carrier identity mismatch"
    );
    let Some(batch) = entry.execution_batch.as_ref() else {
        return Ok(());
    };
    ensure!(
        norito::canonical_frame_len(batch)? <= MAX_MERGE_LEDGER_ENTRY_BYTES,
        "execution batch exceeds helper frame bound"
    );
    ensure!(
        batch.version == 1
            && !batch.lanes.is_empty()
            && batch.lanes.len() <= limits.max_leaves_per_carrier
            && batch
                .lanes
                .windows(2)
                .all(|pair| lane_order(&pair[0]) < lane_order(&pair[1])),
        "invalid execution batch or noncanonical lane order"
    );
    let mut leaves = 0_usize;
    for lane in &batch.lanes {
        ensure!(
            norito::canonical_frame_len(lane)? <= MAX_MERGE_LEDGER_ENTRY_BYTES,
            "lane transcript exceeds helper frame bound"
        );
        let count = lane.entrypoints.len();
        ensure!(
            count > 0
                && count == lane.results.len()
                && count == lane.entrypoint_hashes.len()
                && count == lane.result_hashes.len(),
            "unaligned full merge entrypoint/result vectors"
        );
        leaves = leaves
            .checked_add(count)
            .ok_or_else(|| eyre!("merge leaf count overflow"))?;
        ensure!(
            leaves
                <= limits
                    .max_leaves_per_carrier
                    .saturating_sub(block.entrypoint_hashes().len())
                && leaves <= limits.max_total_leaves - hashes.len(),
            "merge leaf count exceeds allocation"
        );
        for (index, (entrypoint, result)) in lane.entrypoints.iter().zip(&lane.results).enumerate()
        {
            ensure!(
                Hash::from(entrypoint.hash()) == lane.entrypoint_hashes[index]
                    && Hash::from(result.hash()) == lane.result_hashes[index],
                "merge leaf hashes do not match the exact transcript"
            );
        }
    }
    ensure!(
        batch.entrypoint_count == u64::try_from(leaves)?
            && batch.application_block_header
                == merge_application_header_from_carrier(&block.header())
            && merge_execution_batch_commitments_match(batch),
        "merge execution commitments mismatch"
    );
    reserve_leaves(hashes, leaves, limits.max_total_leaves)?;
    reserve_leaves(queries, leaves, limits.max_total_leaves)?;
    for (index, (entrypoint, result)) in batch
        .lanes
        .iter()
        .flat_map(|lane| lane.entrypoints.iter().zip(&lane.results))
        .enumerate()
    {
        let hash = entrypoint.hash();
        let queried =
            norito::with_decode_limits_scope(limits.decode(limits.max_value_decode_bytes), || {
                verify()?;
                client.get_transaction_details(hash)
            })?
            .transaction;
        verify()?;
        ensure!(
            queried.merge_inclusion.is_some()
                && queried.verify_inclusion_in_block(block)
                && usize::try_from(queried.entrypoint_proof.leaf_index())? == index
                && queried.entrypoint == *entrypoint
                && queried.result == *result,
            "committed query does not match the exact merge carrier, index and result"
        );
        hashes.push(hash);
        queries.push(queried);
    }
    Ok(())
}

fn lane_order(lane: &MergeLaneExecution) -> (LaneId, DataSpaceId, Hash, u64, u64, u64, Hash, Hash) {
    let d = &lane.proposal.descriptor;
    (
        d.lane_id,
        d.dataspace_id,
        d.lane_incarnation,
        d.lane_block_height,
        d.proposal_height,
        d.lane_block_view,
        d.descriptor_hash,
        lane.proposal.proposal_hash,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "one fixed receipt joins original client, context, output and count identities"
)]
fn write_reply(
    writer: &mut impl Write,
    maximum: usize,
    height: u64,
    proofs: usize,
    queries: usize,
    identity: CanonicalInputsIdentity,
    invocation: &str,
    config_digest: [u8; 32],
    verify: &impl Fn() -> Result<()>,
) -> Result<()> {
    verify()?;
    raw_sha256(invocation)?;
    use std::fmt::Write as _;
    ensure!(
        (1..=MAX_REPLY_BYTES).contains(&maximum),
        "invalid canonical input reply cap"
    );
    // Fixed scalar schema: fixed labels, five bounded digests and six count values. No path, key,
    // proof-controlled string or transaction body enters this fixed stack buffer.
    let mut digests = [[0_u8; 64]; 4];
    for (target, source) in digests.iter_mut().zip([
        identity.context.raw_sha256,
        identity.finality.raw_sha256,
        identity.queries.raw_sha256,
        config_digest,
    ]) {
        hex::encode_to_slice(source, target)?;
    }
    let mut reply = Reply {
        bytes: [0_u8; MAX_REPLY_BYTES],
        length: 0,
        maximum,
    };
    write!(reply,
        "{{\"version\":1,\"operation\":\"collect_scaling_inputs\",\"invocation_id\":\"{invocation}\",\"client_config_sha256\":\"{}\",\"committed_height\":{height},\"finality_count\":{proofs},\"query_count\":{queries},\"context_sha256\":\"{}\",\"context_bytes\":{},\"finality_sha256\":\"{}\",\"finality_bytes\":{},\"queries_sha256\":\"{}\",\"queries_bytes\":{}}}\n",
        std::str::from_utf8(&digests[3])?,
        std::str::from_utf8(&digests[0])?,
        identity.context.byte_length,
        std::str::from_utf8(&digests[1])?,
        identity.finality.byte_length,
        std::str::from_utf8(&digests[2])?,
        identity.queries.byte_length
    ).map_err(|_| eyre!("canonical input reply exceeds allocation"))?;
    verify()?;
    writer
        .write_all(&reply.bytes[..reply.length])
        .wrap_err("write canonical input reply")?;
    verify()?;
    writer.flush().wrap_err("flush canonical input reply")?;
    verify()
}

struct Reply {
    bytes: [u8; MAX_REPLY_BYTES],
    length: usize,
    maximum: usize,
}
impl std::fmt::Write for Reply {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let end = self
            .length
            .checked_add(value.len())
            .ok_or(std::fmt::Error)?;
        if end > self.maximum {
            return Err(std::fmt::Error);
        }
        self.bytes[self.length..end].copy_from_slice(value.as_bytes());
        self.length = end;
        Ok(())
    }
}

#[cfg(all(
    test,
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
mod fixture;
#[cfg(test)]
mod tests;
