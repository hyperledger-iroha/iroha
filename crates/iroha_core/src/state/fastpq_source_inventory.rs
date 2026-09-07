//! Validator-owned FASTPQ inventory reconciled against applied execution sources.

use std::{collections::BTreeSet, sync::Arc};

use iroha_data_model::{
    fastpq::{
        FastpqOrdinarySourceStatementLeafV1, FastpqOrdinarySourceStatementManifestV1,
        FastpqSourceStatementContextV1,
    },
    transaction::TransactionEntrypoint,
};

use super::{BTreeMap, DataSpaceId, Hash, StateBlock};
use crate::{
    fastpq::{
        FastpqSourceExecutionEntryV1, FastpqSourceStatementBuildLimits,
        derive_fastpq_ordinary_source_manifest_v1,
    },
    queue::RoutingDecision,
};

mod content_verification;
pub(crate) mod owned_d7_capture;
mod public_seal;
use public_seal::{SourceTranscriptSeal, seal_public_transcripts};

/// Complete local source projection captured by a validator's block execution.
///
/// Entries are external calls in block order, then time invocations in invocation
/// order, then remaining applied transcript sources in ascending hash order. The
/// last group includes native purposes and internally derived calls. This is a
/// canonical source projection, not the physical order of state fragments.
/// External and time entries without transfers are retained, including rejected
/// entries. Native work without a transcript is not a proof source.
///
/// Private construction prevents a supplied archive from becoming an owned
/// inventory. The seal retains exact finalized public occurrences while excluding
/// private SMT paths. This local record is not a finality attestation or admission token.
/// Later applied transcript capture invalidates final witness capture. The seal
/// does not freeze unrelated ledger effects or authenticate source finality.
/// Final raw and retained ordinary transcript contents must match this seal before
/// capture, extraction or commit.
/// TODO: bind the source manifest to the ordinary-write commitment and source finality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastpqSourceInventoryV1 {
    source: FastpqSourceStatementContextV1,
    entries: Vec<FastpqSourceExecutionEntryV1>,
    transcript_entry_hashes: BTreeSet<Hash>,
    transcript_seal: SourceTranscriptSeal,
    tx_set_hash: [u8; 32],
}

impl FastpqSourceInventoryV1 {
    /// Frozen network and height of this inventory.
    pub const fn source(&self) -> FastpqSourceStatementContextV1 {
        self.source
    }

    /// Complete ordered source projection, including block entries without transfers.
    pub fn entries(&self) -> &[FastpqSourceExecutionEntryV1] {
        &self.entries
    }

    /// Exact applied transcript keys required by source-manifest construction.
    pub fn transcript_entry_hashes(&self) -> &BTreeSet<Hash> {
        &self.transcript_entry_hashes
    }

    /// Authoritative ordered canonical transaction-wire commitment supplied by block execution.
    pub const fn tx_set_hash(&self) -> [u8; 32] {
        self.tx_set_hash
    }

    /// Derive statements for the exact finalized public transcripts sealed by execution.
    ///
    /// Unlike the supplied-archive helper, removing an entry and its bundle cannot
    /// shrink this inventory. Construction bounds and the exact ordered public
    /// seal are checked before the strict producer. Supplied private paths are
    /// bounded but excluded from the seal because the producer derives its own paths.
    /// Callers must obtain slot and permission root from execution; this method
    /// alone does not authenticate those inputs or source finality.
    ///
    /// # Errors
    /// Rejects changed public occurrences, omitted or extra bundles, and every
    /// strict producer or resource error.
    pub fn derive_manifest(
        &self,
        slot: u64,
        perm_root: [u8; 32],
        transcripts: &BTreeMap<Hash, Vec<iroha_data_model::fastpq::TransferTranscript>>,
        limits: FastpqSourceStatementBuildLimits,
    ) -> Result<
        (
            FastpqOrdinarySourceStatementManifestV1,
            Vec<FastpqOrdinarySourceStatementLeafV1>,
        ),
        String,
    > {
        if !self.transcript_entry_hashes.iter().eq(transcripts.keys()) {
            return Err("FASTPQ manifest transcript keys differ from the owned inventory".into());
        }
        crate::fastpq::preflight_fastpq_source_transcripts(transcripts, limits)?;
        if seal_public_transcripts(transcripts)? != self.transcript_seal {
            return Err(
                "FASTPQ manifest public transcripts differ from the owned inventory seal".into(),
            );
        }
        derive_fastpq_ordinary_source_manifest_v1(
            self.source,
            &self.entries,
            slot,
            perm_root,
            self.tx_set_hash,
            transcripts,
            limits,
        )
    }
}

impl StateBlock<'_> {
    /// Seal the source inventory before draining this block's transcript accumulator.
    ///
    /// Both block execution paths supply their complete external entrypoints,
    /// validated routes and actual time-invocation hashes. Applied captures supply
    /// every other transcript source. A mismatch is latched and cannot be repaired
    /// by retrying with a smaller archive. No partially built inventory is published.
    #[cfg(test)]
    pub(crate) fn finalize_fastpq_source_inventory(
        &mut self,
        external: &[TransactionEntrypoint],
        routing: &[RoutingDecision],
        time_calls: &[Hash],
    ) -> Result<(), String> {
        self.finalize_fastpq_source_inventory_with_pending(external, routing, time_calls, None)
    }

    /// Finalize pending public digests and seal the still-owned transcript accumulator.
    ///
    /// A previous success or failure is preserved before any transcript mutation.
    /// The caller may drain transcripts only after this method succeeds. Private
    /// paths are excluded from the seal; supplied single-delta digests are checked
    /// fallibly before legacy finalization and missing digests are completed before
    /// the public facts are committed. Invalid supplied digests remain unchanged.
    pub(crate) fn finalize_fastpq_source_inventory_with_pending(
        &mut self,
        external: &[TransactionEntrypoint],
        routing: &[RoutingDecision],
        time_calls: &[Hash],
        pending: Option<crate::fastpq::PendingTransferTranscriptDigests>,
    ) -> Result<(), String> {
        if self.fastpq_source_inventory.is_some() {
            return Err("FASTPQ source inventory has already been finalized".into());
        }
        // Reject a malformed identity before digest finalization inspects a
        // precomputed digest against that identity. Preserve the original shape
        // error rather than exposing an assertion in a debug digest backend.
        let inventory = self
            .fastpq_tx_set_hash
            .filter(|hash| *hash != [0; 32])
            .ok_or_else(|| {
                crate::fastpq::TranscriptBatchError::MissingTransactionSetCommitment.to_string()
            })
            .and_then(|tx_set_hash| {
                self.validate_fastpq_source_transcript_shape()?;
                crate::fastpq::validate_precomputed_transfer_transcript_digests_in_map(
                    &self.fastpq_transcripts,
                )?;
                crate::fastpq::finalize_transfer_transcript_digests_in_map_with_pending(
                    &mut self.fastpq_transcripts,
                    pending,
                );
                let inventory =
                    self.build_fastpq_source_inventory(external, routing, time_calls, tx_set_hash)?;
                self.fastpq_source_captures
                    .seal()
                    .map_err(|error| error.to_string())?;
                Ok(inventory)
            });
        match inventory {
            Ok(inventory) => {
                self.fastpq_entry_dataspaces = inventory
                    .entries
                    .iter()
                    .map(|entry| (entry.entry_hash, entry.dataspace_id))
                    .collect();
                self.fastpq_source_inventory = Some(Ok(Arc::new(inventory)));
                Ok(())
            }
            Err(error) => {
                self.fastpq_source_inventory = Some(Err(error.clone()));
                Err(error)
            }
        }
    }

    fn validate_fastpq_source_transcript_shape(&self) -> Result<(), String> {
        for (hash, bundle) in &self.fastpq_transcripts {
            if bundle.is_empty()
                || bundle.iter().any(|transcript| {
                    transcript.batch_hash != *hash || transcript.deltas.is_empty()
                })
            {
                return Err(
                    "FASTPQ inventory contains an empty or misidentified transcript".into(),
                );
            }
        }
        Ok(())
    }

    fn build_fastpq_source_inventory(
        &self,
        external: &[TransactionEntrypoint],
        routing: &[RoutingDecision],
        time_calls: &[Hash],
        tx_set_hash: [u8; 32],
    ) -> Result<FastpqSourceInventoryV1, String> {
        if external.len() != routing.len() {
            return Err("FASTPQ external entrypoints do not align with validated routes".into());
        }
        let frozen = self
            .fastpq_source_context
            .as_ref()
            .ok_or_else(|| "FASTPQ inventory has no frozen source-height context".to_owned())?;
        let captures = self
            .captured_fastpq_transcript_sources()
            .map_err(ToString::to_string)?;
        if !captures.keys().eq(self.fastpq_transcripts.keys()) {
            return Err("FASTPQ applied source keys differ from the transcript accumulator".into());
        }
        let mut entries = Vec::new();
        let mut identities = BTreeSet::new();
        let mut push = |entry: FastpqSourceExecutionEntryV1| -> Result<(), String> {
            if !identities.insert(entry.entry_hash) {
                return Err(
                    "FASTPQ source inventory contains a duplicate execution identity".into(),
                );
            }
            if entries.len() >= u32::MAX as usize {
                return Err("FASTPQ source inventory entry count exceeds u32".into());
            }
            entries.push(entry);
            Ok(())
        };
        for (hash, lane, dataspace) in external
            .iter()
            .zip(routing)
            .map(|(entry, route)| {
                (
                    Hash::from(entry.execution_call_hash()),
                    Some(route.lane_id),
                    route.dataspace_id,
                )
            })
            .chain(
                time_calls
                    .iter()
                    .map(|hash| (*hash, None, DataSpaceId::UNIVERSAL)),
            )
        {
            let expected = frozen
                .capture_transcript(Some(hash), hash, lane, Some(dataspace), 0)
                .map_err(|error| error.to_string())?;
            if let Some(captured) = captures.get(&hash) {
                if captured.source() != expected.source()
                    || captured.route() != expected.route()
                    || captured.dataspace_id() != expected.dataspace_id()
                    || captured.execution_kind() != expected.execution_kind()
                {
                    return Err("FASTPQ block entry differs from its applied source context".into());
                }
            }
            push(FastpqSourceExecutionEntryV1 {
                entry_hash: hash,
                execution_kind: expected.execution_kind(),
                route: expected.route(),
                dataspace_id: dataspace,
            })?;
        }
        // The capture map is sorted by hash, independently of apply scheduling or
        // shared fragment indices. Do not infer physical execution order from it.
        for (hash, captured) in captures {
            if captured.source() != frozen.source || captured.entry_hash() != *hash {
                return Err("FASTPQ captured source differs from the frozen block scope".into());
            }
            if identities.contains(hash) {
                continue;
            }
            if !identities.insert(*hash) || entries.len() >= u32::MAX as usize {
                return Err("FASTPQ additional source identity or count is invalid".into());
            }
            entries.push(FastpqSourceExecutionEntryV1 {
                entry_hash: *hash,
                execution_kind: captured.execution_kind(),
                route: captured.route(),
                dataspace_id: captured.dataspace_id(),
            });
        }
        Ok(FastpqSourceInventoryV1 {
            source: frozen.source,
            entries,
            transcript_entry_hashes: captures.keys().copied().collect(),
            transcript_seal: seal_public_transcripts(&self.fastpq_transcripts)?,
            tx_set_hash,
        })
    }

    /// Read the sealed local inventory, or its first construction or final content failure.
    ///
    /// `None` means the execution path has not sealed an inventory. Authenticated
    /// replay does not manufacture a local inventory from advertised proof data.
    ///
    /// # Errors
    /// Returns the first latched construction or final witness content failure without
    /// exposing a partial or invalidated inventory.
    pub fn fastpq_source_inventory(&self) -> Result<Option<&FastpqSourceInventoryV1>, &str> {
        match &self.fastpq_source_inventory {
            None => Ok(None),
            Some(Ok(inventory)) => Ok(Some(inventory)),
            Some(Err(error)) => Err(error.as_str()),
        }
    }

    /// Require the same locally owned sealed source projection at final witness capture.
    ///
    /// Applied captures remain sealed after transcript draining. Any later applied
    /// occurrence invalidates this check, including another occurrence under an
    /// existing key. Empty or rolled-back transactions do not invent new sources.
    /// This method never reconstructs ownership from advertised witness contents.
    ///
    /// # Errors
    /// Rejects missing inventory, failed construction or final witness content verification,
    /// late applied captures,
    /// changed frozen source context, or disagreement with the retained source caches.
    pub(crate) fn verified_fastpq_source_inventory_for_capture(
        &self,
    ) -> Result<Arc<FastpqSourceInventoryV1>, String> {
        let inventory = match &self.fastpq_source_inventory {
            None => return Err("FASTPQ witness capture has no finalized owned inventory".into()),
            Some(Err(error)) => return Err(error.clone()),
            Some(Ok(inventory)) => inventory,
        };
        let captures = self
            .fastpq_source_captures
            .sealed_sources()
            .map_err(|error| error.to_string())?;
        let frozen = self.fastpq_source_context.as_ref().ok_or_else(|| {
            "FASTPQ witness capture has no frozen source-height context".to_owned()
        })?;
        if inventory.source != frozen.source
            || inventory.source.network_id != self.network_id
            || inventory.source.height != self._curr_block.height().get()
        {
            return Err(
                "FASTPQ witness capture differs from its sealed source-height context".into(),
            );
        }
        if !inventory.transcript_entry_hashes.iter().eq(captures.keys()) {
            return Err("FASTPQ witness capture keys differ from the owned inventory".into());
        }
        if self.fastpq_tx_set_hash != Some(inventory.tx_set_hash)
            || self.fastpq_entry_dataspaces.len() != inventory.entries.len()
        {
            return Err(
                "FASTPQ witness capture source caches differ from the owned inventory".into(),
            );
        }
        for entry in &inventory.entries {
            if self.fastpq_entry_dataspaces.get(&entry.entry_hash) != Some(&entry.dataspace_id) {
                return Err(
                    "FASTPQ witness capture dataspace cache differs from the owned inventory"
                        .into(),
                );
            }
            if let Some(captured) = captures.get(&entry.entry_hash) {
                if captured.source() != inventory.source
                    || captured.entry_hash() != entry.entry_hash
                    || captured.route() != entry.route
                    || captured.dataspace_id() != entry.dataspace_id
                    || captured.execution_kind() != entry.execution_kind
                {
                    return Err(
                        "FASTPQ witness capture execution source differs from the owned inventory"
                            .into(),
                    );
                }
            }
        }
        Ok(Arc::clone(inventory))
    }
}

#[cfg(test)]
mod capture_tests;
#[cfg(test)]
mod commit_tests;
#[cfg(test)]
mod content_capture_tests;
#[cfg(test)]
mod precomputed_digest_tests;
#[cfg(test)]
mod tests;
