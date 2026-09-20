//! Native prefix outputs retain their actual captures in the same global overlay.
//!
//! Only actual execution constructs the private prefix seal. The common inventory
//! rejoins retained rows and captures before sealing; no portable source claim can
//! recreate either owner. Actual witness capture and exact durable finality must
//! join the common seal before State publication; runtime input admission remains
//! gated on complete global source and resource qualification.

use super::{
    BTreeMap, Hash, MergeLedgerCommitError, StateBlock, TransactionEntrypoint,
    TransferTranscriptBundle, lane_decision_execution::PreexecutedLaneDecisionGroupV1,
};
use crate::fastpq::FastpqCapturedTranscriptSource;

/// Locally executed native subset, including exact absence of transfer outputs.
/// No serialization, public constructor, consensus proof or publication authority.
pub(super) struct NativeLaneFastpqSeal {
    rows: BTreeMap<Hash, NativeLaneFastpqRowSeal>,
}

struct NativeLaneFastpqRowSeal {
    entry_hash: Hash,
    transcript_hash: Option<Hash>,
    capture: Option<FastpqCapturedTranscriptSource>,
}

/// Hash one actual finalized vector, preserving call/outer identity and absence.
fn native_row_hash(
    call: Hash,
    entry: Hash,
    transcripts: Option<&Vec<iroha_data_model::fastpq::TransferTranscript>>,
) -> Result<Option<Hash>, String> {
    transcripts
        .map(|rows| {
            if rows.is_empty() || rows.iter().any(|row| row.batch_hash != call) {
                return Err("native FASTPQ rows have invalid execution-call ownership".into());
            }
            let bytes = norito::encode_canonical(rows).map_err(|error| error.to_string())?;
            Ok(Hash::new_from_chunks(&[
                b"iroha:native-fastpq:private-prefix:v1\0",
                call.as_ref(),
                entry.as_ref(),
                &bytes,
            ]))
        })
        .transpose()
}

impl StateBlock<'_> {
    /// Join every native input, including zero-transcript rejections, to the
    /// common producer's exact ordered source inventory and captured prefix.
    pub(super) fn verify_native_owned_fastpq_output_join(
        &self,
        sources: &super::output_capacity::OwnedExecutionSources,
    ) -> Result<(), String> {
        let Some((batch, _)) = self
            .native_lane_stage_for_inventory()
            .map_err(|error| error.to_string())?
        else {
            return Err("native owned inventory has no actual stage".into());
        };
        if !sources.is_native()
            || sources.proposal() != self._curr_block.hash()
            || sources.network_routes().len() != batch.groups.len()
            || sources.entries().len() < batch.groups.len()
        {
            return Err("native owned inventory lost its exact input positions".into());
        }
        for ((group, source), route) in batch
            .groups
            .iter()
            .zip(sources.entries())
            .zip(sources.network_routes())
        {
            let input = &group.payload.input;
            if source.call() != Hash::from(input.entrypoint.execution_call_hash())
                || *route != input.routing_plan()?.coordinator_route()
                || source.lane() != Some(route.lane_id)
                || source.dataspace() != route.dataspace_id
            {
                return Err("native owned inventory substituted a source or route".into());
            }
        }
        self.verify_native_lane_fastpq_output_join(&[], &[])
    }

    /// Finalize and snapshot selected native outputs without draining either owner.
    /// All binding/capture/digest checks precede the only mutation (digest completion).
    pub(super) fn retain_native_lane_fastpq_outputs(
        &mut self,
        entrypoints: &[TransactionEntrypoint],
    ) -> Result<Vec<TransferTranscriptBundle>, MergeLedgerCommitError> {
        let (bindings, selected) = self.lane_fastpq_transcript_selection(entrypoints)?;
        self.fastpq_source_captures
            .validate_unsealed_selection(&selected)
            .map_err(|error| MergeLedgerCommitError::ExecutionDivergence(error.to_string()))?;
        let mut finalized = selected
            .iter()
            .map(|hash| {
                self.fastpq_transcripts
                    .get(hash)
                    .cloned()
                    .map(|transcripts| (*hash, transcripts))
                    .ok_or_else(|| {
                        MergeLedgerCommitError::ExecutionDivergence(
                            "native FASTPQ selection lost its actual transcript owner".into(),
                        )
                    })
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        crate::fastpq::validate_precomputed_transfer_transcript_digests_in_map(&finalized)
            .map_err(MergeLedgerCommitError::ExecutionDivergence)?;
        crate::fastpq::finalize_transfer_transcript_digests_in_map(&mut finalized);
        // Keep actual rows under execution-call identities. The private result
        // snapshot uses outer input identity, including for sealed reveals.
        for (call, transcripts) in &finalized {
            self.fastpq_transcripts.insert(*call, transcripts.clone());
        }
        Ok(bindings
            .into_iter()
            .filter_map(|(entry_hash, call_hash)| {
                finalized
                    .remove(&call_hash)
                    .map(|transcripts| TransferTranscriptBundle {
                        entry_hash,
                        transcripts,
                    })
            })
            .collect())
    }

    /// Seal only actual constructor outputs joined to the still-owned native map.
    /// The optional capture is copied from execution custody, never reconstructed.
    pub(super) fn seal_native_lane_fastpq_outputs(
        &self,
        executions: &[PreexecutedLaneDecisionGroupV1],
    ) -> Result<NativeLaneFastpqSeal, MergeLedgerCommitError> {
        let invalid = MergeLedgerCommitError::ExecutionDivergence;
        let captures = self
            .captured_fastpq_transcript_sources()
            .map_err(|error| invalid(error.to_string()))?;
        let mut rows = BTreeMap::new();
        for execution in executions {
            let entry = &execution.source.payload.input.entrypoint;
            let entry_hash = Hash::from(entry.hash());
            let call = Hash::from(entry.execution_call_hash());
            let actual = self.fastpq_transcripts.get(&call);
            let snapshots = &execution.fastpq_transcripts;
            if snapshots.len() != usize::from(actual.is_some())
                || snapshots.first().is_some_and(|snapshot| {
                    snapshot.entry_hash != entry_hash || Some(&snapshot.transcripts) != actual
                })
            {
                return Err(invalid(
                    "native FASTPQ actual output snapshot differs from its retained rows".into(),
                ));
            }
            let capture = captures.get(&call).copied();
            if actual.is_some() != capture.is_some() {
                return Err(invalid(
                    "native FASTPQ output and applied capture ownership differ".into(),
                ));
            }
            let transcript_hash = native_row_hash(call, entry_hash, actual).map_err(invalid)?;
            if rows
                .insert(
                    call,
                    NativeLaneFastpqRowSeal {
                        entry_hash,
                        transcript_hash,
                        capture,
                    },
                )
                .is_some()
            {
                return Err(invalid(
                    "native FASTPQ prefix repeats an execution call".into(),
                ));
            }
        }
        self.fastpq_source_captures
            .validate_unsealed_selection(
                &rows
                    .iter()
                    .filter_map(|(call, row)| row.capture.is_some().then_some(*call))
                    .collect(),
            )
            .map_err(|error| invalid(error.to_string()))?;
        Ok(NativeLaneFastpqSeal { rows })
    }

    /// Rejoin exact native rows and captures before common inventory sealing.
    /// Additional Time/protocol/nested calls retain the existing common owner.
    pub(super) fn verify_native_lane_fastpq_output_join(
        &self,
        external: &[TransactionEntrypoint],
        routing: &[crate::queue::RoutingDecision],
    ) -> Result<(), String> {
        let Some((batch, seal)) = self
            .native_lane_stage_for_inventory()
            .map_err(|error| error.to_string())?
        else {
            return Ok(());
        };
        if !external.is_empty() || !routing.is_empty() {
            return Err("native source inventory cannot accept competing external inputs".into());
        }
        if seal.rows.len() != batch.groups.len() {
            return Err("native FASTPQ seal lost exact source cardinality".into());
        }
        let captures = self
            .captured_fastpq_transcript_sources()
            .map_err(|error| error.to_string())?;
        for group in &batch.groups {
            let input = &group.payload.input.entrypoint;
            let call = Hash::from(input.execution_call_hash());
            let row = seal
                .rows
                .get(&call)
                .ok_or_else(|| "native FASTPQ seal lost its exact source call".to_owned())?;
            if row.entry_hash != Hash::from(input.hash())
                || row.transcript_hash
                    != native_row_hash(call, row.entry_hash, self.fastpq_transcripts.get(&call))?
            {
                return Err("native FASTPQ rows differ from the exact executed prefix".into());
            }
            if row.capture != captures.get(&call).copied() {
                return Err("native FASTPQ captures differ from the exact executed prefix".into());
            }
        }
        Ok(())
    }
}
