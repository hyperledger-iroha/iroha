//! Bounded, read-only public beacon candidate projection from canonical Kura carriers.
//!
//! Structural coherence is deliberately distinct from finality, successful state
//! replay, and eligible provider custody. No report from this module proves absence.

use super::*;
use color_eyre::eyre::Result;
use iroha_core::{
    beacon::FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    lane_consensus::LaneExecutablePayloadV1,
    merge::{merge_application_header_from_carrier, merge_execution_batch_commitments_match},
    merge_sidecar::decode_certified_merge_sidecar,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    block::{
        SignedBlock,
        execution_output::{ExecutionOutputV1, TriggerFailureRootV1},
        lane_admission::LaneAdmittedInputV1,
    },
    isi::{
        InstructionBox, SetParameter,
        consensus_keys::{ApplyThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleActionV1},
    },
    merge::{MAX_MERGE_LEDGER_ENTRY_BYTES, MergeLedgerEntry},
    parameter::system::{KagemushaMintFinalityNextEpochParameterV1, Parameter},
    transaction::{Executable, ExecutableBatchItem, signed::TransactionEntrypoint},
};
use norito::json::{self, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
};

const MAX_BLOCKS: u64 = 4_096;
const MAX_RECORDS: usize = 16_384;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_SIDECAR_BYTES: usize = 64 * 1024 * 1024;
const MAX_SIDECARS: usize = 64;

/// Read-only beacon history options. An exact, unclamped range is mandatory.
#[derive(Debug, ClapArgs, Clone)]
pub(super) struct Args {
    /// Exact number of blocks, from the --from height (1..=4096).
    #[clap(long)]
    pub(super) length: u64,
    /// Exact canonical public merge-entry file; repeat for referenced carriers only.
    #[clap(long = "merge-sidecar", value_name = "FILE")]
    pub(super) merge_sidecars: Vec<PathBuf>,
    /// Write bounded JSON outside the inspected store; defaults to stdout.
    #[clap(short, long, value_name = "OUTPUT")]
    pub(super) output: Option<PathBuf>,
}

#[derive(Default)]
struct Projection {
    records: Vec<Value>,
    counts: BTreeMap<String, u64>,
    record_bytes: usize,
    gaps: BTreeSet<&'static str>,
}
impl Projection {
    fn count(&mut self, name: &str) {
        *self.counts.entry(name.to_owned()).or_default() += 1;
    }
    fn gap(&mut self, name: &'static str) {
        self.gaps.insert(name);
        self.count(name);
    }
    fn record(&mut self, value: Value) -> Result<()> {
        if self.records.len() >= MAX_RECORDS {
            return Err(eyre!("beacon candidate report exceeds record bound"));
        }
        let rendered =
            json::to_json_bounded(&value, MAX_OUTPUT_BYTES.saturating_sub(self.record_bytes))?;
        self.record_bytes = self
            .record_bytes
            .checked_add(rendered.len())
            .ok_or_else(|| eyre!("beacon report size overflow"))?;
        self.records.push(value);
        Ok(())
    }
    fn instruction(
        &mut self,
        instruction: &InstructionBox,
        occurrence: &Value,
        path: String,
    ) -> Result<()> {
        self.count("explicit_native_instructions");
        if let Some(isi) = instruction
            .as_any()
            .downcast_ref::<ApplyThresholdKeyLifecycleCertificateV1>()
        {
            let cert = &isi.certificate;
            let action = match cert.action {
                ThresholdKeyLifecycleActionV1::InstallGlobalBeaconKey => {
                    "install_global_beacon_key"
                }
                ThresholdKeyLifecycleActionV1::RetireGlobalBeaconKey => "retire_global_beacon_key",
                _ => return Ok(()),
            };
            // Output only typed public identity fields. Opaque certificate bytes are never emitted.
            let public_record = if cert.action
                == ThresholdKeyLifecycleActionV1::InstallGlobalBeaconKey
            {
                let decoded = norito::decode_canonical_with_limits::<
                    FinalizedGlobalThresholdBeaconKeySessionRecordV1,
                >(
                    &cert.public_state,
                    norito::canonical_decode_limits(cert.public_state.len()),
                );
                match decoded {
                    Ok(record) => norito::json!({
                        "decoded": true,
                        "network_id": (record.session.network_id.to_string()),
                        "session_id": (hex::encode(record.session.session_id)),
                        "roster_hash": (hex::encode(record.session.roster_hash)),
                        "transcript_hash": (hex::encode(record.session.transcript_hash)),
                        "committee_size": (record.session.committee_size),
                        "threshold": (record.session.threshold),
                        "dkg_finalized_at_height": (record.session.adaptive_dkg.finalized_at_height),
                        "activated_at_height": (record.activated_at_height),
                        "retired_at_height": (record.retired_at_height),
                        "certificate_identity_matches": (record.session.network_id == cert.network_id
                            && record.session.session_id == cert.session_id
                            && record.session.transcript_hash == cert.transcript_hash),
                        "public_dkg_validated": false
                    }),
                    Err(_) => norito::json!({"decoded": false, "public_dkg_validated": false}),
                }
            } else {
                Value::Null
            };
            self.record(norito::json!({
                "kind": "global_beacon_lifecycle_candidate", "occurrence": (occurrence.clone()),
                "instruction_path": path, "action": action, "certificate_version": (cert.version),
                "network_id": (cert.network_id.to_string()), "effective_height": (cert.effective_height),
                "next_height_effective_at": (cert.effective_height.checked_add(1)),
                "expected_active_session_id": (cert.expected_active_session_id.map(hex::encode)),
                "session_id": (hex::encode(cert.session_id)), "roster_hash": (hex::encode(cert.roster_hash)),
                "transcript_hash": (hex::encode(cert.transcript_hash)),
                "committee_size": (cert.committee_size), "quorum": (cert.quorum),
                "signature_count": (cert.signatures.len()), "public_state_bytes": (cert.public_state.len()),
                "public_state_hash": (Hash::new(&cert.public_state).to_string()),
                "public_record": public_record, "certificate_authority_verified": false
            }))?;
        } else if let Some(isi) = instruction.as_any().downcast_ref::<SetParameter>() {
            if let Parameter::Custom(custom) = isi.inner()
                && custom.id() == &KagemushaMintFinalityNextEpochParameterV1::parameter_id()
            {
                let parsed =
                    KagemushaMintFinalityNextEpochParameterV1::from_custom_parameter(custom);
                let roster = parsed.map(|parameter| {
                    norito::json!({
                        "epoch": (parameter.roster.epoch),
                        "roster": (parameter.roster)
                    })
                });
                self.record(norito::json!({
                    "kind": "mint_finality_next_roster_candidate", "occurrence": (occurrence.clone()),
                    "instruction_path": path, "parameter_id": (custom.id().to_string()),
                    "typed_valid_roster": roster
                }))?;
            }
        }
        Ok(())
    }
    fn executable(&mut self, executable: &Executable, occurrence: &Value) -> Result<()> {
        match executable {
            Executable::Instructions(instructions) => {
                for (index, isi) in instructions.iter().enumerate() {
                    self.instruction(isi, occurrence, format!("instructions/{index}"))?;
                }
            }
            Executable::Batch(items) => {
                for (index, item) in items.iter().enumerate() {
                    match item {
                        ExecutableBatchItem::Instruction(isi) => {
                            self.instruction(isi, occurrence, format!("batch/{index}"))?
                        }
                        ExecutableBatchItem::ContractCall(_) => {
                            self.gap("contract_calls_not_replayed")
                        }
                    }
                }
            }
            Executable::IvmProved(proved) => {
                self.gap("proved_ivm_runtime_not_replayed");
                for (index, isi) in proved.overlay.iter().enumerate() {
                    self.instruction(isi, occurrence, format!("proved_overlay/{index}"))?;
                }
            }
            Executable::Ivm(_) => self.gap("ivm_runtime_not_replayed"),
            Executable::ContractCall(_) => self.gap("contract_calls_not_replayed"),
        }
        Ok(())
    }
    fn entrypoint(
        &mut self,
        entrypoint: &TransactionEntrypoint,
        block: &SignedBlock,
        source: &str,
        index: usize,
        result: Option<&iroha_data_model::transaction::signed::TransactionResult>,
        merge_hash: Option<String>,
    ) -> Result<()> {
        self.count(source);
        let tx = match entrypoint {
            TransactionEntrypoint::External(tx) => Some(tx),
            TransactionEntrypoint::SealedReveal(reveal) => Some(reveal.signed_transaction()),
            TransactionEntrypoint::SealedCommitment(_) => None,
        };
        let occurrence = norito::json!({
            "carrier_height": (block.header().height().get()), "carrier_hash": (block.hash().to_string()),
            "source": source, "entrypoint_index": index, "entrypoint_hash": (entrypoint.hash().to_string()),
            "signed_transaction_hash": (tx.map(|tx| tx.hash().to_string())), "stored_outcome": (outcome(result)),
            "merge_entry_hash": (if source == "reference_bound_merge_execution" { merge_hash.clone() } else { None }),
            "source_reference": merge_hash, "authenticated_execution_verified": false
        });
        if let Some(tx) = tx {
            self.executable(tx.instructions(), &occurrence)?;
        }
        if matches!(entrypoint, TransactionEntrypoint::SealedCommitment(_)) {
            self.gap("sealed_commitment_bodies_unavailable");
        }
        if let Some(result) = result {
            self.recorded_steps(result, &occurrence, "recorded_trigger")?;
        }
        Ok(())
    }

    // A full result owns nested Data and ExecuteTrigger steps together. It does
    // not encode which dispatch mechanism selected each step; do not invent it.
    fn recorded_steps(
        &mut self,
        result: &iroha_data_model::transaction::signed::TransactionResult,
        occurrence: &Value,
        path: &str,
    ) -> Result<()> {
        if let Ok(steps) = result.as_ref() {
            for (step_index, step) in steps.iter().enumerate() {
                for (index, isi) in step.instructions.iter().enumerate() {
                    self.instruction(isi, occurrence, format!("{path}/{step_index}/{index}"))?;
                }
            }
        }
        Ok(())
    }

    fn output(
        &mut self,
        block: &SignedBlock,
        index: usize,
        output: &ExecutionOutputV1,
    ) -> Result<()> {
        let (source, invocation, result, failure_root, completions) = match output {
            ExecutionOutputV1::Network(row) => {
                let input_index = usize::try_from(row.input_index)?;
                let entrypoint = block
                    .network_entrypoint_at(input_index)
                    .ok_or_else(|| eyre!("Network output lacks its exact input"))?;
                let source = if block
                    .execution_context()
                    .is_some_and(|c| c.native_lane_decisions.is_some())
                {
                    "native_decision_execution"
                } else {
                    "block_network_execution"
                };
                return self.entrypoint(
                    entrypoint,
                    block,
                    source,
                    input_index,
                    Some(&row.result),
                    None,
                );
            }
            ExecutionOutputV1::Pipeline(row) => (
                "pipeline_execution",
                json::to_value(&row.invocation)?,
                &row.result,
                &row.failure_root,
                &row.completions,
            ),
            ExecutionOutputV1::Time(row) => (
                "time_execution",
                json::to_value(&row.invocation)?,
                &row.result,
                &row.failure_root,
                &row.completions,
            ),
        };
        self.count(source);
        let occurrence = norito::json!({
            "carrier_height": (block.header().height().get()), "carrier_hash": (block.hash().to_string()),
            "source": source, "output_index": index, "output_hash": (iroha_crypto::HashOf::new(output).to_string()),
            "invocation": invocation, "completion_count": (completions.len()),
            "stored_outcome": (outcome(Some(result))), "authenticated_execution_verified": false
        });
        self.recorded_steps(result, &occurrence, "recorded_trigger")?;
        match failure_root {
            Some(TriggerFailureRootV1::DeclaredInstructionProjection(step))
            | Some(TriggerFailureRootV1::ReturnedBeforeRollback(step)) => {
                let path = if matches!(
                    failure_root,
                    Some(TriggerFailureRootV1::DeclaredInstructionProjection(_))
                ) {
                    "rejected_declared_trigger"
                } else {
                    "rolled_back_trigger"
                };
                for (index, isi) in step.iter().enumerate() {
                    self.instruction(isi, &occurrence, format!("{path}/{index}"))?;
                }
            }
            Some(
                TriggerFailureRootV1::OmittedByOutputLimit
                | TriggerFailureRootV1::OmittedAfterRejection,
            ) => {
                self.gap("rejected_trigger_program_omitted");
            }
            None => {}
        }
        Ok(())
    }
}

fn outcome(
    result: Option<&iroha_data_model::transaction::signed::TransactionResult>,
) -> &'static str {
    match result.map(|result| result.as_ref()) {
        Some(Ok(_)) => "recorded_success_unverified",
        Some(Err(_)) => "recorded_rejection_unverified",
        None => "not_an_execution_result",
    }
}

fn project_block(
    block: &SignedBlock,
    projection: &mut Projection,
    sidecars: &BTreeMap<String, Vec<u8>>,
    used: &mut BTreeSet<String>,
) -> Result<()> {
    block
        .validate_proposal_commitments()
        .map_err(|_| eyre!("block proposal commitments differ"))?;
    if block.has_results() {
        // The immutable proposal commits network inputs. BlockResult owns the
        // separate complete output tree, including internal invocation identities.
        block
            .validate_output_merkle_cache()
            .map_err(|_| eyre!("block typed output ownership or Merkle cache differs"))?;
    }
    if let Some(context) = block.execution_context() {
        context
            .validate_native_lane_decisions_shape()
            .map_err(|_| eyre!("invalid native decision carrier shape"))?;
        for (index, bytes) in context.queue_plan_admissions.iter().enumerate() {
            let admitted = LaneAdmittedInputV1::decode_canonical(bytes)
                .map_err(|_| eyre!("invalid canonical QueuePlan admission control"))?;
            projection.entrypoint(
                &admitted.entrypoint,
                block,
                "queue_plan_admission_only",
                index,
                None,
                None,
            )?;
        }
        for (anchor_index, envelope) in context.autonomous_lane_payloads.iter().enumerate() {
            let payload = norito::decode_canonical_with_limits::<LaneExecutablePayloadV1>(
                &envelope.canonical_payload,
                norito::canonical_decode_limits(envelope.canonical_payload.len()),
            )
            .map_err(|_| eyre!("invalid canonical autonomous proposal payload"))?;
            if envelope.version
                != iroha_data_model::block::AUTONOMOUS_LANE_PAYLOAD_ENVELOPE_VERSION_V1
                || payload.version != 1
                || payload.network_id != envelope.network_id
                || payload.epoch != envelope.epoch
                || payload.payload_hash != envelope.payload_hash
                || payload.origin_proposal.proposal_hash != envelope.proposal_hash
            {
                return Err(eyre!(
                    "autonomous proposal layout or envelope identity differs"
                ));
            }
            projection.gap("autonomous_proposal_authority_not_verified");
            for (index, entrypoint) in payload.entrypoints.iter().enumerate() {
                projection.entrypoint(
                    entrypoint,
                    block,
                    "autonomous_proposal_only",
                    index,
                    None,
                    Some(format!("anchor:{anchor_index}")),
                )?;
            }
        }
        if let Some(reference) = &context.merge_entry {
            let hash = reference.entry_hash.to_string();
            if let Some(bytes) = sidecars.get(&hash) {
                let entry = norito::with_decode_limits_scope(
                    norito::canonical_decode_limits(bytes.len()),
                    || decode_certified_merge_sidecar(reference, bytes),
                )
                .map_err(|_| eyre!("merge sidecar differs from its exact carrier reference"))?;
                if entry.merge_qc.carrier_height != block.header().height().get()
                    || Some(entry.merge_qc.carrier_parent_hash) != block.header().prev_block_hash()
                    || entry.merge_qc.view != block.header().view_change_index()
                {
                    return Err(eyre!("merge certificate carrier coordinates differ"));
                }
                used.insert(hash.clone());
                if let Some(batch) = &entry.execution_batch {
                    if !merge_execution_batch_commitments_match(batch)
                        || batch.application_block_header
                            != merge_application_header_from_carrier(&block.header())
                    {
                        return Err(eyre!(
                            "merge batch commitments or application header differ"
                        ));
                    }
                    let mut index = 0;
                    for lane in &batch.lanes {
                        if lane.entrypoints.len() != lane.results.len() {
                            return Err(eyre!("merge lane entrypoint/result count differs"));
                        }
                        for (entrypoint, result) in lane.entrypoints.iter().zip(&lane.results) {
                            projection.entrypoint(
                                entrypoint,
                                block,
                                "reference_bound_merge_execution",
                                index,
                                Some(result),
                                Some(hash.clone()),
                            )?;
                            index += 1;
                        }
                    }
                }
            } else {
                projection.gap("merge_execution_sidecar_missing");
                projection.record(norito::json!({"kind": "unresolved_merge_execution_reference",
                    "carrier_height": (block.header().height().get()), "carrier_hash": (block.hash().to_string()),
                    "merge_entry_hash": hash, "encoded_len": (reference.encoded_len),
                    "entrypoint_count": (reference.entrypoint_count)}))?;
            }
        }
    }
    if block.has_results() {
        for (index, output) in block.execution_outputs().iter().enumerate() {
            projection.output(block, index, output)?;
        }
    } else {
        for (index, entrypoint) in block.network_entrypoints().enumerate() {
            projection.entrypoint(
                entrypoint,
                block,
                "resultless_proposal_only",
                index,
                None,
                None,
            )?;
        }
    }
    if let Some(pulse) = block
        .npos_consensus_effects()
        .and_then(|effects| effects.finalized_global_beacon_pulse.as_ref())
    {
        projection.record(norito::json!({"kind": "global_beacon_pulse_candidate",
            "carrier_height": (block.header().height().get()), "carrier_hash": (block.hash().to_string()),
            "network_id": (pulse.network_id.to_string()), "session_id": (hex::encode(pulse.session_id)),
            "roster_hash": (hex::encode(pulse.roster_hash)), "transcript_hash": (hex::encode(pulse.transcript_hash)),
            "pulse_height": (pulse.height), "pulse_id": (hex::encode(pulse.pulse_id)),
            "pulse_signature_verified": false}))?;
    }
    // Recorded trigger steps are included; dynamic VM/contract effects still require native replay.
    projection
        .gaps
        .insert("state_dependent_ivm_and_contract_effects_not_replayed");
    Ok(())
}

fn bounded_public_file(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() || before.file_type().is_symlink() || before.len() > limit as u64 {
        return Err(eyre!(
            "public sidecar must be a bounded direct regular file"
        ));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    let opened = file.metadata()?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    let after = fs::symlink_metadata(path)?;
    if bytes.len() > limit || !same_metadata(&before, &opened) || !same_metadata(&opened, &after) {
        return Err(eyre!("public sidecar changed during read"));
    }
    Ok(bytes)
}
fn same_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if left.dev() != right.dev()
            || left.ino() != right.ino()
            || left.ctime() != right.ctime()
            || left.ctime_nsec() != right.ctime_nsec()
        {
            return false;
        }
    }
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

pub(super) fn inspect(
    writer: &mut dyn Write,
    path: &Path,
    from: Option<u64>,
    args: &Args,
) -> Outcome {
    let start = from.ok_or_else(|| eyre!("beacon-history requires explicit --from"))?;
    if args.length == 0 || args.length > MAX_BLOCKS {
        return Err(eyre!("beacon-history length must be in 1..=4096"));
    }
    let end = start
        .checked_add(args.length)
        .ok_or_else(|| eyre!("beacon-history range overflow"))?;
    let path = super::resolve_block_store_dir(path)?;
    if let Some(output) = &args.output {
        let output = crate::atomic_output::resolve_output_file(output)?;
        for sidecar in &args.merge_sidecars {
            if output == fs::canonicalize(sidecar)? {
                return Err(eyre!("output would replace an input merge sidecar"));
            }
        }
    }
    let stamps = ["blocks.index", "blocks.data", "blocks.hashes"]
        .map(|name| fs::symlink_metadata(path.join(name)))
        .into_iter()
        .collect::<std::io::Result<Vec<_>>>()?;
    let mut store = BlockStore::open_read_only(&path)?;
    let index_count = store.read_index_count()?;
    if end > index_count {
        return Err(eyre!(
            "requested beacon-history range exceeds retained index; ranges are never clamped"
        ));
    }
    if args.merge_sidecars.len() > MAX_SIDECARS {
        return Err(eyre!("too many explicit merge sidecars"));
    }
    let mut sidecars = BTreeMap::new();
    let mut total_bytes = 0usize;
    for path in &args.merge_sidecars {
        let bytes = bounded_public_file(
            path,
            MAX_MERGE_LEDGER_ENTRY_BYTES.min(MAX_SIDECAR_BYTES - total_bytes),
        )?;
        total_bytes += bytes.len();
        let entry = norito::decode_canonical_with_limits::<MergeLedgerEntry>(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| eyre!("invalid canonical public merge sidecar"))?;
        if sidecars
            .insert(entry.canonical_hash().to_string(), bytes)
            .is_some()
        {
            return Err(eyre!("duplicate public merge sidecar"));
        }
    }
    let mut projection = Projection::default();
    let mut used = BTreeSet::new();
    let mut previous = None;
    let mut first_hash = None;
    let mut last_hash = None;
    for index in start..end {
        let mut indices = [BlockIndex {
            start: 0,
            length: 0,
        }];
        store.read_block_indices(index, &mut indices)?;
        let entry = indices[0];
        if entry.length == 0 || entry.length > MAX_EXECUTED_BLOCK_WIRE_BYTES {
            return Err(eyre!("invalid canonical block wire bound"));
        }
        let mut bytes = vec![0; usize::try_from(entry.length)?];
        store.read_block_data(entry.start, &mut bytes)?;
        let block = decode_framed_signed_block(&bytes)
            .map_err(|error| eyre!("invalid canonical block at height {}: {error}", index + 1))?;
        if block.header().height().get() != index + 1
            || previous.is_some_and(|hash| block.header().prev_block_hash() != Some(hash))
        {
            return Err(eyre!(
                "block height or in-range predecessor linkage differs"
            ));
        }
        first_hash.get_or_insert_with(|| block.hash().to_string());
        last_hash = Some(block.hash().to_string());
        previous = Some(block.hash());
        project_block(&block, &mut projection, &sidecars, &mut used)?;
    }
    if used.len() != sidecars.len() {
        return Err(eyre!(
            "supplied merge sidecar is not referenced by the selected range"
        ));
    }
    for (name, before) in ["blocks.index", "blocks.data", "blocks.hashes"]
        .into_iter()
        .zip(&stamps)
    {
        if !same_metadata(before, &fs::symlink_metadata(path.join(name))?) {
            return Err(eyre!("Kura source changed during inspection"));
        }
    }
    let report = norito::json!({"schema": "iroha.kura.beacon-history.v1", "from_height": (start + 1),
        "through_height": end, "retained_index_count": index_count, "decoded_blocks": (args.length),
        "first_block_hash": first_hash, "last_block_hash": last_hash, "in_range_header_linkage_checked": true,
        "coverage_counts": (projection.counts), "coverage_gaps": (projection.gaps.into_iter().collect::<Vec<_>>()),
        "records": (projection.records), "authenticated_history_verified": false,
        "authenticated_absence_proven": false, "provider_custody_verified": false,
        "hash_journal_content_verified": false,
        "interpretation": "Admission and proposal appearances are not execution. Recorded success and reference-bound merge results remain unauthenticated until native finality and successful replay are independently established."});
    let encoded = json::to_json_bounded(&report, MAX_OUTPUT_BYTES)?;
    writer.write_all(encoded.as_bytes())?;
    writer.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::{block::BlockBuilder, tx::AcceptedTransaction};
    use iroha_crypto::HashOf;
    use iroha_data_model::{
        NetworkId,
        account::AccountId,
        block::{
            BlockHeader,
            execution_output::{
                InvocationCompletionV1, NetworkExecutionOutputV1, PipelineEventPositionV1,
                PipelineExecutionOutputV1, PipelineInvocationV1, TimeExecutionOutputV1,
                TimeInvocationV1, TriggerUseV1,
            },
            output_budget::ExecutionOutputLimits,
        },
        events::{
            time::{TimeEvent, TimeInterval},
            trigger_completed::TriggerCompletedOutcome,
        },
        isi::{Log, consensus_keys::ThresholdKeyLifecycleCertificateV1},
        transaction::{
            ExecutionStep, FeePaymentIntent, IvmBytecode, TransactionBuilder,
            signed::TransactionResult,
        },
        trigger::DataTriggerStep,
    };
    use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;
    use std::{borrow::Cow, sync::Arc};

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"beacon-history-test",
        )))
    }
    fn lifecycle() -> InstructionBox {
        ApplyThresholdKeyLifecycleCertificateV1 {
            certificate: ThresholdKeyLifecycleCertificateV1 {
                version: 1,
                action: ThresholdKeyLifecycleActionV1::RetireGlobalBeaconKey,
                expected_active_session_id: Some([3; 32]),
                effective_height: 19,
                network_id: network(),
                roster_hash: [2; 32],
                committee_size: 4,
                quorum: 3,
                session_id: [3; 32],
                transcript_hash: [4; 32],
                public_state: Vec::new(),
                signatures: Vec::new(),
            },
        }
        .into()
    }
    fn block(instructions: Vec<InstructionBox>) -> Arc<SignedBlock> {
        crate::init_instruction_registry();
        let tx = TransactionBuilder::new(
            network(),
            AccountId::new(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions)
        .try_sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .expect("sign fixture");
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let block: SignedBlock = BlockBuilder::new(vec![accepted])
            .chain(0, None)
            .try_sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
            .expect("sign block")
            .unpack(|_| {})
            .into();
        Arc::new(block)
    }
    // Public report fixtures claim only structural output coherence, never State
    // execution, trigger selection, finality or provider authority.
    fn attach_outputs(block: &mut SignedBlock, outputs: Vec<ExecutionOutputV1>) {
        block
            .set_execution_outputs(
                outputs,
                0,
                BTreeMap::new(),
                Vec::new(),
                Default::default(),
                BTreeSet::new(),
                Vec::new(),
                &ExecutionOutputLimits {
                    max_outputs: 16,
                    max_output_bytes: 1024 * 1024,
                    max_total_output_bytes: 4 * 1024 * 1024,
                    max_executed_wire_bytes:
                        iroha_data_model::block::consensus_v2::MAX_EXECUTED_BLOCK_WIRE_BYTES,
                },
            )
            .expect("structurally coherent typed outputs");
    }
    fn network_output(steps: Vec<DataTriggerStep>) -> ExecutionOutputV1 {
        ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
            input_index: 0,
            result: TransactionResult::new(Ok(steps)),
            completions: Vec::new(),
        })
    }
    fn trigger_use(block: &SignedBlock, name: &str) -> TriggerUseV1 {
        TriggerUseV1 {
            trigger_id: name.parse().expect("trigger ID"),
            registered_at_height: block.header().height().get() - 1,
            action_hash: Hash::new(name.as_bytes()),
        }
    }
    fn trigger_step(name: &str) -> DataTriggerStep {
        DataTriggerStep {
            id: name.parse().expect("trigger ID"),
            instructions: ExecutionStep(vec![lifecycle()].into()),
        }
    }
    fn time_invocation(block: &SignedBlock) -> TimeInvocationV1 {
        TimeInvocationV1 {
            schedule_index: 0,
            event: TimeEvent {
                interval: TimeInterval {
                    since_ms: block.header().creation_time_ms.saturating_sub(1),
                    length_ms: 1,
                },
            },
            trigger: trigger_use(block, "beacon_lifecycle_timer"),
        }
    }
    fn store(block: &SignedBlock) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("store directory");
        let mut store = BlockStore::new(dir.path());
        store.create_files_if_they_do_not_exist().expect("journals");
        store.append_block_to_chain(block).expect("append block");
        dir
    }
    fn options() -> Args {
        Args {
            length: 1,
            merge_sidecars: Vec::new(),
            output: None,
        }
    }

    #[test]
    fn beacon_history_projects_only_typed_public_candidates_and_keeps_proof_limits() {
        let block = block(vec![
            lifecycle(),
            Log::new(
                iroha_data_model::Level::INFO,
                "unrelated-private-sentinel".to_owned(),
            )
            .into(),
        ]);
        let dir = store(&block);
        let mut output = Vec::new();
        inspect(&mut output, dir.path(), Some(0), &options()).expect("inspect");
        let text = String::from_utf8(output).expect("UTF8");
        assert!(text.contains("retire_global_beacon_key"));
        assert!(text.contains(&block.hash().to_string()));
        assert!(!text.contains("unrelated-private-sentinel"));
        assert!(!text.contains("public_state\":[]"));
        let value: Value = json::from_str(&text).expect("JSON");
        assert_eq!(value["authenticated_absence_proven"].as_bool(), Some(false));
        assert_eq!(
            value["authenticated_history_verified"].as_bool(),
            Some(false)
        );
        assert_eq!(value["records"].as_array().expect("records").len(), 1);
        assert_eq!(
            value["records"][0]["certificate_authority_verified"].as_bool(),
            Some(false)
        );
    }

    #[test]
    fn beacon_history_distinguishes_admission_from_recorded_execution_and_nested_effects() {
        let block = block(vec![lifecycle()]);
        let entrypoint = block.external_entrypoints_slice()[0].clone();
        let result = TransactionResult::from(Ok(Vec::new()));
        let mut projection = Projection::default();
        projection
            .entrypoint(
                &entrypoint,
                &block,
                "queue_plan_admission_only",
                0,
                None,
                None,
            )
            .expect("admission");
        projection
            .entrypoint(
                &entrypoint,
                &block,
                "reference_bound_merge_execution",
                0,
                Some(&result),
                Some("merge-hash".to_owned()),
            )
            .expect("execution");
        assert_eq!(
            projection.records[0]["occurrence"]["stored_outcome"].as_str(),
            Some("not_an_execution_result")
        );
        assert_eq!(
            projection.records[1]["occurrence"]["stored_outcome"].as_str(),
            Some("recorded_success_unverified")
        );
        assert_eq!(
            projection.records[1]["occurrence"]["authenticated_execution_verified"].as_bool(),
            Some(false)
        );
        projection
            .executable(
                &Executable::Ivm(IvmBytecode::from_compiled(vec![1, 2, 3])),
                &Value::Null,
            )
            .expect("opaque executable");
        assert!(projection.gaps.contains("ivm_runtime_not_replayed"));
        let mut trigger_result = result;
        trigger_result.0.as_mut().expect("success").push(
            iroha_data_model::trigger::DataTriggerStep {
                id: "beacon_candidate_trigger".parse().expect("trigger ID"),
                instructions: iroha_data_model::transaction::ExecutionStep(
                    vec![lifecycle()].into(),
                ),
            },
        );
        projection
            .entrypoint(
                &entrypoint,
                &block,
                "block_entrypoint_execution",
                0,
                Some(&trigger_result),
                None,
            )
            .expect("recorded trigger");
        assert!(
            projection
                .records
                .iter()
                .any(|row| row["instruction_path"].as_str() == Some("recorded_trigger/0/0"))
        );
    }

    #[test]
    fn beacon_history_separates_external_and_time_execution_roots_without_weakening_results() {
        let mut block = (*block(vec![
            Log::new(
                iroha_data_model::Level::INFO,
                "unrelated-external-work".to_owned(),
            )
            .into(),
        ]))
        .clone();
        let proposal_header = block.header();
        let input_hash = block.external_entrypoints_slice()[0].hash();
        let time = ExecutionOutputV1::Time(TimeExecutionOutputV1 {
            invocation: time_invocation(&block),
            result: TransactionResult::new(Ok(vec![trigger_step("beacon_lifecycle_timer")])),
            failure_root: None,
            completions: Vec::new(),
        });
        attach_outputs(&mut block, vec![network_output(Vec::new()), time]);
        assert_eq!(block.header(), proposal_header);
        assert_eq!(block.network_entrypoint_count(), 1);
        assert_eq!(block.execution_outputs().len(), 2);
        assert_eq!(
            block.header().merkle_root(),
            [input_hash]
                .into_iter()
                .collect::<iroha_crypto::MerkleTree<_>>()
                .root()
        );
        assert_ne!(
            block
                .network_input_merkle_commitment()
                .expect("input commitment")
                .root()
                .to_string(),
            block
                .output_merkle_commitment()
                .expect("output commitment")
                .root()
                .to_string()
        );
        let dir = store(&block);
        let mut output = Vec::new();
        inspect(&mut output, dir.path(), Some(0), &options())
            .expect("project canonical Time-bearing carrier");
        let value: Value = json::from_slice(&output).expect("bounded JSON");
        let records = value["records"].as_array().expect("candidate records");
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0]["instruction_path"].as_str(),
            Some("recorded_trigger/0/0")
        );
        assert_eq!(
            records[0]["occurrence"]["stored_outcome"].as_str(),
            Some("recorded_success_unverified")
        );
        assert_eq!(
            value["authenticated_history_verified"].as_bool(),
            Some(false)
        );

        assert_eq!(
            records[0]["occurrence"]["source"].as_str(),
            Some("time_execution")
        );
        assert_eq!(records[0]["occurrence"]["output_index"].as_u64(), Some(1));
        assert!(
            records[0]["occurrence"]
                .as_object()
                .expect("occurrence object")
                .get("entrypoint_hash")
                .is_none()
        );

        // Neither a changed proposal input root nor a stale output tree is
        // accepted. Outputs never rewrite the signed proposal header.
        let mut bad_input = json::to_value(&block).expect("fixture JSON");
        bad_input
            .as_object_mut()
            .unwrap()
            .get_mut("payload")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .get_mut("header")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("merkle_root".to_owned(), Value::Null);
        let mut bad_output = json::to_value(&block).expect("fixture JSON");
        bad_output
            .as_object_mut()
            .unwrap()
            .get_mut("result")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(
                "output_merkle".to_owned(),
                json::to_value(&iroha_crypto::MerkleTree::<ExecutionOutputV1>::default()).unwrap(),
            );
        let mut bad_owner = json::to_value(&block).expect("fixture JSON");
        let mut changed_outputs = block.execution_outputs().to_vec();
        let ExecutionOutputV1::Network(row) = &mut changed_outputs[0] else {
            panic!("Network fixture")
        };
        row.input_index = 1;
        let result = bad_owner
            .as_object_mut()
            .unwrap()
            .get_mut("result")
            .unwrap()
            .as_object_mut()
            .unwrap();
        result.insert(
            "outputs".to_owned(),
            json::to_value(&changed_outputs).unwrap(),
        );
        result.insert(
            "output_merkle".to_owned(),
            json::to_value(
                &changed_outputs
                    .iter()
                    .map(HashOf::new)
                    .collect::<iroha_crypto::MerkleTree<_>>(),
            )
            .unwrap(),
        );
        for (value, expected) in [
            (bad_input, "proposal commitments"),
            (bad_output, "typed output ownership or Merkle cache"),
            (bad_owner, "typed output ownership or Merkle cache"),
        ] {
            let corrupt: SignedBlock = json::from_value(value).expect("structural fixture");
            let error = project_block(
                &corrupt,
                &mut Projection::default(),
                &BTreeMap::new(),
                &mut BTreeSet::new(),
            )
            .expect_err("reject changed commitment");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn beacon_history_projects_nested_callbacks_once_and_distinguishes_rejected_roots() {
        let original = block(vec![
            Log::new(iroha_data_model::Level::INFO, "unrelated".to_owned()).into(),
        ]);
        let pipeline = ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 {
            invocation: PipelineInvocationV1 {
                event: PipelineEventPositionV1::Network(0),
                candidate_index: 0,
                trigger: trigger_use(&original, "pipeline_root"),
            },
            result: TransactionResult::new(Ok(vec![
                trigger_step("pipeline_root"),
                trigger_step("nested_by_call"),
            ])),
            failure_root: None,
            completions: Vec::new(),
        });
        for (failure_root, expected_path) in [
            (
                TriggerFailureRootV1::DeclaredInstructionProjection(ExecutionStep(
                    vec![lifecycle()].into(),
                )),
                "rejected_declared_trigger/0",
            ),
            (
                TriggerFailureRootV1::ReturnedBeforeRollback(ExecutionStep(
                    vec![lifecycle()].into(),
                )),
                "rolled_back_trigger/0",
            ),
        ] {
            let mut block = (*original).clone();
            let invocation = time_invocation(&block);
            let completion = InvocationCompletionV1 {
                callback_index: 0,
                trigger_id: invocation.trigger.trigger_id.clone(),
                outcome: TriggerCompletedOutcome::Failure("actual invocation rejected".to_owned()),
            };
            let failed = ExecutionOutputV1::Time(TimeExecutionOutputV1 {
                invocation,
                result: TransactionResult::new(Err(
                    iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                        iroha_data_model::ValidationFail::NotPermitted(
                            "fixture rejection".to_owned(),
                        ),
                    ),
                )),
                failure_root: Some(failure_root),
                completions: vec![completion],
            });
            attach_outputs(
                &mut block,
                vec![
                    network_output(vec![
                        trigger_step("nested_data"),
                        trigger_step("nested_execute_trigger"),
                    ]),
                    pipeline.clone(),
                    failed,
                ],
            );
            let mut projection = Projection::default();
            project_block(
                &block,
                &mut projection,
                &BTreeMap::new(),
                &mut BTreeSet::new(),
            )
            .expect("typed candidate projection");
            assert_eq!(
                projection.records.len(),
                5,
                "each full result is projected once"
            );
            for (row, source) in projection.records.iter().zip([
                "block_network_execution",
                "block_network_execution",
                "pipeline_execution",
                "pipeline_execution",
                "time_execution",
            ]) {
                assert_eq!(row["occurrence"]["source"].as_str(), Some(source));
                assert_eq!(
                    row["occurrence"]["authenticated_execution_verified"].as_bool(),
                    Some(false)
                );
            }
            assert_eq!(
                projection.records[4]["instruction_path"].as_str(),
                Some(expected_path)
            );
            assert_eq!(
                projection.records[4]["occurrence"]["stored_outcome"].as_str(),
                Some("recorded_rejection_unverified")
            );
        }
        let mut omitted = (*original).clone();
        let terminal = ExecutionOutputV1::time_output_limit_rejection(time_invocation(&omitted));
        attach_outputs(&mut omitted, vec![network_output(Vec::new()), terminal]);
        let mut projection = Projection::default();
        project_block(
            &omitted,
            &mut projection,
            &BTreeMap::new(),
            &mut BTreeSet::new(),
        )
        .expect("omitted rejected program");
        assert!(projection.records.is_empty());
        assert!(projection.gaps.contains("rejected_trigger_program_omitted"));
    }

    #[test]
    fn beacon_history_requires_exact_bounded_range_and_preserves_read_only_journals() {
        let block = block(vec![lifecycle()]);
        let dir = store(&block);
        let names = ["blocks.index", "blocks.data", "blocks.hashes"];
        let before = names.map(|name| fs::read(dir.path().join(name)).expect("read journals"));
        let mut output = Vec::new();
        assert!(inspect(&mut output, dir.path(), None, &options()).is_err());
        for length in [0, 2, MAX_BLOCKS + 1] {
            assert!(
                inspect(
                    &mut output,
                    dir.path(),
                    Some(0),
                    &Args {
                        length,
                        ..options()
                    }
                )
                .is_err()
            );
        }
        assert!(inspect(&mut output, dir.path(), Some(u64::MAX), &options()).is_err());
        assert!(output.is_empty());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for name in names {
                fs::set_permissions(dir.path().join(name), fs::Permissions::from_mode(0o400))
                    .expect("read-only file");
            }
            fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o500))
                .expect("read-only directory");
        }
        inspect(&mut output, dir.path(), Some(0), &options()).expect("read-only inspect");
        for (name, original) in names.into_iter().zip(before) {
            assert_eq!(fs::read(dir.path().join(name)).expect("journal"), original);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700))
                .expect("allow fixture cleanup");
        }
    }

    #[test]
    fn beacon_history_rejects_malformed_sidecars_and_preserves_their_source() {
        let block = block(vec![lifecycle()]);
        let dir = store(&block);
        let sidecar_dir = tempfile::tempdir().expect("sidecar dir");
        let sidecar = sidecar_dir.path().join("invalid.norito");
        fs::write(&sidecar, b"not a public merge entry").expect("fixture");
        let mut output = Vec::new();
        assert!(
            inspect(
                &mut output,
                dir.path(),
                Some(0),
                &Args {
                    merge_sidecars: vec![sidecar.clone()],
                    ..options()
                }
            )
            .is_err()
        );
        assert!(
            inspect(
                &mut output,
                dir.path(),
                Some(0),
                &Args {
                    merge_sidecars: vec![sidecar.clone()],
                    output: Some(sidecar.clone()),
                    ..options()
                }
            )
            .is_err()
        );
        assert!(output.is_empty());
        assert_eq!(
            fs::read(&sidecar).expect("preserved input"),
            b"not a public merge entry"
        );
        #[cfg(unix)]
        {
            let link = sidecar_dir.path().join("linked.norito");
            std::os::unix::fs::symlink(&sidecar, &link).expect("link");
            assert!(bounded_public_file(&link, 100).is_err());
        }
    }

    #[test]
    fn beacon_history_rejects_block_height_mismatch_without_publishing_partial_json() {
        let block = block(vec![lifecycle()]);
        let dir = store(&block);
        let mut store = BlockStore::new(dir.path());
        store
            .append_block_to_chain(&block)
            .expect("append repeated height");
        let mut output = Vec::new();
        assert!(
            inspect(
                &mut output,
                dir.path(),
                Some(0),
                &Args {
                    length: 2,
                    ..options()
                }
            )
            .is_err()
        );
        assert!(output.is_empty());
    }
    #[test]
    fn beacon_history_never_emits_opaque_install_state_or_unrelated_parameter_payloads() {
        let original = lifecycle();
        let mut install = original
            .as_any()
            .downcast_ref::<ApplyThresholdKeyLifecycleCertificateV1>()
            .expect("typed fixture")
            .clone();
        install.certificate.action = ThresholdKeyLifecycleActionV1::InstallGlobalBeaconKey;
        install.certificate.public_state = b"opaque-private-sentinel".to_vec();
        let mut projection = Projection::default();
        projection
            .instruction(&install.into(), &Value::Null, "instructions/0".to_owned())
            .expect("candidate");
        let text = json::to_json(&projection.records).expect("render projection");
        assert!(text.contains("install_global_beacon_key"));
        assert!(!text.contains("opaque-private-sentinel"));
        assert_eq!(
            projection.records[0]["public_record"]["decoded"].as_bool(),
            Some(false)
        );
        let unrelated = SetParameter::new(Parameter::Custom(
            iroha_data_model::parameter::CustomParameter::new(
                "unrelated_parameter".parse().expect("parameter ID"),
                iroha_primitives::json::Json::new("unrelated-private-sentinel"),
            ),
        ));
        projection
            .instruction(&unrelated.into(), &Value::Null, "instructions/1".to_owned())
            .expect("ignore unrelated parameter");
        assert_eq!(projection.records.len(), 1);
    }

    #[test]
    fn beacon_history_cli_exposes_explicit_bounded_scope() {
        use clap::Parser;
        assert!(
            crate::Cli::try_parse_from([
                "kagami",
                "advanced",
                "kura",
                "beacon-history",
                "./public-core",
                "--from",
                "1",
                "--length",
                "3598"
            ])
            .is_ok()
        );
        assert!(
            crate::Cli::try_parse_from([
                "kagami",
                "advanced",
                "kura",
                "beacon-history",
                "./public-core"
            ])
            .is_err()
        );
    }
}
