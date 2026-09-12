//! Prepare one exact local transcript occurrence before its movement is applied.

use super::*;

/// Already allocated transcript and the source facts observed at its preparation boundary.
/// This private value cannot be transported between transaction owners by a caller.
struct PreparedTransferOccurrence {
    transcript: TransferTranscript,
    capture: Result<
        crate::fastpq::FastpqCapturedTranscriptSource,
        crate::fastpq::FastpqSourceCaptureError,
    >,
}

impl StateTransaction<'_, '_> {
    fn prepare_transfer_occurrence(
        &self,
        authority: &AccountId,
        batch_hash: Hash,
        deltas: Vec<TransferDeltaTranscript>,
    ) -> Option<PreparedTransferOccurrence> {
        if deltas.is_empty() {
            return None;
        }
        let authority_digest = crate::fastpq::authority_digest(authority);
        let poseidon_preimage_digest = match deltas.as_slice() {
            [delta] => Some(crate::fastpq::poseidon_preimage_digest(delta, &batch_hash)),
            _ => None,
        };
        Some(PreparedTransferOccurrence {
            transcript: TransferTranscript {
                batch_hash,
                deltas,
                authority_digest,
                poseidon_preimage_digest,
            },
            capture: self.fastpq_source_context.capture_transcript(
                self.tx_call_hash,
                batch_hash,
                self.current_lane_id,
                self.current_dataspace_id,
                *self.committed_fragments,
            ),
        })
    }

    fn stage_transfer_occurrence(&mut self, occurrence: Option<PreparedTransferOccurrence>) {
        let Some(PreparedTransferOccurrence {
            transcript,
            capture,
        }) = occurrence
        else {
            return;
        };
        self.pending_fastpq_source_captures.record(capture);
        crate::sumeragi::witness::record_fastpq_transcript(&transcript);
        self.pending_transfer_transcripts.push(transcript);
    }

    /// Preserve immediate recording for existing single and batch transcript owners.
    pub(super) fn stage_transfer_transcripts_with_batch_hash(
        &mut self,
        authority: &AccountId,
        batch_hash: Hash,
        deltas: Vec<TransferDeltaTranscript>,
    ) {
        let occurrence = self.prepare_transfer_occurrence(authority, batch_hash, deltas);
        self.stage_transfer_occurrence(occurrence);
    }

    /// Prepare an exact occurrence, run its movement, then stage it only on success.
    ///
    /// The numeric movement caller supplies its exact prechecked delta and resolves its
    /// purpose identity immediately before this call. Its callback must keep execution
    /// identity, route and fragment context unchanged. The current callback only applies
    /// the prepared balance/control plan; receiver admission occurs before this boundary.
    /// No prepared occurrence escapes the exclusive transaction borrow.
    ///
    /// Captured source errors keep their existing sticky-on-success semantics. This does
    /// not validate source completeness, reserve a budget, or roll back callback writes.
    /// The callback's existing transaction rollback contract remains authoritative.
    ///
    /// # Errors
    ///
    /// Returns the movement error without staging its occurrence. No fallible operation
    /// is added after a successful callback; ordinary infallible collection allocation
    /// and witness recording remain part of staging.
    pub(crate) fn apply_with_prepared_transfer_transcripts<T>(
        &mut self,
        authority: &AccountId,
        batch_hash: Hash,
        deltas: Vec<TransferDeltaTranscript>,
        apply: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let occurrence = self.prepare_transfer_occurrence(authority, batch_hash, deltas);
        let applied = apply(self)?;
        self.stage_transfer_occurrence(occurrence);
        Ok(applied)
    }
    /// Prepare accepted deltas incrementally and publish one occurrence after the body succeeds.
    ///
    /// The independent-batch caller keeps its interleaved preparation and balance/control
    /// application order. It appends each exact accepted delta immediately before that
    /// leg's plan is applied, and propagates every preparation or apply error out of the
    /// whole callback. A preparation error must not become an independent-leg rejection.
    /// Rejected leg preparation must not append. These ordering and stable execution-context
    /// requirements are caller obligations; the scoped `FnMut` does not enforce them by type.
    ///
    /// The initial capture result is retained without publication. The first accepted delta
    /// receives its singleton digest before mutation; a second accepted delta clears that
    /// digest. The same Vec, values and capture result are staged once after whole-body
    /// success. An empty body never stages a transcript or capture error.
    ///
    /// The supplied capacity is an inclusive accepted-delta bound derived from the
    /// instruction's entry count. Storage grows only for accepted deltas; rejected legs
    /// do not preallocate a full transcript. Exceeding the bound rejects the whole body,
    /// even if a callback incorrectly ignores its preparation error.
    ///
    /// TODO: reserve the exact complete-entry source budget at this fallible preparation
    /// boundary. Callback mutations still require the caller's transaction rollback;
    /// final accumulator/witness/pending-vector publication can still allocate.
    ///
    /// # Errors
    ///
    /// Propagates the callback error without publishing the prepared occurrence. The
    /// caller must preserve its existing transaction and witness rollback boundaries.
    pub(crate) fn apply_with_incremental_transfer_transcripts<T>(
        &mut self,
        authority: &AccountId,
        batch_hash: Hash,
        capacity: usize,
        apply: impl FnOnce(
            &mut Self,
            &mut dyn FnMut(TransferDeltaTranscript) -> Result<(), Error>,
        ) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let capture = self.fastpq_source_context.capture_transcript(
            self.tx_call_hash,
            batch_hash,
            self.current_lane_id,
            self.current_dataspace_id,
            *self.committed_fragments,
        );
        let mut empty_deltas = Vec::new();
        let mut occurrence: Option<PreparedTransferOccurrence> = None;
        let mut preparation_failed = false;
        let bound_error = || {
            Error::InvariantViolation("transfer transcript exceeds its declared entry bound".into())
        };
        let applied = {
            let mut append = |delta: TransferDeltaTranscript| {
                let count = occurrence
                    .as_ref()
                    .map_or(0, |entry| entry.transcript.deltas.len());
                if preparation_failed || count >= capacity {
                    preparation_failed = true;
                    return Err(bound_error());
                }
                if let Some(occurrence) = &mut occurrence {
                    occurrence.transcript.poseidon_preimage_digest = None;
                    occurrence.transcript.deltas.push(delta);
                } else {
                    let authority_digest = crate::fastpq::authority_digest(authority);
                    let poseidon_preimage_digest =
                        Some(crate::fastpq::poseidon_preimage_digest(&delta, &batch_hash));
                    empty_deltas.push(delta);
                    occurrence = Some(PreparedTransferOccurrence {
                        transcript: TransferTranscript {
                            batch_hash,
                            deltas: core::mem::take(&mut empty_deltas),
                            authority_digest,
                            poseidon_preimage_digest,
                        },
                        capture,
                    });
                }
                Ok(())
            };
            apply(self, &mut append)?
        };
        if preparation_failed {
            return Err(bound_error());
        }
        self.stage_transfer_occurrence(occurrence);
        Ok(applied)
    }
}

#[cfg(test)]
mod tests;
