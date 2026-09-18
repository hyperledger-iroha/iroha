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
    block::{SignedBlock, lane_admission::LaneAdmittedInputV1},
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
    /// Exact number of blocks, from the enclosing --from height (1..=4096).
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
            TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => None,
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
        match entrypoint {
            TransactionEntrypoint::Time(time) => {
                for (index, isi) in time.instructions.iter().enumerate() {
                    self.instruction(isi, &occurrence, format!("time_trigger/{index}"))?;
                }
            }
            TransactionEntrypoint::SealedCommitment(_) => {
                self.gap("sealed_commitment_bodies_unavailable")
            }
            _ => {}
        }
        if let Some(Ok(steps)) = result.map(|result| result.as_ref()) {
            for (step_index, step) in steps.iter().enumerate() {
                for (index, isi) in step.instructions.iter().enumerate() {
                    self.instruction(
                        isi,
                        &occurrence,
                        format!("recorded_data_trigger/{step_index}/{index}"),
                    )?;
                }
            }
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
        block
            .validate_entrypoint_merkle_cache()
            .map_err(|_| eyre!("block entrypoint Merkle cache differs"))?;
        block
            .validate_result_merkle_cache()
            .map_err(|_| eyre!("block result Merkle cache differs"))?;
        // Consensus commits physical external inputs; the retained execution
        // tree additionally includes native inputs and Time entrypoints.
        let external_root = block
            .external_entrypoints_slice()
            .iter()
            .map(TransactionEntrypoint::hash)
            .collect::<iroha_crypto::MerkleTree<_>>()
            .root();
        if external_root != block.header().merkle_root()
            || block
                .result_hashes()
                .collect::<iroha_crypto::MerkleTree<_>>()
                .root()
                != block.header().result_merkle_root()
        {
            return Err(eyre!(
                "block header differs from its retained execution roots"
            ));
        }
        if block.entrypoint_hashes().len() != block.results().len() {
            return Err(eyre!("block entrypoint/result count differs"));
        }
        let source = if block
            .execution_context()
            .is_some_and(|c| c.native_lane_decisions.is_some())
        {
            "native_decision_execution"
        } else {
            "block_entrypoint_execution"
        };
        for (index, entrypoint, result) in block.entrypoint_results() {
            projection.entrypoint(&entrypoint, block, source, index, Some(result), None)?;
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
            .map_err(|_| eyre!("invalid canonical block at height {}", index + 1))?;
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
    use iroha_data_model::trigger::{DataTriggerSequence, TimeTriggerEntrypoint};
    use iroha_data_model::{
        NetworkId,
        account::AccountId,
        block::BlockHeader,
        isi::{Log, consensus_keys::ThresholdKeyLifecycleCertificateV1},
        transaction::{
            ExecutionStep, FeePaymentIntent, IvmBytecode, TransactionBuilder,
            signed::{TransactionResult, TransactionResultInner},
        },
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
                .any(|row| row["instruction_path"].as_str() == Some("recorded_data_trigger/0/0"))
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
        let time = TimeTriggerEntrypoint {
            id: "beacon_lifecycle_timer".parse().expect("trigger ID"),
            instructions: ExecutionStep(vec![lifecycle()].into()),
            authority: AccountId::new(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone()),
        };
        let hashes = [
            block.external_entrypoints_slice()[0].hash(),
            time.hash_as_entrypoint(),
        ];
        block
            .set_transaction_results(
                vec![time],
                &hashes,
                vec![
                    TransactionResultInner::Ok(DataTriggerSequence::default()),
                    TransactionResultInner::Ok(DataTriggerSequence::default()),
                ],
            )
            .expect("attach canonical external and Time results");
        assert_ne!(block.full_entry_merkle_root(), block.header().merkle_root());
        assert_eq!(
            block.header().merkle_root(),
            [hashes[0]]
                .into_iter()
                .collect::<iroha_crypto::MerkleTree<_>>()
                .root()
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
            Some("time_trigger/0")
        );
        assert_eq!(
            records[0]["occurrence"]["stored_outcome"].as_str(),
            Some("recorded_success_unverified")
        );
        assert_eq!(
            value["authenticated_history_verified"].as_bool(),
            Some(false)
        );

        // Keep both header commitments mandatory even though their input sets differ.
        for field in ["merkle_root", "result_merkle_root"] {
            let mut corrupt = json::to_value(&block).expect("fixture JSON");
            corrupt
                .as_object_mut()
                .expect("block object")
                .get_mut("payload")
                .expect("block payload")
                .as_object_mut()
                .expect("payload object")
                .get_mut("header")
                .expect("block header")
                .as_object_mut()
                .expect("header object")
                .insert(field.to_owned(), Value::Null);
            let corrupt: SignedBlock = json::from_value(corrupt).expect("structural fixture");
            let error = project_block(
                &corrupt,
                &mut Projection::default(),
                &BTreeMap::new(),
                &mut BTreeSet::new(),
            )
            .expect_err("reject mismatched header commitment");
            assert!(error.to_string().contains("retained execution roots"));
        }
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
                "--from",
                "1",
                "./public-core",
                "beacon-history",
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
                "./public-core",
                "beacon-history"
            ])
            .is_err()
        );
    }
}
