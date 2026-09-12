//! Atomic pairing of incremental source frame lengths with per-occurrence semantics.
//!
//! This covers quantity arithmetic, repeated-key chronology and digest policy for
//! original deltas, plus exact canonical prefix frame lengths. It does not build
//! selected-scale rows, establish source authority, or reserve a State budget.
//! TODO: Consume this checked prefix from the real State-owned occurrence adapter
//! with matching world-state rollback and authenticated admission policy.

use fastpq_prover::gadgets::public_transfer_statement::{
    PendingQuantityPrefixUpdate, QuantityPrefixLimits, QuantityPrefixValidator,
};
use iroha_crypto::Hash;
use iroha_data_model::fastpq::TransferDeltaTranscript;

use super::{
    PrefixFrameLengths, PrefixLengthError, PrefixLengthLimits, SourcePrefixFrameSizer,
    checked_row_count,
};

/// Failure before either the sizing or semantic prefix is published.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CheckedPrefixError {
    /// Exact frame sizing, digest presence or an explicit sizing ceiling failed.
    #[error(transparent)]
    Length(#[from] PrefixLengthError),
    /// Full-domain quantity arithmetic, original digest or chronology failed.
    #[error(transparent)]
    Semantic(#[from] fastpq_prover::Error),
}

/// One checked original occurrence with incremental lengths and bounded chronology.
///
/// Retains canonical participant keys and their last comparison values, plus
/// constant-size sizing state. It retains no original delta or private witness.
/// The owner must preserve every previously measured value; later mutation
/// requires rebuilding this prefix. Construct a separate instance for each
/// transcript occurrence, even when several occurrences share an entry hash.
///
/// This result is not complete public preparation: selected-scale row order,
/// exact row matching, key/path allocation and final commitments remain with
/// the strict statement constructor. Error precedence may differ from it.
pub(crate) struct CheckedSourcePrefix {
    lengths: SourcePrefixFrameSizer,
    semantics: QuantityPrefixValidator,
}

/// Checked append held unpublished while its owner decides a reservation outcome.
///
/// The private fields retain an immutable original delta borrow and exclusive
/// borrows of both live prefixes. Dropping the value leaves both unchanged.
/// A State adapter may use the measured lengths to reserve before committing
/// this prefix; it still owns the matching ledger and world-state rollback.
#[must_use = "commit the checked prefix after reservation or drop it to preserve previous state"]
pub(crate) struct PendingCheckedPrefix<'a> {
    current_lengths: &'a mut SourcePrefixFrameSizer,
    next_lengths: SourcePrefixFrameSizer,
    semantics: PendingQuantityPrefixUpdate<'a>,
    measured: PrefixFrameLengths,
}

impl PendingCheckedPrefix<'_> {
    /// Exact complete prospective prefix lengths, not an execution-entry quota.
    pub(crate) const fn lengths(&self) -> PrefixFrameLengths {
        self.measured
    }

    /// Original unchanged delta borrowed throughout preparation and publication.
    pub(crate) fn delta(&self) -> &TransferDeltaTranscript {
        self.semantics.delta()
    }

    /// Publish both accepted prefixes after every fallible check has succeeded.
    /// This does not apply a reservation or mutate world state.
    pub(crate) fn commit(self) -> PrefixFrameLengths {
        self.semantics.commit();
        *self.current_lengths = self.next_lengths;
        self.measured
    }
}

impl CheckedSourcePrefix {
    /// Bind an empty occurrence to its exact headers and explicit inclusive caps.
    /// No runtime defaults, execution-entry identities or quota authority are selected.
    pub(crate) fn new(
        batch_hash: Hash,
        authority_digest: Hash,
        limits: PrefixLengthLimits,
    ) -> Result<Self, CheckedPrefixError> {
        let lengths = SourcePrefixFrameSizer::new(batch_hash, authority_digest, limits)?;
        // Every delta contains at most two distinct full participant keys. The
        // exact public cardinality is u32-bounded even on a wider local host.
        let maximum_rows =
            checked_row_count(limits.max_deltas.min((u32::MAX / 2) as usize))? as usize;
        let semantics = QuantityPrefixValidator::new(
            batch_hash,
            QuantityPrefixLimits {
                max_deltas: limits.max_deltas,
                max_unique_keys: maximum_rows,
            },
        );
        Ok(Self { lengths, semantics })
    }

    /// Prepare a single unchanged original delta without publishing either state.
    ///
    /// Byte limits reject before retaining any new semantic key. The pending
    /// semantic update exclusively borrows the validator and immutable delta;
    /// all fallible work finishes before either accepted prefix is published.
    /// Errors leave both prefixes unchanged. Allocator or serializer side effects
    /// are not transaction rollback, and this method does not undo ledger writes.
    pub(crate) fn prepare<'a>(
        &'a mut self,
        delta: &'a TransferDeltaTranscript,
        poseidon_preimage_digest: Option<Hash>,
    ) -> Result<PendingCheckedPrefix<'a>, CheckedPrefixError> {
        let mut lengths = self.lengths.clone();
        let measured = lengths.append(delta, poseidon_preimage_digest)?;
        let semantics = self.semantics.prepare(delta, poseidon_preimage_digest)?;
        Ok(PendingCheckedPrefix {
            current_lengths: &mut self.lengths,
            next_lengths: lengths,
            semantics,
            measured,
        })
    }

    /// Prepare and publish both prefixes when no intervening owner decision is needed.
    /// Runtime reservations should use [`Self::prepare`] and commit only after admission.
    pub(crate) fn append(
        &mut self,
        delta: &TransferDeltaTranscript,
        poseidon_preimage_digest: Option<Hash>,
    ) -> Result<PrefixFrameLengths, CheckedPrefixError> {
        Ok(self.prepare(delta, poseidon_preimage_digest)?.commit())
    }

    /// Last accepted nonempty prefix; an empty occurrence supplies no T or byte usage.
    pub(crate) const fn latest(&self) -> Option<PrefixFrameLengths> {
        self.lengths.latest()
    }
}

#[cfg(test)]
#[path = "checked/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "checked/reservation_tests.rs"]
mod reservation_tests;
