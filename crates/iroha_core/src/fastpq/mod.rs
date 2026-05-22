//! FASTPQ-specific transcript helpers shared across the host.
pub mod lane;

use std::{
    collections::BTreeMap,
    io::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
};

use fastpq_prover::{
    Bn254PoseidonBatchSlice, OperationKind, PendingBn254PoseidonWordBatch, PoseidonSponge,
    PublicInputs, StateTransition, TransitionBatch,
    gadgets::transfer::attach_transfer_smt_witnesses, try_hash_bn254_poseidon_word_batches,
    try_submit_bn254_poseidon_word_batches,
};
#[cfg(test)]
use iroha_config::parameters::actual::FastpqExecutionMode;
use iroha_config::parameters::actual::{Fastpq, FastpqPoseidonMode};
use iroha_crypto::Hash;
use iroha_data_model::{
    DataSpaceId,
    account::AccountId,
    asset::id::AssetDefinitionId,
    block::{BlockHeader, consensus::ExecWitness},
    fastpq::{
        FastpqOperationKind, FastpqPublicInputs, FastpqRolePermissionDelta, FastpqStateTransition,
        FastpqTransitionBatch, TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript,
        TransferTranscript, TransferTranscriptBundle, normalized_numeric_to_u64,
    },
    role::{Role, RoleId},
};
use iroha_primitives::numeric::Numeric;
use iroha_zkp_halo2::poseidon as halo2_poseidon;
use norito::{codec::Encode as NoritoEncode, to_bytes};
use thiserror::Error;

const AUTHORITY_DIGEST_DOMAIN: &[u8] = b"iroha:fastpq:v1:authority|";
const TX_SET_HASH_DOMAIN: &[u8] = b"fastpq:v1:tx_set";
const PERMISSION_TABLE_NODE_DOMAIN: &[u8] = b"fastpq:v1:poseidon_node";
/// Metadata key storing the originating entry hash for a batch.
pub const ENTRY_HASH_METADATA_KEY: &str = "entry_hash";
/// Metadata key storing the transcript count embedded in a batch.
pub const TRANSCRIPT_COUNT_METADATA_KEY: &str = "transcript_count";

/// Canonical FASTPQ parameter name used across the host and CLI helpers.
pub const FASTPQ_CANONICAL_PARAMETER_SET: &str = "fastpq-lane-balanced";
const DIGEST_FINALIZE_PARALLEL_THRESHOLD: usize = 32;
const DIGEST_FINALIZE_GPU_THRESHOLD: usize = 64;
const POSEIDON_DIGEST_WORDS_PER_TRANSCRIPT_HINT: usize = 24;
static DIGEST_ACCELERATION_ENABLED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static DIGEST_ACCELERATION_TEST_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

/// Base fields for FASTPQ public inputs shared across batches in a block.
#[derive(Debug, Clone, Copy)]
pub struct FastpqPublicInputsTemplate {
    /// Data-space identifier (little-endian UUID bytes).
    pub dsid: [u8; 16],
    /// Slot timestamp (nanoseconds since epoch).
    pub slot: u64,
    /// Sparse Merkle tree root before executing the batch.
    pub old_root: [u8; 32],
    /// Sparse Merkle tree root after executing the batch.
    pub new_root: [u8; 32],
    /// Permission table commitment for this slot.
    pub perm_root: [u8; 32],
}

/// Local context needed to build FASTPQ batches outside the consensus commit path.
#[derive(Debug, Clone, Default)]
pub(crate) struct FastpqWitnessContext {
    /// Public-input fields shared by every FASTPQ batch in the witness.
    pub(crate) public_inputs: Option<FastpqPublicInputsTemplate>,
    /// Hash of external transaction entrypoints in the committed block.
    pub(crate) tx_set_hash: Option<[u8; 32]>,
    /// Per-entry dataspace ids keyed by entrypoint hash.
    pub(crate) entry_dataspaces: BTreeMap<Hash, [u8; 16]>,
}

impl FastpqPublicInputsTemplate {
    /// Build full public inputs using a precomputed transaction set hash.
    #[must_use]
    pub const fn with_tx_set_hash(self, tx_set_hash: [u8; 32]) -> FastpqPublicInputs {
        FastpqPublicInputs {
            dsid: self.dsid,
            slot: self.slot,
            old_root: self.old_root,
            new_root: self.new_root,
            perm_root: self.perm_root,
            tx_set_hash,
        }
    }
}

pub(crate) fn configure_poseidon_digest_acceleration(cfg: &Fastpq) {
    configure_poseidon_digest_acceleration_with_preflight(cfg, || {
        #[cfg(feature = "fastpq-gpu")]
        {
            fastpq_prover::preflight_bn254_poseidon_word_batches()
        }
        #[cfg(not(feature = "fastpq-gpu"))]
        {
            false
        }
    });
}

pub(crate) fn poseidon_digest_acceleration_configured(cfg: &Fastpq) -> bool {
    match cfg.poseidon_mode {
        FastpqPoseidonMode::Cpu => false,
        FastpqPoseidonMode::Gpu => true,
    }
}

pub(crate) fn set_poseidon_digest_acceleration_enabled(enabled: bool) {
    DIGEST_ACCELERATION_ENABLED.store(enabled, Ordering::Release);
}

fn configure_poseidon_digest_acceleration_with_preflight(
    cfg: &Fastpq,
    preflight: impl FnOnce() -> bool,
) -> bool {
    let enabled = poseidon_digest_acceleration_configured(cfg) && preflight();
    DIGEST_ACCELERATION_ENABLED.store(enabled, Ordering::Release);
    enabled
}

#[inline]
fn poseidon_digest_acceleration_enabled() -> bool {
    DIGEST_ACCELERATION_ENABLED.load(Ordering::Acquire)
}

/// Errors that can occur while mapping transfer transcripts into FASTPQ transition batches.
#[derive(Debug, Error)]
pub enum TranscriptBatchError {
    /// Encountered a Numeric value that cannot be normalized into FASTPQ witness units.
    #[error("numeric value `{value}` cannot be normalized into 64-bit FASTPQ witness units")]
    NumericEncoding {
        /// Numeric value that fell outside the FASTPQ prover's supported range.
        value: Numeric,
    },
    /// Norito serialization of transcript metadata failed.
    #[error("failed to encode transfer transcripts for gadget metadata")]
    MetadataEncoding {
        /// Underlying Norito error.
        #[from]
        source: norito::core::Error,
    },
    /// Transfer SMT witness materialization failed.
    #[error("failed to attach transfer SMT witnesses")]
    TransferWitness {
        /// Underlying FASTPQ prover error.
        source: fastpq_prover::Error,
    },
    /// Execution witness does not carry precomputed FASTPQ batches.
    #[error("execution witness missing fastpq batches with public inputs")]
    MissingFastpqBatches,
}

/// Compute the canonical authority digest hashed by the host.
#[must_use]
pub fn authority_digest(authority: &AccountId) -> Hash {
    let mut payload = Vec::with_capacity(AUTHORITY_DIGEST_DOMAIN.len() + 96);
    payload.extend_from_slice(AUTHORITY_DIGEST_DOMAIN);
    payload.extend_from_slice(&authority.encode());
    Hash::new(payload)
}

/// Compute the Poseidon digest of a transfer delta preimage.
#[inline(always)]
#[must_use]
pub fn poseidon_preimage_digest(delta: &TransferDeltaTranscript, batch_hash: &Hash) -> Hash {
    let mut scratch = PoseidonDigestScratch::default();
    poseidon_preimage_digest_with_scratch(delta, batch_hash, &mut scratch)
}

/// Reusable scratch space for canonical single-transfer Poseidon digests.
#[derive(Debug, Default)]
pub(crate) struct PoseidonDigestScratch {
    words: Vec<u64>,
}

/// Compute the canonical Poseidon digest using caller-owned scratch storage.
#[inline(always)]
#[must_use]
pub(crate) fn poseidon_preimage_digest_with_scratch(
    delta: &TransferDeltaTranscript,
    batch_hash: &Hash,
    scratch: &mut PoseidonDigestScratch,
) -> Hash {
    scratch.words.clear();
    scratch
        .words
        .reserve(POSEIDON_DIGEST_WORDS_PER_TRANSCRIPT_HINT);
    append_transfer_digest_words(&mut scratch.words, delta, batch_hash);
    Hash::prehashed(halo2_poseidon::hash_u64_words_bytes(&scratch.words))
}

#[inline(always)]
fn append_encoded_words<W, T>(writer: &mut W, value: &T)
where
    W: Write,
    T: NoritoEncode,
{
    value.encode_to(writer);
}

#[inline(always)]
fn u64_from_le_bytes(bytes: &[u8]) -> u64 {
    debug_assert!(bytes.len() >= 8);
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

#[inline(always)]
fn partial_u64_from_le_bytes(bytes: &[u8; 8], len: usize) -> u64 {
    debug_assert!(len <= bytes.len());
    let word = u64::from_le_bytes(*bytes);
    match len {
        0 => 0,
        8 => word,
        _ => word & ((1u64 << (len * 8)) - 1),
    }
}

#[derive(Debug)]
struct PoseidonWordPacker<'a> {
    words: &'a mut Vec<u64>,
    pending_bytes: [u8; 8],
    pending_len: usize,
}

impl<'a> PoseidonWordPacker<'a> {
    #[inline]
    fn new(words: &'a mut Vec<u64>) -> Self {
        Self {
            words,
            pending_bytes: [0; 8],
            pending_len: 0,
        }
    }

    #[inline]
    fn update(&mut self, mut bytes: &[u8]) {
        if self.pending_len > 0 {
            let needed = self.pending_bytes.len() - self.pending_len;
            let take = needed.min(bytes.len());
            self.pending_bytes[self.pending_len..self.pending_len + take]
                .copy_from_slice(&bytes[..take]);
            self.pending_len += take;
            bytes = &bytes[take..];
            if self.pending_len == self.pending_bytes.len() {
                self.words.push(u64::from_le_bytes(self.pending_bytes));
                self.pending_len = 0;
            }
        }

        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.words.push(u64_from_le_bytes(chunk));
        }

        let remainder = chunks.remainder();
        if !remainder.is_empty() {
            self.pending_bytes[..remainder.len()].copy_from_slice(remainder);
            self.pending_len = remainder.len();
        }
    }

    #[inline]
    fn finish(self) {
        if self.pending_len > 0 {
            self.words.push(partial_u64_from_le_bytes(
                &self.pending_bytes,
                self.pending_len,
            ));
        }
    }
}

impl Write for PoseidonWordPacker<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
fn transfer_digest_words(delta: &TransferDeltaTranscript, batch_hash: &Hash) -> Vec<u64> {
    let mut words = Vec::with_capacity(POSEIDON_DIGEST_WORDS_PER_TRANSCRIPT_HINT);
    append_transfer_digest_words(&mut words, delta, batch_hash);
    words
}

fn append_transfer_digest_words(
    words: &mut Vec<u64>,
    delta: &TransferDeltaTranscript,
    batch_hash: &Hash,
) {
    let mut packer = PoseidonWordPacker::new(words);
    append_encoded_words(&mut packer, &delta.from_account);
    append_encoded_words(&mut packer, &delta.to_account);
    append_encoded_words(&mut packer, &delta.asset_definition);
    append_encoded_words(&mut packer, &delta.amount);
    packer.update(batch_hash.as_ref());
    packer.finish();
}

#[derive(Debug, Default)]
struct PoseidonDigestBatch {
    words: Vec<u64>,
    slices: Vec<Bn254PoseidonBatchSlice>,
}

impl PoseidonDigestBatch {
    fn with_capacity(digest_count: usize) -> Self {
        Self {
            words: Vec::with_capacity(
                digest_count.saturating_mul(POSEIDON_DIGEST_WORDS_PER_TRANSCRIPT_HINT),
            ),
            slices: Vec::with_capacity(digest_count),
        }
    }

    fn push(&mut self, delta: &TransferDeltaTranscript, batch_hash: &Hash) {
        let offset = self.words.len();
        append_transfer_digest_words(&mut self.words, delta, batch_hash);
        self.slices.push(Bn254PoseidonBatchSlice::new(
            offset,
            self.words.len() - offset,
        ));
    }

    fn try_hash_gpu(&self) -> Option<Vec<Hash>> {
        if self.slices.len() < DIGEST_FINALIZE_GPU_THRESHOLD
            || !poseidon_digest_acceleration_enabled()
        {
            return None;
        }
        match try_hash_bn254_poseidon_word_batches(&self.words, &self.slices) {
            Some(digests) => Some(digests.into_iter().map(Hash::prehashed).collect::<Vec<_>>()),
            None => {
                set_poseidon_digest_acceleration_enabled(false);
                None
            }
        }
    }

    fn hash_cpu_or_gpu(&self) -> Vec<Hash> {
        self.try_hash_gpu().unwrap_or_else(|| self.hash_cpu())
    }

    fn try_submit_gpu(&self) -> Option<PendingBn254PoseidonWordBatch> {
        if self.slices.len() < DIGEST_FINALIZE_GPU_THRESHOLD
            || !poseidon_digest_acceleration_enabled()
        {
            return None;
        }
        match try_submit_bn254_poseidon_word_batches(&self.words, &self.slices) {
            Some(pending) => Some(pending),
            None => {
                set_poseidon_digest_acceleration_enabled(false);
                None
            }
        }
    }

    fn hash_cpu(&self) -> Vec<Hash> {
        if self.slices.len() >= DIGEST_FINALIZE_PARALLEL_THRESHOLD {
            use rayon::prelude::*;

            return self
                .slices
                .par_iter()
                .map(|slice| {
                    let offset = slice.offset();
                    let end = offset + slice.len();
                    Hash::prehashed(halo2_poseidon::hash_u64_words_bytes(
                        &self.words[offset..end],
                    ))
                })
                .collect();
        }
        self.slices
            .iter()
            .map(|slice| {
                let offset = slice.offset();
                let end = offset + slice.len();
                Hash::prehashed(halo2_poseidon::hash_u64_words_bytes(
                    &self.words[offset..end],
                ))
            })
            .collect()
    }
}

/// Pending FASTPQ transfer transcript digest batch.
pub(crate) struct PendingTransferTranscriptDigests {
    digest_count: usize,
    batch: PoseidonDigestBatch,
    pending: PendingBn254PoseidonWordBatch,
}

impl PendingTransferTranscriptDigests {
    fn into_digests(self) -> Vec<Hash> {
        let Self { batch, pending, .. } = self;
        match pending.wait() {
            Some(digests) => digests.into_iter().map(Hash::prehashed).collect::<Vec<_>>(),
            None => {
                set_poseidon_digest_acceleration_enabled(false);
                batch.hash_cpu()
            }
        }
    }
}

/// Fill missing single-delta transcript digests before block or witness data is exposed.
pub(crate) fn finalize_transfer_transcript_digests_in_map(
    transcripts: &mut BTreeMap<Hash, Vec<TransferTranscript>>,
) {
    let pending = try_submit_transfer_transcript_digests_in_map(transcripts);
    finalize_transfer_transcript_digests_in_map_with_pending(transcripts, pending);
}

/// Fill missing single-delta transcript digests using a previously submitted GPU batch.
pub(crate) fn finalize_transfer_transcript_digests_in_map_with_pending(
    transcripts: &mut BTreeMap<Hash, Vec<TransferTranscript>>,
    pending: Option<PendingTransferTranscriptDigests>,
) {
    #[cfg(debug_assertions)]
    for entries in transcripts.values() {
        debug_assert_precomputed_transfer_transcript_digests(entries);
    }
    let digest_count = transcripts
        .values()
        .map(|entries| missing_single_delta_transcript_count(entries))
        .sum::<usize>();
    if digest_count == 0 {
        return;
    }
    if let Some(pending) = pending {
        if pending.digest_count == digest_count {
            let digests = pending.into_digests();
            let mut digests = digests.into_iter();
            for entries in transcripts.values_mut() {
                apply_transfer_transcript_digests(entries, &mut digests);
            }
            debug_assert!(
                digests.next().is_none(),
                "FASTPQ transcript digest batch output count must match inputs",
            );
            return;
        }
        debug_assert_eq!(
            pending.digest_count, digest_count,
            "pending FASTPQ transcript digest batch must match current transcript map",
        );
    }
    if digest_count >= DIGEST_FINALIZE_PARALLEL_THRESHOLD
        && try_finalize_transfer_transcript_digests_in_map_batched(transcripts, digest_count)
    {
        return;
    }
    if digest_count >= DIGEST_FINALIZE_PARALLEL_THRESHOLD {
        use rayon::prelude::*;

        transcripts
            .values_mut()
            .collect::<Vec<_>>()
            .into_par_iter()
            .for_each(|entries| finalize_transfer_transcripts_serial(entries));
    } else {
        for entries in transcripts.values_mut() {
            finalize_transfer_transcripts_serial(entries);
        }
    }
}

/// Submit missing single-delta transcript digests without waiting for completion.
pub(crate) fn try_submit_transfer_transcript_digests_in_map(
    transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
) -> Option<PendingTransferTranscriptDigests> {
    if !poseidon_digest_acceleration_enabled() {
        return None;
    }
    let digest_count = transcripts
        .values()
        .map(|entries| missing_single_delta_transcript_count(entries))
        .sum::<usize>();
    if digest_count < DIGEST_FINALIZE_GPU_THRESHOLD {
        return None;
    }
    let mut batch = PoseidonDigestBatch::with_capacity(digest_count);
    for entries in transcripts.values() {
        collect_transfer_transcript_digests(entries, &mut batch);
    }
    let pending = batch.try_submit_gpu()?;
    Some(PendingTransferTranscriptDigests {
        digest_count,
        batch,
        pending,
    })
}

/// Fill missing single-delta transcript digests in witness bundles.
pub(crate) fn finalize_transfer_transcript_bundle_digests_in_place(
    bundles: &mut [TransferTranscriptBundle],
) {
    #[cfg(debug_assertions)]
    for bundle in bundles.iter() {
        debug_assert_precomputed_transfer_transcript_digests(&bundle.transcripts);
    }
    let digest_count = bundles
        .iter()
        .map(|bundle| missing_single_delta_transcript_count(&bundle.transcripts))
        .sum::<usize>();
    if digest_count == 0 {
        return;
    }
    if digest_count >= DIGEST_FINALIZE_PARALLEL_THRESHOLD
        && try_finalize_transfer_transcript_bundle_digests_batched(bundles, digest_count)
    {
        return;
    }
    if digest_count >= DIGEST_FINALIZE_PARALLEL_THRESHOLD {
        use rayon::prelude::*;

        bundles
            .par_iter_mut()
            .for_each(|bundle| finalize_transfer_transcripts_serial(&mut bundle.transcripts));
    } else {
        for bundle in bundles {
            finalize_transfer_transcripts_serial(&mut bundle.transcripts);
        }
    }
}

fn try_finalize_transfer_transcript_digests_in_map_batched(
    transcripts: &mut BTreeMap<Hash, Vec<TransferTranscript>>,
    digest_count: usize,
) -> bool {
    debug_assert!(digest_count >= DIGEST_FINALIZE_PARALLEL_THRESHOLD);
    let mut batch = PoseidonDigestBatch::with_capacity(digest_count);
    for entries in transcripts.values() {
        collect_transfer_transcript_digests(entries, &mut batch);
    }
    let digests = batch.hash_cpu_or_gpu();
    let mut digests = digests.into_iter();
    for entries in transcripts.values_mut() {
        apply_transfer_transcript_digests(entries, &mut digests);
    }
    debug_assert!(
        digests.next().is_none(),
        "FASTPQ transcript digest batch output count must match inputs",
    );
    true
}

fn try_finalize_transfer_transcript_bundle_digests_batched(
    bundles: &mut [TransferTranscriptBundle],
    digest_count: usize,
) -> bool {
    debug_assert!(digest_count >= DIGEST_FINALIZE_PARALLEL_THRESHOLD);
    let mut batch = PoseidonDigestBatch::with_capacity(digest_count);
    for bundle in bundles.iter() {
        collect_transfer_transcript_digests(&bundle.transcripts, &mut batch);
    }
    let digests = batch.hash_cpu_or_gpu();
    let mut digests = digests.into_iter();
    for bundle in bundles {
        apply_transfer_transcript_digests(&mut bundle.transcripts, &mut digests);
    }
    debug_assert!(
        digests.next().is_none(),
        "FASTPQ transcript digest batch output count must match inputs",
    );
    true
}

fn missing_single_delta_transcript_count(transcripts: &[TransferTranscript]) -> usize {
    transcripts
        .iter()
        .filter(|transcript| needs_transfer_transcript_digest(transcript))
        .count()
}

fn is_single_delta_transcript(transcript: &TransferTranscript) -> bool {
    matches!(transcript.deltas.as_slice(), [_])
}

fn needs_transfer_transcript_digest(transcript: &TransferTranscript) -> bool {
    transcript.poseidon_preimage_digest.is_none() && is_single_delta_transcript(transcript)
}

fn collect_transfer_transcript_digests(
    transcripts: &[TransferTranscript],
    batch: &mut PoseidonDigestBatch,
) {
    for transcript in transcripts {
        if transcript.poseidon_preimage_digest.is_some() {
            continue;
        }
        let [delta] = transcript.deltas.as_slice() else {
            continue;
        };
        batch.push(delta, &transcript.batch_hash);
    }
}

fn apply_transfer_transcript_digests(
    transcripts: &mut [TransferTranscript],
    digests: &mut impl Iterator<Item = Hash>,
) {
    for transcript in transcripts {
        if needs_transfer_transcript_digest(transcript) {
            let digest = digests
                .next()
                .expect("FASTPQ transcript digest batch output missing digest");
            set_transfer_transcript_digest(transcript, digest);
        }
    }
}

fn finalize_transfer_transcripts_serial(transcripts: &mut [TransferTranscript]) {
    let mut scratch = PoseidonDigestScratch::default();
    for transcript in transcripts {
        finalize_transfer_transcript_digest_with_scratch(transcript, &mut scratch);
    }
}

fn finalize_transfer_transcript_digest_with_scratch(
    transcript: &mut TransferTranscript,
    scratch: &mut PoseidonDigestScratch,
) {
    let [delta] = transcript.deltas.as_slice() else {
        return;
    };
    if transcript.poseidon_preimage_digest.is_some() {
        #[cfg(debug_assertions)]
        {
            let existing = transcript
                .poseidon_preimage_digest
                .expect("digest presence checked above");
            debug_assert_eq!(
                existing,
                poseidon_preimage_digest_with_scratch(delta, &transcript.batch_hash, scratch),
                "precomputed FASTPQ transfer transcript digest must match canonical digest",
            );
        }
        return;
    }
    let digest = poseidon_preimage_digest_with_scratch(delta, &transcript.batch_hash, scratch);
    set_transfer_transcript_digest(transcript, digest);
}

fn set_transfer_transcript_digest(transcript: &mut TransferTranscript, digest: Hash) {
    if let Some(existing) = transcript.poseidon_preimage_digest {
        debug_assert_eq!(
            existing, digest,
            "precomputed FASTPQ transfer transcript digest must match canonical digest",
        );
    } else {
        transcript.poseidon_preimage_digest = Some(digest);
    }
}

#[cfg(debug_assertions)]
fn debug_assert_precomputed_transfer_transcript_digests(transcripts: &[TransferTranscript]) {
    let mut scratch = PoseidonDigestScratch::default();
    for transcript in transcripts {
        let Some(existing) = transcript.poseidon_preimage_digest else {
            continue;
        };
        let [delta] = transcript.deltas.as_slice() else {
            continue;
        };
        debug_assert_eq!(
            existing,
            poseidon_preimage_digest_with_scratch(delta, &transcript.batch_hash, &mut scratch),
            "precomputed FASTPQ transfer transcript digest must match canonical digest",
        );
    }
}

/// Build a FASTPQ public input template for the supplied block witness.
#[must_use]
pub fn public_inputs_template_from_block(
    header: &BlockHeader,
    witness: &ExecWitness,
    perm_root: [u8; 32],
) -> FastpqPublicInputsTemplate {
    let creation_ms = u64::try_from(header.creation_time().as_millis()).unwrap_or(u64::MAX);
    let slot = creation_ms.saturating_mul(1_000_000);
    let old_root = crate::sumeragi::exec::parent_state_from_witness(witness);
    let new_root = crate::sumeragi::exec::post_state_from_witness(witness);
    FastpqPublicInputsTemplate {
        dsid: dataspace_id_bytes(DataSpaceId::UNIVERSAL),
        slot,
        old_root: old_root.into(),
        new_root: new_root.into(),
        perm_root,
    }
}

pub(crate) fn dataspace_id_bytes(dsid: DataSpaceId) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&dsid.as_u64().to_le_bytes());
    out
}

pub(crate) fn permission_table_root<'a, I>(roles: I) -> [u8; 32]
where
    I: IntoIterator<Item = (&'a RoleId, &'a Role)>,
{
    let mut entries = Vec::new();
    for (role_id, role) in roles {
        let role_bytes = hash_encoded(role_id);
        for permission in &role.permissions {
            let epoch = role.permission_epoch(permission).unwrap_or_default();
            entries.push(PermissionTableEntry {
                role_bytes,
                permission_bytes: hash_encoded(permission),
                epoch_bytes: epoch.to_le_bytes(),
            });
        }
    }
    if entries.is_empty() {
        return [0u8; 32];
    }
    entries.sort_unstable_by(|left, right| {
        (left.role_bytes, left.permission_bytes, left.epoch_bytes).cmp(&(
            right.role_bytes,
            right.permission_bytes,
            right.epoch_bytes,
        ))
    });
    let hashes: Vec<u64> = entries.iter().map(permission_hash_from_entry).collect();
    field_element_bytes(poseidon_merkle_root(&hashes))
}

#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_field_names)]
struct PermissionTableEntry {
    role_bytes: [u8; 32],
    permission_bytes: [u8; 32],
    epoch_bytes: [u8; 8],
}

fn hash_encoded<T: NoritoEncode>(value: &T) -> [u8; 32] {
    let hash = Hash::new(value.encode());
    hash.into()
}

fn permission_hash_from_entry(entry: &PermissionTableEntry) -> u64 {
    let mut payload = Vec::with_capacity(32 + 32 + 8);
    payload.extend_from_slice(&entry.role_bytes);
    payload.extend_from_slice(&entry.permission_bytes);
    payload.extend_from_slice(&entry.epoch_bytes);
    let packed = fastpq_prover::pack_bytes(&payload);
    fastpq_prover::hash_field_elements(&packed.limbs)
}

fn poseidon_merkle_root(leaves: &[u64]) -> u64 {
    if leaves.is_empty() {
        return 0;
    }
    let mut current = leaves.to_vec();
    while current.len() > 1 {
        if current.len() % 2 == 1 {
            let last = *current.last().expect("non-empty vector");
            current.push(last);
        }
        let mut next = Vec::with_capacity(current.len() / 2);
        for pair in current.chunks(2) {
            next.push(hash_field_with_domain(
                PERMISSION_TABLE_NODE_DOMAIN,
                &[pair[0], pair[1]],
            ));
        }
        current = next;
    }
    current[0]
}

fn hash_field_with_domain(domain: &[u8], values: &[u64]) -> u64 {
    let mut sponge = PoseidonSponge::new();
    sponge.absorb(domain_seed(domain));
    sponge.absorb_slice(values);
    sponge.squeeze()
}

fn domain_seed(domain: &[u8]) -> u64 {
    let digest = Hash::new(domain);
    let bytes = digest.as_ref();
    let raw = u64_from_le_bytes(bytes);
    let reduced = u128::from(raw) % u128::from(fastpq_prover::FIELD_MODULUS);
    u64::try_from(reduced).expect("modulus reduction fits u64")
}

fn field_element_bytes(value: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&value.to_le_bytes());
    out
}

fn public_inputs_from_template(
    template: FastpqPublicInputsTemplate,
    tx_set_hash: [u8; 32],
) -> FastpqPublicInputs {
    template.with_tx_set_hash(tx_set_hash)
}

/// Compute a transaction set commitment from ordered entrypoint hashes.
pub(crate) fn tx_set_hash_from_ordered_hashes<I, H>(hashes: I) -> [u8; 32]
where
    I: IntoIterator<Item = H>,
    H: AsRef<[u8; 32]>,
{
    let iter = hashes.into_iter();
    let (lower, _) = iter.size_hint();
    let mut payload =
        Vec::with_capacity(TX_SET_HASH_DOMAIN.len() + lower.saturating_mul(Hash::LENGTH));
    payload.extend_from_slice(TX_SET_HASH_DOMAIN);
    for hash in iter {
        let bytes = hash.as_ref();
        payload.extend_from_slice(bytes);
    }
    Hash::new(payload).into()
}

/// Convert a collection of transfer transcripts into a canonical FASTPQ transition batch.
///
/// The caller is responsible for supplying `public_inputs` and threading metadata
/// (entry hash, transcript count, etc.) into the returned batch if required by downstream consumers.
///
/// # Errors
/// Returns [`TranscriptBatchError`] if any transcript fails to append to the batch.
pub fn batch_from_transcripts<'a, I>(
    parameter_set: impl Into<String>,
    public_inputs: FastpqPublicInputs,
    transcripts: I,
) -> Result<TransitionBatch, TranscriptBatchError>
where
    I: IntoIterator<Item = &'a TransferTranscript>,
{
    let mut transcripts: Vec<TransferTranscript> = transcripts.into_iter().cloned().collect();
    let transfer_roots = if transcripts.is_empty() {
        None
    } else {
        Some(
            attach_transfer_smt_witnesses(&mut transcripts)
                .map_err(|source| TranscriptBatchError::TransferWitness { source })?,
        )
    };
    finalize_transfer_transcripts_serial(&mut transcripts);
    let mut batch = TransitionBatch::new(parameter_set, public_inputs_from_dto(&public_inputs));
    if let Some((old_root, new_root)) = transfer_roots {
        batch.public_inputs.old_root = old_root;
        batch.public_inputs.new_root = new_root;
    }
    for transcript in &transcripts {
        append_transcript(&mut batch, transcript)?;
    }
    attach_transcript_metadata(&mut batch, transcripts)?;
    batch.sort();
    Ok(batch)
}

/// Build a FASTPQ batch from a committed transcript bundle and attach the entry-level metadata
/// required by AXT proof binding.
///
/// This is the public reconstruction form used by recovery surfaces when compact sidecars retain
/// public inputs but omit transition rows.
///
/// # Errors
/// Returns [`TranscriptBatchError`] if any transcript fails to append to the batch.
pub fn batch_from_transcript_bundle(
    parameter_set: impl Into<String>,
    public_inputs: PublicInputs,
    entry_hash: Hash,
    transcripts: &[TransferTranscript],
) -> Result<TransitionBatch, TranscriptBatchError> {
    let mut batch = batch_from_transcripts(
        parameter_set,
        public_inputs_to_dto(&public_inputs),
        transcripts,
    )?;
    annotate_metadata(&mut batch, &entry_hash, transcripts.len());
    Ok(batch)
}

fn append_transcript(
    batch: &mut TransitionBatch,
    transcript: &TransferTranscript,
) -> Result<(), TranscriptBatchError> {
    for delta in &transcript.deltas {
        push_transfer_delta(batch, delta)?;
    }
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn attach_transcript_metadata(
    batch: &mut TransitionBatch,
    transcripts: Vec<TransferTranscript>,
) -> Result<(), TranscriptBatchError> {
    if transcripts.is_empty() {
        return Ok(());
    }
    let encoded = to_bytes(&transcripts)?;
    batch
        .metadata
        .insert(TRANSFER_TRANSCRIPTS_METADATA_KEY.into(), encoded);
    Ok(())
}

fn push_transfer_delta(
    batch: &mut TransitionBatch,
    delta: &TransferDeltaTranscript,
) -> Result<(), TranscriptBatchError> {
    let from_key = balance_key(&delta.asset_definition, &delta.from_account);
    let to_key = balance_key(&delta.asset_definition, &delta.to_account);
    let target_scale = delta.normalized_scale();
    let from_pre = encode_numeric_le(&delta.from_balance_before, target_scale)?;
    let from_post = encode_numeric_le(&delta.from_balance_after, target_scale)?;
    let to_pre = encode_numeric_le(&delta.to_balance_before, target_scale)?;
    let to_post = encode_numeric_le(&delta.to_balance_after, target_scale)?;

    batch.push(StateTransition::new(
        from_key,
        from_pre,
        from_post,
        OperationKind::Transfer,
    ));
    batch.push(StateTransition::new(
        to_key,
        to_pre,
        to_post,
        OperationKind::Transfer,
    ));
    Ok(())
}

fn balance_key(asset: &AssetDefinitionId, account: &AccountId) -> Vec<u8> {
    format!("asset/{asset}/{account}").into_bytes()
}

fn encode_numeric_le(value: &Numeric, target_scale: u32) -> Result<Vec<u8>, TranscriptBatchError> {
    let integer = normalized_numeric_to_u64(value, target_scale).ok_or_else(|| {
        TranscriptBatchError::NumericEncoding {
            value: value.clone(),
        }
    })?;
    Ok(integer.to_le_bytes().to_vec())
}

/// Convert the FASTPQ batches stored in an [`ExecWitness`] into prover batches.
///
/// # Errors
/// Returns [`TranscriptBatchError::MissingFastpqBatches`] when transcripts are present
/// without prebuilt batches.
pub fn batches_from_exec_witness(
    witness: &ExecWitness,
) -> Result<Vec<TransitionBatch>, TranscriptBatchError> {
    if !witness.fastpq_batches.is_empty() {
        return Ok(witness
            .fastpq_batches
            .iter()
            .map(transition_batch_from_dto)
            .collect());
    }
    if witness.fastpq_transcripts.is_empty() {
        return Ok(Vec::new());
    }
    Err(TranscriptBatchError::MissingFastpqBatches)
}

/// Convert transcript bundles into FASTPQ batches, preserving execution order.
///
/// # Errors
/// Returns [`TranscriptBatchError`] if constructing a batch fails.
pub fn batches_from_bundles<'a, I>(
    parameter_set: &str,
    public_inputs: FastpqPublicInputsTemplate,
    tx_set_hash: [u8; 32],
    bundles: I,
) -> Result<Vec<TransitionBatch>, TranscriptBatchError>
where
    I: IntoIterator<Item = &'a TransferTranscriptBundle>,
{
    let mut batches = Vec::new();
    let public_inputs = public_inputs_from_template(public_inputs, tx_set_hash);
    for bundle in bundles {
        let mut batch = batch_from_transcripts(
            parameter_set.to_string(),
            public_inputs,
            &bundle.transcripts,
        )?;
        annotate_metadata(&mut batch, &bundle.entry_hash, bundle.transcripts.len());
        batches.push(batch);
    }
    Ok(batches)
}

fn annotate_metadata(batch: &mut TransitionBatch, entry_hash: &Hash, transcript_count: usize) {
    batch
        .metadata
        .insert(ENTRY_HASH_METADATA_KEY.into(), entry_hash.as_ref().to_vec());
    batch.metadata.insert(
        TRANSCRIPT_COUNT_METADATA_KEY.into(),
        (transcript_count as u64).to_le_bytes().to_vec(),
    );
}

/// Convert a map of transcripts grouped by entry hash into DTO batches.
///
/// # Errors
/// Returns [`TranscriptBatchError`] if constructing the batches fails.
pub fn dto_batches_from_transcripts(
    parameter_set: &str,
    public_inputs: FastpqPublicInputsTemplate,
    tx_set_hash: [u8; 32],
    transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
) -> Result<Vec<FastpqTransitionBatch>, TranscriptBatchError> {
    let bundles: Vec<_> = transcripts
        .iter()
        .map(|(entry_hash, entries)| TransferTranscriptBundle {
            entry_hash: *entry_hash,
            transcripts: entries.clone(),
        })
        .collect();
    let batches = batches_from_bundles(parameter_set, public_inputs, tx_set_hash, bundles.iter())?;
    Ok(batches.iter().map(transition_batch_to_dto).collect())
}

/// Convert a prover batch into its DTO representation suitable for `ExecWitness`.
#[must_use]
pub fn transition_batch_to_dto(batch: &TransitionBatch) -> FastpqTransitionBatch {
    transition_batch_to_dto_ref(batch)
}

/// Convert a prover batch reference into a DTO (borrowing-friendly helper).
#[must_use]
pub fn transition_batch_to_dto_ref(batch: &TransitionBatch) -> FastpqTransitionBatch {
    let transitions = batch
        .transitions
        .iter()
        .map(state_transition_to_dto)
        .collect();
    FastpqTransitionBatch {
        parameter: batch.parameter.clone(),
        public_inputs: public_inputs_to_dto(&batch.public_inputs),
        transitions,
        metadata: batch.metadata.clone(),
    }
}

/// Convert a DTO batch back into the prover representation.
#[must_use]
pub fn transition_batch_from_dto(dto: &FastpqTransitionBatch) -> TransitionBatch {
    let mut batch = TransitionBatch::new(
        dto.parameter.clone(),
        public_inputs_from_dto(&dto.public_inputs),
    );
    for transition in &dto.transitions {
        batch.push(StateTransition::new(
            transition.key.clone(),
            transition.pre_value.clone(),
            transition.post_value.clone(),
            operation_from_dto(&transition.operation),
        ));
    }
    batch.metadata = dto.metadata.clone();
    batch
}

fn public_inputs_to_dto(inputs: &PublicInputs) -> FastpqPublicInputs {
    FastpqPublicInputs {
        dsid: inputs.dsid,
        slot: inputs.slot,
        old_root: inputs.old_root,
        new_root: inputs.new_root,
        perm_root: inputs.perm_root,
        tx_set_hash: inputs.tx_set_hash,
    }
}

fn public_inputs_from_dto(inputs: &FastpqPublicInputs) -> PublicInputs {
    PublicInputs {
        dsid: inputs.dsid,
        slot: inputs.slot,
        old_root: inputs.old_root,
        new_root: inputs.new_root,
        perm_root: inputs.perm_root,
        tx_set_hash: inputs.tx_set_hash,
    }
}

fn state_transition_to_dto(transition: &StateTransition) -> FastpqStateTransition {
    FastpqStateTransition {
        key: transition.key.clone(),
        pre_value: transition.pre_value.clone(),
        post_value: transition.post_value.clone(),
        operation: operation_to_dto(&transition.operation),
    }
}

fn operation_to_dto(operation: &OperationKind) -> FastpqOperationKind {
    match operation {
        OperationKind::Transfer => FastpqOperationKind::Transfer,
        OperationKind::Mint => FastpqOperationKind::Mint,
        OperationKind::Burn => FastpqOperationKind::Burn,
        OperationKind::RoleGrant {
            role_id,
            permission_id,
            epoch,
        } => FastpqOperationKind::RoleGrant(FastpqRolePermissionDelta {
            role_id: role_id.clone(),
            permission_id: permission_id.clone(),
            epoch: *epoch,
        }),
        OperationKind::RoleRevoke {
            role_id,
            permission_id,
            epoch,
        } => FastpqOperationKind::RoleRevoke(FastpqRolePermissionDelta {
            role_id: role_id.clone(),
            permission_id: permission_id.clone(),
            epoch: *epoch,
        }),
        OperationKind::MetaSet => FastpqOperationKind::MetaSet,
    }
}

fn operation_from_dto(operation: &FastpqOperationKind) -> OperationKind {
    match operation {
        FastpqOperationKind::Transfer => OperationKind::Transfer,
        FastpqOperationKind::Mint => OperationKind::Mint,
        FastpqOperationKind::Burn => OperationKind::Burn,
        FastpqOperationKind::RoleGrant(delta) => OperationKind::RoleGrant {
            role_id: delta.role_id.clone(),
            permission_id: delta.permission_id.clone(),
            epoch: delta.epoch,
        },
        FastpqOperationKind::RoleRevoke(delta) => OperationKind::RoleRevoke {
            role_id: delta.role_id.clone(),
            permission_id: delta.permission_id.clone(),
            epoch: delta.epoch,
        },
        FastpqOperationKind::MetaSet => OperationKind::MetaSet,
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, num::NonZeroU64};

    use iroha_data_model::{
        Registrable,
        block::{
            BlockHeader,
            consensus::{ExecKv, ExecWitness},
        },
        domain::DomainId,
        fastpq::{TransferTranscript, TransferTranscriptBundle},
        permission::Permission,
        role::{Role, RoleId},
    };
    use iroha_primitives::{json::Json, numeric::Numeric};
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use norito::decode_from_bytes;

    use super::*;

    #[test]
    fn authority_digest_matches_known_vector() {
        let digest = authority_digest(&ALICE_ID);
        assert_eq!(
            hex::encode(digest.as_ref()),
            "e1e0bb25f044ba013bfb99711a2f409472d1f941b68e6716a677ac6d1bcd5fcb"
        );
    }

    #[test]
    fn poseidon_digest_matches_known_vector() {
        let asset = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let delta = TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: (*BOB_ID).clone(),
            asset_definition: asset,
            amount: Numeric::from(42u32),
            from_balance_before: Numeric::from(200u32),
            from_balance_after: Numeric::from(158u32),
            to_balance_before: Numeric::from(1u32),
            to_balance_after: Numeric::from(43u32),
            from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
            to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        };
        let batch_hash = Hash::prehashed([0x11; 32]);
        let mut encoded_preimage = Vec::new();
        encoded_preimage.extend_from_slice(&delta.from_account.encode());
        encoded_preimage.extend_from_slice(&delta.to_account.encode());
        encoded_preimage.extend_from_slice(&delta.asset_definition.encode());
        encoded_preimage.extend_from_slice(&delta.amount.encode());
        encoded_preimage.extend_from_slice(batch_hash.as_ref());
        let mut streamed_preimage = Vec::new();
        delta.from_account.encode_to(&mut streamed_preimage);
        delta.to_account.encode_to(&mut streamed_preimage);
        delta.asset_definition.encode_to(&mut streamed_preimage);
        delta.amount.encode_to(&mut streamed_preimage);
        streamed_preimage.extend_from_slice(batch_hash.as_ref());
        assert_eq!(streamed_preimage, encoded_preimage);
        let digest = poseidon_preimage_digest(&delta, &batch_hash);
        assert_eq!(
            digest,
            Hash::prehashed(halo2_poseidon::hash_bytes(&encoded_preimage))
        );
        assert_eq!(
            hex::encode(digest.as_ref()),
            "e18ad4e6b7fc5a63b849db3f3d8da27fe551fee68954e849b8c253a18efd1623"
        );
    }

    #[test]
    fn poseidon_digest_scratch_matches_canonical_oracle() {
        let asset = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let delta = TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: (*BOB_ID).clone(),
            asset_definition: asset,
            amount: Numeric::from(42u32),
            from_balance_before: Numeric::from(200u32),
            from_balance_after: Numeric::from(158u32),
            to_balance_before: Numeric::from(1u32),
            to_balance_after: Numeric::from(43u32),
            from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
            to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        };
        let mut scratch = PoseidonDigestScratch::default();
        let first_hash = Hash::prehashed([0x11; 32]);
        let second_hash = Hash::prehashed([0x22; 32]);

        assert_eq!(
            poseidon_preimage_digest_with_scratch(&delta, &first_hash, &mut scratch),
            poseidon_preimage_digest(&delta, &first_hash)
        );
        assert_eq!(
            poseidon_preimage_digest_with_scratch(&delta, &second_hash, &mut scratch),
            poseidon_preimage_digest(&delta, &second_hash)
        );
    }

    #[test]
    fn poseidon_word_packer_matches_little_endian_chunks() {
        for len in [0usize, 1, 7, 8, 9, 15, 16, 17, 63, 64, 65] {
            let input = (0..len)
                .map(|idx| (idx as u8).wrapping_mul(17).wrapping_add(3))
                .collect::<Vec<_>>();
            let mut words = Vec::new();
            {
                let mut packer = PoseidonWordPacker::new(&mut words);
                for chunk in input.chunks(5) {
                    packer.update(chunk);
                }
                packer.finish();
            }
            let expected = input
                .chunks(8)
                .map(|chunk| {
                    let mut word = [0u8; 8];
                    word[..chunk.len()].copy_from_slice(chunk);
                    u64::from_le_bytes(word)
                })
                .collect::<Vec<_>>();
            assert_eq!(words, expected, "len {len}");
        }
    }

    #[test]
    fn transfer_digest_word_writer_matches_streaming_oracle() {
        let transcript = sample_transcript();
        let delta = &transcript.deltas[0];
        let encoded_preimage = encoded_transfer_digest_preimage(delta, &transcript.batch_hash);
        let words = transfer_digest_words(delta, &transcript.batch_hash);
        assert_eq!(
            halo2_poseidon::hash_u64_words_bytes(&words),
            halo2_poseidon::hash_bytes(&encoded_preimage)
        );
        assert_eq!(
            poseidon_preimage_digest(delta, &transcript.batch_hash),
            Hash::prehashed(halo2_poseidon::hash_bytes(&encoded_preimage))
        );
    }

    #[test]
    fn poseidon_digest_batch_cpu_hash_matches_single_digest() {
        let transcript = sample_transcript();
        let delta = &transcript.deltas[0];
        let mut batch = PoseidonDigestBatch::with_capacity(1);
        batch.push(delta, &transcript.batch_hash);

        assert_eq!(
            batch.hash_cpu(),
            vec![poseidon_preimage_digest(delta, &transcript.batch_hash)]
        );
        assert!(
            batch.try_hash_gpu().is_none(),
            "single digest should stay below the GPU threshold"
        );
    }

    #[test]
    fn poseidon_digest_batch_parallel_cpu_preserves_input_order() {
        let mut batch = PoseidonDigestBatch::with_capacity(DIGEST_FINALIZE_PARALLEL_THRESHOLD);
        let mut expected = Vec::with_capacity(DIGEST_FINALIZE_PARALLEL_THRESHOLD);

        for idx in 0..DIGEST_FINALIZE_PARALLEL_THRESHOLD {
            let mut transcript = sample_transcript();
            transcript.batch_hash = Hash::prehashed([idx as u8; Hash::LENGTH]);
            let delta = &transcript.deltas[0];
            expected.push(poseidon_preimage_digest(delta, &transcript.batch_hash));
            batch.push(delta, &transcript.batch_hash);
        }

        assert_eq!(batch.hash_cpu(), expected);
    }

    #[test]
    fn poseidon_digest_batch_cpu_or_gpu_matches_ordered_cpu_output() {
        let _guard = DigestAccelerationGuard::new();
        set_poseidon_digest_acceleration_enabled(true);
        let mut batch = PoseidonDigestBatch::with_capacity(DIGEST_FINALIZE_GPU_THRESHOLD);

        for idx in 0..DIGEST_FINALIZE_GPU_THRESHOLD {
            let mut transcript = sample_transcript();
            transcript.batch_hash = Hash::prehashed([idx as u8; Hash::LENGTH]);
            batch.push(&transcript.deltas[0], &transcript.batch_hash);
        }

        assert_eq!(batch.hash_cpu_or_gpu(), batch.hash_cpu());
    }

    #[test]
    #[cfg(not(feature = "fastpq-gpu"))]
    fn poseidon_digest_batch_failed_gpu_submission_disables_acceleration() {
        let _guard = DigestAccelerationGuard::new();
        set_poseidon_digest_acceleration_enabled(true);
        let mut batch = PoseidonDigestBatch::with_capacity(DIGEST_FINALIZE_GPU_THRESHOLD);

        for idx in 0..DIGEST_FINALIZE_GPU_THRESHOLD {
            let mut transcript = sample_transcript();
            transcript.batch_hash = Hash::prehashed([idx as u8; Hash::LENGTH]);
            batch.push(&transcript.deltas[0], &transcript.batch_hash);
        }

        assert_eq!(batch.hash_cpu_or_gpu(), batch.hash_cpu());
        assert!(
            !poseidon_digest_acceleration_enabled(),
            "failed GPU submission should latch the core digest gate off"
        );
    }

    #[test]
    fn digest_acceleration_respects_configured_modes() {
        let _guard = DigestAccelerationGuard::new();
        let explicit_gpu = fastpq_cfg(FastpqExecutionMode::Cpu, FastpqPoseidonMode::Gpu);
        assert!(poseidon_digest_acceleration_configured(&explicit_gpu));
        assert!(configure_poseidon_digest_acceleration_with_preflight(
            &explicit_gpu,
            || true
        ));
        assert!(poseidon_digest_acceleration_enabled());

        set_poseidon_digest_acceleration_enabled(true);
        assert!(!configure_poseidon_digest_acceleration_with_preflight(
            &explicit_gpu,
            || false
        ));
        assert!(!poseidon_digest_acceleration_enabled());

        let poseidon_cpu = fastpq_cfg(FastpqExecutionMode::Gpu, FastpqPoseidonMode::Cpu);
        assert!(!poseidon_digest_acceleration_configured(&poseidon_cpu));
        assert!(!configure_poseidon_digest_acceleration_with_preflight(
            &poseidon_cpu,
            || true
        ));
        assert!(!poseidon_digest_acceleration_enabled());
    }

    #[test]
    fn configure_digest_acceleration_keeps_cpu_mode_disabled() {
        let _guard = DigestAccelerationGuard::new();
        let cpu = fastpq_cfg(FastpqExecutionMode::Cpu, FastpqPoseidonMode::Cpu);

        configure_poseidon_digest_acceleration(&cpu);

        assert!(!poseidon_digest_acceleration_enabled());
    }

    #[test]
    fn finalize_transfer_transcripts_fills_only_single_delta_digests() {
        let mut single = sample_transcript();
        let expected = poseidon_preimage_digest(&single.deltas[0], &single.batch_hash);
        let mut multi = sample_transcript();
        multi.deltas.push(multi.deltas[0].clone());

        let mut map = BTreeMap::new();
        map.insert(
            Hash::prehashed([0x77; 32]),
            vec![single.clone(), multi.clone()],
        );
        finalize_transfer_transcript_digests_in_map(&mut map);

        let entries = map.values().next().expect("entries");
        assert_eq!(entries[0].poseidon_preimage_digest, Some(expected));
        assert!(entries[1].poseidon_preimage_digest.is_none());

        single.poseidon_preimage_digest = Some(expected);
        let mut bundles = vec![TransferTranscriptBundle {
            entry_hash: Hash::prehashed([0x78; 32]),
            transcripts: vec![single],
        }];
        finalize_transfer_transcript_bundle_digests_in_place(&mut bundles);
        assert_eq!(
            bundles[0].transcripts[0].poseidon_preimage_digest,
            Some(expected)
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(
        expected = "precomputed FASTPQ transfer transcript digest must match canonical digest"
    )]
    fn finalize_transfer_transcripts_debug_asserts_precomputed_mismatch() {
        let mut single = sample_transcript();
        single.poseidon_preimage_digest = Some(Hash::prehashed([0xEE; Hash::LENGTH]));
        let mut map = BTreeMap::from([(Hash::prehashed([0x7A; Hash::LENGTH]), vec![single])]);

        finalize_transfer_transcript_digests_in_map(&mut map);
    }

    #[test]
    fn finalize_transfer_transcripts_batched_cpu_matches_canonical_oracle() {
        let _guard = DigestAccelerationGuard::new();
        set_poseidon_digest_acceleration_enabled(false);

        let mut entries = Vec::with_capacity(DIGEST_FINALIZE_PARALLEL_THRESHOLD);
        let mut expected = Vec::with_capacity(DIGEST_FINALIZE_PARALLEL_THRESHOLD);
        for idx in 0..DIGEST_FINALIZE_PARALLEL_THRESHOLD {
            let mut transcript = sample_transcript();
            transcript.batch_hash = Hash::prehashed([idx as u8; Hash::LENGTH]);
            transcript.poseidon_preimage_digest = None;
            expected.push(poseidon_preimage_digest(
                &transcript.deltas[0],
                &transcript.batch_hash,
            ));
            entries.push(transcript);
        }

        let mut map = BTreeMap::from([(Hash::prehashed([0x79; Hash::LENGTH]), entries)]);
        finalize_transfer_transcript_digests_in_map(&mut map);

        let actual = map
            .values()
            .next()
            .expect("entries")
            .iter()
            .map(|transcript| transcript.poseidon_preimage_digest)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected.into_iter().map(Some).collect::<Vec<_>>());
    }

    #[test]
    fn missing_single_delta_transcript_count_ignores_precomputed_and_multi_delta() {
        let mut precomputed = sample_transcript();
        precomputed.poseidon_preimage_digest = Some(poseidon_preimage_digest(
            &precomputed.deltas[0],
            &precomputed.batch_hash,
        ));
        let missing = sample_transcript();
        let mut multi = sample_transcript();
        multi.deltas.push(multi.deltas[0].clone());

        assert_eq!(
            missing_single_delta_transcript_count(&[precomputed, missing, multi]),
            1
        );
    }

    #[test]
    fn permission_table_root_is_order_independent() {
        let perm_a = Permission::new("perm_a".to_string(), Json::new(()));
        let perm_b = Permission::new("perm_b".to_string(), Json::new(()));
        let role_a: RoleId = "role_a".parse().expect("role id");
        let role_b: RoleId = "role_b".parse().expect("role id");
        let role_a = Role::new(role_a.clone(), (*ALICE_ID).clone())
            .add_permission(perm_a.clone())
            .add_permission(perm_b)
            .build(&ALICE_ID);
        let role_b = Role::new(role_b.clone(), (*ALICE_ID).clone())
            .add_permission(perm_a)
            .build(&ALICE_ID);
        let first = [
            (role_b.id.clone(), role_b.clone()),
            (role_a.id.clone(), role_a.clone()),
        ];
        let second = [
            (role_a.id.clone(), role_a.clone()),
            (role_b.id.clone(), role_b.clone()),
        ];
        let root_first = permission_table_root(first.iter().map(|(id, role)| (id, role)));
        let root_second = permission_table_root(second.iter().map(|(id, role)| (id, role)));
        assert_eq!(root_first, root_second);
        assert_ne!(root_first, [0u8; 32]);
    }

    #[test]
    fn permission_table_root_tracks_permission_epochs() {
        let perm = Permission::new("perm_epoch".to_string(), Json::new(()));
        let role_id: RoleId = "role_epoch".parse().expect("role id");
        let role_epoch_0 = Role::new(role_id.clone(), (*ALICE_ID).clone())
            .add_permission_with_epoch(perm.clone(), 0)
            .build(&ALICE_ID);
        let role_epoch_7 = Role::new(role_id.clone(), (*ALICE_ID).clone())
            .add_permission_with_epoch(perm.clone(), 7)
            .build(&ALICE_ID);
        let root_epoch_0 = permission_table_root(
            [(role_id.clone(), role_epoch_0)]
                .iter()
                .map(|(id, role)| (id, role)),
        );
        let root_epoch_7 = permission_table_root(
            [(role_id.clone(), role_epoch_7)]
                .iter()
                .map(|(id, role)| (id, role)),
        );
        assert_ne!(root_epoch_0, root_epoch_7);
    }

    #[test]
    fn public_inputs_template_from_block_uses_header_and_roots() {
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("height"),
            None,
            None,
            None,
            123,
            0,
        );
        let witness = ExecWitness {
            reads: vec![ExecKv {
                key: b"key".to_vec(),
                value: b"old".to_vec(),
            }],
            writes: vec![ExecKv {
                key: b"key".to_vec(),
                value: b"new".to_vec(),
            }],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        let perm_root = [0x11; 32];
        let template = public_inputs_template_from_block(&header, &witness, perm_root);
        let mut expected_dsid = [0u8; 16];
        expected_dsid[..8].copy_from_slice(&DataSpaceId::UNIVERSAL.as_u64().to_le_bytes());
        assert_eq!(template.dsid, expected_dsid);
        assert_eq!(template.slot, 123_000_000);
        assert_eq!(template.perm_root, perm_root);
        assert_eq!(
            template.old_root,
            <[u8; 32]>::from(crate::sumeragi::exec::parent_state_from_witness(&witness))
        );
        assert_eq!(
            template.new_root,
            <[u8; 32]>::from(crate::sumeragi::exec::post_state_from_witness(&witness))
        );
    }

    #[test]
    fn public_inputs_from_template_uses_tx_set_hash() {
        let template = sample_template();
        let tx_set_hash = [0x22; 32];
        let inputs = public_inputs_from_template(template, tx_set_hash);
        assert_eq!(inputs.tx_set_hash, tx_set_hash);
        assert_eq!(inputs.dsid, template.dsid);
        assert_eq!(inputs.slot, template.slot);
        assert_eq!(inputs.old_root, template.old_root);
        assert_eq!(inputs.new_root, template.new_root);
        assert_eq!(inputs.perm_root, template.perm_root);
    }

    #[test]
    fn tx_set_hash_from_ordered_hashes_matches_domain() {
        let first = Hash::prehashed([0x11; 32]);
        let second = Hash::prehashed([0x22; 32]);
        let tx_set_hash = tx_set_hash_from_ordered_hashes([first, second]);
        let mut payload = Vec::with_capacity(TX_SET_HASH_DOMAIN.len() + 2 * Hash::LENGTH);
        payload.extend_from_slice(TX_SET_HASH_DOMAIN);
        payload.extend_from_slice(first.as_ref());
        payload.extend_from_slice(second.as_ref());
        let expected: [u8; 32] = Hash::new(payload).into();
        assert_eq!(tx_set_hash, expected);
        let reversed = tx_set_hash_from_ordered_hashes([second, first]);
        assert_ne!(tx_set_hash, reversed);
    }

    #[test]
    fn batch_from_transcripts_builds_transfer_rows() {
        let transcript = sample_transcript();
        let batch = batch_from_transcripts(
            "fastpq-lane-balanced",
            sample_public_inputs(),
            [&transcript],
        )
        .unwrap();
        assert_eq!(batch.transitions.len(), 2);
        let delta = &transcript.deltas[0];
        let sender_key = format!("asset/{}/{}", delta.asset_definition, delta.from_account);
        let receiver_key = format!("asset/{}/{}", delta.asset_definition, delta.to_account);

        let sender_row = batch
            .transitions
            .iter()
            .find(|row| row.key == sender_key.as_bytes())
            .expect("sender row present");
        assert_eq!(sender_row.operation_rank(), OperationKind::Transfer.rank());
        assert_eq!(decode_le(&sender_row.pre_value), 200);
        assert_eq!(decode_le(&sender_row.post_value), 158);

        let receiver_row = batch
            .transitions
            .iter()
            .find(|row| row.key == receiver_key.as_bytes())
            .expect("receiver row present");
        assert_eq!(decode_le(&receiver_row.pre_value), 1);
        assert_eq!(decode_le(&receiver_row.post_value), 43);
    }

    #[test]
    fn batch_from_transcripts_embeds_transfer_metadata() {
        let transcript = sample_transcript();
        let batch = batch_from_transcripts(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_public_inputs(),
            [&transcript],
        )
        .unwrap();
        let encoded = batch
            .metadata
            .get(TRANSFER_TRANSCRIPTS_METADATA_KEY)
            .expect("transfer metadata");
        let decoded: Vec<TransferTranscript> =
            decode_from_bytes(encoded).expect("decode transcripts");
        let mut expected = transcript;
        fastpq_prover::gadgets::transfer::attach_transfer_smt_witnesses(std::slice::from_mut(
            &mut expected,
        ))
        .expect("attach expected witnesses");
        expected.poseidon_preimage_digest = Some(poseidon_preimage_digest(
            &expected.deltas[0],
            &expected.batch_hash,
        ));
        assert_eq!(decoded, vec![expected]);
    }

    #[test]
    fn batch_from_transcripts_attaches_transfer_smt_witnesses() {
        let transcript = sample_transcript();
        let batch = batch_from_transcripts(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_public_inputs(),
            [&transcript],
        )
        .expect("batch");
        let encoded = batch
            .metadata
            .get(TRANSFER_TRANSCRIPTS_METADATA_KEY)
            .expect("transfer metadata");
        let decoded: Vec<TransferTranscript> =
            decode_from_bytes(encoded).expect("decode transcripts");
        let delta = &decoded[0].deltas[0];
        assert_eq!(delta.from_smt_witness.path_bits.len(), 4);
        assert_eq!(delta.from_smt_witness.siblings.len(), 32);
        assert_eq!(delta.to_smt_witness.path_bits.len(), 4);
        assert_eq!(delta.to_smt_witness.siblings.len(), 32);
        assert_ne!(batch.public_inputs.old_root, [0; 32]);
        assert_ne!(batch.public_inputs.new_root, [0; 32]);
        fastpq_prover::gadgets::transfer::verify_transcripts(&batch.transitions, &decoded)
            .expect("transfer transcript rows verify");
        fastpq_prover::gadgets::transfer::transcripts_to_witnesses(
            &decoded,
            &batch.public_inputs.old_root,
            &batch.public_inputs.new_root,
        )
        .expect("transfer SMT witnesses verify");
    }

    #[test]
    fn batch_from_transcripts_chains_repeated_balance_keys() {
        let first = sample_transcript();
        let second = sample_transcript();
        let batch = batch_from_transcripts(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_public_inputs(),
            [&first, &second],
        )
        .expect("batch");

        let encoded = batch
            .metadata
            .get(TRANSFER_TRANSCRIPTS_METADATA_KEY)
            .expect("transfer metadata");
        let decoded: Vec<TransferTranscript> =
            decode_from_bytes(encoded).expect("decode transcripts");
        let second_delta = &decoded[1].deltas[0];
        assert_eq!(second_delta.from_balance_before, Numeric::from(158u32));
        assert_eq!(second_delta.from_balance_after, Numeric::from(116u32));
        assert_eq!(second_delta.to_balance_before, Numeric::from(43u32));
        assert_eq!(second_delta.to_balance_after, Numeric::from(85u32));

        fastpq_prover::gadgets::transfer::verify_transcripts(&batch.transitions, &decoded)
            .expect("transfer transcript rows verify");
        fastpq_prover::gadgets::transfer::transcripts_to_witnesses(
            &decoded,
            &batch.public_inputs.old_root,
            &batch.public_inputs.new_root,
        )
        .expect("transfer SMT witnesses verify");
    }

    #[test]
    fn batch_from_transcripts_normalizes_mixed_scale_values() {
        let mut transcript = sample_transcript();
        transcript.deltas[0].amount = Numeric::new(5, 1);
        transcript.deltas[0].from_balance_before = Numeric::new(1, 0);
        transcript.deltas[0].from_balance_after = Numeric::new(5, 1);
        transcript.deltas[0].to_balance_before = Numeric::new(0, 0);
        transcript.deltas[0].to_balance_after = Numeric::new(5, 1);
        let batch = batch_from_transcripts(
            "fastpq-lane-balanced",
            sample_public_inputs(),
            [&transcript],
        )
        .expect("batch");
        let sender_row = batch
            .transitions
            .iter()
            .find(|row| {
                row.key
                    == balance_key(
                        &transcript.deltas[0].asset_definition,
                        &transcript.deltas[0].from_account,
                    )
            })
            .expect("sender row");
        let receiver_row = batch
            .transitions
            .iter()
            .find(|row| {
                row.key
                    == balance_key(
                        &transcript.deltas[0].asset_definition,
                        &transcript.deltas[0].to_account,
                    )
            })
            .expect("receiver row");

        assert_eq!(decode_le(&sender_row.pre_value), 10);
        assert_eq!(decode_le(&sender_row.post_value), 5);
        assert_eq!(decode_le(&receiver_row.pre_value), 0);
        assert_eq!(decode_le(&receiver_row.post_value), 5);
    }

    #[test]
    fn batch_from_transcripts_trims_padded_balance_scale() {
        let mut transcript = sample_transcript();
        transcript.deltas[0].amount = Numeric::new(11, 3);
        transcript.deltas[0].from_balance_before =
            Numeric::new(120_000_000_000_000_000_000_000_i128, 18);
        transcript.deltas[0].from_balance_after =
            Numeric::new(119_999_989_000_000_000_000_000_i128, 18);
        transcript.deltas[0].to_balance_before = Numeric::zero();
        transcript.deltas[0].to_balance_after = Numeric::new(11_000_000_000_000_000_i128, 18);

        let batch = batch_from_transcripts(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_public_inputs(),
            [&transcript],
        )
        .expect("batch");
        let sender_row = batch
            .transitions
            .iter()
            .find(|row| {
                row.key
                    == balance_key(
                        &transcript.deltas[0].asset_definition,
                        &transcript.deltas[0].from_account,
                    )
            })
            .expect("sender row");
        let receiver_row = batch
            .transitions
            .iter()
            .find(|row| {
                row.key
                    == balance_key(
                        &transcript.deltas[0].asset_definition,
                        &transcript.deltas[0].to_account,
                    )
            })
            .expect("receiver row");

        assert_eq!(decode_le(&sender_row.pre_value), 120_000_000);
        assert_eq!(decode_le(&sender_row.post_value), 119_999_989);
        assert_eq!(decode_le(&receiver_row.pre_value), 0);
        assert_eq!(decode_le(&receiver_row.post_value), 11);
    }

    #[test]
    fn batches_from_bundles_add_metadata() {
        let bundle = sample_bundle(Hash::prehashed([0x33; 32]));
        let batches = batches_from_bundles(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_template(),
            sample_tx_set_hash(),
            [&bundle],
        )
        .expect("batches");
        assert_eq!(batches.len(), 1);
        let batch = &batches[0];
        let entry_hex = batch
            .metadata
            .get(ENTRY_HASH_METADATA_KEY)
            .map(hex::encode)
            .expect("entry hash metadata");
        assert_eq!(entry_hex, hex::encode(bundle.entry_hash.as_ref()));
        let transcript_count_bytes = batch
            .metadata
            .get(TRANSCRIPT_COUNT_METADATA_KEY)
            .expect("transcript count metadata");
        assert_eq!(
            decode_le(transcript_count_bytes),
            bundle.transcripts.len() as u64
        );
    }

    #[test]
    fn batch_from_transcript_bundle_adds_axt_entry_metadata() {
        let bundle = sample_bundle(Hash::prehashed([0x34; 32]));
        let batch = batch_from_transcript_bundle(
            FASTPQ_CANONICAL_PARAMETER_SET,
            public_inputs_from_dto(&sample_public_inputs()),
            bundle.entry_hash,
            &bundle.transcripts,
        )
        .expect("batch");

        assert_eq!(batch.transitions.len(), 2);
        let entry_hex = batch
            .metadata
            .get(ENTRY_HASH_METADATA_KEY)
            .map(hex::encode)
            .expect("entry hash metadata");
        assert_eq!(entry_hex, hex::encode(bundle.entry_hash.as_ref()));
        assert!(
            batch
                .metadata
                .contains_key(TRANSFER_TRANSCRIPTS_METADATA_KEY)
        );
    }

    #[test]
    fn batches_from_exec_witness_match_bundle_order() {
        let bundle_a = sample_bundle(Hash::prehashed([0x41; 32]));
        let bundle_b = sample_bundle(Hash::prehashed([0x42; 32]));
        let bundles = [&bundle_a, &bundle_b];
        let built = batches_from_bundles(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_template(),
            sample_tx_set_hash(),
            bundles,
        )
        .expect("batches");
        let witness = ExecWitness {
            reads: Vec::new(),
            writes: Vec::new(),
            fastpq_transcripts: vec![bundle_a.clone(), bundle_b.clone()],
            fastpq_batches: built.iter().map(transition_batch_to_dto).collect(),
        };
        let batches = batches_from_exec_witness(&witness).expect("batches");
        assert_eq!(batches.len(), 2);
        let first_entry = hex::encode(
            batches[0]
                .metadata
                .get(ENTRY_HASH_METADATA_KEY)
                .expect("metadata"),
        );
        let second_entry = hex::encode(
            batches[1]
                .metadata
                .get(ENTRY_HASH_METADATA_KEY)
                .expect("metadata"),
        );
        assert_eq!(first_entry, hex::encode(bundle_a.entry_hash.as_ref()));
        assert_eq!(second_entry, hex::encode(bundle_b.entry_hash.as_ref()));
    }

    #[test]
    fn batches_from_exec_witness_rejects_missing_batches() {
        let bundle = sample_bundle(Hash::prehashed([0x43; 32]));
        let witness = ExecWitness {
            reads: Vec::new(),
            writes: Vec::new(),
            fastpq_transcripts: vec![bundle],
            fastpq_batches: Vec::new(),
        };
        let err = batches_from_exec_witness(&witness).expect_err("missing batches");
        assert!(matches!(err, TranscriptBatchError::MissingFastpqBatches));
    }

    #[test]
    fn batches_from_exec_witness_prefers_prebuilt_batches() {
        let transcript = sample_transcript();
        let batch = batch_from_transcripts(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_public_inputs(),
            [&transcript],
        )
        .unwrap();
        let dto = transition_batch_to_dto(&batch);
        let witness = ExecWitness {
            reads: Vec::new(),
            writes: Vec::new(),
            fastpq_transcripts: Vec::new(),
            fastpq_batches: vec![dto],
        };
        let batches = batches_from_exec_witness(&witness).expect("batches");
        assert_eq!(batches.len(), 1);
        assert_eq!(
            dto_transitions(&batches[0].transitions),
            dto_transitions(&batch.transitions)
        );
    }

    #[test]
    fn transition_batch_dto_roundtrip_preserves_metadata() {
        let transcript = sample_transcript();
        let mut batch = batch_from_transcripts(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_public_inputs(),
            [&transcript],
        )
        .unwrap();
        batch.public_inputs = PublicInputs {
            dsid: [0x11; 16],
            slot: 42,
            old_root: [0x22; 32],
            new_root: [0x33; 32],
            perm_root: [0x44; 32],
            tx_set_hash: [0x55; 32],
        };
        batch.metadata.insert("test".into(), vec![0xAA, 0xBB, 0xCC]);
        let dto = transition_batch_to_dto(&batch);
        let restored = transition_batch_from_dto(&dto);
        assert_eq!(restored.parameter, batch.parameter);
        assert_eq!(
            dto_transitions(&restored.transitions),
            dto_transitions(&batch.transitions)
        );
        assert_eq!(restored.public_inputs, batch.public_inputs);
        assert_eq!(restored.metadata, batch.metadata);
    }

    #[test]
    fn dto_batches_from_transcripts_embed_entry_hash() {
        let bundle = sample_bundle(Hash::prehashed([0x24; 32]));
        let mut map = BTreeMap::new();
        map.insert(bundle.entry_hash, bundle.transcripts.clone());
        let batches = dto_batches_from_transcripts(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_template(),
            sample_tx_set_hash(),
            &map,
        )
        .expect("dto");
        assert_eq!(batches.len(), 1);
        let entry_hex = hex::encode(
            batches[0]
                .metadata
                .get(ENTRY_HASH_METADATA_KEY)
                .expect("entry metadata"),
        );
        assert_eq!(entry_hex, hex::encode(Hash::prehashed([0x24; 32]).as_ref()));
    }

    fn dto_transitions(transitions: &[StateTransition]) -> Vec<FastpqStateTransition> {
        transitions.iter().map(state_transition_to_dto).collect()
    }

    fn decode_le(bytes: &[u8]) -> u64 {
        let mut chunk = [0u8; 8];
        chunk[..bytes.len()].copy_from_slice(bytes);
        u64::from_le_bytes(chunk)
    }

    struct DigestAccelerationGuard {
        previous: bool,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl DigestAccelerationGuard {
        fn new() -> Self {
            let lock = super::DIGEST_ACCELERATION_TEST_LOCK
                .lock()
                .expect("digest acceleration test lock poisoned");
            Self {
                previous: poseidon_digest_acceleration_enabled(),
                _lock: lock,
            }
        }
    }

    impl Drop for DigestAccelerationGuard {
        fn drop(&mut self) {
            set_poseidon_digest_acceleration_enabled(self.previous);
        }
    }

    fn fastpq_cfg(
        execution_mode: FastpqExecutionMode,
        poseidon_mode: FastpqPoseidonMode,
    ) -> Fastpq {
        Fastpq {
            execution_mode,
            poseidon_mode,
            proof_sidecar_queue_cap:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_QUEUE_CAP,
            proof_sidecar_max_bytes:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_MAX_BYTES,
            proof_sidecar_max_retries:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_MAX_RETRIES,
            device_class: None,
            chip_family: None,
            gpu_kind: None,
            metal_queue_fanout: None,
            metal_queue_column_threshold: None,
            metal_max_in_flight: None,
            metal_threadgroup_width: None,
            metal_trace: iroha_config::parameters::defaults::zk::fastpq::METAL_TRACE,
            metal_debug_enum: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_ENUM,
            metal_debug_fused: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_FUSED,
        }
    }

    fn sample_template() -> FastpqPublicInputsTemplate {
        FastpqPublicInputsTemplate {
            dsid: [0u8; 16],
            slot: 0,
            old_root: [0u8; 32],
            new_root: [0u8; 32],
            perm_root: [0u8; 32],
        }
    }

    fn sample_public_inputs() -> FastpqPublicInputs {
        sample_template().with_tx_set_hash(sample_tx_set_hash())
    }

    fn sample_tx_set_hash() -> [u8; 32] {
        [0xCC; 32]
    }

    fn sample_transcript() -> TransferTranscript {
        let asset = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        TransferTranscript {
            batch_hash: Hash::prehashed([0xAA; 32]),
            deltas: vec![TransferDeltaTranscript {
                from_account: (*ALICE_ID).clone(),
                to_account: (*BOB_ID).clone(),
                asset_definition: asset,
                amount: Numeric::from(42u32),
                from_balance_before: Numeric::from(200u32),
                from_balance_after: Numeric::from(158u32),
                to_balance_before: Numeric::from(1u32),
                to_balance_after: Numeric::from(43u32),
                from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
                to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
            }],
            authority_digest: authority_digest(&ALICE_ID),
            poseidon_preimage_digest: None,
        }
    }

    fn encoded_transfer_digest_preimage(
        delta: &TransferDeltaTranscript,
        batch_hash: &Hash,
    ) -> Vec<u8> {
        let mut encoded = Vec::new();
        delta.from_account.encode_to(&mut encoded);
        delta.to_account.encode_to(&mut encoded);
        delta.asset_definition.encode_to(&mut encoded);
        delta.amount.encode_to(&mut encoded);
        encoded.extend_from_slice(batch_hash.as_ref());
        encoded
    }

    fn sample_bundle(entry_hash: Hash) -> TransferTranscriptBundle {
        TransferTranscriptBundle {
            entry_hash,
            transcripts: vec![sample_transcript()],
        }
    }
}
