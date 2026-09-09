//! Complete local source-manifest derivation from execution identities and finalized transcripts.
//!
//! This producer checks the supplied archive; it cannot authenticate its provenance.
//! The validator-owned inventory wraps this helper with its retained exact key set.
//! TODO: wire pre-commit manifest insertion, consensus capture/replay,
//! and finality-owned admission before using this output as production source authority.

use std::collections::{BTreeMap, BTreeSet};

use fastpq_prover::gadgets::public_transfer_statement::{
    PublicTransferLimits, TransferSmtBuildLimits,
};
use iroha_crypto::Hash;
use iroha_data_model::fastpq::{
    FastpqOrdinarySourceStatementLeafV1, FastpqOrdinarySourceStatementManifestV1,
    FastpqSourceStatementContextV1, TransferTranscript,
    build_fastpq_ordinary_source_statement_manifest_v1,
};

#[cfg(test)]
use iroha_data_model::{
    fastpq::{FastpqSourceExecutionKindV1, FastpqSourceRouteV1},
    nexus::DataSpaceId,
};

use super::{
    FastpqPublicInputsTemplate, dataspace_id_bytes, quantity_statement_from_finalized_transcripts,
};

pub use iroha_data_model::fastpq::FastpqSourceExecutionEntryV1;

/// Explicit local construction bounds; these do not select a production admission profile.
#[derive(Debug, Clone, Copy)]
pub struct FastpqSourceStatementBuildLimits {
    /// Maximum complete execution-entry count, including entries with no transcripts.
    pub max_executed_entries: u32,
    /// Maximum cumulative number of transcript occurrences.
    pub max_transcripts: usize,
    /// Maximum cumulative number of transfer deltas.
    pub max_deltas: usize,
    /// Maximum cumulative canonical transcript bytes, including supplied private paths.
    pub max_input_transcript_bytes: usize,
    /// Maximum canonical frame bytes for one public statement.
    pub max_statement_bytes: usize,
    /// Maximum cumulative canonical public-statement frame bytes.
    pub max_total_statement_bytes: usize,
}

/// Validate complete archive shape and cumulative input limits before ownership hashing.
/// Counts complete canonical frames, including supplied private paths, without allocating
/// their output buffers. Serializer-internal scratch is separate from that frame allocation.
fn measure_fastpq_source_transcript_inputs(
    transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
    limits: FastpqSourceStatementBuildLimits,
) -> Result<(usize, usize, usize), String> {
    u32::try_from(limits.max_transcripts)
        .map_err(|_| "FASTPQ source transcript limit exceeds u32".to_owned())?;
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let mut transcript_count = 0usize;
    let mut delta_count = 0usize;
    let mut input_bytes_remaining = limits.max_input_transcript_bytes;
    for (entry_hash, bundle) in transcripts {
        if bundle.is_empty() {
            return Err("FASTPQ transcript bundle is empty".into());
        }
        transcript_count = transcript_count
            .checked_add(bundle.len())
            .filter(|count| *count <= limits.max_transcripts)
            .ok_or_else(|| "FASTPQ transcript occurrence limit exceeded".to_owned())?;
        for transcript in bundle {
            if transcript.batch_hash != *entry_hash || transcript.deltas.is_empty() {
                return Err(
                    "FASTPQ transcript call identity differs from its bundle or has no deltas"
                        .into(),
                );
            }
            delta_count = delta_count
                .checked_add(transcript.deltas.len())
                .filter(|count| *count <= limits.max_deltas)
                .ok_or_else(|| "FASTPQ transfer-delta limit exceeded".to_owned())?;
            // This is the same real counting pass used by `to_bytes_bounded`, including
            // canonical header/alignment bytes and checked framing arithmetic. The concrete
            // immutable transcript needs only its measured length here, not an output frame.
            let encoded_bytes = norito::core::encoded_frame_len(transcript)
                .map_err(norito::core::BoundedEncodeError::from)
                .and_then(|encoded_bytes| {
                    if encoded_bytes > input_bytes_remaining {
                        Err(norito::core::BoundedEncodeError::FrameTooLarge {
                            encoded_bytes,
                            max_bytes: input_bytes_remaining,
                        })
                    } else {
                        Ok(encoded_bytes)
                    }
                })
                .map_err(|error| {
                    format!(
                        "FASTPQ canonical input transcript exceeds construction budget: {error}"
                    )
                })?;
            input_bytes_remaining -= encoded_bytes;
        }
    }
    Ok((
        transcript_count,
        delta_count,
        limits.max_input_transcript_bytes - input_bytes_remaining,
    ))
}

/// Validate complete archive shape and input caps without allocating output frames.
pub(crate) fn preflight_fastpq_source_transcripts(
    transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
    limits: FastpqSourceStatementBuildLimits,
) -> Result<usize, String> {
    measure_fastpq_source_transcript_inputs(transcripts, limits).map(|usage| usage.0)
}

mod budget;
use budget::measure_fastpq_source_statement_usage;

fn source_statement_public_limits(
    limits: FastpqSourceStatementBuildLimits,
) -> Result<PublicTransferLimits, String> {
    let max_rows = limits
        .max_deltas
        .checked_mul(2)
        .ok_or_else(|| "FASTPQ source construction row limit overflows".to_owned())?;
    // Preparation counts bare public fields, while the final statement cap counts
    // its complete model frame. Keep a separate finite scratch envelope derived
    // from the caller's input/output caps; enforce exact output bytes below.
    let max_public_bytes = limits
        .max_input_transcript_bytes
        .checked_add(limits.max_statement_bytes)
        .and_then(|bytes| bytes.checked_mul(2))
        .ok_or_else(|| "FASTPQ source preparation byte limit overflows".to_owned())?;
    Ok(PublicTransferLimits {
        max_transcripts: limits.max_transcripts,
        max_deltas: limits.max_deltas,
        max_rows,
        max_public_bytes,
        max_unique_keys: max_rows,
        max_allocation_steps: max_rows
            .checked_mul(4)
            .ok_or_else(|| "FASTPQ source allocation limit overflows".to_owned())?,
    })
}

/// Derive a complete ordinary source manifest without accepting caller-provided leaf digests.
///
/// The ordered archive includes entries without transfers. Every transcript-map key must
/// match exactly one entry; every nonempty entry bundle produces one leaf in original
/// entry order. Every transcript and delta remains whole and in original occurrence order.
/// Common scales, key allocation and repeated-key chronology span the complete bundle.
/// The supplied transaction-set commitment binds the ordered canonical transaction wires.
/// The manifest independently binds every ordered source entry, including entries without
/// transfers. Each statement uses its entry's dataspace plus the supplied slot and permission
/// root. Old/new roots are
/// derived by the strict finalized producer from the touched-balance tree, never from the
/// ordinary-write tree which will contain this manifest. Proof bytes are not an input.
///
/// All six source caps, including exact statement bytes, are checked with shared public
/// quantity preparation before constructing any private tree. No output roots are changed.
/// Counts and canonical input bytes are bounded before construction. Supplied private paths
/// are not reused. Full-domain rows and private tree work have derived construction caps. Public
/// statements are built and dropped one at a time under individual and cumulative byte caps.
/// These are local producer bounds, not a network decoder or a proof-verification budget.
/// All inputs remain unchanged on success or failure.
///
/// Callers must supply the complete validator-owned archive and authenticate the resulting
/// ordinary-write root separately. Removing an entry and its transcripts from both supplied
/// collections cannot be detected here. No compact profile or spend authority is granted.
///
/// # Errors
/// Rejects incomplete, duplicate or inconsistent execution identities, empty bundles/deltas,
/// exceeded bounds, and any rewrite of the finalized public transcript projection.
/// TODO: support intervening supply/permission/metadata changes in the final complete
/// execution relation. A transfer-only bundle with discontinuous balances is rejected;
/// it is never split into separately accepted per-transcript leaves.
pub fn derive_fastpq_ordinary_source_manifest_v1(
    source: FastpqSourceStatementContextV1,
    entries: &[FastpqSourceExecutionEntryV1],
    slot: u64,
    perm_root: [u8; 32],
    tx_set_hash: [u8; 32],
    transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
    limits: FastpqSourceStatementBuildLimits,
) -> Result<
    (
        FastpqOrdinarySourceStatementManifestV1,
        Vec<FastpqOrdinarySourceStatementLeafV1>,
    ),
    String,
> {
    if tx_set_hash == [0; 32] {
        return Err(super::TranscriptBatchError::MissingTransactionSetCommitment.to_string());
    }
    let executed_entry_count = u32::try_from(entries.len())
        .map_err(|_| "FASTPQ executed-entry count exceeds u32".to_owned())?;
    if source.height == 0 || executed_entry_count > limits.max_executed_entries {
        return Err("FASTPQ source height or executed-entry count is invalid".into());
    }
    if transcripts.len() > entries.len() {
        return Err("FASTPQ transcript archive contains more bundles than executed entries".into());
    }
    let identities: BTreeSet<_> = entries.iter().map(|entry| entry.entry_hash).collect();
    if identities.len() != entries.len() {
        return Err("FASTPQ execution archive contains duplicate call identities".into());
    }
    // Canonical framing matches the artifact identity digest, independently of ambient codec flags.
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    if transcripts.keys().any(|key| !identities.contains(key)) {
        return Err("FASTPQ transcript bundle has no executed entry".into());
    }
    measure_fastpq_source_statement_usage(executed_entry_count, transcripts, limits)?;
    let max_rows = limits
        .max_deltas
        .checked_mul(2)
        .ok_or_else(|| "FASTPQ source construction row limit overflows".to_owned())?;
    let public_limits = source_statement_public_limits(limits)?;
    let tree_limits = TransferSmtBuildLimits::for_update_limit(max_rows)
        .ok_or_else(|| "FASTPQ source tree limits overflow".to_owned())?;
    let max_statements = u32::try_from(limits.max_transcripts)
        .map_err(|_| "FASTPQ source transcript limit exceeds u32".to_owned())?;
    // The complete-bundle preflight above precedes every leaf/tree allocation.
    let mut leaves = Vec::with_capacity(transcripts.len());
    let mut output_bytes_remaining = limits.max_total_statement_bytes;
    for (entry_index, entry) in entries.iter().enumerate() {
        let Some(bundle) = transcripts.get(&entry.entry_hash) else {
            continue;
        };
        let inputs = FastpqPublicInputsTemplate {
            dsid: dataspace_id_bytes(entry.dataspace_id),
            slot,
            old_root: [0; 32],
            new_root: [0; 32],
            perm_root,
        }
        .with_tx_set_hash(tx_set_hash);
        let entry_transcript_count = u32::try_from(bundle.len())
            .map_err(|_| "FASTPQ entry transcript count exceeds u32".to_owned())?;
        let produced = quantity_statement_from_finalized_transcripts(
            inputs,
            bundle,
            public_limits,
            tree_limits,
        )
        .map_err(|error| format!("FASTPQ source entry {entry_index} is not finalized: {error}"))?;
        let bytes = norito::core::to_bytes_bounded(
            produced.statement(),
            limits.max_statement_bytes.min(output_bytes_remaining),
        )
        .map_err(|error| {
            format!("FASTPQ canonical source statement exceeds construction budget: {error}")
        })?;
        output_bytes_remaining -= bytes.len();
        leaves.push(FastpqOrdinarySourceStatementLeafV1 {
            source,
            // Every count was checked against u32 before materializing any leaf.
            statement_index: leaves.len() as u32,
            entry_index: entry_index as u32,
            entry_transcript_count,
            entry_hash: entry.entry_hash,
            execution_kind: entry.execution_kind,
            route: entry.route,
            dataspace_id: entry.dataspace_id,
            statement_digest: Hash::new(&bytes).into(),
        });
    }
    let manifest = build_fastpq_ordinary_source_statement_manifest_v1(
        source,
        entries,
        &leaves,
        limits.max_executed_entries,
        max_statements,
    )
    .ok_or_else(|| "FASTPQ derived source manifest is inconsistent".to_owned())?;
    Ok((manifest, leaves))
}

#[cfg(test)]
mod tests;
