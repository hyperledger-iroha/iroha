//! Exact framing of a whole logical entry, independent of transfer-only semantics.
//!
//! Original transcript frames contribute I independently. Public occurrences and
//! participant rows contribute to two shared sequences in one statement, so M=S
//! is not the sum of independently framed occurrence statements. This measures
//! the existing quantity-statement shape; it proves neither that a statement can
//! be materialized nor that the supplied facts describe authorized execution.

use super::*;
use crate::fastpq::{FastpqSourceStatementBuildLimits, FastpqSourceTranscriptUsage};

/// Shape, codec, arithmetic or local construction-bound failure before publication.
#[derive(Debug, thiserror::Error)]
pub(crate) enum EntryFrameLengthError {
    /// All original occurrences must belong to the selected logical entry.
    #[error("source transcript belongs to a different logical entry")]
    EntryIdentity,
    /// An empty entry has no occurrences; it must not contain an empty occurrence.
    #[error("source entry contains an empty transcript occurrence")]
    EmptyOccurrence,
    /// Shared occurrence framing, including digest-presence and integer checks.
    #[error(transparent)]
    Prefix(#[from] PrefixLengthError),
    /// An explicit local construction ceiling was exceeded or is unrepresentable.
    #[error("source entry construction bound: {0}")]
    Bound(String),
}

/// Constant retained framing state for one complete logical entry.
///
/// An append counts one immutable original occurrence and retains no private paths,
/// public claims or participant keys. Clone restores these counters only, not State
/// or reservation ownership. Every occurrence must be appended once in entry order;
/// changing an earlier occurrence requires remeasurement of the complete bundle.
#[derive(Clone)]
pub(crate) struct SourceEntryFrameSizer {
    entry_hash: Hash,
    limits: FastpqSourceStatementBuildLimits,
    flags: u8,
    empty_sequence_bytes: usize,
    rows_bytes: usize,
    transcripts_bytes: usize,
    usage: FastpqSourceTranscriptUsage,
}

impl SourceEntryFrameSizer {
    /// Create an empty logical entry under explicit bounds, including its separate E=1.
    /// No policy, source or admission authority is inferred from the supplied hash.
    pub(crate) fn new(
        entry_hash: Hash,
        limits: FastpqSourceStatementBuildLimits,
    ) -> Result<Self, EntryFrameLengthError> {
        let usage = FastpqSourceTranscriptUsage::default();
        usage
            .check_limits(1, limits)
            .map_err(EntryFrameLengthError::Bound)?;
        let flags = canonical_flags(norito::core::default_encode_flags())?;
        let empty_sequence_bytes = SequencePayloadLength::new(flags)
            .map_err(PrefixLengthError::from)?
            .len();
        Ok(Self {
            entry_hash,
            limits,
            flags,
            empty_sequence_bytes,
            rows_bytes: empty_sequence_bytes,
            transcripts_bytes: empty_sequence_bytes,
            usage,
        })
    }

    /// Measure and append one whole original occurrence atomically.
    ///
    /// The entry hash, nonempty shape and singleton digest presence are checked.
    /// Digest values, arithmetic and chronology are deliberately not validated:
    /// intervening mint/burn work may make valid execution discontinuous in this
    /// transfer-only projection. Account identities and original quantity frames
    /// are still counted exactly. Fixed-width quantity row values and public
    /// context affect bytes but not their lengths. All counters remain unchanged
    /// on error; serializer-side resource charges are not rolled back.
    pub(crate) fn append(
        &mut self,
        transcript: &TransferTranscript,
    ) -> Result<FastpqSourceTranscriptUsage, EntryFrameLengthError> {
        if transcript.batch_hash != self.entry_hash {
            return Err(EntryFrameLengthError::EntryIdentity);
        }
        if transcript.deltas.is_empty() {
            return Err(EntryFrameLengthError::EmptyOccurrence);
        }
        if transcript.poseidon_preimage_digest.is_some() != (transcript.deltas.len() == 1) {
            return Err(PrefixLengthError::DigestShape.into());
        }
        let mut usage = self.usage;
        usage.transcripts = add(usage.transcripts, 1)?;
        usage.deltas = add(usage.deltas, transcript.deltas.len())?;
        checked_row_count(usage.deltas)?;
        usage
            .check_limits(1, self.limits)
            .map_err(EntryFrameLengthError::Bound)?;
        let mut occurrence = SourcePrefixFrameSizer::new(
            transcript.batch_hash,
            transcript.authority_digest,
            PrefixLengthLimits {
                max_deltas: subtract(self.limits.max_deltas, self.usage.deltas)?,
                max_input_frame_bytes: subtract(
                    self.limits.max_input_transcript_bytes,
                    self.usage.input_transcript_bytes,
                )?,
                max_public_statement_frame_bytes: self
                    .limits
                    .max_statement_bytes
                    .min(self.limits.max_total_statement_bytes),
            },
        )?;
        for (index, delta) in transcript.deltas.iter().enumerate() {
            // Intermediate singleton sizing needs the fixed Some(Hash) shape,
            // even when the final multi-delta occurrence carries None. Its value
            // is neither hashed nor emitted; the actual final presence was checked.
            occurrence.append(delta, (index == 0).then_some(transcript.batch_hash))?;
        }
        let measured = occurrence
            .latest()
            .ok_or(EntryFrameLengthError::EmptyOccurrence)?;
        usage.input_transcript_bytes =
            add(usage.input_transcript_bytes, measured.input_frame_bytes)?;
        let rows_bytes = add(
            self.rows_bytes,
            subtract(occurrence.rows.len(), self.empty_sequence_bytes)?,
        )?;
        let claim_bytes = add(
            occurrence.public_claim_fixed[usize::from(transcript.deltas.len() == 1)],
            field_span(occurrence.public_deltas.len(), self.flags)?,
        )?;
        // The canonical generic sequence has one fixed u64 count. Add only each
        // actual element's payload and prefix, never another sequence count or
        // another independently framed public statement.
        let transcripts_bytes = add(self.transcripts_bytes, field_span(claim_bytes, self.flags)?)?;
        let statement_bytes = add(
            occurrence.statement_fixed,
            add(
                field_span(rows_bytes, self.flags)?,
                field_span(transcripts_bytes, self.flags)?,
            )?,
        )?;
        usage.max_statement_bytes = statement_bytes;
        usage.total_statement_bytes = statement_bytes;
        usage
            .check_limits(1, self.limits)
            .map_err(EntryFrameLengthError::Bound)?;
        self.rows_bytes = rows_bytes;
        self.transcripts_bytes = transcripts_bytes;
        self.usage = usage;
        Ok(usage)
    }

    /// Exact currently accepted usage; an empty entry contributes no statement.
    pub(crate) const fn usage(&self) -> FastpqSourceTranscriptUsage {
        self.usage
    }
}

/// Measure one complete borrowed bundle, including chained committed/pending slices.
///
/// E is checked as one logical entry but remains owned separately by the reservation
/// ledger. No result escapes a failed traversal. This function does not validate
/// transfer semantics or grant execution, manifest, proof or finality authority.
pub(crate) fn measure_fastpq_source_entry_frame_usage<'a, I>(
    entry_hash: Hash,
    transcripts: I,
    limits: FastpqSourceStatementBuildLimits,
) -> Result<FastpqSourceTranscriptUsage, EntryFrameLengthError>
where
    I: IntoIterator<Item = &'a TransferTranscript>,
{
    let mut sizer = SourceEntryFrameSizer::new(entry_hash, limits)?;
    for transcript in transcripts {
        sizer.append(transcript)?;
    }
    Ok(sizer.usage())
}

// This file is loaded through `#[path]`, so a bare `mod tests;` would resolve to
// the parent's `source_prefix_lengths/tests.rs` instead of `entry/tests.rs`.
#[cfg(test)]
#[path = "entry/tests.rs"]
mod tests;
