//! Authenticated autonomous merge sources retain one shared execution owner.
//!
//! Source authentication precedes construction. The prefix owns its exact inputs,
//! bounded actual rows, captures and remaining carrier budget until common sealing.
//! Scratch callers suppress recording around the whole disposable block; applying
//! carriers record the prefix through the same transactional witness machinery.

use super::output_capacity::{ExecutionOutputPlanState, OwnedExecutionSources};
use super::*;
use crate::queue::RoutingDecision;
use crate::tx::{AcceptedTransaction, execution_rejection_from_admission_failure};
use iroha_data_model::block::{
    execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1},
    output_budget::{ExecutionOutputBudget, ExecutionOutputPhase, ExecutionOutputTerminalCeilings},
};
use std::{borrow::Cow, sync::Arc};

/// Completed original prefix and its unconsumed internal-tail reservation.
/// It cannot be constructed from a transcript/result projection or deserialized.
pub(super) struct MergeExecutionPrefix {
    seal: Arc<MergeExecutionPrefixSeal>,
    budget: Option<ExecutionOutputBudget>,
    inventory: Option<Arc<FastpqSourceInventoryV1>>,
}

/// Immutable actual execution custody retained through the common output seal.
pub(super) struct MergeExecutionPrefixSeal {
    application_header: BlockHeader,
    recorder: Option<crate::sumeragi::witness::ExecWitnessCaptureIdentity>,
    execution_digest: Hash,
    inputs: Vec<TransactionEntrypoint>,
    routes: Vec<RoutingDecision>,
    rows: Vec<ExecutionOutputV1>,
    row_bytes: u64,
    sources: BTreeMap<Hash, MergeSourceSeal>,
}

struct MergeSourceSeal {
    outer: Hash,
    transcript: Option<Hash>,
    capture: Option<crate::fastpq::FastpqCapturedTranscriptSource>,
}

fn transcripts_hash(
    rows: Option<&Vec<iroha_data_model::fastpq::TransferTranscript>>,
) -> Result<Option<Hash>, String> {
    rows.map(|rows| {
        norito::encode_canonical(rows)
            .map(Hash::new)
            .map_err(|e| e.to_string())
    })
    .transpose()
}

impl MergeExecutionPrefixSeal {
    /// Verify this exact executed archive against the original certified entry
    /// after the consuming carrier owner moves that entry out of mutable State.
    pub(super) fn verify_carrier_entry(
        &self,
        source: &SignedBlock,
        entry: &MergeLedgerEntry,
    ) -> Result<(), String> {
        let batch = entry
            .execution_batch
            .as_ref()
            .ok_or("merge prefix belongs to a control-only merge")?;
        let reference = source
            .execution_context()
            .and_then(|context| context.merge_entry.as_ref())
            .ok_or("merge prefix carrier lost its certified reference")?;
        if !reference.matches_entry(entry)
            || crate::merge::merge_application_header_from_carrier(&source.header())
                != self.application_header
            || batch.application_block_header != self.application_header
            || crate::merge::merge_execution_root(&batch.lanes) != self.execution_digest
        {
            return Err("merge prefix differs from its exact certified carrier".into());
        }
        Ok(())
    }
    pub(super) fn inputs(&self) -> &[TransactionEntrypoint] {
        &self.inputs
    }
    pub(super) fn routes(&self) -> &[RoutingDecision] {
        &self.routes
    }
    pub(super) fn row_bytes(&self) -> u64 {
        self.row_bytes
    }

    fn verify_archive(
        &self,
        transcripts: &BTreeMap<Hash, Vec<iroha_data_model::fastpq::TransferTranscript>>,
        captures: &BTreeMap<Hash, crate::fastpq::FastpqCapturedTranscriptSource>,
    ) -> Result<(), String> {
        if self.inputs.len() != self.rows.len() || self.inputs.len() != self.sources.len() {
            return Err("merge prefix lost its complete original input/output ownership".into());
        }
        for input in &self.inputs {
            let call = Hash::from(input.execution_call_hash());
            let bound = self
                .sources
                .get(&call)
                .ok_or("merge prefix source disappeared")?;
            if bound.outer != Hash::from(input.hash())
                || bound.transcript != transcripts_hash(transcripts.get(&call))?
                || bound.capture != captures.get(&call).copied()
            {
                return Err("merge prefix source or original transcript changed".into());
            }
        }
        Ok(())
    }
}

/// One linear prefix owner; any failure/unwind poisons the disposable carrier.
/// The constructor is called only after the full source/QC/QueuePlan/AMX pass.
pub(super) struct MergePrefixOwner<'owner, 'state> {
    pub(super) state: &'owner mut StateBlock<'state>,
    inputs: Vec<TransactionEntrypoint>,
    routes: Vec<RoutingDecision>,
    admissions:
        Vec<Option<Result<(), iroha_data_model::transaction::error::TransactionRejectionReason>>>,
    quarantine_overflow: Vec<bool>,
    quarantine_policy: (usize, u64),
    rows: Vec<ExecutionOutputV1>,
    budget: Option<ExecutionOutputBudget>,
    witness: Option<crate::sumeragi::witness::ExecWitnessOverlay>,
    recorder: Option<crate::sumeragi::witness::ExecWitnessCaptureIdentity>,
    finished: bool,
    pending_obligations: Vec<(HashOf<TransactionEntrypoint>, Hash)>,
}

/// Reproduce the complete immutable source/QC/QueuePlan/AMX authentication
/// before the execution owner can retain E or publish any physical fragment.
fn authenticate_sources(
    state_block: &StateBlock<'_>,
    sources: &[MergeExecutionSource],
    replay: Option<&crate::block::VerifiedReplayProposal>,
) -> Result<Vec<(HashOf<TransactionEntrypoint>, Hash)>, MergeLedgerCommitError> {
    let mut seen_entrypoints = BTreeSet::new();
    let mut seen_reservations = BTreeSet::new();
    let mut pending_obligations = Vec::new();
    for source in sources {
        crate::kura::Kura::validate_certified_lane_block_artifact(&source.certified)
            .map_err(|message| MergeLedgerCommitError::ExecutionBatchInvalid(message.to_owned()))?;
        crate::kura::Kura::validate_lane_block_execution_input_artifact(&source.input)
            .map_err(|message| MergeLedgerCommitError::ExecutionBatchInvalid(message.to_owned()))?;
        let Some((source_network_id, source_epoch, source_payload_hash)) =
            source.input.source.autonomous_binding()
        else {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "autonomous merge source carries a global-block execution source".to_owned(),
            ));
        };
        let authenticated_bundle = crate::kura::Kura::decode_autonomous_lane_merge_bundle(
            &source.source_bundle,
            source_network_id,
            source_epoch,
        )
        .map_err(|message| MergeLedgerCommitError::ExecutionBatchInvalid(message.to_owned()))?;
        if authenticated_bundle.certified != source.certified
            || authenticated_bundle.bundle_hash().ok() != Some(source.bundle_hash)
        {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "merge source differs from its exact availability-certified bundle".to_owned(),
            ));
        }
        let authenticated_payload = authenticated_bundle.executable_payload();
        if source.origin_proposal != authenticated_payload.origin_proposal
            || source_payload_hash != authenticated_payload.payload_hash
            || source.input.entrypoint_hashes != authenticated_payload.entrypoint_hashes
            || source.input.entrypoints != authenticated_payload.entrypoints
            || source.input.reservation_keys != authenticated_payload.reservation_keys
            || source.input.routing_plans != authenticated_payload.routing_plans
            || source.input.native_amx_receipts != authenticated_payload.native_amx_receipts
        {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "durable execution input differs from its producer-authenticated payload"
                    .to_owned(),
            ));
        }
        if source.certified.proposal != source.input.proposal {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "certified lane proposal differs from execution input".to_owned(),
            ));
        }
        if source.input.reservation_keys.len() != source.input.entrypoints.len()
            || source.input.routing_plans.len() != source.input.entrypoints.len()
            || source.input.native_amx_receipts.len() != source.input.entrypoints.len()
        {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                    "autonomous merge input does not bind one reservation and routing plan per entrypoint"
                        .to_owned(),
                ));
        }
        if source.input.entrypoints.iter().any(|entrypoint| {
            entrypoint.admission_intent()
                != iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced
        }) {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "autonomous merge entrypoint does not carry QueuePlanSynced admission intent"
                    .to_owned(),
            ));
        }
        for hash in &source.input.entrypoint_hashes {
            if !seen_entrypoints.insert(*hash) {
                return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                    "duplicate entrypoint across merge execution lanes".to_owned(),
                ));
            }
        }
        let reservation_descriptor = &source.origin_proposal.descriptor;
        for (((entrypoint, reservation), bound_plan), native_amx_receipt) in source
            .input
            .entrypoints
            .iter()
            .zip(&source.input.reservation_keys)
            .zip(&source.input.routing_plans)
            .zip(&source.input.native_amx_receipts)
        {
            let descriptor = &source.input.proposal.descriptor;
            let entrypoint_hash = Hash::from(entrypoint.hash());
            let canonical_entrypoint_hash = entrypoint.hash();
            if bound_plan.coordinator_route()
                != RoutingDecision::new(descriptor.lane_id, descriptor.dataspace_id)
                || bound_plan.digest() != reservation.routing_plan_digest
                || bound_plan.coordinator_leg() != reservation.coordinator_leg
                || reservation.entrypoint_hash != canonical_entrypoint_hash
                || Hash::from(reservation.entrypoint_hash) != entrypoint_hash
                || !matches!(
                    queue_plan_admission_registry_match(
                        state_block,
                        reservation.entrypoint_hash.clone(),
                        reservation.queue_plan_admission_binding_hash,
                    ),
                    Ok(QueuePlanAdmissionRegistryMatch::Exact)
                )
                || reservation.lane_id != descriptor.lane_id
                || reservation.dataspace_id != descriptor.dataspace_id
                || reservation.lane_incarnation != reservation_descriptor.lane_incarnation
                || reservation.proposal_height != reservation_descriptor.proposal_height
                || reservation.lane_block_height != reservation_descriptor.lane_block_height
                || reservation.lane_block_view != reservation_descriptor.lane_block_view
                || !seen_reservations.insert(reservation.digest())
            {
                return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                    "autonomous merge reservation or routing-plan binding mismatch".to_owned(),
                ));
            }
            pending_obligations.push((
                reservation.entrypoint_hash.clone(),
                reservation.queue_plan_admission_binding_hash,
            ));
            if !crate::native_amx::receipt_shape_matches_coordinator_payload(
                native_amx_receipt.as_ref(),
                bound_plan,
                reservation.entrypoint_hash.as_ref(),
                entrypoint_hash,
                source_network_id,
                &source.origin_proposal,
            ) {
                return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                    "native-AMX receipt shape does not match its authenticated routing plan"
                        .to_owned(),
                ));
            }
            if let Some(receipt) = native_amx_receipt {
                let mut source_id = [0u8; Hash::LENGTH];
                source_id.copy_from_slice(reservation.entrypoint_hash.as_ref());
                let expected_v2_context =
                    crate::block::expected_native_amx_v2_context_from_receipt(
                        receipt,
                        source_epoch,
                    )
                    .map_err(|message| {
                        MergeLedgerCommitError::ExecutionBatchInvalid(format!(
                            "invalid availability-certified native-AMX context: {message}"
                        ))
                    })?;
                let replay_authority = replay
                    .map(|token| token.native_amx_authority(&*state_block))
                    .transpose()
                    .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?;
                let receipt_authority: &dyn crate::block::NativeAmxAuthorityContext =
                    match replay_authority.as_ref() {
                        Some(authority) => authority,
                        None => &*state_block,
                    };
                crate::block::validate_native_amx_receipt_against_plan(
                    receipt,
                    &source.origin_proposal,
                    entrypoint.hash(),
                    bound_plan,
                    source_id,
                    source_network_id,
                    &state_block.nexus.dataspace_catalog,
                    receipt_authority,
                    Some(expected_v2_context),
                )
                .map_err(|message| {
                    MergeLedgerCommitError::ExecutionBatchInvalid(format!(
                        "invalid availability-certified native-AMX receipt: {message}"
                    ))
                })?;
            }
        }
    }
    Ok(pending_obligations)
}

impl<'owner, 'state> MergePrefixOwner<'owner, 'state> {
    pub(super) fn new(
        state: &'owner mut StateBlock<'state>,
        authenticated: &[MergeExecutionSource],
        replay: Option<&crate::block::VerifiedReplayProposal>,
    ) -> Result<Self, MergeLedgerCommitError> {
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        if state.start_of_block_effects_applied
            || state.execution_output_plan.is_some()
            || state.merge_execution_prefix.is_some()
        {
            return Err(invalid(
                "merge prefix does not own a pristine carrier".into(),
            ));
        }
        state
            .fastpq_source_quota
            .as_ref()
            .ok_or_else(|| invalid("merge source quota is absent".into()))?
            .as_ref()
            .map_err(|error| invalid(error.clone()))?
            .verify_ordinary_entries(std::iter::empty())
            .map_err(invalid)?;
        let (profile, output) = state.fastpq_source_policy_at_block_start();
        let count = authenticated.iter().try_fold(0_u32, |count, source| {
            count
                .checked_add(
                    u32::try_from(source.input.entrypoints.len())
                        .map_err(|_| invalid("merge source count exceeds u32".into()))?,
                )
                .ok_or_else(|| invalid("merge source count overflows u32".into()))
        })?;
        if count > profile.maximum_network_inputs(output).map_err(invalid)? {
            return Err(invalid(
                "merge prefix exceeds frozen FASTPQ source capacity".into(),
            ));
        }
        let phases = ExecutionOutputTerminalCeilings::derive()
            .map_err(invalid)?
            .envelope(output.max_pipeline_triggers, output.max_time_invocations)
            .reservations(count, &output.limits())
            .map_err(invalid)?;
        let budget = ExecutionOutputBudget::new(output.limits(), phases).map_err(invalid)?;
        let pending_obligations = authenticate_sources(state, authenticated, replay)?;
        let mut inputs = Vec::new();
        let mut routes = Vec::new();
        let mut admissions = Vec::new();
        let mut rows = Vec::new();
        inputs
            .try_reserve_exact(count as usize)
            .map_err(|_| invalid("host cannot retain complete merge inputs".into()))?;
        routes
            .try_reserve_exact(count as usize)
            .map_err(|_| invalid("host cannot retain merge routes".into()))?;
        admissions
            .try_reserve_exact(count as usize)
            .map_err(|_| invalid("host cannot retain merge admissions".into()))?;
        rows.try_reserve_exact(count as usize)
            .map_err(|_| invalid("host cannot retain merge outputs".into()))?;
        let mut calls = BTreeSet::new();
        let mut outer = BTreeSet::new();
        let mut signed = BTreeSet::new();
        let mut commitments = BTreeSet::new();
        let mut quarantined = Vec::new();
        for source in authenticated {
            for (input, plan) in source
                .input
                .entrypoints
                .iter()
                .zip(&source.input.routing_plans)
            {
                if !outer.insert(input.hash())
                    || !calls.insert(Hash::from(input.execution_call_hash()))
                    || crate::tx::exact_signed_transaction_hash(input)
                        .is_some_and(|hash| !signed.insert(hash))
                {
                    return Err(invalid(
                        "merge prefix repeats an outer or execution identity".into(),
                    ));
                }
                let commitment = match input {
                    TransactionEntrypoint::SealedCommitment(value) => Some(*value.commitment()),
                    TransactionEntrypoint::SealedReveal(value) => Some(value.commitment),
                    TransactionEntrypoint::External(_) => None,
                };
                if commitment.is_some_and(|hash| !commitments.insert(hash)) {
                    return Err(invalid("merge prefix repeats a sealed commitment".into()));
                }
                let binding = State::pending_queue_plan_binding_for_execution(
                    &*state,
                    input,
                    plan,
                    state._curr_block.height().get(),
                )
                .map_err(invalid)?
                .ok_or_else(|| invalid("merge input lost its exact pending admission".into()))?;
                let parameters = state.world.parameters.get();
                let admission = AcceptedTransaction::accept_borrowed_entrypoint_at_time(
                    input,
                    &state.network_id,
                    parameters.sumeragi().max_clock_drift(),
                    parameters.transaction(),
                    state.crypto.as_ref(),
                    std::time::Duration::from_millis(binding.enqueue_timestamp_ms),
                )
                .map(|_| ())
                .map_err(execution_rejection_from_admission_failure);
                let route = plan.coordinator_route();
                let signed_input = match input {
                    TransactionEntrypoint::External(tx) => Some(tx),
                    TransactionEntrypoint::SealedReveal(reveal) => {
                        Some(reveal.signed_transaction())
                    }
                    TransactionEntrypoint::SealedCommitment(_) => None,
                };
                let admission = if signed_input
                    .is_some_and(|tx| tx.creation_time() >= state._curr_block.creation_time())
                {
                    Err(iroha_data_model::transaction::error::TransactionRejectionReason::Validation(iroha_data_model::ValidationFail::NotPermitted(
                        "merge input creation time must precede its actual carrier".into(),
                    )))
                } else {
                    admission
                };
                if admission.is_ok()
                    && signed_input.is_some_and(crate::tx::is_quarantine_transaction)
                {
                    quarantined.push((input.hash(), inputs.len()));
                }
                inputs.push(input.clone());
                routes.push(route);
                admissions.push(Some(admission));
            }
        }
        if inputs.len() != count as usize {
            return Err(invalid(
                "merge prefix lost an authenticated routing position".into(),
            ));
        }
        let quarantine_policy = (
            state.pipeline.quarantine_max_txs_per_block,
            state.pipeline.quarantine_tx_max_cycles,
        );
        let mut quarantine_overflow = Vec::new();
        quarantine_overflow
            .try_reserve_exact(inputs.len())
            .map_err(|_| {
                invalid("host cannot retain complete merge quarantine admission".into())
            })?;
        quarantine_overflow.resize(inputs.len(), false);
        quarantined.sort_unstable();
        for (_, index) in quarantined
            .into_iter()
            .skip(state.pipeline.quarantine_max_txs_per_block)
        {
            quarantine_overflow[index] = true;
        }
        state.execution_output_plan = Some(ExecutionOutputPlanState::Running);
        Ok(Self {
            state,
            inputs,
            routes,
            admissions,
            quarantine_overflow,
            quarantine_policy,
            rows,
            budget: Some(budget),
            witness: Some(crate::sumeragi::witness::begin_exec_witness_overlay()),
            recorder: crate::sumeragi::witness::current_exec_witness_capture_identity(),
            finished: false,
            pending_obligations,
        })
    }

    pub(super) fn execute_lane(
        &mut self,
        source: &MergeExecutionSource,
        cache: &mut crate::smartcontracts::ivm::cache::IvmCache,
    ) -> Result<Vec<(u64, HashOf<TransactionEntrypoint>, TransactionResult)>, MergeLedgerCommitError>
    {
        let invalid = MergeLedgerCommitError::ExecutionDivergence;
        if self.quarantine_policy
            != (
                self.state.pipeline.quarantine_max_txs_per_block,
                self.state.pipeline.quarantine_tx_max_cycles,
            )
        {
            return Err(invalid(
                "merge quarantine policy changed after source admission".into(),
            ));
        }
        let mut results = Vec::new();
        results
            .try_reserve_exact(source.input.entrypoints.len())
            .map_err(|_| invalid("host cannot retain merge results".into()))?;
        for (execution_index, input) in source
            .input
            .proposal
            .descriptor
            .accepted_candidate_indices
            .iter()
            .copied()
            .zip(&source.input.entrypoints)
        {
            let index = self.rows.len();
            if self.inputs.get(index) != Some(input) {
                return Err(invalid(
                    "merge attempt differs from its authenticated prefix order".into(),
                ));
            }
            let admitted = self.admissions[index]
                .take()
                .ok_or_else(|| invalid("merge admission consumed twice".into()))?
                .map(|()| AcceptedTransaction::new_unchecked_entrypoint(Cow::Borrowed(input)));
            let output_index = u32::try_from(index)
                .map_err(|_| invalid("merge output index exceeds u32".into()))?;
            let reservation = self
                .budget
                .as_mut()
                .ok_or_else(|| invalid("merge prefix lost output budget".into()))?
                .begin(ExecutionOutputV1::network_output_limit_rejection(
                    output_index,
                ))
                .map_err(invalid)?;
            let height = self.state._curr_block.height().get();
            let row = output_capacity::execute_network_attempt(
                self.state,
                self.inputs.as_slice(),
                input,
                output_index,
                execution_index,
                height,
                self.routes[index],
                admitted,
                self.quarantine_overflow[index],
                reservation,
                cache,
            )
            .map_err(invalid)?;
            let ExecutionOutputV1::Network(NetworkExecutionOutputV1 { result, .. }) = &row else {
                return Err(invalid(
                    "merge signed attempt returned a non-Network row".into(),
                ));
            };
            results.push((execution_index, input.hash(), result.clone()));
            self.rows.push(row);
        }
        Ok(results)
    }

    pub(super) fn resolve_pending(
        &mut self,
        signed: BTreeSet<HashOf<SignedTransaction>>,
    ) -> Result<(), MergeLedgerCommitError> {
        self.state.resolve_required_queue_plan_pending_obligations(
            std::mem::take(&mut self.pending_obligations),
            signed,
        )
    }

    pub(super) fn finish(
        mut self,
        executions: &[MergeLaneExecution],
    ) -> Result<(), MergeLedgerCommitError> {
        let invalid = MergeLedgerCommitError::ExecutionDivergence;
        if self.rows.len() != self.inputs.len() || self.admissions.iter().any(Option::is_some) {
            return Err(invalid("merge prefix is incomplete".into()));
        }
        if executions
            .iter()
            .map(|lane| lane.results.len())
            .sum::<usize>()
            != self.rows.len()
        {
            return Err(invalid(
                "merge result projection lost an executed source".into(),
            ));
        }
        if !executions
            .iter()
            .flat_map(|lane| &lane.entrypoints)
            .eq(&self.inputs)
        {
            return Err(invalid(
                "merge archive changed its exact original input ordering".into(),
            ));
        }
        for lane in executions {
            let expected = self
                .state
                .retain_native_lane_fastpq_outputs(&lane.entrypoints)?;
            if expected.as_slice() != lane.fastpq_transcripts.as_ref() {
                return Err(invalid(
                    "merge archive changed an original transcript snapshot".into(),
                ));
            }
        }
        let actual_results = executions.iter().flat_map(|lane| &lane.results);
        if !actual_results.zip(&self.rows).all(|(result, row)| {
            matches!(row,
            ExecutionOutputV1::Network(output) if &output.result == result)
        }) {
            return Err(invalid(
                "merge result projection differs from its shared output owner".into(),
            ));
        }
        let captures = self
            .state
            .captured_fastpq_transcript_sources()
            .map_err(|e| invalid(e.to_string()))?;
        let mut sources = BTreeMap::new();
        for input in &self.inputs {
            let call = Hash::from(input.execution_call_hash());
            let rows = self.state.fastpq_transcripts.get(&call);
            let capture = captures.get(&call).copied();
            if rows.is_some() != capture.is_some() {
                return Err(invalid(
                    "merge prefix transcript/capture custody differs".into(),
                ));
            }
            sources.insert(
                call,
                MergeSourceSeal {
                    outer: input.hash().into(),
                    transcript: transcripts_hash(rows).map_err(invalid)?,
                    capture,
                },
            );
        }
        let row_bytes = self.rows.iter().try_fold(0_u64, |sum, row| {
            let bytes = u64::try_from(
                norito::canonical_frame_len(row).map_err(|e| invalid(e.to_string()))?,
            )
            .map_err(|_| invalid("merge row length exceeds u64".into()))?;
            sum.checked_add(bytes)
                .ok_or_else(|| invalid("merge row byte sum overflows".into()))
        })?;
        if let Some(recorder) = &self.recorder {
            recorder.require_current().map_err(invalid)?;
        }
        let seal = Arc::new(MergeExecutionPrefixSeal {
            application_header: self.state._curr_block.clone(),
            recorder: self.recorder.take(),
            execution_digest: crate::merge::merge_execution_root(executions),
            inputs: std::mem::take(&mut self.inputs),
            routes: std::mem::take(&mut self.routes),
            rows: std::mem::take(&mut self.rows),
            row_bytes,
            sources,
        });
        self.state.merge_execution_prefix = Some(MergeExecutionPrefix {
            seal,
            budget: self.budget.take(),
            inventory: None,
        });
        self.witness
            .take()
            .ok_or_else(|| invalid("merge prefix lost witness owner".into()))?
            .commit();
        self.state.execution_output_plan = None;
        self.finished = true;
        Ok(())
    }
}

impl Drop for MergePrefixOwner<'_, '_> {
    fn drop(&mut self) {
        if !self.finished {
            self.state.execution_output_plan = Some(ExecutionOutputPlanState::Poisoned);
        }
    }
}

impl StateBlock<'_> {
    pub(super) fn merge_prefix_seal(&self) -> Option<&Arc<MergeExecutionPrefixSeal>> {
        self.merge_execution_prefix
            .as_ref()
            .map(|prefix| &prefix.seal)
    }

    /// Move the original remaining budget after the common start hooks.
    pub(super) fn take_merge_prefix_budget(
        &mut self,
        source: &SignedBlock,
        pipeline: u32,
        time: u32,
    ) -> Result<Option<(ExecutionOutputBudget, u32)>, String> {
        if self.merge_execution_prefix.is_none() {
            return Ok(None);
        }
        self.verify_merge_prefix_carrier(source)?;
        if source.network_entrypoint_count() != 0 {
            return Err("merge prefix cannot mix ordinary Network sources".into());
        }
        let (_, output) = self.fastpq_source_policy_at_block_start();
        if pipeline > output.max_pipeline_triggers || time > output.max_time_invocations {
            return Err("merge tail exceeds the frozen complete output envelope".into());
        }
        let prefix = self
            .merge_execution_prefix
            .as_mut()
            .expect("checked original merge prefix");
        let mut budget = prefix
            .budget
            .take()
            .ok_or("merge budget was already consumed")?;
        // Merge source transaction events are committed in the certified merge
        // projection, not ordinary Network positions. Its carrier tail keeps the
        // existing BlockApproved/Time semantics and releases those uninvoked slots.
        let unused_pipeline = u32::try_from(prefix.seal.inputs.len())
            .map_err(|_| "merge input count exceeds u32")?
            .checked_add(1)
            .and_then(|n| n.checked_mul(output.max_pipeline_triggers))
            .and_then(|n| n.checked_sub(pipeline))
            .ok_or("merge Pipeline remainder overflows")?;
        budget.skip_uninvoked(ExecutionOutputPhase::Pipeline, unused_pipeline)?;
        Ok(Some((budget, output.max_time_invocations - time)))
    }

    pub(super) fn verify_merge_prefix_carrier(&self, source: &SignedBlock) -> Result<(), String> {
        let Some(prefix) = &self.merge_execution_prefix else {
            if self
                .staged_merge_entry
                .as_ref()
                .is_some_and(|entry| entry.execution_batch.is_some())
            {
                return Err("certified merge carrier lost its actual prefix owner".into());
            }
            return Ok(());
        };
        let entry = self
            .staged_merge_entry
            .as_ref()
            .ok_or("merge prefix lacks its staged certified entry")?;
        prefix.seal.verify_carrier_entry(source, entry)
    }

    /// A late fixture/driver must not replace the actual prefix recorder, even
    /// when the source produced only ordinary writes and no transfer transcript.
    pub(crate) fn require_merge_prefix_recording(&self) -> Result<(), String> {
        if let Some(prefix) = &self.merge_execution_prefix {
            prefix
                .seal
                .recorder
                .as_ref()
                .ok_or("merge prefix was executed without its applying recorder")?
                .require_current()?;
        }
        Ok(())
    }

    fn verify_merge_owned_positions(&self, sources: &OwnedExecutionSources) -> Result<(), String> {
        match (self.merge_prefix_seal(), sources.merge_prefix()) {
            (None, None) => Ok(()),
            (Some(actual), Some(owned)) if Arc::ptr_eq(actual, owned) => {
                if sources.entries().len() < owned.inputs.len()
                    || sources.network_routes().len() < owned.inputs.len()
                {
                    return Err("merge prefix lost its complete positions".into());
                }
                for ((input, route), (source, actual_route)) in owned
                    .inputs
                    .iter()
                    .zip(&owned.routes)
                    .zip(sources.entries().iter().zip(sources.network_routes()))
                {
                    if source.call() != Hash::from(input.execution_call_hash())
                        || route != actual_route
                        || source.lane() != Some(route.lane_id)
                        || source.dataspace() != route.dataspace_id
                    {
                        return Err("merge prefix substituted a source or route".into());
                    }
                }
                Ok(())
            }
            _ => Err("merge prefix lost its original source custody".into()),
        }
    }

    pub(super) fn verify_merge_owned_sources(
        &self,
        sources: &OwnedExecutionSources,
    ) -> Result<(), String> {
        self.verify_merge_owned_positions(sources)?;
        if let Some(owned) = sources.merge_prefix() {
            owned.verify_archive(
                &self.fastpq_transcripts,
                self.captured_fastpq_transcript_sources()
                    .map_err(|e| e.to_string())?,
            )?;
        }
        Ok(())
    }

    /// Rejoin the actual source owner and original inventory/witness after the
    /// consuming carrier owner has removed them from mutable State fields.
    pub(super) fn verify_captured_merge_prefix(
        &self,
        sources: &OwnedExecutionSources,
        inventory: &Arc<FastpqSourceInventoryV1>,
        witness: &ExecWitness,
    ) -> Result<(), String> {
        self.verify_merge_owned_positions(sources)?;
        let Some(prefix) = &self.merge_execution_prefix else {
            return Ok(());
        };
        self.verify_merge_inventory_positions(inventory)?;
        inventory.verify_ordinary_witness_bundles(&witness.fastpq_transcripts)?;
        for (call, original) in &prefix.seal.sources {
            let actual = witness
                .fastpq_transcripts
                .binary_search_by_key(call, |bundle| bundle.entry_hash)
                .ok()
                .map(|index| &witness.fastpq_transcripts[index].transcripts);
            if original.transcript != transcripts_hash(actual)? {
                return Err("merge witness changed an original prefix transcript".into());
            }
        }
        if !witness.fastpq_batches.is_empty() {
            return Err("merge witness contains unowned prepared proof batches".into());
        }
        Ok(())
    }

    /// Retain the exact immutable inventory only after the common seal has
    /// consumed and checked the original prefix source capsule.
    pub(super) fn bind_merge_prefix_inventory(
        &mut self,
        sources: &OwnedExecutionSources,
    ) -> Result<(), String> {
        self.verify_merge_owned_sources(sources)?;
        if self.merge_execution_prefix.is_none() {
            return Ok(());
        }
        let inventory = Arc::clone(
            self.fastpq_source_inventory
                .as_ref()
                .ok_or("merge prefix inventory was not finalized")?
                .as_ref()
                .map_err(Clone::clone)?,
        );
        let prefix = self
            .merge_execution_prefix
            .as_mut()
            .expect("checked prefix owner");
        if prefix.inventory.is_some() {
            return Err("merge prefix inventory was already bound".into());
        }
        prefix.inventory = Some(inventory);
        Ok(())
    }

    fn verify_merge_inventory_positions(
        &self,
        inventory: &Arc<FastpqSourceInventoryV1>,
    ) -> Result<(), String> {
        let prefix = self
            .merge_execution_prefix
            .as_ref()
            .ok_or("merge inventory lost its prefix")?;
        if !prefix
            .inventory
            .as_ref()
            .is_some_and(|original| Arc::ptr_eq(original, inventory))
        {
            return Err("merge inventory differs from its original complete source owner".into());
        }
        let frozen = self
            .fastpq_source_context
            .as_ref()
            .ok_or("merge inventory lost its source-height context")?;
        let captures = self
            .captured_fastpq_transcript_sources()
            .map_err(|e| e.to_string())?;
        if inventory.entries().len() < prefix.seal.inputs.len() {
            return Err("merge inventory lost an original prefix position".into());
        }
        for ((input, route), entry) in prefix
            .seal
            .inputs
            .iter()
            .zip(&prefix.seal.routes)
            .zip(inventory.entries())
        {
            let call = Hash::from(input.execution_call_hash());
            let expected = frozen
                .capture_transcript(
                    Some(call),
                    call,
                    Some(route.lane_id),
                    Some(route.dataspace_id),
                    0,
                )
                .map_err(|e| e.to_string())?;
            let original = prefix
                .seal
                .sources
                .get(&call)
                .ok_or("merge source disappeared")?;
            if entry.entry_hash != call
                || entry.execution_kind != expected.execution_kind()
                || entry.route != expected.route()
                || entry.dataspace_id != expected.dataspace_id()
                || original.capture != captures.get(&call).copied()
                || original.transcript.is_some()
                    != inventory.transcript_entry_hashes().contains(&call)
            {
                return Err("sealed inventory changed an original merge source or capture".into());
            }
        }
        Ok(())
    }

    /// The old merge publication surface may retain only authenticated prefix
    /// transcripts before sealing, or an exactly checked common witness after it.
    pub(super) fn verify_merge_prefix_surface(&self) -> Result<(), String> {
        let Some(prefix) = &self.merge_execution_prefix else {
            return if self.fastpq_transcripts.is_empty() && self.exec_witness.is_none() {
                Ok(())
            } else {
                Err("merge surface has no owner for its transcript or witness".into())
            };
        };
        if let Some(original) = &prefix.inventory {
            let inventory = match self.fastpq_source_inventory.as_ref() {
                Some(inventory) => inventory.as_ref().map_err(Clone::clone)?,
                None if matches!(
                    self.execution_output_plan,
                    Some(ExecutionOutputPlanState::Captured)
                ) && self.exec_witness.is_none() =>
                {
                    original
                }
                None => return Err("merge prefix lost its sealed complete source inventory".into()),
            };
            if !Arc::ptr_eq(original, inventory)
                || !self.fastpq_transcripts.is_empty()
                || !matches!(
                    self.execution_output_plan,
                    Some(
                        ExecutionOutputPlanState::Sealed(_)
                            | ExecutionOutputPlanState::Authorized(_)
                            | ExecutionOutputPlanState::Finalizing
                            | ExecutionOutputPlanState::Finalized(_)
                            | ExecutionOutputPlanState::Captured
                    )
                )
            {
                return Err("merge prefix lost its original sealed inventory custody".into());
            }
            self.verify_merge_inventory_positions(inventory)?;
            if let Some(witness) = &self.exec_witness {
                inventory.verify_ordinary_witness_bundles(&witness.fastpq_transcripts)?;
                for (call, original) in &prefix.seal.sources {
                    let actual = witness
                        .fastpq_transcripts
                        .binary_search_by_key(call, |bundle| bundle.entry_hash)
                        .ok()
                        .map(|index| &witness.fastpq_transcripts[index].transcripts);
                    if original.transcript != transcripts_hash(actual)? {
                        return Err("merge witness changed an original prefix transcript".into());
                    }
                }
                self.verify_cached_ordinary_witness_content(inventory)?;
            }
            return Ok(());
        }
        if self.exec_witness.is_some() || self.fastpq_source_inventory.is_some() {
            return Err("merge prefix witness bypassed its original inventory owner".into());
        }
        prefix.seal.verify_archive(
            &self.fastpq_transcripts,
            self.captured_fastpq_transcript_sources()
                .map_err(|e| e.to_string())?,
        )?;
        if self
            .fastpq_transcripts
            .keys()
            .any(|call| !prefix.seal.sources.contains_key(call))
        {
            return Err("merge surface retained an unowned non-prefix transcript".into());
        }
        Ok(())
    }
}
