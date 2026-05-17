//! FASTPQ prover lane: converts execution witnesses into transition batches and
//! drives the Stage 6 prover in the background.

use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use fastpq_prover::{
    ExecutionMode as ProverExecutionMode, MetalOverrides,
    PoseidonExecutionMode as ProverPoseidonMode, Prover, TransitionBatch, apply_metal_overrides,
    set_metal_queue_policy,
};
use iroha_config::parameters::actual::{Fastpq, FastpqExecutionMode, FastpqPoseidonMode};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::{BlockHeader, consensus::ExecWitness};
use iroha_logger::{debug, info, warn};
use tokio::sync::mpsc;

use crate::{
    fastpq::{
        ENTRY_HASH_METADATA_KEY, FASTPQ_CANONICAL_PARAMETER_SET, FastpqWitnessContext,
        TranscriptBatchError, batches_from_bundles, batches_from_exec_witness,
    },
    kura::{FastpqProofSnapshot, Kura},
};

/// Handle used to submit FASTPQ prover jobs.
#[derive(Clone)]
pub struct FastpqLaneHandle {
    tx: mpsc::Sender<FastpqWitnessJob>,
    backpressure: Option<crate::queue::BackpressureHandle>,
    ready: Arc<AtomicBool>,
}

impl FastpqLaneHandle {
    /// Submit a prover job to the lane.
    pub fn submit(&self, job: FastpqWitnessJob) -> bool {
        if !self.ready.load(Ordering::Acquire) {
            debug!(
                height = job.height,
                view = job.view,
                "fastpq lane: deferring background prover job while backend is initialising"
            );
            return false;
        }
        if self
            .backpressure
            .as_ref()
            .is_some_and(|handle| handle.snapshot().is_saturated())
        {
            debug!(
                height = job.height,
                view = job.view,
                "fastpq lane: deferring background prover job while queue is saturated"
            );
            return false;
        }
        self.tx.try_send(job).is_ok()
    }

    #[cfg(test)]
    fn is_ready_for_test(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
}

/// Execution witness metadata forwarded to the prover lane.
#[derive(Clone)]
pub struct FastpqWitnessJob {
    /// Hash of the block this witness belongs to.
    pub block_hash: HashOf<BlockHeader>,
    /// Block height.
    pub height: u64,
    /// Consensus view.
    pub view: u64,
    /// Execution witness carrying FASTPQ transcripts/batches.
    pub witness: ExecWitness,
    /// Local-only batch construction context captured outside the witness wire payload.
    pub(crate) context: FastpqWitnessContext,
}

/// Proof bytes and digest produced by the FASTPQ lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastpqProofOutput {
    /// Norito-encoded FASTPQ proof payload.
    pub proof_bytes: Vec<u8>,
    /// Stable digest of `proof_bytes` for relay metadata and telemetry.
    pub proof_digest: Hash,
    /// Batch trace commitment proven by the proof.
    pub trace_commitment: Hash,
}

/// Trait abstracting over the FASTPQ prover backend so tests can inject mocks.
pub trait FastpqProofEngine: Send + Sync + 'static {
    /// Prove the supplied transition batch.
    ///
    /// # Errors
    /// Returns an error when the prover backend fails to generate a proof.
    fn prove(
        &self,
        batch: &fastpq_prover::TransitionBatch,
    ) -> fastpq_prover::Result<FastpqProofOutput>;
}

struct RealProofEngine {
    prover: Prover,
}

impl FastpqProofEngine for RealProofEngine {
    fn prove(
        &self,
        batch: &fastpq_prover::TransitionBatch,
    ) -> fastpq_prover::Result<FastpqProofOutput> {
        let proof = self.prover.prove(batch)?;
        let trace_commitment = proof.commitment();
        let proof_bytes = norito::to_bytes(&proof)?;
        let proof_digest = Hash::new(&proof_bytes);
        Ok(FastpqProofOutput {
            proof_bytes,
            proof_digest,
            trace_commitment,
        })
    }
}

static GLOBAL_SENDER: OnceLock<FastpqLaneHandle> = OnceLock::new();

#[cfg(test)]
static TEST_ENGINE: OnceLock<Arc<dyn FastpqProofEngine>> = OnceLock::new();

/// Start the FASTPQ prover lane. Returns the handle and the spawned task when successful.
pub fn start(cfg: &Fastpq) -> Option<(FastpqLaneHandle, tokio::task::JoinHandle<()>)> {
    start_with_backpressure(cfg, None, None)
}

/// Start the FASTPQ prover lane with optional queue backpressure and Kura proof persistence.
pub fn start_with_backpressure(
    cfg: &Fastpq,
    backpressure: Option<crate::queue::BackpressureHandle>,
    kura: Option<Arc<Kura>>,
) -> Option<(FastpqLaneHandle, tokio::task::JoinHandle<()>)> {
    crate::fastpq::configure_poseidon_digest_acceleration(cfg);
    if let Some(existing) = GLOBAL_SENDER.get() {
        return Some((existing.clone(), tokio::spawn(async {})));
    }
    let (tx, rx) = mpsc::channel::<FastpqWitnessJob>(32);
    let ready = Arc::new(AtomicBool::new(false));
    let handle = FastpqLaneHandle {
        tx: tx.clone(),
        backpressure,
        ready: Arc::clone(&ready),
    };
    if GLOBAL_SENDER.set(handle.clone()).is_err() {
        return Some((GLOBAL_SENDER.get().unwrap().clone(), tokio::spawn(async {})));
    }
    let cfg = cfg.clone();
    let task = spawn_worker(rx, ready, kura, move || build_engine(&cfg));
    Some((handle, task))
}

/// Submit a prover job if the lane is running.
pub fn try_submit(job: FastpqWitnessJob) -> bool {
    GLOBAL_SENDER.get().is_some_and(|handle| handle.submit(job))
}

fn build_engine(cfg: &Fastpq) -> Option<Arc<dyn FastpqProofEngine>> {
    #[cfg(test)]
    if let Some(engine) = TEST_ENGINE.get().cloned() {
        return Some(engine);
    }
    if let Err(err) = apply_metal_overrides(metal_overrides_from_config(cfg)) {
        warn!(%err, "fastpq lane: failed to apply Metal overrides");
    }
    if let Err(err) =
        set_metal_queue_policy(cfg.metal_queue_fanout, cfg.metal_queue_column_threshold)
    {
        warn!(%err, "fastpq lane: failed to apply Metal queue policy override");
    }
    let mode = map_execution_mode(cfg.execution_mode);
    let poseidon_mode = map_poseidon_mode(cfg.poseidon_mode);
    #[cfg(feature = "fastpq-gpu")]
    let (mode, poseidon_mode) = preflight_prover_modes(cfg, mode, poseidon_mode)?;
    match Prover::canonical_with_modes(FASTPQ_CANONICAL_PARAMETER_SET, mode, poseidon_mode) {
        Ok(prover) => Some(Arc::new(RealProofEngine { prover })),
        Err(err) => {
            warn!(?err, "fastpq lane: failed to construct canonical prover");
            None
        }
    }
}

#[cfg(feature = "fastpq-gpu")]
fn preflight_prover_modes(
    cfg: &Fastpq,
    mode: ProverExecutionMode,
    poseidon_mode: ProverPoseidonMode,
) -> Option<(ProverExecutionMode, ProverPoseidonMode)> {
    preflight_prover_modes_with_preflights(
        cfg,
        mode,
        poseidon_mode,
        fastpq_prover::preflight_poseidon_gpu_backend,
        fastpq_prover::preflight_bn254_poseidon_word_batches,
    )
}

#[cfg(feature = "fastpq-gpu")]
fn preflight_prover_modes_with_preflights(
    cfg: &Fastpq,
    mode: ProverExecutionMode,
    poseidon_mode: ProverPoseidonMode,
    prover_preflight: impl FnOnce() -> bool,
    digest_preflight: impl FnOnce() -> bool,
) -> Option<(ProverExecutionMode, ProverPoseidonMode)> {
    preflight_digest_acceleration(cfg, digest_preflight);
    if !should_preflight_poseidon(mode, poseidon_mode) {
        return Some((mode, poseidon_mode));
    }
    let started_at = Instant::now();
    let poseidon_ok = prover_preflight();
    info!(
        ok = poseidon_ok,
        elapsed_ms = started_at.elapsed().as_millis(),
        "fastpq lane: Poseidon GPU preflight completed"
    );
    if !poseidon_ok {
        if matches!(mode, ProverExecutionMode::Gpu)
            || matches!(poseidon_mode, ProverPoseidonMode::Gpu)
        {
            warn!("fastpq lane: explicit GPU prover backend failed preflight; lane disabled");
            return None;
        }
        warn!("fastpq lane: using CPU prover backend after Poseidon GPU preflight failed");
        return Some((ProverExecutionMode::Cpu, ProverPoseidonMode::Cpu));
    }
    Some((mode, poseidon_mode))
}

#[cfg(feature = "fastpq-gpu")]
fn preflight_digest_acceleration(cfg: &Fastpq, preflight: impl FnOnce() -> bool) {
    if !crate::fastpq::poseidon_digest_acceleration_configured(cfg) {
        crate::fastpq::set_poseidon_digest_acceleration_enabled(false);
        return;
    }
    let started_at = Instant::now();
    let ok = preflight();
    crate::fastpq::set_poseidon_digest_acceleration_enabled(ok);
    info!(
        ok,
        elapsed_ms = started_at.elapsed().as_millis(),
        "fastpq lane: BN254 Poseidon digest GPU preflight completed"
    );
}

fn spawn_worker(
    mut rx: mpsc::Receiver<FastpqWitnessJob>,
    ready: Arc<AtomicBool>,
    kura: Option<Arc<Kura>>,
    build_engine: impl FnOnce() -> Option<Arc<dyn FastpqProofEngine>> + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let engine = match tokio::task::spawn_blocking(build_engine).await {
            Ok(Some(engine)) => engine,
            Ok(None) => {
                warn!("fastpq lane: failed to initialise prover backend; lane disabled");
                return;
            }
            Err(err) => {
                warn!(
                    ?err,
                    "fastpq lane: prover backend initialisation task panicked"
                );
                return;
            }
        };
        ready.store(true, Ordering::Release);
        while let Some(job) = rx.recv().await {
            let engine = Arc::clone(&engine);
            let kura = kura.clone();
            if let Err(err) =
                tokio::task::spawn_blocking(move || process_job(&engine, &job, kura.as_deref()))
                    .await
            {
                warn!(?err, "fastpq lane: prover task panicked");
            }
        }
    })
}

fn metal_overrides_from_config(cfg: &Fastpq) -> MetalOverrides {
    MetalOverrides {
        max_in_flight: cfg.metal_max_in_flight,
        threadgroup_size: cfg.metal_threadgroup_width,
        dispatch_trace: cfg.metal_trace,
        debug_enum: cfg.metal_debug_enum,
        debug_fused: cfg.metal_debug_fused,
    }
}

fn map_execution_mode(mode: FastpqExecutionMode) -> ProverExecutionMode {
    match mode {
        FastpqExecutionMode::Auto => ProverExecutionMode::Auto,
        FastpqExecutionMode::Cpu => ProverExecutionMode::Cpu,
        FastpqExecutionMode::Gpu => ProverExecutionMode::Gpu,
    }
}

fn map_poseidon_mode(mode: FastpqPoseidonMode) -> ProverPoseidonMode {
    match mode {
        FastpqPoseidonMode::Auto => ProverPoseidonMode::Auto,
        FastpqPoseidonMode::Cpu => ProverPoseidonMode::Cpu,
        FastpqPoseidonMode::Gpu => ProverPoseidonMode::Gpu,
    }
}

#[cfg(feature = "fastpq-gpu")]
fn should_preflight_poseidon(mode: ProverExecutionMode, poseidon_mode: ProverPoseidonMode) -> bool {
    matches!(poseidon_mode, ProverPoseidonMode::Gpu)
        || (matches!(poseidon_mode, ProverPoseidonMode::Auto)
            && !matches!(mode, ProverExecutionMode::Cpu))
}

fn process_job(engine: &Arc<dyn FastpqProofEngine>, job: &FastpqWitnessJob, kura: Option<&Kura>) {
    if job.witness.fastpq_transcripts.is_empty() && job.witness.fastpq_batches.is_empty() {
        debug!(
            height = job.height,
            view = job.view,
            "fastpq lane: witness contains no transcripts"
        );
        return;
    }
    let batches = match batches_for_job(job) {
        Ok(batches) => batches,
        Err(err) => {
            warn!(
                height = job.height,
                view = job.view,
                ?err,
                "fastpq lane: failed to build batches"
            );
            return;
        }
    };
    if batches.is_empty() {
        debug!(
            height = job.height,
            view = job.view,
            "fastpq lane: no batches produced from witness"
        );
        return;
    }
    let batch_count = batches.len();
    let job_started = Instant::now();
    let mut proved = 0usize;
    let mut failed = 0usize;
    let mut persisted = 0usize;
    let mut transition_count = 0usize;
    for (idx, batch) in batches.into_iter().enumerate() {
        let entry_hash = entry_hash_for_batch(idx, &job.witness, &batch);
        let entry_hash_hex = entry_hash
            .map(|hash| hex::encode(hash.as_ref()))
            .unwrap_or_else(|| "unknown".to_string());
        transition_count = transition_count.saturating_add(batch.transitions.len());
        let started = Instant::now();
        match engine.prove(&batch) {
            Ok(output) => {
                proved = proved.saturating_add(1);
                if let Some((kura, entry_hash, batch_index, transition_count)) =
                    kura.zip(entry_hash).and_then(|(kura, entry_hash)| {
                        let batch_index = u32::try_from(idx).ok()?;
                        let transition_count = u32::try_from(batch.transitions.len()).ok()?;
                        Some((kura, entry_hash, batch_index, transition_count))
                    })
                {
                    kura.enqueue_fastpq_proof_snapshot(FastpqProofSnapshot {
                        height: job.height,
                        block_hash: job.block_hash,
                        entry_hash,
                        batch_index,
                        parameter: batch.parameter.clone(),
                        transition_count,
                        trace_commitment: output.trace_commitment,
                        proof_digest: output.proof_digest,
                        batch: batch.clone(),
                        proof: output.proof_bytes.clone(),
                    });
                    persisted = persisted.saturating_add(1);
                }
                debug!(
                    height = job.height,
                    view = job.view,
                    entry_hash = entry_hash_hex,
                    transitions = batch.transitions.len(),
                    proof_bytes = output.proof_bytes.len(),
                    proof_digest = ?output.proof_digest,
                    elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0,
                    "fastpq lane: generated proof"
                );
            }
            Err(err) => {
                failed = failed.saturating_add(1);
                warn!(
                    height = job.height,
                    view = job.view,
                    entry_hash = entry_hash_hex,
                    ?err,
                    "fastpq lane: prover error"
                );
            }
        }
    }
    info!(
        height = job.height,
        view = job.view,
        batch_count,
        proved,
        failed,
        persisted,
        transition_count,
        elapsed_ms = job_started.elapsed().as_secs_f64() * 1_000.0,
        "fastpq lane: processed prover job"
    );
}

fn entry_hash_for_batch(
    idx: usize,
    witness: &ExecWitness,
    batch: &fastpq_prover::TransitionBatch,
) -> Option<Hash> {
    if let Some(bundle) = witness.fastpq_transcripts.get(idx) {
        return Some(bundle.entry_hash);
    }
    let bytes = batch.metadata.get(ENTRY_HASH_METADATA_KEY)?;
    let digest: [u8; 32] = bytes.as_slice().try_into().ok()?;
    Some(Hash::prehashed(digest))
}

/// Install a deterministic FASTPQ engine for tests, bypassing the real prover backend.
///
/// This lets unit tests inject a mock [`FastpqProofEngine`] so the lane can
/// exercise batching logic without spawning the real GPU/CPU prover pipeline.
#[cfg(test)]
pub fn install_test_engine(engine: Arc<dyn FastpqProofEngine>) {
    let _ = TEST_ENGINE.set(engine);
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::atomic::AtomicBool, time::Duration};

    use crate::fastpq::{
        FastpqPublicInputsTemplate, authority_digest, batches_from_bundles, transition_batch_to_dto,
    };
    use iroha_data_model::domain::DomainId;
    use iroha_data_model::fastpq::{
        TransferDeltaTranscript, TransferTranscript, TransferTranscriptBundle,
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{ALICE_ID, BOB_ID};

    use super::*;

    #[tokio::test]
    async fn lane_processes_transcripts_with_mock_engine() {
        use tokio::time::{Instant, sleep};

        let calls = Arc::new(std::sync::Mutex::new(0usize));
        install_test_engine(Arc::new(MockEngine {
            calls: Arc::clone(&calls),
        }));
        let cfg = Fastpq {
            execution_mode: FastpqExecutionMode::Cpu,
            poseidon_mode: FastpqPoseidonMode::Cpu,
            device_class: None,
            chip_family: None,
            gpu_kind: None,
            metal_queue_fanout: None,
            metal_queue_column_threshold: None,
            metal_max_in_flight: None,
            metal_threadgroup_width: None,
            metal_trace: iroha_config::parameters::defaults::zk::fastpq::METAL_TRACE,
            metal_debug_enum: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_ENUM,
            metal_debug_fused: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_FUSED,
        };
        let (handle, _task) = {
            let _digest_lock = super::super::DIGEST_ACCELERATION_TEST_LOCK
                .lock()
                .expect("digest acceleration test lock poisoned");
            let previous = crate::fastpq::poseidon_digest_acceleration_enabled();
            let started = start(&cfg).expect("lane starts");
            crate::fastpq::set_poseidon_digest_acceleration_enabled(previous);
            started
        };
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if handle.is_ready_for_test() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "fastpq lane mock engine did not initialise"
            );
            sleep(Duration::from_millis(10)).await;
        }
        let bundle = sample_bundle();
        let template = FastpqPublicInputsTemplate {
            dsid: [0u8; 16],
            slot: 0,
            old_root: [0u8; 32],
            new_root: [0u8; 32],
            perm_root: [0u8; 32],
        };
        let tx_set_hash = [0x44; 32];
        let batches = batches_from_bundles(
            FASTPQ_CANONICAL_PARAMETER_SET,
            template,
            tx_set_hash,
            [&bundle],
        )
        .expect("batches");
        let witness = ExecWitness {
            reads: Vec::new(),
            writes: Vec::new(),
            fastpq_transcripts: vec![bundle],
            fastpq_batches: batches.iter().map(transition_batch_to_dto).collect(),
        };
        let job = FastpqWitnessJob {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAA; 32])),
            height: 42,
            view: 7,
            witness,
            context: FastpqWitnessContext::default(),
        };
        assert!(try_submit(job));
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if *calls.lock().unwrap() > 0 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "fastpq lane mock engine was not invoked"
            );
            sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn worker_start_does_not_wait_for_backend_initialisation() {
        use std::time::Instant as StdInstant;
        use tokio::time::sleep;

        let (_tx, rx) = mpsc::channel::<FastpqWitnessJob>(1);
        let ready = Arc::new(AtomicBool::new(false));
        let started_at = StdInstant::now();
        let task = spawn_worker(rx, Arc::clone(&ready), None, || {
            std::thread::sleep(Duration::from_millis(200));
            None
        });

        assert!(
            started_at.elapsed() < Duration::from_millis(100),
            "worker startup waited for backend initialisation"
        );
        assert!(!ready.load(Ordering::Acquire));
        sleep(Duration::from_millis(250)).await;
        assert!(!ready.load(Ordering::Acquire));
        task.await.expect("worker task joins");
    }

    #[test]
    fn job_context_builds_batches_for_transcript_only_witness() {
        let bundle = sample_bundle();
        let template = FastpqPublicInputsTemplate {
            dsid: [0u8; 16],
            slot: 123,
            old_root: [0x11; 32],
            new_root: [0x22; 32],
            perm_root: [0x33; 32],
        };
        let tx_set_hash = [0x44; 32];
        let dsid = [0x55; 16];
        let mut entry_dataspaces = BTreeMap::new();
        entry_dataspaces.insert(bundle.entry_hash, dsid);
        let witness = ExecWitness {
            reads: Vec::new(),
            writes: Vec::new(),
            fastpq_transcripts: vec![bundle],
            fastpq_batches: Vec::new(),
        };
        let job = FastpqWitnessJob {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAA; 32])),
            height: 42,
            view: 7,
            witness,
            context: FastpqWitnessContext {
                public_inputs: Some(template),
                tx_set_hash: Some(tx_set_hash),
                entry_dataspaces,
            },
        };

        let batches = batches_for_job(&job).expect("context builds batches");

        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].public_inputs.dsid, dsid);
        assert_eq!(batches[0].public_inputs.tx_set_hash, tx_set_hash);
        assert_eq!(batches[0].public_inputs.perm_root, template.perm_root);
    }

    #[test]
    #[cfg(feature = "fastpq-gpu")]
    fn prover_poseidon_preflight_failure_preserves_digest_acceleration() {
        let _digest_lock = super::super::DIGEST_ACCELERATION_TEST_LOCK
            .lock()
            .expect("digest acceleration test lock poisoned");
        let previous = crate::fastpq::poseidon_digest_acceleration_enabled();
        crate::fastpq::set_poseidon_digest_acceleration_enabled(false);
        let cfg = Fastpq {
            execution_mode: FastpqExecutionMode::Auto,
            poseidon_mode: FastpqPoseidonMode::Auto,
            device_class: None,
            chip_family: None,
            gpu_kind: None,
            metal_queue_fanout: None,
            metal_queue_column_threshold: None,
            metal_max_in_flight: None,
            metal_threadgroup_width: None,
            metal_trace: iroha_config::parameters::defaults::zk::fastpq::METAL_TRACE,
            metal_debug_enum: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_ENUM,
            metal_debug_fused: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_FUSED,
        };

        let (mode, poseidon_mode) = preflight_prover_modes_with_preflights(
            &cfg,
            ProverExecutionMode::Auto,
            ProverPoseidonMode::Auto,
            || false,
            || true,
        )
        .expect("auto GPU preflight may resolve to CPU");

        assert!(matches!(mode, ProverExecutionMode::Cpu));
        assert!(matches!(poseidon_mode, ProverPoseidonMode::Cpu));
        assert!(
            crate::fastpq::poseidon_digest_acceleration_enabled(),
            "BN254 digest acceleration must stay enabled after the prover lane falls back to CPU"
        );
        crate::fastpq::set_poseidon_digest_acceleration_enabled(previous);
    }

    #[derive(Clone)]
    struct MockEngine {
        calls: Arc<std::sync::Mutex<usize>>,
    }

    impl FastpqProofEngine for MockEngine {
        fn prove(
            &self,
            batch: &fastpq_prover::TransitionBatch,
        ) -> fastpq_prover::Result<FastpqProofOutput> {
            *self.calls.lock().unwrap() += 1;
            let _ = &batch.parameter;
            let proof_bytes = b"mock-fastpq-proof".to_vec();
            Ok(FastpqProofOutput {
                proof_digest: Hash::new(&proof_bytes),
                trace_commitment: Hash::new(b"mock-fastpq-trace-commitment"),
                proof_bytes,
            })
        }
    }

    fn sample_bundle() -> TransferTranscriptBundle {
        TransferTranscriptBundle {
            entry_hash: Hash::prehashed([0x11; 32]),
            transcripts: vec![TransferTranscript {
                batch_hash: Hash::prehashed([0x22; 32]),
                deltas: vec![TransferDeltaTranscript {
                    from_account: (*ALICE_ID).clone(),
                    to_account: (*BOB_ID).clone(),
                    asset_definition: iroha_data_model::asset::AssetDefinitionId::new(
                        DomainId::try_new("wonderland", "universal").unwrap(),
                        "rose".parse().unwrap(),
                    ),
                    amount: Numeric::from(10u32),
                    from_balance_before: Numeric::from(100u32),
                    from_balance_after: Numeric::from(90u32),
                    to_balance_before: Numeric::from(5u32),
                    to_balance_after: Numeric::from(15u32),
                    from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
                    to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
                }],
                authority_digest: authority_digest(&ALICE_ID),
                poseidon_preimage_digest: None,
            }],
        }
    }

    #[test]
    fn maps_config_to_metal_overrides() {
        let cfg = Fastpq {
            execution_mode: FastpqExecutionMode::Gpu,
            poseidon_mode: FastpqPoseidonMode::Gpu,
            device_class: None,
            chip_family: None,
            gpu_kind: None,
            metal_queue_fanout: None,
            metal_queue_column_threshold: None,
            metal_max_in_flight: Some(8),
            metal_threadgroup_width: Some(256),
            metal_trace: true,
            metal_debug_enum: true,
            metal_debug_fused: true,
        };

        let overrides = metal_overrides_from_config(&cfg);
        assert_eq!(overrides.max_in_flight, Some(8));
        assert_eq!(overrides.threadgroup_size, Some(256));
        assert!(overrides.dispatch_trace);
        assert!(overrides.debug_enum);
        assert!(overrides.debug_fused);
    }
}

fn batches_for_job(job: &FastpqWitnessJob) -> Result<Vec<TransitionBatch>, TranscriptBatchError> {
    match batches_from_exec_witness(&job.witness) {
        Ok(batches) => return Ok(batches),
        Err(TranscriptBatchError::MissingFastpqBatches) => {}
        Err(err) => return Err(err),
    }

    if job.witness.fastpq_transcripts.is_empty() {
        return Ok(Vec::new());
    }

    let Some(public_inputs) = job.context.public_inputs else {
        return Err(TranscriptBatchError::MissingFastpqBatches);
    };
    let Some(tx_set_hash) = job.context.tx_set_hash else {
        return Err(TranscriptBatchError::MissingFastpqBatches);
    };

    let mut batches = batches_from_bundles(
        FASTPQ_CANONICAL_PARAMETER_SET,
        public_inputs,
        tx_set_hash,
        job.witness.fastpq_transcripts.iter(),
    )?;
    for (bundle, batch) in job
        .witness
        .fastpq_transcripts
        .iter()
        .zip(batches.iter_mut())
    {
        if let Some(dsid) = job.context.entry_dataspaces.get(&bundle.entry_hash) {
            batch.public_inputs.dsid = *dsid;
        }
    }
    Ok(batches)
}
