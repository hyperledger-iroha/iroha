//! End-to-end proof of the shared phase and resource harness on a genuine
//! FASTPQ prove-and-verify workload.
//!
//! The workload is the eight-row public transfer of
//! `crates/fastpq_prover/tests/resource_profile.rs`: a real FASTPQ STARK is
//! constructed, self-checked, framed, decoded and verified. It uses the
//! transparent replay prover that `fastpq_prover` exposes only under its
//! `dev-tools` feature, so this target is opt-in:
//!
//! ```text
//! cargo test -p iroha_core_privacy --features fastpq-replay-measurement \
//!     --test fastpq_phase_tree_measurement
//! ```
//!
//! Demonstrated phase granularity: the public prover and verifier calls. The
//! proof flow separates prover construction, proving including the mandatory
//! self-verification, and proof-frame encoding; the verification flow, which
//! is measured as its own record, separates frame decoding and verification.
//! Phases inside the backend (trace, low-degree extension, commitments, FRI,
//! openings) are not separated here: threading the recorder through the
//! shared backend is the consumer work of B.2, and the masked quantity and
//! AXT producers are measured by their own consumers. The only allocation
//! source is the scoped counter for the frame buffers this test owns.
//!
//! The one-percent rule is met here by three consecutive wrapper phases:
//! almost all of the proving root is the single phase
//! `prove_with_self_verification`. The test prints that share, and a consumer
//! that needs finer attribution states a bound with the harness option
//! `--max-undivided-phase-ppm`.
//!
//! Under `scripts/zk_resource_harness.py` the records are written to the
//! harness output directory with the identity the harness supplied; without
//! it they are checked in memory with an explicitly unbound identity. The
//! session writes the record itself when it ends, so a flow that fails, is
//! abandoned on an error return or unwinds reaches the harness as well.

use std::path::{Path, PathBuf};

use fastpq_isi::{FASTPQ_FINAL_V1_ID, resource_limits::FASTPQ_DEFAULT_MAX_PROOF_FRAME_BYTES_V1};
use fastpq_prover::{
    ExecutionMode, OperationKind, Proof, Prover, PublicInputs, StateTransition, TransitionBatch,
    gadgets::transfer::{attach_transfer_smt_witnesses, compute_poseidon_digest},
    verify,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    asset::id::AssetDefinitionId,
    fastpq::{
        TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferSmtWitness,
        TransferTranscript,
    },
    prelude::Quantity,
};
use iroha_measurement::{
    ByteKind, CollectingSink, DirectoryReport, DirectorySink, Finding, FlowKind,
    HARNESS_OUTPUT_DIR_ENV, MeasurementRecord, RecordSink, RunContext, RunIdentity, RunOutcome,
    Session, TeeSink, WorkerDeclaration, read_harness_context, unclassified_numbers,
};
use iroha_model_base::domain::DomainId;

const EMITTER: &str = "rust.iroha_core_privacy.fastpq_replay";
const PROVE_WORKLOAD: &str = "fastpq_replay_public_transfer_8_rows.prove";
const VERIFY_WORKLOAD: &str = "fastpq_replay_public_transfer_8_rows.verify";
const REJECT_WORKLOAD: &str = "fastpq_replay_public_transfer_8_rows.verify_tampered";

/// The deterministic fully witnessed transfer batch of the FASTPQ resource
/// profile fixture: `rows / 2` transfers, each touching two balance rows.
fn transfer_fixture(rows: usize) -> TransitionBatch {
    let domain = DomainId::try_new("resource", "universal").expect("fixture domain");
    let asset = AssetDefinitionId::derive_from_components(domain, "xor".parse().unwrap());
    let mut batch = TransitionBatch::new(
        FASTPQ_FINAL_V1_ID,
        PublicInputs {
            dsid: [0x3D; 16],
            slot: 23,
            perm_root: [0x33; 32],
            tx_set_hash: [0x44; 32],
            ..PublicInputs::default()
        },
    );
    let mut transcripts = Vec::with_capacity(rows / 2);
    for index in 0..rows / 2 {
        let account = |role: &str| {
            let seed: [u8; Hash::LENGTH] =
                Hash::new(format!("fastpq-resource-v1/{role}/{index:08}")).into();
            let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::default())
                .expect("deterministic fixture account");
            AccountId::new(keypair.public_key().clone())
        };
        let sender = account("sender");
        let receiver = account("receiver");
        let amount = 1 + index as u64;
        let sender_before = 1_000_000 + index as u64;
        let receiver_before = 500_000 + index as u64;
        let delta = TransferDeltaTranscript {
            from_account: sender.clone(),
            to_account: receiver.clone(),
            asset_definition: asset.clone(),
            amount: Quantity::from(amount),
            from_balance_before: Quantity::from(sender_before),
            from_balance_after: Quantity::from(sender_before - amount),
            to_balance_before: Quantity::from(receiver_before),
            to_balance_after: Quantity::from(receiver_before + amount),
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        };
        let batch_hash = Hash::new(format!("fastpq-resource-v1/batch/{index:08}"));
        let digest = compute_poseidon_digest(&delta, &batch_hash);
        transcripts.push(TransferTranscript {
            batch_hash,
            deltas: vec![delta],
            authority_digest: Hash::new(b"fastpq-resource-v1/authority"),
            poseidon_preimage_digest: Some(digest),
        });
        for (owner, before, after) in [
            (sender, sender_before, sender_before - amount),
            (receiver, receiver_before, receiver_before + amount),
        ] {
            batch.push(StateTransition::new(
                iroha_data_model::fastpq::transfer_balance_key(&asset, &owner)
                    .expect("canonical balance key"),
                before.to_le_bytes().to_vec(),
                after.to_le_bytes().to_vec(),
                OperationKind::Transfer,
            ));
        }
    }
    let (old_root, new_root) =
        attach_transfer_smt_witnesses(&mut transcripts).expect("chained transfer SMT witnesses");
    batch.public_inputs.old_root = old_root;
    batch.public_inputs.new_root = new_root;
    batch.metadata.insert(
        TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
        norito::to_bytes(&transcripts).expect("canonical transfer transcripts"),
    );
    batch.sort();
    batch
}

/// Identity context and output directory handed over by the harness, if any.
/// This diagnostic-only variable selects where public records are written.
fn harness() -> (RunContext, Option<PathBuf>) {
    std::env::var_os(HARNESS_OUTPUT_DIR_ENV)
        .map(PathBuf::from)
        .map_or_else(
            || (RunContext::unbound(), None),
            |directory| {
                (
                    read_harness_context(&directory).expect("harness context"),
                    Some(directory),
                )
            },
        )
}

fn workers() -> WorkerDeclaration {
    WorkerDeclaration {
        workers: u32::try_from(rayon::current_num_threads()).unwrap_or(u32::MAX),
        provenance: "rayon.current_num_threads",
    }
}

/// Where one measured flow delivers its record: always to memory for the
/// assertions of this test, and also to `directory` when a harness (or a
/// test standing in for one) supplied it.
struct Delivery {
    memory: CollectingSink,
    files: Option<DirectoryReport>,
}

impl Delivery {
    /// The sink to hand to the session, and the handles to read it back.
    fn new(directory: Option<&Path>, stem: &'static str) -> (Box<dyn RecordSink>, Self) {
        let memory = CollectingSink::new();
        let collector: Box<dyn RecordSink> = Box::new(memory.clone());
        match directory {
            Some(directory) => {
                let files = DirectorySink::new(directory.to_path_buf(), stem);
                let report = files.report();
                (
                    Box::new(TeeSink::new(collector, Box::new(files))),
                    Self {
                        memory,
                        files: Some(report),
                    },
                )
            }
            None => (
                collector,
                Self {
                    memory,
                    files: None,
                },
            ),
        }
    }

    /// The one record the session delivered. When a directory was given the
    /// session wrote it there too, in both forms and exactly once.
    fn only(&self) -> MeasurementRecord {
        let mut records = self.memory.take();
        assert_eq!(records.len(), 1);
        if let Some(files) = &self.files {
            assert!(files.errors().is_empty(), "{:?}", files.errors());
            assert!(files.partial().is_empty());
            assert_eq!(files.written().len(), 1);
        }
        records.remove(0)
    }
}

fn begin(
    context: &RunContext,
    workload: &'static str,
    flow: FlowKind,
    root: &'static str,
    sink: Box<dyn RecordSink>,
) -> Session {
    Session::begin(
        RunIdentity::new(context.clone(), workload, flow, EMITTER),
        root,
        workers(),
        sink,
    )
}

fn labels(record: &MeasurementRecord) -> Vec<(&str, Option<u32>, u64, u64)> {
    record
        .phase_tree
        .nodes
        .iter()
        .map(|node| (node.label.as_str(), node.parent, node.calls, node.completed))
        .collect()
}

/// Findings other than an identity the harness did not bind.
fn findings_beyond_unbound_identity(record: &MeasurementRecord, bound: bool) -> Vec<Finding> {
    record
        .findings()
        .into_iter()
        .filter(|finding| {
            bound
                || !matches!(
                    finding,
                    Finding::IdentityIncomplete { .. } | Finding::DirtyDigestMismatch
                )
        })
        .collect()
}

fn canonical_frame(proof: &Proof) -> Vec<u8> {
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    norito::core::to_bytes_bounded(proof, FASTPQ_DEFAULT_MAX_PROOF_FRAME_BYTES_V1)
        .expect("proof frame fits the shared encoded bound")
}

/// Prove inside a measured proof flow and return the canonical proof frame.
///
/// A prover error is not a panic of the measurement: the flow is finished as
/// a failed run and its record is returned, and written for the harness, like
/// any other.
fn measured_prove(
    context: &RunContext,
    directory: Option<&Path>,
    batch: &TransitionBatch,
    batch_bytes: u64,
) -> Result<(Vec<u8>, MeasurementRecord), Box<MeasurementRecord>> {
    let (sink, delivery) = Delivery::new(directory, "fastpq-replay-prove");
    let session = begin(context, PROVE_WORKLOAD, FlowKind::Proof, "prove_flow", sink);
    session.record_bytes(ByteKind::PublicInput, "transition_batch", batch_bytes);
    let frames = session.allocation_counter("frame_buffers");

    let construction = session.enter("prover_construction");
    let prover = Prover::canonical_with_execution_mode(FASTPQ_FINAL_V1_ID, ExecutionMode::Cpu)
        .expect("canonical CPU prover");
    construction.complete();

    let proving = session.enter("prove_with_self_verification");
    let Ok(proof) = prover.prove(batch) else {
        // Only the stage and a fixed code are recorded, never the error.
        session.record_failure("prove_with_self_verification", "prover_error");
        drop(proving);
        session.finish();
        return Err(Box::new(delivery.only()));
    };
    proving.complete();

    let encoding = session.enter("proof_frame_encode");
    let frame = canonical_frame(&proof);
    frames.allocated(frame.len() as u64);
    session.record_bytes(ByteKind::Proof, "proof_frame", frame.len() as u64);
    // Releasing the in-memory proof belongs to this phase, not to the root.
    drop(proof);
    encoding.complete();

    session.finish();
    Ok((frame, delivery.only()))
}

/// Decode and verify inside a measured verification flow.
fn measured_verify(
    context: &RunContext,
    directory: Option<&Path>,
    workload: &'static str,
    batch: &TransitionBatch,
    frame: &[u8],
) -> (bool, MeasurementRecord) {
    let (sink, delivery) = Delivery::new(directory, "fastpq-replay-verify");
    let session = begin(
        context,
        workload,
        FlowKind::Verification,
        "verify_flow",
        sink,
    );
    session.record_bytes(ByteKind::Proof, "proof_frame", frame.len() as u64);

    let decoding = session.enter("proof_frame_decode");
    let decoded = norito::decode_from_bytes::<Proof>(frame);
    let Ok(proof) = decoded else {
        session.record_failure("proof_frame_decode", "malformed_frame");
        drop(decoding);
        session.finish();
        return (false, delivery.only());
    };
    decoding.complete();

    let verifying = session.enter("verify");
    let accepted = verify(batch, &proof).is_ok();
    if accepted {
        verifying.complete();
    } else {
        session.record_failure("verify", "rejected");
        drop(verifying);
    }
    session.finish();
    (accepted, delivery.only())
}

#[test]
fn eight_row_transfer_prove_and_verify_have_complete_phase_trees() {
    let (context, directory) = harness();
    let bound = directory.is_some();
    // Fixture construction is not part of either measured flow.
    let batch = transfer_fixture(8);
    let batch_bytes = norito::to_bytes(&batch).expect("canonical batch").len() as u64;

    let (frame, proof_record) = measured_prove(&context, directory.as_deref(), &batch, batch_bytes)
        .expect("the supported witnessed transfer must prove");
    let (accepted, verify_record) = measured_verify(
        &context,
        directory.as_deref(),
        VERIFY_WORKLOAD,
        &batch,
        &frame,
    );
    assert!(accepted, "the verifier accepts the generated proof");

    // Proof flow: one complete tree at the public-call granularity.
    assert_eq!(proof_record.identity.flow, FlowKind::Proof);
    assert_eq!(proof_record.identity.workload, PROVE_WORKLOAD);
    assert_eq!(proof_record.identity.context, context);
    assert_eq!(proof_record.outcome, RunOutcome::Succeeded);
    assert_eq!(
        labels(&proof_record),
        [
            ("prove_flow", None, 1, 1),
            ("prover_construction", Some(0), 1, 1),
            ("prove_with_self_verification", Some(0), 1, 1),
            ("proof_frame_encode", Some(0), 1, 1),
        ]
    );
    let attribution = proof_record.attribution().expect("root phase");
    // The rule is met at wrapper granularity: say how much of the root the
    // largest phase holds without any child phase dividing it.
    let undivided = proof_record
        .largest_undivided_phase()
        .expect("phases below the root");
    eprintln!(
        "fastpq_replay_prove root_wall_ns={} unattributed_wall_ns={} unattributed_ppm={} \
         largest_undivided_phase={} largest_undivided_ppm={} proof_frame_bytes={} \
         peak_rss_bytes={}",
        attribution.root_wall_ns,
        attribution.unattributed_wall_ns,
        attribution.unattributed_parts_per_million(),
        proof_record.phase_tree.nodes[undivided.phase as usize].label,
        undivided.share_parts_per_million(),
        frame.len(),
        proof_record.process.peak_rss_bytes,
    );
    assert_eq!(undivided.phase, 2, "prove_with_self_verification");
    assert!(undivided.share_parts_per_million() > 500_000);
    // At most 1% of the proving root is outside every phase.
    assert!(
        attribution.within_limit(),
        "{} ppm of the proving time is unattributed",
        attribution.unattributed_parts_per_million()
    );
    assert_eq!(findings_beyond_unbound_identity(&proof_record, bound), []);
    let root = &proof_record.phase_tree.nodes[0];
    let children: u64 = proof_record.phase_tree.nodes[1..]
        .iter()
        .map(|node| node.wall_inclusive_ns)
        .sum();
    assert_eq!(root.wall_exclusive_ns, root.wall_inclusive_ns - children);
    let proving = &proof_record.phase_tree.nodes[2];
    assert!(proving.wall_inclusive_ns > root.wall_inclusive_ns / 2);
    assert!(proving.process_cpu_window_inclusive_ns > 0);
    assert!(proof_record.process.peak_rss_bytes > 0);
    assert_eq!(
        proof_record.address_space.enforced,
        proof_record.address_space.soft_limit_bytes.is_some()
    );
    // Sizes are recorded, never the bytes.
    let sizes: Vec<_> = proof_record
        .byte_counters
        .entries
        .iter()
        .map(|entry| {
            (
                entry.kind,
                entry.label.as_str(),
                entry.phase,
                entry.total_bytes,
            )
        })
        .collect();
    assert_eq!(
        sizes,
        [
            (ByteKind::PublicInput, "transition_batch", 0, batch_bytes),
            (ByteKind::Proof, "proof_frame", 3, frame.len() as u64),
        ]
    );
    assert!(frame.len() > 512 * 1024, "a genuine proof frame");
    let source = &proof_record.allocations.sources[0];
    assert_eq!(
        (
            source.label.as_str(),
            source.allocations,
            source.live_bytes_high_water
        ),
        ("frame_buffers", 1, frame.len() as u64)
    );
    assert_eq!(
        proof_record.phase_tree.nodes[3].allocated_bytes,
        frame.len() as u64
    );
    assert!(unclassified_numbers(&proof_record.to_json_value()).is_empty());

    // Verification flow: measured separately, never part of the proving ratio.
    assert_eq!(verify_record.identity.flow, FlowKind::Verification);
    assert_eq!(verify_record.outcome, RunOutcome::Succeeded);
    assert_eq!(
        labels(&verify_record),
        [
            ("verify_flow", None, 1, 1),
            ("proof_frame_decode", Some(0), 1, 1),
            ("verify", Some(0), 1, 1),
        ]
    );
    let verification = verify_record.attribution().expect("root phase");
    eprintln!(
        "fastpq_replay_verify root_wall_ns={} unattributed_wall_ns={} unattributed_ppm={}",
        verification.root_wall_ns,
        verification.unattributed_wall_ns,
        verification.unattributed_parts_per_million(),
    );
    assert!(verification.within_limit());
    assert_eq!(findings_beyond_unbound_identity(&verify_record, bound), []);

    // Both wire forms of both records are exact and deterministic.
    for record in [&proof_record, &verify_record] {
        let bytes = record.to_norito_bytes().expect("framed record");
        assert_eq!(bytes, record.to_norito_bytes().unwrap());
        assert_eq!(
            &MeasurementRecord::from_norito_bytes(&bytes).unwrap(),
            record
        );
        assert_eq!(
            &MeasurementRecord::from_json_view(&record.to_json_view()).unwrap(),
            record
        );
    }
}

#[test]
fn rejected_proof_is_retained_as_a_failed_verification_record() {
    let (context, directory) = harness();
    let batch = transfer_fixture(8);
    let batch_bytes = norito::to_bytes(&batch).expect("canonical batch").len() as u64;
    // Only the rejected verification is the subject of this test: the proof
    // it needs is produced without writing a second record for the harness.
    let (frame, proof_record) = measured_prove(&context, None, &batch, batch_bytes)
        .expect("the supported witnessed transfer must prove");
    assert_eq!(proof_record.outcome, RunOutcome::Succeeded);

    // The same proof against a different statement must be rejected, and the
    // rejected run is a retained record, not a dropped one.
    let mut other = batch.clone();
    other.public_inputs.slot += 1;
    let (accepted, record) = measured_verify(
        &context,
        directory.as_deref(),
        REJECT_WORKLOAD,
        &other,
        &frame,
    );
    assert!(!accepted, "a proof for another statement is rejected");
    assert_eq!(record.outcome, RunOutcome::Failed);
    assert_eq!(record.failures.dropped, 0);
    assert_eq!(record.failures.entries.len(), 1);
    assert_eq!(record.failures.entries[0].stage, "verify");
    assert_eq!(record.failures.entries[0].code, "rejected");
    let (verify_index, verify_node) = record
        .phase_tree
        .nodes
        .iter()
        .enumerate()
        .find(|(_, node)| node.label == "verify")
        .expect("verify phase");
    assert_eq!(record.failures.entries[0].phase as usize, verify_index);
    assert_eq!((verify_node.completed, verify_node.interrupted), (0, 1));
    assert!(record.findings().contains(&Finding::RunNotSucceeded {
        outcome: RunOutcome::Failed
    }));

    // A malformed frame is likewise recorded at the stage that refused it.
    let (accepted, malformed) =
        measured_verify(&context, None, REJECT_WORKLOAD, &batch, &frame[..64]);
    assert!(!accepted);
    assert_eq!(malformed.outcome, RunOutcome::Failed);
    assert_eq!(malformed.failures.entries[0].stage, "proof_frame_decode");
    assert_eq!(malformed.failures.entries[0].code, "malformed_frame");
    assert_eq!(
        labels(&malformed),
        [
            ("verify_flow", None, 1, 1),
            ("proof_frame_decode", Some(0), 1, 0)
        ]
    );
}

/// Records the session wrote into `directory`, in the order they were
/// written, each checked against the JSON view written beside it. One
/// process writes one stem with increasing sequence numbers, so the file
/// names sort in write order.
fn written_records(directory: &Path) -> Vec<MeasurementRecord> {
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .expect("record directory")
        .map(|entry| entry.expect("record entry").path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "norito"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let record = MeasurementRecord::from_norito_bytes(&std::fs::read(path).unwrap())
                .expect("retained record");
            let view = std::fs::read_to_string(path.with_extension("json")).unwrap();
            assert_eq!(MeasurementRecord::from_json_view(&view).unwrap(), record);
            record
        })
        .collect()
}

#[test]
fn failed_abandoned_and_unwound_proving_flows_are_written_for_the_harness() {
    // A scratch directory stands in for the harness output directory, so the
    // records the session writes do not mix with an accepted harness run.
    let scratch = tempfile::tempdir().expect("scratch record directory");
    let directory = scratch.path();
    let context = RunContext::unbound();

    // A genuine prover error: the batch claims a post-state root its own
    // witnessed transfers do not produce.
    let mut inconsistent = transfer_fixture(8);
    inconsistent.public_inputs.new_root[0] ^= 1;
    let failed = *measured_prove(&context, Some(directory), &inconsistent, 0)
        .expect_err("a batch whose stated root differs from its witness must not prove");
    assert_eq!(failed.outcome, RunOutcome::Failed);
    assert_eq!(failed.failures.entries.len(), 1);
    assert_eq!(
        failed.failures.entries[0].stage,
        "prove_with_self_verification"
    );
    assert_eq!(failed.failures.entries[0].code, "prover_error");
    assert_eq!(
        labels(&failed),
        [
            ("prove_flow", None, 1, 1),
            ("prover_construction", Some(0), 1, 1),
            ("prove_with_self_verification", Some(0), 1, 0),
        ]
    );

    // A flow that returns early without finishing its session.
    let abandon = |directory: &Path| -> Result<(), ()> {
        let (sink, _delivery) = Delivery::new(Some(directory), "fastpq-replay-prove");
        let session = begin(
            &context,
            PROVE_WORKLOAD,
            FlowKind::Proof,
            "prove_flow",
            sink,
        );
        let _phase = session.enter("prover_construction");
        Err(())
    };
    assert!(abandon(directory).is_err());

    // A flow whose prover call panics.
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (sink, _delivery) = Delivery::new(Some(directory), "fastpq-replay-prove");
        let session = begin(
            &context,
            PROVE_WORKLOAD,
            FlowKind::Proof,
            "prove_flow",
            sink,
        );
        let _phase = session.enter("prove_with_self_verification");
        panic!("synthetic prover unwind");
    }));
    assert!(unwound.is_err());

    // All three are on disk in both forms, with the phases that were open.
    let records = written_records(directory);
    assert_eq!(
        records
            .iter()
            .map(|record| record.outcome)
            .collect::<Vec<_>>(),
        [
            RunOutcome::Failed,
            RunOutcome::Abandoned,
            RunOutcome::Unwound
        ]
    );
    assert_eq!(records[0], failed);
    assert_eq!(
        labels(&records[1]),
        [
            ("prove_flow", None, 1, 0),
            ("prover_construction", Some(0), 1, 0)
        ]
    );
    let unwound_phase = &records[2].phase_tree.nodes[1];
    assert_eq!(unwound_phase.label, "prove_with_self_verification");
    assert_eq!((unwound_phase.interrupted, unwound_phase.unwound), (1, 1));
    for record in &records {
        assert!(
            record
                .findings()
                .iter()
                .any(|finding| matches!(finding, Finding::RunNotSucceeded { .. }))
        );
    }
    assert_eq!(std::fs::read_dir(directory).unwrap().count(), 6);
}

#[test]
fn fixture_is_the_deterministic_eight_row_public_transfer() {
    let batch = transfer_fixture(8);
    assert_eq!(batch.transitions.len(), 8);
    assert_eq!(
        norito::to_bytes(&batch).unwrap(),
        norito::to_bytes(&transfer_fixture(8)).unwrap()
    );
    assert_ne!(batch.public_inputs.old_root, batch.public_inputs.new_root);
    let (context, directory) = harness();
    assert_eq!(context == RunContext::unbound(), directory.is_none());
    assert!(workers().workers >= 1);
}
