//! Collect existing canonical SDK proof/query vectors against an entire stopped Kura store.
//!
//! The launcher owns successful validator termination, the surviving query endpoint and the
//! original genesis context. This command authenticates every Native carrier, its historical
//! context witness and complete Network outputs. It preserves committed rejections. Kagami owns launch-authority and workload qualification.

use std::{io::Write, mem::size_of, num::NonZeroU64, path::PathBuf};

use eyre::{Result, WrapErr, ensure, eyre};
use iroha::{client::Client, config::Config};
use iroha_core::{
    kura::{
        CanonicalKuraEvidenceComplete, CanonicalKuraEvidenceError, CanonicalKuraEvidenceLimits,
        CanonicalKuraEvidenceReader,
    },
    state::{
        FinalizedNativeContextV1, NativeExecutionEvidenceLimits, NativeExecutionEvidenceVerifier,
        NativeLaneContextsEvidenceV1, VerifiedNativeExecutionCarrier,
    },
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::{SignedBlock, consensus_v2::HeightContext, decode_versioned_signed_block},
    bridge::BridgeFinalityVerifier,
    query::CommittedTransaction,
    transaction::signed::TransactionEntrypoint,
};

use super::output::canonical_inputs::{
    CanonicalInputCaps, CanonicalInputPair, CanonicalInputsIdentity, OriginalInputBinding,
    RetainedOriginalInput,
};
use crate::{Run, RunContext};

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
    /// Original canonical complete chronological Vec<NativeLaneContextsEvidenceV1>.
    #[arg(long)]
    native_contexts: PathBuf,
    /// Independently pinned lowercase raw SHA-256 of the Native context archive.
    #[arg(long)]
    native_contexts_sha256: String,
    /// Reserved maximum Native context archive bytes, at most 256 MiB.
    #[arg(long)]
    native_contexts_max_bytes: u64,
    /// New canonical Vec<FinalizedNativeContextV1> destination.
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
    /// Original context, Native archive, client config and both output reservations, at most 256 MiB.
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
    /// Maximum cumulative canonical carrier, finality and context-evidence bytes.
    #[arg(long)]
    max_input_bytes: u64,
    /// Maximum Network inputs across the complete history.
    #[arg(long)]
    max_total_leaves: usize,
    /// Maximum Network inputs in one carrier.
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
    let frame = limits.max_carrier_bytes.max(8 * MIB);
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
        raw_sha256(&self.native_contexts_sha256)?;
        ensure!(
            (1..=8 * MIB as u64).contains(&self.context_max_bytes)
                && (1..=MAX_FRAME_BYTES as u64).contains(&self.native_contexts_max_bytes)
                && (1..=MAX_FRAME_BYTES as u64).contains(&self.finality_max_bytes)
                && (1..=MAX_FRAME_BYTES as u64).contains(&self.queries_max_bytes)
                && (1..=MAX_FRAME_BYTES as u64).contains(&self.total_max_bytes)
                && (1..=MAX_REPLY_BYTES).contains(&self.reply_max_bytes),
            "invalid context, transport or reply allocation"
        );
        let total = self
            .context_max_bytes
            .checked_add(self.native_contexts_max_bytes)
            .and_then(|n| n.checked_add(self.client_config_max_bytes))
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
            let native_original =
                RetainedOriginalInput::open_native_contexts(OriginalInputBinding {
                    path: self.native_contexts.clone(),
                    raw_sha256: raw_sha256(&self.native_contexts_sha256)?,
                    max_bytes: self.native_contexts_max_bytes,
                })?;
            let contexts: Vec<NativeLaneContextsEvidenceV1> =
                native_original.with_bytes(|bytes| {
                    let contexts: Vec<NativeLaneContextsEvidenceV1> =
                        norito::decode_canonical_with_limits(
                            bytes,
                            self.limits.decode(self.limits.max_value_decode_bytes),
                        )?;
                    ensure!(
                        contexts.len() as u64 == self.limits.last_height,
                        "Native context archive must cover the exact complete interval"
                    );
                    Ok(contexts)
                })?;
            let verify = &|| -> Result<()> {
                verify()?;
                native_original.identity()?;
                Ok(())
            };
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
                anchor,
                &mut verifier,
                contexts,
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
    finality: Vec<FinalizedNativeContextV1>,
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

#[expect(
    clippy::too_many_arguments,
    reason = "original context and exact retained inputs remain independent"
)]
fn collect(
    mut reader: CanonicalKuraEvidenceReader,
    client: &Client,
    network: NetworkId,
    anchor: iroha_data_model::block::consensus_v2::HeightContextId,
    verifier: &mut BridgeFinalityVerifier,
    contexts: Vec<NativeLaneContextsEvidenceV1>,
    limits: &Limits,
    verify: &impl Fn() -> Result<()>,
) -> Result<Collected> {
    verify()?;
    let heights = usize::try_from(limits.last_height)?;
    ensure!(
        contexts.len() == heights,
        "incomplete Native context archive"
    );
    let mut carriers = bounded_vec::<VerifiedNativeExecutionCarrier>(heights)?;
    let mut finality = bounded_vec::<FinalizedNativeContextV1>(heights)?;
    let mut queries = Vec::<CommittedTransaction>::new();
    let mut hashes = Vec::<HashOf<TransactionEntrypoint>>::new();
    let mut native = NativeExecutionEvidenceVerifier::new(
        network,
        anchor,
        NativeExecutionEvidenceLimits {
            max_carriers: limits.last_height,
            max_carrier_bytes: limits.max_carrier_bytes as u64,
            max_proof_bytes: limits.max_input_bytes.min(MAX_FRAME_BYTES as u64),
            max_retained_bytes: limits.max_input_bytes,
        },
    )
    .map_err(|error| eyre!(error))?;
    for (index, contexts) in contexts.into_iter().enumerate() {
        verify()?;
        let height = u64::try_from(index)? + 1;
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
        let count = block.network_entrypoint_count();
        ensure!(
            count <= limits.max_leaves_per_carrier
                && count <= limits.max_total_leaves - hashes.len(),
            "Network input count exceeds allocation"
        );
        reserve_leaves(&mut hashes, count, limits.max_total_leaves)?;
        hashes.extend(block.network_entrypoints().map(TransactionEntrypoint::hash));
        let context_len = norito::canonical_frame_len(&contexts)?;
        ensure!(
            context_len <= MAX_FRAME_BYTES && context_len as u64 <= limits.max_input_bytes,
            "Native context evidence exceeds its frame allocation"
        );
        norito::core::reserve_decode_allocation(context_len)?;
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let context_bytes = norito::core::to_bytes_bounded(&contexts, context_len)?;
        let authenticated = native
            .push_height(&proof, block, &context_bytes)
            .map_err(|error| eyre!(error))?;
        verify()?;
        carriers.push(authenticated);
        finality.push(FinalizedNativeContextV1 {
            finality: proof,
            contexts,
        });
    }
    verify()?;
    reader.scan_merge_entries(&[], |_, _, _| {
        Err(CanonicalKuraEvidenceError::Invalid(
            "Native collection cannot admit executable MergeQC evidence",
        ))
    })?;
    for carrier in &carriers {
        collect_native(client, carrier.block(), limits, &mut queries, verify)?;
    }
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

fn collect_native(
    client: &Client,
    block: &SignedBlock,
    limits: &Limits,
    queries: &mut Vec<CommittedTransaction>,
    verify: &impl Fn() -> Result<()>,
) -> Result<()> {
    verify()?;
    if block
        .execution_context()
        .and_then(|context| context.native_lane_decisions.as_ref())
        .is_none()
    {
        return Ok(());
    }
    reserve_leaves(
        queries,
        block.network_entrypoint_count(),
        limits.max_total_leaves,
    )?;
    for (index, entrypoint) in block.network_entrypoints().enumerate() {
        let (output_index, _) = block
            .network_output_at(u32::try_from(index)?)
            .ok_or_else(|| eyre!("Native Network input has no complete execution output"))?;
        let queried =
            norito::with_decode_limits_scope(limits.decode(limits.max_value_decode_bytes), || {
                verify()?;
                client.get_transaction_details(entrypoint.hash())
            })?
            .transaction;
        verify()?;
        ensure!(
            queried.verify_inclusion_in_block(block)
                && usize::try_from(queried.entrypoint_proof.leaf_index())? == index
                && queried.output_proof.leaf_index() == output_index
                && queried.entrypoint == *entrypoint
                && queried.output == block.execution_outputs()[output_index as usize],
            "committed query does not match the exact Native carrier, input index and output"
        );
        queries.push(queried);
    }
    Ok(())
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
