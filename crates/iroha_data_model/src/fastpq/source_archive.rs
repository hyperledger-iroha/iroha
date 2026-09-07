//! Bounded complete ordinary source archives with one shared manifest inclusion path.
//!
//! These transport values prove consistency with caller-supplied source facts and an
//! ordinary-write root. They do not authenticate finality, execution completeness,
//! storage durability or spending authority.
//! TODO: integrate the source-specific durable archive owner and authenticated finality admission.

use super::{
    FastpqOrdinarySourceStatementLeafV1, FastpqOrdinarySourceStatementManifestV1,
    FastpqSourceExecutionEntryV1, FastpqSourceStatementContextV1,
    build_fastpq_ordinary_source_statement_manifest_v1,
    verify_fastpq_ordinary_source_statement_manifest_write_v1,
};
use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::{NoritoDeserialize, NoritoSerialize};

/// First-release ordinary source archive payload version.
pub const FASTPQ_ORDINARY_SOURCE_STATEMENT_ARCHIVE_VERSION_V1: u16 = 1;

/// Complete untrusted leaf archive and one shared ordinary-write inclusion path.
///
/// An empty leaf sequence still carries its actual manifest and path. Statement
/// preimages, private transfer paths and per-leaf membership proofs are not included.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
#[norito(schema_name = "iroha_data_model::fastpq::FastpqOrdinarySourceStatementArchiveV1")]
pub struct FastpqOrdinarySourceStatementArchiveV1 {
    /// Must equal [`FASTPQ_ORDINARY_SOURCE_STATEMENT_ARCHIVE_VERSION_V1`].
    pub version: u16,
    /// Advertised manifest matching the complete ordered leaf sequence.
    pub manifest: FastpqOrdinarySourceStatementManifestV1,
    /// Every original statement leaf in its exact canonical occurrence order.
    pub leaves: Vec<FastpqOrdinarySourceStatementLeafV1>,
    /// Fixed ordinary-write SMT path for the manifest, leaf level first.
    pub manifest_siblings: [Hash; 256],
}

/// Caller-owned resource limits; no production policy or defaults are selected here.
#[derive(Debug, Clone, Copy)]
pub struct FastpqSourceArchiveDecodeLimits {
    /// Complete canonical archive wire bytes, checked before decoding its header.
    pub max_wire_bytes: usize,
    /// Complete executed-entry count, including entries without a source leaf.
    pub max_executed_entries: u32,
    /// Complete leaf count; also clamps variable-sequence allocation during decoding.
    pub max_statements: u32,
    /// Per-field and cumulative codec budgets, preserving any stricter outer scope.
    pub norito: norito::DecodeLimits,
}

/// Validate a complete in-memory archive against separately expected source facts/root.
///
/// Complete leaf order, counts and contents are rebuilt against the independently
/// expected complete ordered source entries before manifest inclusion. Entries with
/// no leaf remain bound by the manifest's source-entry digest. Callers must obtain
/// authority for these entries and the source/root independently. This consistency
/// check does not establish finality, completeness of supplied expectations,
/// durability, or succinct proof verification.
#[must_use]
pub fn verify_fastpq_ordinary_source_statement_archive_v1(
    archive: &FastpqOrdinarySourceStatementArchiveV1,
    expected_source: FastpqSourceStatementContextV1,
    expected_entries: &[FastpqSourceExecutionEntryV1],
    expected_ordinary_root: Hash,
    max_executed_entries: u32,
    max_statements: u32,
) -> bool {
    if archive.version != FASTPQ_ORDINARY_SOURCE_STATEMENT_ARCHIVE_VERSION_V1
        || archive.manifest.source != expected_source
    {
        return false;
    }
    let Some(rebuilt) = build_fastpq_ordinary_source_statement_manifest_v1(
        expected_source,
        expected_entries,
        &archive.leaves,
        max_executed_entries,
        max_statements,
    ) else {
        return false;
    };
    archive.manifest == rebuilt
        && verify_fastpq_ordinary_source_statement_manifest_write_v1(
            &archive.manifest,
            expected_source,
            &archive.manifest_siblings,
            expected_ordinary_root,
            max_executed_entries,
            max_statements,
        )
}

/// Decode and validate one canonical nominal archive under explicit caller limits.
///
/// Only the leaf vector is a variable-length sequence in this fixed payload graph.
/// Its sequence limit is narrowed to the statement cap before allocation; fixed
/// hash arrays and the 256-sibling path do not consume that variable-sequence cap.
/// Other codec limits and all enclosing cumulative charges remain effective.
/// The separately supplied complete entry projection is capped before traversal.
/// It is not transported in this leaf-only payload and must obtain independent
/// authority. Successful validation does not authenticate these source expectations
/// or the ordinary-write root, and is not a succinct proof-verification API.
///
/// # Errors
/// Rejects oversized/noncanonical/wrong-schema frames, exceeded codec or leaf caps,
/// unsupported versions, incomplete/malformed leaf archives and incorrect inclusion.
pub fn decode_fastpq_ordinary_source_statement_archive_v1(
    bytes: &[u8],
    expected_source: FastpqSourceStatementContextV1,
    expected_entries: &[FastpqSourceExecutionEntryV1],
    expected_ordinary_root: Hash,
    limits: FastpqSourceArchiveDecodeLimits,
) -> Result<FastpqOrdinarySourceStatementArchiveV1, norito::Error> {
    if bytes.len() > limits.max_wire_bytes {
        return Err(norito::Error::Message(
            "FASTPQ source archive exceeds wire-byte limit".into(),
        ));
    }
    if u32::try_from(expected_entries.len())
        .ok()
        .is_none_or(|count| count > limits.max_executed_entries)
    {
        return Err(norito::Error::Message(
            "FASTPQ expected source entries exceed the executed-entry limit".into(),
        ));
    }
    let leaf_cap = usize::try_from(limits.max_statements).unwrap_or(usize::MAX);
    let narrowed = norito::DecodeLimits::new(
        limits.norito.max_sequence_elements().min(leaf_cap),
        limits.norito.max_field_bytes(),
        limits.norito.max_total_elements(),
        limits.norito.max_total_allocated_bytes(),
        limits.norito.max_nesting_depth(),
    );
    let archive = norito::decode_canonical_with_limits(bytes, narrowed)?;
    if !verify_fastpq_ordinary_source_statement_archive_v1(
        &archive,
        expected_source,
        expected_entries,
        expected_ordinary_root,
        limits.max_executed_entries,
        limits.max_statements,
    ) {
        return Err(norito::Error::Message(
            "FASTPQ source archive does not match its expected source and ordinary root".into(),
        ));
    }
    Ok(archive)
}

#[cfg(test)]
mod tests;
