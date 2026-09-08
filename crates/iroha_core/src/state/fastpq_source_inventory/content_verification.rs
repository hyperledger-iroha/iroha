//! Exact final recorder and ordinary-witness contents checked against the private source seal.
//!
//! These checks borrow the complete original public fields. They do not repair digests, sort
//! supplied bundles, construct a tree or trace, or inspect private paths. Exact owned counts
//! bound occurrence traversal before public hashing; the local seal does not define a new
//! untrusted-network byte budget. The caller owns atomic recorder draining and failure latching.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_data_model::fastpq::{TransferTranscript, TransferTranscriptBundle};

use super::{
    FastpqSourceInventoryV1,
    public_seal::{SourceTranscriptSeal, seal_public_transcript_bundles, seal_public_transcripts},
};

impl FastpqSourceInventoryV1 {
    /// Verify the exact finalized recorder map before any digest repair or witness publication.
    ///
    /// The caller must first require intact execution-owned source context and keep recorder
    /// validation and draining within the same lock. This method does not mutate the map or
    /// authenticate a supplied archive; it compares it with this privately retained execution seal.
    ///
    /// # Errors
    /// Rejects incomplete keys or counts, malformed ordinary identities, and any substituted
    /// public fact, including a missing finalized digest or changed atomic grouping.
    pub(crate) fn verify_finalized_transcript_map(
        &self,
        transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
    ) -> Result<(), String> {
        self.preflight_final_transcript_entries(
            transcripts
                .iter()
                .map(|(key, bundle)| (key, bundle.as_slice())),
        )?;
        self.verify_final_public_seal(seal_public_transcripts(transcripts)?)
    }

    /// Verify retained ordinary witness bundles without sorting or copying their contents.
    ///
    /// Ordinary bundle keys must already be strictly ordered, unique and equal to the owned
    /// execution-source key set. Each inner transcript must retain that same identity. Autonomous
    /// merge-lane evidence uses a different outer-identity rule and is not accepted by this API.
    ///
    /// # Errors
    /// Rejects any outer/inner identity, shape, count, ordering or public-content disagreement.
    pub(crate) fn verify_ordinary_witness_bundles(
        &self,
        bundles: &[TransferTranscriptBundle],
    ) -> Result<(), String> {
        self.preflight_final_transcript_entries(
            bundles
                .iter()
                .map(|bundle| (&bundle.entry_hash, bundle.transcripts.as_slice())),
        )?;
        self.verify_final_public_seal(seal_public_transcript_bundles(bundles)?)
    }

    fn preflight_final_transcript_entries<'a>(
        &self,
        entries: impl ExactSizeIterator<Item = (&'a Hash, &'a [TransferTranscript])>,
    ) -> Result<(), String> {
        if entries.len() != self.transcript_entry_hashes.len() {
            return Err(
                "FASTPQ final witness transcript keys differ from the owned inventory".into(),
            );
        }
        let mut transcript_count = 0;
        let mut delta_count = 0;
        for ((key, bundle), expected_key) in entries.zip(&self.transcript_entry_hashes) {
            // Comparison with the exact sorted set rejects both order changes and duplicate keys.
            if key != expected_key {
                return Err(
                    "FASTPQ final witness transcript keys differ from the owned inventory".into(),
                );
            }
            if bundle.is_empty() {
                return Err("FASTPQ final witness contains an empty transcript bundle".into());
            }
            transcript_count = checked_owned_count(
                transcript_count,
                bundle.len(),
                self.transcript_seal.transcript_count,
                "transcript",
            )?;
            for transcript in bundle {
                if transcript.batch_hash != *key || transcript.deltas.is_empty() {
                    return Err(
                        "FASTPQ final witness ordinary identity or delta shape is invalid".into(),
                    );
                }
                delta_count = checked_owned_count(
                    delta_count,
                    transcript.deltas.len(),
                    self.transcript_seal.delta_count,
                    "delta",
                )?;
            }
        }
        if transcript_count != self.transcript_seal.transcript_count
            || delta_count != self.transcript_seal.delta_count
        {
            return Err(
                "FASTPQ final witness occurrence counts differ from the owned inventory".into(),
            );
        }
        Ok(())
    }

    fn verify_final_public_seal(&self, observed: SourceTranscriptSeal) -> Result<(), String> {
        if observed != self.transcript_seal {
            return Err(
                "FASTPQ final witness public content differs from the owned inventory seal".into(),
            );
        }
        Ok(())
    }
}

fn checked_owned_count(total: u64, next: usize, owned: u64, kind: &str) -> Result<u64, String> {
    u64::try_from(next)
        .ok()
        .and_then(|next| total.checked_add(next))
        .filter(|total| *total <= owned)
        .ok_or_else(|| format!("FASTPQ final witness {kind} count exceeds the owned inventory"))
}

#[cfg(test)]
mod tests;
